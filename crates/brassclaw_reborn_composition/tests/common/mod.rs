//! Private native PostgreSQL rig for each composition test.
//! Uses the bundled, verified PostgreSQL binary and isolated temporary data.
//! Initialization failures fail the test; database checks never silently skip.
//! A pool never crosses independent Tokio test runtimes. The final owner
//! closes the pool and stops PostgreSQL before removing its temporary files.

use brassclaw_pg::PgPool;
use brassclaw_reborn_composition::RebornBuildInput;
use brassclaw_secrets::SecretMaterial;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

mod native_pg;

pub(crate) struct PgRig {
    pub(crate) pool: Arc<PgPool>,
    pub(crate) url: SecretMaterial,
    db_lock: Mutex<()>,
    _server: native_pg::NativePostgres,
}

pub(crate) async fn pg_rig() -> Arc<PgRig> {
    let server = native_pg::NativePostgres::start().await;
    Arc::new(PgRig {
        pool: Arc::clone(&server.pool),
        url: server.url.clone(),
        db_lock: Mutex::new(()),
        _server: server,
    })
}

impl PgRig {
    /// Serialize operations sharing this test's private database.
    pub(crate) async fn lock_db(&self) -> MutexGuard<'_, ()> {
        self.db_lock.lock().await
    }

    /// Build a hybrid-path (`Postgres` storage) [`RebornBuildInput`] rooted at
    /// `reborn_home`. Callers chain `.with_runtime_policy(...)` and any other
    /// `.with_*` they need (including `with_local_dev_workspace_root` /
    /// `with_local_dev_confirmed_host_home_root`, which apply to the Postgres
    /// variant and are threaded into the inner LocalDev substrate).
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
