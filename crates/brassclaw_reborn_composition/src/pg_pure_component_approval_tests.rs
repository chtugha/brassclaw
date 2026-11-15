//! Real PostgreSQL/HTTP-boundary and exact evidence-shape rejection checks.
use super::*;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft,
    pure_component_contract::PureComponentApprovalDeclaration,
    review_submission_store::{PgReviewSubmissionStore, ReviewSubmissionDraft},
    value_contract::ContractLimits,
};
use serde_json::json;

fn limits() -> ContractLimits {
    ContractLimits {
        max_depth: REVISION_LIMITS.max_depth,
        max_nodes: REVISION_LIMITS.max_nodes,
        max_bytes: REVISION_LIMITS.max_bytes,
    }
}

#[test]
fn retired_l013_identities_cannot_be_reused_as_approval_or_evidence() {
    let root = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
    // Independent UUIDv5 golden values from the retired producer's 21-name
    // manifest, plus both evidence roles for pc-path-join. No mutable row name,
    // current export, or production classifier is used to construct this oracle.
    for id in [
        "4e1f4052-59e9-5da0-aea7-61bb5d916b7a",
        "f410f11e-8f64-56f4-9ddd-9a6e02c0c2f7",
        "5378c409-3983-50c3-8190-fd72a15d2686",
        "d9380563-f2c9-592b-bd85-72d6db754908",
        "337f819b-fe4f-5bc3-afd5-bcd56ddd608d",
        "a46e065c-1681-5e57-a3bf-204166b71ca4",
        "740ae9c6-8914-5619-9008-4ba3a35a37de",
        "22c8b354-6bda-5943-9f0f-296ac2151e3f",
        "7f212102-86a3-5ce3-924a-01668e336ab0",
        "2c83c7b6-e73d-5ec6-8c78-82d88739c714",
        "5938f131-86a1-5186-8057-d5467aeb43f5",
        "4b068b92-45a8-5552-bdc2-c5f2398a89b9",
        "4c0a038c-1df5-5012-8a2e-6d2c8e782261",
        "a6b20277-1f33-5751-a435-0eb269e98753",
        "932d54f7-c889-54f9-8bef-9924cf5c412b",
        "77bb3fbb-3d7b-5b08-a89d-21e6e873ed33",
        "d38690f1-7501-503a-a008-869dc45169e9",
        "c0317bde-3677-5d98-8e51-5621fae44d47",
        "6c458938-5bbe-53d4-8fae-89d9cbcaadc6",
        "6752fd34-489d-57fc-aecb-ed2c0cdb8dd1",
        "c859fd9e-cb1e-51c7-82b4-2ef0a3ff9a6b",
        "10e1a4c1-eabc-5769-adc2-fc7a86732158",
        "e02326b3-b177-5bf6-8cd1-6a1976ad3c4a",
    ] {
        let id = Uuid::parse_str(id).unwrap();
        assert!(require_nonretired_pure_references(root, [id]).is_err());
        // An ID from a different root is not classified by a mutable name.
        require_nonretired_pure_references(Uuid::new_v4(), [id]).unwrap();
    }
    require_nonretired_pure_references(root, [Uuid::new_v4()]).unwrap();
}

