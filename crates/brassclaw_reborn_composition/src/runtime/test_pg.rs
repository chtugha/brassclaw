//! Private native PostgreSQL rig for each composition test.
//! Uses the bundled, verified PostgreSQL binary and isolated temporary data.
//! Initialization failures fail the test; database checks never silently skip.
//! A pool never crosses independent Tokio test runtimes. The final owner
//! closes the pool and stops PostgreSQL before removing its temporary files.

use crate::input::RebornBuildInput;
use brassclaw_pg::PgPool;
use brassclaw_secrets::SecretMaterial;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

#[path = "../../tests/common/native_pg.rs"]
pub(crate) mod native_pg;

pub(crate) struct PgRig {
    pub(crate) pool: Arc<PgPool>,
    pub(crate) url: SecretMaterial,
    db_lock: Mutex<()>,
    _server: native_pg::NativePostgres,
}

pub(crate) async fn pg_rig() -> Arc<PgRig> {
    let server = native_pg::NativePostgres::start().await;
    rig_from_server(server)
}

/// Genuine previous-schema fixture; ordinary runtime startup performs upgrade.
#[cfg(all(feature = "postgres", feature = "skills-db"))]
pub(crate) async fn pg_rig_at_version(version: u32) -> Arc<PgRig> {
    rig_from_server(native_pg::NativePostgres::start_schema(Some(version)).await)
}

fn rig_from_server(server: native_pg::NativePostgres) -> Arc<PgRig> {
    Arc::new(PgRig {
        pool: Arc::clone(&server.pool),
        url: server.url.clone(),
        db_lock: Mutex::new(()),
        _server: server,
    })
}

/// Manual gate/tree fixtures transition durable state themselves. Stop only
/// the worker before constructing those fixtures so it cannot claim fake inputs.
/// The production shutdown still joins its completed handle afterwards.
pub(crate) async fn stop_worker_for_state_fixture(runtime: &super::RebornRuntime) {
    runtime.worker_cancel.cancel();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !runtime.worker_handle.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("fixture worker stops");
}

impl PgRig {
    /// Runtime tests use explicit finite settings without depending on the
    /// machine having the production default reserve free. Startup-mode tests
    /// force below-floor sizing through the real probe and an oversized reserve.
    pub(crate) async fn configure_runtime_memory(
        &self,
        mode: brassclaw_product_workflow::MontyMemoryMode,
    ) {
        use brassclaw_product_workflow::{MontyMemoryMode, MontyVmSettingsStore};

        let store = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            Arc::clone(&self.pool),
            "default",
            "default",
        );
        let current = store.get("default", "default").await.expect("settings");
        let mut policy = current.memory_policy;
        policy.mode = mode;
        if mode == MontyMemoryMode::Startup {
            policy.reserve_bytes = i64::MAX as u64;
        }
        let request = serde_json::from_value(serde_json::json!({
            "expected_revision": current.revision,
            "memory_policy": policy,
            "max_memory_bytes": brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES,
        }))
        .expect("finite fixture settings");
        store
            .upsert("default", "default", &request)
            .await
            .expect("persist finite fixture settings");
    }

    /// Serialize operations sharing this test's private database.
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

#[tokio::test]
async fn native_postgres_failed_stop_preserves_cluster_without_forced_shutdown() {
    let database = native_pg::NativePostgres::start().await;
    let directory = database.directory.path().to_owned();
    let port = {
        let client = database.pool.get().await.expect("fixture connection");
        let row = client
            .query_one("SELECT inet_server_port()", &[])
            .await
            .expect("actual private server port");
        u16::try_from(row.get::<_, i32>(0)).expect("PostgreSQL port")
    };
    let bin = directory.join("bin").join("bin");
    let control = bin.join(format!("pg_ctl{}", std::env::consts::EXE_SUFFIX));
    let unavailable = control.with_extension("temporarily-unavailable");
    std::fs::rename(&control, &unavailable).expect("hide only the fixture control binary");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(database)));
    // Restore before assertions so the real server can be stopped normally.
    // If restoration itself fails, retained resources remain for diagnosis.
    std::fs::rename(&unavailable, &control).expect("restore the fixture control binary");
    let pid = directory.join("data").join("postmaster.pid");
    let retained = directory.is_dir() && pid.is_file();
    let stopped =
        brassclaw_embedded_postgres::pgctl::PgCtl::new(&bin, &directory.join("data"), port)
            .stop()
            .await;
    let exited = !pid.exists();
    // Never delete this cluster until pg_ctl -w confirms the orderly stop.
    if stopped.is_ok() && exited {
        std::fs::remove_dir_all(&directory).expect("remove confirmed stopped fixture");
    }
    assert!(result.is_err(), "a failed stop must fail the test visibly");
    assert!(retained, "the failed stop must preserve the live cluster");
    stopped.expect("orderly recovery after restoring the control binary");
    assert!(exited, "the recovered server must have exited");
    assert!(!directory.exists());

    let normal = native_pg::NativePostgres::start().await;
    let directory = normal.directory.path().to_owned();
    drop(normal);
    assert!(
        !directory.exists(),
        "confirmed cleanup still removes its files"
    );
}
