//! Canonical one-usage drafts for immutable generation storage. These reuse the
//! registered primitive; draft construction grants no approval or activation.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub enum GenerationUsage {
    Read,
    Retain,
}

pub fn generation_usage_drafts(
    ids: UsageComponentIds,
    usage: GenerationUsage,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let retain = matches!(usage, GenerationUsage::Retain);
    if ids.formatter.is_some() != retain {
        return Err(RevisionError::Invalid(
            "retention needs a separate decimal-revision preparation component",
        ));
    }
    let mut unique = std::collections::BTreeSet::new();
    for id in [
        ids.recipe,
        ids.python_code,
        ids.tool_skill,
        ids.tool,
        ids.skill,
    ]
    .into_iter()
    .chain(ids.formatter)
    {
        if id.is_nil() || !unique.insert(id) {
            return Err(RevisionError::Invalid(
                "generation usage identities must be distinct and non-nil",
            ));
        }
    }
    let string =
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]});
    let integer = json!({"type":"integer","required":true,"checks":[{"kind":"min","value":1},{"kind":"max","value":9007199254740991i64}]});
    let mut integer_value = integer.clone();
    integer_value
        .as_object_mut()
        .ok_or(RevisionError::Invalid(
            "generation revision result schema must be an object",
        ))?
        .remove("required");
    let summary = json!({"type":"object","allow_extra_fields":false,"fields":{
        "profile":string,"generation":string,"profile_revision":integer,"kind":string,"qualification":string,"created_at":string}});
    let result = if retain {
        summary.clone()
    } else {
        json!({"type":"object","allow_extra_fields":false,"fields":{
        "summary":{"type":"object","required":true,"allow_extra_fields":false,"fields":summary["fields"]},"generation_bytes":string}})
    };
    let operation = if retain { "retain" } else { "read" };
    let export = if retain {
        "retain_prefix_generation"
    } else {
        "read_prefix_generation"
    };
    let inputs = if retain {
        json!({"profile":string,"expected_profile_revision":integer,"generation_bytes":string})
    } else {
        json!({"profile":string,"generation":string})
    };
    let arguments = if retain {
        json!({"profile":"profile","expected_profile_revision":"expected_profile_revision","generation_bytes":"generation_bytes"})
    } else {
        json!({"profile":"profile","generation":"generation"})
    };
    let body = if retain {
        "def retain_prefix_generation(inputs):\n    return host.prefix_generation(operation='retain', profile=inputs['profile'], expected_profile_revision=inputs['expected_profile_revision'], generation_bytes=inputs['generation_bytes'])\n"
    } else {
        "def read_prefix_generation(inputs):\n    return host.prefix_generation(operation='read', profile=inputs['profile'], generation=inputs['generation'])\n"
    };
    let preload = json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":export});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association = json!({"format":"skill-association/1","skill_uuid":ids.skill,"python_code_uuid":ids.python_code,"tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,
        "callable":"host.prefix_generation","inputs":inputs,"arguments":arguments,
        "code_arguments":{"operation":{"type":"string","checks":[],"depends_on":[],"meaning":format!("Exactly one immutable generation {operation} operation")}},
        "result":result,"failure":failure}).to_string();
    let interface = json!({"format":"skill-interface/1","python_code_uuid":ids.python_code,"exports":preload["exports"],"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]});
    let prose = format!(
        "Purpose\n{operation} one exact immutable prefix generation through the registered storage primitive.\n\nInputs\n{inputs}\nMissing/null/extra inputs fail. Read requires an exact content hash. Retain consumes complete typed package bytes and an exact positive profile revision.\n\nInterface\nAssociated PythonCode {} preloads {export}(inputs); loading has no effects. Tool {} and ToolSkill {} prepare host.prefix_generation. The export fixes operation={operation} and calls it once.\n\nResult\n{result}\nRetention returns a quarantined immutable receipt; it never compiles, selects sources/designs, establishes model/cache quality or activates a prefix. A read returns exact retained bytes and qualification metadata. Legacy-unverified data remains unverified.\n\nWorkflow\nUse the canonical execution Recipe {}. Other Recipes may reuse this usage. Snapshot/render/tokenize/validate/accept/activate are separate operations, not hidden here.\n\nAuthority and limits\nCurrent instance Tool policy, technical bounds, actual retained implementation and task/attempt fences still apply. No binding, status label or stored bytes grant authority. Input/output are bounded to the registered transport capacity and applicable VM limits.\n\nFailure\nStop on invalid data, conflict, missing reference, policy block, stale/cancelled attempt, backend failure or invalid output. One initial dispatch; no automatic retry, whole-Recipe replay or Tier-2 fallback. A lost publication result requires exact immutable reconciliation by a separate usage; never infer no effect from a timeout.\n\nAcceptance\nVerify exact stored bytes, immutable reread, predecessor conflict, malformed packages and no active-pointer change through actual PostgreSQL/registered dispatch. These drafts require the installation owner's exact integrity/Q1/behavioral bootstrap qualification before activation.",
        ids.python_code, ids.tool, ids.tool_skill, ids.recipe
    );
    let mut drafts = vec![
        (
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,"callable":"host.prefix_generation","capability_id":"builtin.prefix_generation"}}),
            vec![ids.tool],
            Value::Null,
        ),
        (
            ids.python_code,
            22,
            json!({"content":body,"preload":preload,"input_contract":inputs,"result_contract":result}),
            vec![],
            Value::Null,
        ),
        (
            ids.skill,
            1,
            json!({"body":prose,"interface":interface}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        ),
    ];
    let mut steps = Vec::new();
    let mut layouts = serde_json::Map::new();
    let mut invocations = serde_json::Map::new();
    let mut dependencies = vec![ids.skill, ids.tool_skill, ids.python_code];
    let offset = usize::from(retain);
    let task_inputs = if retain {
        json!({"profile":string,"revision":string,"generation_bytes":string})
    } else {
        inputs.clone()
    };
    if let Some(prepare) = ids.formatter {
        let body = "def prepare_generation_revision(inputs):\n    text = inputs['revision']\n    if not text or len(text) > 16 or text[0] == '0' or any(character < '0' or character > '9' for character in text):\n        raise ValueError('canonical positive decimal revision required')\n    value = int(text)\n    if value > 9007199254740991:\n        raise ValueError('revision exceeds portable integer range')\n    return value\n";
        drafts.push((prepare,22,json!({"content":body,"input_contract":{"revision":string},"result_contract":integer_value,
            "preload":{"format":"python-preload/2","exports":{"prepare_generation_revision":{"symbol":"prepare_generation_revision","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"prepare_generation_revision"}}),vec![],Value::Null));
        dependencies.push(prepare);
        steps.push(json!({"stepnumber":1,"knowledge":"orchestrator","goal":"Validate the canonical revision data","content":"","type":"component","include":[prepare]}));
        layouts.insert(
            "0:1".into(),
            json!({"revision":{"kind":"task_input","reference":"{{vars.revision}}"}}),
        );
        invocations.insert("0:1".into(), json!("prepare_generation_revision"));
    }
    let execution = format!("0:{}", 2 + offset);
    steps.push(json!({"stepnumber":1+offset,"knowledge":"rust","goal":"Bind immutable generation storage","content":"","type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":"prefix_generation","params":{},"error_policy":{"policy":"fail"}}]}));
    steps.push(json!({"stepnumber":2+offset,"knowledge":"orchestrator","goal":format!("{operation} one immutable generation"),"content":"","type":"component","include":[ids.python_code]}));
    layouts.insert(execution.clone(),if retain { json!({"profile":{"kind":"task_input","reference":"{{vars.profile}}"},"expected_profile_revision":{"kind":"result","step_id":"0:1","path":[]},"generation_bytes":{"kind":"task_input","reference":"{{vars.generation_bytes}}"}}) } else { json!({"profile":{"kind":"task_input","reference":"{{vars.profile}}"},"generation":{"kind":"task_input","reference":"{{vars.generation}}"}}) });
    invocations.insert(execution, json!(export));
    // Preserve sentence/variable order rather than relying on map-key order.
    let variables: Vec<_> = if retain {
        ["profile", "revision", "generation_bytes"].as_slice()
    } else {
        ["profile", "generation"].as_slice()
    }
    .iter()
    .map(|name| json!({"name":name,"pattern":null,"description":null}))
    .collect();
    let pattern = if retain {
        "retain prefix profile=% revision=% package=%"
    } else {
        "read prefix profile=% generation=%"
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
    drafts.push((ids.recipe,21,json!({"name":format!("prefix-generation-{operation}"),"description":format!("Canonical Tier-0 immutable-generation {operation} usage; no activation or model call."),
        "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":variables}],
        "step_descriptions":[{"desc_idx":0,"label":"Reusable immutable storage usage","yaml_source":"","steps":steps}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":task_inputs,"steps":layouts}},"invocations":{"selected":invocations}}),dependencies,Value::Null));
    drafts.into_iter().map(|(id,class,document,dependencies,association)|ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":class,"document":document,"dependencies":dependencies,"association":association}).to_string())).collect()
}

/// Metadata for the actual registered primitive. Optional operation-specific
/// fields are checked by the primitive on dispatch; this contract grants nothing.
pub fn generation_tool_draft(
    id: uuid::Uuid,
    native_identity: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let string =
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]});
    let optional =
        json!({"type":"string","required":false,"checks":[{"kind":"min_length","value":1}]});
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":0,
        "document":{"capability_id":"builtin.prefix_generation","callable":"host.prefix_generation",
        "input_contract":{"operation":{"type":"string","required":true,"checks":[]},"profile":string,
            "generation":optional,"generation_bytes":optional,
            "expected_profile_revision":{"type":"integer","required":false,"checks":[{"kind":"min","value":1},{"kind":"max","value":9007199254740991i64}]}},
        "implementation":{"format":"native-first-party/1","identity":native_identity}},
        "dependencies":[],"association":null}).to_string())
}