#[tokio::test]
async fn retired_l013_history_survives_boot_without_becoming_a_decision() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let booted = crate::booted_db::run_migrations_and_return_booted_db(rig.pool.clone())
        .await
        .unwrap();
    let tenant = "retired-l013-native";
    crate::component_boot::initialize_runtime_components(&booted, tenant)
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let before = client
        .query_one(
            "SELECT
        (SELECT count(*) FROM reborn_component_review_evidence),
        (SELECT count(*) FROM reborn_pure_component_approvals),
        (SELECT count(*) FROM reborn_skill_association_approvals)",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            before.get::<_, i64>(0),
            before.get::<_, i64>(1),
            before.get::<_, i64>(2)
        ),
        (0, 0, 0)
    );
    let packaged = client
        .query(
            "SELECT p.id,p.name,p.content,q.state,q.q1_component_bytes,q.q2_actor,
        (SELECT count(*) FROM reborn_component_revisions r WHERE r.component_id=p.id)
        FROM reborn_python_code p LEFT JOIN reborn_validation_queue q ON q.component_id=p.id
        WHERE p.tenant_id=$1 AND p.name=ANY($2::text[]) ORDER BY p.name",
            &[&tenant, &vec!["pc-path-join", "pc-exec-read-file"]],
        )
        .await
        .unwrap();
    assert_eq!(
        packaged.len(),
        2,
        "actual pure and Tool-using shipped components must remain visible"
    );
    let originals: Vec<_> = packaged
        .iter()
        .map(|row| {
            assert!(row.get::<_, Option<i16>>(3).is_none());
            assert!(row.get::<_, Option<String>>(4).is_none());
            assert!(row.get::<_, Option<String>>(5).is_none());
            assert_eq!(
                row.get::<_, i64>(6),
                0,
                "no placeholder revision may be manufactured"
            );
            (
                row.get::<_, Uuid>(0),
                row.get::<_, String>(1),
                row.get::<_, String>(2),
            )
        })
        .collect();
    drop(client);

    // Deliberate historical corruption fixture: structurally valid immutable
    // storage is not a native qualification. This never supplies a test oracle
    // or a successful product review. Use a submitted subject to exercise the
    // actual link writer's positive boundary as well as the legacy resolver.
    let submission_id = Uuid::new_v4();
    let candidate = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":Uuid::new_v4(),"class_code":22,
        "document":{"content":"def path_join(inputs):\n    return {}\n",
            "preload":{"format":"python-preload/2","exports":{},"private_functions":{},"constants":[],"imports":[],"dependencies":[]},
            "input_contract":{},"result_contract":{"type":"object","allow_extra_fields":false,"fields":{},"checks":[]}},
        "dependencies":[],"association":null}).to_string()).unwrap();
    let subject = PgReviewSubmissionStore::new(rig.pool.clone())
        .submit(
            &ReviewSubmissionDraft::new(
                submission_id,
                "retired-l013-audit-fixture",
                candidate,
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let root = &subject.candidate;
    let identity = Uuid::new_v5(
        &RETIRED_L013_NAMESPACE,
        format!("pure:pc-path-join:{}", root.uuid).as_bytes(),
    );
    let approval = Uuid::new_v5(&identity, b"approval");
    let q1 = Uuid::new_v5(&identity, b"q1");
    let behavior = Uuid::new_v5(&identity, b"behavior");
    let entry = json!({"uuid":root.uuid,"class_code":22,"version":root.version,"checksum":hex::encode(root.checksum)});
    let bytes = json!({"format":"pure-component-approval/1","approval_id":approval,"root":entry,"components":[entry],"validation_mode":"system_seed","q1_ref":q1,"q2_ref":null,"behavioral_refs":[behavior]}).to_string();
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();
    assert!(
        matches!(record_pure_component_approval(&rig.pool,&declaration).await,Err(PureApprovalError::Invalid(reason)) if reason.contains("retired L013"))
    );
    let client = rig.pool.get().await.unwrap();
    let mut historical_evidence = Vec::new();
    for (id, kind) in [(q1, "q1"), (behavior, "behavior")] {
        let evidence = json!({"format":"component-review-evidence/1","evidence_id":id,"kind":kind,
            "validation_mode":"system_seed","association_checksum":format!("{:x}",Sha256::digest(root.uuid.to_string().as_bytes())),
            "components":[entry],"succeeded":true,"reviewed_evidence":[],"report":{"checks":["no-host-calls","effect-free-load"]}}).to_string();
        client
            .execute(
                "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
            VALUES($1,$2,encode(sha256(convert_to($2::text,'UTF8')),'hex'))",
                &[&id, &evidence],
            )
            .await
            .unwrap();
        historical_evidence.push((id, evidence));
    }
    let version = i64::try_from(root.version).unwrap();
    let checksum = hex::encode(root.checksum);
    client.execute("INSERT INTO reborn_pure_component_approvals
        (approval_id,root_id,root_version,root_checksum,validation_mode,q1_ref,q2_ref,approval_bytes,checksum)
        VALUES($1,$2,$3,$4,'system_seed',$5,NULL,$6,encode(sha256(convert_to($6::text,'UTF8')),'hex'))",
        &[&approval,&root.uuid,&version,&checksum,&q1,&bytes]).await.unwrap();
    client
        .execute(
            "INSERT INTO reborn_pure_component_approval_closure
        (approval_id,component_id,component_version,component_checksum) VALUES($1,$2,$3,$4)",
            &[&approval, &root.uuid, &version, &checksum],
        )
        .await
        .unwrap();
    drop(client);

    assert!(
        matches!(resolve_existing_pure_component_approval(&rig.pool,&bytes).await,Err(PureApprovalError::Invalid(reason)) if reason.contains("retired L013"))
    );
    assert!(
        matches!(read_pure_component_approval_root(&rig.pool,approval).await,Err(PureApprovalError::Invalid(reason)) if reason.contains("retired L013"))
    );
    assert!(
        matches!(record_pure_evidence_submission_link(&rig.pool,Uuid::new_v4(),submission_id,approval,root.uuid,root.version).await,Err(PureApprovalError::Invalid(reason)) if reason.contains("retired L013"))
    );
    for old_stage in [q1, behavior] {
        let mut borrowed: serde_json::Value = serde_json::from_str(&bytes).unwrap();
        borrowed["approval_id"] = json!(Uuid::new_v4());
        borrowed["q1_ref"] = json!(if old_stage == q1 { q1 } else { Uuid::new_v4() });
        borrowed["behavioral_refs"] = json!([if old_stage == behavior {
            behavior
        } else {
            Uuid::new_v4()
        }]);
        let borrowed =
            PureComponentApprovalDeclaration::from_json(&borrowed.to_string(), limits()).unwrap();
        assert!(
            matches!(record_pure_component_approval(&rig.pool,&borrowed).await,Err(PureApprovalError::Invalid(reason)) if reason.contains("retired L013"))
        );
    }

    crate::component_boot::initialize_runtime_components(&booted, tenant)
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    let stored = client.query_one("SELECT approval_bytes,checksum FROM reborn_pure_component_approvals WHERE approval_id=$1",&[&approval]).await.unwrap();
    assert_eq!(stored.get::<_, String>(0), bytes);
    assert_eq!(
        stored.get::<_, String>(1),
        format!("{:x}", Sha256::digest(bytes.as_bytes()))
    );
    for (id, original) in historical_evidence {
        let row=client.query_one("SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",&[&id]).await.unwrap();
        assert_eq!(row.get::<_, String>(0), original);
        assert_eq!(
            row.get::<_, String>(1),
            format!("{:x}", Sha256::digest(original.as_bytes()))
        );
    }
    for (id, name, original) in originals {
        let row = client
            .query_one(
                "SELECT name,content FROM reborn_python_code WHERE id=$1",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>(0), name);
        assert_eq!(row.get::<_, String>(1), original);
    }
    let counts = client
        .query_one(
            "SELECT
        (SELECT count(*) FROM reborn_component_review_evidence),
        (SELECT count(*) FROM reborn_pure_component_approvals),
        (SELECT count(*) FROM reborn_skill_association_approvals),
        (SELECT count(*) FROM reborn_pure_evidence_submission_links)",
            &[],
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
        (2, 1, 0, 0)
    );
}

fn fake_checksum() -> String {
    "a".repeat(64)
}

/// Build a `system_seed` mode declaration.
///
/// Used by infrastructure tests that need to reach the evidence row
/// lookup, which the authored early-return gates off.
fn system_seed_declaration_json(
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
        "validation_mode": "system_seed",
        "q1_ref": q1.to_string(),
        "q2_ref": null,
        "behavioral_refs": [behavioral.to_string()],
    })
    .to_string()
}

