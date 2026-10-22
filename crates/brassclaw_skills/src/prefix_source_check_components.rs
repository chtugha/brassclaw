//! Pure source-check building blocks. No fetch, digest, storage, approval or
//! activation is hidden here. A qualified Recipe must supply real HTTP/digest
//! results and retain their exact provenance before projecting availability.
use crate::component_revision::{ComponentRevisionDraft, RevisionError};
use crate::global_bootstrap_components::UsageComponentIds;
use serde_json::{Value, json};
use uuid::Uuid;

pub const TEXT_SOURCE: &str = "def http_reference_text(inputs):\n    response = inputs['response']\n    return response['body_text'] if 'body_text' in response else ''\n";
pub const CLASSIFY_SOURCE: &str = r#"def classify_exact_http_source(inputs):
    response = inputs['response']
    expected = inputs['expected_sha256']
    digest = inputs['observed_sha256']
    if len(digest) != 64 or any(ch not in '0123456789abcdef' for ch in digest):
        raise ValueError('exact response digest required')
    if expected is not None and (len(expected) != 64 or any(ch not in '0123456789abcdef' for ch in expected)):
        raise ValueError('exact original digest required')
    result = {'availability': 'check-error', 'reason': 'unsupported-http-status',
              'content_identity': 'not-checked', 'expected_sha256': expected,
              'observed_sha256': None}
    status = response['status']
    if status in (404, 410):
        result['availability'] = 'unavailable'
        result['reason'] = 'missing'
    elif status in (401, 403):
        result['availability'] = 'unavailable'
        result['reason'] = 'authentication-required'
    elif status == 429:
        result['reason'] = 'rate-limited'
    elif 300 <= status < 400:
        result['reason'] = 'redirect-not-followed'
    elif 500 <= status < 600:
        result['reason'] = 'server-error'
    elif 400 <= status < 500:
        result['availability'] = 'unavailable'
        result['reason'] = 'http-error'
    elif 200 <= status < 300:
        text = 'body_text' in response
        binary = 'body_base64' in response
        if text == binary:
            result['reason'] = 'invalid-response-representation'
        elif binary:
            result['reason'] = 'unsupported-content-representation'
        else:
            result['observed_sha256'] = digest
            if not response['body_text'].strip():
                result['availability'] = 'unavailable'
                result['reason'] = 'empty-content'
            elif expected is None:
                result['reason'] = 'content-identity-not-established'
                result['content_identity'] = 'unverified'
            elif expected != digest:
                result['reason'] = 'content-changed'
                result['content_identity'] = 'changed'
            else:
                result['availability'] = 'available'
                result['reason'] = 'exact-content'
                result['content_identity'] = 'exact-original-match'
    return result
"#;

