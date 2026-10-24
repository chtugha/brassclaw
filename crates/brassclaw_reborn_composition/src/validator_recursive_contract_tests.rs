//! Actual pure Monty rule compared with the existing native contract readers.
//! This is grammar acceptance, not complete class Q1 or an approval writer.
use std::{collections::BTreeMap, sync::Arc, time::Duration};

use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_inputs::prepare_retained_unbound_program,
        retained_instruction::{WorkflowClass, compile_retained_recipe},
    },
};
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RetainedComponentSnapshot},
    global_bootstrap_components::packaged_component_id,
    revision_store::PgComponentRevisionStore,
    validator_components::{RECURSIVE_CONTRACT_SOURCE, recursive_contract_draft},
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use uuid::Uuid;

#[tokio::test]
async fn native_recursive_contract_rule_matches_real_input_parameter_and_output_readers() {
    let draft = recursive_contract_draft(Uuid::new_v4()).unwrap();
    let recipe = Uuid::new_v4();
    let recipe_draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":recipe,"class_code":21,
        "document":{
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
            "step_descriptions":[{"desc_idx":0,"label":"Private contract check","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"orchestrator","goal":"Check one complete declared contract","content":"","type":"component","include":[draft.uuid()]}
            ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1",
                "task_inputs":draft.document()["input_contract"],"steps":{"0:1":{
                    "contract_json":{"kind":"task_input","reference":"{{vars.contract_json}}"},
                    "position":{"kind":"task_input","reference":"{{vars.position}}"}
                }}}}
        },"dependencies":[draft.uuid()],"association":null
    }).to_string()).unwrap();
    let snapshot = Arc::new(
        RetainedComponentSnapshot::new(
            &[recipe],
            vec![
                recipe_draft.at_version(1).unwrap(),
                draft.clone().at_version(1).unwrap(),
            ],
        )
        .unwrap(),
    );
    let program = prepare_retained_unbound_program(
        compile_retained_recipe(snapshot, recipe, "selected", WorkflowClass::Deterministic)
            .unwrap(),
    )
    .unwrap();
    let worker = installed_worker().unwrap();
    let inspected =
        InspectedRetainedProgram::inspect(RetainedProgram::Unbound(Arc::new(program)), &worker)
            .await
            .unwrap();
    assert!(inspected.pure_component(draft.uuid()).is_ok());
    assert_eq!(
        inspected.invocation("0:1"),
        Some("result = check_recursive_contract(inputs=inputs)")
    );
    let inputs =
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS).unwrap();
    let output =
        ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)
            .unwrap();

    let mut schemas = vec![
        json!({"type":"string","required":true,"checks":[]}),
        json!({"type":"string","required":false,"checks":[]}),
        json!({"type":"integer","required":false,"checks":[],"default":null}),
        json!({"type":"integer","required":false,"checks":[],"nullable":true,"default":null}),
        json!({"type":"integer","required":true,"checks":[],"default":2}),
        json!({"type":"integer","required":"true","checks":[]}),
        json!({"type":"integer","required":true,"checks":[],"nullable":0}),
        json!({"type":"any","required":true,"checks":[]}),
        json!({"type":"string","required":true,"checks":[],"enum":["x"]}),
        json!({"type":"integer","required":true}),
        json!({"type":"integer","required":true,"checks":{}}),
        json!({"type":"integer","required":true,"checks":[{"kind":"min","value":true}]}),
        json!({"type":"integer","required":true,"checks":[{"kind":"min","value":0.0}]}),
        json!({"type":"string","required":true,"checks":[{"kind":"min","value":0}]}),
        json!({"type":"boolean","required":true,"checks":[{"kind":"min","value":0}]}),
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":-1}]}),
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1.0}]}),
        json!({"type":"string","required":false,"checks":[{"kind":"min_length","value":2}],"default":"Ü🙂"}),
        json!({"type":"string","required":false,"checks":[{"kind":"min_length","value":2}],"default":"Ü"}),
        json!({"type":"integer","required":true,"checks":[],"depends_on":[],"meaning":"computed is a separate profile"}),
        json!({"type":"list","required":true,"checks":[]}),
        json!({"type":"list","required":true,"checks":[],"items":{"type":"string"}}),
        json!({"type":"list","required":true,"checks":[],"items":{"type":"string","required":true}}),
        json!({"type":"object","required":true,"checks":[],"allow_extra_fields":false}),
        json!({"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":false,"extra_fields":{"type":"string"}}),
        json!({"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":true}),
        json!({"type":"object","required":true,"checks":[],"fields":{},"allow_extra_fields":true,"extra_fields":{"type":"string"}}),
        json!({"type":"object","required":true,"checks":[],"fields":{"":{"type":"string","required":true}},"allow_extra_fields":false}),
        json!({"type":"object","required":false,"checks":[],"fields":{"name":{"type":"string","required":true}},"allow_extra_fields":false,"default":{}}),
        json!({"type":"object","required":false,"checks":[],"fields":{"name":{"type":"string","required":false,"default":"exact"}},"allow_extra_fields":false,"default":{}}),
        json!({"type":"object","required":false,"checks":[],"fields":{"parent":{"type":"object","required":false,"fields":{"child":{"type":"integer","required":true}},"allow_extra_fields":false}},"allow_extra_fields":false,"default":{}}),
        json!({"type":"object","required":false,"checks":[],"fields":{},"allow_extra_fields":false,"default":{"unknown":"data"}}),
        json!({"type":"null","required":false,"checks":[],"default":null}),
        json!({"type":"null","required":true,"checks":[],"nullable":true}),
    ];
    for (lower, upper) in [
        (json!(0), json!(0.1)),
        (json!(0.1), json!(0)),
        (json!(-0.1), json!(0)),
        (json!(0), json!(-0.1)),
        (json!(-0.0), json!(0.0)),
        (json!(1e-100), json!(0)),
        (
            json!(9_007_199_254_740_993_u64),
            json!(9_007_199_254_740_992.0),
        ),
        (
            json!(9_007_199_254_740_992.0),
            json!(9_007_199_254_740_993_u64),
        ),
        (json!(u64::MAX), json!(u64::MAX - 1)),
        (json!(i64::MIN), json!(i64::MIN + 1)),
        (json!(1e20), json!(u64::MAX)),
        (json!(-1e100), json!(-1e20)),
    ] {
        schemas.push(json!({"type":"number","required":true,"checks":[
            {"kind":"min","value":lower},{"kind":"max","value":upper}
        ]}));
    }
    let mut cases: Vec<(String, Value)> =
        vec![("input".into(), json!({})), ("input".into(), Value::Null)];
    for schema in &schemas {
        for mode in ["input", "parameters"] {
            cases.push((mode.into(), json!({"value":schema})));
        }
        // A result has no top-level presence fields; nested placement is still
        // checked and defaults are forbidden throughout the output tree.
        let mut result = schema.clone();
        result.as_object_mut().unwrap().remove("required");
        cases.push(("result".into(), result));
    }
    for target in [
        json!({"type":"integer","required":true,"checks":[]}),
        json!({"type":"number","required":true,"checks":[],"nullable":true}),
        json!({"type":"string","required":true,"checks":[]}),
    ] {
        cases.push(("input".into(),json!({
            "value":{"type":"integer","required":true,"checks":[{"kind":"max_field","input":"ceiling"}]},
            "ceiling":target
        })));
    }
    cases.extend([
        ("input".into(),json!({"Value":{"type":"string","required":true,"checks":[]}})),
        ("input".into(),json!({"value":{"type":"integer","required":true,"checks":[{"kind":"max_field","input":"absent"}]}})),
        ("computed".into(),json!({"type":"integer"})),
    ]);
    cases.push(cases[0].clone());
    assert!(cases.len() > 140);
    let mut expected = Vec::new();
    let mut encoded = Vec::new();
    for (position, contract) in &cases {
        let valid = match position.as_str() {
            "input" => InputContract::from_value(contract, REVISION_LIMITS).is_ok(),
            "parameters" => {
                InputContract::tool_parameters_from_value(contract, REVISION_LIMITS).is_ok()
            }
            "result" => ValueContract::result_from_value(contract, REVISION_LIMITS).is_ok(),
            _ => false,
        };
        expected.push(valid);
        let value = json!({"position":position,"contract_json":contract.to_string()});
        encoded.push(Value::Object(inputs.bind(&value).unwrap()));
    }
    let limits = UtilityLimits::from_execution(Default::default()).unwrap();
    let UtilityOutput::Evaluated {value,stdout} = utility::execute(&worker,UtilityRequest::Evaluate {
        source:format!("{RECURSIVE_CONTRACT_SOURCE}\n[check_recursive_contract(inputs=case) for case in cases]\n"),
        inputs:BTreeMap::from([("cases".into(),json!(encoded))]),bounds:limits.bounds,
        max_compute_time:Duration::from_secs(10),
    },limits.process).await.unwrap() else { panic!("actual native rule results required") };
    assert!(stdout.is_empty());
    let actual = value.as_array().unwrap();
    assert_eq!(actual.len(), cases.len());
    for (index, (actual, wanted)) in actual.iter().zip(expected).enumerate() {
        output.validate(actual).unwrap();
        assert_eq!(actual["rule_id"], "recursive_contract");
        assert_eq!(actual["ok"], wanted, "case {index}: {}", cases[index].1);
        assert_eq!(actual["errors"].as_array().unwrap().is_empty(), wanted);
    }
}