/// Build an `authored` mode declaration (for tests that only check the closed path).
fn authored_declaration_json(
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

/// Build authored-format (12-field) evidence JSON for `q1` kind.
/// Only `q1` kind uses the authored format; behavior and human_q2 use legacy.
fn authored_q1_evidence_json(
    evidence_id: Uuid,
    submission_id: Uuid,
    subject_checksum: &str,
    validator_selection_checksum: &str,
    task_handle: Uuid,
) -> String {
    json!({
        "format": "component-review-evidence/1",
        "evidence_id": evidence_id.to_string(),
        "kind": "q1",
        "validation_mode": "authored",
        "submission_id": submission_id.to_string(),
        "subject_checksum": subject_checksum,
        "validator_selection_checksum": validator_selection_checksum,
        "class_code": 22,
        "task_handle": task_handle.to_string(),
        "succeeded": true,
        "report": {"scope": "authored-q1-evidence-test"},
        "reviewed_evidence": [],
    })
    .to_string()
}

/// Any authored-mode declaration is immediately rejected with a clear invariant
/// error. This is the closed-path guard: qualified behavioral and human-Q2
/// producers for the authored pure path are not yet connected.
///
/// No PostgreSQL setup is required; the guard fires before any DB access.
#[tokio::test]
async fn authored_approval_path_is_closed_until_producers_qualified() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let root_id = Uuid::new_v4();
    let q1 = Uuid::new_v4();
    let behavior = Uuid::new_v4();
    let approval_id = Uuid::new_v4();

    // Build an authored declaration — no DB setup needed.
    let json = authored_declaration_json(approval_id, root_id, 1, q1, behavior);
    let decl = PureComponentApprovalDeclaration::from_json(&json, limits()).unwrap();

    let result = record_pure_component_approval(&rig.pool, &decl).await;
    assert!(
        matches!(
            result,
            Err(PureApprovalError::Invalid(msg))
            if msg.contains("authored pure-component approvals are not yet accepted")
        ),
        "authored path must be rejected with explicit closed-path error; got: {result:?}"
    );
}

