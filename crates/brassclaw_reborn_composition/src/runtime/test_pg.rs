//! Shared Postgres testcontainer rig for the `runtime` unit tests.
//!
//! `build_reborn_runtime` is fail-closed on a missing Postgres pool (commit
//! `0ba4899f` — "postgres is mandatory; in-memory fallback removed"). The
//! runtime unit tests therefore run on the production *hybrid* path
//! (`RebornStorageInput::Postgres` + a per-test `reborn_home` tempdir), the
//! only path that now wires a pool into `services`.
//!
//! One Postgres-16 container is started lazily, the full schema (V000–V051)
//! is applied once, and the pool is cached in a `tokio::sync::OnceCell`. Each
//! test acquires `lock_db()` so the idempotent migration re-runs inside
//! `build_reborn_services` and the test bodies never overlap on the shared
//! database. When docker/testcontainers is unavailable, [`pg_rig`] returns
//! `None` and callers skip cleanly (matching `tests/postgres_substrate.rs`).

#![allow(dead_code)]

use std::sync::Arc;

use tokio::sync::{Mutex, MutexGuard, OnceCell};

use brassclaw_pg::PgPool;
use brassclaw_secrets::SecretMaterial;

use crate::input::RebornBuildInput;

type PostgresContainer = testcontainers_modules::testcontainers::ContainerAsync<
    testcontainers_modules::postgres::Postgres,
>;

/// One shared Postgres testcontainer + pool + URL + DB-serialization lock.
pub(crate) struct PgRig {
    pub pool: Arc<PgPool>,
    pub url: SecretMaterial,
    db_lock: Mutex<()>,
    _container: PostgresContainer,
}

static PG_RIG: OnceCell<Arc<PgRig>> = OnceCell::const_new();

/// Lazily start the shared Postgres testcontainer, run all migrations once,
/// and cache the rig. Returns `None` when docker/testcontainers is unavailable.
pub(crate) async fn pg_rig() -> Option<Arc<PgRig>> {
    match PG_RIG
        .get_or_try_init(|| async { start_and_migrate().await })
        .await
    {
        Ok(rig) => Some(Arc::clone(rig)),
        Err(()) => None,
    }
}

async fn start_and_migrate() -> Result<Arc<PgRig>, ()> {
    use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

    let image = testcontainers_modules::postgres::Postgres::default()
        .with_db_name("brassclaw_test")
        .with_user("postgres")
        .with_password("postgres")
        .with_tag("16-alpine");
    let container = image.start().await.map_err(|error| {
        eprintln!(
            "skipping runtime Postgres unit tests: docker/testcontainers unavailable ({error})"
        );
    })?;
    let host = container.get_host().await.map_err(|error| {
        eprintln!(
            "skipping runtime Postgres unit tests: could not resolve container host ({error})"
        );
    })?;
    let port = container.get_host_port_ipv4(5432).await.map_err(|error| {
        eprintln!(
            "skipping runtime Postgres unit tests: could not resolve container port ({error})"
        );
    })?;
    let database_url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");

    let config: deadpool_postgres::tokio_postgres::Config = database_url
        .parse()
        .expect("testcontainer database URL must parse");
    let manager = deadpool_postgres::Manager::new(config, deadpool_postgres::tokio_postgres::NoTls);
    let pool = deadpool_postgres::Pool::builder(manager)
        .max_size(8)
        .build()
        .expect("Postgres pool must build");
    let _connection = pool.get().await.map_err(|error| {
        eprintln!(
            "skipping runtime Postgres unit tests: testcontainer refused a connection ({error})"
        );
    })?;

    let pool_arc = Arc::new(pool);
    // Run the full schema once, serialized inside this init, so the idempotent
    // re-runs inside `build_reborn_services` never race on refinery_schema_history.
    brassclaw_pg::migrations::run_migrations(&pool_arc)
        .await
        .expect("testcontainer schema migrations must succeed");

    Ok(Arc::new(PgRig {
        pool: pool_arc,
        url: SecretMaterial::from(database_url),
        db_lock: Mutex::new(()),
        _container: container,
    }))
}

impl PgRig {
    /// Acquire the shared DB lock. Tests hold this guard across
    /// `build_reborn_runtime` and the test body so migration re-runs and test
    /// bodies never overlap on the shared database.
    pub(crate) async fn lock_db(&self) -> MutexGuard<'_, ()> {
        self.db_lock.lock().await
    }

    /// Build a hybrid-path (`Postgres` storage) [`RebornBuildInput`] rooted at
    /// `reborn_home`. Callers chain `.with_runtime_policy(...)` and any other
    /// `.with_*` they need (including `with_local_dev_workspace_root` /
    /// `with_local_dev_confirmed_host_home_root`, which now apply to the
    /// Postgres variant and are threaded into the inner LocalDev substrate).
    pub(crate) fn build_input(
        &self,
        owner: &str,
        reborn_home: &std::path::Path,
    ) -> RebornBuildInput {
        RebornBuildInput::postgres_with_reborn_home(
            owner.to_string(),
            (*self.pool).clone(),
            self.url.clone(),
            reborn_home.to_path_buf(),
        )
    }
}

/// The local-dev filesystem substrate root used by the hybrid path:
/// `reborn_home.join("db")` (see `build_pg_runtime_stores`). Filesystem-skill
/// tests write skill/workspace files under this root so `build_local_dev`
/// finds them.
pub(crate) fn storage_root(reborn_home: &std::path::Path) -> std::path::PathBuf {
    reborn_home.join("db")
}
