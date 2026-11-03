//! Real PostgreSQL/HTTP-boundary and exact evidence-shape rejection checks.
use super::*;
use brassclaw_skills::{
    pure_component_contract::PureComponentApprovalDeclaration, value_contract::ContractLimits,
};
use serde_json::json;

fn limits() -> ContractLimits {
    ContractLimits {
        max_depth: REVISION_LIMITS.max_depth,
        max_nodes: REVISION_LIMITS.max_nodes,
        max_bytes: REVISION_LIMITS.max_bytes,
    }
}

fn fake_checksum() -> String {
    "a".repeat(64)
}

fn declaration_json(
    approval_id: Uuid,
    root_id: Uuid,
    version: u64,
    q1: Uuid,
    behavioral: Uuid,
) -> String {
    json!({
        "format": "pure-component-approval/1",
        "approval_id": approval_id.to_string(),
        "root": {
            "uuid": root_id.to_string(),
            "class_code": 22,
            "version": version,
            "checksum": fake_checksum(),
        },
        "components": [{
            "uuid": root_id.to_string(),
            "class_code": 22,
            "version": version,
            "checksum": fake_checksum(),
        }],
        "validation_mode": "authored",
        "q1_ref": q1.to_string(),
        "q2_ref": Uuid::new_v4().to_string(),
        "behavioral_refs": [behavioral.to_string()],
    })
    .to_string()
}

#[tokio::test]
async fn missing_evidence_record_is_rejected() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let q1 = Uuid::new_v4();
    let behavioral = Uuid::new_v4();
    let root_id = Uuid::new_v4();
    let approval_id = Uuid::new_v4();
    let json = declaration_json(approval_id, root_id, 1, q1, behavioral);
    let decl = PureComponentApprovalDeclaration::from_json(&json, limits()).unwrap();
    // No evidence records exist — must fail.
    let result = record_pure_component_approval(&rig.pool, &decl).await;
    assert!(
        matches!(result, Err(PureApprovalError::Invalid(msg)) if msg.contains("evidence record is missing")),
        "expected missing-evidence error, got: {result:?}"
    );
}

#[test]
fn positive_flags_cannot_replace_exact_evidence_stage_or_revision() {
    let q1 = Uuid::new_v4();
    let behavior = Uuid::new_v4();
    let root = Uuid::new_v4();
    let bytes = declaration_json(Uuid::new_v4(), root, 1, q1, behavior);
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();
    let valid = json!({"format":"component-review-evidence/1","evidence_id":q1,
            "kind":"q1","validation_mode":"authored","succeeded":true,
            "association_checksum":"b".repeat(64),
            "components":[{"uuid":root,"class_code":22,"version":1,"checksum":fake_checksum()}],
            "reviewed_evidence":[],"report":{"scope":"shape-test-only"}});
    validate_evidence_declaration(&valid, q1, "q1", &declaration).unwrap();
    let mut wrongs = Vec::new();
    for (field, value) in [
        ("kind", json!("behavior")),
        ("evidence_id", json!(Uuid::new_v4())),
        ("validation_mode", json!("system_seed")),
        ("succeeded", json!(false)),
    ] {
        let mut value_doc = valid.clone();
        value_doc[field] = value;
        wrongs.push(value_doc);
    }
    let mut changed = valid.clone();
    changed["components"][0]["version"] = json!(2);
    wrongs.push(changed);
    let mut omitted = valid.clone();
    omitted["components"] = json!([]);
    wrongs.push(omitted);
    let mut reused = valid.clone();
    reused["reviewed_evidence"] = json!([q1]);
    wrongs.push(reused);
    for value in wrongs {
        assert!(validate_evidence_declaration(&value, q1, "q1", &declaration).is_err());
    }
}

