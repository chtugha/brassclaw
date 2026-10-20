//! Typed successors for the existing Kohai usage and its finite JSON input
//! profile. These drafts grant no approval, activation or Tool permission.
//! Resolve the existing Kohai Tool/ToolSkill/Skill/PythonCode UUIDs before use;
//! never create a second model executor merely to obtain these contracts.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};

fn string_input() -> Value {
    json!({"type":"string","required":true,"checks":[]})
}

/// Complete three-text-role profile supported by the task-bound native adapter.
/// This is not a provider Tool-result replay or root-control message schema.
pub fn model_prompt_result_schema() -> Value {
    json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "user_query":string_input(),
        "chat_history":{"type":"list","required":true,"checks":[],"items":{
            "type":"object","checks":[],"allow_extra_fields":false,
            "fields":{"role":string_input(),"content":string_input()}
        }}
    }})
}

fn prompt_input() -> Value {
    let mut schema = model_prompt_result_schema();
    schema["required"] = json!(true);
    schema
}

/// One finite profile of the actual model wire result. Structured capability
/// proposals require a different approved profile and explicit Recipe steps.
/// Optional observations remain absent; this contract supplies no defaults.
pub fn assistant_model_response_schema() -> Value {
    let token_count = json!({"type":"integer","required":true,"checks":[
        {"kind":"min","value":0},{"kind":"max","value":4294967295_u64}
    ]});
    json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "chunks":{"type":"list","required":true,"checks":[],"items":{
            "type":"object","checks":[],"allow_extra_fields":false,
            "fields":{"safe_text_delta":string_input()}
        }},
        "safe_reasoning_deltas":{"type":"list","required":false,"checks":[],
            "items":{"type":"string","checks":[]}},
        "output":{"type":"object","required":true,"checks":[],"allow_extra_fields":false,"fields":{
            "assistant_reply":{"type":"object","required":true,"checks":[],"allow_extra_fields":false,
                "fields":{"content":string_input()}}
        }},
        "effective_model_profile_id":string_input(),
        "usage":{"type":"object","required":false,"checks":[],"allow_extra_fields":false,"fields":{
            "input_tokens":token_count,"output_tokens":token_count,
            "cache_read_input_tokens":token_count,"cache_creation_input_tokens":token_count
        }}
    }})
}

pub const MODEL_ANSWER_TEXT_SOURCE: &str = "def extract_model_answer(inputs):\n    return inputs['response']['output']['assistant_reply']['content']\n";

/// R03 pure extraction after a separately bound JSON usage has checked the
/// complete assistant-response profile. No ToolSkill, Skill or model replay.
pub fn model_answer_text_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    if id.is_nil() {
        return Err(RevisionError::Invalid(
            "model answer component identity required",
        ));
    }
    let mut response = assistant_model_response_schema();
    response["required"] = json!(true);
    draft(
        id,
        22,
        json!({
            "name":"extract-model-answer-text",
            "description":"Extract the assistant_reply.content string from a complete validated native model response. Parse JSON in a separate bound usage with this exact finite profile first. Capability proposals, errors, nulls, unknown fields and incomplete envelopes fail validation before extraction. Empty text is valid; returned text remains data, never code or a confirmed effect. No model, Tool, reply, retry, history or activation side effect. Preserve the original received response and its accounting/custody evidence independently of this projection.",
            "content":MODEL_ANSWER_TEXT_SOURCE,
            "input_contract":{"response":response},
            "result_contract":{"type":"string","checks":[]},
            "preload":preload("extract_model_answer", &[])
        }),
        vec![],
        Value::Null,
    )
}

fn validate_ids(
    groups: &[(UsageComponentIds, bool, &str)],
    extra: &[Uuid],
) -> Result<(), RevisionError> {
    let mut used = BTreeSet::new();
    for id in extra {
        if id.is_nil() || !used.insert(*id) {
            return Err(RevisionError::Invalid("model workflow identities conflict"));
        }
    }
    let mut tools = BTreeMap::new();
    for (ids, formatter, callable) in groups {
        if ids.formatter.is_some() != *formatter {
            return Err(RevisionError::Invalid("unexpected model usage formatter"));
        }
        for id in [ids.recipe, ids.python_code, ids.tool_skill, ids.skill]
            .into_iter()
            .chain(ids.formatter)
        {
            if id.is_nil() || !used.insert(id) {
                return Err(RevisionError::Invalid(
                    "model usage identities must be distinct and non-nil",
                ));
            }
        }
        if ids.tool.is_nil()
            || tools
                .insert(ids.tool, *callable)
                .is_some_and(|previous| previous != *callable)
        {
            return Err(RevisionError::Invalid("model Tool identities conflict"));
        }
    }
    // Different usages of the same primitive share its identity, never any
    // mutable usage/component identity. A Tool cannot alias another component.
    if tools.keys().any(|id| used.contains(id)) {
        return Err(RevisionError::Invalid("model Tool aliases a component"));
    }
    Ok(())
}

