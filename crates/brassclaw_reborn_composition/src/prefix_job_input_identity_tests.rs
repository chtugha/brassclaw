//! Real PostgreSQL request identity and insert-winner races. No job execution,
//! synthetic acknowledgement or alternative admission implementation.
use super::*;
use serde_json::json;
use std::time::Duration;

pub(super) async fn assert_input_identity(
    pool: Arc<PgPool>,
    template: Reservation<'_>,
    replacement: &InspectedRetainedProgram,
) {
    let store = Store::new(pool.clone());
    let original = store.get(template.id).await.unwrap().unwrap();
    let replacement_bytes = replacement
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .unwrap();
    assert_ne!(replacement_bytes.exact_bytes(), original.program_bytes);
    // This is an automatically selected compiler, not an explicit request
    // field. Retry must preserve the first definition instead of repinning it.
    let retried = store
        .reserve(Reservation {
            program: replacement,
            ..template
        })
        .await
        .unwrap();
    assert_eq!(retried.program_bytes, original.program_bytes);
    assert_eq!(retried.input, original.input);

    let id = Uuid::new_v4();
    let mut scope = (*template.scope).clone();
    scope.thread_id = brassclaw_host_api::ThreadId::new(format!("prefix-job-{id}")).unwrap();
    let left_input = (*template.input).clone();
    let mut right_input = left_input.clone();
    right_input["snapshot_bytes"] = json!("different explicitly offered source data");
    let mut client = pool.get().await.unwrap();
    // Activity snapshots inside one open transaction may remain cached. The
    // separate autocommit observer must see each contender's current lock wait.
    let observer = pool.get().await.unwrap();
    let lock = client.transaction().await.unwrap();
    let blocker: i32 = lock
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    // SHARE permits both initial SELECTs but blocks both INSERTs. Observing
    // both real lock waits proves the loser reaches the insert-winner check.
    lock.batch_execute("LOCK TABLE brassclaw_prefix_job_definitions IN SHARE MODE")
        .await
        .unwrap();
    let competing = async {
        tokio::join!(
            store.reserve(Reservation {
                id,
                scope: &scope,
                input: &left_input,
                ..template
            }),
            store.reserve(Reservation {
                id,
                scope: &scope,
                input: &right_input,
                ..template
            }),
        )
    };
    let release =
        async {
            let observed =
                tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        let waiting: i64 = observer.query_one(
                    "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database()
                     AND wait_event_type='Lock'
                     AND $1=ANY(pg_blocking_pids(pid))
                     AND query LIKE 'INSERT INTO brassclaw_prefix_job_definitions%'", &[&blocker]
                ).await.unwrap().get(0);
                        if waiting == 2 {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await;
            // Release before asserting, including on timeout, so neither actual
            // transaction is detached or left holding test resources.
            lock.commit().await.unwrap();
            observed
        };
    let ((left, right), observed) = tokio::join!(competing, release);
    observed.expect("both actual input insert contenders must reach the lock");
    let winner = match (left, right) {
        (Ok(winner), Err(Error::Conflict)) | (Err(Error::Conflict), Ok(winner)) => winner,
        _ => panic!("one committed original and one conflicting input required"),
    };
    assert!(winner.input == left_input || winner.input == right_input);
    let saved = store.get(id).await.unwrap().unwrap();
    assert_eq!(saved.input, winner.input);
    assert_eq!(saved.program_bytes, winner.program_bytes);
    assert_eq!(
        pool.get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_prefix_job_definitions WHERE job_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        pool.get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        pool.get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}
