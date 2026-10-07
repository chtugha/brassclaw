//! Real immutable draft graph and IBS for behavioral validation only.
use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
fn draft(
    id: Uuid,
    class: i32,
    document: Value,
    dependencies: &[Uuid],
    association: Value,
) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,"class_code":class,
        "document":document,"dependencies":dependencies,"association":association})
        .to_string(),
    )
    .unwrap()
}
pub async fn program(
    store: &PgComponentRevisionStore,
    invalid_output: bool,
) -> Arc<RetainedToolProgram> {
    program_with_source(store, invalid_output, None).await
}
pub async fn program_with_source(
    store: &PgComponentRevisionStore,
    invalid_output: bool,
    source: Option<&str>,
) -> Arc<RetainedToolProgram> {
    let (root, code, descriptor, tool, skill) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let inputs = json!({"data":{"type":"string","required":true,"checks":[]}});
    let result = json!({"type":"object","allow_extra_fields":false,"fields":{"text":{"type":"string","required":true}}});
    let association = json!({"format":"skill-association/1","skill_uuid":skill,"python_code_uuid":code,"tool_skill_uuid":descriptor,"tool_uuid":tool,
        "callable":"host.json","inputs":inputs,"arguments":{"data":"data"},"code_arguments":{"operation":{"type":"string","checks":[],"depends_on":[],"meaning":"Fixed parse operation for this usage"}},"result":result,
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}}).to_string();
    let body = source.unwrap_or(if invalid_output {
        "value = host.json(operation='parse', data=inputs['data'])\nresult = 'invalid output'"
    } else {
        "result = host.json(operation='parse', data=inputs['data'])"
    });
    let recipe = json!({"variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["parse %"],"variable_patterns":[{"name":"data","pattern":null,"description":null}]}],
        "step_descriptions":[{"desc_idx":0,"label":"two explicit usages","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"rust","goal":"Bind JSON","content":"","type":"component","include":[descriptor],"tool_bindings":[{"tool_id":tool,"tool_name":"json","params":{},"error_policy":{"policy":"fail"}}]},
            {"stepnumber":2,"knowledge":"orchestrator","goal":"Parse JSON","content":"","type":"component","include":[code]},
            {"stepnumber":3,"knowledge":"rust","goal":"Bind JSON","content":"","type":"component","include":[descriptor],"tool_bindings":[{"tool_id":tool,"tool_name":"json","params":{},"error_policy":{"policy":"fail"}}]},
            {"stepnumber":4,"knowledge":"orchestrator","goal":"Parse JSON again","content":"","type":"component","include":[code]}]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":{
            "0:2":{"data":{"kind":"task_input","reference":"{{vars.data}}"}},"0:4":{"data":{"kind":"task_input","reference":"{{vars.data}}"}}}}}});
    let mut refs = Vec::new();
    for d in [
        draft(
            tool,
            0,
            json!({"capability_id":"builtin.json","callable":"host.json","input_contract":{
            "operation":{"type":"string","required":true,"checks":[]},"data":{"type":"string","required":true,"checks":[]}}}),
            &[],
            Value::Null,
        ),
        draft(
            descriptor,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":tool,"callable":"host.json","capability_id":"builtin.json"}}),
            &[tool],
            Value::Null,
        ),
        draft(
            code,
            22,
            json!({"content":body,"input_contract":inputs,"result_contract":result}),
            &[],
            Value::Null,
        ),
        draft(
            skill,
            1,
            json!({"body":"Parse the supplied JSON text and return its object."}),
            &[code, descriptor, tool],
            json!(association),
        ),
        draft(root, 21, recipe, &[descriptor, code, skill], Value::Null),
    ] {
        refs.push(store.stage(&d, 0).await.unwrap());
    }
    let retained = Arc::new(store.read_exact(&[root], &refs).await.unwrap());
    Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(retained, root, "selected", WorkflowClass::Deterministic)
                .unwrap(),
        )
        .unwrap(),
    )
}
