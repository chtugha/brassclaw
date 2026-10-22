//! Typed bounded-GET successor drafts for the existing HTTP component family.
//! Resolve the existing http / ts-http-fetch / pc-exec-http-get / skill-http-get /
//! http-get identities before use. These constructors neither allocate a second
//! Tool/Recipe family nor write legacy rows, approve, activate or perform I/O.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};
use serde_json::{Value, json};
use uuid::Uuid;

fn text(required: bool) -> Value {
    json!({"type":"string","required":required,"checks":[]})
}
fn positive(required: bool, max: u64) -> Value {
    json!({"type":"integer","required":required,"checks":[{"kind":"min","value":1},{"kind":"max","value":max}]})
}
fn inputs() -> Value {
    json!({"url":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},
        "response_body_limit":positive(true,9_007_199_254_740_991),
        "timeout_ms":positive(true,u32::MAX as u64)})
}
/// Declared typed profile of the existing native HTTP ABI. JSON bodies must be
/// serialized explicitly; headers use the supported object-of-strings profile.
/// The native implementation and live egress/kernel remain authoritative.
pub fn http_tool_draft(id: Uuid, identity: Value) -> Result<ComponentRevisionDraft, RevisionError> {
    if id.is_nil() {
        return Err(RevisionError::Invalid(
            "existing HTTP Tool identity required",
        ));
    }
    let contract = json!({"url":text(true),"method":text(false),
        "headers":{"type":"object","required":false,"checks":[],"fields":{},
            "allow_extra_fields":true,"extra_fields":{"type":"string","checks":[]}},
        "body":text(false),"body_base64":text(false),
        "response_body_limit":positive(false,9_007_199_254_740_991),
        "timeout_ms":positive(false,u32::MAX as u64)});
    draft(
        id,
        0,
        json!({"capability_id":"builtin.http","callable":"host.http",
        "input_contract":contract,"implementation":{"format":"native-first-party/1","identity":identity}}),
        vec![],
        Value::Null,
    )
}
/// Complete sanitized network-only response shape. The native formatter supplies
/// exactly one of body_text/body_base64; consumers must check the representation.
pub fn http_response_schema() -> Value {
    let count = json!({"type":"integer","required":true,"checks":[{"kind":"min","value":0},{"kind":"max","value":9_007_199_254_740_991_u64}]});
    json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "status":{"type":"integer","required":true,"checks":[{"kind":"min","value":100},{"kind":"max","value":599}]},
        "headers":{"type":"list","required":true,"checks":[],"items":{"type":"object","checks":[],
            "allow_extra_fields":false,"fields":{"name":text(true),"value":text(true)}}},
        "body_text":text(false),"body_base64":text(false),"request_bytes":count,"response_bytes":count,
        "redaction_applied":{"type":"boolean","required":true,"checks":[]}
    }})
}

pub const BOUNDED_GET_SOURCE: &str = "def fetch_http_bounded(inputs):\n    return host.http(url=inputs['url'], method='get', response_body_limit=inputs['response_body_limit'], timeout_ms=inputs['timeout_ms'])\n";

