//! Real original authoring -> qualified W -> shared Worker -> native parser ->
//! post-Factory delivery -> source-only confirmation. No candidate is invoked.
use super::*;
use crate::RebornRuntime;
use brassclaw_product_workflow::{
    MontyMemoryMode, MontyVmSettingsStore, ReviewWorkStatus, SubmitComponentReviewRequest,
};
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft, selection},
    revision_store::PgComponentRevisionStore,
};
use brassclaw_turns::{PgTurnStateStore, private_work::PrivateWorkRequest};
use serde_json::{Value, json};
use uuid::Uuid;

async fn start(rig: &crate::runtime::test_pg::PgRig, home: &std::path::Path) -> RebornRuntime {
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("i01-qualified-source", home)
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap()
}
async fn deadline(runtime: &RebornRuntime) -> Duration {
    Duration::from_millis(
        runtime
            .installed_catalogue
            .preparation_policy()
            .await
            .unwrap()
            .response_timeout_millis,
    )
}
fn candidate(id: Uuid, hidden_host: bool) -> String {
    let mut value = json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"Authored pure source","description":"Source data; never invoked by this review",
            "prior_knowledge_content":"","override_prompt_creation":false,"consumer_tags":[],
            "intent_examples":null,"dependency_registry":null,"includes":[],
            "similarity_parent_id":null,"replaces_id":null,"parent_version":null,
            "content":"def candidate(inputs):\n    return inputs['text']\n",
            "input_contract":{"text":{"type":"string","required":true,"checks":[]}},"result_contract":{"type":"string"},
            "preload":{"format":"python-preload/2","exports":{"candidate":{"symbol":"candidate","parameters":["inputs"],"mapping":true}},
                "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"candidate"}},
        "dependencies":[],"association":null});
    if hidden_host {
        value["document"]["content"] = json!(
            "def candidate(inputs):\n    return inputs['text']\ndef unused(inputs):\n    return host.post_reply(text=inputs['text'])\n"
        );
        value["document"]["preload"]["exports"]["unused"] =
            json!({"symbol":"unused","parameters":["inputs"],"mapping":true});
    }
    value.to_string()
}
fn request(id: Uuid, component: Uuid, hidden_host: bool) -> SubmitComponentReviewRequest {
    SubmitComponentReviewRequest {
        queue_ids: vec![],
        create_queue_project: None,
        submission_id: id.to_string(),
        candidate_bytes: candidate(component, hidden_host),
        expected_previous_version: Some(0),
        base: None,
        dependencies: vec![],
        function_partner: None,
    }
}
async fn submit(
    runtime: &RebornRuntime,
    request: SubmitComponentReviewRequest,
) -> Result<
    brassclaw_product_workflow::ComponentReviewSubmissionReceipt,
    brassclaw_product_workflow::RecipeStoreError,
