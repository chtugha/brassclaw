//! Actual PostgreSQL/held wire acknowledgements, without starting a VM or
//! re-running a fixture to recover its interrupted admission.
use super::*;
use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
use brassclaw_product_workflow::MontyVmSettingsStore;
use brassclaw_turns::TurnRunId;
use secrecy::ExposeSecret;
use std::time::Duration;

struct Pause {
    phase: &'static str,
    reached: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
tokio::task_local! { static PAUSE: Arc<Pause>; }
pub(super) async fn before_phase(phase: &'static str) {
    if let Ok(pause) = PAUSE.try_with(Arc::clone)
        && pause.phase == phase
    {
        pause.reached.notify_one();
        pause.release.notified().await;
    }
}

async fn policy(
    rig: &crate::runtime::test_pg::PgRig,
) -> Arc<crate::resource_store_settings::AcknowledgedResourceStoreSettings> {
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        rig.pool.clone(),
        "default",
        "default",
    )
    .get("default", "default")
    .await
    .unwrap();
    let policy = Arc::new(crate::resource_store_settings::AcknowledgedResourceStoreSettings::new());
    policy
        .prepare_bootstrap(
            settings.revision,
            settings.execution_limits.resource_store_timeout_millis,
        )
        .unwrap();
    policy
}

async fn pending_bytes(pool: &PgPool, id: Uuid) -> String {
    pool.get()
        .await
        .unwrap()
        .query_one(
            "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0)
}
fn require_private_binding(original: &PreparedAdmission, bytes: &str) {
    let record: Value = serde_json::from_str(bytes).unwrap();
    assert_eq!(record["succeeded"], false);
    let binding = &record["report"];
    assert_eq!(binding["format"], "bootstrap-admission-original/1");
    assert_eq!(
        binding["run_id"],
        serde_json::to_value(original.attempt.run_id).unwrap()
    );
    assert_eq!(binding["conversation_id"], original.conversation.as_str());
    assert_eq!(binding["claim_checksum"].as_str().unwrap().len(), 64);
    assert!(binding.get("lease_token").is_none());
    assert!(binding.get("admission_key").is_none());
    assert!(!bytes.contains(&serde_json::to_string(&original.attempt.lease_token).unwrap()));
}
async fn require_unexecuted(pool: &PgPool, run: TurnRunId, expected_admissions: i64) {
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT (SELECT count(*) FROM brassclaw_monty_task_admissions WHERE run_id=$1),
        (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM brassclaw_monty_recipe_selections WHERE run_id=$1)",
            &[&run.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), expected_admissions);
    assert_eq!(row.get::<_, i64>(1), 0);
    assert_eq!(row.get::<_, i64>(2), 0);
}

async fn pending_binding_interruption(caller_cancel: bool) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let policy = policy(&rig).await;
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::next_commit(1).hold()],
    )
    .await
    .unwrap();
    let pool = Arc::new(
        brassclaw_pg::pool::build_pool_with_max_size(wire.connection_string(), 1).unwrap(),
    );
    let pid: i32 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let mut original =
        PreparedAdmission::new(pool.clone(), "pending-binding-original", "data-only")
            .await
            .unwrap();
    let id = Uuid::new_v4();
    let mut ticket = wire.arm_hold(1, pid).unwrap();
    let mut retain = Box::pin(original.retain(policy.as_ref(), id, json!({"id":id})));
    tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(30),ticket.wait_held()) => { assert_eq!(result.unwrap().unwrap().backend_pid,pid); },
        _ = &mut retain => panic!("the original pending write must reach its real COMMIT hold"),
    }
    if caller_cancel {
        drop(retain);
    } else {
        assert!(
            tokio::time::timeout(Duration::from_secs(30), retain)
                .await
                .unwrap()
                .is_err()
        );
    }
    let bytes = pending_bytes(&rig.pool, id).await;
    require_private_binding(&original, &bytes);
    assert!(!original.retained);
    assert!(original.admit(None).await.is_err());
    let row = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT (SELECT count(*) FROM brassclaw_session_threads WHERE id=$1),
        (SELECT count(*) FROM brassclaw_turns WHERE tenant_id=$2 AND turn_id=$1)",
            &[&original.conversation.as_str(), &NAMESPACE],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), 0);
    assert_eq!(row.get::<_, i64>(1), 0);
    require_unexecuted(&rig.pool, original.attempt.run_id, 0).await;
    tokio::time::timeout(Duration::from_secs(30), ticket.wait_closed())
        .await
        .unwrap()
        .unwrap();
    assert!(ticket.release().await.is_err());
    pool.close();
    drop(original);
    wire.shutdown().await.unwrap();
}

