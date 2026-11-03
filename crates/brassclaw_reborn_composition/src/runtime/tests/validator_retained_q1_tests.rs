//! Integration test: end-to-end `run_q1_validation_retained` pass (PKG-C Step 6).
//!
//! Verifies the complete retained Q1 path:
//! 1. Structural validator Recipe drafts are seeded.
//! 2. A class-22 component is submitted to review via `PgReviewSubmissionStore`.
//! 3. `retain_validator_selection` seals the selection (done inside `run_q1_validation_retained`).
//! 4. `ValidationQueueStore::submit` enqueues the component.
//! 5. `run_q1_validation_retained` is called with the bridge wired to the live test driver.
//! 6. `Q1Outcome::Passed` is returned.
//! 7. DB evidence rows in `reborn_component_review_evidence` and `reborn_validator_q1_runs`
//!    confirm `verify_receipt` ran and persisted real Q1 evidence.
//!
//! This test exercises the full live path:
//! `run_q1_validation_retained` → `ValidatorTaskDriver::submit` →
//! `bootstrap_validation::admit_validator_handoff` → `GlobalMontyDriver::drive_admitted` →
//! `GlobalTaskPortsFactory::build` → `capture_internal` → `ReviewAdmissionSource` →
//! `verify_receipt` → `persist_validator_q1_evidence`.
use super::*;
use crate::{
    installed_monty_catalogue::ValidatorServiceBridge,
    pg_python_code_store::{NewPgPythonCode, PgPythonCodeStore},
    q1_orchestrator::{Q1Outcome, run_q1_validation_retained},
    validation_queue::ValidationQueueStore,
    validator_task_driver::ValidatorTaskDriver,
};
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, review_submission_store::ReviewSubmissionDraft,
};
use sha2::Digest;
use uuid::Uuid;

fn q1_retained_scope() -> ComponentScope {
    ComponentScope {
        tenant_id: "q1-retained-t".into(),
        user_id: "q1-retained-u".into(),
        agent_id: "q1-retained-a".into(),
        project_id: "q1-retained-p".into(),
    }
}