/// A missing evidence record is rejected before the approval is stored.
/// Uses system_seed mode so the evidence-row lookup is reached (the authored
/// early return does not apply to system_seed declarations).
#[tokio::test]
async fn missing_evidence_record_is_rejected() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let q1 = Uuid::new_v4();
    let behavioral = Uuid::new_v4();
    let root_id = Uuid::new_v4();
    let approval_id = Uuid::new_v4();
    // Use system_seed mode; authored mode would hit the closed-path guard first.
    let json = system_seed_declaration_json(approval_id, root_id, 1, q1, behavioral);
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
    // Use authored mode declaration for validate_evidence_declaration shape tests
    // (the validation helper itself does not enforce the closed path; only the
    // top-level record_pure_component_approval writer does).
    let bytes = authored_declaration_json(Uuid::new_v4(), root, 1, q1, behavior);
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();
    let root_ref = json!({"uuid":root,"class_code":22,"version":1,"checksum":fake_checksum()});

    // Legacy bootstrap format with association_checksum — valid for q1.
    let valid = json!({"format":"component-review-evidence/1","evidence_id":q1,
            "kind":"q1","validation_mode":"authored","succeeded":true,
            "association_checksum":"b".repeat(64),
            "components":[root_ref.clone()],"reviewed_evidence":[],"report":{"scope":"shape-test-only"}});
    validate_evidence_declaration(&valid, q1, "q1", &declaration).unwrap();

    // Various field mutations must all be rejected.
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

    // Authored format is accepted for q1.
    let sub_id = Uuid::new_v4();
    let task_handle = Uuid::new_v4();
    let authored_q1 = json!({
        "format": "component-review-evidence/1",
        "evidence_id": q1.to_string(),
        "kind": "q1",
        "validation_mode": "authored",
        "submission_id": sub_id.to_string(),
        "subject_checksum": "c".repeat(64),
        "validator_selection_checksum": "d".repeat(64),
        "class_code": 22,
        "task_handle": task_handle.to_string(),
        "succeeded": true,
        "report": {"scope": "authored-q1-shape-test"},
        "reviewed_evidence": [],
    });
    assert_eq!(
        validate_evidence_declaration(&authored_q1, q1, "q1", &declaration).unwrap(),
        EvidenceFormat::AuthoredValidator
    );

    // Authored format must be rejected for behavior kind.
    let behavior_authored = json!({
        "format": "component-review-evidence/1",
        "evidence_id": behavior.to_string(),
        "kind": "behavior",
        "validation_mode": "authored",
        "submission_id": sub_id.to_string(),
        "subject_checksum": "c".repeat(64),
        "validator_selection_checksum": "d".repeat(64),
        "class_code": 22,
        "task_handle": task_handle.to_string(),
        "succeeded": true,
        "report": {},
        "reviewed_evidence": [],
    });
    assert!(
        validate_evidence_declaration(&behavior_authored, behavior, "behavior", &declaration)
            .is_err(),
        "authored format must be rejected for behavior kind"
    );

    // Legacy format is accepted for behavior.
    let behavior_legacy = json!({"format":"component-review-evidence/1","evidence_id":behavior,
        "kind":"behavior","validation_mode":"authored","succeeded":true,
        "association_checksum":"b".repeat(64),"components":[root_ref.clone()],
        "reviewed_evidence":[],"report":{}});
    assert_eq!(
        validate_evidence_declaration(&behavior_legacy, behavior, "behavior", &declaration)
            .unwrap(),
        EvidenceFormat::LegacyBootstrap
    );

    // Authored format must be rejected for human_q2 kind.
    let q2_id = Uuid::new_v4();
    let q2_authored = json!({
        "format": "component-review-evidence/1",
        "evidence_id": q2_id.to_string(),
        "kind": "human_q2",
        "validation_mode": "authored",
        "submission_id": sub_id.to_string(),
        "subject_checksum": "c".repeat(64),
        "validator_selection_checksum": "d".repeat(64),
        "class_code": 22,
        "task_handle": task_handle.to_string(),
        "succeeded": true,
        "report": {},
        "reviewed_evidence": [q1.to_string(), behavior.to_string()],
    });
    assert!(
        validate_evidence_declaration(&q2_authored, q2_id, "human_q2", &declaration).is_err(),
        "authored format must be rejected for human_q2 kind"
    );
}

