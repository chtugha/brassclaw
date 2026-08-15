use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

pub mod checksums;
pub mod config;
pub mod download;
pub mod error;
pub mod extract;
pub mod health;
pub mod initdb;
pub mod pgctl;

pub use config::EmbeddedPostgresConfig;
pub use error::EmbeddedPostgresError;

/// A managed embedded PostgreSQL instance.
///
/// Lifecycle:
/// 1. Call `ManagedPostgres::start(config)` — this downloads (if needed),
///    verifies the checksum, runs `initdb` (if needed), detects orphaned
///    servers, and either reuses or starts the server.
/// 2. Use `connection_url()` to build a connection pool.
/// 3. Before process exit, call `shutdown().await` to stop the server cleanly
///    *after* closing the connection pool. `Drop` provides a best-effort
///    fallback only.
pub struct ManagedPostgres {
    config: EmbeddedPostgresConfig,
    pg_bin_dir: std::path::PathBuf,
    /// True if this instance owns the running server (started it).
    /// False if a pre-existing server was detected and reused.
    owns_server: bool,
    /// Mutex guarding the shutdown path so concurrent callers do not
    /// double-stop the server.
    shutdown_lock: Arc<Mutex<bool>>,
}

impl ManagedPostgres {
    /// Start (or reuse) the embedded PostgreSQL server.
    ///
    /// # Steps
    ///
    /// 1. Suppress `POSTGRESQL_VERSION` / `GITHUB_TOKEN` env vars.
    /// 2. Download and verify the PG 16 binary if not already cached.
    /// 3. Check whether the port is already in use:
    ///    - If yes, check `postmaster.pid` — if the PID is alive, reuse the
    ///      running server (return with `owns_server = false`).
    ///    - If yes and PID is dead (stale), abort: the port is in use by
    ///      something else.
    /// 4. Run `initdb` (skipped if data dir is non-empty).
    /// 5. Start the server and wait for it to accept connections.
    pub async fn start(config: EmbeddedPostgresConfig) -> Result<Self, EmbeddedPostgresError> {
        // Step 1: suppress env vars that could alter the downloaded version.
        download::suppress_postgresql_embedded_env();

        // Step 2: resolve the binary directory. We use `postgresql_embedded`
        // to handle the download and caching, then locate the `bin/` directory.
        let pg_install_dir = resolve_pg_install_dir(&config).await?;
        let pg_bin_dir = pg_install_dir.join("bin");

        // Step 3: check for an existing server on the target port.
        if health::is_port_in_use(config.port).await {
            // Something is already on the port. Check if it's our server.
            if let Some(_pid) = health::check_postmaster_pid(&config.data_dir).await {
                debug!(
                    port = config.port,
                    "detected live embedded Postgres server; reusing"
                );
                return Ok(Self {
                    config,
                    pg_bin_dir,
                    owns_server: false,
                    shutdown_lock: Arc::new(Mutex::new(false)),
                });
            }
            // Port in use but PID is dead or no postmaster.pid — abort.
            return Err(EmbeddedPostgresError::PortInUse { port: config.port });
        }

        // Remove stale postmaster.pid if the PID is dead.
        let pid_file = config.data_dir.join("postmaster.pid");
        if pid_file.exists()
            && health::check_postmaster_pid(&config.data_dir)
                .await
                .is_none()
        {
            debug!("removing stale postmaster.pid");
            if let Err(e) = tokio::fs::remove_file(&pid_file).await {
                // Log but do not abort — pg_ctl start will report a clear error
                // if the stale file truly blocks startup.
                debug!(
                    path = %pid_file.display(),
                    error = %e,
                    "could not remove stale postmaster.pid; continuing"
                );
            }
        }

        // Step 4: run initdb if needed. PG_VERSION is written by initdb as the
        // very first file — its absence means this is a fresh cluster.
        let is_fresh_cluster = !config.data_dir.join("PG_VERSION").exists();
        initdb::run_initdb(&config, &pg_bin_dir).await?;

        // Step 5: start the server.
        let ctl = pgctl::PgCtl::new(&pg_bin_dir, &config.data_dir, config.port);
        ctl.start().await?;
        health::wait_for_ready(config.port).await?;

        // Step 6: on a fresh cluster, create the role and database. initdb
        // creates the superuser but not the application database/role.
        if is_fresh_cluster {
            create_app_db(&config).await?;
        }

        Ok(Self {
            config,
            pg_bin_dir,
            owns_server: true,
            shutdown_lock: Arc::new(Mutex::new(false)),
        })
    }

    /// The connection URL for the embedded Postgres server.
    pub fn connection_url(&self) -> String {
        self.config.connection_url()
    }

    /// Gracefully shut down the embedded Postgres server.
    ///
    /// Must be called **after** closing the connection pool. Calling this while
    /// open connections exist can cause a hang because `pg_ctl stop -m fast`
    /// waits for active transactions to complete.
    ///
    /// If this instance did not start the server (it was reused), this is a
    /// no-op.
    pub async fn shutdown(&self) -> Result<(), EmbeddedPostgresError> {
        if !self.owns_server {
            return Ok(());
        }

        let mut guard = self.shutdown_lock.lock().await;
        if *guard {
            // Already shut down.
            return Ok(());
        }
        *guard = true;

        let ctl = pgctl::PgCtl::new(&self.pg_bin_dir, &self.config.data_dir, self.config.port);
        ctl.stop().await
    }
}

