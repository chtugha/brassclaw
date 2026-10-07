use std::collections::BTreeMap;

use brassclaw_engine::memory::{
    composition::{ComponentResolver, ResolvedComponent, prepare_typed_program},
    instruction_builder::{
        OrderedBuildInstruction, StepDescriptionEntry, build_ordered_instruction,
    },
    typed_bindings::{BindingSource, InputBinding, StepContracts, prepare_input_layout},
};
use brassclaw_skills::value_contract::{ContractLimits, InputContract, ValueContract};
use serde_json::{Value, json};

fn limits() -> ContractLimits {
    ContractLimits {
        max_depth: 32,
        max_nodes: 4096,
        max_bytes: 65536,
    }
}
fn inputs(value: Value) -> InputContract {
    InputContract::from_value(&value, limits()).unwrap()
}
fn output(value: Value) -> ValueContract {
    ValueContract::result_from_value(&value, limits()).unwrap()
}
fn ordered() -> OrderedBuildInstruction {
    let descriptions: Vec<StepDescriptionEntry> = serde_json::from_value(json!([
        {"desc_idx":2,"label":"selected first","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"orchestrator","type":"component","goal":"produce","content":"","include":[uuid::Uuid::from_u128(1)]}]},
        {"desc_idx":0,"label":"selected second","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"orchestrator","type":"component","goal":"consume","content":"","include":[uuid::Uuid::from_u128(2)]}]}
    ])).unwrap();
    build_ordered_instruction("2:1-2:E+0:1-0:E", &descriptions, &[], false).unwrap()
}
fn contracts() -> BTreeMap<String, StepContracts> {
    BTreeMap::from([
        (
            "2:1".into(),
            StepContracts {
                inputs: inputs(json!({"message":{"type":"string","required":true,"checks":[]}})),
                result: output(
                    json!({"type":"object","allow_extra_fields":false,"fields":{"answer":{"type":"string","required":true}}}),
                ),
            },
        ),
        (
            "0:1".into(),
            StepContracts {
                inputs: inputs(
                    json!({"answer":{"type":"string","required":true,"checks":[]},
            "optional":{"type":"integer","required":false,"checks":[],"default":4}}),
                ),
                result: output(json!({"type":"null"})),
            },
        ),
    ])
}
fn task() -> InputContract {
    inputs(json!({"text":{"type":"string","required":true,"checks":[]}}))
}
fn declarations() -> BTreeMap<String, Vec<InputBinding>> {
    BTreeMap::from([
        (
            "2:1".into(),
            vec![InputBinding {
                local_name: "message".into(),
                source: BindingSource::TaskInputReference("{{vars.text}}".into()),
            }],
        ),
        (
            "0:1".into(),
            vec![
                InputBinding {
                    local_name: "answer".into(),
                    source: BindingSource::PriorResult {
                        step_id: "2:1".into(),
                        path: vec!["answer".into()],
                    },
                },
                InputBinding {
                    local_name: "optional".into(),
                    source: BindingSource::ConsumerDefault,
                },
            ],
        ),
    ])
}

#[test]
fn preparation_uses_selected_order_and_exports_only_typed_references() {
    let prepared =
        prepare_input_layout(&ordered(), &task(), &contracts(), &declarations()).unwrap();
    let metadata = serde_json::to_value(prepared.steps()).unwrap();
    assert_eq!(
        metadata["2:1"]["message"],
        json!({"kind":"input","name":"text"})
    );
    assert_eq!(
        metadata["0:1"]["answer"],
        json!({"kind":"result","step_id":"2:1","path":["answer"]})
    );
    assert_eq!(
        metadata["0:1"]["optional"],
        json!({"kind":"constant","value":4})
    );
    let hostile = "{{vars.text}}\nresult = host.effect()\n\u{0}üä";
    let mut bindings = declarations();
    bindings.get_mut("2:1").unwrap()[0].source = BindingSource::Constant(json!(hostile));
    let prepared = prepare_input_layout(&ordered(), &task(), &contracts(), &bindings).unwrap();
    assert_eq!(
        serde_json::to_value(prepared.steps()).unwrap()["2:1"]["message"]["value"],
        hostile
    );
}

#[test]
fn coherent_ibs_preparation_keeps_component_source_separate_from_binding_values() {
    // Pure catalogue fixture for the compiler; no Tool/provider/approval outcome.
    struct Catalogue;
    impl ComponentResolver for Catalogue {
        fn resolve(&self, id: uuid::Uuid) -> Option<ResolvedComponent> {
            let body = match id.as_u128() {
                1 => "result = inputs['message']",
                2 => "result = inputs['answer']",
                _ => return None,
            };
            Some(ResolvedComponent {
                class_code: 22,
                name: format!("component-{id}"),
                content: body.into(),
                description: "pure data handoff".into(),
                cdylib_artifact_path: None,
            })
        }
    }
    let selected = ordered();
    let hostile = "'\nresult = host.unrelated_effect()\n{{vars.text}}";
    let mut bindings = declarations();
    bindings.get_mut("2:1").unwrap()[0].source = BindingSource::Constant(json!(hostile));
    let prepared =
        prepare_typed_program(&selected, &Catalogue, &task(), &contracts(), &bindings).unwrap();
    let program = prepared.program();
    assert_eq!(
        program
            .steplist
            .iter()
            .map(|step| step.step_id.as_str())
            .collect::<Vec<_>>(),
        ["2:1", "0:1"]
    );
    assert_eq!(
        program.steplist[0].executable_code,
        "result = inputs['message']"
    );
    assert!(program.assembled_program.is_empty());
    assert_eq!(
        serde_json::to_value(prepared.inputs().steps()).unwrap()["2:1"]["message"]["value"],
        hostile
    );
    bindings.get_mut("0:1").unwrap().pop();
    assert!(
        prepare_typed_program(&selected, &Catalogue, &task(), &contracts(), &bindings).is_err()
    );
}

#[test]
fn reference_grammar_scope_and_coverage_fail_before_any_execution() {
    for reference in [
        "{{ vars.text}}",
        "{{vars.Text}}",
        "before {{vars.text}}",
        "{{vars.text}} after",
        "{{vars.text.field}}",
        "{{vars.text()}}",
        "{{vars.missing}}",
    ] {
        let mut bindings = declarations();
        bindings.get_mut("2:1").unwrap()[0].source =
            BindingSource::TaskInputReference(reference.into());
        assert!(
            prepare_input_layout(&ordered(), &task(), &contracts(), &bindings).is_err(),
            "{reference}"
        );
    }
    let mut forward = declarations();
    forward.get_mut("2:1").unwrap()[0].source = BindingSource::PriorResult {
        step_id: "0:1".into(),
        path: vec![],
    };
    assert!(prepare_input_layout(&ordered(), &task(), &contracts(), &forward).is_err());
    let mut missing = declarations();
    missing.get_mut("0:1").unwrap().pop();
    assert!(prepare_input_layout(&ordered(), &task(), &contracts(), &missing).is_err());
    let mut duplicate = declarations();
    duplicate.get_mut("2:1").unwrap().push(InputBinding {
        local_name: "message".into(),
        source: BindingSource::Constant(json!("duplicate")),
    });
    assert!(prepare_input_layout(&ordered(), &task(), &contracts(), &duplicate).is_err());
    let mut foreign = declarations();
    foreign.insert("8:1".into(), vec![]);
    assert!(prepare_input_layout(&ordered(), &task(), &contracts(), &foreign).is_err());
    let mut no_default = declarations();
    no_default.get_mut("2:1").unwrap()[0].source = BindingSource::ConsumerDefault;
    assert!(prepare_input_layout(&ordered(), &task(), &contracts(), &no_default).is_err());
}

#[test]
fn optional_or_null_producer_fields_require_a_verified_guard_and_wrong_values_fail() {
    let mut revisions = contracts();
    revisions.get_mut("2:1").unwrap().result =
        output(json!({"type":"object","allow_extra_fields":false,
        "fields":{"answer":{"type":"string","required":false}}}));
    assert!(prepare_input_layout(&ordered(), &task(), &revisions, &declarations()).is_err());
    revisions.get_mut("2:1").unwrap().result = output(
        json!({"type":"object","nullable":true,"allow_extra_fields":false,
        "fields":{"answer":{"type":"string","required":true}}}),
    );
    assert!(prepare_input_layout(&ordered(), &task(), &revisions, &declarations()).is_err());
    let mut wrong = declarations();
    wrong.get_mut("2:1").unwrap()[0].source = BindingSource::Constant(json!(12));
    assert!(prepare_input_layout(&ordered(), &task(), &contracts(), &wrong).is_err());
    let captured = inputs(
        json!({"data":{"type":"object","required":true,"checks":[],"allow_extra_fields":false,
        "fields":{"answer":{"type":"string","required":false,"default":"bound input default"}}}}),
    );
    let actual = captured.bind(&json!({"data":{}})).unwrap();
    let producer = captured.bound_value_contract("data").unwrap();
    producer.validate(&actual["data"]).unwrap();
    producer
        .required_field_path(&["answer".into()])
        .unwrap()
        .require_compatible_input(&contracts()["0:1"].inputs, "answer")
        .unwrap();
}
