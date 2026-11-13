//! Real Source -> original case/Work -> current global VM -> registered kernel
//! primitive -> normal ResultOnly delivery. These tests grant no candidate Q1.
use super::local_dev_runtime_policy;
use crate::{RebornRuntime, RebornRuntimeInput, build_reborn_runtime};
use brassclaw_product_workflow::{
    MontyMemoryMode, OriginalPureCaseStatus, OriginalPureCaseView, RebornServicesApi as _,
    ReviewWorkStatus, SubmitComponentReviewRequest, SubmitOriginalPureCaseRequest,
    WebUiAuthenticatedCaller,
};
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
fn caller(runtime: &RebornRuntime) -> WebUiAuthenticatedCaller {
    WebUiAuthenticatedCaller::new(
        runtime.thread_scope.tenant_id.clone(),
        runtime.actor_user_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
    )
}
fn product_api(runtime: &RebornRuntime) -> brassclaw_product_workflow::RebornServices {
    // The production product facade and actual catalogue-bound PG owner, not a
    // fake executor, service or successful response. Reuses this running runtime.
    brassclaw_product_workflow::RebornServices::new(
        runtime.webui_thread_service(),
        runtime.webui_turn_coordinator(),
    )
    .with_recipe_store(std::sync::Arc::new(
        crate::pg_recipe_store::PgRecipeStoreFacade::new(
            runtime.installed_catalogue.pool.clone(),
            runtime.thread_scope.tenant_id.as_str(),
            runtime.thread_scope.agent_id.as_str(),
        )
        .with_catalogue(runtime.installed_catalogue.clone()),
    ))
}
async fn submit_case(
    runtime: &RebornRuntime,
    id: Uuid,
    submission: Uuid,
    export: &str,
    inputs: &Value,
) -> Result<OriginalPureCaseView, brassclaw_product_workflow::RebornServicesError> {
    product_api(runtime)
        .submit_original_pure_case(
            caller(runtime),
            SubmitOriginalPureCaseRequest {
                case_id: id.to_string(),
                submission_id: submission.to_string(),
                export: export.into(),
                inputs: inputs.clone(),
            },
            None,
        )
        .await
}
async fn assert_technical_case(runtime: &RebornRuntime, id: Uuid) {
    tokio::time::timeout(WATCHDOG,async {
        loop {
            let view=product_api(runtime).get_original_pure_case(caller(runtime),&id.to_string()).await.unwrap();
            assert!(view.observation_bytes.is_none());
            assert!(view.settlement_checksum.is_none());
            assert_ne!(view.status,OriginalPureCaseStatus::Settled);
            assert_ne!(view.status,OriginalPureCaseStatus::ObservationPendingDelivery,
                "a failed/unresolved Harness cannot be presented as a merely pending observation delivery");
            match view.status {
                OriginalPureCaseStatus::TechnicalFailure|OriginalPureCaseStatus::ReconciliationRequired=>break,
                _=>tokio::task::yield_now().await,
            }
        }
    }).await.expect("actual failed/unresolved terminal must have a technical operator projection");
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
            name.to_owned(),
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
    product_api(runtime)
        .submit_component_review(caller(runtime), request, None)
        .await
        .unwrap();
    tokio::time::timeout(WATCHDOG, async {
        loop {
            let view = product_api(runtime)
                .get_component_review_submission(caller(runtime), &submission.to_string())
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
    let state = submit_case(runtime, id, submission, export, &json!({"text":text}))
        .await
        .unwrap();
    assert_eq!(state.case_id, id.to_string());
    let driven = tokio::time::timeout(WATCHDOG, waiter.wait())
        .await
        .expect("original case must reach its actual native completion")
        .unwrap();
    assert!(driven.internal_completion.is_some());
    tokio::time::timeout(WATCHDOG, async {
        loop {
            let original = product_api(runtime)
                .get_original_pure_case(caller(runtime), &id.to_string())
                .await
                .unwrap_or_else(|error| panic!("actual original case read failed: case_id={id} export={export} error={error:?}"));
            assert_eq!(original.case_id, id.to_string());
            if original.status == OriginalPureCaseStatus::Settled {
                assert!(original.settlement_checksum.is_some());
                assert!(!original.observation_bytes.as_deref().unwrap().is_empty());
                break;
            }
            assert!(original.settlement_checksum.is_none());
            assert!(original.observation_bytes.is_none());
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
    // L-01: the actual native case above traverses V213's preserved J guards.
    // Read its real original and verify its row policy in this same snapshot;
    // no SQL-generated Case, approval or successful effect substitutes for it.
    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        assert!(
            tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM refinery_schema_history WHERE version=213)",
                &[]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
        );
        let row=tx.query_one("SELECT w.private_request_bytes,w.private_request_checksum,p.definition_bytes,p.checksum FROM brassclaw_turns w JOIN brassclaw_monty_private_preparations p USING(work_id) WHERE w.work_id=$1",&[&id]).await.unwrap();
        let request: brassclaw_turns::private_work::PrivateWorkRequest =
            serde_json::from_str(row.get(0)).unwrap();
        let raw: Value = serde_json::from_str(row.get(2)).unwrap();
        assert_eq!(raw["format"], "private-work-definition/2");
        assert_eq!(raw["domain"]["purpose"], "original_pure_case/1");
        assert!(raw.get("root_preparation_bytes").is_some());
        assert!(raw.get("root_preparation_checksum").is_some());
        let restored = crate::private_work_definition::PrivateWorkDefinition::restore(
            &tx,
            &request,
            row.get::<_, Vec<u8>>(1).try_into().unwrap(),
            row.get(2),
            row.get::<_, Vec<u8>>(3).try_into().unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(
            restored.root().reference().version,
            raw["root_revision_version"].as_u64().unwrap()
        );
        tx.commit().await.unwrap();
    }
    assert_eq!(returned["outcome"], json!({"kind":"return","value":text}));
    let state = submit_case(
        &runtime,
        id,
        submission,
        "return_value",
        &json!({"text":text}),
    )
    .await
    .unwrap();
    assert_eq!(state.case_id, id.to_string());
    assert!(
        submit_case(
            &runtime,
            id,
            submission,
            "return_value",
            &json!({"text":"different"})
        )
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
            submit_case(&runtime, id, submission, export, &inputs)
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
    submit_case(
        &runtime,
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
    submit_case(
        &runtime,
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
    assert_technical_case(&runtime, id).await;
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
    submit_case(
        &runtime,
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
    assert_technical_case(&runtime, id).await;
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
    submit_case(
        &runtime,
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
    assert_technical_case(&runtime, id).await;
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
    submit_case(
        &runtime,
        id,
        submission,
        "resource_limit",
        &json!({"text":"x"}),
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
    // This case targets the allocator's actual Python MemoryError path. The
    // separate control tracker resource path is not a Python domain exception.
    assert_eq!(
        failure["failure"]["primary"]["cause"],
        json!({"kind":"process","failure":{"kind":"vm","failure":"Python"}})
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
    assert_technical_case(&runtime, id).await;
    let historical_api = product_api(&runtime);
    let historical_caller = caller(&runtime);
    runtime.shutdown().await.unwrap();
    let closed = historical_api
        .get_original_pure_case(historical_caller, &id.to_string())
        .await
        .unwrap();
    assert_eq!(closed.status, OriginalPureCaseStatus::TechnicalFailure);
    assert!(closed.observation_bytes.is_none());
    assert!(closed.settlement_checksum.is_none());
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
    submit_case(
        &runtime,
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
    submit_case(
        &runtime,
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
    submit_case(
        &runtime,
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
    submit_case(
        &runtime,
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
            brassclaw_turns::LoopExit::Cancelled(_)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_operator_http_uses_original_capture_and_real_delivery() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use sha2::Digest as _;
    use tower::ServiceExt as _;
    struct Operator(brassclaw_host_api::UserId);
    #[async_trait::async_trait]
    impl crate::WebuiAuthenticator for Operator {
        async fn authenticate(&self, token: &str) -> Option<brassclaw_host_api::UserId> {
            matches!(token, "j01-case-operator" | "j01-case-user").then(|| self.0.clone())
        }
        fn allows_operator_webui_config(&self) -> bool {
            true
        }
        async fn authenticate_operator(&self, token: &str) -> Option<brassclaw_host_api::UserId> {
            (token == "j01-case-operator").then(|| self.0.clone())
        }
    }
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = std::sync::Arc::new(start(&rig, home.path()).await);
    let (submission, _) = original_source(&runtime).await;
    let app = crate::webui_v2_app(
        crate::build_webui_services(runtime.clone(), None)
            .await
            .unwrap(),
        crate::WebuiServeConfig::new(
            runtime.thread_scope.tenant_id.clone(),
            std::sync::Arc::new(Operator(runtime.actor_user_id.clone())),
            vec![],
        ),
    )
    .unwrap();
    let id = Uuid::new_v4();
    let uri = "/api/webchat/v2/original-pure-cases";
    let request = json!({"case_id":id,"submission_id":submission,"export":"return_value",
        "inputs":{"text":"HTTP original Ü {{not_source}} 'quotes'"}})
    .to_string();
    for token in [None, Some("j01-case-user")] {
        let mut builder = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .clone()
            .oneshot(builder.body(Body::from(request.clone())).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "ordinary authenticated user is not the protected operator ingress"
        );
    }
    let client = rig.pool.get().await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_original_pure_cases WHERE case_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    drop(client);
    let pause = runtime
        .monty_test_controls
        .1
        .pause_private_settlement(TurnRunId::from_uuid(id));
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    let old_preparation = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    let captured = std::sync::Arc::new(tokio::sync::Notify::new());
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    *runtime.installed_catalogue.capture_gate.lock().await =
        Some((captured.clone(), release.clone()));
    let send = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("authorization", "Bearer j01-case-operator")
            .header("content-type", "application/json")
            .body(Body::from(request.clone()))
            .unwrap(),
    );
    let change = async {
        tokio::time::timeout(WATCHDOG, captured.notified())
            .await
            .unwrap();
        let store = runtime.webui_monty_settings_store();
        let before = store.get("default", "default").await.unwrap();
        assert_eq!(before.revision, old_preparation.revision());
        let mut limits = before.execution_limits;
        limits.max_value_bytes = limits.max_value_bytes.checked_sub(1).unwrap();
        let update = serde_json::from_value(
            json!({"expected_revision":before.revision,"execution_limits":limits}),
        )
        .unwrap();
        let saved = store.upsert("default", "default", &update).await.unwrap();
        assert!(saved.revision > old_preparation.revision());
        release.notify_one();
    };
    let (response, ()) = tokio::join!(send, change);
    let response = response.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let initial: OriginalPureCaseView =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(initial.case_id, id.to_string());
    assert_ne!(initial.status, OriginalPureCaseStatus::Settled);
    assert!(initial.observation_bytes.is_none());
    tokio::time::timeout(WATCHDOG, pause.reached.notified())
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap();
    let definition: Value = serde_json::from_str(row.get(0)).unwrap();
    let original = brassclaw_host_api::task_preparation::RetainedPreparation::restore_original(
        definition["preparation_bytes"].as_str().unwrap(),
        definition["preparation_checksum"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(original.bytes(), old_preparation.bytes());
    assert_eq!(original.checksum(), old_preparation.checksum());
    assert_eq!(original.policy(), old_preparation.policy());
    assert_eq!(
        definition["input_checksum"],
        json!(sha2::Sha256::digest(b"{}").to_vec())
    );
    drop(client);
    let get_uri = format!("{uri}/{id}");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&get_uri)
                .header("authorization", "Bearer j01-case-operator")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let provisional: OriginalPureCaseView =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_ne!(provisional.status, OriginalPureCaseStatus::Settled);
    assert!(provisional.observation_bytes.is_none());
    pause.release.notify_one();
    assert!(
        tokio::time::timeout(WATCHDOG, waiter.wait())
            .await
            .unwrap()
            .is_ok()
    );
    tokio::time::timeout(WATCHDOG, async {
        loop {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(&get_uri)
                        .header("authorization", "Bearer j01-case-operator")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let settled: OriginalPureCaseView =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            if settled.status == OriginalPureCaseStatus::Settled {
                let observed: Value =
                    serde_json::from_str(settled.observation_bytes.as_deref().unwrap()).unwrap();
                assert_eq!(
                    observed["outcome"]["value"],
                    "HTTP original Ü {{not_source}} 'quotes'"
                );
                assert!(settled.settlement_checksum.is_some());
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    for (field, replacement) in [("export", json!("unknown_export")), ("inputs", Value::Null)] {
        let rejected = Uuid::new_v4();
        let mut bad: Value = serde_json::from_str(&request).unwrap();
        bad["case_id"] = json!(rejected);
        bad[field] = replacement;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("authorization", "Bearer j01-case-operator")
                    .header("content-type", "application/json")
                    .body(Body::from(bad.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let counts = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT (SELECT count(*) FROM reborn_original_pure_cases WHERE case_id=$1),
                (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1)",
                &[&rejected],
            )
            .await
            .unwrap();
        assert_eq!((counts.get::<_, i64>(0), counts.get::<_, i64>(1)), (0, 0));
    }
    let mut changed: Value = serde_json::from_str(&request).unwrap();
    changed["inputs"] = json!({"text":"different data under same original ID"});
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("authorization", "Bearer j01-case-operator")
                .header("content-type", "application/json")
                .body(Body::from(changed.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    // Exact same-ID HTTP retry returns metadata, never another candidate start.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("authorization", "Bearer j01-case-operator")
                .header("content-type", "application/json")
                .body(Body::from(request))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
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
    drop(app);
    std::sync::Arc::try_unwrap(runtime)
        .ok()
        .unwrap()
        .shutdown()
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_actual_compute_tracker_limit_is_not_a_python_outcome() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    let store = runtime.webui_monty_settings_store();
    let before = store.get("default", "default").await.unwrap();
    let update =
        serde_json::from_value(json!({"expected_revision":before.revision,"max_duration_secs":1}))
            .unwrap();
    let saved = store.upsert("default", "default", &update).await.unwrap();
    assert_eq!(saved.max_duration_secs, 1);
    let id = Uuid::new_v4();
    let waiter = runtime
        .monty_test_controls
        .0
        .private_completion_waiter(TurnRunId::from_uuid(id))
        .unwrap();
    submit_case(
        &runtime,
        id,
        submission,
        "never_complete",
        &json!({"text":"actual shared task compute limit"}),
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
    let failure:Value=serde_json::from_str(client.query_one(
        "SELECT failure_bytes FROM reborn_original_pure_case_failures WHERE case_id=$1 AND phase='operation'",&[&id])
        .await.unwrap().get(0)).unwrap();
    assert_eq!(
        failure["failure"]["primary"]["native_failure"],
        json!({"kind":"vm","failure":"ResourceLimit"})
    );
    let actual = failure["originals"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(|original| original["bytes"].as_str())
        .map(|bytes| serde_json::from_str::<Value>(bytes).unwrap())
        .find(|record| record["failure"] == "Vm(ResourceLimit)")
        .expect("actual tracker resource-failed receipt required");
    assert!(actual["python_exception"].is_null());
    assert!(
        actual["snapshot"]["task_accounting"]
            .as_array()
            .unwrap()
            .iter()
            .any(|account| account["task"] == failure["native"]["task"]
                && !account["failure"].is_null())
    );
    let counts = client
        .query_one(
            "SELECT
        (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_observations WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_deliveries WHERE work_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, i64>(1),
            counts.get::<_, i64>(2),
            counts.get::<_, i64>(3)
        ),
        (1, 0, 0, 0)
    );
    drop(client);
    assert_technical_case(&runtime, id).await;
    let (_, next) = one_case(
        &runtime,
        submission,
        "return_value",
        "independent task after compute failure",
    )
    .await;
    assert_eq!(
        next["outcome"]["value"],
        "independent task after compute failure"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_actual_receipt_envelope_obeys_acknowledged_byte_boundary() {
    use serde::Serialize;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (submission, _) = original_source(&runtime).await;
    let (id, observed) = one_case(
        &runtime,
        submission,
        "return_value",
        &"quotes \\\" Ü\n".repeat(64),
    )
    .await;
    let client = rig.pool.get().await.unwrap();
    let rows=client.query("SELECT receipt_bytes FROM reborn_original_pure_case_native_receipts WHERE case_id=$1 ORDER BY ordinal",&[&id]).await.unwrap();
    let originals = rows
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert!((4..=6).contains(&originals.len()));
    drop(client);
    #[derive(Serialize)]
    struct BorrowedActual<'a> {
        format: &'static str,
        originals: Vec<&'a str>,
    }
    let borrowed = BorrowedActual {
        format: "actual-original-receipt-capacity-test/1",
        originals: originals.iter().map(String::as_str).collect(),
    };
    // Actual worker receipts, not fabricated outcomes. This is a pure metadata
    // encoding boundary test and writes no new Case observation/failure rows.
    let exact = brassclaw_common::json::count_json_bytes(&borrowed, None).unwrap();
    let store = runtime.webui_monty_settings_store();
    let before = store.get("default", "default").await.unwrap();
    let mut limits = before.execution_limits;
    assert!(exact < limits.max_ipc_frame_bytes && exact > 1);
    limits.max_value_bytes = exact;
    let update = serde_json::from_value(
        json!({"expected_revision":before.revision,"execution_limits":limits}),
    )
    .unwrap();
    store.upsert("default", "default", &update).await.unwrap();
    let original = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert_eq!(original.policy().max_value_bytes, exact);
    let encoded =
        crate::pure_case_evidence::encode_original(&borrowed, original.policy().max_value_bytes)
            .unwrap();
    assert_eq!(encoded.len() as u64, exact);
    let saved = store.get("default", "default").await.unwrap();
    limits.max_value_bytes = exact - 1;
    let update = serde_json::from_value(
        json!({"expected_revision":saved.revision,"execution_limits":limits}),
    )
    .unwrap();
    store.upsert("default", "default", &update).await.unwrap();
    let narrower = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert!(matches!(
        crate::pure_case_evidence::encode_original(&borrowed, narrower.policy().max_value_bytes),
        Err(crate::pure_case_evidence::CaseEncodingError::PolicyCapacity)
    ));
    let historical = product_api(&runtime)
        .get_original_pure_case(caller(&runtime), &id.to_string())
        .await
        .unwrap();
    assert_eq!(historical.status, OriginalPureCaseStatus::Settled);
    assert_eq!(
        serde_json::from_str::<Value>(historical.observation_bytes.as_deref().unwrap()).unwrap(),
        observed
    );
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
    let saved = store.get("default", "default").await.unwrap();
    let restore = serde_json::from_value(
        json!({"expected_revision":saved.revision,"execution_limits":before.execution_limits}),
    )
    .unwrap();
    store.upsert("default", "default", &restore).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_pure_case_initial_live_block_is_visible_unready_without_blocking_source_work()
 {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let tool = brassclaw_skills::pure_case_components::usage_ids().tool;
    // Enforce the real instance policy as soon as this new component identity
    // exists, before its first fixture invocation. No qualification is faked.
    rig.pool.get().await.unwrap().batch_execute(&format!(r#"
        CREATE FUNCTION block_new_original_case_tool_for_test() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.component_id='{tool}'::uuid THEN
                INSERT INTO brassclaw_instance_tool_settings(tool_id,enabled) VALUES(NEW.component_id,false)
                    ON CONFLICT(tool_id) DO NOTHING;
            END IF;
            RETURN NEW;
        END $$;
        CREATE TRIGGER block_new_original_case_tool_for_test AFTER INSERT ON reborn_component_revision_heads
            FOR EACH ROW EXECUTE FUNCTION block_new_original_case_tool_for_test();
    "#)).await.unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    assert!(runtime.installed_catalogue.case_harness.is_none());
    let readiness = product_api(&runtime)
        .catalogue_status(caller(&runtime))
        .await
        .unwrap()
        .original_pure_case
        .expect("existing catalogue status exposes optional private support");
    assert!(!readiness.ready);
    assert!(!readiness.evidence_pending);
    assert_eq!(readiness.phase.as_deref(), Some("initial_tool_policy"));
    assert_eq!(readiness.reason_kind.as_deref(), Some("case_tool_blocked"));
    let (submission, _) = original_source(&runtime).await;
    let id = Uuid::new_v4();
    let error = submit_case(
        &runtime,
        id,
        submission,
        "return_value",
        &json!({"text":"blocked only new case capability"}),
    )
    .await
    .err()
    .expect("unqualified case support cannot admit a native invocation");
    assert_eq!(error.status_code, 503);
    let client = rig.pool.get().await.unwrap();
    let counts = client
        .query_one(
            "SELECT
        (SELECT count(*) FROM reborn_original_pure_cases WHERE case_id=$1),
        (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1),
        (SELECT count(*) FROM reborn_original_pure_case_settlements WHERE case_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, i64>(1),
            counts.get::<_, i64>(2)
        ),
        (0, 0, 0)
    );
    assert!(
        !client
            .query_one(
                "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                &[&tool]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    drop(client);
    runtime.shutdown().await.unwrap();
}
