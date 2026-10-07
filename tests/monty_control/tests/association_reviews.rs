//! Real immutable-record storage/selection fixtures. These are deliberately
//! inactive drafts and review fixtures, not actual Q1, behavioral or human-Q2
//! acceptance, activation evidence or ordinary application startup tests.
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::{ComponentRevisionRef, SkillAssociation},
    association_review_store::{AssociationReviewStoreError, retain_authored_association_reviews},
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn components(selected: &[ComponentRevisionRef]) -> Value {
    json!(
        selected
            .iter()
            .map(|r| json!({"uuid":r.uuid,"class_code":r.class_code,
        "version":r.version,"checksum":hex(r.checksum)}))
            .collect::<Vec<_>>()
    )
}

async fn usage(pool: &PgPool) -> (SkillAssociation, Vec<ComponentRevisionRef>, Uuid) {
    let (skill, python, binding, tool) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let association_bytes = json!({"format":"skill-association/1","skill_uuid":skill,
        "python_code_uuid":python,"tool_skill_uuid":binding,"tool_uuid":tool,
        "callable":"host.post_reply","inputs":{"answer":{"type":"string","required":true,"checks":[]}},
        "arguments":{"answer":"answer"},"code_arguments":{},"result":{"type":"string"},
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed",
            "idempotency_evidence_ref":null,"retryable_outcomes":[]}}).to_string();
    let mut selected = Vec::new();
    let mut client = pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    for (id, class, document, dependencies, association) in [
        (
            tool,
            0,
            json!({"capability_id":"fixture.post_reply"}),
            vec![],
            Value::Null,
        ),
        (binding, 13, json!({}), vec![tool], Value::Null),
        (
            python,
            22,
            json!({"content":"result = inputs['answer']"}),
            vec![],
            Value::Null,
        ),
        (
            skill,
            1,
            json!({"body":"Inactive review-record fixture."}),
            vec![python, binding, tool],
            json!(association_bytes),
        ),
    ] {
        let draft = ComponentRevisionDraft::from_json(
            &json!({"format":"component-revision/1",
            "uuid":id,"class_code":class,"document":document,"dependencies":dependencies,
            "association":association})
            .to_string(),
        )
        .unwrap();
        selected.push(
            PgComponentRevisionStore::stage_in_transaction(&tx, &draft, 0)
                .await
                .unwrap(),
        );
    }
    tx.commit().await.unwrap();
    (
        SkillAssociation::from_json(&association_bytes, REVISION_LIMITS).unwrap(),
        selected,
        python,
    )
}
fn evidence(
    id: Uuid,
    kind: &str,
    a: &SkillAssociation,
    refs: &[ComponentRevisionRef],
    reviewed: &[Uuid],
) -> Value {
    json!({"format":"component-review-evidence/1","evidence_id":id,"kind":kind,
        "validation_mode":"authored","association_checksum":hex(a.checksum()),
        "components":components(refs),"succeeded":true,
        "report":{"fixture":"record lifecycle only; no validation producer ran"},
        "reviewed_evidence":reviewed})
}
fn approval(
    id: Uuid,
    a: &SkillAssociation,
    refs: &[ComponentRevisionRef],
    q1: Uuid,
    q2: Uuid,
    behavior: &[Uuid],
) -> Value {
    json!({"format":"skill-association-approval/1","approval_id":id,
        "association_checksum":hex(a.checksum()),"components":components(refs),
        "validation_mode":"authored","q1_ref":q1,"q2_ref":q2,"behavioral_refs":behavior})
}
async fn insert_evidence(pool: &PgPool, value: &Value) {
    let id = Uuid::parse_str(value["evidence_id"].as_str().unwrap()).unwrap();
    let bytes = value.to_string();
    pool.get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
         VALUES ($1,$2,encode(sha256(convert_to($2::text,'UTF8')),'hex'))",
            &[&id, &bytes],
        )
        .await
        .unwrap();
}
async fn insert_approval(pool: &PgPool, value: &Value) {
    let id = Uuid::parse_str(value["approval_id"].as_str().unwrap()).unwrap();
    let bytes = value.to_string();
    pool.get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO reborn_skill_association_approvals(approval_id,approval_bytes,checksum)
         VALUES ($1,$2,encode(sha256(convert_to($2::text,'UTF8')),'hex'))",
            &[&id, &bytes],
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn exact_review_records_and_old_revisions_survive_replacement() {
    let rig = native_pg::NativePostgres::start().await;
    let (a, refs, python) = usage(&rig.pool).await;
    let (q1, behavior, q2, id) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let records = [
        evidence(q1, "q1", &a, &refs, &[]),
        evidence(behavior, "behavior", &a, &refs, &[]),
        evidence(q2, "human_q2", &a, &refs, &[q1, behavior]),
    ];
    for value in &records {
        insert_evidence(&rig.pool, value).await;
    }
    let record = approval(id, &a, &refs, q1, q2, &[behavior]);
    insert_approval(&rig.pool, &record).await;
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    let retained = retain_authored_association_reviews(&tx, id, &a, &refs)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(retained.approval().exact_bytes(), record.to_string());
    assert_eq!(retained.snapshot().revisions().len(), 4);
    for value in &records {
        let evidence_id = Uuid::parse_str(value["evidence_id"].as_str().unwrap()).unwrap();
        assert_eq!(retained.evidence()[&evidence_id], value.to_string());
    }
    let replacement = ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1",
        "uuid":python,"class_code":22,"document":{"content":"result = 'replacement'"},
        "dependencies":[],"association":null})
        .to_string(),
    )
    .unwrap();
    let newer = PgComponentRevisionStore::new(rig.pool.clone())
        .stage(&replacement, 1)
        .await
        .unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    retain_authored_association_reviews(&tx, id, &a, &refs)
        .await
        .unwrap();
    let mut changed = refs.clone();
    *changed.iter_mut().find(|r| r.uuid == python).unwrap() = newer;
    assert!(matches!(
        retain_authored_association_reviews(&tx, id, &a, &changed).await,
        Err(AssociationReviewStoreError::Combination)
    ));
    tx.commit().await.unwrap();
    for sql in [
        "UPDATE reborn_component_review_evidence SET evidence_bytes=evidence_bytes",
        "DELETE FROM reborn_component_review_evidence",
        "TRUNCATE reborn_component_review_evidence",
        "UPDATE reborn_skill_association_approvals SET approval_bytes=approval_bytes",
        "DELETE FROM reborn_skill_association_approvals",
        "TRUNCATE reborn_skill_association_approvals",
    ] {
        let error = client.execute(sql, &[]).await.unwrap_err();
        assert_eq!(error.code().unwrap().code(), "23514");
    }
    // Retained sensitive evidence has not become a grant or an active component.
    assert_eq!(retained.evidence().len(), 3);
}

