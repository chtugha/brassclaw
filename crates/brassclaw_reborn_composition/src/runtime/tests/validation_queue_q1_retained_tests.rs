//! Integration tests for `run_q1_validation_retained` deferred paths (PKG-D B/C/D).
//!
//! These tests verify that `run_q1_validation_retained` correctly defers or errors
//! at each expected boundary without fabricating outcomes. They live here (instead
//! of `validation_queue.rs`) to access private `RebornRuntime` fields.
use super::*;
use crate::{
    installed_monty_catalogue::ValidatorServiceBridge,
    q1_orchestrator::{Q1Outcome, run_q1_validation_retained},
    validation_queue::ValidationQueueStore,
    validator_task_driver::ValidatorTaskDriver,
};
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use uuid::Uuid;

fn test_scope() -> ComponentScope {
    ComponentScope {
        tenant_id: "t".into(),
        user_id: "u".into(),
        agent_id: "a".into(),
        project_id: "p".into(),
    }
}

/// PKG-D test case B: `run_q1_validation_retained` returns `Deferred` when the
/// structural validator revision draft is not seeded for the class code.
///
/// The bridge is provided so step 1 passes; no drafts are seeded
/// so `read_allocation_head` returns `None` and step 2 defers.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_q1_validation_retained_defers_when_no_revision_draft_seeded() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("q1-retained-no-draft-owner", directory.path())
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
    let scope = test_scope();
    let store = ValidationQueueStore::new(rig.pool.clone());
    let cid = Uuid::new_v4();
    store.submit(&scope, cid, 22, None).await.unwrap();

    // No validator drafts seeded → read_allocation_head returns None for class 22.
    let outcome = run_q1_validation_retained(
        &rig.pool,
        &scope,
        cid,
        22,
        &store,
        brassclaw_monty_host::utility::UtilityLimits::from_execution(Default::default()).unwrap(),
        Some(&bridge),
    )
    .await
    .expect("run_q1_validation_retained must not error — defer expected");

    assert!(
        matches!(outcome, Q1Outcome::Deferred { .. }),
        "expected Deferred when no revision draft seeded for class 22; got {outcome:?}"
    );
    let Q1Outcome::Deferred { reason } = &outcome else {
        panic!("expected Deferred")
    };
    assert!(
        reason.contains("no structural validator revision draft for class"),
        "deferred reason must mention the missing draft; got: {reason}"
    );
}

/// PKG-D test case C: `run_q1_validation_retained` returns `Deferred` when no
/// active submission exists for the component in `reborn_component_review_submissions`.
///
/// The bridge is provided and drafts are seeded so steps 1–4 pass.
/// No submission is created → step 5 defers.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_q1_validation_retained_defers_when_no_submission() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("q1-retained-no-submission-owner", directory.path())
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

    // Seed validator drafts for class 22 so step 2 passes.
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();

    let scope = test_scope();
    let store = ValidationQueueStore::new(rig.pool.clone());
    // Create a test note component and submit it to the queue.
    let cid = Uuid::new_v4();
    store.submit(&scope, cid, 22, None).await.unwrap();
    // Do NOT create a review submission — so step 5 defers.

    // class_code=22 has drafts seeded — no submission for cid → defers at step 5.
    let outcome = run_q1_validation_retained(
        &rig.pool,
        &scope,
        cid,
        22,
        &store,
        brassclaw_monty_host::utility::UtilityLimits::from_execution(Default::default()).unwrap(),
        Some(&bridge),
    )
    .await
    .expect("run_q1_validation_retained must not error — defer expected");

    assert!(
        matches!(outcome, Q1Outcome::Deferred { .. }),
        "expected Deferred when no submission exists for component; got {outcome:?}"
    );
    let Q1Outcome::Deferred { reason } = &outcome else {
        panic!("expected Deferred")
    };
    assert!(
        reason.contains("no active submission for component"),
        "deferred reason must mention missing submission; got: {reason}"
    );
}

/// PKG-D test case D: `run_q1_validation_retained` returns `Err(Q1Error::Queue(NotFound))`
/// — NOT `Deferred` — when a submission exists but there is no queue row.
///
/// This confirms the architectural boundary: a component with a submission but
/// no queue row is a caller error, not an expected infrastructure absence.
/// The function must NOT convert `NotFound` to `Deferred`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn run_q1_validation_retained_errors_not_defers_when_no_queue_row() {
    use crate::q1_orchestrator::Q1Error;
    use crate::validation_queue::ValidationQueueError;
    use brassclaw_skills::{
        component_revision::ComponentRevisionDraft,
        review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft},
    };
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("q1-retained-no-queue-row-owner", directory.path())
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

    // Seed validator drafts so step 2 passes.
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();

    let scope = test_scope();
    let store = ValidationQueueStore::new(rig.pool.clone());

    // Create a minimal class-22 component for the review.
    let candidate_id = Uuid::new_v4();
    let candidate_doc = serde_json::json!({
        "format": "component-revision/1",
        "uuid": candidate_id,
        "class_code": 22,
        "document": {
            "content": "def test_fn(inputs):\n    return inputs['v']\n",
            "input_contract": {"v": {"type":"string","required":true,"checks":[]}},
            "result_contract": {"type":"string"},
            "preload": {
                "format": "python-preload/2",
                "exports": {
                    "test_fn": {
                        "symbol": "test_fn",
                        "parameters": ["inputs"],
                        "mapping": true
                    }
                },
                "private_functions": {},
                "constants": [],
                "imports": [],
                "dependencies": [],
                "default_export": "test_fn"
            }
        },
        "dependencies": [],
        "association": null
    })
    .to_string();

    // Submit a review via PgReviewSubmissionStore (no queue row created).
    let submission_id = Uuid::new_v4();
    PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                submission_id,
                "validator-structural-review",
                ComponentRevisionDraft::from_json(&candidate_doc).unwrap(),
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();

    // Do NOT submit to the validation queue.
    // The function must fail at capture_q1_candidate with NotFound — not Deferred.
    let result = run_q1_validation_retained(
        &rig.pool,
        &scope,
        candidate_id,
        22,
        &store,
        brassclaw_monty_host::utility::UtilityLimits::from_execution(Default::default()).unwrap(),
        Some(&bridge),
    )
    .await;

    match result {
        Err(Q1Error::Queue(ValidationQueueError::NotFound { component_id })) => {
            assert_eq!(
                component_id, candidate_id,
                "NotFound must reference the correct component_id"
            );
        }
        Ok(Q1Outcome::Deferred { reason }) => {
            panic!("NotFound must NOT be converted to Deferred; got Deferred: {reason}");
        }
        other => {
            panic!("expected Err(Q1Error::Queue(NotFound)); got: {other:?}");
        }
    }
}
