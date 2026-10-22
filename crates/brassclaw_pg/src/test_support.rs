//! Actual loopback PostgreSQL wire transport for native caller tests. It forwards
//! real bytes and can withhold a selected actual acknowledgement. It never
//! executes SQL, manufactures a result, interprets a task or logs credentials.
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
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

use crate::PgPool;

#[derive(Clone, Copy)]
pub enum AckBoundary {
    Commit,
    IdleReady,
}

/// Caller-owned test selectors, not application admission or success evidence.
pub struct AckCutRule {
    id: u8,
    bind: Option<BindSelector>,
    boundary: AckBoundary,
}
struct BindSelector {
    required: Vec<Vec<u8>>,
    forbidden: Vec<Vec<u8>>,
}
impl AckCutRule {
    /// The private test pool must issue only the intended transaction while
    /// armed. Authentication and ordinary frames are still forwarded unchanged.
    pub fn next_commit(id: u8) -> Self {
        Self {
            id,
            bind: None,
            boundary: AckBoundary::Commit,
        }
    }
    pub fn after_bind(
        id: u8,
        required: Vec<Vec<u8>>,
        forbidden: Vec<Vec<u8>>,
        boundary: AckBoundary,
    ) -> Self {
        Self {
            id,
            bind: Some(BindSelector {
                required,
                forbidden,
            }),
            boundary,
        }
    }
    fn valid(&self) -> bool {
        self.id != 0
            && self.bind.as_ref().is_none_or(|selector| {
                !selector.required.is_empty()
                    && selector
                        .required
                        .iter()
                        .chain(&selector.forbidden)
                        .all(|needle| !needle.is_empty())
            })
    }
    fn matches(&self, payload: &[u8]) -> bool {
        self.bind.as_ref().is_some_and(|selector| {
            let contains =
                |needle: &[u8]| payload.windows(needle.len()).any(|window| window == needle);
            selector.required.iter().map(Vec::as_slice).all(contains)
                && !selector.forbidden.iter().map(Vec::as_slice).any(contains)
        })
    }
}

/// Owns the listener, forwarding connections and explicit shutdown. Product test
/// callers supply byte selectors and observe real database/coordinator results.
/// The connection string remains private and has no Debug implementation.
pub struct AckCutTransport {
    pub pool: Arc<PgPool>,
    connection: String,
    rules: Arc<BTreeMap<u8, AckCutRule>>,
    armed: Arc<AtomicU8>,
    cut: Arc<AtomicUsize>,
    cancel: CancellationToken,
    task: Option<tokio::task::JoinHandle<io::Result<()>>>,
}

async fn loopback_upstream(endpoint: &url::Url, port: u16) -> io::Result<Vec<SocketAddr>> {
    let denied = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "test proxy requires a loopback upstream",
        )
    };
    let addresses = match endpoint.host() {
        Some(url::Host::Ipv4(host)) => vec![SocketAddr::new(IpAddr::V4(host), port)],
        Some(url::Host::Ipv6(host)) => vec![SocketAddr::new(IpAddr::V6(host), port)],
        Some(url::Host::Domain("localhost")) => tokio::net::lookup_host(("localhost", port))
            .await?
            .collect(),
        // Non-special schemes, including PostgreSQL, preserve an IPv4 literal
        // as a domain string. Parse that literal without resolving other names.
        Some(url::Host::Domain(host)) => {
            vec![SocketAddr::new(host.parse().map_err(|_| denied())?, port)]
        }
        None => return Err(denied()),
    };
    if addresses.is_empty() || addresses.iter().any(|address| !address.ip().is_loopback()) {
        return Err(denied());
    }
    Ok(addresses)
}