/// One usage and its canonical `http-get` / `bounded` variant. No source checking,
/// hashing, redirects, parsing, observation storage or provider calls are hidden.
/// This successor requires independent native/binding/behavior qualification;
/// do not overwrite other existing variants or activate it by a status label.
pub fn bounded_http_get_drafts(
    ids: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let unique = std::collections::BTreeSet::from([
        ids.recipe,
        ids.python_code,
        ids.tool_skill,
        ids.tool,
        ids.skill,
    ]);
    if ids.formatter.is_some() || unique.len() != 5 || unique.contains(&Uuid::nil()) {
        return Err(RevisionError::Invalid(
            "distinct existing HTTP usage identities required",
        ));
    }
    let inputs = inputs();
    let result = http_response_schema();
    let export = "fetch_http_bounded";
    let preload = json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":export});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association = json!({"format":"skill-association/1","skill_uuid":ids.skill,
        "python_code_uuid":ids.python_code,"tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,
        "callable":"host.http","inputs":inputs,
        "arguments":{"url":"url","response_body_limit":"response_body_limit","timeout_ms":"timeout_ms"},
        "code_arguments":{"method":{"type":"string","checks":[],"depends_on":[],"meaning":"Fixed lowercase GET method; no caller override"}},"result":result,"failure":failure}).to_string();
    let prose = format!(
        "Purpose\nPerform exactly one bounded HTTP GET through the existing builtin.http primitive.\n\nInterface\nAssociated PythonCode {} preloads fetch_http_bounded(inputs); loading has no effect. Tool {} and ToolSkill {} prepare host.http. Canonical Recipe {} variant bounded invokes it once. Reuse these existing stable identities; a changed interface is a reviewed successor, not a new parallel HTTP registry.\n\nInputs\nurl is a required non-null HTTP(S) URL string. response_body_limit is an explicit positive exact byte limit within the transport's integer representation; timeout_ms is an explicit positive u32 millisecond limit. No defaults, string interpolation, browser paths, arbitrary headers, request body, credentials or executable source are accepted by this usage. Native egress validates URL/credential/network policy and enforces acknowledged technical limits; these values cannot bypass it.\n\nResult\nReturn the complete sanitized native response object: status, header name/value list, exactly one of body_text/body_base64, request_bytes, response_bytes and redaction_applied. The recursive contract checks the finite field profile; native behavior and consumer checks establish the text/binary alternative. Non-2xx statuses are data. No response clipping or implicit redirect follow occurs; an exceeded body/time bound is a classified failure.\n\nWorkflow\nThis Skill does not crawl, execute source content, hash, judge a document or write availability/approval. Consuming Recipes perform independent fetch/hash/interpret/retain steps with typed handoffs. HTTP 200, nonempty text or a matching title alone never establishes referenced-content identity or knowledge quality. Authentication uses the supported host boundary; never place secrets in URL or reusable component data.\n\nAuthority/failure\nCurrent global Tool policy and network/secret/resource enforcement apply before dispatch. Binding and component approval grant no Tool access. One initial attempt, no automatic retry, effect replay or Tier-2 fallback. Retain actual outcomes, including uncertain dispatches; timeout is not proof that no request happened.\n\nAcceptance\nVerify exact primitive/adapter/artifact, recursive contracts, preload/invocation separation, URL-as-data, actual loopback HTTP responses and denial/overflow/cancellation behavior through the retained path. These drafts supply no approval, activation or installed source-check readiness.",
        ids.python_code, ids.tool, ids.tool_skill, ids.recipe
    );
    let pattern = "fetch url=% max-bytes=% timeout-ms=%";
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
    .iter()
    .map(|lead| format!("{lead}{pattern}"))
    .collect();
    let mut bindings = serde_json::Map::new();
    for name in ["url", "response_body_limit", "timeout_ms"] {
        bindings.insert(
            name.into(),
            json!({"kind":"task_input","reference":format!("{{{{vars.{name}}}}}")}),
        );
    }
    let variables = vec![
        json!({"name":"url","pattern":null,"description":"Exact URL, treated only as data"}),
        json!({"name":"response_body_limit","pattern":null,"description":"Exact positive response byte limit"}),
        json!({"name":"timeout_ms","pattern":null,"description":"Exact positive timeout milliseconds"}),
    ];
    [
        draft(ids.tool_skill,13,json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,
            "callable":"host.http","capability_id":"builtin.http"}}),vec![ids.tool],Value::Null),
        draft(ids.python_code,22,json!({"name":"pc-exec-http-get","description":"Typed bounded GET export; no source substitution or workflow side effects.",
            "content":BOUNDED_GET_SOURCE,"input_contract":inputs,"result_contract":result,"preload":preload}),vec![],Value::Null),
        draft(ids.skill,1,json!({"body":prose,"interface":{"format":"skill-interface/1","python_code_uuid":ids.python_code,
            "exports":preload["exports"],"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![ids.python_code,ids.tool_skill,ids.tool],json!(association)),
        draft(ids.recipe,21,json!({"name":"http-get","description":"Canonical bounded-GET variant; merge/qualify existing variants before coherent activation.",
            "variants":[{"variant_key":"bounded","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":variables}],
            "step_descriptions":[{"desc_idx":0,"label":"Bounded HTTP GET","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"rust","goal":"Prepare the existing HTTP binding","content":"","type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":"http","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Invoke pinned bounded GET","content":"","type":"component","include":[ids.python_code]}]}],
            "input_layouts":{"bounded":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":{"0:2":bindings}}},
            "invocations":{"bounded":{"0:2":export}}}),vec![ids.tool_skill,ids.python_code,ids.skill],Value::Null),
    ].into_iter().collect()
}
fn draft(
    id: Uuid,
    class: i32,
    document: Value,
    dependencies: Vec<Uuid>,
    association: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,"class_code":class,
        "document":document,"dependencies":dependencies,"association":association})
        .to_string(),
    )
}
