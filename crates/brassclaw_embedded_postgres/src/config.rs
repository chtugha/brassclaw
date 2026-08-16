use std::path::{Path, PathBuf};

/// Configuration for the embedded PostgreSQL instance.
#[derive(Debug, Clone)]
pub struct EmbeddedPostgresConfig {
    /// TCP port the embedded Postgres server listens on.
    /// Default: 5434 (avoids colliding with a system Postgres on 5432).
    /// Configurable via `BRASSCLAW_EMBEDDED_PG_PORT`.
    pub port: u16,

    /// Directory where the PostgreSQL data cluster lives.
    /// Created by `initdb` on first start.
    /// Default: `$REBORN_HOME/postgres/data`
    pub data_dir: PathBuf,

    /// Directory where the downloaded PostgreSQL binaries are cached.
    /// Default: `$REBORN_HOME/postgres/bin`
    pub bin_cache_dir: PathBuf,

    /// Name of the Postgres database and role created on first run.
    /// Default: `brassclaw`
    pub database: String,

    /// Name of the Postgres superuser created during initdb.
    /// Default: `brassclaw`
    pub superuser: String,
}

/// Default port for the embedded Postgres instance when `BRASSCLAW_EMBEDDED_PG_PORT` is unset.
pub const DEFAULT_EMBEDDED_PG_PORT: u16 = 5434;

/// Default listen addresses for the embedded Postgres instance.
///
/// The default is `127.0.0.1` (loopback-only). Set
/// `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES=0.0.0.0` before first boot to
/// allow connections from the local network. This is a first-boot setting:
/// `postgresql.conf` is written by `initdb` and is not modified on restart;
/// to change it on an existing cluster, edit `$REBORN_HOME/postgres/data/postgresql.conf`
/// directly and restart the service.
pub const DEFAULT_EMBEDDED_PG_LISTEN_ADDRESSES: &str = "127.0.0.1";

impl EmbeddedPostgresConfig {
    /// Construct a config from a `$REBORN_HOME` base directory.
    pub fn from_reborn_home(home: &Path) -> Self {
        let port = std::env::var("BRASSCLAW_EMBEDDED_PG_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(DEFAULT_EMBEDDED_PG_PORT);

        Self {
            port,
            data_dir: home.join("postgres").join("data"),
            bin_cache_dir: home.join("postgres").join("bin"),
            database: "brassclaw".to_string(),
            superuser: "brassclaw".to_string(),
        }
    }

    /// The PostgreSQL connection URL for the embedded server.
    pub fn connection_url(&self) -> String {
        format!(
            "postgresql://{}@127.0.0.1:{}/{}",
            self.superuser, self.port, self.database
        )
    }
}