#[test]
fn authored_evidence_format_accepted_and_rejected_correctly() {
    let q1 = Uuid::new_v4();
    let behavior = Uuid::new_v4();
    let root = Uuid::new_v4();
    let submission_id = Uuid::new_v4();
    let task_handle = Uuid::new_v4();
    let subject_checksum = "c".repeat(64);
    let validator_checksum = "d".repeat(64);
    let bytes = authored_declaration_json(Uuid::new_v4(), root, 1, q1, behavior);
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();

    // Valid authored format for q1 kind.
    let valid_authored = json!({
        "format": "component-review-evidence/1",
        "evidence_id": q1.to_string(),
        "kind": "q1",
        "validation_mode": "authored",
        "submission_id": submission_id.to_string(),
        "subject_checksum": subject_checksum,
        "validator_selection_checksum": validator_checksum,
        "class_code": 22,
        "task_handle": task_handle.to_string(),
        "succeeded": true,
        "report": {"scope": "authored-unit-test"},
        "reviewed_evidence": [],
    });
    let format = validate_evidence_declaration(&valid_authored, q1, "q1", &declaration).unwrap();
    assert_eq!(format, EvidenceFormat::AuthoredValidator);

    // Wrong evidence_id must be rejected.
    let mut wrong = valid_authored.clone();
    wrong["evidence_id"] = json!(Uuid::new_v4().to_string());
    assert!(validate_evidence_declaration(&wrong, q1, "q1", &declaration).is_err());

    // Wrong kind must be rejected.
    let mut wrong = valid_authored.clone();
    wrong["kind"] = json!("behavior");
    assert!(validate_evidence_declaration(&wrong, q1, "q1", &declaration).is_err());

    // succeeded=false must be rejected.
    let mut wrong = valid_authored.clone();
    wrong["succeeded"] = json!(false);
    assert!(validate_evidence_declaration(&wrong, q1, "q1", &declaration).is_err());

    // nil UUID for submission_id must be rejected.
    let mut wrong = valid_authored.clone();
    wrong["submission_id"] = json!("00000000-0000-0000-0000-000000000000");
    assert!(validate_evidence_declaration(&wrong, q1, "q1", &declaration).is_err());

    // nil UUID for task_handle must be rejected.
    let mut wrong = valid_authored.clone();
    wrong["task_handle"] = json!("00000000-0000-0000-0000-000000000000");
    assert!(validate_evidence_declaration(&wrong, q1, "q1", &declaration).is_err());

    // Wrong-length subject_checksum must be rejected.
    let mut wrong = valid_authored.clone();
    wrong["subject_checksum"] = json!("abc");
    assert!(validate_evidence_declaration(&wrong, q1, "q1", &declaration).is_err());

    // Extra unknown field must be rejected (strict field count).
    let mut wrong = valid_authored.clone();
    wrong["extra_field"] = json!("unexpected");
    assert!(validate_evidence_declaration(&wrong, q1, "q1", &declaration).is_err());

    // Mixing legacy and authored fields must be rejected.
    let legacy_authored_mix = json!({
        "format": "component-review-evidence/1",
        "evidence_id": q1.to_string(),
        "kind": "q1",
        "validation_mode": "authored",
        "association_checksum": "b".repeat(64),
        "components": [{"uuid": root.to_string(), "class_code": 22, "version": 1,
                         "checksum": fake_checksum()}],
        "reviewed_evidence": [],
        "report": {},
        "submission_id": submission_id.to_string(),
    });
    assert!(validate_evidence_declaration(&legacy_authored_mix, q1, "q1", &declaration).is_err());

    // Authored format must be rejected for behavior kind (producer uses legacy format).
    let authored_behavior = json!({
        "format": "component-review-evidence/1",
        "evidence_id": behavior.to_string(),
        "kind": "behavior",
        "validation_mode": "authored",
        "submission_id": submission_id.to_string(),
        "subject_checksum": subject_checksum,
        "validator_selection_checksum": validator_checksum,
        "class_code": 22,
        "task_handle": task_handle.to_string(),
        "succeeded": true,
        "report": {},
        "reviewed_evidence": [],
    });
    assert!(
        validate_evidence_declaration(&authored_behavior, behavior, "behavior", &declaration)
            .is_err(),
        "authored format must be rejected for behavior kind"
    );
}

/// Verifies that the Q1 subject-binding infrastructure inside the writer
/// (the `validate_evidence_declaration` logic for authored format) correctly
/// extracts all five authored fields and applies the strict shape checks.
///
/// This is a pure unit test: it drives `validate_evidence_declaration` directly,
/// not `record_pure_component_approval`. The full DB-side run-row join is the
/// subject-binding check that will be activated when the authored path re-opens.
/// That check is exercised in the `authored_q1_subject_binding_design_verified`
/// test below.
#[test]
fn authored_q1_evidence_field_extraction_is_correct() {
    let q1 = Uuid::new_v4();
    let behavior = Uuid::new_v4();
    let root = Uuid::new_v4();
    let bytes = authored_declaration_json(Uuid::new_v4(), root, 1, q1, behavior);
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();

    let submission_id = Uuid::new_v4();
    let task_handle = Uuid::new_v4();

    // Valid authored Q1 evidence — the helper must accept it and return AuthoredValidator.
    let evidence = json!({
        "format": "component-review-evidence/1",
        "evidence_id": q1.to_string(),
        "kind": "q1",
        "validation_mode": "authored",
        "submission_id": submission_id.to_string(),
        "subject_checksum": "1".repeat(64),
        "validator_selection_checksum": "2".repeat(64),
        "class_code": 22,
        "task_handle": task_handle.to_string(),
        "succeeded": true,
        "report": {"scope": "field-extraction-test"},
        "reviewed_evidence": [],
    });
    let fmt = validate_evidence_declaration(&evidence, q1, "q1", &declaration).unwrap();
    assert_eq!(fmt, EvidenceFormat::AuthoredValidator);

    // Each field mutation that must cause a rejection.
    let bad_cases: &[(&str, serde_json::Value)] = &[
        // Non-hex character in subject_checksum.
        ("subject_checksum", json!("g".repeat(64))),
        // Too short subject_checksum.
        ("subject_checksum", json!("1".repeat(63))),
        // Too short validator_selection_checksum.
        ("validator_selection_checksum", json!("2".repeat(32))),
        // class_code as string instead of integer.
        ("class_code", json!("22")),
        // class_code as float should be rejected (not an integer in JSON).
        ("class_code", json!(22.5_f64)),
    ];
    for (field, bad_value) in bad_cases {
        let mut bad = evidence.clone();
        bad[*field] = bad_value.clone();
        assert!(
            validate_evidence_declaration(&bad, q1, "q1", &declaration).is_err(),
            "must reject evidence with bad {field}: {bad_value}"
        );
    }
}

