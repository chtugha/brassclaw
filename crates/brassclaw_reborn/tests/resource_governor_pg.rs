use std::sync::{Arc, Barrier};

use brassclaw_resources::{PgResourceGovernorStore, ResourceError, ResourceGovernorStore};

#[path = "common/native_pg.rs"]
mod native_pg;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_snapshot_cas_fences_concurrent_initialization_and_deleted_rows() {
    let database = native_pg::NativePostgres::start().await;
    let barrier = Arc::new(Barrier::new(2));
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let store = PgResourceGovernorStore::new(database.pool.clone(), "budget-cas");
        let barrier = barrier.clone();
        tasks.push(tokio::task::spawn_blocking(move || {
            store.update(move |_| {
                // Both real reads finish before either real write starts.
                barrier.wait();
                Ok(())
            })
        }));
    }
    let mut successes = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(()) => successes += 1,
            Err(ResourceError::Storage { .. }) => {}
            other => panic!("unexpected CAS result: {other:?}"),
        }
    }
    assert_eq!(successes, 1);
    let client = database.pool.get().await.unwrap();
    let version: i64 = client
        .query_one(
            "SELECT version FROM brassclaw_resource_accounts WHERE tenant_id = 'budget-cas'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(version, 1);
    drop(client);

    let pool = database.pool.clone();
    let store = PgResourceGovernorStore::new(pool.clone(), "budget-cas");
    let result = tokio::task::spawn_blocking(move || {
        store.update(move |_| {
            tokio::runtime::Handle::current().block_on(async {
                pool.get()
                    .await
                    .unwrap()
                    .execute(
                        "DELETE FROM brassclaw_resource_accounts WHERE tenant_id = 'budget-cas'",
                        &[],
                    )
                    .await
                    .unwrap();
            });
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(matches!(result, Err(ResourceError::Storage { .. })));
    let count: i64 = database
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_resource_accounts WHERE tenant_id = 'budget-cas'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0, "a stale writer must not recreate deleted state");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_snapshot_version_exhaustion_keeps_existing_payload() {
    let database = native_pg::NativePostgres::start().await;
    let store = PgResourceGovernorStore::new(database.pool.clone(), "budget-exhausted");
    store.update(|_| Ok(())).unwrap();
    let client = database.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE brassclaw_resource_accounts SET version = $1 WHERE tenant_id = 'budget-exhausted'",
            &[&i64::MAX],
        )
        .await
        .unwrap();
    let before: serde_json::Value = client
        .query_one(
            "SELECT payload FROM brassclaw_resource_accounts WHERE tenant_id = 'budget-exhausted'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(matches!(
        store.update(|_| Ok(())),
        Err(ResourceError::Storage { .. })
    ));
    let row = client
        .query_one(
            "SELECT version, payload FROM brassclaw_resource_accounts WHERE tenant_id = 'budget-exhausted'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), i64::MAX);
    assert_eq!(row.get::<_, serde_json::Value>(1), before);
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
