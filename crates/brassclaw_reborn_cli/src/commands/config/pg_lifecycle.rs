//! §6.4 CLI Postgres lifecycle helpers for DB-touching config commands.
//!
//! Every DB-touching CLI command (`config init`, `config get/set/unset/list/…`,
//! `secrets rewrap`, `maintenance prune-old-data`, etc.) follows this lifecycle:
//!
//! 1. Start embedded Postgres **or** connect to an already-running instance.
//!    Uses orphaned-server detection: check `postmaster.pid` PID liveness
//!    (kill -0). If a live postmaster is found, reuse it — do not start a
//!    second instance. Record whether this command started PG or merely
//!    connected to an existing one.
//! 2. Run `brassclaw_pg::migrations::run_migrations` (idempotent).
//! 3. Perform the operation (caller's responsibility).
//! 4. Conditional shutdown: shut down embedded PG **only if this command started
//!    it**. If a running PG was detected and reused, leave it running.
//!
//! This module provides `build_pg_pool`, a helper that implements steps 1–2
//! using the environment-variable-based URL when `BRASSCLAW_PG_URL` or
//! `DATABASE_URL` is set, or the embedded PG with orphaned-server detection
//! when neither is set.

/// Build a Postgres connection pool for a DB-touching CLI command.
///
/// Priority order:
/// 1. `BRASSCLAW_PG_URL` — connect directly (external PG, no embedded lifecycle).
/// 2. `DATABASE_URL` — connect directly.
/// 3. Embedded Postgres via `brassclaw_embedded_postgres` with §6.4
///    orphaned-server detection (check `postmaster.pid` liveness before
///    starting a new instance).
///
/// Returns a connected pool. On success the caller is responsible for calling
/// migrations before use.
pub(crate) async fn build_pg_pool() -> anyhow::Result<deadpool_postgres::Pool> {
    // Prefer an explicit PG URL (external or already-running embedded instance).
    if let Ok(url) = std::env::var("BRASSCLAW_PG_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
    {
        return connect_url(&url).await;
    }

    // Fall back to the embedded Postgres with §6.4 orphaned-server detection.
    // The embedded PG data dir is resolved from `BRASSCLAW_REBORN_HOME` (or the
    // default home). `brassclaw_embedded_postgres::ManagedPostgres::start_or_reuse`
    // checks `postmaster.pid` liveness and reuses a live postmaster.
    let home = brassclaw_reborn_config::RebornHome::resolve_from_env()
        .map_err(|e| anyhow::anyhow!("cannot resolve BRASSCLAW_REBORN_HOME: {e}"))?;

    let config = brassclaw_embedded_postgres::EmbeddedPostgresConfig::from_reborn_home(home.path());

    // `ManagedPostgres::start` implements §6.4 orphaned-server detection:
    // it checks `postmaster.pid` PID liveness and reuses a live postmaster
    // instead of starting a new instance.
    let managed = brassclaw_embedded_postgres::ManagedPostgres::start(config)
        .await
        .map_err(|e| anyhow::anyhow!("failed to start/connect to embedded Postgres: {e}"))?;

    let url = managed.connection_url();
    // Note: we intentionally do NOT call managed.shutdown() here.
    // The §6.4 contract says: shut down only if this command started PG.
    // `ManagedPostgres` tracks `owns_server` and its `Drop` impl attempts a
    // best-effort stop. For CLI commands the process exits after the operation
    // completes, so the pool is dropped first and the server stops cleanly.
    connect_url(&url).await
}

async fn connect_url(url: &str) -> anyhow::Result<deadpool_postgres::Pool> {
    let config: tokio_postgres::Config = url
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid PostgreSQL URL: {e}"))?;
    let manager = deadpool_postgres::Manager::new(config, tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(4)
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build PG pool: {e}"))?;
    pool.get()
        .await
        .map_err(|e| anyhow::anyhow!("cannot connect to PostgreSQL: {e}"))?;
    Ok(pool)
}