fn draft(
    id: Uuid,
    class: i32,
    document: Value,
    dependencies: Vec<Uuid>,
    association: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,
        "class_code":class,"document":document,"dependencies":dependencies,"association":association}).to_string())
}

fn preload(export: &str, imports: &[&str]) -> Value {
    json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":imports,"dependencies":[],"default_export":export})
}

struct UsageDefinition<'a> {
    callable: &'a str,
    capability: &'a str,
    export: &'a str,
    source: &'a str,
    inputs: Value,
    result: Value,
    arguments: Value,
    code_arguments: Value,
    prose: String,
}

fn usage(
    ids: UsageComponentIds,
    definition: UsageDefinition<'_>,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let UsageDefinition {
        callable,
        capability,
        export,
        source,
        inputs,
        result,
        arguments,
        code_arguments,
        prose,
    } = definition;
    let definitions = preload(export, &[]);
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association = json!({"format":"skill-association/1","skill_uuid":ids.skill,"python_code_uuid":ids.python_code,
        "tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,"callable":callable,"inputs":inputs,
        "arguments":arguments,"code_arguments":code_arguments,"result":result,"failure":failure}).to_string();
    Ok(vec![
        draft(
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,
            "callable":callable,"capability_id":capability}}),
            vec![ids.tool],
            Value::Null,
        )?,
        draft(
            ids.python_code,
            22,
            json!({"content":source,"input_contract":inputs,"result_contract":result,"preload":definitions}),
            vec![],
            Value::Null,
        )?,
        draft(
            ids.skill,
            1,
            json!({"body":prose,"interface":{"format":"skill-interface/1","python_code_uuid":ids.python_code,
            "exports":definitions["exports"],"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        )?,
    ])
}

fn bind(number: u32, ids: UsageComponentIds, name: &str) -> Value {
    json!({"stepnumber":number,"knowledge":"rust","goal":"Prepare the exact selected Tool binding","content":"",
        "type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]})
}

fn execute(number: u32, code: Uuid, goal: &str) -> Value {
    json!({"stepnumber":number,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[code]})
}

fn result(step: &str) -> Value {
    json!({"kind":"result","step_id":step,"path":[]})
}

fn task_json(variable: &str) -> Value {
    json!({"kind":"task_input","reference":format!("{{{{vars.{variable}}}}}")})
}

struct JsonCommand<'a> {
    name: &'a str,
    sentence: &'a str,
    variable: &'a str,
    description: &'a str,
}

fn recipe(
    id: Uuid,
    command: JsonCommand<'_>,
    steps: Value,
    layouts: Value,
    invocations: Value,
    dependencies: Vec<Uuid>,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let JsonCommand {
        name,
        sentence,
        variable,
        description,
    } = command;
    let examples: Vec<_> = [
        "",
        "please ",
        "now ",
        "please now ",
        "execute ",
        "run ",
        "please execute ",
        "please run ",
        "perform ",
        "please perform ",
    ]
    .into_iter()
    .map(|prefix| format!("{prefix}{sentence}"))
    .collect();
    draft(
        id,
        21,
        json!({"name":name,"description":"Canonical command with explicit typed parsing and normal reply publication.",
        "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,
            "variable_patterns":[{"name":variable,"pattern":null,"description":description}]}],
        "step_descriptions":[{"desc_idx":0,"label":name,"yaml_source":"","steps":steps}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{variable:string_input()},"steps":layouts}},
        "invocations":{"selected":invocations}}),
        dependencies,
        Value::Null,
    )
}