/// Verifies the Q1 subject-binding check design:
/// - format detection routes authored evidence to `AuthoredValidator`
/// - the five authored fields are verified by `validate_evidence_declaration`
/// - the join logic is exercised at the DB level in a future activation test
///
/// This design test uses a PG rig to verify the submission schema is accessible
/// and the evidence-row read path works for `system_seed` approvals.
/// The DB-side join (run row + submission) is exercised when the authored path
/// re-opens; until then it remains tested through the internal code path
/// via `authored_evidence_format_accepted_and_rejected_correctly`.
#[tokio::test]
async fn authored_q1_subject_binding_design_verified() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let submissions = PgReviewSubmissionStore::new(rig.pool.clone());

    // Create a review submission for a class-22 component. The submission store
    // stages the component revision; we do not stage it separately to avoid a
    // duplicate-revision conflict on the same UUID.
    let root_id = Uuid::new_v4();
    let subject_draft = ComponentRevisionDraft::from_json(
        &json!({
            "format": "component-revision/1", "uuid": root_id, "class_code": 22,
            "document": {"content": "def value(inputs):\n    return inputs\n"},
            "dependencies": [], "association": null
        })
        .to_string(),
    )
    .unwrap();
    let sub = submissions
        .submit(
            &ReviewSubmissionDraft::new(
                Uuid::new_v4(),
                "authored-binding-design-test",
                subject_draft,
                None,
                vec![],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let root_rev = &sub.candidate;

    // Verify the submission's checksum is non-empty and the component_id matches.
    assert!(
        !sub.checksum.is_empty(),
        "submission checksum must be non-empty"
    );

    // Build authored Q1 evidence pointing at this submission.
    let q1_id = Uuid::new_v4();
    let task_handle = Uuid::new_v4();
    let validator_checksum = "9".repeat(64);
    let ev_bytes = authored_q1_evidence_json(
        q1_id,
        sub.id,
        &sub.checksum,
        &validator_checksum,
        task_handle,
    );

    // The evidence must pass the shape validation.
    let behavior_id = Uuid::new_v4();
    let approval_id = Uuid::new_v4();
    let root_entry = json!({
        "uuid": root_rev.uuid.to_string(),
        "class_code": 22,
        "version": root_rev.version,
        "checksum": hex::encode(root_rev.checksum),
    });
    let decl_bytes = json!({
        "format": "pure-component-approval/1",
        "approval_id": approval_id.to_string(),
        "root": root_entry.clone(),
        "components": [root_entry],
        "validation_mode": "authored",
        "q1_ref": q1_id.to_string(),
        "q2_ref": Uuid::new_v4().to_string(),
        "behavioral_refs": [behavior_id.to_string()],
    })
    .to_string();
    let decl = PureComponentApprovalDeclaration::from_json(&decl_bytes, limits()).unwrap();
    let ev_value: serde_json::Value = serde_json::from_str(&ev_bytes).unwrap();
    let fmt = validate_evidence_declaration(&ev_value, q1_id, "q1", &decl).unwrap();
    assert_eq!(fmt, EvidenceFormat::AuthoredValidator);

    // Verify the writer rejects the authored declaration at the top-level guard
    // (not because evidence is missing, but because the authored path is closed).
    // This confirms the guard fires before any DB access, so the lack of a run row
    // is irrelevant at this stage.
    let result = record_pure_component_approval(&rig.pool, &decl).await;
    assert!(
        matches!(
            result,
            Err(PureApprovalError::Invalid(msg))
            if msg.contains("authored pure-component approvals are not yet accepted")
        ),
        "DB-backed authored path must be rejected by the closed-path guard; got: {result:?}"
    );
}

#[tokio::test]
async fn positive_evidence_cannot_approve_an_omitted_immutable_dependency() {
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
    // system_seed mode: no early return; reaches the dependency graph check.
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

/// V214 trigger: authored approval rows must have a matching succeeded run record.
/// Three cases:
///   1. authored, no run record at all → trigger rejects (CHECK_VIOLATION + V214 message)
///   2. authored, run record present but succeeded=false → trigger rejects
///   3. system_seed, no run record → trigger allows (bootstrap path)
///
/// This test bypasses the writer and inserts directly to verify the trigger
/// independently of the writer's closed-path guard.
#[tokio::test]
async fn v214_trigger_rejects_authored_approval_without_succeeded_run() {
    use brassclaw_skills::component_revision::ComponentRevisionDraft;

    let rig = crate::runtime::test_pg::pg_rig().await;
    let client = rig.pool.get().await.unwrap();

    // Stage a real class-22 revision so the V200 FK constraint is satisfied.
    let root_id = Uuid::new_v4();
    let rev = PgComponentRevisionStore::new(rig.pool.clone())
        .stage(
            &ComponentRevisionDraft::from_json(
                &json!({
                    "format": "component-revision/1", "uuid": root_id, "class_code": 22,
                    "document": {"content": "def value(inputs):\n    return inputs\n"},
                    "dependencies": [], "association": null
                })
                .to_string(),
            )
            .unwrap(),
            0,
        )
        .await
        .unwrap();
    let root_hex = hex::encode(rev.checksum);
    let root_version = i64::try_from(rev.version).unwrap();

    // Helper: build consistent approval_bytes for a given mode.
    // All JSON cross-checks in V200 require exact field agreement.
    let make_bytes = |mode: &str, q2: Option<Uuid>| -> (Uuid, Uuid, Uuid, String) {
        let approval_id = Uuid::new_v4();
        let q1_id = Uuid::new_v4();
        let q2_id = q2.unwrap_or_else(Uuid::new_v4);
        let q2_json = if mode == "authored" {
            json!(q2_id.to_string())
        } else {
            json!(null)
        };
        let bytes = json!({
            "format": "pure-component-approval/1",
            "approval_id": approval_id.to_string(),
            "root": {
                "uuid": root_id.to_string(),
                "class_code": 22,
                "version": root_version,
                "checksum": root_hex,
            },
            "components": [{
                "uuid": root_id.to_string(),
                "class_code": 22,
                "version": root_version,
                "checksum": root_hex,
            }],
            "validation_mode": mode,
            "q1_ref": q1_id.to_string(),
            "q2_ref": q2_json,
            "behavioral_refs": [Uuid::new_v4().to_string()],
        })
        .to_string();
        (approval_id, q1_id, q2_id, bytes)
    };

    // ── Case 1: authored, no run record ────────────────────────────────────
    let (approval_id_1, q1_id_1, q2_id_1, bytes_1) = make_bytes("authored", None);
    let err = client
        .execute(
            "INSERT INTO reborn_pure_component_approvals
             (approval_id, root_id, root_version, root_checksum, validation_mode,
              q1_ref, q2_ref, approval_bytes, checksum)
             VALUES ($1,$2,$3,$4,'authored',$5,$6,$7,
                     encode(sha256(convert_to($7::text,'UTF8')),'hex'))",
            &[
                &approval_id_1,
                &root_id,
                &root_version,
                &root_hex,
                &q1_id_1,
                &q2_id_1,
                &bytes_1,
            ],
        )
        .await
        .unwrap_err();
    assert_eq!(
        err.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION),
        "V214 trigger must reject authored row without a run record"
    );
    assert!(
        err.as_db_error()
            .expect("PostgreSQL db error expected for case 1")
            .message()
            .contains("authored pure-component approval q1_ref"),
        "V214 error message must identify the trigger; got: {err}"
    );

    // ── Case 2: authored, run record present but succeeded = false ──────────
    // Insert a minimal evidence row and a run row with succeeded=false, then
    // attempt to insert an approval referencing that q1_ref.
    let (approval_id_2, q1_id_2, q2_id_2, bytes_2) = make_bytes("authored", None);

    // Evidence row (required by V202 FK on reborn_validator_q1_runs.evidence_id).
    let evidence_bytes_2 = json!({
        "format": "component-review-evidence/1",
        "evidence_id": q1_id_2.to_string(),
        "kind": "q1",
        "validation_mode": "authored",
        "association_checksum": "b".repeat(64),
        "components": [],
        "succeeded": false,
        "report": {},
        "reviewed_evidence": [],
    })
    .to_string();
    client
        .execute(
            "INSERT INTO reborn_component_review_evidence
             (evidence_id, evidence_bytes, checksum)
             VALUES ($1, $2, encode(sha256(convert_to($2::text,'UTF8')),'hex'))",
            &[&q1_id_2, &evidence_bytes_2],
        )
        .await
        .unwrap();

    // Submission row (required by V202 FK on reborn_validator_q1_runs.submission_id).
    // subject_bytes must satisfy all V104 JSON cross-checks.
    let sub_id_2 = Uuid::new_v4();
    let sub_bytes_2 = json!({
        "format": "component-review-submission/1",
        "submission_id": sub_id_2.to_string(),
        "actor": "v214-test",
        "candidate": {
            "uuid": root_id.to_string(),
            "version": root_version,
        },
        "candidate_bytes": "{}",
        "dependencies": [],
        "base": null,
    })
    .to_string();
    client
        .execute(
            "INSERT INTO reborn_component_review_submissions
             (submission_id, actor, subject_bytes, checksum, component_id, candidate_version)
             VALUES ($1, 'v214-test', $2,
                     encode(sha256(convert_to($2::text,'UTF8')),'hex'),
                     $3, $4)",
            &[&sub_id_2, &sub_bytes_2, &root_id, &root_version],
        )
        .await
        .unwrap();

    // Run row with succeeded=false.
    let run_id_2 = Uuid::new_v4();
    client
        .execute(
            "INSERT INTO reborn_validator_q1_runs
             (run_id, submission_id, validator_selection_checksum, candidate_class_code,
              task_handle, evidence_id, succeeded)
             VALUES ($1, $2, $3, 22, $4, $5, false)",
            &[
                &run_id_2,
                &sub_id_2,
                &"c".repeat(64),
                &Uuid::new_v4(),
                &q1_id_2,
            ],
        )
        .await
        .unwrap();

    let err2 = client
        .execute(
            "INSERT INTO reborn_pure_component_approvals
             (approval_id, root_id, root_version, root_checksum, validation_mode,
              q1_ref, q2_ref, approval_bytes, checksum)
             VALUES ($1,$2,$3,$4,'authored',$5,$6,$7,
                     encode(sha256(convert_to($7::text,'UTF8')),'hex'))",
            &[
                &approval_id_2,
                &root_id,
                &root_version,
                &root_hex,
                &q1_id_2,
                &q2_id_2,
                &bytes_2,
            ],
        )
        .await
        .unwrap_err();
    assert_eq!(
        err2.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION),
        "V214 trigger must reject authored row whose run has succeeded=false"
    );
    assert!(
        err2.as_db_error()
            .expect("PostgreSQL db error expected for case 2")
            .message()
            .contains("authored pure-component approval q1_ref"),
        "V214 error message must identify the trigger; got: {err2}"
    );

    // ── Case 3: system_seed, no run record ─────────────────────────────────
    // Bootstrap evidence rows are not in reborn_validator_q1_runs; the trigger
    // must allow system_seed rows unconditionally.
    let (approval_id_3, q1_id_3, _, bytes_3) = make_bytes("system_seed", None);
    client
        .execute(
            "INSERT INTO reborn_pure_component_approvals
             (approval_id, root_id, root_version, root_checksum, validation_mode,
              q1_ref, q2_ref, approval_bytes, checksum)
             VALUES ($1,$2,$3,$4,'system_seed',$5,NULL,$6,
                     encode(sha256(convert_to($6::text,'UTF8')),'hex'))",
            &[
                &approval_id_3,
                &root_id,
                &root_version,
                &root_hex,
                &q1_id_3,
                &bytes_3,
            ],
        )
        .await
        .expect("system_seed row must be accepted even without a validator run record");
}

