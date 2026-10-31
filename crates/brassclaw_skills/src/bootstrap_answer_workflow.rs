//! Actual B1/B2 answer protocol graph over existing independently owned usages.
//! Draft construction is not qualification, import, activation or Free authority.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
    prefix_bootstrap_components::bootstrap_prompt_draft,
    strict_json_components::StrictJsonUsageIds,
};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct BootstrapAnswerWorkflowIds {
    pub recipe: Uuid,
    pub prompt: Uuid,
    pub protocol: Uuid,
}

/// Pure wiring, not a second protocol validator. Both strings originate from
/// this exact Recipe: the captured packet and strict usage's canonical result.
pub const BOOTSTRAP_PROTOCOL_SOURCE: &str = r#"def validate_captured_bootstrap_answer(inputs):
    import json
    packet = json.loads(inputs['packet_bytes'])
    answer = json.loads(inputs['answer_bytes'])
    payload = packet['payload']
    context = {
        'answer': answer,
        'expected_task': payload['task'],
        'expected_request_id': packet['request_id'],
        'expected_input_sha256': packet['input_sha256'],
        'required_facts': payload['required_facts'],
        'eligible_candidate_ids': [option['candidate_id'] for option in payload['eligible_options']],
    }
    return bootstrap_answer_stage({
        'phase': 'answer',
        'data_bytes': json.dumps(context, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False),
    })
"#;

/// Model/response usages refer to existing real usages. Their separate consumer
/// Recipe/formatter IDs are not required or referenced here. The strict usage
/// already has its own four-ID interface. No new usage/Tool/parser is authored.
/// answer_text is the existing R03; answer_stage is the existing B1/B2 library.
/// The closed owner must verify every exact referenced definition/approval and
/// compile this as RequiresModel before any contained execution or activation.
pub fn bootstrap_answer_workflow_drafts(
    ids: BootstrapAnswerWorkflowIds,
    model: UsageComponentIds,
    response: UsageComponentIds,
    strict: StrictJsonUsageIds,
    answer_text: Uuid,
    answer_stage: Uuid,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let components = [
        ids.recipe,
        ids.prompt,
        ids.protocol,
        answer_text,
        answer_stage,
        model.tool_skill,
        model.python_code,
        model.skill,
        response.tool_skill,
        response.python_code,
        response.skill,
        strict.tool_skill,
        strict.python_code,
        strict.skill,
    ];
    if components.iter().any(Uuid::is_nil)
        || components
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != components.len()
        || model.tool.is_nil()
        || response.tool.is_nil()
        || strict.tool != response.tool
        || model.tool == response.tool
        || components.contains(&model.tool)
        || components.contains(&response.tool)
    {
        return Err(RevisionError::Invalid(
            "bootstrap answer graph identities conflict",
        ));
    }
    let required_text =
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]});
    let protocol = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":ids.protocol,"class_code":22,
        "document":{
            "name":"validate-captured-bootstrap-answer",
            "description":"Pure protocol wiring after the same captured packet was checked by prepare_bootstrap_prompt and actual assistant text passed the bound strict JSON usage. Derive all expected echoes/requirements/options from original packet, then call the pinned existing bootstrap_answer_stage. No raw-response entry point, provenance, technical-query replacement, evidence/import/CAS, model call, retry or approval. Native dispatch correlation and source/target scope remain mandatory owner checks.",
            "content":BOOTSTRAP_PROTOCOL_SOURCE,
            "input_contract":{"packet_bytes":required_text,"answer_bytes":required_text},
            "result_contract":{"type":"string","checks":[{"kind":"min_length","value":1}]},
            "preload":{"format":"python-preload/2",
                "exports":{"validate_captured_bootstrap_answer":{"symbol":"validate_captured_bootstrap_answer","parameters":["inputs"],"mapping":true}},
                "private_functions":{},"constants":[],"imports":["json"],
                "dependencies":[answer_stage],"default_export":"validate_captured_bootstrap_answer"}
        },"dependencies":[answer_stage],"association":null
    }).to_string())?;
    let execute = |number, code, goal| json!({"stepnumber":number,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[code]});
    let bind = |number, tool, usage, name| json!({"stepnumber":number,"knowledge":"rust","goal":"Prepare existing single-Tool binding","content":"","type":"component","include":[usage],"tool_bindings":[{"tool_id":tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]});
    let task = |name: &str| json!({"kind":"task_input","reference":format!("{{{{vars.{name}}}}}")});
    let result = |step| json!({"kind":"result","step_id":step,"path":[]});
    let prompt = bootstrap_prompt_draft(ids.prompt)?;
    let inputs = prompt.document()["input_contract"].clone();
    let recipe = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":ids.recipe,"class_code":21,
        "document":{
            "name":"bootstrap-model-answer-protocol",
            "description":"One explicit optional B1 or B2 model request over its captured packet. Technical discovery and deterministic decisions precede this Recipe; it is invoked only if LLM interpretation is needed. Output is protocol-checked data, never an import or design approval. Qualification, original executing pair, final encoding, required child/review closure and native custody remain mandatory before dispatch. This definition alone grants no intentional prefix omission or public route.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E",
                "intent_examples":[],"variable_patterns":[]}],
            "model_input_bindings":{"format":"recipe-model-input-bindings/1","variants":{"selected":{"selection":"prefix_free_workflow","purpose":"basic_provider_configuration_values"}}},
            "step_descriptions":[{"desc_idx":0,"label":"Captured B1/B2 protocol request","yaml_source":"","steps":[
                execute(1,ids.prompt,"Render fixed instructions and original packet with explicit output reservation"),
                bind(2,model.tool,model.tool_skill,"kohai_complete"),
                execute(3,model.python_code,"Invoke existing retained model usage once"),
                bind(4,response.tool,response.tool_skill,"json"),
                execute(5,response.python_code,"Validate finite native assistant response envelope"),
                execute(6,answer_text,"Extract unchanged assistant text through existing R03"),
                bind(7,strict.tool,strict.tool_skill,"json"),
                execute(8,strict.python_code,"Strictly parse full assistant text without repair"),
                execute(9,ids.protocol,"Check B1/B2 protocol against original packet")
            ]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":{
                "0:1":{"packet_bytes":task("packet_bytes"),"max_output_tokens":task("max_output_tokens")},
                "0:3":{"prompt":result("0:1")},"0:5":{"json_text":result("0:3")},
                "0:6":{"response":result("0:5")},"0:8":{"json_text":result("0:6")},
                "0:9":{"packet_bytes":task("packet_bytes"),"answer_bytes":result("0:8")}
            }}},
            "invocations":{"selected":{"0:1":"prepare_bootstrap_prompt","0:3":"complete_model_usage","0:5":"parse_assistant_model_response","0:6":"extract_model_answer","0:8":"parse_strict_json_text","0:9":"validate_captured_bootstrap_answer"}}
        },
        "dependencies":[ids.prompt,ids.protocol,model.skill,model.tool_skill,model.python_code,response.skill,response.tool_skill,response.python_code,answer_text,strict.skill,strict.tool_skill,strict.python_code],
        "association":null
    }).to_string())?;
    Ok(vec![prompt, protocol, recipe])
}

#[cfg(test)]
#[path = "bootstrap_answer_workflow_tests.rs"]
mod tests;