fn response_input() -> Value {
    let mut value = crate::http_get_components::http_response_schema();
    value["required"] = json!(true);
    value
}
fn digest(nullable: bool, required: bool) -> Value {
    json!({"type":"string","nullable":nullable,"required":required,"checks":[]})
}
fn preload(export: &str) -> Value {
    json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":export})
}
fn draft(
    id: Uuid,
    name: &str,
    description: &str,
    source: &str,
    export: &str,
    inputs: Value,
    result: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    if id.is_nil() {
        return Err(RevisionError::Invalid(
            "source check component identity required",
        ));
    }
    crate::value_contract::InputContract::from_value(
        &inputs,
        crate::component_revision::REVISION_LIMITS,
    )?;
    crate::value_contract::ValueContract::result_from_value(
        &result,
        crate::component_revision::REVISION_LIMITS,
    )?;
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":name,"description":description,"content":source,
            "input_contract":inputs,"result_contract":result,"preload":preload(export)},
        "dependencies":[],"association":null})
        .to_string(),
    )
}
/// Return the complete text representation for a separate hashing usage. Empty
/// string is a placeholder for unsupported binary data, never a content proof.
pub fn response_text_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    draft(
        id,
        "http-reference-text",
        "Project the complete native body_text without normalization. Binary responses produce an empty hashing placeholder; the separate classifier must reject binary representation before using any text digest. No network, hash, source decision or storage effect.",
        TEXT_SOURCE,
        "http_reference_text",
        json!({"response":response_input()}),
        json!({"type":"string","checks":[]}),
    )
}
pub fn exact_content_classifier_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let result = json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "availability":{"type":"string","required":true,"checks":[]},
        "reason":{"type":"string","required":true,"checks":[]},
        "content_identity":{"type":"string","required":true,"checks":[]},
        "expected_sha256":digest(true,true),"observed_sha256":digest(true,true)}});
    draft(
        id,
        "classify-exact-http-source",
        "Classify one real bounded HTTP response against an independently retained original SHA-256. The consuming Recipe supplies actual response and UTF-8 digest results in separate steps; this function neither hashes nor proves their provenance. Exact nonempty text/hash equality means available content, never knowledge approval. HTTP 200 alone, changed content, unknown expected identity, unsupported binary representation and redirects cannot yield green. Missing/authentication failures are unavailable; server/rate/identity/representation failures are neutral check errors. No normalization, hidden fetch, retry, approval, source selection or observation write. A redacted header does not invalidate an independently exact body hash. Raw response/effect evidence remains retained separately.",
        CLASSIFY_SOURCE,
        "classify_exact_http_source",
        json!({"response":response_input(),
            "expected_sha256":digest(true,true),"observed_sha256":digest(false,true)}),
        result,
    )
}

/// References supplied by the existing component owner. HTTP and JSON remain
/// shared usages; this workflow introduces no network/storage primitive.
#[derive(Clone, Copy)]
pub struct HttpSourceCheckIds {
    pub recipe: Uuid,
    pub preflight: Uuid,
    pub texts: Uuid,
    pub digest_value: Uuid,
    pub classifier: Uuid,
    pub http: UsageComponentIds,
    pub digest: UsageComponentIds,
}