impl AckCutTransport {
    pub async fn start(connection: &str, rules: Vec<AckCutRule>) -> io::Result<Self> {
        let mut ids = BTreeSet::new();
        if rules.is_empty()
            || rules
                .iter()
                .any(|rule| !rule.valid() || !ids.insert(rule.id))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid ACK cut rules",
            ));
        }
        let mut endpoint = url::Url::parse(connection).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid test PostgreSQL URL")
        })?;
        if !matches!(endpoint.scheme(), "postgres" | "postgresql") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid test PostgreSQL scheme",
            ));
        }
        if endpoint
            .query_pairs()
            .any(|(key, _)| matches!(key.as_ref(), "host" | "hostaddr" | "port"))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "test proxy address overrides are not supported",
            ));
        }
        let port = endpoint.port().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "explicit test PostgreSQL port required",
            )
        })?;
        let upstream = loopback_upstream(&endpoint, port).await?;
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        endpoint
            .set_host(Some("127.0.0.1"))
            .map_err(|_| io::Error::other("test proxy URL host failed"))?;
        endpoint
            .set_port(Some(listener.local_addr()?.port()))
            .map_err(|_| io::Error::other("test proxy URL port failed"))?;
        let parameters: Vec<_> = endpoint
            .query_pairs()
            .filter(|(key, _)| key != "sslmode")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        endpoint.set_query(None);
        endpoint
            .query_pairs_mut()
            .extend_pairs(parameters)
            .append_pair("sslmode", "disable");
        let pool = Arc::new(
            crate::pool::build_pool(endpoint.as_str())
                .map_err(|_| io::Error::other("test proxy pool failed"))?,
        );
        let rules: Arc<BTreeMap<_, _>> =
            Arc::new(rules.into_iter().map(|rule| (rule.id, rule)).collect());
        let armed = Arc::new(AtomicU8::new(0));
        let cut = Arc::new(AtomicUsize::new(0));
        let cancel = CancellationToken::new();
        let stopped = cancel.clone();
        let next = armed.clone();
        let observed = cut.clone();
        let selected = rules.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = stopped.cancelled() => {
                        connections.shutdown().await;
                        return Ok(());
                    },
                    accepted = listener.accept() => {
                        let (downstream, _) = accepted?;
                        let upstream = tokio::select! {
                            _ = stopped.cancelled() => {
                                connections.shutdown().await;
                                return Ok(());
                            },
                            connected = TcpStream::connect(upstream.as_slice()) => connected?,
                        };
                        downstream.set_nodelay(true)?;
                        upstream.set_nodelay(true)?;
                        connections.spawn(relay(downstream, upstream, selected.clone(), next.clone(), observed.clone()));
                    },
                    finished = connections.join_next(), if !connections.is_empty() => {
                        finished.ok_or_else(|| io::Error::other("missing wire task"))?
                            .map_err(|_| io::Error::other("wire task failed"))??;
                    },
                }
            }
        });
        let transport = Self {
            pool,
            connection: endpoint.to_string(),
            rules,
            armed,
            cut,
            cancel,
            task: Some(task),
        };
        // Complete actual driver authentication before allowing a cut.
        drop(
            transport
                .pool
                .get()
                .await
                .map_err(|_| io::Error::other("test proxy authentication failed"))?,
        );
        Ok(transport)
    }
    pub fn connection_string(&self) -> &str {
        &self.connection
    }
    pub fn arm(&self, id: u8) -> io::Result<()> {
        if !self.rules.contains_key(&id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown ACK cut rule",
            ));
        }
        self.armed
            .compare_exchange(0, id, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| io::Error::other("ACK cut already armed"))
    }
    pub fn cut_count(&self) -> usize {
        self.cut.load(Ordering::Acquire)
    }
    pub async fn shutdown(mut self) -> io::Result<()> {
        self.cancel.cancel();
        self.task
            .take()
            .ok_or_else(|| io::Error::other("missing wire task"))?
            .await
            .map_err(|_| io::Error::other("wire task failed"))?
    }
}
impl Drop for AckCutTransport {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn payload_length(header: &[u8; 4]) -> io::Result<usize> {
    usize::try_from(
        u32::from_be_bytes(*header).checked_sub(4).ok_or_else(|| {
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
    rules: Arc<BTreeMap<u8, AckCutRule>>,
    armed: Arc<AtomicU8>,
    cut: Arc<AtomicUsize>,
) -> io::Result<()> {
    let (mut server, mut to_server) = upstream.into_split();
    let (mut client, mut to_client) = downstream.into_split();
    let matched = AtomicU8::new(0);
    let requests = async {
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
            let phase = armed.load(Ordering::Acquire);
            if tag == b'B' && rules.get(&phase).is_some_and(|rule| rule.matches(&payload)) {
                matched.store(phase, Ordering::Release);
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
            let phase = armed.load(Ordering::Acquire);
            let rule = rules
                .get(&phase)
                .filter(|rule| rule.bind.is_none() || matched.load(Ordering::Acquire) == phase);
            let acknowledged = rule.is_some_and(|rule| match rule.boundary {
                AckBoundary::Commit => tag == b'C' && payload == b"COMMIT\0",
                AckBoundary::IdleReady => tag == b'Z' && payload == b"I",
            });
            if acknowledged
                && armed
                    .compare_exchange(phase, 0, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                cut.fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                    count.checked_add(1)
                })
                .map_err(|_| io::Error::other("ACK cut counter exhausted"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn postgres_literal_loopbacks_resolve_without_domain_lookup() {
        for (connection, ip) in [
            ("postgres://test:private@127.0.0.1:5432/test", "127.0.0.1"),
            ("postgresql://test:private@127.0.0.2:5432/test", "127.0.0.2"),
            ("postgres://test:private@[::1]:5432/test", "::1"),
        ] {
            let endpoint = url::Url::parse(connection).unwrap();
            assert_eq!(
                loopback_upstream(&endpoint, 5432).await.unwrap(),
                vec![SocketAddr::new(ip.parse().unwrap(), 5432)]
            );
        }
    }

    #[test]
    fn bind_selectors_require_every_literal_and_exclude_forbidden_bytes() {
        let rule = AckCutRule::after_bind(
            1,
            vec![b"first\0".to_vec(), "ü🙂".as_bytes().to_vec()],
            vec![b"denied".to_vec()],
            AckBoundary::Commit,
        );
        assert!(rule.valid());
        assert!(rule.matches("first\0 data ü🙂".as_bytes()));
        assert!(!rule.matches(b"first\0 only"));
        assert!(!rule.matches("first\0 ü🙂 denied".as_bytes()));
    }

    #[tokio::test]
    async fn invalid_rules_schemes_and_nonloopback_upstreams_reject_before_connecting() {
        let local = "postgres://test:private@127.0.0.1:1/test";
        for (url, rules) in [
            (local, vec![]),
            (local, vec![AckCutRule::next_commit(0)]),
            (
                local,
                vec![AckCutRule::next_commit(1), AckCutRule::next_commit(1)],
            ),
            (
                local,
                vec![AckCutRule::after_bind(
                    1,
                    vec![],
                    vec![],
                    AckBoundary::Commit,
                )],
            ),
            (
                local,
                vec![AckCutRule::after_bind(
                    1,
                    vec![vec![]],
                    vec![],
                    AckBoundary::Commit,
                )],
            ),
            (
                local,
                vec![AckCutRule::after_bind(
                    1,
                    vec![b"x".to_vec()],
                    vec![vec![]],
                    AckBoundary::Commit,
                )],
            ),
            ("http://127.0.0.1:1/test", vec![AckCutRule::next_commit(1)]),
            (
                "postgres://test:private@127.0.0.1:1/test?host=localhost",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@127.0.0.1:1/test?hostaddr=127.0.0.1",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@127.0.0.1:1/test?port=2",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@203.0.113.1:1/test",
                vec![AckCutRule::next_commit(1)],
            ),
            (
                "postgres://test:private@[2001:db8::1]:1/test",
                vec![AckCutRule::next_commit(1)],
            ),
        ] {
            let Err(error) = AckCutTransport::start(url, rules).await else {
                panic!("invalid wire fixture must not open a connection")
            };
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }
}
