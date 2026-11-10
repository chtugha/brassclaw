//! Private single-case harness over one native Tool usage and an independent
//! pure receipt encoder. Drafts are neither qualification nor activation.
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::{UsageComponentIds, packaged_component_id},
};

pub const INVOKE_SOURCE: &str =
    "def invoke_original_pure_case(inputs):\n    return host.invoke_original_pure_case()\n";
pub const ENCODE_SOURCE: &str = include_str!("pure_case_receipt.py");

pub fn usage_ids() -> UsageComponentIds {
    let id = |part| packaged_component_id(&format!("validator:pure-case:{part}"));
    UsageComponentIds {
        recipe: id("recipe"),
        python_code: id("code"),
        skill: id("skill"),
        tool_skill: id("tool-skill"),
        tool: id("tool"),
        formatter: None,
    }
}
pub fn encoder_id() -> Uuid {
    packaged_component_id("validator:pure-case:receipt-encoder")
}

pub fn qualification_recipe() -> Result<ComponentRevisionDraft, RevisionError> {
    let ids = usage_ids();
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1",
        "uuid":packaged_component_id("validator:pure-case:qualification-recipe"),"class_code":21,
        "association":null,"dependencies":[ids.tool_skill,ids.python_code,ids.skill],
        "document":{"name":"contained-original-pure-usage-qualification",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
            "step_descriptions":[{"desc_idx":0,"label":"Observe one closed seed instrument","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"rust","goal":"Prepare the actual original invocation binding","content":"","type":"component","include":[ids.tool_skill],
                    "tool_bindings":[{"tool_id":ids.tool,"tool_name":"invoke_original_pure_case","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Invoke the exact original usage once","content":"","type":"component","include":[ids.python_code]}
            ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{},"steps":{"0:2":{}}}},
            "invocations":{"selected":{"0:2":"invoke_original_pure_case"}}}}).to_string())
}

pub fn drafts() -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let ids = usage_ids();
    let encoder = encoder_id();
    let text = json!({"type":"string","checks":[]});
    let inputs = json!({});
    let export = "invoke_original_pure_case";
    let preload = |name: &str, imports: Vec<&str>| {
        json!({"format":"python-preload/2",
        "exports":{name:{"symbol":name,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":imports,"dependencies":[],"default_export":name})
    };
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association = json!({"format":"skill-association/1","skill_uuid":ids.skill,
        "python_code_uuid":ids.python_code,"tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,
        "callable":"host.invoke_original_pure_case","inputs":inputs,"arguments":{},
        "code_arguments":{},"result":text,"failure":failure})
    .to_string();
    let make = |uuid: Uuid,
                class_code: i32,
                document: Value,
                dependencies: Vec<Uuid>,
                association: Value| {
        ComponentRevisionDraft::from_json(
            &json!({"format":"component-revision/1","uuid":uuid,"class_code":class_code,
            "document":document,"dependencies":dependencies,"association":association})
            .to_string(),
        )
    };
    Ok(vec![
        make(
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1",
            "tool_uuid":ids.tool,"callable":"host.invoke_original_pure_case",
            "capability_id":"builtin.pure_review.invoke_original"}}),
            vec![ids.tool],
            Value::Null,
        )?,
        make(
            ids.python_code,
            22,
            json!({"name":"invoke-original-pure-case",
            "description":"Invoke the already sealed original once through the exact native operation. No source, export, case, input, expected result or authority is supplied by this usage.",
            "content":INVOKE_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":inputs,"result_contract":text,"preload":preload(export,vec![])}),
            vec![],
            Value::Null,
        )?,
        make(
            ids.skill,
            1,
            json!({"body":format!(
            "Purpose\nInvoke one already sealed original Pure export through Tool {}, ToolSkill {}, and associated PythonCode {}. Export invoke_original_pure_case(inputs) calls host.invoke_original_pure_case() exactly once. Loading defines the function without invoking it. No helpers, constants or code dependencies.\n\nInput and authority\ninputs is an empty non-null object. The request carries no source, export, input, case selector, expected result or authorization flag. Composition must supply a checked original, immutable case and actual live same-task executor window through the native invocation service. Missing, swapped, stale or reused service state fails. Binding grants no permission. The distinct ExecuteCode Tool's current instance policy and all native technical checks still apply.\n\nResult\nReturn the exact original-pure-invocation/1 observation JSON string after confirmed child-context release. Return data, actual Python exception data and result-contract defects are observations, never a pass verdict. Supported unexpected exceptions remain visible with their actual class. Native MemoryError, TimeoutError and RecursionError have ambiguous resource origin and fail technically, including explicit candidate raises of those classes. Worker, preload, protocol, budget, cancellation and unresolved outcomes fail the operation. The task retains the original native journals and transport receipts; uncertainty never permits a second start.\n\nFailure and consumer\nStop with one initial attempt, no retry or assumed idempotency. No failure-to-Tier-2 conversion. A consuming private ResultOnly Recipe invokes this usage and separately encodes its receipt. An observed candidate failure can be delivered successfully by that harness; ordinary Recipe failures remain fail-stop. The private delivery owner settles the case only after its ordinary success receipt. This usage grants no Q1, behavioral adequacy, human Q2, component approval or publication. No public command, MCP or reply is defined.",ids.tool,ids.tool_skill,ids.python_code),
            "interface":{"format":"skill-interface/1","python_code_uuid":ids.python_code,
                "exports":preload(export,vec![])["exports"],"inputs":inputs,"result":text,
                "failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        )?,
        make(
            encoder,
            22,
            json!({"name":"encode-original-pure-case-receipt",
            "description":"Encode a complete native observation as the private harness receipt. Preserve all observation fields and outcome data. This is not a behavioral verdict or approval.",
            "content":ENCODE_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{"case_outcome_json":{"type":"string","checks":[],"required":true}},
            "result_contract":text,"preload":preload("encode_original_pure_case_receipt",vec!["json"])}),
            vec![],
            Value::Null,
        )?,
        make(
            ids.recipe,
            21,
            json!({"name":"private-original-pure-case",
            "description":"One private ResultOnly case: invoke the sealed original and encode its complete observation in a separate pure step. No matching, model, reply, approval or publication.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
            "step_descriptions":[{"desc_idx":0,"label":"Observe one original case","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"rust","goal":"Prepare the exact native original binding","content":"","type":"component","include":[ids.tool_skill],
                    "tool_bindings":[{"tool_id":ids.tool,"tool_name":"invoke_original_pure_case","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Invoke the pinned original usage once","content":"","type":"component","include":[ids.python_code]},
                {"stepnumber":3,"knowledge":"orchestrator","goal":"Encode the whole native observation","content":"","type":"component","include":[encoder]}
            ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{},"steps":{
                "0:2":{},"0:3":{"case_outcome_json":{"kind":"result","step_id":"0:2","path":[]}}
            }}},"invocations":{"selected":{"0:2":export,"0:3":"encode_original_pure_case_receipt"}}}),
            vec![ids.tool_skill, ids.python_code, ids.skill, encoder],
            Value::Null,
        )?,
    ])
}
