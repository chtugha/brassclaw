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
    q1_orchestrator::{Q1Outcome, run_q1_validation_retained},
    validation_queue::ValidationQueueStore,
    validator_task_driver::ValidatorTaskDriver,
};
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft,
    review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft},
};
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

    // Create a minimal class-22 component and submit it for review.
    let component_id = Uuid::new_v4();
    let component_name = format!("test-component-{}", component_id.as_simple());
    let doc = serde_json::json!({
        "format": "component-revision/1",
        "uuid": component_id,
        "class_code": 22,
        "document": {
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
    PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                submission_id,
                "validator-structural-review",
                ComponentRevisionDraft::from_json(&doc).expect("minimal class-22 doc must parse"),
                None,
                vec![],
            )
            .expect("ReviewSubmissionDraft::new must succeed"),
        )
        .await
        .expect("PgReviewSubmissionStore::submit must succeed");

    // Submit to the validation queue (required for capture_q1_candidate).
    store
        .submit(&scope, component_id, 22, None)
        .await
        .expect("ValidationQueueStore::submit must succeed");

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
