//! Actual serializer/PG retention/migration checks. Drafts and passive pin records
//! below establish no component approval, activation or Tool permission.
#![cfg(feature = "skills-db")]
use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_selection_v3::build_v3_selection,
};
use brassclaw_host_api::{
    MontyExecutionLimits,
    task_preparation::{PreparationOrigin, RetainedPreparation},
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    retained_selection_contract::{
        FORMAT_V3, SelectionContractError, SelectionVersion, parse_selection_version,
    },
    review_submission_store::{
        PgReviewSubmissionStore, PreparedReviewContext, ReviewSubmissionDraft,
        SubmissionStoreError, preparation_value_limits, selection,
    },
    revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;
#[path = "../../brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;
fn digest(bytes: &str) -> String {
    format!("{:x}", Sha256::digest(bytes.as_bytes()))
}
fn reference(r: ComponentRevisionRef) -> Value {
    json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":r.checksum.iter().map(|b| format!("{b:02x}")).collect::<String>()})
}
fn draft(
    id: Uuid,
    class: i32,
    doc: Value,
    deps: &[Uuid],
    preparation: &RetainedPreparation,
) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json_with_preparation(
        &json!({"format":"component-revision/1","uuid":id,"class_code":class,
        "document":doc,"dependencies":deps,"association":null})
        .to_string(),
        Some(preparation),
    )
    .unwrap()
}
struct Fixture {
    submission: Uuid,
    subject: String,
    candidate: ComponentRevisionRef,
    v1: String,
    v3: String,
    preparation: RetainedPreparation,
    root: ComponentRevisionRef,
}
impl Fixture {
    fn outer(&self, inner: &str) -> String {
        json!({"format":"component-validator-selection/1","submission_id":self.submission,"subject_checksum":self.subject,
            "candidate":reference(self.candidate),"validator_selection_bytes":inner,"validator_selection_checksum":digest(inner),"scope":"preparation-only"}).to_string()
    }
}
async fn fixture(pool: &Arc<brassclaw_pg::PgPool>, large: bool) -> Fixture {
    let policy = MontyExecutionLimits::default(); // existing real profile; no invented product quota
    let preparation =
        RetainedPreparation::encode(1, PreparationOrigin::AcknowledgedStartup, policy).unwrap();
    let limits = preparation_value_limits(&preparation).unwrap();
    let store = PgComponentRevisionStore::new(pool.clone());
    let code_id = Uuid::new_v4();
    let value_schema = if large {
        json!({"type":"list","items":{"type":"null"}})
    } else {
        json!({"type":"string"})
    };
    let mut input_schema = value_schema.clone();
    input_schema["required"] = json!(true);
    input_schema["checks"] = json!([]);
    let code = draft(
        code_id,
        22,
        json!({"content":"def run_step(inputs):\n    return inputs[\"value\"]\n", "input_contract":{"value":input_schema}, "result_contract":value_schema, "preload":{"format":"python-preload/2",
        "exports":{"run_step":{"symbol":"run_step","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"run_step"}}),
        &[],
        &preparation,
    );
    let code_ref = store.stage(&code, 0).await.unwrap();
    let constant = if large {
        json!(vec![Value::Null; REVISION_LIMITS.max_nodes + 1])
    } else {
        json!("original")
    };
    let root_id = Uuid::new_v4();
    let recipe = draft(
        root_id,
        21,
        json!({"validates_class_code":22,"consumer_tags":["05:validator"],
        "variants":[{"variant_key":"selected","description":"exact fixture","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
        "step_descriptions":[{"desc_idx":0,"label":"fixture","yaml_source":"","steps":[{"stepnumber":1,"knowledge":"orchestrator","goal":"retain","content":"","type":"component","include":[code_id]}]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{},"steps":{"0:1":{"value":{"kind":"constant","value":constant}}}}},
        "invocations":{"selected":{"0:1":"run_step"}}}),
        &[code_id],
        &preparation,
    );
    let root = store.stage(&recipe, 0).await.unwrap();
    let snapshot = Arc::new(
        store
            .read_exact(
                &[root_id],
                &[root, code_ref],
                brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
            )
            .await
            .unwrap(),
    );
    let instruction = compile_retained_recipe(
        snapshot.clone(),
        root_id,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    let serialized = instruction.retained_selection_with_limits(limits).unwrap();
    let v1 = serialized.exact_bytes().to_owned();
    let parsed = parse_selection_version(&v1, limits).unwrap();
    parsed.require_snapshot(&snapshot).unwrap();
    assert_eq!(parsed.selection().checksum(), serialized.checksum());
    assert!(parsed.selection().value()["recipe"]["checksum"].is_string());
    let mut value = parsed.into_value();
    value["format"] = json!(FORMAT_V3);
    value["functional_approvals"] = json!({"0:1":{"profile":"pure_own","implementation":code_ref,"export":"run_step","interface_checksum":"a".repeat(64),
        "own_approvals":{code_id.to_string():{"root":code_ref,"approval_id":Uuid::new_v4(),"approval_checksum":"b".repeat(64),"subject_checksum":"c".repeat(64)}}}});
    let SelectionVersion::V3(pins) = parse_selection_version(&value.to_string(), limits).unwrap()
    else {
        panic!()
    };
    drop(value); // the builder borrows the already parsed original below
    let v3 = build_v3_selection(
        pins.base(),
        &BTreeMap::new(),
        pins.functional_approvals(),
        None,
        limits,
    )
    .unwrap()
    .exact_bytes()
    .to_owned();
    parse_selection_version(&v3, limits)
        .unwrap()
        .require_snapshot(&snapshot)
        .unwrap();
    let submission = Uuid::new_v4();
    let candidate = draft(
        Uuid::new_v4(),
        22,
        json!({"content":"def candidate(inputs):\n    return inputs\n"}),
        &[],
        &preparation,
    );
    let subject = PgReviewSubmissionStore::new(pool.clone())
        .submit(
            &ReviewSubmissionDraft::new_prepared(
                submission,
                "b2-r01",
                candidate,
                None,
                vec![],
                PreparedReviewContext {
                    expected_previous_version: 0,
                    partner: None,
                    preparation: preparation.clone(),
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();
    Fixture {
        submission,
        subject: subject.checksum,
        candidate: subject.candidate,
        v1,
        v3,
        preparation,
        root,
    }
}
#[tokio::test]
async fn selection_contract_pg_retention_rejects_v1_extensions_and_preserves_originals() {
    let rig = native_pg::NativePostgres::start().await;
    let f = fixture(&rig.pool, false).await;
    let limits = preparation_value_limits(&f.preparation).unwrap();
    // Each forged selection starts from the actual supported serializer output.
    for field in ["unknown", "functional_approvals", "execution_profile"] {
        let mut forged: Value = serde_json::from_str(&f.v1).unwrap();
        forged[field] = json!({});
        let mut client = rig.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        assert!(matches!(
            selection::retain_in_transaction(&tx, &f.outer(&forged.to_string()), limits).await,
            Err(SubmissionStoreError::Invalid)
        ));
        assert_eq!(
            tx.query_one(
                "SELECT count(*) FROM reborn_component_validator_selections WHERE submission_id=$1",
                &[&f.submission]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
            0
        );
        tx.rollback().await.unwrap();
    }
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let bytes = f.outer(&f.v1);
    selection::retain_in_transaction(&tx, &bytes, limits)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let original = selection::read_preparation_in_transaction(&tx, f.submission, limits)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.bytes(), bytes);
    assert_eq!(original.validator_bytes(), f.v1);
    // Same-ID different bytes may never rewrite an immutable selection.
    assert!(matches!(
        selection::retain_in_transaction(&tx, &f.outer(&f.v3), limits).await,
        Err(SubmissionStoreError::Conflict)
    ));
    assert_eq!(
        selection::read_in_transaction(&tx, f.submission, limits)
            .await
            .unwrap()
            .unwrap(),
        bytes
    );
    tx.rollback().await.unwrap();
}
#[tokio::test]
async fn selection_contract_pg_v3_first_parse_and_original_limits_survive_reduction() {
    let rig = native_pg::NativePostgres::start().await;
    let f = fixture(&rig.pool, true).await;
    assert!(matches!(
        parse_selection_version(&f.v3, REVISION_LIMITS),
        Err(SelectionContractError::Capacity)
    ));
    let limits = preparation_value_limits(&f.preparation).unwrap();
    let bytes = f.outer(&f.v3);
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    selection::retain_in_transaction(&tx, &bytes, limits)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    // A later caller's smaller policy cannot replace the persisted preparation.
    let mut client = rig.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let saved = selection::read_preparation_in_transaction(&tx, f.submission, REVISION_LIMITS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.bytes(), bytes);
    assert_eq!(saved.validator_bytes(), f.v3);
    let original = compile_retained_recipe(
        saved.snapshot(),
        f.root.uuid,
        "selected",
        WorkflowClass::Deterministic,
    )
    .unwrap();
    assert_eq!(
        original
            .retained_selection_with_limits(saved.parsing_limits())
            .unwrap()
            .exact_bytes(),
        f.v1
    );

    assert_eq!(
        saved.snapshot().revisions()[&f.root.uuid].reference(),
        f.root
    );
    tx.rollback().await.unwrap();
}
async fn insert_admission(tx: &tokio_postgres::Transaction<'_>) -> Uuid {
    let run = Uuid::new_v4();
    tx.execute("INSERT INTO brassclaw_monty_task_admissions(run_id,turn_id,scope,accepted_message_ref,runner_id,claim_checksum,admission_key,phase)
        VALUES($1,$2,'{}'::jsonb,'migration-fixture',$3,$4,$5,'reserved')",&[&run,&Uuid::new_v4(),&Uuid::new_v4(),&vec![1u8;32],&Sha256::digest(run.as_bytes()).to_vec()]).await.unwrap();
    run
}
async fn insert_workflow_row(
    tx: &tokio_postgres::Transaction<'_>,
    root: ComponentRevisionRef,
    bytes: &str,
) -> Result<Uuid, tokio_postgres::Error> {
    let id = Uuid::new_v4();
    let hash = digest(bytes);
    let evidence=json!({"format":"workflow-review-evidence/1","evidence_id":id,"kind":"q1","selection_checksum":hash}).to_string();
    tx.execute("INSERT INTO reborn_workflow_review_evidence(evidence_id,recipe_id,recipe_version,selection_bytes,selection_checksum,evidence_kind,evidence_bytes,checksum)
        VALUES($1,$2,$3,$4,$5,'q1',$6,$7)",&[&id,&root.uuid,&(root.version as i64),&bytes,&hash,&evidence,&digest(&evidence)]).await?;
    Ok(id)
}
#[tokio::test]
async fn selection_contract_pg_v215_upgrade_preserves_v1_bytes_and_integrity_guards() {
    let rig = native_pg::NativePostgres::start().await;
    let f = fixture(&rig.pool, false).await;
    let mut client = rig.pool.get().await.unwrap();
    let tx = client.transaction().await.unwrap();
    // Reconstruct only the genuine predecessor format constraints on the actual
    // migrated tables. All FKs, checksum and immutable-record triggers stay live.
    tx.batch_execute("ALTER TABLE brassclaw_monty_recipe_selections DROP CONSTRAINT brassclaw_monty_recipe_selections_format_v1_or_v3;
        ALTER TABLE reborn_workflow_review_evidence DROP CONSTRAINT reborn_workflow_review_evidence_format_v1_or_v3;
        ALTER TABLE brassclaw_monty_recipe_selections ADD CHECK ((selection_bytes::jsonb->>'format'='monty-retained-recipe-selection/1') IS TRUE);
        ALTER TABLE reborn_workflow_review_evidence ADD CHECK ((selection_bytes::jsonb->>'format'='monty-retained-recipe-selection/1') IS TRUE);").await.unwrap();
    let run = insert_admission(&tx).await;
    tx.execute("INSERT INTO brassclaw_monty_recipe_selections(run_id,recipe_id,selection_bytes,selection_checksum) VALUES($1,$2,$3,$4)",
        &[&run,&f.root.uuid,&f.v1,&digest(&f.v1)]).await.unwrap();
    let evidence = insert_workflow_row(&tx, f.root, &f.v1).await.unwrap();
    tx.batch_execute(include_str!(
        "../../brassclaw_pg/migrations/V215__retained_selection_v3_format.sql"
    ))
    .await
    .unwrap();
    assert_eq!(tx.query_one("SELECT selection_bytes,selection_checksum FROM brassclaw_monty_recipe_selections WHERE run_id=$1",&[&run]).await.unwrap().get::<_,String>(0),f.v1);
    assert_eq!(
        tx.query_one(
            "SELECT selection_bytes FROM reborn_workflow_review_evidence WHERE evidence_id=$1",
            &[&evidence]
        )
        .await
        .unwrap()
        .get::<_, String>(0),
        f.v1
    );
    let original_row=tx.query_one("SELECT selection_bytes,selection_checksum FROM brassclaw_monty_recipe_selections WHERE run_id=$1",&[&run]).await.unwrap();
    assert_eq!(original_row.get::<_, String>(0), f.v1);
    assert_eq!(original_row.get::<_, String>(1), digest(&f.v1));
    let checksum_run = insert_admission(&tx).await;
    tx.batch_execute("SAVEPOINT invalid_checksum")
        .await
        .unwrap();
    let bad_hash = "0".repeat(64);
    let error=tx.execute("INSERT INTO brassclaw_monty_recipe_selections(run_id,recipe_id,selection_bytes,selection_checksum) VALUES($1,$2,$3,$4)",
        &[&checksum_run,&f.root.uuid,&f.v3,&bad_hash]).await.unwrap_err();
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    tx.batch_execute("ROLLBACK TO SAVEPOINT invalid_checksum")
        .await
        .unwrap();
    // Identical rows/bytes with the actual digest establish that only the hash
    // differed in the preceding negative, with all foreign keys still active.
    tx.execute("INSERT INTO brassclaw_monty_recipe_selections(run_id,recipe_id,selection_bytes,selection_checksum) VALUES($1,$2,$3,$4)",
        &[&checksum_run,&f.root.uuid,&f.v3,&digest(&f.v3)]).await.unwrap();
    let newer = insert_admission(&tx).await;
    tx.execute("INSERT INTO brassclaw_monty_recipe_selections(run_id,recipe_id,selection_bytes,selection_checksum) VALUES($1,$2,$3,$4)",
        &[&newer,&f.root.uuid,&f.v3,&digest(&f.v3)]).await.unwrap();
    insert_workflow_row(&tx, f.root, &f.v3).await.unwrap();
    let mut unknown: Value = serde_json::from_str(&f.v3).unwrap();
    unknown["format"] = json!("monty-retained-recipe-selection/999");
    let unknown = unknown.to_string();
    for table in 0..2 {
        tx.batch_execute("SAVEPOINT invalid_format").await.unwrap();
        let failure = if table == 0 {
            let extra = insert_admission(&tx).await;
            tx.execute("INSERT INTO brassclaw_monty_recipe_selections(run_id,recipe_id,selection_bytes,selection_checksum) VALUES($1,$2,$3,$4)",
                &[&extra,&f.root.uuid,&unknown,&digest(&unknown)]).await.unwrap_err()
        } else {
            insert_workflow_row(&tx, f.root, &unknown)
                .await
                .unwrap_err()
        };
        assert_eq!(
            failure.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
        let expected = if table == 0 {
            "brassclaw_monty_recipe_selections_format_v1_or_v3"
        } else {
            "reborn_workflow_review_evidence_format_v1_or_v3"
        };
        assert_eq!(failure.as_db_error().unwrap().constraint(), Some(expected));
        tx.batch_execute("ROLLBACK TO SAVEPOINT invalid_format")
            .await
            .unwrap();
    }
    tx.batch_execute("SAVEPOINT immutable_original")
        .await
        .unwrap();
    let err=tx.execute("UPDATE brassclaw_monty_recipe_selections SET selection_bytes=$2,selection_checksum=$3 WHERE run_id=$1",&[&run,&f.v3,&digest(&f.v3)]).await.unwrap_err();
    assert_eq!(
        err.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    assert!(err.as_db_error().unwrap().message().contains("immutable"));
    tx.batch_execute("ROLLBACK TO SAVEPOINT immutable_original")
        .await
        .unwrap();
    tx.batch_execute("SAVEPOINT reserved_q_gate;
        ALTER TABLE brassclaw_monty_recipe_selections DROP CONSTRAINT brassclaw_monty_recipe_selections_format_v1_or_v3;
        ALTER TABLE brassclaw_monty_recipe_selections ADD CONSTRAINT q_reserved_v2_guard CHECK ((
            selection_bytes::jsonb->>'format'='monty-retained-recipe-selection/1' OR
            (selection_bytes::jsonb->>'format'='monty-retained-recipe-selection/2' AND
             selection_bytes::jsonb->>'execution_profile' IN ('flat_stop/1','structured_sequential_stop/1'))
        ) IS TRUE) NOT VALID;").await.unwrap();
    let guarded = tx
        .batch_execute(include_str!(
            "../../brassclaw_pg/migrations/V215__retained_selection_v3_format.sql"
        ))
        .await
        .unwrap_err();
    assert_eq!(
        guarded.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    assert_eq!(
        guarded.as_db_error().unwrap().message(),
        "retained selection /2 requires coordinated format adoption"
    );
    tx.batch_execute("ROLLBACK TO SAVEPOINT reserved_q_gate")
        .await
        .unwrap();
    tx.rollback().await.unwrap();
}