> {
    let id = Uuid::parse_str(&request.submission_id).unwrap();
    watchdog(
        runtime,
        id,
        "original submit/readback",
        crate::pg_review_submission::submit_with_catalogue(
            &runtime.installed_catalogue.pool,
            runtime.actor_user_id.as_str(),
            runtime.thread_scope.tenant_id.as_str(),
            runtime.thread_scope.agent_id.as_str(),
            request,
            &runtime.installed_catalogue,
        ),
    )
    .await
}
async fn selection_bytes(runtime: &RebornRuntime, id: Uuid) -> String {
    watchdog(runtime, id, "original selection read", async { runtime.installed_catalogue.pool.get().await.unwrap().query_one(
        "SELECT selection_bytes FROM reborn_component_validator_selections WHERE submission_id=$1", &[&id]
    ).await.unwrap().get(0) }).await
}
async fn read_positive(
    runtime: &RebornRuntime,
    id: Uuid,
) -> Option<crate::validator_inventory_review::CheckedPureSourceOriginal> {
    watchdog(
        runtime,
        id,
        "positive original read",
        async {
        let context = brassclaw_turns::private_work::PrivateExecutionContext {
            tenant_id: runtime.thread_scope.tenant_id.clone(),
            agent_id: Some(runtime.thread_scope.agent_id.clone()),
            project_id: runtime.thread_scope.project_id.clone(),
            actor: Some(brassclaw_turns::TurnActor::new(
                runtime.actor_user_id.clone(),
            )),
        };
        let mut client = runtime.installed_catalogue.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let checked = runtime
            .installed_catalogue
            .pure_source_review
            .read_positive_original(&tx, id, &context)
            .await
            .unwrap();
        if let Some(original) = &checked {
            assert_eq!(original.subject().submission_id(), id);
            assert_eq!(original.selection().submission_id(), id);
            assert_eq!(
                original.selection().subject_checksum(),
                original.subject().subject_checksum()
            );
            assert_eq!(
                original
                    .selection()
                    .source_qualification()
                    .unwrap()
                    .worker_checksum,
                hex::encode(original.worker_checksum())
            );
            assert_eq!(original.delivery_reference().id, id);
            assert_eq!(original.confirmation_reference().id, id);
            let row = tx.query_one("SELECT d.checksum,c.checksum FROM brassclaw_monty_private_deliveries d JOIN reborn_component_review_work_confirmations c ON c.submission_id=d.work_id WHERE d.work_id=$1", &[&id]).await.unwrap();
            assert_eq!(
                original.delivery_reference().checksum,
                hex::encode(row.get::<_, Vec<u8>>(0))
            );
            assert_eq!(
                original.confirmation_reference().checksum,
                hex::encode(row.get::<_, Vec<u8>>(1))
            );
            let mut foreign = context.clone();
            foreign.actor = Some(brassclaw_turns::TurnActor::new(
                brassclaw_host_api::UserId::new("i01-foreign-source-actor").unwrap(),
            ));
            assert!(
                matches!(runtime.installed_catalogue.pure_source_review.read_positive_original(&tx,id,&foreign).await,
                Err(crate::RebornBuildError::InvalidConfig { reason }) if reason=="agent loop driver failed: private_delivery_original_invalid")
            );
        }
        tx.commit().await.unwrap();
        checked
        },
    )
    .await
}
#[path = "qualified_source_progress.rs"]
mod progress;
use progress::{OriginalReview, watchdog};