#[tokio::test]
async fn missing_failed_wrong_combination_and_unreviewed_behavior_fail_closed() {
    let rig = native_pg::NativePostgres::start().await;
    let (a, refs, _) = usage(&rig.pool).await;
    for scenario in 0..7 {
        let (q1, b, q2, id) = (
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );
        let mut q1_record = evidence(q1, "q1", &a, &refs, &[]);
        let mut behavior_record = evidence(b, "behavior", &a, &refs, &[]);
        let mut q2_record = evidence(q2, "human_q2", &a, &refs, &[q1, b]);
        let mut declaration = approval(id, &a, &refs, q1, q2, &[b]);
        match scenario {
            0 => {} // missing actual Q1 record
            1 => q1_record["succeeded"] = json!(false),
            2 => behavior_record["components"][0]["checksum"] = json!("00".repeat(32)),
            3 => q2_record["reviewed_evidence"] = json!([q1]),
            4 => declaration["behavioral_refs"] = json!([b, b]),
            5 => q2_record["kind"] = json!("q1"),
            6 => {
                declaration["validation_mode"] = json!("system_seed");
                declaration["q2_ref"] = Value::Null;
            }
            _ => unreachable!(),
        }
        if scenario != 0 {
            insert_evidence(&rig.pool, &q1_record).await;
        }
        insert_evidence(&rig.pool, &behavior_record).await;
        insert_evidence(&rig.pool, &q2_record).await;
        insert_approval(&rig.pool, &declaration).await;
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let error = retain_authored_association_reviews(&tx, id, &a, &refs)
            .await
            .err()
            .unwrap();
        assert!(matches!(
            error,
            AssociationReviewStoreError::Evidence
                | AssociationReviewStoreError::UnsupportedSystemSeed
        ));
        tx.commit().await.unwrap();
        let tx = client.transaction().await.unwrap();
        assert!(matches!(
            retain_authored_association_reviews(&tx, id, &a, &refs).await,
            Err(AssociationReviewStoreError::Isolation)
        ));
        tx.rollback().await.unwrap();
    }
}
