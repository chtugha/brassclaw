//! Reusable registered-source storage usages and canonical execution Recipes.
//! Drafts are neither approval nor activation; they never fetch or classify.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum SourceUsage {
    Read,
    RetainObservation,
}
fn string() -> Value {
    json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]})
}
pub fn source_identity_schema() -> Value {
    json!({"type":"object","required":true,"allow_extra_fields":false,"checks":[],"fields":{
        "profile":string(),"source_id":string(),"registration_sha256":string(),
        "revision":{"type":"integer","required":true,"checks":[{"kind":"min","value":1},{"kind":"max","value":9007199254740991i64}]}}})
}
pub fn source_tool_draft(
    id: Uuid,
    native_identity: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let mut optional = string();
    optional["required"] = json!(false);
    make(
        id,
        0,
        json!({"name":"prefix-source","capability_id":"builtin.prefix_source","callable":"host.prefix_source",
        "input_contract":{"operation":string(),"source":source_identity_schema(),"diagnostic_bytes":optional},
        "implementation":{"format":"native-first-party/1","identity":native_identity}}),
        vec![],
        Value::Null,
    )
}
fn preload(export: &str, imports: Vec<&str>) -> Value {
    json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":imports,"dependencies":[],"default_export":export})
}
pub fn source_usage_drafts(
    ids: UsageComponentIds,
    usage: SourceUsage,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let parser = ids.formatter.ok_or(RevisionError::Invalid(
        "source command requires canonical JSON parser",
    ))?;
    let unique = std::collections::BTreeSet::from([
        ids.recipe,
        ids.tool,
        ids.tool_skill,
        ids.skill,
        ids.python_code,
        parser,
    ]);
    if unique.len() != 6 || unique.contains(&Uuid::nil()) {
        return Err(RevisionError::Invalid(
            "distinct source usage identities required",
        ));
    }
    let retain = matches!(usage, SourceUsage::RetainObservation);
    let operation = if retain { "retain_observation" } else { "read" };
    let export = if retain {
        "retain_source_observation"
    } else {
        "read_registered_source"
    };
    let parse = if retain {
        "parse_source_observation_request"
    } else {
        "parse_source_read_request"
    };
    let mut inputs = json!({"source":source_identity_schema()});
    if retain {
        inputs["diagnostic_bytes"] = string();
    }
    let result = if retain {
        json!({"type":"object","allow_extra_fields":false,"fields":{
        "observation_id":string(),"checksum":string(),"qualification":string()}})
    } else {
        json!({"type":"object","allow_extra_fields":false,"fields":{"source":source_identity_schema(),"registration_bytes":string()}})
    };
    let mut arguments = json!({"source":"source"});
    if retain {
        arguments["diagnostic_bytes"] = json!("diagnostic_bytes");
    }
    let source = if retain {
        "def retain_source_observation(inputs):\n    return host.prefix_source(operation='retain_observation', source=inputs['source'], diagnostic_bytes=inputs['diagnostic_bytes'])\n"
    } else {
        "def read_registered_source(inputs):\n    return host.prefix_source(operation='read', source=inputs['source'])\n"
    };
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association=json!({"format":"skill-association/1","skill_uuid":ids.skill,"python_code_uuid":ids.python_code,"tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,
        "callable":"host.prefix_source","inputs":inputs,"arguments":arguments,
        "code_arguments":{"operation":{"type":"string","checks":[{"kind":"min_length","value":1}],"depends_on":[],"meaning":format!("Fixed {operation} storage operation; no workflow choice")}},"result":result,"failure":failure}).to_string();
    let interface = json!({"format":"skill-interface/1","python_code_uuid":ids.python_code,"exports":preload(export,vec![])["exports"],"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]});
    let parser_source = format!(
        "def {parse}(inputs):\n    import json\n    value = json.loads(inputs['request_json'])\n    if json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')) != inputs['request_json']:\n        raise ValueError('canonical source request required')\n    return value\n"
    );
    let parser_result = json!({"type":"object","allow_extra_fields":false,"fields":inputs});
    let task = json!({"request_json":string()});
    let mut links = serde_json::Map::new();
    for key in inputs
        .as_object()
        .ok_or(RevisionError::Invalid("source inputs required"))?
        .keys()
    {
        links.insert(
            key.clone(),
            json!({"kind":"result","step_id":"0:1","path":[key]}),
        );
    }
    let pattern = if retain {
        "retain prefix source diagnostic json=%"
    } else {
        "read prefix source json=%"
    };
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
    let prose = format!(
        "Purpose\nPerform one {operation} operation on the existing immutable registered-source store.\n\nInterface\nAssociated PythonCode {} preloads {export}(inputs). Tool {} and ToolSkill {} bind host.prefix_source; canonical execution Recipe {} parses canonical JSON command data, binds once and invokes once. Loading has no effect.\n\nInputs/result\nRecursive inputs: {inputs}. Recursive result: {result}. Exact profile/source UUID/revision/registration SHA-256 pins are required. Null, missing, extra, malformed identity and changed registration fail. Registration identity is separate from referenced document identity. Complete registration/diagnostic bytes stay data, never Python.\n\nAuthority and evidence\nRead performs no fetch or source selection. Retain appends exact diagnostic bytes as UNQUALIFIED evidence using the actual native invocation identity; a caller cannot make it trusted by claiming success/approval. Equal repeated retention under that identity returns the same receipt; changed bytes conflict. Storage success cannot publish availability, complete a job, approve knowledge, alter source review, change a generation or activate a design. An admitting Recipe owner must separately verify actual pinned source, I/O/digest effect records and receipt before presenting diagnostics. Current Tool policy, kernel, cancellation/fencing and technical bounds remain independent. One initial dispatch; no automatic retry, effect replay, hidden model call or Tier-2 fallback.\n\nQualification\nVerify real retained registration reads, immutable append/duplicate/conflict behavior, invalid inputs, live policy and durable receipt/effect correlation. These drafts confer no approval or activation.",
        ids.python_code, ids.tool, ids.tool_skill, ids.recipe
    );
    Ok(vec![
        make(
            parser,
            22,
            json!({"name":parse,"description":"Parse canonical source request JSON as typed data without effects.","content":parser_source,"input_contract":task,"result_contract":parser_result,"preload":preload(parse,vec!["json"])}),
            vec![],
            Value::Null,
        )?,
        make(
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,"callable":"host.prefix_source","capability_id":"builtin.prefix_source"}}),
            vec![ids.tool],
            Value::Null,
        )?,
        make(
            ids.python_code,
            22,
            json!({"name":export,"description":"One immutable source storage operation; no source checking or approval.","content":source,"input_contract":inputs,"result_contract":result,"preload":preload(export,vec![])}),
            vec![],
            Value::Null,
        )?,
        make(
            ids.skill,
            1,
            json!({"body":prose,"interface":interface}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        )?,
        make(
            ids.recipe,
            21,
            json!({"name":format!("prefix-source-{operation}"),"description":"Canonical model-free source storage usage; no availability publication.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":[{"name":"request_json","pattern":null,"description":"Canonical typed JSON source request"}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Source storage usage","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"orchestrator","goal":"Parse source command data","content":"","type":"component","include":[parser]},
                {"stepnumber":2,"knowledge":"rust","goal":"Prepare pinned source storage binding","content":"","type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":"prefix_source","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":3,"knowledge":"orchestrator","goal":"Perform exactly one storage operation","content":"","type":"component","include":[ids.python_code]}]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":task,"steps":{"0:1":{"request_json":{"kind":"task_input","reference":"{{vars.request_json}}"}},"0:3":links}}},
            "invocations":{"selected":{"0:1":parse,"0:3":export}}}),
            vec![parser, ids.skill, ids.tool_skill, ids.python_code],
            Value::Null,
        )?,
    ])
}
fn make(
    id: Uuid,
    class: i32,
    document: Value,
    dependencies: Vec<Uuid>,
    association: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":class,"document":document,"dependencies":dependencies,"association":association}).to_string())
}