/// N06 specialized to the model prompt's exact finite result, reusing builtin.json.
/// Its canonical Recipe parses, renders the typed value and posts through the
/// supplied existing reply usage. No model call is hidden in this parser.
pub fn model_prompt_json_usage_drafts(
    ids: UsageComponentIds,
    reply: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    validate_ids(
        &[(ids, true, "host.json"), (reply, false, "host.post_reply")],
        &[],
    )?;
    let formatter = ids
        .formatter
        .ok_or(RevisionError::Invalid("model prompt formatter missing"))?;
    let source = "def parse_model_prompt(inputs):\n    prompt = host.json(operation='parse', data=inputs['json_text'])\n    for message in prompt['chat_history']:\n        if message['role'] not in ('system', 'user', 'assistant'):\n            raise ValueError('model_prompt_role_invalid')\n    return prompt\n";
    let prose = format!(
        "Purpose\nParse one complete model-prompt JSON string with the existing JSON Tool, enforcing the exact three-text-role result profile. No model call or source execution.\n\nUse when\nAn explicit Recipe has JSON command data for the task-bound model usage.\n\nRequired components and interface\nTool {}; ToolSkill {}; associated PythonCode {}. Preload parse_model_prompt(inputs); definitions have no effects, helpers, constants, imports or dependencies. Each invocation is isolated.\n\nInputs\nRequired non-null json_text:string, with no default. The entire string is Tool data. Native JSON syntax/number/duplicate-key behavior applies; this usage does not claim duplicate-key rejection. No shortening, source substitution or alternative parser.\n\nResults\nExactly user_query:string and chat_history:list of objects containing required role:string and content:string, with no extra keys or defaults. Roles are exactly system, user or assistant. Empty history and text are valid. This profile does not represent structured provider Tool results. Wrong shapes, roles or missing/null fields fail before a downstream model operation.\n\nFailure and authority\nOne parse dispatch, stop on failure; no retry, repair defaults or Tier-2 replay. Selection and qualification do not grant Tool permission. Current policy, task/attempt freshness, cancellation and acknowledged limits still apply.\n\nExecution Recipe and command\nRecipe {}, variant selected. Canonical sentence: parse model prompt json= followed by one complete JSON object. The single percent slot in parse model prompt json=% is prompt_json:string; JSON escaping encodes quotes/newlines, and the original captured string is passed verbatim to the parser. Example: parse model prompt json={{\"user_query\":\"Hello ü\",\"chat_history\":[]}}. The Recipe renders the validated typed value as JSON and uses the existing reply usage; its normal chat owner handles history. Qualification/activation remain separate.",
        ids.tool, ids.tool_skill, ids.python_code, ids.recipe
    );
    let mut drafts = usage(
        ids,
        UsageDefinition {
            callable: "host.json",
            capability: "builtin.json",
            export: "parse_model_prompt",
            source,
            inputs: json!({"json_text":string_input()}),
            result: model_prompt_result_schema(),
            arguments: json!({"data":"json_text"}),
            code_arguments: json!({"operation":{"type":"string","checks":[],"depends_on":[],"meaning":"Fixed parse selector; parse exactly the complete supplied json_text"}}),
            prose,
        },
    )?;
    drafts.push(draft(formatter,22,json!({"content":"def render_model_prompt(inputs):\n    import json\n    return json.dumps(inputs['prompt'], ensure_ascii=False)\n",
        "input_contract":{"prompt":prompt_input()},"result_contract":{"type":"string","checks":[]},"preload":preload("render_model_prompt",&["json"])}),vec![],Value::Null)?);
    drafts.push(recipe(ids.recipe,JsonCommand {name:"parse-model-prompt",sentence:"parse model prompt json=%",variable:"prompt_json",description:"One complete JSON object; the parser validates its exact model-prompt profile"},
        json!([bind(1,ids,"json"),execute(2,ids.python_code,"Parse and validate complete prompt data"),execute(3,formatter,"Render complete typed prompt as JSON data"),bind(4,reply,"post_reply"),execute(5,reply.python_code,"Publish through the selected reply usage")]),
        json!({"0:2":{"json_text":task_json("prompt_json")},"0:3":{"prompt":result("0:2")},"0:5":{"answer":result("0:3")}}),
        json!({"0:2":"parse_model_prompt","0:3":"render_model_prompt","0:5":"publish_turn_reply"}),
        vec![ids.skill,ids.tool_skill,ids.python_code,formatter,reply.skill,reply.tool_skill,reply.python_code])?);
    Ok(drafts)
}

pub const ASSISTANT_RESPONSE_JSON_SOURCE: &str = "def parse_assistant_model_response(inputs):\n    return host.json(operation='parse', data=inputs['json_text'])\n";

