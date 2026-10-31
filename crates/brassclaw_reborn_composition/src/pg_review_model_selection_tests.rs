//! Actual immutable subject -> IBS -> PostgreSQL preparation -> IBS restoration.
//! The small private workflow reuses real presence/collection/aggregate exports.
//! Preparation is not execution, a complete class validator, Q1 or Free approval.
use std::{collections::BTreeMap, sync::Arc};

use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
        retained_validator::RetainedValidatorCoverage,
    },
};
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS},
    review_submission_store::{
        PgReviewSubmissionStore, ReviewSubmissionDraft, SubmissionStoreError, selection,
    },
    revision_store::PgComponentRevisionStore,
    validator_components::{aggregate_draft, collect_draft, structural_presence_draft},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

pub(super) fn recipe(root: Uuid, rule: Uuid, collection: Uuid, aggregation: Uuid) -> Value {
    let layout = json!({"format":"recipe-input-layout/1",
    "task_inputs":{"subject_json":{"type":"string","required":true,"checks":[]}},
    "steps":{
        "0:1":{"name":{"kind":"constant","value":"preparation only"},
            "description":{"kind":"constant","value":"Presence only, no class verdict"},
            "content":{"kind":"task_input","reference":"{{vars.subject_json}}"}},
        "0:2":{"structural_presence":{"kind":"result","step_id":"0:1","path":[]}},
        "0:3":{"required_rule_ids":{"kind":"constant","value":["structural_presence"]},
            "checks":{"kind":"result","step_id":"0:2","path":[]}}
    }});
    let invocations = json!({"0:1":"check_structural_presence",
        "0:2":"collect_validator_checks","0:3":"aggregate_validator_checks"});
    json!({"format":"component-revision/1","uuid":root,"class_code":21,
        "document":{"name":"private preparation fixture",
            "description":"Unapproved partial presence workflow; no execution or Q1 claimed",
            "validates_class_code":22,"consumer_tags":["05:validator"],
            "validator_coverage":{"format":"validator-coverage/1",
                "rules":[{"rule_id":"structural_presence","step_id":"0:1"}],
                "aggregate_step":"0:3"},
            "variants":[
                {"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]},
                {"variant_key":"other","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}
            ],
            "step_descriptions":[{"desc_idx":0,"label":"Partial review only","yaml_source":"",
                "steps":[
                    {"stepnumber":1,"knowledge":"orchestrator","goal":"Check structural presence only",
                        "content":"","type":"component","include":[rule]},
                    {"stepnumber":2,"knowledge":"orchestrator","goal":"Collect the real rule result",
                        "content":"","type":"component","include":[collection]},
                    {"stepnumber":3,"knowledge":"orchestrator","goal":"Aggregate declared coverage only",
                        "content":"","type":"component","include":[aggregation]}
                ]}],
            "input_layouts":{"selected":layout,"other":layout},
            "invocations":{"selected":invocations,"other":invocations}},
        "dependencies":[rule,collection,aggregation],"association":null})
}

/// Rewrite the nested selection AND its checksum so a rejection exercises the
/// actual original-document binding, not merely a broken outer digest.
fn rewrite_selection(bytes: &str, edit: impl FnOnce(&mut Value)) -> String {
    let mut envelope: Value = serde_json::from_str(bytes).unwrap();
    let mut selected: Value =
        serde_json::from_str(envelope["validator_selection_bytes"].as_str().unwrap()).unwrap();
    edit(&mut selected);
    let selected = selected.to_string();
    envelope["validator_selection_checksum"] =
        json!(hex::encode(Sha256::digest(selected.as_bytes())));
    envelope["validator_selection_bytes"] = json!(selected);
    envelope.to_string()
}

#[tokio::test]
async fn native_validator_preparation_pins_model_input_and_rejects_rewritten_bindings() {
    let database = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let (root, rule, collection, aggregation) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let helpers = [
        structural_presence_draft(rule).unwrap(),
        collect_draft(collection, &["structural_presence".into()]).unwrap(),
        aggregate_draft(aggregation).unwrap(),
    ];
    let mut references = Vec::new();
    for draft in &helpers {
        references.push(store.retain_packaged_draft(draft).await.unwrap());
    }
    let cases = [
        (None, None),
        (
            Some(json!({"selected":{"selection":"default"}})),
            Some(json!({"selection":"default"})),
        ),
        (
            Some(json!({"selected":{"selection":"selected","profile":"review-prefix-a"}})),
            Some(json!({"selection":"selected","profile":"review-prefix-a"})),
        ),
        (
            Some(json!({"selected":{"selection":"selected","profile":"review-prefix-b"}})),
            Some(json!({"selection":"selected","profile":"review-prefix-b"})),
        ),
        (
            Some(json!({"other":{"selection":"selected","profile":"other-variant-only"}})),
            Some(json!({"selection":"default"})),
        ),
        (
            Some(
                json!({"selected":{"selection":"prefix_free_workflow","purpose":"basic_provider_configuration_values"}}),
            ),
            Some(
                json!({"selection":"prefix_free_workflow","purpose":"basic_provider_configuration_values"}),
            ),
        ),
    ];
    let mut previous_version = 0;
    let mut originals = BTreeMap::new();
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    for (declaration, expected) in cases {
        let mut document = recipe(root, rule, collection, aggregation);
        if let Some(declaration) = declaration {
            document["document"]["model_input_bindings"] =
                json!({"format":"recipe-model-input-bindings/1","variants":declaration});
        }
        let draft = ComponentRevisionDraft::from_json(&document.to_string()).unwrap();
        let root_ref = store.stage(&draft, previous_version).await.unwrap();
        previous_version = root_ref.version;
        let mut refs = references.clone();
        refs.push(root_ref);
        let snapshot = Arc::new(store.read_exact(&[root], &refs).await.unwrap());
        let instruction =
            compile_retained_recipe(snapshot, root, "selected", WorkflowClass::RequiresModel)
                .unwrap();
        let inspected = InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(Arc::new(
                prepare_retained_tool_program(instruction).unwrap(),
            )),
            &worker,
        )
        .await
        .unwrap();
        let original = inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .unwrap();
        let actual: Value = serde_json::from_str(original.exact_bytes()).unwrap();
        let expected = expected
            .map(|binding| json!({"format":"recipe-model-input-selection/1","binding":binding}));
        assert_eq!(actual.get("model_input_binding"), expected.as_ref());

        let submission = Uuid::new_v4();
        // Real immutable source subject; no candidate or validator is activated.
        PgReviewSubmissionStore::new(database.pool.clone())
            .submit(
                &ReviewSubmissionDraft::new(
                    submission,
                    "native-review-preparation",
                    structural_presence_draft(Uuid::new_v4()).unwrap(),
                    None,
                    vec![],
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let mut client = database.pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        let subject = PgReviewSubmissionStore::read_validator_subject_in_transaction(
            &tx,
            submission,
            REVISION_LIMITS,
        )
        .await
        .unwrap()
        .unwrap();
        let bytes =
            RetainedValidatorCoverage::selection_bytes(&subject, &inspected, REVISION_LIMITS)
                .unwrap();
        selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS)
            .await
            .unwrap();
        selection::retain_in_transaction(&tx, &bytes, REVISION_LIMITS)
            .await
            .unwrap();
        let saved = selection::read_preparation_in_transaction(&tx, submission, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.bytes(), bytes);
        let restored = RetainedValidatorCoverage::restore_preparation(&saved).unwrap();
        assert_eq!(
            restored
                .inputs()
                .instruction()
                .retained_selection()
                .unwrap()
                .exact_bytes(),
            original.exact_bytes()
        );

        let mut invalid = vec![
            rewrite_selection(&bytes, |s| {
                s["model_input_binding"] = Value::Null;
            }),
            rewrite_selection(&bytes, |s| {
                s["model_input_binding"] = json!({"format":"recipe-model-input-selection/2","binding":{"selection":"default"}});
            }),
            rewrite_selection(&bytes, |s| {
                s["model_input_binding"] = json!({"format":"recipe-model-input-selection/1","binding":{"selection":"selected","profile":"different-profile"}});
            }),
            rewrite_selection(&bytes, |s| {
                s["model_input_binding"] = json!({"format":"recipe-model-input-selection/1","binding":{"selection":"prefix_free_workflow","purpose":"prefix_generation"}});
            }),
            rewrite_selection(&bytes, |s| {
                s["unknown_future_binding"] = json!({});
            }),
        ];
        if expected.is_some() {
            invalid.push(rewrite_selection(&bytes, |s| {
                s.as_object_mut().unwrap().remove("model_input_binding");
            }));
            invalid.push(rewrite_selection(&bytes, |s| {
                s["model_input_binding"]["extra"] = json!(true);
            }));
            invalid.push(rewrite_selection(&bytes, |s| {
                s["model_input_binding"]["binding"]["extra"] = json!(true);
            }));
        } else {
            invalid.push(rewrite_selection(&bytes, |s| { s["model_input_binding"] = json!({"format":"recipe-model-input-selection/1","binding":{"selection":"default"}}); }));
        }
        for changed in invalid {
            assert!(
                matches!(
                    selection::retain_in_transaction(&tx, &changed, REVISION_LIMITS).await,
                    Err(SubmissionStoreError::Invalid)
                ),
                "forged binding was retained"
            );
        }
        assert_eq!(
            selection::read_in_transaction(&tx, submission, REVISION_LIMITS)
                .await
                .unwrap()
                .unwrap(),
            bytes
        );
        tx.commit().await.unwrap();
        originals.insert(submission, (bytes, original.exact_bytes().to_owned()));
    }
    // Later Recipe revisions/profile choices never replace the first selection.
    let mut client = database.pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    for (submission, (bytes, selected)) in originals {
        let saved = selection::read_preparation_in_transaction(&tx, submission, REVISION_LIMITS)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.bytes(), bytes);
        assert_eq!(
            RetainedValidatorCoverage::restore_preparation(&saved)
                .unwrap()
                .inputs()
                .instruction()
                .retained_selection()
                .unwrap()
                .exact_bytes(),
            selected
        );
    }
    let counts = tx
        .query_one(
            "SELECT (SELECT count(*) FROM reborn_component_review_evidence),
        (SELECT count(*) FROM reborn_skill_association_approvals),
        (SELECT count(*) FROM brassclaw_monty_task_admissions),
        (SELECT count(*) FROM brassclaw_monty_tool_invocations)",
            &[],
        )
        .await
        .unwrap();
    for column in 0_usize..4 {
        assert_eq!(counts.get::<_, i64>(column), 0);
    }
    tx.commit().await.unwrap();
}
