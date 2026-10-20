//! One reusable JSON-Tool usage. Packaged drafts confer no approval or grant.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};
use serde_json::{Value, json};

/// Shared canonical identities; consumers reuse the same usage, not copies.
pub fn usage_ids(
    tool: uuid::Uuid,
    mut allocate: impl FnMut(&str) -> uuid::Uuid,
) -> UsageComponentIds {
    UsageComponentIds {
        recipe: allocate("utf8-digest:recipe"),
        python_code: allocate("utf8-digest:code"),
        skill: allocate("utf8-digest:skill"),
        tool_skill: allocate("utf8-digest:binding"),
        tool,
        formatter: Some(allocate("utf8-digest:argument-parser")),
    }
}

pub fn utf8_digest_usage_drafts(
    ids: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let parser = ids.formatter.ok_or(RevisionError::Invalid(
        "canonical digest Recipe requires a JSON-list parser",
    ))?;
    let identities = [
        ids.recipe,
        ids.python_code,
        ids.tool_skill,
        ids.tool,
        ids.skill,
        parser,
    ];
    if identities.iter().any(uuid::Uuid::is_nil)
        || identities
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 6
    {
        return Err(RevisionError::Invalid(
            "digest usage needs six distinct identities",
        ));
    }
    let list =
        json!({"type":"list","required":true,"checks":[],"items":{"type":"string","checks":[]}});
    let inputs = json!({"texts":list});
    let result = json!({"type":"object","allow_extra_fields":false,"fields":{"digests":list}});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let preload = json!({"format":"python-preload/2","exports":{"digest_utf8_texts":{"symbol":"digest_utf8_texts","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"digest_utf8_texts"});
    let association=json!({"format":"skill-association/1","skill_uuid":ids.skill,"python_code_uuid":ids.python_code,"tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,"callable":"host.json","inputs":inputs,"arguments":{},"code_arguments":{"data":{"type":"string","checks":[],"depends_on":["texts"],"meaning":"Exact ordered text list serialized as compact JSON without ASCII normalization; the native operation decodes it before hashing UTF-8 strings"},"operation":{"type":"string","checks":[],"depends_on":[],"meaning":"Fixed sha256_utf8 operation over the ordered complete text list"}},"result":result,"failure":failure}).to_string();
    let interface = json!({"format":"skill-interface/1","python_code_uuid":ids.python_code,"exports":preload["exports"],"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]});
    let rows = vec![
        (
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,"callable":"host.json","capability_id":"builtin.json"}}),
            vec![ids.tool],
            Value::Null,
        ),
        (
            ids.python_code,
            22,
            json!({"name":"digest-utf8-texts","content":"def digest_utf8_texts(inputs):\n    import json\n    data = json.dumps(inputs['texts'], ensure_ascii=False, separators=(',', ':'))\n    return host.json(operation='sha256_utf8', data=data)\n","input_contract":inputs,"result_contract":result,"preload":preload}),
            vec![],
            Value::Null,
        ),
        (
            ids.skill,
            1,
            json!({"body":format!("Hash one ordered batch of at most 8192 Unicode strings as exact UTF-8 SHA-256 through host.json. Preserve order, duplicate strings, newlines and Unicode code points; perform no normalization. Return exactly digests:list[string], one lowercase hexadecimal digest per input. Empty batches are valid. Associated export digest_utf8_texts(inputs) serializes the ordered list as JSON text for the shared JSON Tool string transport, fixes operation sha256_utf8 and dispatches once. Loading executes nothing. Canonical execution Recipe {} uses binding {} then code {}. This reusable usage performs no source selection, file access, package construction, provider calls, quality approval or activation. Current Tool policy and retained native identity still apply. Stop on invalid types, capacity, cancellation, fencing, policy block, backend or output failure; one dispatch, no automatic retries or Tier-2 fallback. Qualification must verify known SHA-256 vectors, byte distinctions, ordering, duplicates and invalid inputs through the retained registered path.",ids.recipe,ids.tool_skill,ids.python_code),"interface":interface}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        ),
        (
            parser,
            22,
            json!({"name":"parse-utf8-text-list","content":"def parse_utf8_texts(inputs):\n    import json\n    texts = json.loads(inputs['texts_json'])\n    if not isinstance(texts, list) or len(texts) > 8192 or any(not isinstance(text, str) for text in texts):\n        raise ValueError('bounded JSON string list required')\n    return texts\n","input_contract":{"texts_json":{"type":"string","required":true,"checks":[]}},"result_contract":{"type":"list","items":{"type":"string"}},"preload":{"format":"python-preload/2","exports":{"parse_utf8_texts":{"symbol":"parse_utf8_texts","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"parse_utf8_texts"}}),
            vec![],
            Value::Null,
        ),
        (
            ids.recipe,
            21,
            json!({"name":"digest-utf8-texts","description":"Canonical Tier-0 JSON-list command to exact UTF-8 digest batch usage.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["digest utf8 json=%","please digest utf8 json=%","now digest utf8 json=%","please now digest utf8 json=%","execute digest utf8 json=%","run digest utf8 json=%","please execute digest utf8 json=%","please run digest utf8 json=%","perform digest utf8 json=%","please perform digest utf8 json=%"],"variable_patterns":[{"name":"texts_json","pattern":null,"description":null}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Parse and digest one batch","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"orchestrator","goal":"Parse JSON command data without executing it","content":"","type":"component","include":[parser]},
                {"stepnumber":2,"knowledge":"rust","goal":"Bind JSON primitive","content":"","type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":"json","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":3,"knowledge":"orchestrator","goal":"Hash exact ordered texts","content":"","type":"component","include":[ids.python_code]}]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"texts_json":{"type":"string","required":true,"checks":[]}},"steps":{"0:1":{"texts_json":{"kind":"task_input","reference":"{{vars.texts_json}}"}},"0:3":{"texts":{"kind":"result","step_id":"0:1","path":[]}}}}},"invocations":{"selected":{"0:1":"parse_utf8_texts","0:3":"digest_utf8_texts"}}}),
            vec![parser, ids.skill, ids.tool_skill, ids.python_code],
            Value::Null,
        ),
    ];
    rows.into_iter().map(|(id,class,document,dependencies,association)| ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":class,"document":document,"dependencies":dependencies,"association":association}).to_string())).collect()
}

/// Common typed JSON-text transport, compatible with parse/query/stringify/
/// validate and digest operations. The native primitive also accepts structured
/// values, but usages need not introduce incompatible per-operation Tool rows.
pub fn json_text_tool_draft(
    id: uuid::Uuid,
    native_identity: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":0,
        "document":{"name":"json","capability_id":"builtin.json","callable":"host.json",
        "input_contract":{"operation":{"type":"string","required":true,"checks":[]},"data":{"type":"string","required":true,"checks":[]},"path":{"type":"string","required":false,"checks":[]}},
        "implementation":{"format":"native-first-party/1","identity":native_identity}},"dependencies":[],"association":null}).to_string())
}