/// Full end-to-end Q1 retained pass: seeds drafts, creates a class-22 component,
/// submits to review, submits to queue, calls `run_q1_validation_retained` and
/// asserts `Q1Outcome::Passed` plus real evidence rows in the DB.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_q1_validation_retained_passes_and_persists_evidence() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("q1-retained-pass-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();

    // Build bridge from the real test-path driver.
    let bridge = ValidatorServiceBridge {
        driver: std::sync::Arc::new(ValidatorTaskDriver::new(
            runtime.monty_test_controls.0.clone(),
        )),
        pool: rig.pool.clone(),
    };

    // Seed validator drafts for all classes including class 22.
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .expect("retain_validator_check_drafts must succeed");

    let scope = q1_retained_scope();
    let store = ValidationQueueStore::new(rig.pool.clone());

    // Insert the component row into reborn_python_code first — capture_q1_candidate
    // requires the row to exist in the component table (component_bytes query).
    let component_name = format!("test-component-{}", Uuid::new_v4().as_simple());
    let pc_store = PgPythonCodeStore::new(rig.pool.clone());
    let component_id = pc_store
        .insert(NewPgPythonCode {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            name: component_name.clone(),
            description: "Minimal test PythonCode component for Q1 retained integration test."
                .into(),
            content: "def retain_value(inputs):\n    return inputs['value']\n".into(),
            prior_knowledge_content: None,
            override_prompt_creation: false,
            consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
            intent_examples: None,
            source: "authored".into(),
            dependency_registry: None,
            includes: vec![],
            content_checksum: None,
        })
        .await
        .expect("PgPythonCodeStore::insert must succeed");

    // Build the ComponentRevisionDraft referencing the assigned component_id.
    let doc = serde_json::json!({
        "format": "component-revision/1",
        "uuid": component_id,
        "class_code": 22,
        "document": {
            "consumer_tags":["02:orchestrator","05:validator"],"prior_knowledge_content":null,
            "override_prompt_creation":false,"intent_examples":null,"dependency_registry":null,"includes":[],
            "name": component_name,
            "description": "Minimal test PythonCode component for Q1 retained integration test.",
            "content": "def retain_value(inputs):\n    return inputs['value']\n",
            "input_contract": {
                "value": {"type": "string", "required": true, "checks": []}
            },
            "result_contract": {"type": "string"},
            "preload": {
                "format": "python-preload/2",
                "exports": {
                    "retain_value": {
                        "symbol": "retain_value",
                        "parameters": ["inputs"],
                        "mapping": true
                    }
                },
                "private_functions": {},
                "constants": [],
                "imports": [],
                "dependencies": [],
                "default_export": "retain_value"
            }
        },
        "dependencies": [],
        "association": null
    })
    .to_string();

    let submission_id = Uuid::new_v4();
    let original_draft = ReviewSubmissionDraft::new(
        submission_id,
        "validator-structural-review",
        ComponentRevisionDraft::from_json(&doc).expect("minimal class-22 doc must parse"),
        None,
        vec![],
    )
    .expect("ReviewSubmissionDraft::new must succeed");

    // Submit to the validation queue (required for capture_q1_candidate).
    // Fire-and-forget kick from submit() is a no-op here (bridge not wired on the store).
    store
        .submit(&scope, component_id, 22, None)
        .await
        .expect("ValidationQueueStore::submit must succeed");
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let queue_id: Uuid = tx
        .query_one(
            "SELECT id FROM reborn_validation_queue WHERE component_id=$1",
            &[&component_id],
        )
        .await
        .unwrap()
        .get(0);
    ValidationQueueStore::submit_original_in_transaction(
        &tx,
        &original_draft,
        &[(scope.clone(), component_id, queue_id)],
        &[queue_id],
        "existing_queue",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(client);

    // An unrelated validator result for the same submission must not decide
    // this execution. Deliberately future-date this negative historical fixture
    // so a latest-row query would select it after the real run finishes. It is
    // not a trusted positive observation or an approval fixture.
    let decoy = Uuid::new_v4();
    let decoy_bytes = serde_json::json!({
        "format":"component-review-evidence/1", "evidence_id":decoy,
        "kind":"q1", "succeeded":false,
        "report":{"fixture":"unrelated negative validator result"}
    })
    .to_string();
    let decoy_checksum = hex::encode(sha2::Sha256::digest(decoy_bytes.as_bytes()));
    let decoy_selection = hex::encode(sha2::Sha256::digest(
        b"unrelated validator selection fixture",
    ));
    {
        let mut client = rig.pool.get().await.unwrap();
        let tx = client.transaction().await.unwrap();
        tx.execute(
            "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
             VALUES($1,$2,$3)",
            &[&decoy, &decoy_bytes, &decoy_checksum],
        )
        .await
        .unwrap();
        tx.execute(
            "INSERT INTO reborn_validator_q1_runs
             (run_id,submission_id,validator_selection_checksum,candidate_class_code,
              task_handle,evidence_id,succeeded,recorded_at)
             VALUES($1,$2,$3,22,$4,$1,false,clock_timestamp()+interval '1 day')",
            &[&decoy, &submission_id, &decoy_selection, &Uuid::new_v4()],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    // Run the full retained Q1 path.
    let outcome = run_q1_validation_retained(
        &rig.pool,
        &scope,
        component_id,
        22,
        &store,
        brassclaw_monty_host::utility::UtilityLimits::from_execution(Default::default()).unwrap(),
        Some(&bridge),
    )
    .await
    .expect("run_q1_validation_retained must not return Err");

    assert!(
        matches!(outcome, Q1Outcome::Passed),
        "expected Q1Outcome::Passed for a valid class-22 component; got {outcome:?}"
    );

    // Assert Q1 evidence rows were written by verify_receipt.
    let client = rig.pool.get().await.unwrap();
    let ev_count: i64 = client
        .query_one(
            "SELECT count(*) FROM reborn_component_review_evidence e
             JOIN reborn_validator_q1_runs r ON r.evidence_id = e.evidence_id
             WHERE r.submission_id = $1 AND r.succeeded = true",
            &[&submission_id],
        )
        .await
        .expect("evidence query must succeed")
        .get(0);
    assert!(
        ev_count >= 1,
        "expected at least one Q1 evidence+run row for submission {submission_id} with succeeded=true; got {ev_count}"
    );

    // Queue row must be at state 2 (gate1-passed) after Q1Outcome::Passed.
    let rows = store
        .list(&scope, Some(2))
        .await
        .expect("list must succeed");
    let row = rows
        .iter()
        .find(|r| r.component_id == component_id)
        .expect("queue row must exist at state 2 after Q1Outcome::Passed");
    assert_eq!(row.state, 2, "queue row must be at state 2 (gate1-passed)");
}

/// T5: end-to-end fail — component with empty `content` produces `Q1Outcome::Failed`.
///
/// Seeds validator drafts, creates a class-22 component with an empty `content`
/// field (which the structural validator rejects), submits it for review and to the
/// queue, calls `run_q1_validation_retained`, and asserts `Q1Outcome::Failed` with
/// a non-empty error list. The queue row must stay at state 1 (Q1 failed records
/// the failure but does not promote to state 2).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_q1_validation_retained_fails_for_empty_content() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("q1-retained-fail-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();

    let bridge = ValidatorServiceBridge {
        driver: std::sync::Arc::new(ValidatorTaskDriver::new(
            runtime.monty_test_controls.0.clone(),
        )),
        pool: rig.pool.clone(),
    };

    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .expect("retain_validator_check_drafts must succeed");

    let scope = ComponentScope {
        tenant_id: "q1-retained-fail-t".into(),
        user_id: "q1-retained-fail-u".into(),
        agent_id: "q1-retained-fail-a".into(),
        project_id: "q1-retained-fail-p".into(),
    };
    let store = ValidationQueueStore::new(rig.pool.clone());

    // Insert the component row into reborn_python_code first — capture_q1_candidate
    // requires the row to exist in the component table (component_bytes query).
    // The content is deliberately empty so the structural validator rejects it.
    let fail_name = format!("fail-component-{}", Uuid::new_v4().as_simple());
    let pc_store = PgPythonCodeStore::new(rig.pool.clone());
    let component_id = pc_store
        .insert(NewPgPythonCode {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            name: fail_name.clone(),
            description: "A component whose content is deliberately empty.".into(),
            content: "".into(),
            prior_knowledge_content: None,
            override_prompt_creation: false,
            consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
            intent_examples: None,
            source: "authored".into(),
            dependency_registry: None,
            includes: vec![],
            content_checksum: None,
        })
        .await
        .expect("PgPythonCodeStore::insert must succeed");

    let doc = serde_json::json!({
        "format": "component-revision/1",
        "uuid": component_id,
        "class_code": 22,
        "document": {
            "consumer_tags":["02:orchestrator","05:validator"],"prior_knowledge_content":null,
            "override_prompt_creation":false,"intent_examples":null,"dependency_registry":null,"includes":[],
            "name": fail_name,
            "description": "A component whose content is deliberately empty.",
            "content": "",
            "input_contract": {},
            "result_contract": {"type": "null"},
            "preload": {
                "format": "python-preload/2",
                "exports": {},
                "private_functions": {},
                "constants": [],
                "imports": [],
                "dependencies": [],
                "default_export": null
            }
        },
        "dependencies": [],
        "association": null
    })
    .to_string();

    let submission_id = Uuid::new_v4();
    let original_draft = brassclaw_skills::review_submission_store::ReviewSubmissionDraft::new(
        submission_id,
        "validator-structural-review",
        brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(&doc)
            .expect("malformed doc must parse at the JSON/schema level"),
        None,
        vec![],
    )
    .expect("ReviewSubmissionDraft::new must succeed");

    store
        .submit(&scope, component_id, 22, None)
        .await
        .expect("ValidationQueueStore::submit must succeed");
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let queue_id: Uuid = tx
        .query_one(
            "SELECT id FROM reborn_validation_queue WHERE component_id=$1",
            &[&component_id],
        )
        .await
        .unwrap()
        .get(0);
    ValidationQueueStore::submit_original_in_transaction(
        &tx,
        &original_draft,
        &[(scope.clone(), component_id, queue_id)],
        &[queue_id],
        "existing_queue",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(client);

    let outcome = run_q1_validation_retained(
        &rig.pool,
        &scope,
        component_id,
        22,
        &store,
        brassclaw_monty_host::utility::UtilityLimits::from_execution(Default::default()).unwrap(),
        Some(&bridge),
    )
    .await
    .expect("run_q1_validation_retained must not return Err for a structural failure");

    match &outcome {
        Q1Outcome::Failed { errors } => {
            assert!(
                !errors.is_empty(),
                "expected non-empty error list for empty-content component; got none"
            );
            assert!(
                errors.iter().any(|e| e.ends_with(": content is empty")),
                "expected the actual rule diagnostic, not a generic failure; got: {errors:?}"
            );
        }
        other => panic!("expected Q1Outcome::Failed for empty-content component; got {other:?}"),
    }

    // Queue row must remain at state 1 (Q1_pending/failed) — NOT promoted to 2.
    let rows = store
        .list(&scope, Some(1))
        .await
        .expect("list must succeed");
    assert!(
        rows.iter().any(|r| r.component_id == component_id),
        "queue row must stay at state 1 after Q1Outcome::Failed"
    );
}
