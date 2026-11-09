//! Real Source -> original case/Work -> current global VM -> registered kernel
//! primitive -> normal ResultOnly delivery. These tests grant no candidate Q1.
use super::local_dev_runtime_policy;
use crate::{RebornRuntime, RebornRuntimeInput, build_reborn_runtime};
use brassclaw_product_workflow::{MontyMemoryMode, ReviewWorkStatus, SubmitComponentReviewRequest};
use brassclaw_turns::{TurnRunId, private_work::PrivateExecutionContext};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

// A test watchdog for the whole connected workflow, independent of the one-IPC
// response deadline or any product task compute limit. On expiry the test fails;
// no original is replayed or filled in by the fixture.
const WATCHDOG: Duration = Duration::from_secs(600);
async fn start(rig: &crate::runtime::test_pg::PgRig, home: &std::path::Path) -> RebornRuntime {
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("j01-original-case", home)
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
fn source(component: Uuid) -> String {
    let exports = [
        "return_value",
        "domain_error",
        "output_defect",
        "many_yields",
        "resource_limit",
        "never_complete",
    ]
    .into_iter()
    .map(|name| {
        (
            name,
            json!({"symbol":name,"parameters":["inputs"],"mapping":true}),
        )
    })
    .collect::<serde_json::Map<_, _>>();
    json!({"format":"component-revision/1","uuid":component,"class_code":22,"dependencies":[],"association":null,
        "document":{"name":"Original case candidate","description":"Authored case fixture, no behavior approval",
        "content":"def return_value(inputs):\n    return inputs['text']\n\ndef domain_error(inputs):\n    raise ValueError(inputs['text'])\n\ndef output_defect(inputs):\n    return 7\n\ndef many_yields(inputs):\n    total = 0\n    for item in range(2500000):\n        total = total + item\n    return inputs['text']\n\ndef resource_limit(inputs):\n    return inputs['text'] * 1000000000\n\ndef never_complete(inputs):\n    total = 0\n    while True:\n        total = total + 1\n",
        "input_contract":{"text":{"type":"string","required":true,"checks":[]}},"result_contract":{"type":"string"},
        "includes":[],"dependency_registry":null,"preload":{"format":"python-preload/2","exports":exports,
            "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"return_value"}}}).to_string()
}
async fn original_source(runtime: &RebornRuntime) -> (Uuid, Uuid) {
    let (submission, component) = (Uuid::new_v4(), Uuid::new_v4());
    let request = SubmitComponentReviewRequest {
        queue_ids: vec![],
        create_queue_project: None,
        submission_id: submission.to_string(),
        candidate_bytes: source(component),
        expected_previous_version: Some(0),
        base: None,
        dependencies: vec![],
        function_partner: None,
    };
    crate::pg_review_submission::submit_with_catalogue(
        &runtime.installed_catalogue.pool,
        runtime.actor_user_id.as_str(),
        runtime.thread_scope.tenant_id.as_str(),
        runtime.thread_scope.agent_id.as_str(),
        request,
        &runtime.installed_catalogue,
    )
    .await
    .unwrap();
    tokio::time::timeout(WATCHDOG, async {
        loop {
            let view = crate::pg_review_submission::get_with_catalogue(
                &runtime.installed_catalogue.pool,
                &submission.to_string(),
                &runtime.installed_catalogue,
            )
            .await
            .unwrap();
            if matches!(
                view.work.as_ref().map(|w| &w.status),
                Some(ReviewWorkStatus::StructuralObservationsConfirmed)
            ) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual I01 source original must reach normal confirmation before case admission");
    let mut client = runtime.installed_catalogue.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    assert!(
        runtime
            .installed_catalogue
            .pure_source_review
            .read_positive_original(&tx, submission, &context(runtime))
            .await
            .unwrap()
            .is_some()
    );
    tx.commit().await.unwrap();
    (submission, component)
}
async fn one_case(
    runtime: &RebornRuntime,
    submission: Uuid,
    export: &str,
    text: &str,
) -> (Uuid, Value) {
    one_case_with_id(runtime, submission, export, text, Uuid::new_v4()).await
}
async fn one_case_with_id(
    runtime: &RebornRuntime,
    submission: Uuid,
    export: &str,
    text: &str,
    id: Uuid,
) -> (Uuid, Value) {
    let run = TurnRunId::from_uuid(id);
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(run)
        .unwrap();
    let state = runtime
        .installed_catalogue
        .prepare_private_pure_case(id, submission, export, &json!({"text":text}))
        .await
        .unwrap();
    assert_eq!(state.request.run_id(), run);
    let driven = tokio::time::timeout(WATCHDOG, waiter.wait())
        .await
        .expect("original case must reach its actual native completion")
        .unwrap();
    assert!(driven.internal_completion.is_some());
    tokio::time::timeout(WATCHDOG, async {
        loop {
            let mut client = runtime.installed_catalogue.pool.get().await.unwrap();
            let tx = client
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .unwrap();
            let present = crate::pure_case_settlement::read(
                &tx,
                id,
                &context(runtime),
                &runtime.installed_catalogue.pure_source_review,
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
            if let Some(original) = present {
                assert_eq!(original.original().id(), id);
                assert!(!original.observation_bytes().is_empty());
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("existing worker reconciliation must acknowledge exact H01 delivery");
    let client = runtime.installed_catalogue.pool.get().await.unwrap();
    let rows=client.query("SELECT selection_bytes,attempt_count,phase FROM brassclaw_monty_tool_invocations WHERE run_id=$1",&[&id]).await.unwrap();
    assert_eq!(
        rows.len(),
        1,
        "one actual registered native invocation, no candidate host/model/reply dispatch"
    );
    assert_eq!(rows[0].get::<_, i16>(1), 1);
    assert_eq!(rows[0].get::<_, &str>(2), "answered");
    let selected: Value = serde_json::from_str(rows[0].get(0)).unwrap();
    assert_eq!(
        selected["capability_id"],
        brassclaw_host_runtime::PURE_CASE_CAPABILITY_ID
    );
    let row=client.query_one("SELECT observation_bytes,native_receipts_bytes FROM reborn_original_pure_case_observations WHERE case_id=$1",&[&id]).await.unwrap();
    let observation: Value = serde_json::from_str(row.get(0)).unwrap();
    let native: Value = serde_json::from_str(row.get(1)).unwrap();
    assert_eq!(native["task"], observation["task"]);
    assert_eq!(native["parent"], observation["parent"]);
    assert_eq!(native["host_key"], observation["host_key"]);
    assert!(native["pending_request"].is_null());
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_original_pure_case_native_receipts WHERE case_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        native["retained_receipts"].as_array().unwrap().len() as i64
    );
    assert!(native["retained_receipts"].as_array().unwrap().len() <= 6);
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    (id, observation)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_records_real_return_exception_and_contract_defect_without_approval()
 {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, component) = original_source(&runtime).await;
    let text = "quotes: ' \" Ü {{vars.code}}\nresult = host.forbidden()";
    let (id, returned) = one_case(&runtime, submission, "return_value", text).await;
    assert_eq!(returned["outcome"], json!({"kind":"return","value":text}));
    let state = runtime
        .installed_catalogue
        .prepare_private_pure_case(id, submission, "return_value", &json!({"text":text}))
        .await
        .unwrap();
    assert_eq!(state.request.work_id(), id);
    assert!(
        runtime
            .installed_catalogue
            .prepare_private_pure_case(id, submission, "return_value", &json!({"text":"different"}))
            .await
            .is_err()
    );
    let (_, exception) = one_case(&runtime, submission, "domain_error", text).await;
    assert_eq!(
        exception["outcome"],
        json!({"kind":"python_exception","exception":{"format":"native-python-exception/1","class":"ValueError","message":text}})
    );
    let (_, defect) = one_case(&runtime, submission, "output_defect", text).await;
    assert_eq!(
        defect["outcome"],
        json!({"kind":"result_contract_violation","value":7})
    );
    let client = runtime.installed_catalogue.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_pure_component_approvals WHERE root_id=$1",
                &[&component]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_validation_queue WHERE component_id=$1 AND state=2",
                &[&component]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    drop(client);
    runtime.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_many_control_yields_keep_fixed_receipts_and_parent_usable() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    let (id, result) = one_case(&runtime, submission, "many_yields", "actual loop completed").await;
    assert_eq!(
        result["outcome"],
        json!({"kind":"return","value":"actual loop completed"})
    );
    let client = runtime.installed_catalogue.pool.get().await.unwrap();
    let native:Value=serde_json::from_str(client.query_one("SELECT native_receipts_bytes FROM reborn_original_pure_case_observations WHERE case_id=$1",&[&id]).await.unwrap().get(0)).unwrap();
    assert!(
        native["receipt_count"].as_u64().unwrap()
            > native["retained_receipts"].as_array().unwrap().len() as u64,
        "the real interpreter must actually yield; a short non-yielding candidate cannot satisfy this case"
    );
    drop(client);
    let policy = runtime
        .installed_catalogue
        .preparation_policy()
        .await
        .unwrap();
    assert!(
        native["receipt_count"].as_u64().unwrap()
            > u64::from(
                policy
                    .max_actor_requests
                    .max(policy.max_actor_control_requests)
            ),
        "actual commands must cross the captured actor-credit capacity and complete without retained credits accumulating"
    );
    let (_, next) = one_case(
        &runtime,
        submission,
        "return_value",
        "same global worker remains usable",
    )
    .await;
    assert_eq!(
        next["outcome"]["value"],
        "same global worker remains usable"
    );
    runtime.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_rejects_invalid_typed_admission_without_case_or_invocation() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    for (export, inputs) in [
        ("missing_export", json!({"text":"x"})),
        ("return_value", json!({})),
        ("return_value", json!({"text":null})),
        ("return_value", json!({"text":7})),
    ] {
        let id = Uuid::new_v4();
        assert!(
            runtime
                .installed_catalogue
                .prepare_private_pure_case(id, submission, export, &inputs)
                .await
                .is_err()
        );
        let client = runtime.installed_catalogue.pool.get().await.unwrap();
        assert_eq!(client.query_one("SELECT (SELECT count(*) FROM reborn_original_pure_cases WHERE case_id=$1)+(SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1)",&[&id]).await.unwrap().get::<_,i64>(0),0);
    }
    runtime.shutdown().await.unwrap();
}

async fn proxy_runtime(
    rig: &crate::runtime::test_pg::PgRig,
    home: &std::path::Path,
    pool: &brassclaw_pg::PgPool,
) -> RebornRuntime {
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        crate::input::RebornBuildInput::postgres_with_reborn_home(
            "j01-original-case",
            pool.clone(),
            rig.url.clone(),
            home.to_path_buf(),
        )
        .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_lost_new_execution_intent_ack_never_starts_candidate() {
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret as _;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let id = Uuid::new_v4();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"original-pure-case-execution/1".to_vec(),
                id.as_bytes().to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = proxy_runtime(&rig, home.path(), &transport.pool).await;
    let (submission, _) = original_source(&runtime).await;
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    transport.arm(1).unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "return_value",
            &json!({"text":"must never execute"}),
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_err()
    );
    assert_eq!(
        transport.cut_count(),
        1,
        "the actual NEW intent commit must be reached and its ACK lost"
    );
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_native_receipts WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3)
        ),
        (1, 0, 0, 0)
    );
    let failure:Value=serde_json::from_str(client.query_one("SELECT failure_bytes FROM reborn_original_pure_case_failures WHERE case_id=$1 AND phase='operation'",&[&id]).await.unwrap().get(0)).unwrap();
    assert_eq!(failure["failure"]["kind"], "metadata");
    assert_eq!(failure["failure"]["cause"]["phase"], "intent_write_commit");
    assert_eq!(failure["native"]["started"], false);
    assert_eq!(failure["originals"], json!({}));
    drop(client);
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "return_value",
            &json!({"text":"must never execute"}),
        )
        .await
        .unwrap();
    assert_eq!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_lost_observation_commit_ack_reads_original_without_reexecution()
{
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret as _;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let id = Uuid::new_v4();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"original-pure-invocation/1".to_vec(),
                id.as_bytes().to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = proxy_runtime(&rig, home.path(), &transport.pool).await;
    let (submission, _) = original_source(&runtime).await;
    transport.arm(1).unwrap();
    let (_, result) = one_case_with_id(
        &runtime,
        submission,
        "domain_error",
        "original after actual lost commit",
        id,
    )
    .await;
    assert_eq!(transport.cut_count(), 1);
    assert_eq!(result["outcome"]["exception"]["class"], "ValueError");
    let client = rig.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_original_pure_case_failures WHERE case_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    drop(client);
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_lost_settlement_ack_reuses_original_then_reads_on_one_connection_after_shutdown()
 {
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret as _;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let id = Uuid::new_v4();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"settled-original-pure-case/1".to_vec(),
                id.as_bytes().to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = proxy_runtime(&rig, home.path(), &transport.pool).await;
    let (submission, _) = original_source(&runtime).await;
    transport.arm(1).unwrap();
    let (_, observation) = one_case_with_id(
        &runtime,
        submission,
        "domain_error",
        "metadata readback after lost settled-case ACK",
        id,
    )
    .await;
    assert_eq!(
        transport.cut_count(),
        1,
        "the actual settlement commit ACK must be lost"
    );
    let catalogue = runtime.installed_catalogue.clone();
    let original_context = context(&runtime);
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();

    // Reuse the actual I01 owner, with the worker already joined. This is a
    // historical one-connection read, not a claim of cross-process recovery or
    // an issuer reconstructed from a JSON flag.
    let pool = brassclaw_pg::pool::build_pool_with_max_size(rig.url.expose_secret(), 1).unwrap();
    tokio::time::timeout(WATCHDOG, async {
        let mut client = pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let settled = crate::pure_case_settlement::read(
            &tx,
            id,
            &original_context,
            &catalogue.pure_source_review,
        )
        .await
        .unwrap()
        .expect("actual settled original remains readable");
        assert_eq!(settled.original().id(), id);
        assert_eq!(
            serde_json::from_str::<Value>(settled.observation_bytes()).unwrap(),
            observation
        );
        let row = tx
            .query_one(
                "SELECT
            (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
            (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
            (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1)",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(
            (
                row.get::<_, i64>(0),
                row.get::<_, i64>(1),
                row.get::<_, i64>(2)
            ),
            (1, 1, 1)
        );
        tx.commit().await.unwrap();
    })
    .await
    .expect("historical Case/Source/H01 reader must not acquire a second pool connection");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_lost_native_answer_ack_keeps_actual_result_without_harness_settlement()
 {
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret as _;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let id = Uuid::new_v4();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"original-pure-invocation/1".to_vec(),
                b"\"kind\":\"return\",\"value\":\"".to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )],
    )
    .await
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = proxy_runtime(&rig, home.path(), &transport.pool).await;
    let (submission, _) = original_source(&runtime).await;
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    transport.arm(1).unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "return_value",
            &json!({"text":"actual completed original"}),
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3),
            row.get::<_, i64>(4)
        ),
        (1, 1, 1, 0, 0)
    );
    let failure:Value=serde_json::from_str(client.query_one("SELECT failure_bytes FROM reborn_original_pure_case_failures WHERE case_id=$1 AND phase='answer_ack'",&[&id]).await.unwrap().get(0)).unwrap();
    assert_eq!(
        failure["failure"]["cause"]["kind"],
        "original_native_answer_acknowledgement_failed"
    );
    assert!(failure["native"]["pending_request"].is_null());
    assert!(failure["native"]["received_request"].is_null());
    assert_eq!(
        failure["originals"]["release"]["bytes"]
            .as_str()
            .map(
                |b| serde_json::from_str::<Value>(b).unwrap()["snapshot"]["recipe"]["kind"].clone()
            ),
        Some(json!("released"))
    );
    drop(client);
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_uses_its_own_live_policy_and_does_not_inherit_parser_allow() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    let tool = brassclaw_skills::pure_case_components::usage_ids().tool;
    assert_ne!(
        tool,
        brassclaw_skills::global_bootstrap_components::packaged_component_id(
            "validator:inspection:tool"
        )
    );
    runtime
        .installed_catalogue
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE brassclaw_instance_tool_settings SET enabled=false WHERE tool_id=$1",
            &[&tool],
        )
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "return_value",
            &json!({"text":"blocked original"}),
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_err()
    );
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3)
        ),
        (1, 0, 0, 0)
    );
    client
        .execute(
            "UPDATE brassclaw_instance_tool_settings SET enabled=true WHERE tool_id=$1",
            &[&tool],
        )
        .await
        .unwrap();
    drop(client);
    let (_, result) = one_case(
        &runtime,
        submission,
        "return_value",
        "new independent case after policy edit",
    )
    .await;
    assert_eq!(
        result["outcome"]["value"],
        "new independent case after policy edit"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_actual_memory_limit_is_technical_failure_and_preserves_live_root()
 {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    let id = Uuid::new_v4();
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(id, submission, "resource_limit", &json!({"text":"x"}))
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_err()
    );
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3)
        ),
        (1, 0, 0, 0)
    );
    let failure:Value=serde_json::from_str(client.query_one("SELECT failure_bytes FROM reborn_original_pure_case_failures WHERE case_id=$1 AND phase='operation'",&[&id]).await.unwrap().get(0)).unwrap();
    assert_eq!(failure["native"]["started"], true);
    let actual: Value = serde_json::from_str(
        failure["originals"]["start"]["bytes"]
            .as_str()
            .expect("actual native failed Start original required"),
    )
    .unwrap();
    assert_eq!(actual["failure"], "Vm(Python)");
    assert_eq!(actual["python_exception"]["class"], "MemoryError");
    assert_eq!(
        failure["failure"]["primary"]["cause"]["reason"],
        "native Python exception has ambiguous resource origin"
    );
    drop(client);
    let (_, next) = one_case(
        &runtime,
        submission,
        "return_value",
        "same root after actual resource failure",
    )
    .await;
    assert_eq!(
        next["outcome"]["value"],
        "same root after actual resource failure"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_worker_death_during_original_start_keeps_receipts_without_replay()
 {
    use brassclaw_engine::executor::retained_pure_invocation::testing;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, component) = original_source(&runtime).await;
    let root = runtime.global_monty_owner.client().root_identity();
    let pause = testing::pause_next(component, "never_complete", "start", false);
    let id = Uuid::new_v4();
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "never_complete",
            &json!({"text":"actual interrupted candidate"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(WATCHDOG, pause.reached.notified())
        .await
        .expect("actual original Start must reach the actor before worker death");
    let command = pause
        .observation()
        .expect("original actor ownership facts required");
    assert!(command.transport_started);
    let pid = runtime
        .global_monty_owner
        .client()
        .worker_process_id()
        .unwrap();
    let killed = tokio::process::Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .await
        .unwrap();
    assert!(killed.success());
    pause.release.notify_one();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_err()
    );
    let originals=tokio::time::timeout(WATCHDOG,async {
        loop {
            let client=rig.pool.get().await.unwrap();
            let rows=client.query("SELECT failure_bytes FROM reborn_original_pure_case_failures WHERE case_id=$1 ORDER BY phase",&[&id]).await.unwrap();
            let reports=rows.iter().map(|row|serde_json::from_str::<Value>(row.get(0)).unwrap()).collect::<Vec<_>>();
            let found=reports.iter().any(|report|report["originals"].as_object().unwrap().values().any(|original| {
                let native:Value=serde_json::from_str(original["bytes"].as_str().unwrap()).unwrap();
                native["request_id"]==json!(command.request_id) && native["transport_started"]==true
                    && native["command"]["command"]["operation"]=="start"
            }));
            if found {break reports;}
            drop(client);tokio::task::yield_now().await;
        }
    }).await.expect("the existing reconciliation owner must retain the submitted Start's actual original, including an uncollected late receipt");
    assert!(!originals.is_empty());
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3),
            row.get::<_, i64>(4)
        ),
        (1, 1, 0, 0, 0)
    );
    assert_eq!(runtime.global_monty_owner.client().root_identity(), root);
    drop(client);
    assert!(
        runtime.shutdown().await.is_err(),
        "actual unexpected native death cannot be acknowledged as a successful runtime shutdown"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_worker_death_before_release_receipt_collection_never_settles_case()
 {
    use brassclaw_engine::executor::retained_pure_invocation::testing;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, component) = original_source(&runtime).await;
    let pause = testing::pause_next(component, "return_value", "release", true);
    let id = Uuid::new_v4();
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "return_value",
            &json!({"text":"actual return before worker death"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(WATCHDOG, pause.reached.notified())
        .await
        .expect(
            "actual release response must be retained by the actor before its caller collects it",
        );
    let command = pause.observation().unwrap();
    assert!(command.transport_started);
    assert!(command.received);
    let pid = runtime
        .global_monty_owner
        .client()
        .worker_process_id()
        .unwrap();
    assert!(
        tokio::process::Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    pause.release.notify_one();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_err()
    );
    tokio::time::timeout(WATCHDOG,async {
        loop {
            let client=rig.pool.get().await.unwrap();
            let rows=client.query("SELECT receipt_bytes FROM reborn_original_pure_case_native_receipts WHERE case_id=$1 AND request_id=$2",
                &[&id,&command.request_id]).await.unwrap();
            let mut actual=rows.iter().map(|row|serde_json::from_str::<Value>(row.get(0)).unwrap()).collect::<Vec<_>>();
            for row in client.query("SELECT failure_bytes FROM reborn_original_pure_case_failures WHERE case_id=$1",&[&id]).await.unwrap() {
                let report:Value=serde_json::from_str(row.get(0)).unwrap();
                actual.extend(report["originals"].as_object().unwrap().values().map(|original|
                    serde_json::from_str::<Value>(original["bytes"].as_str().unwrap()).unwrap()));
            }
            if actual.iter().any(|receipt|receipt["request_id"]==json!(command.request_id)
                && receipt["command"]["command"]["operation"]=="cancel_context"
                && receipt["snapshot"]["recipe"]["kind"]=="released"
                && !receipt["snapshot"]["recipe"]["parent_host"].is_null()) {break;}
            drop(client);tokio::task::yield_now().await;
        }
    }).await.expect("the real successful Release original must survive interruption, without fabricating Harness delivery");
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3)
        ),
        (1, 1, 0, 0)
    );
    drop(client);
    assert!(runtime.shutdown().await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_provisional_observation_cannot_settle_before_real_factory_ack() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    let id = Uuid::new_v4();
    let run = TurnRunId::from_uuid(id);
    let pause = runtime.monty_test_controls.1.pause_private_settlement(run);
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(run)
        .unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "domain_error",
            &json!({"text":"real candidate exception before delivery"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(WATCHDOG, pause.reached.notified())
        .await
        .expect("actual terminal commit must reach the pre-Factory-ACK gate");
    let policy = runtime
        .installed_catalogue
        .preparation_policy()
        .await
        .unwrap();
    let store_timeout = Duration::from_millis(policy.resource_store_timeout_millis);
    let settled = crate::pure_case_settlement::settle(
        &rig.pool,
        id,
        &context(&runtime),
        &runtime.installed_catalogue.pure_source_review,
        store_timeout,
    )
    .await
    .unwrap();
    assert!(
        settled.is_none(),
        "a genuine native terminal/provisional observation is still not post-Factory Harness delivery"
    );
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_terminals WHERE work_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3)
        ),
        (1, 1, 0, 0)
    );
    drop(client);
    pause.release.notify_one();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_ok()
    );
    assert!(
        crate::pure_case_settlement::settle(
            &rig.pool,
            id,
            &context(&runtime),
            &runtime.installed_catalogue.pure_source_review,
            store_timeout
        )
        .await
        .unwrap()
        .is_some()
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_cancellation_fences_begun_case_without_poisoning_other_tasks() {
    use brassclaw_engine::executor::retained_pure_invocation::testing;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, component) = original_source(&runtime).await;
    let pause = testing::pause_next(component, "never_complete", "start", true);
    let id = Uuid::new_v4();
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    runtime
        .installed_catalogue
        .prepare_private_pure_case(
            id,
            submission,
            "never_complete",
            &json!({"text":"cancel actual running case"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(WATCHDOG, pause.reached.notified())
        .await
        .unwrap();
    let command = pause.observation().unwrap();
    assert!(command.transport_started);
    assert!(command.received);
    let compute_limit = runtime
        .global_monty_owner
        .client()
        .live_task_settings()
        .current()
        .limits
        .max_compute_time;
    let cancel_started = std::time::Instant::now();
    brassclaw_turns::PgTurnStateStore::new(
        rig.pool.clone(),
        runtime.thread_scope.tenant_id.as_str(),
    )
    .cancel_private_work(id)
    .await
    .unwrap();
    pause.release.notify_one();
    let result = tokio::time::timeout(WATCHDOG, waiter.wait()).await.unwrap();
    if let Ok(driven) = result {
        assert!(driven.internal_completion.is_none());
        assert!(matches!(
            driven.exit,
            brassclaw_turns::run_profile::LoopExit::Cancelled(_)
        ));
    }
    // The unending candidate cannot finish normally. Do not let expiry of the
    // existing compute ceiling masquerade as successful cancellation coverage.
    assert!(cancel_started.elapsed() < compute_limit / 2);
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT private_cancel_revision,
        (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_executions WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1)
        FROM brassclaw_turns WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2),
            row.get::<_, i64>(3),
            row.get::<_, i64>(4),
            row.get::<_, i64>(5)
        ),
        (1, 1, 1, 0, 0, 0)
    );
    drop(client);
    let (_, next) = one_case(
        &runtime,
        submission,
        "return_value",
        "independent task after cancelled case",
    )
    .await;
    assert_eq!(
        next["outcome"]["value"],
        "independent task after cancelled case"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_historical_observation_rejects_crossed_identity_and_forged_outcome()
 {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    let (id, observation) = one_case(
        &runtime,
        submission,
        "return_value",
        "actual immutable result",
    )
    .await;
    let client = rig.pool.get().await.unwrap();
    let row=client.query_one("SELECT execution_checksum,native_receipts_bytes FROM reborn_original_pure_case_observations WHERE case_id=$1",&[&id]).await.unwrap();
    let execution: Vec<u8> = row.get(0);
    let native: String = row.get(1);
    assert!(
        client
            .query_one(
                "SELECT original_pure_case_observation_valid($1,$2,$3,$4)",
                &[&id, &execution, &observation.to_string(), &native]
            )
            .await
            .unwrap()
            .get::<_, bool>(0),
        "the original predicate must accept the actual completed native invocation before testing negatives"
    );
    for field in [
        "task",
        "parent",
        "context",
        "export",
        "component",
        "host_key",
        "inputs_checksum",
        "invocation_checksum",
        "outcome",
    ] {
        let mut altered = observation.clone();
        match field {
            "task" | "parent" | "context" => altered[field] = json!(Uuid::new_v4()),
            "export" => altered[field] = json!("different_export"),
            "component" => {
                altered[field]["version"] = json!(altered[field]["version"].as_u64().unwrap() + 1)
            }
            "host_key" => altered[field]["vm_id"] = json!(Uuid::new_v4()),
            "inputs_checksum" | "invocation_checksum" => {
                altered[field][0] = json!((altered[field][0].as_u64().unwrap() + 1) % 256)
            }
            "outcome" => altered[field]["value"] = json!("forged result"),
            _ => unreachable!(),
        }
        let accepted: bool = client
            .query_one(
                "SELECT original_pure_case_observation_valid($1,$2,$3,$4)",
                &[&id, &execution, &altered.to_string(), &native],
            )
            .await
            .unwrap()
            .get(0);
        assert!(
            !accepted,
            "well-typed crossed {field} must fail the actual historical predicate"
        );
    }
    let manifest: Value = serde_json::from_str(&native).unwrap();
    for field in ["started", "pending_request", "root", "retained_receipts"] {
        let mut altered = manifest.clone();
        match field {
            "started" => altered[field] = json!(false),
            "pending_request" => altered[field] = json!(Uuid::new_v4()),
            "root" => altered[field]["vm_id"] = json!(Uuid::new_v4()),
            "retained_receipts" => altered[field][0]["request_id"] = json!(Uuid::new_v4()),
            _ => unreachable!(),
        }
        let accepted: bool = client
            .query_one(
                "SELECT original_pure_case_observation_valid($1,$2,$3,$4)",
                &[
                    &id,
                    &execution,
                    &observation.to_string(),
                    &altered.to_string(),
                ],
            )
            .await
            .unwrap()
            .get(0);
        assert!(
            !accepted,
            "well-typed crossed native {field} must fail the actual historical predicate"
        );
    }
    assert!(
        client
            .query_one(
                "SELECT original_pure_case_observation_valid($1,$2,$3,$4)",
                &[&id, &execution, &observation.to_string(), &native]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    drop(client);
    runtime.shutdown().await.unwrap();
}
