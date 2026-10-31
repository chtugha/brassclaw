//! Registered-source variant of the existing HTTP diagnostic Recipe.
//! Drafts are not approval, admission, availability publication or activation.
use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    global_bootstrap_components::UsageComponentIds,
    prefix_source_check_components::{HttpSourceCheckIds, http_source_check_drafts},
    prefix_source_components::{SourceUsage, source_usage_drafts},
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use uuid::Uuid;

pub const PARSE: &str = r#"def parse_registered_source_check(inputs):
    import json
    value = json.loads(inputs['request_json'])
    if not isinstance(value, dict) or sorted(value) != ['response_body_limit', 'source', 'timeout_ms']:
        raise ValueError('exact registered-source check request required')
    if json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')) != inputs['request_json']:
        raise ValueError('canonical registered-source check request required')
    return value
"#;
pub const PREPARE: &str = r#"def prepare_registered_source_check(inputs):
    import json
    detail = inputs['detail']
    if detail['source'] != inputs['source']:
        raise ValueError('registration read differs from pinned source')
    registration = json.loads(detail['registration_bytes'])
    if not isinstance(registration, dict) or registration.get('format') != 'brassclaw-prefix-source-registration/1':
        raise ValueError('supported native registration required')
    if registration.get('profile') != inputs['source']['profile'] or registration.get('kind') != 'web':
        raise ValueError('pinned web registration required before HTTP dispatch')
    url = registration.get('reference')
    if not isinstance(url, str) or not url.strip():
        raise ValueError('registered web reference required')
    return {'url': url, 'expected_sha256': None}
"#;
pub const PACK: &str = r#"def pack_registered_source_diagnostic(inputs):
    import json
    classification = inputs['classification']
    if classification['expected_sha256'] is not None or classification['availability'] == 'available':
        raise ValueError('uncollected registration has no retained content baseline')
    response = inputs['response']
    digest = inputs['digest']
    if len(digest) != 64 or any(ch not in '0123456789abcdef' for ch in digest):
        raise ValueError('exact response-text digest required')
    text = 'body_text' in response
    binary = 'body_base64' in response
    representation = 'invalid' if text == binary else ('text' if text else 'binary')
    value = {'format': 'brassclaw-registered-source-diagnostic/1',
             'source': inputs['source'], 'url': inputs['prepared']['url'],
             'classification': classification,
             'response': {'status': response['status'], 'request_bytes': response['request_bytes'],
                          'response_bytes': response['response_bytes'],
                          'redaction_applied': response['redaction_applied'],
                          'representation': representation,
                          'text_sha256': digest if representation == 'text' else None},
             'qualification': 'unqualified; no availability publication, collection, review or activation'}
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
"#;
pub const RECEIPT: &str = r#"def registered_source_check_receipt(inputs):
    receipt = inputs['receipt']
    if receipt['qualification'] != 'unqualified':
        raise ValueError('unqualified native storage receipt required')
    return {'source': inputs['source'], 'diagnostic_bytes': inputs['diagnostic_bytes'],
            'storage_receipt': receipt,
            'qualification': 'retained-unqualified-diagnostic; not trusted availability or knowledge approval'}
"#;

