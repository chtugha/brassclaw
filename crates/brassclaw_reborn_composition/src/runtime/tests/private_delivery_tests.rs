//! Genuine PG/shared Worker/private product acceptance. Child processes only
//! read original evidence; they never construct a VM or a native live proof.
use super::local_dev_runtime_policy;
use crate::{RebornRuntime, RebornRuntimeInput, build_reborn_runtime};
use brassclaw_pg::{
    PgPool,
    test_support::{AckBoundary, AckCutRule, AckCutTransport},
};
use brassclaw_product_workflow::{ReviewWorkStatus, WebUiAuthenticatedCaller};
use brassclaw_turns::{private_work::PrivateExecutionContext, run_profile::MontyTurnDriverPort};
use secrecy::ExposeSecret as _;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

async fn start(rig: &crate::runtime::test_pg::PgRig, home: &std::path::Path) -> RebornRuntime {
    Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("h01-original-custody", home)
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap()
}
fn context(runtime: &RebornRuntime) -> PrivateExecutionContext {
    PrivateExecutionContext {
        tenant_id: runtime.thread_scope.tenant_id.clone(),
        agent_id: Some(runtime.thread_scope.agent_id.clone()),
        project_id: runtime.thread_scope.project_id.clone(),
        actor: Some(brassclaw_turns::TurnActor::new(
            runtime.actor_user_id.clone(),
        )),
    }
}
async fn timeout(runtime: &RebornRuntime) -> Duration {
    Duration::from_millis(
        runtime
            .installed_catalogue
            .preparation_policy()
            .await
            .unwrap()
            .response_timeout_millis,
    )
}
async fn submit_prefix(runtime: &RebornRuntime, id: Uuid) {
    let revision = runtime
        .installed_catalogue
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get::<_, i64>(0);
    let caller = WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    );
    assert_eq!(
        runtime
            .submit_current_core_prefix_snapshot(
                id,
                "base-prompt",
                u64::try_from(revision).unwrap(),
                caller
            )
            .await
            .unwrap(),
        id
    );
}