/// N06 for the complete assistant-only native response, independently bound
/// from both prompt parsing and R03 extraction. No model operation is repeated.
pub fn model_response_json_usage_drafts(
    ids: UsageComponentIds,
    reply: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    validate_ids(
        &[(ids, true, "host.json"), (reply, false, "host.post_reply")],
        &[],
    )?;
    let formatter = ids
        .formatter
        .ok_or(RevisionError::Invalid("model response formatter missing"))?;
    let prose = format!(
        "Purpose\nParse one complete native assistant-response JSON string with the existing JSON Tool and validate its exact finite result contract. This usage neither calls a model nor publishes an extracted answer.\n\nUse when\nAn explicit Recipe consumes the actual LoopModelResponse JSON and selects the assistant-only profile before R03 extraction or another typed consumer. Structured capability proposals need a separate approved profile and workflow.\n\nRequired components and interface\nTool {}; ToolSkill {}; associated PythonCode {}. Preload parse_assistant_model_response(inputs) without effects, imports, helpers, constants or dependencies. Mutable values are isolated per invocation/task/attempt.\n\nInputs\nRequired non-null json_text:string without default. Pass the complete original JSON as Tool data, never source. No shortening or alternate parser. Native JSON syntax, number and duplicate-key behavior applies; no duplicate-key rejection is claimed.\n\nResults\nExactly the declared assistant_model_response profile: required chunks with safe_text_delta strings, effective_model_profile_id string and output.assistant_reply.content string; optional safe_reasoning_deltas and usage remain absent when omitted. Usage, when present, requires all four unsigned 32-bit token counts. Every object rejects extra fields; no null or defaults. Empty text and chunks are valid, chunk text is preserved independently of answer text and no token usage is inferred from chunks. Missing fields, malformed shapes and capability_calls fail this profile. Preserve the complete received original response and its independent custody/accounting evidence even on validation failure.\n\nFailure and authority\nOne JSON dispatch, stop on failure; no model retry, repaired response, fallback defaults, proposal dispatch or Tier-2 replay. Qualification grants no Tool permission. Current policy, cancellation, task/attempt freshness and acknowledged limits apply.\n\nExecution Recipe and command\nRecipe {}, variant selected. Canonical sentence parse assistant model response json= followed by one complete JSON object. The single percent slot in parse assistant model response json=% is response_json:string. JSON escaping encodes quotes/newlines; the captured text is passed verbatim. Example: parse assistant model response json={{\"chunks\":[],\"output\":{{\"assistant_reply\":{{\"content\":\"Hello ü\"}}}},\"effective_model_profile_id\":\"selected_model\"}}. The canonical Recipe renders the complete validated value as JSON and posts through the selected existing reply usage; chat history remains with its normal owner. Activation, exact association approval and retained artifacts are separate requirements.",
        ids.tool, ids.tool_skill, ids.python_code, ids.recipe
    );
    let mut drafts = usage(
        ids,
        UsageDefinition {
            callable: "host.json",
            capability: "builtin.json",
            export: "parse_assistant_model_response",
            source: ASSISTANT_RESPONSE_JSON_SOURCE,
            inputs: json!({"json_text":string_input()}),
            result: assistant_model_response_schema(),
            arguments: json!({"data":"json_text"}),
            code_arguments: json!({"operation":{"type":"string","checks":[],"depends_on":[],"meaning":"Fixed parse selector; parse exactly the complete supplied json_text"}}),
            prose,
        },
    )?;
    let mut response = assistant_model_response_schema();
    response["required"] = json!(true);
    drafts.push(draft(formatter,22,json!({"content":"def render_assistant_model_response(inputs):\n    import json\n    return json.dumps(inputs['response'], ensure_ascii=False)\n",
        "input_contract":{"response":response},"result_contract":{"type":"string","checks":[]},"preload":preload("render_assistant_model_response",&["json"])}),vec![],Value::Null)?);
    drafts.push(recipe(ids.recipe,JsonCommand {name:"parse-assistant-model-response",sentence:"parse assistant model response json=%",variable:"response_json",description:"One complete JSON object; the parser validates the exact native assistant-response profile"},
        json!([bind(1,ids,"json"),execute(2,ids.python_code,"Parse the complete finite assistant-response profile"),execute(3,formatter,"Render the complete validated response as JSON data"),bind(4,reply,"post_reply"),execute(5,reply.python_code,"Publish through the selected reply usage")]),
        json!({"0:2":{"json_text":task_json("response_json")},"0:3":{"response":result("0:2")},"0:5":{"answer":result("0:3")}}),
        json!({"0:2":"parse_assistant_model_response","0:3":"render_assistant_model_response","0:5":"publish_turn_reply"}),
        vec![ids.skill,ids.tool_skill,ids.python_code,formatter,reply.skill,reply.tool_skill,reply.python_code])?);
    Ok(drafts)
}

