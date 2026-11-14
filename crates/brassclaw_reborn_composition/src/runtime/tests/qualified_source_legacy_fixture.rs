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
use brassclaw_skills::component_revision::{RetainedComponentRevision, RetainedComponentSnapshot};
use sha2::{Digest, Sha256};
use tokio_postgres::Transaction;

// Fixed V101/V104 predecessor wire, also used by the V212 upgrade fixture in
// snapshot_policy_tests. No preparation carrier existed on V209 revision rows.
#[derive(serde::Serialize)]
struct HistoricalSubject<'a> {
    format: &'a str,
    submission_id: Uuid,
    actor: &'a str,
    candidate: brassclaw_product_workflow::ComponentRevisionSelection,
    candidate_bytes: &'a str,
    base: Option<Value>,
    dependencies: Vec<Value>,
}

// Keep a differing native failure actionable without logging SQL messages,
// detail/hint or a failing row, which can contain original author data.
fn sql_failure<T>(error: tokio_postgres::Error) -> T {
    if let Some(database) = error.as_db_error() {
        panic!(
            "historical V209 SQL failed: state={}, table={:?}, column={:?}, constraint={:?}",
            database.code().code(),
            database.table(),
            database.column(),
            database.constraint(),
        );
    }
    panic!("historical V209 database transport failed");
}

/// Test-only first-version writer/readback for the actual V209 DDL. Its SQL
/// errors retain native SQLSTATE/table/column diagnostics without author data.
async fn retain_v209_revision(
    tx: &Transaction<'_>,
    draft: &ComponentRevisionDraft,
) -> RetainedComponentRevision {
    assert!(draft.preparation().is_none());
    let id = draft.uuid();
    let class = i16::try_from(draft.class_code()).unwrap();
    let checksum = hex::encode(draft.checksum());
    tx.execute("INSERT INTO reborn_component_revision_heads(component_id,class_code,last_version) VALUES($1,$2,1)", &[&id,&class]).await.unwrap_or_else(sql_failure);
    tx.execute("INSERT INTO reborn_component_revisions(component_id,class_code,version,revision_bytes,checksum) VALUES($1,$2,1,$3,$4)", &[&id,&class,&draft.exact_bytes(),&checksum]).await.unwrap_or_else(sql_failure);
    let row = tx.query_one("SELECT r.class_code,r.version,r.revision_bytes,r.checksum,h.last_version FROM reborn_component_revisions r JOIN reborn_component_revision_heads h USING(component_id,class_code) WHERE r.component_id=$1 AND r.version=1", &[&id]).await.unwrap_or_else(sql_failure);
    assert_eq!(row.get::<_, i16>(0), class);
    assert_eq!(row.get::<_, i64>(1), 1);
    assert_eq!(row.get::<_, i64>(4), 1);
    let bytes: String = row.get(2);
    assert_eq!(bytes, draft.exact_bytes());
    assert_eq!(
        row.get::<_, String>(3),
        hex::encode(Sha256::digest(bytes.as_bytes()))
    );
    let retained = ComponentRevisionDraft::from_json(&bytes)
        .unwrap()
        .at_version(1)
        .unwrap();
    assert_eq!(
        retained.reference(),
        draft.clone().at_version(1).unwrap().reference()
    );
    retained
}

pub(super) async fn retain_before_upgrade(
    pool: &Arc<brassclaw_pg::PgPool>,
) -> (Uuid, String, InspectedRetainedProgram) {
    // Reuse all five actual supported predecessor package drafts, including
    // the distinct string receipt encoder after aggregation. No hand-authored
    // shortcut declaration or parser relaxation stands in for coverage.
    let drafts = crate::builtin_bootstrap::validator_recipe_drafts_for_class(22).unwrap();
    let root = drafts.iter().find(|d| d.class_code() == 21).unwrap().uuid();
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap_or_else(sql_failure);
    assert_eq!(
        tx.query_one("SELECT max(version) FROM refinery_schema_history", &[])
            .await
            .unwrap_or_else(sql_failure)
            .get::<_, i32>(0),
        209
    );
    let mut revisions = Vec::new();
    for draft in &drafts {
        revisions.push(retain_v209_revision(&tx, draft).await);
    }
    // All exact rows come from this same repeatable-read transaction. The
    // normal snapshot owner rejects cycles, missing and unrelated components.
    let snapshot = Arc::new(RetainedComponentSnapshot::new(&[root], revisions).unwrap());
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
    let coverage = RetainedValidatorCoverage::from_inputs(inspected.program().inputs()).unwrap();
    let id = Uuid::new_v4();
    let candidate = ComponentRevisionDraft::from_json(&candidate(Uuid::new_v4(), false)).unwrap();
    let candidate = retain_v209_revision(&tx, &candidate).await;
    assert_eq!(coverage.class_code(), candidate.reference().class_code);
    let subject_bytes = serde_json::to_string(&HistoricalSubject {
        format: "component-review-submission/1",
        submission_id: id,
        actor: "i01-legacy",
        candidate: crate::pg_review_submission::selection(candidate.reference()),
        candidate_bytes: candidate.draft().exact_bytes(),
        base: None,
        dependencies: vec![],
    })
    .unwrap();
    let subject_checksum = hex::encode(Sha256::digest(subject_bytes.as_bytes()));
    // V149's real trigger creates and checks the immutable review request.
    tx.execute("INSERT INTO reborn_component_review_submissions(submission_id,actor,component_id,candidate_version,base_version,subject_bytes,checksum) VALUES($1,'i01-legacy',$2,1,NULL,$3,$4)", &[&id,&candidate.reference().uuid,&subject_bytes,&subject_checksum]).await.unwrap_or_else(sql_failure);
    let row = tx.query_one("SELECT s.subject_bytes,s.checksum,r.subject_checksum FROM reborn_component_review_submissions s JOIN reborn_component_review_requests r USING(submission_id) WHERE submission_id=$1", &[&id]).await.unwrap_or_else(sql_failure);
    assert_eq!(row.get::<_, String>(0), subject_bytes);
    assert_eq!(row.get::<_, String>(1), subject_checksum);
    assert_eq!(row.get::<_, String>(2), subject_checksum);
    let selection = inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection_with_limits(REVISION_LIMITS)
        .unwrap();
    // Historical preparation-only /1, never qualification or approval. After
    // upgrade the sealed subject/selection readers and coverage writer must
    // reproduce these exact bytes through their current real implementations.
    let bytes = json!({"format":"component-validator-selection/1",
        "submission_id":id,"subject_checksum":subject_checksum,
        "candidate":crate::pg_review_submission::selection(candidate.reference()),
        "validator_selection_bytes":selection.exact_bytes(),
        "validator_selection_checksum":hex::encode(selection.checksum()),
        "scope":"preparation-only"})
    .to_string();
    brassclaw_skills::value_contract::strict_json(&bytes, REVISION_LIMITS).unwrap();
    tx.execute("INSERT INTO reborn_component_validator_selections(submission_id,selection_bytes,checksum) VALUES($1,$2,$3)", &[&id,&bytes,&hex::encode(Sha256::digest(bytes.as_bytes()))]).await.unwrap_or_else(sql_failure);
    let row = tx.query_one("SELECT selection_bytes,checksum FROM reborn_component_validator_selections WHERE submission_id=$1", &[&id]).await.unwrap_or_else(sql_failure);
    assert_eq!(row.get::<_, String>(0), bytes);
    assert_eq!(
        row.get::<_, String>(1),
        hex::encode(Sha256::digest(bytes.as_bytes()))
    );
    tx.commit().await.unwrap_or_else(sql_failure);
    (id, bytes, inspected)
}
