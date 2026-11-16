//! Containment before exec succeeds, never retry of a worker or its operation.
use std::{fmt, io, time::Duration};

use tokio::{
    process::{Child, Command},
    time::{Instant, sleep_until},
};

// Tokio timers have millisecond resolution. Yield one timer tick on executable
// write contention instead of spinning; the caller's existing deadline remains
// the sole lifetime bound, with no additional budget or attempt quota.
const CONTENTION_TICK: Duration = Duration::from_millis(1);

pub(crate) struct StartFailure {
    pub(crate) cause: io::Error,
    pub(crate) deadline_elapsed: bool,
}

impl fmt::Debug for StartFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StartFailure")
            .field("kind", &self.cause.kind())
            .field("os_code", &self.cause.raw_os_error())
            .field("deadline_elapsed", &self.deadline_elapsed)
            .finish()
    }
}

/// Callers configure only trusted executable paths, without pre-exec callbacks.
/// ETXTBSY rejects exec before any worker body/request can run. In particular,
/// neither an ordinary spawn error nor a successfully started child is retried.
pub(crate) async fn start(command: &mut Command, deadline: Instant) -> Result<Child, StartFailure> {
    let mut last_busy = None;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(StartFailure {
                cause: last_busy.unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "executable start deadline elapsed")
                }),
                deadline_elapsed: true,
            });
        }
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(cause) if cause.kind() == io::ErrorKind::ExecutableFileBusy => {
                last_busy = Some(cause);
                let next = Instant::now()
                    .checked_add(CONTENTION_TICK)
                    .unwrap_or(deadline)
                    .min(deadline);
                sleep_until(next).await;
            }
            Err(cause) => {
                return Err(StartFailure {
                    cause,
                    deadline_elapsed: false,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    fn listing_command(path: &std::path::Path) -> Command {
        let mut command = Command::new(path);
        command
            .arg("--list")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }

    #[tokio::test]
    async fn actual_start_returns_one_child_and_its_real_output() {
        let mut command = listing_command(&std::env::current_exe().unwrap());
        let child = start(&mut command, Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
        let output = child.wait_with_output().await.unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8(output.stdout).unwrap().contains(
            "executable_start::tests::actual_start_returns_one_child_and_its_real_output"
        ));
    }

    #[tokio::test]
    async fn missing_executable_returns_actual_cause_before_deadline() {
        let directory = tempfile::tempdir().unwrap();
        let mut command = listing_command(&directory.path().join("missing"));
        let failure = start(&mut command, Instant::now() + Duration::from_secs(2))
            .await
            .unwrap_err();
        assert!(!failure.deadline_elapsed);
        assert_eq!(failure.cause.kind(), io::ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn expired_deadline_never_attempts_start() {
        let mut command = listing_command(&std::env::current_exe().unwrap());
        let failure = start(&mut command, Instant::now()).await.unwrap_err();
        assert!(failure.deadline_elapsed);
        assert_eq!(failure.cause.kind(), io::ErrorKind::TimedOut);
    }

    // These cases require Linux's real executable-write exclusion. Their
    // acceptance must run on a native kernel, not be inferred from a container
    // that does not implement ETXTBSY (see the owning CI subplan).
    #[cfg(target_os = "linux")]
    mod write_exclusion {
        use super::*;
        use std::{fs::File, future::Future, os::unix::fs::PermissionsExt, task::Poll};

        fn held_image() -> (tempfile::TempDir, std::path::PathBuf, File) {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("executable");
            std::fs::copy(std::env::current_exe().unwrap(), &path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let writer = File::options().write(true).open(&path).unwrap();
            (directory, path, writer)
        }

        async fn require_pending<F: Future>(future: std::pin::Pin<&mut F>) {
            let mut future = future;
            std::future::poll_fn(|cx| match future.as_mut().poll(cx) {
                Poll::Pending => Poll::Ready(()),
                Poll::Ready(_) => panic!("exec must remain pending while the real writer is held"),
            })
            .await;
        }

        #[tokio::test]
        async fn real_writer_release_allows_exactly_one_successful_child() {
            let (_directory, path, writer) = held_image();
            let mut command = listing_command(&path);
            let pending = start(&mut command, Instant::now() + Duration::from_secs(2));
            tokio::pin!(pending);
            require_pending(pending.as_mut()).await;
            drop(writer);
            let child = pending.await.unwrap();
            let output = child.wait_with_output().await.unwrap();
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            let listing = String::from_utf8(output.stdout).unwrap();
            assert_eq!(
                listing
                    .matches("real_writer_release_allows_exactly_one_successful_child: test")
                    .count(),
                1
            );
        }

        #[tokio::test]
        async fn permanent_writer_is_bounded_and_retains_actual_os_cause() {
            let (_directory, path, _writer) = held_image();
            let mut command = listing_command(&path);
            let deadline = Instant::now() + Duration::from_millis(50);
            let failure = start(&mut command, deadline).await.unwrap_err();
            assert!(failure.deadline_elapsed);
            assert!(Instant::now() >= deadline);
            assert_eq!(failure.cause.kind(), io::ErrorKind::ExecutableFileBusy);
            assert!(failure.cause.raw_os_error().is_some());
        }

        #[tokio::test]
        async fn cancellation_does_not_leave_a_detached_start() {
            let (directory, path, writer) = held_image();
            let output_path = directory.path().join("listing");
            let mut command = listing_command(&path);
            command.stdout(File::create(&output_path).unwrap());
            {
                let pending = start(&mut command, Instant::now() + Duration::from_secs(2));
                tokio::pin!(pending);
                require_pending(pending.as_mut()).await;
                // Dropping the sole future cancels its timer. There is no
                // independently spawned task that could execute after release.
            }
            drop(writer);
            assert_eq!(std::fs::metadata(&output_path).unwrap().len(), 0);
            let status = command.spawn().unwrap().wait().await.unwrap();
            assert!(status.success());
            assert_eq!(
                std::fs::read_to_string(output_path)
                    .unwrap()
                    .matches("cancellation_does_not_leave_a_detached_start: test")
                    .count(),
                1
            );
        }
    }
}