/// Separate assistant-text consumer of the existing raw completion usage.
/// Bind each Tool call independently; R03 remains a pure component. The native
/// reply primitive rejects blank text without replaying a received model call.
pub fn model_answer_recipe_draft(
    id: Uuid,
    model: UsageComponentIds,
    prompt: UsageComponentIds,
    response: UsageComponentIds,
    projection: Uuid,
    reply: UsageComponentIds,
) -> Result<ComponentRevisionDraft, RevisionError> {
    validate_ids(
        &[
            (prompt, true, "host.json"),
            (model, false, "host.kohai_complete"),
            (response, true, "host.json"),
            (reply, false, "host.post_reply"),
        ],
        &[id, projection],
    )?;
    if prompt.tool != response.tool {
        return Err(RevisionError::Invalid(
            "model parsers must reuse the existing JSON Tool",
        ));
    }
    recipe(
        id,
        JsonCommand {
            name: "answer-model-prompt",
            sentence: "answer model prompt json=%",
            variable: "prompt_json",
            description: "One complete JSON object; parse the model prompt, then validate the complete native assistant-response profile before extracting answer text. Blank answers fail the existing reply primitive; no retry or Tier-2 replay",
        },
        json!([
            bind(1, prompt, "json"),
            execute(2, prompt.python_code, "Parse complete typed model prompt"),
            bind(3, model, "kohai_complete"),
            execute(4, model.python_code, "Invoke one retained model operation"),
            bind(5, response, "json"),
            execute(
                6,
                response.python_code,
                "Validate the complete finite assistant-response profile without retrying the model"
            ),
            execute(7, projection, "Extract validated assistant text as data"),
            bind(8, reply, "post_reply"),
            execute(
                9,
                reply.python_code,
                "Publish nonblank assistant text through the existing reply usage"
            )
        ]),
        json!({"0:2":{"json_text":task_json("prompt_json")},"0:4":{"prompt":result("0:2")},"0:6":{"json_text":result("0:4")},"0:7":{"response":result("0:6")},"0:9":{"answer":result("0:7")}}),
        json!({"0:2":"parse_model_prompt","0:4":"complete_model_usage","0:6":"parse_assistant_model_response","0:7":"extract_model_answer","0:9":"publish_turn_reply"}),
        vec![
            prompt.skill,
            prompt.tool_skill,
            prompt.python_code,
            model.skill,
            model.tool_skill,
            model.python_code,
            response.skill,
            response.tool_skill,
            response.python_code,
            projection,
            reply.skill,
            reply.tool_skill,
            reply.python_code,
        ],
    )
}

