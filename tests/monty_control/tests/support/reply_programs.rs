//! Immutable draft usages for actual reply/history behavioral validation.
//! These records are not approval evidence or an activated catalogue.
use std::sync::Arc;

use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) async fn program(
    store: &PgComponentRevisionStore,
    history: bool,
) -> Arc<RetainedToolProgram> {
    let (root, code, descriptor, tool, skill, formatter) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let string = json!({"type":"string","required":true,"checks":[]});
    let (callable, capability, name, body, inputs, arguments, fixed, result, prose) = if history {
        (
            "host.memory_write",
            "builtin.memory_write",
            "memory_write",
            "result = host.memory_write(content=inputs['content'], target='daily_log')",
            json!({"content":string}),
            json!({"content":"content"}),
            json!({"target":{"type":"string","checks":[],"depends_on":[],"meaning":"Append to the daily log"}}),
            json!({"type":"object","allow_extra_fields":false,"fields":{
             "status":{"type":"string","required":true},"path":{"type":"string","required":true},
             "append":{"type":"boolean","required":true},"content_length":{"type":"integer","required":true}}}),
            "Append the supplied completed-turn record to the daily memory log.",
        )
    } else {
        (
            "host.post_reply",
            "host.post_reply",
            "post_reply",
            "result = host.post_reply(answer=inputs['answer'])",
            json!({"answer":string}),
            json!({"answer":"answer"}),
            json!({}),
            json!({"type":"string"}),
            "Publish the supplied answer once to this admitted task's transcript.",
        )
    };
    let association =
        json!({"format":"skill-association/1","skill_uuid":skill,"python_code_uuid":code,
        "tool_skill_uuid":descriptor,"tool_uuid":tool,"callable":callable,"inputs":inputs,
        "arguments":arguments,"code_arguments":fixed,"result":result,
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]}})
        .to_string();
    let mut tool_inputs = inputs.clone();
    if history {
        tool_inputs["target"] = string.clone();
    }
    let task_inputs = if history {
        json!({"user_input":string,"answer":string,"reply_ref":string})
    } else {
        inputs.clone()
    };
    let offset = i32::from(history);
    let mut steps = Vec::new();
    let mut layouts = serde_json::Map::new();
    if history {
        steps.push(
            json!({"stepnumber":1,"knowledge":"orchestrator","goal":"Format the completed turn",
            "content":"","type":"component","include":[formatter]}),
        );
        layouts.insert(
            "0:1".into(),
            json!({
            "user_input":{"kind":"task_input","reference":"{{vars.user_input}}"},
            "answer":{"kind":"task_input","reference":"{{vars.answer}}"},
            "reply_ref":{"kind":"task_input","reference":"{{vars.reply_ref}}"}}),
        );
        layouts.insert(
            "0:3".into(),
            json!({"content":{"kind":"result","step_id":"0:1","path":[]}}),
        );
    } else {
        layouts.insert(
            "0:2".into(),
            json!({"answer":{"kind":"task_input","reference":"{{vars.answer}}"}}),
        );
    }
    steps.push(
        json!({"stepnumber":1+offset,"knowledge":"rust","goal":"Bind the usage",
        "content":"","type":"component","include":[descriptor],"tool_bindings":[{
            "tool_id":tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]}),
    );
    steps.push(
        json!({"stepnumber":2+offset,"knowledge":"orchestrator","goal":"Execute the usage",
        "content":"","type":"component","include":[code]}),
    );
    let recipe = json!({"variants":[{"variant_key":"selected","step_link":"0:1-0:E",
        "intent_examples":["reply %"],"variable_patterns":[{"name":"answer","pattern":null,"description":null}]}],
        "step_descriptions":[{"desc_idx":0,"label":"Explicit typed usage","yaml_source":"","steps":steps}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":task_inputs,"steps":layouts}}});
    let mut drafts = vec![
        (
            tool,
            0,
            json!({"capability_id":capability,"callable":callable,"input_contract":tool_inputs}),
            vec![],
            Value::Null,
        ),
        (
            descriptor,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":tool,
            "callable":callable,"capability_id":capability}}),
            vec![tool],
            Value::Null,
        ),
        (
            code,
            22,
            json!({"content":body,"input_contract":inputs,"result_contract":result}),
            vec![],
            Value::Null,
        ),
        (
            skill,
            1,
            json!({"body":prose}),
            vec![code, descriptor, tool],
            json!(association),
        ),
    ];
    let mut dependencies = vec![descriptor, code, skill];
    if history {
        dependencies.push(formatter);
        drafts.push((formatter,22,json!({"content":
            r"result = 'User: ' + inputs['user_input'] + '\nAssistant: ' + inputs['answer'] + '\nReply: ' + inputs['reply_ref']",
            "input_contract":task_inputs,"result_contract":{"type":"string"}}),vec![],Value::Null));
    }
    drafts.push((root, 21, recipe, dependencies, Value::Null));
    let mut refs = Vec::new();
    for (id, class, document, dependencies, association) in drafts {
        let draft = ComponentRevisionDraft::from_json(
            &json!({"format":"component-revision/1",
            "uuid":id,"class_code":class,"document":document,"dependencies":dependencies,
            "association":association})
            .to_string(),
        )
        .unwrap();
        refs.push(store.stage(&draft, 0).await.unwrap());
    }
    Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                Arc::new(store.read_exact(&[root], &refs).await.unwrap()),
                root,
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap(),
    )
}