/// Also called in a fresh native test process with the exact parent's original
/// context and expected evidence. No globals, live slot or current VM is reused.
async fn child_readback() -> bool {
    let Ok(bytes) = std::env::var("BRASSCLAW_H01_READBACK") else {
        return false;
    };
    let request: Value = serde_json::from_str(&bytes).unwrap();
    let url = std::env::var("BRASSCLAW_H01_PG").unwrap();
    let pool = brassclaw_pg::pool::build_pool_with_max_size(&url, 1).unwrap();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let id: Uuid = serde_json::from_value(request["id"].clone()).unwrap();
    let expected_context: PrivateExecutionContext =
        serde_json::from_value(request["context"].clone()).unwrap();
    let delivery = crate::pg_monty_admission::PgMontyAdmission::read_private_delivery(
        &tx,
        id,
        &expected_context,
    )
    .await
    .unwrap();
    assert_eq!(
        delivery.as_ref().map(|v| hex::encode(v.checksum())),
        request["delivery_checksum"].as_str().map(str::to_owned)
    );
    if request["review"] == true {
        let original = crate::review_work_confirmation::read(&tx, id, &expected_context)
            .await
            .unwrap();
        assert_eq!(
            original,
            request
                .get("confirmation")
                .filter(|v| !v.is_null())
                .cloned()
        );
    }
    let mut foreign = expected_context;
    foreign.actor = Some(brassclaw_turns::TurnActor::new(
        brassclaw_host_api::UserId::new("h01-foreign-actor").unwrap(),
    ));
    assert!(
        crate::pg_monty_admission::PgMontyAdmission::read_private_delivery(&tx, id, &foreign)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    drop(client);
    pool.close();
    true
}
async fn read_in_child(
    rig: &crate::runtime::test_pg::PgRig,
    id: Uuid,
    context: &PrivateExecutionContext,
    checksum: Option<String>,
    confirmation: Option<Value>,
    deadline: Duration,
) {
    let request = json!({"id":id,"context":context,"delivery_checksum":checksum,
        "review":confirmation.is_some(),"confirmation":confirmation});
    let result=tokio::time::timeout(deadline,tokio::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact").arg("runtime::tests::private_delivery_tests::private_delivery_cold_process_reads_only_original_ack")
        .arg("--nocapture")
        .env("BRASSCLAW_H01_READBACK",request.to_string())
        .env("BRASSCLAW_H01_PG",rig.url.expose_secret())
        .kill_on_drop(true).output()).await.unwrap().unwrap();
    assert!(
        result.status.success(),
        "actual original-readback process failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8_lossy(&result.stdout);
    assert!(
        output.contains("1 passed") && !output.contains("0 passed"),
        "child must actually execute its selected readback case"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_delivery_cold_process_reads_only_original_ack() {
    if child_readback().await {
        return;
    }
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let deadline = timeout(&runtime).await;
    let id = Uuid::new_v4();
    let run = brassclaw_turns::TurnRunId::from_uuid(id);
    let original_context = context(&runtime);
    let factory_pause = runtime.monty_test_controls.1.pause_private_settlement(run);
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(run)
        .unwrap();
    submit_prefix(&runtime, id).await;
    tokio::time::timeout(timeout(&runtime).await, factory_pause.reached.notified())
        .await
        .unwrap();
    let detail =
        serde_json::to_value(runtime.inspect_prefix_job("base-prompt", id).await.unwrap()).unwrap();
    assert_eq!(detail["delivery_pending"], true);
    assert!(detail["internal_receipt"].is_null());
    let row = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_private_terminals WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        row.get::<_, i64>(0),
        1,
        "the genuine terminal COMMIT precedes Factory acknowledgement"
    );
    read_in_child(&rig, id, &original_context, None, None, deadline).await;
    // The actual native terminal already exists, but its Factory delivery is
    // paused. Fencing new admission must not invent this producer's result.
    runtime.global_monty_owner.client().close_admission();
    let mut original_completion = Box::pin(waiter.wait());
    let observed = std::future::poll_fn(|cx| {
        std::task::Poll::Ready(std::future::Future::poll(original_completion.as_mut(), cx))
    })
    .await;
    assert!(
        observed.is_pending(),
        "admission closure is not native producer completion"
    );
    factory_pause.release.notify_one();
    let driven = tokio::time::timeout(timeout(&runtime).await, original_completion)
        .await
        .unwrap()
        .unwrap();
    assert!(driven.internal_completion.is_some());
    drop(driven);
    assert!(
        !runtime
            .monty_test_controls
            .0
            .private_completion_live(run)
            .unwrap()
    );
    let wire =
        serde_json::to_value(runtime.inspect_prefix_job("base-prompt", id).await.unwrap()).unwrap();
    assert_eq!(wire["delivery_pending"], false);
    let checksum = wire["delivery_checksum"].as_str().unwrap().to_owned();
    let before = counts(&rig.pool, id).await;
    runtime.shutdown().await.unwrap();
    read_in_child(
        &rig,
        id,
        &original_context,
        Some(checksum.clone()),
        None,
        deadline,
    )
    .await;
    let restarted = start(&rig, home.path()).await;
    assert!(
        !restarted
            .monty_test_controls
            .0
            .private_completion_live(run)
            .unwrap()
    );
    let wire = serde_json::to_value(
        restarted
            .inspect_prefix_job("base-prompt", id)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(wire["delivery_checksum"], checksum);
    assert_eq!(
        counts(&rig.pool, id).await,
        before,
        "cold read/restart cannot replay original effects"
    );
    restarted.shutdown().await.unwrap();
}

async fn counts(pool: &PgPool, id: Uuid) -> Value {
    let row = pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT private_claim_attempts,
        (SELECT count(*) FROM brassclaw_monty_task_admissions WHERE private_work_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_terminals WHERE work_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1),
        (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1)
        FROM brassclaw_turns WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    json!([
        row.get::<_, i64>(0),
        row.get::<_, i64>(1),
        row.get::<_, i64>(2),
        row.get::<_, i64>(3),
        row.get::<_, i64>(4)
    ])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_delivery_lost_commit_ack_does_not_reexecute_source_compiler() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let id = Uuid::new_v4();
    let run = brassclaw_turns::TurnRunId::from_uuid(id);
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"native-post-factory-acknowledgement".to_vec(),
                id.as_bytes().to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        crate::input::RebornBuildInput::postgres_with_reborn_home(
            "h01-original-custody",
            transport.pool.as_ref().clone(),
            rig.url.clone(),
            home.path().to_owned(),
        )
        .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let pause = runtime.monty_test_controls.1.pause_private_settlement(run);
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(run)
        .unwrap();
    submit_prefix(&runtime, id).await;
    tokio::time::timeout(timeout(&runtime).await, pause.reached.notified())
        .await
        .unwrap();
    transport.arm(1).unwrap();
    pause.release.notify_one();
    assert!(
        tokio::time::timeout(timeout(&runtime).await, waiter.wait())
            .await
            .unwrap()
            .is_ok(),
        "fresh coherent readback must reconcile the actual lost COMMIT acknowledgement"
    );
    assert_eq!(transport.cut_count(), 1);
    let detail =
        serde_json::to_value(runtime.inspect_prefix_job("base-prompt", id).await.unwrap()).unwrap();
    assert_eq!(detail["delivery_pending"], false);
    assert!(detail["delivery_checksum"].is_string());
    let original = counts(&rig.pool, id).await;
    assert_eq!(original[0], 1);
    assert_eq!(original[1], 1);
    assert_eq!(original[2], 1);
    assert_eq!(original[3], 1);
    read_in_child(
        &rig,
        id,
        &context(&runtime),
        Some(detail["delivery_checksum"].as_str().unwrap().to_owned()),
        None,
        timeout(&runtime).await,
    )
    .await;
    assert_eq!(counts(&rig.pool, id).await, original);
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn original_review_discovery_never_fills_missing_pins_or_strands_later_originals() {
    use brassclaw_turns::run_profile::PrivateWorkReconciliationPort;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let capacity = runtime
        .installed_catalogue
        .preparation_policy()
        .await
        .unwrap()
        .max_pending_ownership_checks;
    let mut ids = Vec::new();
    for _ in 0..=capacity {
        ids.push(
            super::validator_retained_q1_tests::queued_structural_original(&rig)
                .await
                .3,
        );
    }
    let owner = crate::review_work_reconciliation::ReviewWorkReconciler::new(
        rig.pool.clone(),
        runtime.installed_catalogue.clone(),
        context(&runtime),
    );
    let ordered=rig.pool.get().await.unwrap().query("SELECT r.submission_id FROM reborn_component_review_requests r
        WHERE NOT EXISTS(SELECT 1 FROM reborn_component_review_work_confirmations c WHERE c.submission_id=r.submission_id)
        ORDER BY r.requested_at,r.submission_id",&[]).await.unwrap().iter().map(|r|r.get::<_,Uuid>(0)).collect::<Vec<_>>();
    for page in ordered.chunks(usize::try_from(capacity).unwrap()) {
        owner.reconcile_originals().await.unwrap();
        assert_eq!(
            owner.original_cursor().await,
            page.last().copied(),
            "later originals must not starve behind unresolved earlier requests"
        );
    }
    owner.reconcile_originals().await.unwrap();
    assert_eq!(owner.original_cursor().await, None);
    owner.reconcile_originals().await.unwrap();
    assert_eq!(
        owner.original_cursor().await,
        ordered.get(usize::try_from(capacity).unwrap() - 1).copied(),
        "a new pass revisits old unresolved originals"
    );
    for id in ids {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let state = crate::review_work_reconciliation::inspect(&tx, id)
            .await
            .unwrap();
        assert!(matches!(
            state.status,
            ReviewWorkStatus::OriginalPreparationMissing
        ));
        assert!(
            state.work_id.is_none() && !state.selection_preparation_present && !state.q1_qualified
        );
        assert!(
            !tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM brassclaw_turns WHERE validator_submission_id=$1)",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
        );
        tx.commit().await.unwrap();
    }
    runtime
        .monty_test_controls
        .0
        .reconcile_private_originals()
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn review_confirmation_commit_ack_loss_reuses_original_delivery_with_one_connection() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (scope, store, component_id, id) =
        super::validator_retained_q1_tests::queued_structural_original(&rig).await;
    // The real PostgreSQL insert fails after genuine observations/delivery.
    // A sequence witnesses reachability even though the transaction rolls back.
    rig.pool.get().await.unwrap().batch_execute("CREATE SEQUENCE h01_confirmation_reached;
        CREATE FUNCTION h01_hold_confirmation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('h01_confirmation_reached'); RAISE EXCEPTION 'h01 actual confirmation fault'; END $$;
        CREATE TRIGGER h01_hold_confirmation BEFORE INSERT ON reborn_component_review_work_confirmations
        FOR EACH ROW EXECUTE FUNCTION h01_hold_confirmation();").await.unwrap();
    let bridge = crate::installed_monty_catalogue::ValidatorServiceBridge {
        driver: Arc::new(crate::validator_task_driver::ValidatorTaskDriver::new(
            runtime.monty_test_controls.0.clone(),
        )),
        pool: rig.pool.clone(),
    };
    let result = crate::q1_orchestrator::run_q1_validation_retained(
        &rig.pool,
        &scope,
        component_id,
        22,
        &store,
        runtime
            .installed_catalogue
            .preparation_limits()
            .await
            .unwrap(),
        Some(&bridge),
    )
    .await;
    assert!(result.is_err());
    runtime.worker_cancel.cancel();
    tokio::time::timeout(timeout(&runtime).await, async {
        while !runtime.worker_handle.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let row = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT is_called FROM h01_confirmation_reached", &[])
        .await
        .unwrap();
    assert!(
        row.get::<_, bool>(0),
        "the intended original confirmation insert must really be reached"
    );
    let original = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT private_request_bytes FROM brassclaw_turns WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    let request: brassclaw_turns::private_work::PrivateWorkRequest =
        serde_json::from_str(original.get(0)).unwrap();
    let subject_actor = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT actor FROM reborn_component_review_submissions WHERE submission_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get::<_, String>(0);
    assert_ne!(
        subject_actor, scope.user_id,
        "operator actor and storage owner must actually differ"
    );
    assert_eq!(
        request.context().actor.as_ref().unwrap().user_id.as_str(),
        subject_actor
    );
    // Genuine owner-authored component plus independently authenticated original
    // operator; changing only execution actor still fails closed.
    let mut foreign = request.context().clone();
    foreign.actor = Some(brassclaw_turns::TurnActor::new(
        brassclaw_host_api::UserId::new("h01-foreign-review-operator").unwrap(),
    ));
    assert!(
        tokio::time::timeout(
            timeout(&runtime).await,
            crate::review_work_confirmation::confirm_original(&rig.pool, id, &foreign)
        )
        .await
        .unwrap()
        .is_err()
    );
    let before = counts(&rig.pool, id).await;
    assert_eq!(before[3], 1);
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER h01_hold_confirmation ON reborn_component_review_work_confirmations; DROP FUNCTION h01_hold_confirmation();").await.unwrap();
    // The native delivery does not authorize applying observations to a changed
    // queued candidate. Exercise the real store CAS before the successful ACK.
    let description = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT description FROM reborn_python_code WHERE id=$1",
            &[&component_id],
        )
        .await
        .unwrap()
        .get::<_, String>(0);
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE reborn_python_code SET description=$2 WHERE id=$1",
            &[
                &component_id,
                &format!("{description} changed after capture"),
            ],
        )
        .await
        .unwrap();
    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        assert!(matches!(
            crate::review_work_reconciliation::inspect(&tx, id)
                .await
                .unwrap()
                .status,
            ReviewWorkStatus::OriginalQueueChanged
        ));
        assert!(
            crate::review_work_confirmation::confirm(&tx, id, request.context())
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
    }
    assert!(!rig.pool.get().await.unwrap().query_one("SELECT EXISTS(SELECT 1 FROM reborn_component_review_work_confirmations WHERE submission_id=$1)",&[&id]).await.unwrap().get::<_,bool>(0));
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE reborn_python_code SET description=$2 WHERE id=$1",
            &[&component_id, &description],
        )
        .await
        .unwrap();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"component-review-work-confirmation/1".to_vec(),
                id.as_bytes().to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    transport.arm(1).unwrap();
    let mut client = crate::bootstrap_evidence::io::acquire(&transport.pool)
        .await
        .unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let expected = crate::review_work_confirmation::confirm(&tx, id, request.context())
        .await
        .unwrap()
        .unwrap();
    let leader_pid = tx
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get::<_, i32>(0);
    // A genuine concurrent confirmer opens RR before waiting on the original's
    // advisory lock. Its stale snapshot must fail, then reconcile in a fresh
    // coherent read-only transaction after the leader commits.
    let contender = tokio::time::timeout(
        timeout(&runtime).await,
        crate::review_work_confirmation::confirm_original(&rig.pool, id, request.context()),
    );
    let release = async {
        tokio::time::timeout(timeout(&runtime).await,async {
            loop {
                let blocked=rig.pool.get().await.unwrap().query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND $1::integer=ANY(pg_blocking_pids(pid)))",
                    &[&leader_pid]).await.unwrap().get::<_,bool>(0);
                if blocked {break;}
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("contender must actually wait on this original's advisory lock");
        assert!(
            tx.commit().await.is_err(),
            "lose the actual confirmation COMMIT acknowledgement"
        );
    };
    let (reconciled, ()) = tokio::join!(contender, release);
    assert_eq!(reconciled.unwrap().unwrap(), Some(expected.clone()));
    drop(client);
    assert_eq!(transport.cut_count(), 1);
    // A later authored edit cannot rewrite an already committed acknowledgement.
    // This does not simulate Q2 or claim support for full-Q1 queue removal.
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE reborn_python_code SET description=$2 WHERE id=$1",
            &[
                &component_id,
                &"later authored description after original confirmation",
            ],
        )
        .await
        .unwrap();
    let pool = brassclaw_pg::pool::build_pool_with_max_size(rig.url.expose_secret(), 1).unwrap();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    assert_eq!(
        crate::review_work_confirmation::confirm(&tx, id, request.context())
            .await
            .unwrap(),
        Some(expected.clone())
    );
    assert_eq!(expected["q1_qualified"], false);
    assert_eq!(expected["scope"], "structural-observations-only");
    tx.commit().await.unwrap();
    drop(client);
    pool.close();
    let checksum=rig.pool.get().await.unwrap().query_one("SELECT encode(checksum,'hex') FROM brassclaw_monty_private_deliveries WHERE work_id=$1",&[&id]).await.unwrap().get::<_,String>(0);
    read_in_child(
        &rig,
        id,
        request.context(),
        Some(checksum),
        Some(expected),
        timeout(&runtime).await,
    )
    .await;
    assert_eq!(counts(&rig.pool, id).await, before);
    assert_eq!(
        store
            .list(&scope, Some(1))
            .await
            .unwrap()
            .iter()
            .find(|r| r.component_id == component_id)
            .unwrap()
            .state,
        1
    );
    transport.shutdown().await.unwrap();
    runtime.shutdown().await.unwrap();
}

// ---------------------------------------------------------------------------
// B3-06: durable Q1-work reconciliation — three focused acceptance cases
// ---------------------------------------------------------------------------

/// Case 1 — Deferral when selection is absent:
/// The reconciler must defer (take no action) when
/// `selection_preparation_present` is false. Creating a selection from draft
/// heads is B1-07/B2-04 work; the reconciler must not attempt it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconciler_defers_when_qualified_selection_absent() {
    use brassclaw_turns::run_profile::PrivateWorkReconciliationPort;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    // Stop the background worker before constructing controlled fixtures so it
    // cannot race to prepare the original or trigger the reconciler itself.
    crate::runtime::test_pg::stop_worker_for_state_fixture(&runtime).await;

    // Queue a submission but do not create the validator selection.
    let (_scope, _store, _component_id, submission_id) =
        super::validator_retained_q1_tests::queued_structural_original(&rig).await;

    let reconciler = crate::review_work_reconciliation::ReviewWorkReconciler::new(
        rig.pool.clone(),
        runtime.installed_catalogue.clone(),
        context(&runtime),
    );

    // Reconcile: selection absent → reconciler must skip the submission.
    reconciler.reconcile_originals().await.unwrap();

    // Verify status is still OriginalPreparationMissing — no private_work row.
    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let view = crate::review_work_reconciliation::inspect(&tx, submission_id)
            .await
            .unwrap();
        assert!(
            matches!(view.status, ReviewWorkStatus::OriginalPreparationMissing),
            "status must remain OriginalPreparationMissing when selection is absent"
        );
        assert!(
            !view.selection_preparation_present,
            "selection must not exist"
        );
        assert!(
            view.work_id.is_none(),
            "no private_work row must be created"
        );
        tx.commit().await.unwrap();
    }

    runtime.shutdown().await.unwrap();
}

/// Case 2 — Restart without wake, qualified selection pre-seeded:
/// When a qualified selection already exists (created by B1-07/B2-04 or the
/// pure-source path), a fresh reconciler must create the `private_work` row on
/// the first `reconcile_originals()` call without any prior wake notification.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconciler_creates_private_work_without_wake_when_selection_present() {
    use brassclaw_turns::run_profile::PrivateWorkReconciliationPort;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    // Stop the background worker before constructing controlled fixtures so it
    // cannot race to prepare the original or call reconcile_originals itself.
    crate::runtime::test_pg::stop_worker_for_state_fixture(&runtime).await;

    // Use the test helper that seeds validator drafts, creates a queued
    // submission, and retains the validator selection via the draft-head path
    // (the same code path that B1-07/B2-04 targets in production, restricted
    // to test-only here).
    let (submission_id, _component_id) =
        super::validator_retained_q1_tests::queued_structural_original_with_selection(
            &rig, &runtime,
        )
        .await;

    // Verify the selection is present but the private_work row is not yet.
    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let view = crate::review_work_reconciliation::inspect(&tx, submission_id)
            .await
            .unwrap();
        assert!(
            view.selection_preparation_present,
            "selection must be present"
        );
        tx.commit().await.unwrap();
    }

    // Build a fresh reconciler (no prior wake) and call reconcile_originals once.
    let restarted_reconciler = crate::review_work_reconciliation::ReviewWorkReconciler::new(
        rig.pool.clone(),
        runtime.installed_catalogue.clone(),
        context(&runtime),
    );
    restarted_reconciler.reconcile_originals().await.unwrap();

    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let view = crate::review_work_reconciliation::inspect(&tx, submission_id)
            .await
            .unwrap();
        assert!(
            !matches!(view.status, ReviewWorkStatus::OriginalPreparationMissing),
            "private_work row must be created without wake; status={:?}",
            view.status
        );
        assert!(view.work_id.is_some(), "work_id must be set");
        tx.commit().await.unwrap();
    }

    runtime.shutdown().await.unwrap();
}