#[derive(Clone, Copy)]
pub struct RegisteredSourceCheckIds {
    pub check: HttpSourceCheckIds,
    pub read: UsageComponentIds,
    pub retain: UsageComponentIds,
    pub parse: Uuid,
    pub prepare: Uuid,
    pub pack: Uuid,
    pub receipt: Uuid,
}
fn text() -> Value {
    json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]})
}
fn input(mut schema: Value) -> Value {
    schema["required"] = json!(true);
    if schema.get("checks").is_none() {
        schema["checks"] = json!([]);
    }
    schema
}
fn document(drafts: &[ComponentRevisionDraft], id: Uuid) -> Result<&Value, RevisionError> {
    drafts
        .iter()
        .find(|d| d.uuid() == id)
        .map(|d| d.document())
        .ok_or(RevisionError::Invalid("registered source usage missing"))
}
fn storage_binding(
    drafts: &[ComponentRevisionDraft],
    usage: UsageComponentIds,
    number: usize,
) -> Result<Value, RevisionError> {
    let steps = document(drafts, usage.recipe)?["step_descriptions"][0]["steps"]
        .as_array()
        .ok_or(RevisionError::Invalid("canonical storage steps missing"))?;
    let matches: Vec<_> = steps
        .iter()
        .filter(|step| step["knowledge"] == "rust" && step["include"] == json!([usage.tool_skill]))
        .collect();
    let [step] = matches.as_slice() else {
        return Err(RevisionError::Invalid(
            "exactly one canonical storage binding required",
        ));
    };
    let mut step = (*step).clone();
    step["stepnumber"] = json!(number);
    Ok(step)
}
fn pure(
    id: Uuid,
    name: &str,
    source: &str,
    imports: &[&str],
    inputs: Value,
    result: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    InputContract::from_value(&inputs, REVISION_LIMITS)?;
    ValueContract::result_from_value(&result, REVISION_LIMITS)?;
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":name,"description":"Pure registered-source diagnostic handoff. Native read/digest/storage custody remains authoritative; no fetch, hashing, approval or availability publication.",
        "content":source,"input_contract":inputs,"result_contract":result,
        "preload":{"format":"python-preload/2","exports":{name:{"symbol":name,"parameters":["inputs"],"mapping":true}},
            "private_functions":{},"constants":[],"imports":imports,"dependencies":[],"default_export":name}},
        "dependencies":[],"association":null}).to_string())
}

