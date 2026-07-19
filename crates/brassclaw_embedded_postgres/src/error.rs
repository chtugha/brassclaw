use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbeddedPostgresError {
    #[error("I/O error at {path}: {reason}")]
    Io { path: String, reason: String },

    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("initdb failed: {0}")]
    InitDb(String),

    #[error("pg_ctl failed: {0}")]
    PgCtl(String),

    #[error("embedded PG port {port} in use — set BRASSCLAW_PG_URL or BRASSCLAW_EMBEDDED_PG_PORT")]
    PortInUse { port: u16 },

    #[error("health check timed out after {attempts} attempts on port {port}")]
    HealthCheckTimeout { port: u16, attempts: u32 },

    #[error("failed to spawn child process: {0}")]
    Spawn(String),

    #[error("pgvector library not found in {path}: {reason}")]
    PgvectorMissing { path: String, reason: String },
}
