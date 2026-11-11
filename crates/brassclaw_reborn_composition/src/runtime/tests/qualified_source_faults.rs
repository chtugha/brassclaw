//! Fault instruments act on actual author/evidence wire acknowledgements and
//! actual metadata writes. They construct no native success or review proof.
use super::*;
use crate::RebornRuntime;
use brassclaw_pg::test_support::{AckCutTransport, AckHoldEnd};

pub(super) async fn backend(pool: &brassclaw_pg::PgPool) -> i32 {
    pool.get()
        .await
        .unwrap()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0)
}
async fn require_live(rig: &crate::runtime::test_pg::PgRig, pid: i32) {
    assert!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
                &[&pid]
            )
            .await
            .unwrap()
            .get::<_, bool>(0),
        "the hold instrument must keep the actual backend alive before caller custody ends it"
    );
}
async fn require_discarded(
    runtime: &RebornRuntime,
    frame: &TestObservationFrame,
    id: Uuid,
    rig: &crate::runtime::test_pg::PgRig,
    wire: &AckCutTransport,
    ticket: &mut brassclaw_pg::test_support::AckHoldTicket,
    pid: i32,
) {
    assert_eq!(
        watchdog(
            runtime,
            frame,
            id,
            "held connection closure",
            ticket.wait_closed()
        )
        .await
        .unwrap(),
        AckHoldEnd::PeerClosed,
        "caller custody, not wire release/shutdown/cut, must close the held connection"
    );
    assert!(ticket.release().await.is_err());
    let replacement = watchdog(
        runtime,
        frame,
        id,
        "max-one replacement backend",
        backend(&wire.pool),
    )
    .await;
    assert_ne!(
        replacement, pid,
        "an unacknowledged backend must not return to the pool"
    );
    watchdog(
        runtime,
        frame,
        id,
        "old physical backend termination",
        async {
            loop {
                let exists: bool = rig
                    .pool
                    .get()
                    .await
                    .unwrap()
                    .query_one(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
                        &[&pid],
                    )
                    .await
                    .unwrap()
                    .get(0);
                if !exists {
                    break;
                }
                tokio::task::yield_now().await;
            }
        },
    )
    .await;
}
pub(super) async fn install_write_counter(rig: &crate::runtime::test_pg::PgRig) {
    rig.pool.get().await.unwrap().batch_execute("CREATE SEQUENCE i01_source_metadata_writes;
        CREATE FUNCTION count_i01_source_metadata() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            IF NEW.evidence_bytes::jsonb->>'validation_mode'='authored' AND NEW.evidence_bytes::jsonb->'report'->>'scope'='subject-bound-validator-execution' THEN
                PERFORM nextval('i01_source_metadata_writes'); END IF; RETURN NEW; END $$;
        CREATE TRIGGER count_i01_source_metadata BEFORE INSERT ON reborn_component_review_evidence FOR EACH ROW EXECUTE FUNCTION count_i01_source_metadata()").await.unwrap();
}
pub(super) async fn writes(rig: &crate::runtime::test_pg::PgRig) -> i64 {
    rig.pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT CASE WHEN is_called THEN last_value ELSE 0 END FROM i01_source_metadata_writes",
            &[],
        )
        .await
        .unwrap()
        .get(0)
}
pub(super) async fn cancelled_author_commit(
    runtime: &RebornRuntime,
    frame: &TestObservationFrame,
    rig: &crate::runtime::test_pg::PgRig,
    wire: &AckCutTransport,
    rule: u8,
    subject: (Uuid, Uuid),
    queued: bool,
) {
    let (id, component) = subject;
    let mut review = OriginalReview::new(runtime, frame, id);
    let pid = watchdog(
        runtime,
        frame,
        id,
        "fault backend selection",
        backend(&wire.pool),
    )
    .await;
    let mut ticket = wire.arm_hold(rule, pid).unwrap();
    let mut original = request(id, component, false);
    if queued {
        original.create_queue_project = Some("i01-held-queue".into());
    }
    let catalogue = runtime.installed_catalogue.clone();
    let actor = runtime.actor_user_id.as_str().to_owned();
    let tenant = runtime.thread_scope.tenant_id.as_str().to_owned();
    let agent = runtime.thread_scope.agent_id.as_str().to_owned();
    let retry = original.clone();
    let producer = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        crate::pg_review_submission::submit_with_catalogue(
            &catalogue.pool,
            &actor,
            &tenant,
            &agent,
            original,
            &catalogue,
        )
        .await
    }));
    let held = watchdog(
        runtime,
        frame,
        id,
        "author COMMIT acknowledgement hold",
        ticket.wait_held(),
    )
    .await
    .unwrap();
    assert_eq!(held.backend_pid, pid);
    require_live(rig, pid).await;
    let committed: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        committed, 1,
        "the server really committed the original before its held acknowledgement"
    );
    producer.abort();
    assert!(producer.await.unwrap_err().is_cancelled());
    require_discarded(runtime, frame, id, rig, wire, &mut ticket, pid).await;
    let receipt = submit(runtime, frame, retry).await.unwrap();
    assert_eq!(receipt.queue_ids.len(), usize::from(queued));
    assert_scope(runtime, frame, &mut review, component, true).await;
}
pub(super) async fn timed_out_evidence_commit(
    runtime: &RebornRuntime,
    frame: &TestObservationFrame,
    rig: &crate::runtime::test_pg::PgRig,
    wire: &AckCutTransport,
    rule: u8,
    id: Uuid,
    component: Uuid,
) {
    let mut review = OriginalReview::new(runtime, frame, id);
    let pid = watchdog(
        runtime,
        frame,
        id,
        "fault backend selection",
        backend(&wire.pool),
    )
    .await;
    let mut ticket = wire.arm_hold(rule, pid).unwrap();
    submit(runtime, frame, request(id, component, false))
        .await
        .unwrap();
    let held = watchdog(
        runtime,
        frame,
        id,
        "source evidence COMMIT acknowledgement hold",
        ticket.wait_held(),
    )
    .await
    .unwrap();
    assert_eq!(held.backend_pid, pid);
    require_live(rig, pid).await;
    let committed: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_validator_q1_runs WHERE submission_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        committed, 1,
        "the exact source evidence/run index committed before its held ACK"
    );
    // No test abort or wire release: the writer's actual acknowledged resource
    // store deadline cancels its write future and disposes of that connection.
    require_discarded(runtime, frame, id, rig, wire, &mut ticket, pid).await;
    assert_scope(runtime, frame, &mut review, component, true).await;
}
pub(super) async fn corrupt_run_index(
    runtime: &RebornRuntime,
    frame: &TestObservationFrame,
    rig: &crate::runtime::test_pg::PgRig,
    id: Uuid,
    component: Uuid,
) {
    let wrong_task = Uuid::new_v4();
    let sql = format!("CREATE FUNCTION corrupt_i01_source_index() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        IF NEW.submission_id='{id}'::uuid THEN NEW.task_handle='{wrong_task}'::uuid; NEW.succeeded=NOT NEW.succeeded; END IF;
        RETURN NEW; END $$;
        CREATE TRIGGER corrupt_i01_source_index BEFORE INSERT ON reborn_validator_q1_runs FOR EACH ROW EXECUTE FUNCTION corrupt_i01_source_index()");
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(&sql)
        .await
        .unwrap();
    let mut review = OriginalReview::new(runtime, frame, id);
    submit(runtime, frame, request(id, component, false))
        .await
        .unwrap();
    // Finish-observation failure may surface as the retained service failure;
    // inspect the original producer error, never accept Ok or discard it.
    review.failed(runtime, None).await;
    let row = rig.pool.get().await.unwrap().query_one("SELECT
        (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_validator_q1_runs WHERE submission_id=$1),
        (SELECT count(*) FROM reborn_component_review_evidence WHERE evidence_bytes::jsonb->>'submission_id'=$2),
        (SELECT count(*) FROM reborn_component_review_work_confirmations WHERE submission_id=$1)", &[&id,&id.to_string()]).await.unwrap();
    assert_eq!(
        row.get::<_, i64>(0),
        1,
        "the real parser ran once; no substitute proof or native replay"
    );
    for column in 1..4 {
        assert_eq!(
            row.get::<_, i64>(column),
            0,
            "a corrupt run index must roll back observations and never confirm source success"
        );
    }
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER corrupt_i01_source_index ON reborn_validator_q1_runs; DROP FUNCTION corrupt_i01_source_index()").await.unwrap();
}