/// Case 3a — Concurrent-winner idempotency: when `prepare_private_validator` is
/// called twice on the same submission (simulating a re-entrant or concurrent
/// call after the row already exists), the second call returns the same immutable
/// state without inserting a duplicate row.
///
/// Note: this test does NOT simulate a lost PostgreSQL COMMIT acknowledgement.
/// Real lost-ACK fault injection is covered by Case 3b below.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepare_private_validator_concurrent_winner_returns_existing_original_without_replay() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    // Stop the background worker before constructing controlled fixtures so it
    // cannot race to prepare the original ahead of the test.
    crate::runtime::test_pg::stop_worker_for_state_fixture(&runtime).await;

    let (submission_id, _component_id) =
        super::validator_retained_q1_tests::queued_structural_original_with_selection(
            &rig, &runtime,
        )
        .await;

    // Confirm no private_work row exists before the test-controlled prepare call.
    let count_before: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count_before, 0,
        "no private_work row must exist before test action"
    );

    // First call — succeeds and creates the private_work row.
    let state_a = runtime
        .installed_catalogue
        .prepare_private_validator(submission_id)
        .await
        .expect("first prepare must succeed");

    // Verify exactly one private_work row exists.
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count, 1,
        "exactly one private_work row must exist after first prepare"
    );

    // Second call — row already exists; advisory lock + read-before-write returns
    // the existing state without inserting a duplicate.
    let state_b = runtime
        .installed_catalogue
        .prepare_private_validator(submission_id)
        .await
        .expect("second prepare (concurrent-winner re-read) must succeed");

    assert_eq!(
        state_a.request.work_id(),
        state_b.request.work_id(),
        "both calls must return the same work_id"
    );
    assert_eq!(
        state_a.request_checksum, state_b.request_checksum,
        "both calls must return the same immutable request checksum"
    );

    // Still exactly one row — no duplicate.
    let count_after: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count_after, 1, "no duplicate private_work row after retry");

    runtime.shutdown().await.unwrap();
}