async fn assert_scope(
    runtime: &RebornRuntime,
    review: &mut OriginalReview,
    component: Uuid,
    pass: bool,
) {
    let id = review.id();
    let record = review.confirmation(runtime).await;
    watchdog(
        runtime,
        id,
        "confirmed scope metadata read",
        async {
        assert_eq!(record["format"], "component-review-work-confirmation/1");
        assert_eq!(record["checks_succeeded"], pass);
        assert_eq!(record["q1_qualified"], false);
        assert_eq!(record["human_q2"], false);
        assert_eq!(record["activation"], false);
        let envelope: Value = serde_json::from_str(&selection_bytes(runtime, id).await).unwrap();
        assert_eq!(envelope["format"], "component-validator-selection/2");
        let inner: Value =
            serde_json::from_str(envelope["validator_selection_bytes"].as_str().unwrap()).unwrap();
        assert_eq!(inner["format"], "monty-retained-recipe-selection/1");
        assert!(
            !inner.to_string().contains(&component.to_string()),
            "candidate is data, never an executable W component"
        );
        let client = runtime.installed_catalogue.pool.get().await.unwrap();
        let rows = client.query("SELECT phase,attempt_count,selection_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&id]).await.unwrap();
        assert_eq!(
            rows.len(),
            1,
            "one actual parser dispatch, zero candidate/model/reply dispatches"
        );
        assert_eq!(rows[0].get::<_, String>(0), "answered");
        assert_eq!(rows[0].get::<_, i16>(1), 1);
        let selected: Value = serde_json::from_str(rows[0].get(2)).unwrap();
        assert_eq!(
            selected["capability_id"],
            brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID
        );
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM reborn_pure_component_approvals WHERE root_id=$1",
                &[&component],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
        let promoted: i64 = client
            .query_one(
                "SELECT count(*) FROM reborn_validation_queue WHERE component_id=$1 AND state=2",
                &[&component],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(promoted, 0);
        },
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_original_reaches_existing_worker() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let mut review = OriginalReview::new(&runtime, id);
    let mut queued = request(id, component, false);
    queued.create_queue_project = Some("i01-review".into());
    let receipt = submit(&runtime, queued).await.unwrap();
    assert_eq!(
        receipt.queue_ids.len(),
        1,
        "the actual new queue projection is captured with the source Work"
    );
    assert!(!receipt.catalogue_activated);
    assert_eq!(receipt.review_status, "unreviewed");
    assert_scope(&runtime, &mut review, component, true).await;
    assert!(read_positive(&runtime, id).await.is_some());
    runtime
        .installed_catalogue
        .read_source_submission(id)
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_rejects_hidden_host_in_unused_export() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let mut review = OriginalReview::new(&runtime, id);
    submit(&runtime, request(id, component, true))
        .await
        .unwrap();
    assert_scope(&runtime, &mut review, component, false).await;
    assert!(
        read_positive(&runtime, id).await.is_none(),
        "a genuine negative source confirmation supplies no positive prerequisite"
    );
    let record = review.confirmation(&runtime).await;
    assert!(record["errors"].to_string().contains("host call"));
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_ignores_draft_heads_and_keeps_original_pins() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let mut review = OriginalReview::new(&runtime, id);
    let captured = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *runtime.installed_catalogue.capture_gate.lock().await =
        Some((captured.clone(), release.clone()));
    let edit = async {
        tokio::time::timeout(deadline(&runtime).await, captured.notified())
            .await
            .unwrap();
        let store = PgComponentRevisionStore::new(rig.pool.clone());
        let helper = brassclaw_skills::global_bootstrap_components::packaged_component_id(
            "validator:subject-sources",
        );
        let old = store.read_revision(helper, 1).await.unwrap();
        let mut draft: Value = serde_json::from_str(old.draft().exact_bytes()).unwrap();
        draft["document"]["description"] =
            json!("Unapproved successor must not replace selected W");
        store
            .stage(
                &ComponentRevisionDraft::from_json(&draft.to_string()).unwrap(),
                1,
            )
            .await
            .unwrap();
        release.notify_one();
    };
    let (result, ()) = tokio::join!(submit(&runtime, request(id, component, false)), edit);
    result.unwrap();
    let original = selection_bytes(&runtime, id).await;
    assert_scope(&runtime, &mut review, component, true).await;
    submit(&runtime, request(id, component, false))
        .await
        .unwrap();
    assert_eq!(selection_bytes(&runtime, id).await, original);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_rejects_swapped_subject_base_and_dependency() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let mut review = OriginalReview::new(&runtime, id);
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let old_root = ComponentRevisionDraft::from_json(&candidate(component, false)).unwrap();
    let base_a = store.stage(&old_root, 0).await.unwrap();
    let base_b = store.stage(&old_root, 1).await.unwrap();
    let helper = Uuid::new_v4();
    let helper_draft = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":helper,"class_code":22,
        "document":{"name":"Pure helper","content":"def helper(value):\n    return value\n",
            "preload":{"format":"python-preload/2","exports":{},"private_functions":{"helper":["value"]},"constants":[],"imports":[],"dependencies":[],"default_export":null}},
        "dependencies":[],"association":null}).to_string()).unwrap();
    let dependency_a = store.stage(&helper_draft, 0).await.unwrap();
    let dependency_b = store.stage(&helper_draft, 1).await.unwrap();
    // Both alternatives have real immutable custody and identical behavior;
    // a different revision remains a different exact review subject.
    let mut original_request = request(id, component, false);
    let mut root: Value = serde_json::from_str(&original_request.candidate_bytes).unwrap();
    root["document"]["content"] =
        json!("def candidate(inputs):\n    return helper(inputs['text'])\n");
    root["dependencies"] = json!([helper]);
    root["document"]["preload"]["dependencies"] = json!([helper]);
    original_request.candidate_bytes = root.to_string();
    original_request.expected_previous_version = Some(2);
    original_request.base = Some(crate::pg_review_submission::selection(base_a));
    original_request.dependencies = vec![crate::pg_review_submission::selection(dependency_a)];
    submit(&runtime, original_request.clone()).await.unwrap();
    assert_scope(&runtime, &mut review, component, true).await;
    let original = selection_bytes(&runtime, id).await;
    let mut changed = original_request.clone();
    root["document"]["description"] = json!("Changed complete source subject");
    changed.candidate_bytes = root.to_string();
    assert!(matches!(
        submit(&runtime, changed.clone()).await,
        Err(brassclaw_product_workflow::RecipeStoreError::Conflict(_))
    ));
    changed = original_request.clone();
    changed.base = Some(crate::pg_review_submission::selection(base_b));
    assert!(matches!(
        submit(&runtime, changed.clone()).await,
        Err(brassclaw_product_workflow::RecipeStoreError::Conflict(_))
    ));
    changed = original_request;
    changed.dependencies = vec![crate::pg_review_submission::selection(dependency_b)];
    assert!(matches!(
        submit(&runtime, changed).await,
        Err(brassclaw_product_workflow::RecipeStoreError::Conflict(_))
    ));
    assert_eq!(selection_bytes(&runtime, id).await, original);
    assert_scope(&runtime, &mut review, component, true).await;
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_atomic_submit_and_original_readback() {
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let evidence_cut = Uuid::new_v4();
    let author_held = Uuid::new_v4();
    let queue_held = Uuid::new_v4();
    let evidence_held = Uuid::new_v4();
    let queue_cut = Uuid::new_v4();
    let author_rule = |rule, id: Uuid| {
        AckCutRule::after_bind(
            rule,
            vec![
                id.as_bytes().to_vec(),
                b"component-validator-selection/2".to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )
    };
    let evidence_rule = |rule, id: Uuid| {
        AckCutRule::after_bind(
            rule,
            vec![
                id.to_string().into_bytes(),
                b"observed-subject-validator-checks/1".to_vec(),
            ],
            vec![],
            AckBoundary::Commit,
        )
    };
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![
            author_rule(1, id),
            evidence_rule(2, evidence_cut),
            author_rule(3, author_held).hold(),
            author_rule(4, queue_held).hold(),
            evidence_rule(5, evidence_held).hold(),
            author_rule(6, queue_cut),
        ],
    )
    .await
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        crate::input::RebornBuildInput::postgres_with_reborn_home(
            "i01-qualified-source",
            transport.pool.as_ref().clone(),
            rig.url.clone(),
            home.path().to_owned(),
        )
        .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    // Bootstrap is unchanged; the actual author/evidence/Worker operations
    // below share exactly one ordinary pooled connection.
    transport.pool.resize(1);
    assert_eq!(transport.pool.status().max_size, 1);
    let mut review = OriginalReview::new(&runtime, id);
    faults::install_write_counter(&rig).await;
    transport.arm(1).unwrap();
    submit(&runtime, request(id, component, false))
        .await
        .unwrap();
    assert_eq!(
        transport.cut_count(),
        1,
        "the actual authoring COMMIT acknowledgement was lost"
    );
    let original = selection_bytes(&runtime, id).await;
    submit(&runtime, request(id, component, false))
        .await
        .unwrap();
    assert_eq!(selection_bytes(&runtime, id).await, original);
    assert_scope(&runtime, &mut review, component, true).await;
    assert_eq!(faults::writes(&rig).await, 1);
    let row = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT
        (SELECT count(*) FROM reborn_component_review_submissions WHERE submission_id=$1),
        (SELECT count(*) FROM brassclaw_monty_private_preparations WHERE work_id=$1),
        (SELECT count(*) FROM brassclaw_turns WHERE record_kind='private_work' AND work_id=$1)",
            &[&id],
        )
        .await
        .unwrap();
    for column in 0..3 {
        assert_eq!(row.get::<_, i64>(column), 1);
    }
    // Lose the real Source-observation metadata ACK, rather than an author or
    // terminal ACK. Original readback must compare its entire evidence/index.
    transport.arm(2).unwrap();
    let code = Uuid::new_v4();
    let mut evidence_review = OriginalReview::new(&runtime, evidence_cut);
    submit(&runtime, request(evidence_cut, code, false))
        .await
        .unwrap();
    assert_scope(&runtime, &mut evidence_review, code, true).await;
    assert_eq!(transport.cut_count(), 2);
    assert_eq!(
        faults::writes(&rig).await,
        2,
        "unknown Source ACK must not repeat the metadata write"
    );
    faults::cancelled_author_commit(
        &runtime,
        &rig,
        &transport,
        3,
        author_held,
        Uuid::new_v4(),
        false,
    )
    .await;
    assert_eq!(faults::writes(&rig).await, 3);
    faults::cancelled_author_commit(
        &runtime,
        &rig,
        &transport,
        4,
        queue_held,
        Uuid::new_v4(),
        true,
    )
    .await;
    assert_eq!(faults::writes(&rig).await, 4);
    faults::timed_out_evidence_commit(&runtime, &rig, &transport, 5, evidence_held, Uuid::new_v4())
        .await;
    assert_eq!(
        faults::writes(&rig).await,
        5,
        "the actual bounded evidence write is never repeated after timeout"
    );
    transport.arm(6).unwrap();
    let code = Uuid::new_v4();
    let mut queue_review = OriginalReview::new(&runtime, queue_cut);
    let mut queued = request(queue_cut, code, false);
    queued.create_queue_project = Some("i01-cut-queue".into());
    let receipt = submit(&runtime, queued).await.unwrap();
    assert_eq!(receipt.queue_ids.len(), 1);
    assert_scope(&runtime, &mut queue_review, code, true).await;
    assert_eq!(transport.cut_count(), 3);
    assert_eq!(faults::writes(&rig).await, 6);
    faults::corrupt_run_index(&runtime, &rig, Uuid::new_v4(), Uuid::new_v4()).await;
    assert_eq!(
        faults::writes(&rig).await,
        7,
        "a corrupt index causes one failed atomic write, never metadata/native replay"
    );
    runtime.shutdown().await.unwrap();
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_cancel_or_lost_native_delivery_never_passes() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (pending, pending_component) = (Uuid::new_v4(), Uuid::new_v4());
    let run = brassclaw_turns::TurnRunId::from_uuid(pending);
    let pause = runtime.monty_test_controls.1.pause_private_settlement(run);
    let mut pending_review = OriginalReview::new(&runtime, pending);
    submit(&runtime, request(pending, pending_component, false))
        .await
        .unwrap();
    pending_review
        .reached(&runtime, pause.reached.notified())
        .await;
    let pending_view = crate::pg_review_submission::get_with_catalogue(
        &rig.pool,
        &pending.to_string(),
        &runtime.installed_catalogue,
    )
    .await
    .unwrap()
    .work
    .unwrap();
    assert!(matches!(
        pending_view.status,
        ReviewWorkStatus::NativeTerminalUnacknowledged
    ));
    assert!(pending_view.delivery_checksum.is_none());
    assert!(pending_view.confirmation_checksum.is_none());
    let partial: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_validator_q1_runs WHERE submission_id=$1",
            &[&pending],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        partial, 1,
        "actual source observations already exist but cannot replace native delivery"
    );
    assert!(
        read_positive(&runtime, pending).await.is_none(),
        "a native terminal without the actual Factory ACK supplies no positive prerequisite"
    );
    pause.release.notify_one();
    pending_review.settled(&runtime).await;
    assert_scope(&runtime, &mut pending_review, pending_component, true).await;
    crate::runtime::test_pg::stop_worker_for_state_fixture(&runtime).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    submit(&runtime, request(id, component, false))
        .await
        .unwrap();
    let store = PgTurnStateStore::new(rig.pool.clone(), runtime.thread_scope.tenant_id.as_str());
    store.cancel_private_work(id).await.unwrap();
    let view = crate::pg_review_submission::get_with_catalogue(
        &rig.pool,
        &id.to_string(),
        &runtime.installed_catalogue,
    )
    .await
    .unwrap()
    .work
    .unwrap();
    assert!(matches!(view.status, ReviewWorkStatus::Cancelled));
    assert!(view.confirmation_checksum.is_none());
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_live_parser_block_prevents_dispatch() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let mut review = OriginalReview::new(&runtime, id);

    let captured = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *runtime.installed_catalogue.capture_gate.lock().await =
        Some((captured.clone(), release.clone()));
    let block = async {
        tokio::time::timeout(deadline(&runtime).await, captured.notified())
            .await
            .unwrap();
        let tool = brassclaw_skills::global_bootstrap_components::packaged_component_id(
            "validator:inspection:tool",
        );
        assert_eq!(rig.pool.get().await.unwrap().execute("UPDATE brassclaw_instance_tool_settings SET enabled=false,revision=revision+1 WHERE tool_id=$1", &[&tool]).await.unwrap(),1);
        release.notify_one();
    };
    let (result, ()) = tokio::join!(submit(&runtime, request(id, component, false)), block);
    result.unwrap();
    review
        .failed(&runtime, Some("recipe_execution_failed"))
        .await;
    let count: i64 = rig.pool.get().await.unwrap().query_one("SELECT count(*) FROM reborn_component_review_work_confirmations WHERE submission_id=$1", &[&id]).await.unwrap().get(0);
    assert_eq!(count, 0, "a live parser block cannot become source success");
    let observed: i64 = rig
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
        observed, 0,
        "blocked native parsing supplies no observed source verdict"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_restore_preserves_original_preparation() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let original = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    let settings = runtime.monty_settings_owner.store();
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let mut review = OriginalReview::new(&runtime, id);
    let support = PgComponentRevisionStore::new(rig.pool.clone())
        .read_revision(
            brassclaw_skills::global_bootstrap_components::packaged_component_id(
                "validator:python-pure-calls",
            ),
            1,
        )
        .await
        .unwrap();
    let support_bytes = support.draft().document()["content"]
        .as_str()
        .unwrap()
        .len();
    let comment = "# retained source data, never invoked\n";
    let mut authored = request(id, component, false);
    let mut document: Value = serde_json::from_str(&authored.candidate_bytes).unwrap();
    document["document"]["content"] = json!(format!(
        "{}{}",
        comment.repeat(2 * support_bytes.div_ceil(comment.len())),
        document["document"]["content"].as_str().unwrap()
    ));
    authored.candidate_bytes = document.to_string();
    let captured = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *runtime.installed_catalogue.capture_gate.lock().await =
        Some((captured.clone(), release.clone()));
    let edit = async {
        tokio::time::timeout(deadline(&runtime).await, captured.notified())
            .await
            .unwrap();
        let before = settings.get("default", "default").await.unwrap();
        let mut limits = before.execution_limits;
        limits.response_timeout_millis += 1;
        let update = serde_json::from_value(
            json!({"expected_revision":before.revision,"execution_limits":limits}),
        )
        .unwrap();
        settings
            .upsert("default", "default", &update)
            .await
            .unwrap();
        release.notify_one();
    };
    let (result, ()) = tokio::join!(submit(&runtime, authored), edit);
    result.unwrap();
    assert_scope(&runtime, &mut review, component, true).await;
    let row = rig.pool.get().await.unwrap().query_one("SELECT w.private_request_bytes,w.private_request_checksum,p.definition_bytes,p.checksum FROM brassclaw_turns w JOIN brassclaw_monty_private_preparations p USING(work_id) WHERE w.work_id=$1 AND w.record_kind='private_work'", &[&id]).await.unwrap();
    let request: PrivateWorkRequest = serde_json::from_str(row.get(0)).unwrap();
    let definition = crate::private_work_definition::PrivateWorkDefinition::restore(
        &request,
        row.get::<_, Vec<u8>>(1).try_into().unwrap(),
        row.get(2),
        row.get::<_, Vec<u8>>(3).try_into().unwrap(),
    )
    .unwrap();
    assert_eq!(definition.preparation().bytes(), original.bytes());
    assert_ne!(
        runtime
            .installed_catalogue
            .capture_task_preparation()
            .await
            .unwrap()
            .bytes(),
        original.bytes()
    );
    runtime
        .installed_catalogue
        .read_source_submission(id)
        .await
        .unwrap();
    let checked = read_positive(&runtime, id).await.unwrap();
    let crate::private_work_definition::DomainCapture::ValidatorStructural {
        capture_bytes, ..
    } = definition.domain()
    else {
        panic!("actual source input capture required");
    };
    let capture: Value = serde_json::from_str(capture_bytes).unwrap();
    let input_bytes = capture["input_bytes"].as_str().unwrap();
    let input: Value = serde_json::from_str(input_bytes).unwrap();
    let evidence_bytes: String = rig.pool.get().await.unwrap().query_one("SELECT e.evidence_bytes FROM reborn_validator_q1_runs r JOIN reborn_component_review_evidence e USING(evidence_id) WHERE r.submission_id=$1", &[&id]).await.unwrap().get(0);
    let evidence: Value = serde_json::from_str(&evidence_bytes).unwrap();
    let source_bytes = checked.subject().value()["candidate"]["revision"]["document"]["content"]
        .as_str()
        .unwrap()
        .len();
    // Derive a smaller valid byte limit from the actual persisted values. Keep
    // source/frame/transport, node and depth limits unchanged; no product quota
    // or unsupported depth profile is introduced by this finite test fixture.
    let before = settings.get("default", "default").await.unwrap();
    let mut reduced = before.execution_limits;
    let old_bounds = definition.preparation().limits().bounds;
    let original_limits = brassclaw_skills::value_contract::ContractLimits {
        max_depth: old_bounds.max_value_depth,
        max_nodes: old_bounds.max_value_nodes,
        max_bytes: old_bounds.max_value_bytes,
    };
    // Measure the actual logical-byte requirement through the supported
    // whole-value contract, rather than treating escaped JSON length as that
    // contract's accounting. Depth and node budgets are fixed and already pass.
    let byte_requirement = |value: &Value| {
        brassclaw_skills::value_contract::validate_data_bounds(value, original_limits).unwrap();
        let (mut low, mut high) = (1, original_limits.max_bytes);
        while low < high {
            let middle = low + (high - low) / 2;
            let mut limits = original_limits;
            limits.max_bytes = middle;
            match brassclaw_skills::value_contract::validate_data_bounds(value, limits) {
                Ok(()) => high = middle,
                Err(error) => {
                    assert_eq!(error.reason, "technical value bound exceeded");
                    low = middle + 1;
                }
            }
        }
        low
    };
    reduced.max_value_bytes = u64::try_from(
        byte_requirement(&input)
            .min(byte_requirement(&evidence))
            .min(source_bytes)
            / 2,
    )
    .unwrap();
    assert!(
        reduced.max_value_bytes > 0
            && reduced.max_value_bytes < before.execution_limits.max_value_bytes
    );
    reduced.validate().unwrap();
    let changed = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                json!({"expected_revision":before.revision,"execution_limits":reduced}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_millis(reduced.settings_uptake_timeout_millis),
        async {
            loop {
                let state = settings
                    .runtime_status()
                    .await
                    .unwrap()
                    .unwrap()
                    .execution_limits
                    .unwrap();
                if state.effective_revision == changed.revision
                    && state.uptake == brassclaw_product_workflow::MontyBudgetUptake::Applied
                {
                    assert_eq!(state.desired_revision, changed.revision);
                    assert_eq!(state.limits.max_value_bytes, reduced.max_value_bytes);
                    break;
                }
                tokio::task::yield_now().await;
            }
        },
    )
    .await
    .expect("actual Rust/Monty acknowledgement must apply the smaller supported value limit");
    assert_eq!(
        runtime
            .global_monty_owner
            .client()
            .live_task_settings()
            .current()
            .revision,
        changed.revision
    );
    let current = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert_eq!(current.revision(), changed.revision);
    let bounds = current.limits().bounds;
    let current_limits = brassclaw_skills::value_contract::ContractLimits {
        max_depth: bounds.max_value_depth,
        max_nodes: bounds.max_value_nodes,
        max_bytes: bounds.max_value_bytes,
    };
    for value in [&input, &evidence] {
        assert_eq!(
            brassclaw_skills::value_contract::validate_data_bounds(value, current_limits)
                .unwrap_err()
                .reason,
            "technical value bound exceeded"
        );
    }
    let program = brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage::restore_preparation(checked.selection()).unwrap();
    let coverage =
        brassclaw_engine::memory::retained_validator::RetainedValidatorCoverage::from_inputs(
            program.inputs(),
        )
        .unwrap();
    assert!(
        matches!(
            coverage.subject_task_inputs(checked.subject(), program.inputs(), current_limits),
            Err("validator subject exceeds owner policy")
        ),
        "a new typed input consumer must obey the actual smaller current limit"
    );
    runtime
        .installed_catalogue
        .read_source_submission(id)
        .await
        .unwrap();
    assert!(
        read_positive(&runtime, id).await.is_some(),
        "historical source evidence uses its original acknowledged data profile"
    );
    let stored: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT definition_bytes FROM brassclaw_monty_private_preparations WHERE work_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, definition.bytes());
    let stored_evidence: String = rig.pool.get().await.unwrap().query_one("SELECT e.evidence_bytes FROM reborn_validator_q1_runs r JOIN reborn_component_review_evidence e USING(evidence_id) WHERE r.submission_id=$1", &[&id]).await.unwrap().get(0);
    assert_eq!(stored_evidence, evidence_bytes);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_qualified_pure_source_upgrade_preserves_v1_and_rejects_unqualified_v2() {
    let rig = crate::runtime::test_pg::pg_rig_at_version(209).await;
    let _db = rig.lock_db().await;
    let (legacy, old) = legacy_fixture::retain_before_upgrade(&rig.pool).await;
    let home = tempfile::tempdir().unwrap();
    let runtime = start(&rig, home.path()).await;
    let (id, component) = (Uuid::new_v4(), Uuid::new_v4());
    let mut review = OriginalReview::new(&runtime, id);
    submit(&runtime, request(id, component, false))
        .await
        .unwrap();
    assert_scope(&runtime, &mut review, component, true).await;
    let outer: Value = serde_json::from_str(&selection_bytes(&runtime, id).await).unwrap();
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let saved = selection::read_preparation_in_transaction(&tx, legacy, REVISION_LIMITS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.bytes(), old);
    assert!(saved.source_qualification().is_none());
    tx.commit().await.unwrap();
    drop(client);
    // Copying well-formed pins is no authority: a different original W proof
    // cannot pass the opaque owner's full readback under the same snapshot.
    for field in [
        "workflow_proof",
        "worker_checksum",
        "package_checksum",
        "required_checks_checksum",
    ] {
        let forged_id = Uuid::new_v4();
        let retained = PgReviewSubmissionStore::new(rig.pool.clone())
            .submit(
                &ReviewSubmissionDraft::new(
                    forged_id,
                    "i01-forged",
                    ComponentRevisionDraft::from_json(&candidate(Uuid::new_v4(), false)).unwrap(),
                    None,
                    vec![],
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let mut forged = outer.clone();
        forged["submission_id"] = json!(forged_id);
        forged["subject_checksum"] = json!(retained.checksum);
        forged["candidate"] = json!({"uuid":retained.candidate.uuid,"class_code":22,"version":retained.candidate.version,"checksum":hex::encode(retained.candidate.checksum)});
        if field == "workflow_proof" {
            forged["qualification"][field]["checksum"] = json!("0".repeat(64));
        } else {
            forged["qualification"][field] = json!("0".repeat(64));
        }
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        selection::retain_in_transaction(&tx, &forged.to_string(), REVISION_LIMITS)
            .await
            .unwrap();
        let saved = selection::read_preparation_in_transaction(&tx, forged_id, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(runtime.installed_catalogue.require_source_selection(&tx,&saved).await,
            Err(crate::RebornBuildError::InvalidConfig { reason }) if reason=="original qualified Source selection differs")
        );
        tx.rollback().await.unwrap();
    }
    let strict_id = Uuid::new_v4();
    let strict_subject = PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                strict_id,
                "i01-strict",
                ComponentRevisionDraft::from_json(&candidate(Uuid::new_v4(), false)).unwrap(),
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut wire = outer;
    wire["submission_id"] = json!(strict_id);
    wire["subject_checksum"] = json!(strict_subject.checksum);
    wire["candidate"] = json!({"uuid":strict_subject.candidate.uuid,"class_code":22,"version":strict_subject.candidate.version,"checksum":hex::encode(strict_subject.candidate.checksum)});
    let mut malformed = Vec::new();
    for edit in 0..8 {
        let mut value = wire.clone();
        match edit {
            0 => {
                value.as_object_mut().unwrap().remove("qualification");
            }
            1 => value["qualification"] = Value::Null,
            2 => value["qualification"]["unexpected"] = json!(true),
            3 => value["qualification"]["prerequisite_proofs"] = json!([]),
            4 => {
                value["qualification"]["prerequisite_proofs"][0] =
                    value["qualification"]["workflow_proof"].clone()
            }
            5 => value["qualification"]["workflow_proof"]["id"] = json!(Uuid::nil()),
            6 => {
                value["qualification"]["workflow_proof"]["id"] =
                    json!("ABCDEFAB-CDEF-4ABC-8DEF-ABCDEFABCDEF")
            }
            7 => {
                value["format"] = json!("component-validator-selection/1");
                value["scope"] = json!("preparation-only");
            }
            _ => unreachable!(),
        }
        malformed.push(value.to_string());
    }
    let original_wire = wire.to_string();
    malformed.push(format!(
        "{{\"format\":\"component-validator-selection/2\",{}",
        &original_wire[1..]
    ));
    // /2 is exclusively the outer preparation envelope, never an inner
    // retained-recipe version. Recompute its digest to exercise that boundary.
    let mut inner: Value =
        serde_json::from_str(wire["validator_selection_bytes"].as_str().unwrap()).unwrap();
    inner["format"] = json!("monty-retained-recipe-selection/2");
    let inner = inner.to_string();
    wire["validator_selection_bytes"] = json!(inner);
    wire["validator_selection_checksum"] = json!(crate::bootstrap_evidence::checksum(
        wire["validator_selection_bytes"].as_str().unwrap()
    ));
    malformed.push(wire.to_string());
    for bytes in malformed {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        assert!(matches!(
            selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS).await,
            Err(brassclaw_skills::review_submission_store::SubmissionStoreError::Invalid)
        ));
        tx.rollback().await.unwrap();
    }
    let absent: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM reborn_component_validator_selections WHERE submission_id=$1",
            &[&strict_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        absent, 0,
        "strict decoding cannot leave a preparation behind"
    );
    runtime.shutdown().await.unwrap();
}

#[path = "qualified_source_faults.rs"]
mod faults;
#[path = "qualified_source_legacy_fixture.rs"]
mod legacy_fixture;