async fn interrupted_original(phase: &'static str, caller_cancel: bool, hold_close: bool) {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let policy = policy(&rig).await;
    let wire = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::next_commit(1).hold()],
    )
    .await
    .unwrap();
    let pool = Arc::new(
        brassclaw_pg::pool::build_pool_with_max_size(wire.connection_string(), 1).unwrap(),
    );
    let mut original = PreparedAdmission::new(pool.clone(), "interrupted-original", "data-only")
        .await
        .unwrap();
    let id = Uuid::new_v4();
    original
        .retain(policy.as_ref(), id, json!({"id":id}))
        .await
        .unwrap();
    let bytes = pending_bytes(&rig.pool, id).await;
    require_private_binding(&original, &bytes);
    let run = original.attempt.run_id;
    let scope = original.turn_scope.clone();
    let pid: i32 = pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let pause = Arc::new(Pause {
        phase,
        reached: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let mut exchange = Box::pin(crate::bootstrap_evidence::io::bounded_for(
        policy.as_ref(),
        "original-admission-test",
        id,
        PAUSE.scope(pause.clone(), original.admit(None)),
    ));
    tokio::select! {
        () = pause.reached.notified() => (),
        _ = &mut exchange => panic!("original must reach selected admission phase"),
    }
    let mut ticket = wire.arm_hold(1, pid).unwrap();
    pause.release.notify_one();
    tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(30),ticket.wait_held()) => { assert_eq!(result.unwrap().unwrap().backend_pid,pid); },
        _ = &mut exchange => panic!("original must reach its actual held COMMIT"),
    }
    if caller_cancel {
        // Real cancellation of the borrowed caller future. No replacement IO
        // task, fake callback or successful admission response is constructed.
        drop(exchange);
    } else {
        assert!(
            tokio::time::timeout(Duration::from_secs(30), exchange)
                .await
                .unwrap()
                .is_err()
        );
    }
    assert!(!original.handed_off);
    assert!(original.called);
    assert_eq!(pending_bytes(&rig.pool, id).await, bytes);
    let independent = PgTurnStateStore::new(rig.pool.clone(), NAMESPACE);
    let begun = independent
        .get_run_record(&scope, run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(begun.run_id, run);
    assert_eq!(
        begun.claim_count,
        if phase == "submit_turn" { 0 } else { 1 }
    );
    let admissions = if matches!(phase, "persist_reservation" | "check_and_start") {
        1
    } else {
        0
    };
    require_unexecuted(&rig.pool, run, admissions).await;
    // The real same pool-one socket still has its ACK held. Observation must
    // fail closed until that original response is made observable.
    assert!(original.close(policy.as_ref()).await.is_err());
    assert!(original.admit(None).await.is_err());
    require_unexecuted(&rig.pool, run, admissions).await;
    ticket.release().await.unwrap();
    let cancel_version = if hold_close {
        let mut close_ticket = wire.arm_hold(1, pid).unwrap();
        let mut close = Box::pin(original.close(policy.as_ref()));
        tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(30),close_ticket.wait_held()) => { assert_eq!(result.unwrap().unwrap().backend_pid,pid); },
            _ = &mut close => panic!("the first exact cancellation must reach its real COMMIT hold"),
        }
        assert!(
            tokio::time::timeout(Duration::from_secs(30), close)
                .await
                .unwrap()
                .is_err()
        );
        assert!(original.cancel_started);
        assert!(!original.completion_started);
        let cancelling = independent
            .get_run_record(&scope, run)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cancelling.status, TurnStatus::CancelRequested);
        let version:i64 = rig.pool.get().await.unwrap().query_one(
            "SELECT version FROM brassclaw_turns WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
            &[&NAMESPACE,&scope.thread_id.as_str()],
        ).await.unwrap().get(0);
        close_ticket.release().await.unwrap();
        Some(version)
    } else {
        None
    };
    let closed = original.close(policy.as_ref()).await.unwrap();
    assert_eq!(closed["run_id"], serde_json::to_value(run).unwrap());
    assert_eq!(closed["status"], "cancelled");
    assert_eq!(closed["execution_handed_off"], false);
    let terminal = independent
        .get_run_record(&scope, run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(terminal.status, TurnStatus::Cancelled);
    assert_eq!(terminal.claim_count, begun.claim_count);
    assert!(terminal.runner_id.is_none() && terminal.lease_token.is_none());
    if let Some(version) = cancel_version {
        let final_version:i64 = rig.pool.get().await.unwrap().query_one(
            "SELECT version FROM brassclaw_turns WHERE tenant_id=$1 AND turn_id=$2 AND status='snapshot'",
            &[&NAMESPACE,&scope.thread_id.as_str()],
        ).await.unwrap().get(0);
        assert_eq!(
            final_version,
            version + 1,
            "only the original cancel completion may change the Turn after the lost cancellation ACK"
        );
    }
    require_unexecuted(&rig.pool, run, admissions).await;
    assert_eq!(pending_bytes(&rig.pool, id).await, bytes);
    assert!(original.admit(None).await.is_err());
    pool.close();
    drop(original);
    wire.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_admission_phase_unknown_commit_deadlines_keep_exact_original() {
    for phase in [
        "submit_turn",
        "claim_next_run",
        "persist_reservation",
        "check_and_start",
    ] {
        interrupted_original(phase, false, false).await;
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_admission_phase_caller_cancel_keep_exact_original() {
    for phase in [
        "submit_turn",
        "claim_next_run",
        "persist_reservation",
        "check_and_start",
    ] {
        interrupted_original(phase, true, false).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pending_binding_commit_loss_and_caller_cancel_never_enter_admission() {
    for caller_cancel in [false, true] {
        pending_binding_interruption(caller_cancel).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_cancel_unknown_commit_is_read_back_without_reissuing_cancel() {
    interrupted_original("persist_reservation", true, true).await;
}