/// Keep the existing `selected` diagnostic and add its full `registered` variant.
/// URL and returned identity originate only in the immutable native read result.
/// This variant deliberately has no caller-supplied expected content hash.
pub fn registered_source_check_drafts(
    ids: RegisteredSourceCheckIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    if ids.read.tool != ids.retain.tool {
        return Err(RevisionError::Invalid(
            "reuse one registered-source Tool identity",
        ));
    }
    let mut drafts = http_source_check_drafts(ids.check)?;
    drafts.extend(source_usage_drafts(ids.read, SourceUsage::Read)?);
    drafts.extend(source_usage_drafts(
        ids.retain,
        SourceUsage::RetainObservation,
    )?);
    let mut unique = std::collections::BTreeSet::new();
    for id in drafts
        .iter()
        .map(|d| d.uuid())
        .chain([ids.parse, ids.prepare, ids.pack, ids.receipt])
    {
        if id.is_nil()
            || !unique.insert(id)
            || [ids.read.tool, ids.check.http.tool, ids.check.digest.tool].contains(&id)
        {
            return Err(RevisionError::Invalid(
                "distinct registered-source graph identities required",
            ));
        }
    }
    if ids.read.tool == ids.check.http.tool || ids.read.tool == ids.check.digest.tool {
        return Err(RevisionError::Invalid(
            "distinct native source/HTTP/digest identities required",
        ));
    }
    let read = document(&drafts, ids.read.python_code)?.clone();
    let retain = document(&drafts, ids.retain.python_code)?.clone();
    let http = document(&drafts, ids.check.http.python_code)?.clone();
    let classifier = document(&drafts, ids.check.classifier)?.clone();
    let source = read["input_contract"]["source"].clone();
    let request_fields = json!({"source":source,"response_body_limit":http["input_contract"]["response_body_limit"],"timeout_ms":http["input_contract"]["timeout_ms"]});
    let request =
        json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":request_fields});
    let prepared = json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "url":http["input_contract"]["url"],"expected_sha256":classifier["input_contract"]["expected_sha256"]}});
    let preparation_inputs =
        json!({"source":source,"detail":input(read["result_contract"].clone())});
    ValueContract::result_from_value(&read["result_contract"], REVISION_LIMITS)?
        .require_compatible_input(
            &InputContract::from_value(&preparation_inputs, REVISION_LIMITS)?,
            "detail",
        )?;
    drafts.push(pure(
        ids.parse,
        "parse_registered_source_check",
        PARSE,
        &["json"],
        json!({"request_json":text()}),
        request,
    )?);
    drafts.push(pure(
        ids.prepare,
        "prepare_registered_source_check",
        PREPARE,
        &["json"],
        preparation_inputs,
        prepared.clone(),
    )?);
    let digest_schema =
        input(document(&drafts, ids.check.digest_value)?["result_contract"].clone());
    drafts.push(pure(ids.pack,"pack_registered_source_diagnostic",PACK,&["json"],json!({"source":source,"prepared":input(prepared),
        "response":input(http["result_contract"].clone()),"classification":input(classifier["result_contract"].clone()),
        "digest":digest_schema}),
        json!({"type":"string","checks":[{"kind":"min_length","value":1}]}))?);
    drafts.push(pure(ids.receipt,"registered_source_check_receipt",RECEIPT,&[],
        json!({"source":source,"diagnostic_bytes":text(),"receipt":input(retain["result_contract"].clone())}),
        json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
            "source":source,"diagnostic_bytes":text(),"storage_receipt":input(retain["result_contract"].clone()),"qualification":text()}}))?);
    let position = drafts
        .iter()
        .position(|d| d.uuid() == ids.check.recipe)
        .ok_or(RevisionError::Invalid("existing diagnostic Recipe missing"))?;
    let recipe = drafts.remove(position);
    let mut doc = recipe.document().clone();
    let mut steps = Vec::new();
    let component = |step, id, goal| json!({"stepnumber":step,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[id]});
    steps.push(component(
        1,
        ids.parse,
        "Parse typed diagnostic request without source substitution",
    ));
    steps.push(storage_binding(&drafts, ids.read, 2)?);
    steps.push(component(
        3,
        ids.read.python_code,
        "Read the exact immutable source registration",
    ));
    steps.push(component(
        4,
        ids.prepare,
        "Resolve the stored web reference before network effects",
    ));
    let offset = steps.len();
    let source_step = |export: &str| -> Result<usize, RevisionError> {
        doc["invocations"]["selected"]
            .as_object()
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|(_, value)| value.as_str() == Some(export))
            })
            .and_then(|(key, _)| key.strip_prefix("0:"))
            .and_then(|number| number.parse::<usize>().ok())
            .map(|number| number + offset)
            .ok_or(RevisionError::Invalid("required diagnostic export missing"))
    };
    let response_step = source_step("fetch_http_bounded")?;
    let digest_step = source_step("single_utf8_digest")?;
    let classification_step = source_step("classify_exact_http_source")?;
    let old_steps = doc["step_descriptions"][0]["steps"]
        .as_array()
        .ok_or(RevisionError::Invalid("diagnostic steps missing"))?;
    for step in old_steps {
        let mut step = step.clone();
        let number = step["stepnumber"]
            .as_u64()
            .ok_or(RevisionError::Invalid("diagnostic step number missing"))?;
        step["stepnumber"] = json!(number + offset as u64);
        steps.push(step);
    }
    let pack_step = steps.len() + 1;
    steps.push(component(
        pack_step,
        ids.pack,
        "Serialize bounded metadata; complete response stays in actual execution evidence",
    ));
    steps.push(storage_binding(&drafts, ids.retain, pack_step + 1)?);
    steps.push(component(
        pack_step + 2,
        ids.retain.python_code,
        "Append exact unqualified diagnostic bytes",
    ));
    steps.push(component(
        pack_step + 3,
        ids.receipt,
        "Return the original unqualified storage receipt",
    ));
    let result =
        |step, path: Vec<&str>| json!({"kind":"result","step_id":format!("1:{step}"),"path":path});
    let mut layouts = json!({"1:1":{"request_json":{"kind":"task_input","reference":"{{vars.request_json}}"}},
        "1:3":{"source":result(1,vec!["source"])},"1:4":{"source":result(1,vec!["source"]),"detail":result(3,vec![])}});
    let mut invocations = json!({"1:1":"parse_registered_source_check","1:3":"read_registered_source","1:4":"prepare_registered_source_check"});
    for (key, value) in doc["invocations"]["selected"]
        .as_object()
        .ok_or(RevisionError::Invalid("diagnostic invocations missing"))?
    {
        let number = key
            .strip_prefix("0:")
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or(RevisionError::Invalid("diagnostic reference invalid"))?;
        let key = format!("1:{}", number + offset);
        invocations[&key] = value.clone();
        let mut values = doc["input_layouts"]["selected"]["steps"][format!("0:{number}")].clone();
        for (name, edge) in values
            .as_object_mut()
            .ok_or(RevisionError::Invalid("diagnostic input mapping missing"))?
        {
            if edge["kind"] == "result" {
                let reference = edge["step_id"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("0:"))
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or(RevisionError::Invalid(
                        "diagnostic result reference invalid",
                    ))?;
                edge["step_id"] = json!(format!("1:{}", reference + offset));
            } else if edge["kind"] == "task_input" {
                *edge = result(
                    match name.as_str() {
                        "response_body_limit" | "timeout_ms" => 1,
                        "url" | "expected_sha256" => 4,
                        _ => {
                            return Err(RevisionError::Invalid(
                                "unsupported diagnostic task input",
                            ));
                        }
                    },
                    vec![name],
                );
            } else {
                return Err(RevisionError::Invalid("unsupported diagnostic input edge"));
            }
        }
        layouts[&key] = values;
    }
    let pack_key = format!("1:{pack_step}");
    let retain_key = format!("1:{}", pack_step + 2);
    let receipt_key = format!("1:{}", pack_step + 3);
    layouts[&pack_key] = json!({"source":result(3,vec!["source"]),"prepared":result(4,vec![]),"response":result(response_step,vec![]),
        "classification":result(classification_step,vec![]),"digest":result(digest_step,vec![])});
    layouts[&retain_key] =
        json!({"source":result(3,vec!["source"]),"diagnostic_bytes":result(pack_step,vec![])});
    layouts[&receipt_key] = json!({"source":result(3,vec!["source"]),"diagnostic_bytes":result(pack_step,vec![]),"receipt":result(pack_step+2,vec![])});
    invocations[&pack_key] = json!("pack_registered_source_diagnostic");
    invocations[&retain_key] = json!("retain_source_observation");
    invocations[&receipt_key] = json!("registered_source_check_receipt");
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
    .map(|lead| format!("{lead}check registered prefix source json=%"))
    .collect();
    doc["description"] = json!(
        "Exact HTTP diagnostics; the registered variant reads a pinned registration and retains unqualified diagnostics through the existing native source usage. No job admission or availability publication."
    );
    doc["variants"].as_array_mut().ok_or(RevisionError::Invalid("diagnostic variants missing"))?.push(json!({"variant_key":"registered","step_link":"1:1-1:E","intent_examples":examples,"variable_patterns":[{"name":"request_json","pattern":null,"description":"Canonical pinned source identity and explicit HTTP technical bounds; no URL/hash override"}]}));
    doc["step_descriptions"].as_array_mut().ok_or(RevisionError::Invalid("diagnostic descriptions missing"))?.push(json!({"desc_idx":1,"label":"Check and retain one registered web-source diagnostic","yaml_source":"","steps":steps}));
    doc["input_layouts"]["registered"] = json!({"format":"recipe-input-layout/1","task_inputs":{"request_json":text()},"steps":layouts});
    doc["invocations"]["registered"] = invocations;
    let mut dependencies = recipe.dependencies().clone();
    dependencies.extend([
        ids.parse,
        ids.prepare,
        ids.pack,
        ids.receipt,
        ids.read.skill,
        ids.read.tool_skill,
        ids.read.python_code,
        ids.retain.skill,
        ids.retain.tool_skill,
        ids.retain.python_code,
    ]);
    drafts.push(ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.check.recipe,"class_code":21,"document":doc,"dependencies":dependencies,"association":null}).to_string())?);
    Ok(drafts)
}