/// Case 3b — Real PostgreSQL COMMIT abort via trigger fault injection:
/// a BEFORE INSERT trigger on `brassclaw_turns` raises an exception so that the
/// validator-original write transaction rolls back. The retry must call
/// `prepare_private_validator` again and succeed on a clean connection.
/// Critically, the rolled-back attempt must leave exactly zero rows so that the
/// retry inserts exactly one — no duplicate and no phantom row from the fault.
///
/// This exercises the `TransactionClient` custody under a real rollback: the
/// detached connection is not returned to the pool, the caller's error-path
/// readback in `prepare_private_validator` finds no existing row (because the
/// rollback was real), returns `Err`, and the caller retries cleanly.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepare_private_validator_commit_abort_retries_cleanly_without_duplicate() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    // Stop the background worker before constructing controlled fixtures so it
    // cannot prepare the original before the fault trigger is installed or
    // race to prepare after trigger removal.
    crate::runtime::test_pg::stop_worker_for_state_fixture(&runtime).await;

    let (submission_id, _component_id) =
        super::validator_retained_q1_tests::queued_structural_original_with_selection(
            &rig, &runtime,
        )
        .await;

    // Confirm no private_work row exists before installing the fault trigger.
    let count_before: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count_before, 0,
        "no private_work row must exist before fault trigger"
    );

    // Install a trigger that raises an exception on the INSERT into brassclaw_turns.
    // Scoped to private_work rows for this specific submission to avoid affecting
    // other concurrent test sessions.
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(&format!(
            "CREATE SEQUENCE b3_06_prepare_fault_reached;
             CREATE FUNCTION b3_06_fault_first_prepare() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN
                 IF NEW.record_kind = 'private_work'
                    AND NEW.validator_submission_id = '{submission_id}' THEN
                     PERFORM nextval('b3_06_prepare_fault_reached');
                     RAISE EXCEPTION 'b3-06 test: simulated prepare fault';
                 END IF;
                 RETURN NEW;
             END $$;
             CREATE TRIGGER b3_06_fault_first_prepare
                 BEFORE INSERT ON brassclaw_turns
                 FOR EACH ROW EXECUTE FUNCTION b3_06_fault_first_prepare();"
        ))
        .await
        .unwrap();

    // First call — write transaction rolls back due to the trigger.
    let first_result = runtime
        .installed_catalogue
        .prepare_private_validator(submission_id)
        .await;
    assert!(
        first_result.is_err(),
        "first prepare must fail due to trigger fault"
    );

    // Verify the trigger actually fired (sequence counter incremented).
    let reached: bool = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT is_called FROM b3_06_prepare_fault_reached", &[])
        .await
        .unwrap()
        .get(0);
    assert!(reached, "trigger must actually be reached before rollback");

    // No row must exist — the rollback was real.
    let count_after_fault: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count_after_fault, 0,
        "no private_work row must exist after rollback"
    );

    // Remove the trigger so the retry can succeed.
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(
            "DROP TRIGGER b3_06_fault_first_prepare ON brassclaw_turns;
             DROP FUNCTION b3_06_fault_first_prepare();",
        )
        .await
        .unwrap();

    // Second call — succeeds, inserts exactly one row.
    let state = runtime
        .installed_catalogue
        .prepare_private_validator(submission_id)
        .await
        .expect("retry after fault removal must succeed");
    assert_eq!(
        state.request.work_id(),
        submission_id,
        "returned work_id must match submission"
    );

    let count_final: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count_final, 1,
        "exactly one private_work row after retry — no duplicate"
    );

    runtime.shutdown().await.unwrap();
}