#[tokio::test]
async fn positive_evidence_cannot_approve_an_omitted_immutable_dependency() {
    use brassclaw_skills::component_revision::ComponentRevisionDraft;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let draft = |id, deps| {
        ComponentRevisionDraft::from_json(
            &json!({
            "format":"component-revision/1","uuid":id,"class_code":22,
            "document":{"content":"def value(inputs):\n    return inputs\n"},
            "dependencies":deps,"association":null})
            .to_string(),
        )
        .unwrap()
    };
    let revisions = PgComponentRevisionStore::new(rig.pool.clone());
    let helper_id = Uuid::new_v4();
    revisions.stage(&draft(helper_id, vec![]), 0).await.unwrap();
    let root = revisions
        .stage(&draft(Uuid::new_v4(), vec![helper_id]), 0)
        .await
        .unwrap();
    let q1 = Uuid::new_v4();
    let behavior = Uuid::new_v4();
    let approval = Uuid::new_v4();
    let entry = json!({"uuid":root.uuid,"class_code":22,"version":1,
                          "checksum":hex::encode(root.checksum)});
    let bytes = json!({"format":"pure-component-approval/1","approval_id":approval,
            "root":entry.clone(),"components":[entry.clone()],"validation_mode":"system_seed",
            "q1_ref":q1,"q2_ref":null,"behavioral_refs":[behavior]})
    .to_string();
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();
    let client = rig.pool.get().await.unwrap();
    for (id, kind) in [(q1, "q1"), (behavior, "behavior")] {
        let evidence = json!({"format":"component-review-evidence/1","evidence_id":id,
                "kind":kind,"validation_mode":"system_seed","succeeded":true,
                "association_checksum":"b".repeat(64),
                "components":[entry.clone()],"reviewed_evidence":[],"report":{}})
        .to_string();
        client
            .execute(
                "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
                VALUES($1,$2,encode(sha256(convert_to($2::text,'UTF8')),'hex'))",
                &[&id, &evidence],
            )
            .await
            .unwrap();
    }
    assert!(matches!(
        record_pure_component_approval(&rig.pool, &declaration).await,
        Err(PureApprovalError::Revisions(_))
    ));
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_pure_component_approvals WHERE approval_id=$1",
                &[&approval]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
async fn pure_link_rejects_a_different_approved_root_in_writer_and_database() {
    use brassclaw_skills::{
        component_revision::ComponentRevisionDraft,
        review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft},
    };
    let rig = crate::runtime::test_pg::pg_rig().await;
    let make_draft = |id| {
        ComponentRevisionDraft::from_json(
            &json!({
            "format":"component-revision/1","uuid":id,"class_code":22,
            "document":{"content":"def value(inputs):\n    return inputs\n"},
            "dependencies":[],"association":null})
            .to_string(),
        )
        .unwrap()
    };
    let subject_id = Uuid::new_v4();
    let subject = PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                subject_id,
                "pure-link-integrity-test",
                make_draft(Uuid::new_v4()),
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let other = PgComponentRevisionStore::new(rig.pool.clone())
        .stage(&make_draft(Uuid::new_v4()), 0)
        .await
        .unwrap();
    let approval = Uuid::new_v4();
    let entry = json!({"uuid":other.uuid,"class_code":22,"version":other.version,
                          "checksum":hex::encode(other.checksum)});
    let q1 = Uuid::new_v4();
    let bytes = json!({"format":"pure-component-approval/1","approval_id":approval,
            "root":entry.clone(),"components":[entry],"validation_mode":"system_seed",
            "q1_ref":q1,"q2_ref":null,"behavioral_refs":[Uuid::new_v4()]})
    .to_string();
    let client = rig.pool.get().await.unwrap();
    // Storage-only negative fixture, not a trusted approval producer.
    client.execute("INSERT INTO reborn_pure_component_approvals
            (approval_id,root_id,root_version,root_checksum,validation_mode,q1_ref,q2_ref,approval_bytes,checksum)
            VALUES($1,$2,1,$3,'system_seed',$4,NULL,$5,encode(sha256(convert_to($5::text,'UTF8')),'hex'))",
            &[&approval,&other.uuid,&hex::encode(other.checksum),&q1,&bytes]).await.unwrap();
    let error = record_pure_evidence_submission_link(
        &rig.pool,
        Uuid::new_v4(),
        subject_id,
        approval,
        subject.candidate.uuid,
        subject.candidate.version,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error,PureApprovalError::Invalid(reason) if reason.contains("approval root differs"))
    );
    let sql_error = client.execute("INSERT INTO reborn_pure_evidence_submission_links
            (link_id,submission_id,approval_id,component_id,candidate_version) VALUES($1,$2,$3,$4,1)",
            &[&Uuid::new_v4(),&subject_id,&approval,&subject.candidate.uuid]).await.unwrap_err();
    assert_eq!(
        sql_error.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    let wrong_submission = client.execute("INSERT INTO reborn_pure_evidence_submission_links
            (link_id,submission_id,approval_id,component_id,candidate_version) VALUES($1,$2,$3,$4,1)",
            &[&Uuid::new_v4(),&subject_id,&approval,&other.uuid]).await.unwrap_err();
    assert_eq!(
        wrong_submission.code(),
        Some(&tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION)
    );
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_pure_evidence_submission_links",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}