/// Successor of pc-host-kohai-complete using its existing resolved component
/// identities. The canonical Recipe calls N06, this usage, then normal reply.
pub fn model_completion_usage_drafts(
    ids: UsageComponentIds,
    parser: UsageComponentIds,
    reply: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    validate_ids(
        &[
            (parser, true, "host.json"),
            (ids, false, "host.kohai_complete"),
            (reply, false, "host.post_reply"),
        ],
        &[],
    )?;
    let inputs = json!({"prompt":prompt_input()});
    let source = "def complete_model_usage(inputs):\n    return host.kohai_complete(prompt=inputs['prompt'])\n";
    let prose = format!(
        "Purpose\nPerform one explicit Tier-1 provider model operation through the existing task-bound Kohai primitive and return its complete LoopModelResponse as JSON text.\n\nUse when\nAn approved Recipe needs a selected model's creative/reasoning operation. This usage does not route No-Match work or execute proposals.\n\nRequired components and interface\nExisting Tool {}; ToolSkill {}; associated PythonCode {}. Preload complete_model_usage(inputs) without effects; no imports, helpers, constants, mutable defaults or dependencies. Invoke only with its exact pinned binding; inputs/results are isolated per invocation/task/attempt.\n\nInputs\nRequired prompt:object, no default or extra keys. Required user_query:string and chat_history:list of objects with required role:string and content:string, no extras/defaults. Roles are system, user or assistant. Empty text/history are explicit valid values. Preserve all supplied messages/order/text, including quotes, Unicode, newlines and marker-looking content. Three text roles do not encode structured provider Tool-result replay. No prefix_placeholder, prefix_reference, model/provider selector, Python source or root-control field is accepted. The host supplies this admitted task's retained prefix and model route, scope and invocation identity.\n\nResults\nReturn the complete actual LoopModelResponse JSON string, including output, chunks, safe_reasoning_deltas, effective_model_profile_id and optional usage. Unknown usage stays unknown. This is not an ok/answer envelope. Proposals remain data: a consuming Recipe must parse the actual supported finite response profile, validate meaning and dispatch separate approved usages if required. Never execute returned code, stringify proposals into a claimed answer or infer a confirmed Tool effect.\n\nFailure and authority\nExactly one initial dispatch, no automatic retry or Tier-2 replay. Invalid input, missing route/prefix, provider failure, context/capacity/policy failure or cancellation stops the usage. Received responses belong to the original invocation's evidence before later output validation/delivery; persistence failure retains them for trusted reconciliation and never authorizes another model call. A timeout does not prove no provider effect. Current kernel Tool policy, model technical limits, task/attempt fencing and accounting remain independent of component approval. Internal deterministic-root restrictions remain; this is no root stream_model bypass.\n\nExecution Recipe and command\nRecipe {}, variant selected, always RequiresModel/Tier 1. Canonical sentence: complete model prompt json= followed by one complete JSON object. The single slot in complete model prompt json=% is prompt_json:string, passed verbatim to the separately bound typed JSON usage; runtime values never become source. Example: complete model prompt json={{\"user_query\":\"Say hello\",\"chat_history\":[]}}. The Recipe parses the prompt, invokes this exact model export and posts the entire response JSON through the existing reply usage. It makes no answer-text/structured-proposal selection. Normal chat history remains with its existing owner. Exact package/artifact, association review, coherent activation and production-path acceptance are separate requirements.",
        ids.tool, ids.tool_skill, ids.python_code, ids.recipe
    );
    let mut drafts = usage(
        ids,
        UsageDefinition {
            callable: "host.kohai_complete",
            capability: "host.kohai_complete",
            export: "complete_model_usage",
            source,
            inputs,
            result: json!({"type":"string","checks":[]}),
            arguments: json!({"prompt":"prompt"}),
            code_arguments: json!({}),
            prose,
        },
    )?;
    drafts.push(recipe(ids.recipe,JsonCommand {name:"complete-model-prompt",sentence:"complete model prompt json=%",variable:"prompt_json",description:"One complete JSON object; the parser validates its exact model-prompt profile"},
        json!([bind(1,parser,"json"),execute(2,parser.python_code,"Parse complete typed model prompt"),bind(3,ids,"kohai_complete"),execute(4,ids.python_code,"Invoke one retained model operation"),bind(5,reply,"post_reply"),execute(6,reply.python_code,"Publish the complete response JSON without interpreting proposals")]),
        json!({"0:2":{"json_text":task_json("prompt_json")},"0:4":{"prompt":result("0:2")},"0:6":{"answer":result("0:4")}}),
        json!({"0:2":"parse_model_prompt","0:4":"complete_model_usage","0:6":"publish_turn_reply"}),
        vec![parser.skill,parser.tool_skill,parser.python_code,ids.skill,ids.tool_skill,ids.python_code,reply.skill,reply.tool_skill,reply.python_code])?);
    Ok(drafts)
}

/// Exact linked-package metadata; hashing a caller-supplied string establishes
/// neither retained executable identity nor trusted bootstrap provenance.
pub fn model_completion_tool_draft(
    id: Uuid,
    artifact_checksum: &str,
) -> Result<ComponentRevisionDraft, RevisionError> {
    if id.is_nil()
        || artifact_checksum.len() != 64
        || !artifact_checksum
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(RevisionError::Invalid(
            "exact model package checksum required",
        ));
    }
    draft(
        id,
        0,
        json!({"name":"host.kohai_complete","capability_id":"host.kohai_complete","callable":"host.kohai_complete",
        "input_contract":{"prompt":prompt_input()},"implementation":{"format":"packaged-task-model/1","artifact_checksum":artifact_checksum,"adapter":"TaskModelCapability/1"}}),
        vec![],
        Value::Null,
    )
}