/// Case 3c — Real lost PostgreSQL COMMIT ACK via `AckCutTransport`:
///
/// The runtime and catalogue are started with the AckCutTransport proxy as
/// their sole pool (capacity 1). Both the write transaction and the subsequent
/// bounded readback go through the same proxy pool — this is the actual
/// production recovery path, not a split-pool injection.
///
/// Sequence:
/// 1. Start the proxy; build the runtime with the proxy pool.
/// 2. Seed fixtures while the transport is unarmed (clean connections).
/// 3. Arm the transport so the next matching COMMIT ACK is cut.
/// 4. Call `prepare_private_validator()` — the production entry point.
///    The write transaction triggers the advisory-lock advisory parameter match;
///    the COMMIT ACK is dropped. `commit_reached = true`, so the bounded
///    readback runs. It gets a new connection from the same proxy pool (the
///    uncertain connection was detached+dropped, freeing the slot) and finds
///    the committed row. Returns `Ok`.
/// 5. Assert successful readback, correct work_id, DB-verified checksum, exactly
///    one ACK cut, exactly one stored row, and that the recovery connection is a
///    different PG session (new PID) obtained within the configured deadline.
/// 6. Second call returns the same immutable row; checksums match; still one row.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepare_private_validator_pg_commit_ack_loss_reads_back_without_replay() {
    use secrecy::ExposeSecret as _;

    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;

    // Start the AckCutTransport proxy before the runtime so it is the sole
    // connection path. The transport is unarmed at this point; fixture seeding
    // and runtime startup use clean connections through the proxy.
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![b"brassclaw-validator-original:".to_vec()],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();

    // Build a capacity-1 pool through the proxy. All catalogue I/O — read-only
    // snapshots, write transaction, and the bounded readback — uses this pool.
    let proxy_pool = std::sync::Arc::new(
        brassclaw_pg::pool::build_pool_with_max_size(transport.connection_string(), 1).unwrap(),
    );

    // Start the runtime with the proxy pool so installed_catalogue.pool IS the
    // proxy pool. This matches private_work_admission_tests::Fixture::start.
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        crate::input::RebornBuildInput::postgres_with_reborn_home(
            "h01-ack-loss-proxy",
            proxy_pool.as_ref().clone(),
            rig.url.clone(),
            home.path().to_path_buf(),
        )
        .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    // Stop the background worker before seeding fixtures and before arming the
    // transport so it cannot race to prepare the original ahead of the test-
    // controlled call.
    crate::runtime::test_pg::stop_worker_for_state_fixture(&runtime).await;

    // Seed fixtures while the transport is unarmed. The seeding helpers use
    // rig.pool directly, not the proxy, so they are unaffected by the fault.
    let (submission_id, _component_id) =
        super::validator_retained_q1_tests::queued_structural_original_with_selection(
            &rig, &runtime,
        )
        .await;

    // Confirm no private_work row exists before arming the transport.
    let count_before_arm: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count_before_arm, 0,
        "no private_work row must exist before arming the transport"
    );

    // Record the first backend PID reachable via the proxy pool. This is the
    // PG session that will be used for the write transaction. After the COMMIT
    // ACK is cut and the connection is detached+dropped, the slot is freed and
    // the readback gets a new connection with a different PID.
    let first_pid: i32 = proxy_pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);

    // Now arm the transport: the next advisory-lock COMMIT ACK is intercepted.
    transport.arm(1).unwrap();

    // Production call: write + readback both go through proxy_pool (self.pool).
    // The write ACK is cut; commit_reached = true; the bounded readback gets a
    // new connection from the same proxy pool and resolves the committed row.
    let state_a = runtime
        .installed_catalogue
        .prepare_private_validator(submission_id)
        .await
        .expect("bounded readback on proxy_pool must resolve the committed row after ACK loss");

    assert_eq!(
        state_a.request.work_id(),
        submission_id,
        "resolved work_id must match the submission"
    );

    // Transport must have cut exactly one COMMIT ACK.
    assert_eq!(
        transport.cut_count(),
        1,
        "transport must have cut exactly one COMMIT ACK"
    );

    // Verify DB-persisted checksum — proves the readback returned the actual row.
    let db_checksum: Vec<u8> = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT private_request_checksum FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        state_a.request_checksum,
        db_checksum.as_slice(),
        "resolved request_checksum must match the DB-persisted value"
    );

    // The uncertain write connection was detached (slot freed). The readback
    // already obtained a new connection from proxy_pool. Verify that a fresh
    // checkout from proxy_pool now returns a different PG backend PID — proving
    // the original session ended and recovery ran through the same pool.
    // The checkout and PID query are bounded by the configured deadline.
    let configured_deadline = timeout(&runtime).await;
    let recovery_pid: i32 = tokio::time::timeout(configured_deadline, async {
        proxy_pool
            .get()
            .await
            .expect("proxy_pool must accept a new checkout after detach freed the slot")
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .expect("pg_backend_pid must be readable on fresh connection")
            .get(0)
    })
    .await
    .expect("recovery PID query must complete within the configured deadline");
    assert_ne!(
        recovery_pid, first_pid,
        "recovery connection must be a fresh PG session (original was detached+dropped)"
    );

    // Exactly one private_work row — no replay.
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count, 1,
        "exactly one private_work row after COMMIT ACK loss"
    );

    // Second call: row already exists, returned by read_private_validator_original
    // without a new INSERT. Both calls must resolve the same immutable row.
    let state_b = runtime
        .installed_catalogue
        .prepare_private_validator(submission_id)
        .await
        .expect("second call after ACK loss must succeed");
    assert_eq!(
        state_b.request.work_id(),
        submission_id,
        "second call: work_id must match the submission"
    );
    assert_eq!(
        state_a.request_checksum, state_b.request_checksum,
        "both calls must return the same immutable request checksum (same original row)"
    );

    // Still exactly one row after the second call.
    let count_final: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_turns \
             WHERE validator_submission_id=$1 AND record_kind='private_work'",
            &[&submission_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count_final, 1, "no duplicate row after ACK-loss retry");

    // Shut down the runtime first (joins all background tasks that use
    // proxy_pool), then close the pool, then stop the transport proxy.
    runtime.shutdown().await.unwrap();
    proxy_pool.close();
    transport.shutdown().await.unwrap();
}