impl Drop for ManagedPostgres {
    fn drop(&mut self) {
        // Best-effort fallback: attempt an immediate stop. This MUST NOT be
        // the primary shutdown path — a blocking `pg_ctl stop` with open pool
        // connections can deadlock. The composition root must call `shutdown()`
        // explicitly after closing the pool.
        if !self.owns_server {
            return;
        }

        let already_shutdown = self.shutdown_lock.try_lock().map(|g| *g).unwrap_or(true); // If the lock is held, shutdown is in progress.

        if already_shutdown {
            return;
        }

        // Use eprintln! — warn! in Drop can fire mid-operation and corrupts the
        // terminal UI (AGENTS.md §67).  eprintln! is always visible regardless of
        // the tracing subscriber state.
        eprintln!(
            "brassclaw-embedded-postgres: ManagedPostgres dropped without calling \
             shutdown() — attempting immediate stop"
        );
        let ctl = pgctl::PgCtl::new(&self.pg_bin_dir, &self.config.data_dir, self.config.port);
        ctl.stop_immediate_blocking();
    }
}

/// Resolve the PostgreSQL installation directory, extracting the compile-time
/// embedded archive if the binaries are not yet cached.
///
/// This replaces the old `postgresql_embedded::setup()` download path.
/// Postgres binaries are now baked into the binary at compile time via
/// `build.rs` + `include_bytes!`, so no outbound HTTP is required at runtime.
async fn resolve_pg_install_dir(
    config: &EmbeddedPostgresConfig,
) -> Result<std::path::PathBuf, EmbeddedPostgresError> {
    // Extract from embedded bytes if initdb is not already present.
    extract::ensure_pg_extracted(&config.bin_cache_dir).await?;
    Ok(config.bin_cache_dir.clone())
}

/// Create the application role and database on a freshly initialised cluster.
///
/// `initdb` creates only the superuser (named after `config.superuser`).
/// The application connection URL uses a role and database both named
/// `config.database` — these must be created before migrations can run.
async fn create_app_db(config: &EmbeddedPostgresConfig) -> Result<(), EmbeddedPostgresError> {
    use tokio_postgres::NoTls;

    // Connect as the superuser to the default "postgres" maintenance database
    // (created by initdb). We cannot connect to config.database yet because
    // that is the database we are about to create.
    let connect_url = format!(
        "postgresql://{}@127.0.0.1:{}/postgres",
        config.superuser, config.port
    );

    let (client, connection) = tokio_postgres::connect(&connect_url, NoTls)
        .await
        .map_err(|e| {
            EmbeddedPostgresError::InitDb(format!("could not connect for init SQL: {e}"))
        })?;

    // Drive the connection in the background.
    tokio::spawn(async move {
        let _ = connection.await;
    });

    // CREATE ROLE … LOGIN is idempotent-guarded; skip if it already exists.
    let role_exists: bool = client
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname = $1)",
            &[&config.database],
        )
        .await
        .map(|row| row.get(0))
        .unwrap_or(false);

    if !role_exists {
        client
            .execute(&format!("CREATE ROLE {} LOGIN", config.database), &[])
            .await
            .map_err(|e| EmbeddedPostgresError::InitDb(format!("CREATE ROLE failed: {e}")))?;
        debug!(role = config.database, "created application role");
    }

    // CREATE DATABASE … is idempotent-guarded; skip if it already exists.
    let db_exists: bool = client
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)",
            &[&config.database],
        )
        .await
        .map(|row| row.get(0))
        .unwrap_or(false);

    if !db_exists {
        // CREATE DATABASE cannot run inside a transaction block; use execute directly.
        client
            .execute(
                &format!(
                    "CREATE DATABASE {} OWNER {}",
                    config.database, config.database
                ),
                &[],
            )
            .await
            .map_err(|e| EmbeddedPostgresError::InitDb(format!("CREATE DATABASE failed: {e}")))?;
        debug!(database = config.database, "created application database");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    // Serialise all tests that mutate `BRASSCLAW_EMBEDDED_PG_PORT` so they
    // cannot race with each other when run with the default parallel harness.
    // SAFETY: this is the only lock guarding this env var in this test binary.
    static ENV_PORT_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn config_from_reborn_home() {
        let _guard = ENV_PORT_LOCK.lock().expect("env lock poisoned");
        // SAFETY: under ENV_PORT_LOCK; no other thread touches this var.
        unsafe { std::env::remove_var("BRASSCLAW_EMBEDDED_PG_PORT") };
        let home = std::path::Path::new("/tmp/test-reborn-home");
        let config = EmbeddedPostgresConfig::from_reborn_home(home);
        assert_eq!(config.port, 5434);
        assert_eq!(config.data_dir, home.join("postgres/data"));
        assert_eq!(config.bin_cache_dir, home.join("postgres/bin"));
        assert_eq!(config.database, "brassclaw");
        assert_eq!(
            config.connection_url(),
            "postgresql://brassclaw@127.0.0.1:5434/brassclaw"
        );
    }

    #[test]
    fn config_port_from_env() {
        let _guard = ENV_PORT_LOCK.lock().expect("env lock poisoned");
        // SAFETY: under ENV_PORT_LOCK; no other thread touches this var.
        unsafe { std::env::set_var("BRASSCLAW_EMBEDDED_PG_PORT", "5999") };
        let home = std::path::Path::new("/tmp/test-reborn-home");
        let config = EmbeddedPostgresConfig::from_reborn_home(home);
        assert_eq!(config.port, 5999);
        unsafe { std::env::remove_var("BRASSCLAW_EMBEDDED_PG_PORT") };
    }
}
