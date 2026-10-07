//! Actual immutable-draft -> contained inspection -> structural Q1 persistence.
//! No semantic approval, behavioral evidence, human Q2 or activation is invented.

use std::sync::Arc;

use brassclaw_engine::executor::{
    retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram,
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;
#[path = "../../../crates/brassclaw_reborn_composition/src/pg_retained_q1.rs"]
mod pg_retained_q1;
#[path = "support/retained_program.rs"]
mod retained_program;

use pg_retained_q1::{StructuralReviewFailure, StructuralReviewSet, persist_structural_reviews};

fn worker() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_BIN_EXE_global_worker"))
}

#[tokio::test]
async fn actual_structural_reviews_retain_exact_source_and_are_not_combination_approval() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = retained_program::program(&store, false).await;
    let binding = &program.bindings()["0:2"];
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(RetainedProgram::Tools(program.clone()), worker())
            .await
            .unwrap(),
    );
    let reviews = Arc::new(StructuralReviewSet::prepare(inspected).unwrap());
    assert_eq!(reviews.references().len(), 1);
    assert_eq!(reviews.inspected().source_checks().len(), 1);
    let skill = binding.skill().uuid;
    let id = reviews.references()[&skill];
    let bytes = reviews.evidence(skill).unwrap();
    let evidence: Value = serde_json::from_str(bytes).unwrap();
    assert_eq!(evidence["kind"], "q1");
    assert_eq!(evidence["validation_mode"], "authored");
    assert_eq!(evidence["succeeded"], true);
    assert_eq!(evidence["report"]["scope"], "structural-only");
    assert_eq!(evidence["report"]["semantic_approval"], false);
    assert_eq!(
        evidence["association_checksum"],
        format!(
            "{:x}",
            Sha256::digest(binding.association().exact_bytes().as_bytes())
        )
    );
    assert_eq!(
        evidence["components"].as_array().unwrap().len(),
        binding.combination().len()
    );
    assert_eq!(evidence["reviewed_evidence"], json!([]));
    assert_eq!(
        evidence["report"]["observations"]["source_checksum"],
        reviews.inspected().source_checks()[&binding.python().uuid]
            .observations()
            .source_checksum
    );
    persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap();
    persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap();
    // A real replacement does not change the exact original review target.
    let selected = &program.inputs().instruction().snapshot().revisions()[&binding.python().uuid];
    let mut document = selected.draft().document().clone();
    document["content"] =
        json!("result = host.json(operation='parse', data=inputs['data'])\n# replacement revision");
    let replacement = ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1", "uuid":binding.python().uuid,
        "class_code":22, "document":document, "dependencies":[], "association":null})
        .to_string(),
    )
    .unwrap();
    let newer = store
        .stage(&replacement, binding.python().version)
        .await
        .unwrap();
    assert_ne!(newer, binding.python());
    persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let actual = client.query_one(
        "SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",
        &[&id],
    ).await.unwrap();
    assert_eq!(actual.get::<_, String>(0), bytes);
    assert_eq!(
        actual.get::<_, String>(1),
        format!("{:x}", Sha256::digest(bytes.as_bytes()))
    );
    let counts = client
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_component_review_evidence),
                (SELECT count(*) FROM reborn_skill_association_approvals),
                (SELECT count(*) FROM reborn_component_graduation_receipts)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(counts.get::<_, i64>(0), 1);
    assert_eq!(counts.get::<_, i64>(1), 0);
    assert_eq!(counts.get::<_, i64>(2), 0);

    let conflicting = Arc::new(
        StructuralReviewSet::prepare(Arc::new(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(program.clone()), worker())
                .await
                .unwrap(),
        ))
        .unwrap(),
    );
    let mut changed: Value = serde_json::from_str(conflicting.evidence(skill).unwrap()).unwrap();
    changed["succeeded"] = json!(false);
    let changed = changed.to_string();
    client.execute(
        "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES ($1,$2,$3)",
        &[&conflicting.references()[&skill], &changed, &format!("{:x}", Sha256::digest(changed.as_bytes()))],
    ).await.unwrap();
    let error = persist_structural_reviews(&rig.pool, conflicting.clone())
        .await
        .unwrap_err();
    assert!(matches!(error.failure, StructuralReviewFailure::Integrity));
    assert!(Arc::ptr_eq(&error.retained, &conflicting));
}

#[tokio::test]
async fn actual_commit_failure_retains_the_original_review_for_idempotent_recovery() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = retained_program::program(&store, false).await;
    let skill = program.bindings()["0:2"].skill().uuid;
    let reviews = Arc::new(
        StructuralReviewSet::prepare(Arc::new(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(program), worker())
                .await
                .unwrap(),
        ))
        .unwrap(),
    );
    let id = reviews.references()[&skill];
    let client = rig.pool.get().await.unwrap();
    client.batch_execute(
        "CREATE FUNCTION fail_actual_review_commit() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'actual validation persistence failure' USING ERRCODE='23514'; END; $$;
         CREATE CONSTRAINT TRIGGER actual_review_commit_fault AFTER INSERT
         ON reborn_component_review_evidence DEFERRABLE INITIALLY DEFERRED
         FOR EACH ROW EXECUTE FUNCTION fail_actual_review_commit()"
    ).await.unwrap();
    let error = persist_structural_reviews(&rig.pool, reviews.clone())
        .await
        .unwrap_err();
    let StructuralReviewFailure::Database(ref database) = error.failure else {
        panic!("actual PostgreSQL commit error required");
    };
    assert_eq!(
        database.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    assert!(Arc::ptr_eq(&error.retained, &reviews));
    assert_eq!(error.retained.references()[&skill], id);
    assert!(!format!("{error:?}").contains("actual validation persistence failure"));
    let actual: i64 = client
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        actual, 0,
        "actual failed commit must not publish partial evidence"
    );
    client
        .batch_execute(
            "DROP TRIGGER actual_review_commit_fault ON reborn_component_review_evidence",
        )
        .await
        .unwrap();
    persist_structural_reviews(&rig.pool, error.retained.clone())
        .await
        .unwrap();
    persist_structural_reviews(&rig.pool, error.retained.clone())
        .await
        .unwrap();
    let actual: String = client
        .query_one(
            "SELECT evidence_bytes FROM reborn_component_review_evidence WHERE evidence_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(actual, error.retained.evidence(skill).unwrap());
}
