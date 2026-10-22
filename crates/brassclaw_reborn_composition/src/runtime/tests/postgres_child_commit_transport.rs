//! Byte-preserving PostgreSQL transport; cuts actual child write acknowledgements.
//! It supplies no database result and never fabricates a coordinator response.
use brassclaw_pg::PgPool;
use brassclaw_secrets::SecretMaterial;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy)]
pub(super) enum ChildWrite {
    EnsureThread = 1,
    AcceptInput = 2,
    AdmitRun = 3,
}

pub(super) struct ChildCommitTransport {
    pub(super) pool: Arc<PgPool>,
    pub(super) url: SecretMaterial,
    armed: Arc<AtomicU8>,
    cut: Arc<AtomicUsize>,
    cancel: CancellationToken,
    task: Option<tokio::task::JoinHandle<io::Result<()>>>,
}

impl ChildCommitTransport {
    pub(super) async fn start(connection: &str) -> Self {
        let mut endpoint = url::Url::parse(connection).unwrap();
        let upstream = (
            endpoint.host_str().unwrap().to_owned(),
            endpoint.port().unwrap(),
        );
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        endpoint.set_host(Some("127.0.0.1")).unwrap();
        endpoint
            .set_port(Some(listener.local_addr().unwrap().port()))
            .unwrap();
        let parameters = endpoint
            .query_pairs()
            .filter(|(key, _)| key != "sslmode")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        endpoint.set_query(None);
        endpoint
            .query_pairs_mut()
            .extend_pairs(parameters)
            .append_pair("sslmode", "disable");
        let armed = Arc::new(AtomicU8::new(0));
        let cut = Arc::new(AtomicUsize::new(0));
        let cancel = CancellationToken::new();
        let stopped = cancel.clone();
        let next = armed.clone();
        let observed = cut.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = stopped.cancelled() => return Ok(()),
                    accepted = listener.accept() => {
                        let (downstream, _) = accepted?;
                        let upstream = TcpStream::connect((upstream.0.as_str(), upstream.1)).await?;
                        downstream.set_nodelay(true)?;
                        upstream.set_nodelay(true)?;
                        connections.spawn(relay(downstream, upstream, next.clone(), observed.clone()));
                    },
                    finished = connections.join_next(), if !connections.is_empty() => {
                        finished.expect("nonempty connection set").expect("transport task")?;
                    },
                }
            }
        });
        let pool = Arc::new(brassclaw_pg::pool::build_pool(endpoint.as_str()).unwrap());
        Self {
            pool,
            url: SecretMaterial::from(endpoint.to_string()),
            armed,
            cut,
            cancel,
            task: Some(task),
        }
    }

    pub(super) fn arm(&self, phase: ChildWrite) {
        assert_eq!(self.armed.swap(phase as u8, Ordering::AcqRel), 0);
    }
    pub(super) fn cut_count(&self) -> usize {
        self.cut.load(Ordering::Acquire)
    }
    pub(super) async fn shutdown(mut self) {
        self.cancel.cancel();
        self.task.take().unwrap().await.unwrap().unwrap();
    }
}
impl Drop for ChildCommitTransport {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}
fn payload_length(header: &[u8]) -> io::Result<usize> {
    let length = u32::from_be_bytes(header.try_into().expect("four-byte length"));
    usize::try_from(
        length.checked_sub(4).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid PostgreSQL frame")
        })?,
    )
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unrepresentable PostgreSQL frame",
        )
    })
}
async fn relay(
    downstream: TcpStream,
    upstream: TcpStream,
    armed: Arc<AtomicU8>,
    cut: Arc<AtomicUsize>,
) -> io::Result<()> {
    let (mut server, mut to_server) = upstream.into_split();
    let (mut client, mut to_client) = downstream.into_split();
    let matched = AtomicU8::new(0);
    let requests = async {
        // sslmode=disable: the actual client starts with its untyped startup
        // packet, then uses ordinary typed frontend frames including auth.
        let mut length = [0_u8; 4];
        client.read_exact(&mut length).await?;
        let mut startup = vec![0; payload_length(&length)?];
        client.read_exact(&mut startup).await?;
        to_server.write_all(&length).await?;
        to_server.write_all(&startup).await?;
        loop {
            let tag = match client.read_u8().await {
                Ok(tag) => tag,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            };
            client.read_exact(&mut length).await?;
            let mut payload = vec![0; payload_length(&length)?];
            client.read_exact(&mut payload).await?;
            // Select the current closed fixture's actual write by record
            // shape. These bytes choose a transport cut, never prove admission
            // or supply data/authority to the runtime. Never log the payload.
            let phase = armed.load(Ordering::Acquire);
            if tag == b'B' {
                let target = match phase {
                    1 => {
                        contains(&payload, b"monty-child-v1-")
                            && contains(&payload, b"inbound_idempotency")
                            && contains(&payload, b"messages")
                            && !contains(&payload, b"monty-child-v1:")
                    }
                    2 => {
                        contains(&payload, b"monty-child-v1:")
                            && contains(&payload, b"inbound_idempotency")
                            && !contains(&payload, b"claimed_child_reservation_released")
                    }
                    3 => {
                        contains(&payload, b"monty-child-v1:")
                            && contains(&payload, b"claimed_child_reservation_released")
                    }
                    _ => false,
                };
                if target {
                    matched.store(phase, Ordering::Release);
                }
            }
            to_server.write_u8(tag).await?;
            to_server.write_all(&length).await?;
            to_server.write_all(&payload).await?;
        }
    };
    let answers = async {
        loop {
            let tag = match server.read_u8().await {
                Ok(tag) => tag,
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            };
            let mut length = [0_u8; 4];
            server.read_exact(&mut length).await?;
            let mut payload = vec![0; payload_length(&length)?];
            server.read_exact(&mut payload).await?;
            let phase = matched.load(Ordering::Acquire);
            let committed = if phase == ChildWrite::EnsureThread as u8 {
                // Autocommit is complete at idle ReadyForQuery. The actual
                // tokio-postgres execute waits for this packet after INSERT's
                // CommandComplete, so withholding it loses the real ACK.
                tag == b'Z' && payload == b"I"
            } else {
                phase != 0 && tag == b'C' && payload == b"COMMIT\0"
            };
            if committed
                && armed
                    .compare_exchange(phase, 0, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                cut.fetch_add(1, Ordering::AcqRel);
                to_client.shutdown().await?;
                return Ok(());
            }
            if tag == b'E' || (tag == b'C' && payload == b"ROLLBACK\0") {
                matched.store(0, Ordering::Release);
            }
            to_client.write_u8(tag).await?;
            to_client.write_all(&length).await?;
            to_client.write_all(&payload).await?;
        }
    };
    tokio::select! { result=requests => result, result=answers => result }
}
