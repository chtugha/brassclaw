//! Genuine /1 partial-presence preparation retained on V209 before upgrade.
//! This fixture executes only the native W inspector, never the subject.
use super::*;
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
        retained_validator::RetainedValidatorCoverage,
    },
};

pub(super) async fn retain_before_upgrade(pool: &Arc<brassclaw_pg::PgPool>) -> (Uuid, String) {
    // Reuse all five actual supported predecessor package drafts, including
    // the distinct string receipt encoder after aggregation. No hand-authored
    // shortcut declaration or parser relaxation stands in for coverage.
    let drafts = crate::builtin_bootstrap::validator_recipe_drafts_for_class(22).unwrap();
    let root = drafts.iter().find(|d| d.class_code() == 21).unwrap().uuid();
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut references = Vec::new();
    for draft in &drafts {
        references.push(store.retain_packaged_draft(draft).await.unwrap());
    }
    let snapshot = Arc::new(store.read_exact(&[root], &references).await.unwrap());
    let instruction = compile_retained_recipe(
        snapshot,
        root,
        "validator-class-22",
        WorkflowClass::RequiresModel,
    )
    .unwrap();
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    let inspected = InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(
            prepare_retained_tool_program(instruction).unwrap(),
        )),
        &worker,
    )
    .await
    .unwrap();
    let id = Uuid::new_v4();
    PgReviewSubmissionStore::new(pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                id,
                "i01-legacy",
                ComponentRevisionDraft::from_json(&candidate(Uuid::new_v4(), false)).unwrap(),
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let subject =
        PgReviewSubmissionStore::read_validator_subject_in_transaction(&tx, id, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
    let bytes =
        RetainedValidatorCoverage::selection_bytes(&subject, &inspected, REVISION_LIMITS).unwrap();
    selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (id, bytes)
}
