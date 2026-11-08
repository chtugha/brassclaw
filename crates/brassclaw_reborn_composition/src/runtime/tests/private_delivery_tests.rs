//! Genuine PG/shared Worker/private product acceptance. Child processes only
//! read original evidence; they never construct a VM or a native live proof.
use super::*;
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
    factory_pause.release.notify_one();
    let driven = tokio::time::timeout(timeout(&runtime).await, waiter.wait())
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
            .is_err(),
        "the live caller must observe the actual lost acknowledgement"
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
        let tx = client.transaction().await.unwrap();
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
        let tx = client.transaction().await.unwrap();
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
    let mut client = crate::bootstrap_evidence::io::EvidenceClient::acquire(&transport.pool)
        .await
        .unwrap();
    let tx = client.transaction().await.unwrap();
    let expected = crate::review_work_confirmation::confirm(&tx, id, request.context())
        .await
        .unwrap()
        .unwrap();
    assert!(
        tx.commit().await.is_err(),
        "lose the actual confirmation COMMIT acknowledgement"
    );
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
    let tx = client.transaction().await.unwrap();
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
