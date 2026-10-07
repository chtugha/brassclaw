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
