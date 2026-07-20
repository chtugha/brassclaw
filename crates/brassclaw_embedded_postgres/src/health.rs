use std::path::Path;

use tokio::net::TcpStream;
use tokio::time::{Duration, sleep};
use tracing::debug;

use crate::error::EmbeddedPostgresError;

/// Maximum number of TCP connection attempts before giving up.
const MAX_ATTEMPTS: u32 = 60;
/// Delay between connection attempts.
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// Attempt to establish a TCP connection to the Postgres port, retrying until
/// the server accepts connections or the attempt limit is reached.
pub async fn wait_for_ready(port: u16) -> Result<(), EmbeddedPostgresError> {
    let addr = format!("127.0.0.1:{port}");
    for attempt in 1..=MAX_ATTEMPTS {
        match TcpStream::connect(&addr).await {
            Ok(_) => {
                debug!(port, attempt, "embedded Postgres is ready");
                return Ok(());
            }
            Err(_) => {
                debug!(port, attempt, "waiting for embedded Postgres to start…");
                sleep(RETRY_DELAY).await;
            }
        }
    }
    Err(EmbeddedPostgresError::HealthCheckTimeout {
        port,
        attempts: MAX_ATTEMPTS,
    })
}

/// Check whether the given TCP port is already in use (server already listening).
/// Used to detect a running server before starting a new one.
pub async fn is_port_in_use(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    TcpStream::connect(&addr).await.is_ok()
}

/// Read `postmaster.pid` from the data directory and check if the recorded
/// PID is still alive via `kill -0`.
///
/// Returns:
/// - `Some(pid)` if the file exists and the PID is alive
/// - `None` if the file is absent or the PID is dead
pub async fn check_postmaster_pid(data_dir: &Path) -> Option<u32> {
    let pid_file = data_dir.join("postmaster.pid");
    let contents = tokio::fs::read_to_string(&pid_file).await.ok()?;
    let pid: u32 = contents.lines().next()?.trim().parse().ok()?;

    // Safety: kill(pid, 0) checks existence without sending a signal.
    // POSIX guarantees errno=ESRCH when the process does not exist.
    let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
    if alive { Some(pid) } else { None }
}