/// Fetch -> complete text projection -> UTF-8 hash -> exact classification.
/// This returns diagnostic data only. An admitting owner must pin the source
/// and qualify/retain actual effects before writing an availability observation.
/// The expected hash is typed input, never an approval or provenance claim.
pub fn http_source_check_drafts(
    ids: HttpSourceCheckIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut identities = vec![
        ids.recipe,
        ids.preflight,
        ids.texts,
        ids.digest_value,
        ids.classifier,
        ids.http.recipe,
        ids.http.python_code,
        ids.http.skill,
        ids.http.tool_skill,
        ids.http.tool,
        ids.digest.recipe,
        ids.digest.python_code,
        ids.digest.skill,
        ids.digest.tool_skill,
        ids.digest.tool,
    ];
    identities.extend(ids.http.formatter);
    identities.extend(ids.digest.formatter);
    if identities.iter().any(Uuid::is_nil)
        || identities
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != identities.len()
    {
        return Err(RevisionError::Invalid(
            "distinct source-check graph identities required",
        ));
    }
    let string_values = json!({"type":"list","checks":[],"items":{"type":"string","checks":[]}});
    let mut strings = string_values.clone();
    strings["required"] = json!(true);
    let mut drafts = crate::http_get_components::bounded_http_get_drafts(ids.http)?;
    drafts.extend(crate::utf8_digest_components::utf8_digest_usage_drafts(
        ids.digest,
    )?);
    drafts.push(draft(ids.texts, "http-reference-texts", "Project exactly one complete text for the existing batch digest usage; binary data has an empty placeholder and remains rejected by the classifier.",
        "def http_reference_texts(inputs):\n    response = inputs['response']\n    return [response['body_text'] if 'body_text' in response else '']\n", "http_reference_texts",
        json!({"response":response_input()}), string_values)?);
    drafts.push(draft(ids.digest_value, "single-utf8-digest", "Validate the single returned digest without changing it; no hashing or effects.",
        "def single_utf8_digest(inputs):\n    values = inputs['digests']\n    if len(values) != 1:\n        raise ValueError('exactly one digest required')\n    return values[0]\n", "single_utf8_digest",
        json!({"digests":strings}), json!({"type":"string","checks":[]}))?);
    drafts.push(exact_content_classifier_draft(ids.classifier)?);
    drafts.push(draft(ids.preflight, "validate-original-digest", "Reject malformed expected identity before any network effect; null explicitly means identity is unknown.",
        "def validate_original_digest(inputs):\n    value = inputs['expected_sha256']\n    if value is not None and (len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value)):\n        raise ValueError('exact original digest required')\n    return value\n", "validate_original_digest",
        json!({"expected_sha256":digest(true,true)}), json!({"type":"string","nullable":true,"checks":[]}))?);
    let mut task_inputs = drafts
        .iter()
        .find(|d| d.uuid() == ids.http.python_code)
        .ok_or(RevisionError::Invalid("HTTP usage missing"))?
        .document()["input_contract"]
        .clone();
    task_inputs["expected_sha256"] = digest(true, true);
    let component = |step, id, goal| json!({"stepnumber":step,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[id]});
    let binding = |step, usage: UsageComponentIds, name| json!({"stepnumber":step,"knowledge":"rust","goal":"Prepare retained usage binding","content":"","type":"component","include":[usage.tool_skill],"tool_bindings":[{"tool_id":usage.tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]});
    let result =
        |step, path: Vec<&str>| json!({"kind":"result","step_id":format!("0:{step}"),"path":path});
    let mut http_inputs = serde_json::Map::new();
    for name in ["url", "response_body_limit", "timeout_ms"] {
        http_inputs.insert(
            name.into(),
            json!({"kind":"task_input","reference":format!("{{{{vars.{name}}}}}")}),
        );
    }
    let steps = vec![
        component(
            1,
            ids.preflight,
            "Validate expected identity before network effects",
        ),
        binding(2, ids.http, "http"),
        component(
            3,
            ids.http.python_code,
            "Fetch one bounded complete response",
        ),
        component(4, ids.texts, "Project complete text without normalization"),
        binding(5, ids.digest, "json"),
        component(
            6,
            ids.digest.python_code,
            "Hash the actual complete response text",
        ),
        component(7, ids.digest_value, "Require exactly one digest"),
        component(
            8,
            ids.classifier,
            "Classify content identity without approval or storage",
        ),
    ];
    let layouts = json!({"0:1":{"expected_sha256":{"kind":"task_input","reference":"{{vars.expected_sha256}}"}},
        "0:3":http_inputs,"0:4":{"response":result(3,vec![])},"0:6":{"texts":result(4,vec![])},
        "0:7":{"digests":result(6,vec!["digests"])},"0:8":{"response":result(3,vec![]),"observed_sha256":result(7,vec![]),
            "expected_sha256":result(1,vec![])}});
    let pattern = "check exact http source url=% max-bytes=% timeout-ms=% expected-sha256=%";
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
    let variables: Vec<_> = ["url","response_body_limit","timeout_ms","expected_sha256"].iter().map(|name|json!({"name":name,"pattern":null,"description":"Typed diagnostic input; not source approval"})).collect();
    drafts.push(ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.recipe,"class_code":21,
        "document":{"name":"check-exact-http-source","description":"Model-free retained HTTP/digest diagnostics; source pinning, observation retention and job qualification remain separate.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":variables}],
            "step_descriptions":[{"desc_idx":0,"label":"Check exact HTTP content","yaml_source":"","steps":steps}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":task_inputs,"steps":layouts}},
            "invocations":{"selected":{"0:1":"validate_original_digest","0:3":"fetch_http_bounded","0:4":"http_reference_texts","0:6":"digest_utf8_texts","0:7":"single_utf8_digest","0:8":"classify_exact_http_source"}}},
        "dependencies":[ids.preflight,ids.http.skill,ids.http.tool_skill,ids.http.python_code,ids.digest.skill,ids.digest.tool_skill,ids.digest.python_code,ids.texts,ids.digest_value,ids.classifier],"association":null}).to_string())?);
    Ok(drafts)
}
