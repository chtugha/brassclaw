//! Real PostgreSQL fault fixture: withhold one COMMIT CommandComplete after the
//! server committed. No query or application result is synthesized.
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;

pub(crate) struct CommitResponseProxy {
    pub(crate) port: u16,
    armed: Arc<AtomicBool>,
    dropped: Arc<tokio::sync::Notify>,
    stop: CancellationToken,
    owner: Option<JoinHandle<()>>,
}
impl CommitResponseProxy {
    pub(crate) async fn start(database_port: u16) -> Self {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let armed = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(tokio::sync::Notify::new());
        let stop = CancellationToken::new();
        let (task_armed, task_dropped, task_stop) = (armed.clone(), dropped.clone(), stop.clone());
        let owner = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    () = task_stop.cancelled() => break,
                    incoming = listener.accept() => {
                        let (client, _) = incoming.unwrap();
                        let (armed, dropped, stop) = (task_armed.clone(), task_dropped.clone(), task_stop.clone());
                        connections.spawn(async move {
                            let database = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, database_port)).await?;
                            let (mut client_read, mut client_write) = client.into_split();
                            let (mut server_read, mut server_write) = database.into_split();
                            let upload = tokio::io::copy(&mut client_read, &mut server_write);
                            let download = async {
                                loop {
                                    let tag = server_read.read_u8().await?;
                                    let length = server_read.read_u32().await?;
                                    // Test server messages are framed PostgreSQL, not TLS.
                                    // Catch a corrupt/unexpected protocol before allocation.
                                    assert!((4..=16 * 1024 * 1024).contains(&length));
                                    let mut payload = vec![0; (length - 4) as usize];
                                    server_read.read_exact(&mut payload).await?;
                                    if tag == b'C' && payload == b"COMMIT\0"
                                        && armed.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                                        dropped.notify_one();
                                        return Ok::<(), io::Error>(());
                                    }
                                    client_write.write_u8(tag).await?;
                                    client_write.write_u32(length).await?;
                                    client_write.write_all(&payload).await?;
                                }
                            };
                            tokio::select! {
                                () = stop.cancelled() => Ok(()),
                                result = upload => result.map(|_| ()),
                                result = download => result,
                            }
                        });
                    }
                    Some(result) = connections.join_next(), if !connections.is_empty() => check(result),
                }
            }
            while let Some(result) = connections.join_next().await {
                check(result);
            }
        });
        Self {
            port,
            armed,
            dropped,
            stop,
            owner: Some(owner),
        }
    }
    pub(crate) fn arm(&self) {
        assert!(!self.armed.swap(true, Ordering::SeqCst));
    }
    pub(crate) async fn dropped_response(&self) {
        self.dropped.notified().await;
    }
    pub(crate) async fn shutdown(mut self) {
        self.stop.cancel();
        self.owner.take().unwrap().await.unwrap();
    }
}
impl Drop for CommitResponseProxy {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
fn check(result: Result<io::Result<()>, tokio::task::JoinError>) {
    if let Err(error) = result.expect("fault-proxy task must not panic") {
        // Closing a client or deliberately withholding its COMMIT closes both
        // halves. Other transport failures are test failures, not ignored logs.
        assert!(
            matches!(
                error.kind(),
                io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
            ),
            "unexpected PostgreSQL fault-proxy transport error: {error}"
        );
    }
}