#[tokio::test]
async fn recursive_contract_package_retains_original_without_overwriting_operator_or_approving() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();
    let id = packaged_component_id("validator:recursive-contract");
    let draft = recursive_contract_draft(id).unwrap();
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    assert_eq!(
        store
            .read_revision(id, 1)
            .await
            .unwrap()
            .draft()
            .exact_bytes(),
        draft.exact_bytes()
    );
    let mut successor: Value = serde_json::from_str(draft.exact_bytes()).unwrap();
    successor["document"]["description"] =
        json!("Operator's retained recursive-contract successor");
    let successor = ComponentRevisionDraft::from_json(&successor.to_string()).unwrap();
    let selected = store.stage(&successor, 1).await.unwrap();
    crate::builtin_bootstrap::retain_validator_check_drafts(&rig.pool)
        .await
        .unwrap();
    assert_eq!(store.read_allocation_head(id).await.unwrap(), Some((22, 2)));
    assert_eq!(
        store.read_revision(id, 2).await.unwrap().reference(),
        selected
    );
    assert_eq!(
        store
            .read_revision(id, 1)
            .await
            .unwrap()
            .draft()
            .exact_bytes(),
        draft.exact_bytes()
    );
    let counts=rig.pool.get().await.unwrap().query_one(
        "SELECT (SELECT count(*) FROM reborn_component_review_evidence),(SELECT count(*) FROM reborn_skill_association_approvals),(SELECT count(*) FROM brassclaw_monty_boot_catalogues)",&[],
    ).await.unwrap();
    for column in 0..3 {
        assert_eq!(counts.get::<_, i64>(column), 0);
    }
}