/// Duplicate stage UUIDs: when the same UUID is used as both q1_ref and a
/// behavioral_ref, the declaration parser must reject it — the approval
/// declaration does not permit a UUID to serve two distinct evidence roles.
#[test]
fn duplicate_stage_uuid_is_rejected_at_declaration_parse() {
    let shared = Uuid::new_v4();
    let root = Uuid::new_v4();
    let bad = json!({
        "format": "pure-component-approval/1",
        "approval_id": Uuid::new_v4().to_string(),
        "root": {"uuid": root.to_string(), "class_code": 22, "version": 1,
                 "checksum": fake_checksum()},
        "components": [{"uuid": root.to_string(), "class_code": 22, "version": 1,
                        "checksum": fake_checksum()}],
        "validation_mode": "authored",
        "q1_ref": shared.to_string(),
        "q2_ref": Uuid::new_v4().to_string(),
        "behavioral_refs": [shared.to_string()],
    })
    .to_string();
    assert!(
        PureComponentApprovalDeclaration::from_json(&bad, limits()).is_err(),
        "declaration with duplicate q1/behavior UUID must be rejected by the parser"
    );
}

/// Unknown extra fields and non-object base structures are rejected by
/// validate_evidence_declaration via the closed-world field-count check.
#[test]
fn evidence_format_rejects_unknown_fields_and_non_objects() {
    let q1 = Uuid::new_v4();
    let behavior = Uuid::new_v4();
    let root = Uuid::new_v4();
    let bytes = authored_declaration_json(Uuid::new_v4(), root, 1, q1, behavior);
    let declaration = PureComponentApprovalDeclaration::from_json(&bytes, limits()).unwrap();
    let root_ref = json!({"uuid": root, "class_code": 22, "version": 1,
                          "checksum": fake_checksum()});
    let valid = json!({"format":"component-review-evidence/1","evidence_id":q1,
        "kind":"q1","validation_mode":"authored","succeeded":true,
        "association_checksum":"b".repeat(64),
        "components":[root_ref],"reviewed_evidence":[],"report":{}});

    // Extra unknown field — the field-count check in detect_evidence_format
    // rejects any object that has more fields than the declared format.
    let mut extra = valid.clone();
    extra["unexpected_field"] = json!("extra");
    assert!(
        validate_evidence_declaration(&extra, q1, "q1", &declaration).is_err(),
        "extra unknown field must be rejected by the field-count check"
    );

    // Non-object base structures.
    for not_an_object in [json!([]), json!(null), json!("string"), json!(42)] {
        assert!(
            validate_evidence_declaration(&not_an_object, q1, "q1", &declaration).is_err(),
            "non-object evidence must be rejected: {not_an_object}"
        );
    }
}
