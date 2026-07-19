use std::path::Path;

use tokio::process::Command;
use tracing::{debug, warn};

use crate::error::EmbeddedPostgresError;

/// Wrapper around `pg_ctl` for starting, stopping, and querying a PostgreSQL
/// cluster managed by brassclaw.
pub struct PgCtl {
    pg_ctl_bin: std::path::PathBuf,
    data_dir: std::path::PathBuf,
    port: u16,
}

impl PgCtl {
    /// Create a `PgCtl` wrapper for the given data directory and binary.
    pub fn new(pg_bin_dir: &Path, data_dir: &Path, port: u16) -> Self {
        Self {
            pg_ctl_bin: pg_bin_dir.join("pg_ctl"),
            data_dir: data_dir.to_path_buf(),
            port,
        }
    }

    /// Start the Postgres server in background mode.
    pub async fn start(&self) -> Result<(), EmbeddedPostgresError> {
        let log_path = self.data_dir.join("log").join("postgres-start.log");
        if let Some(parent) = log_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let output = Command::new(&self.pg_ctl_bin)
            .args([
                "start",
                "-D",
                &self.data_dir.display().to_string(),
                "-l",
                &log_path.display().to_string(),
                "-o",
                &format!("-p {}", self.port),
                "-w",
            ])
            .output()
            .await
            .map_err(|e| EmbeddedPostgresError::Spawn(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(EmbeddedPostgresError::PgCtl(format!(
                "start failed: {stderr}"
            )));
        }
        debug!(port = self.port, "embedded Postgres started");
        Ok(())
    }

    /// Stop the Postgres server using `pg_ctl stop -m fast`.
    /// This is the normal shutdown path — fast mode waits for active
    /// transactions to complete rather than killing them immediately.
    pub async fn stop(&self) -> Result<(), EmbeddedPostgresError> {
        let output = Command::new(&self.pg_ctl_bin)
            .args([
                "stop",
                "-D",
                &self.data_dir.display().to_string(),
                "-m",
                "fast",
                "-w",
            ])
            .output()
            .await
            .map_err(|e| EmbeddedPostgresError::Spawn(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(EmbeddedPostgresError::PgCtl(format!(
                "stop failed: {stderr}"
            )));
        }
        debug!("embedded Postgres stopped");
        Ok(())
    }

    /// Stop the Postgres server using `pg_ctl stop -m immediate` (ungraceful).
    /// Use only as a last resort from `Drop`.
    pub fn stop_immediate_blocking(&self) {
        let result = std::process::Command::new(&self.pg_ctl_bin)
            .args([
                "stop",
                "-D",
                &self.data_dir.display().to_string(),
                "-m",
                "immediate",
            ])
            .status();
        match result {
            Ok(s) if s.success() => debug!("embedded Postgres immediate stop succeeded"),
            Ok(s) => warn!("embedded Postgres immediate stop exited with {s}"),
            Err(e) => warn!("embedded Postgres immediate stop failed: {e}"),
        }
    }
}
