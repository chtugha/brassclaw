//! Reusable pure checking components. Packaged drafts are neither complete
//! class validators nor qualification, activation, or successful Q1 evidence.
use crate::component_revision::{ComponentRevisionDraft, RevisionError};
use crate::global_bootstrap_components::UsageComponentIds;
use serde_json::json;
use uuid::Uuid;

pub const AGGREGATE_SOURCE: &str = include_str!("validator_aggregate.py");
pub const COLLECT_SOURCE: &str = include_str!("validator_collect.py");
pub const STRUCTURAL_PRESENCE_SOURCE: &str = include_str!("validator_structural_presence.py");

/// Typed successor to the legacy three-string seed check. This intentionally
/// checks only presence; it is not a class validator, semantic/behavioral review
/// or Q1 qualification. The installation owner stages it without activation.
pub fn structural_presence_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let text = json!({"type":"string","required":true,"checks":[]});
    ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"check-structural-presence",
            "description":"Check only whether typed name, description and content strings are nonempty after stripping; never interpret their contents, approve a component or write evidence.",
            "content":STRUCTURAL_PRESENCE_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{"name":text,"description":text,"content":text},
            "result_contract":rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_structural_presence":{"symbol":"check_structural_presence","parameters":["inputs"],"mapping":true}
            },"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"check_structural_presence"}
        },"dependencies":[],"association":null
    }).to_string())
}

fn nonempty_string() -> serde_json::Value {
    json!({"type":"string","checks":[{"kind":"min_length","value":1}]})
}
fn rule_contract() -> serde_json::Value {
    json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "rule_id":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},
        "ok":{"type":"boolean","required":true,"checks":[]},
        "errors":{"type":"list","required":true,"checks":[],"items":nonempty_string()}
    }})
}

/// Preserve each complete typed rule result for the following aggregate step.
/// Input names come from the pinned workflow declaration. Recipe bindings must
/// supply each actual whole step result; this component grants no approval and
/// neither invents missing observations nor replaces the trusted review owner.
pub fn collect_draft(
    id: Uuid,
    rule_ids: &[String],
) -> Result<ComponentRevisionDraft, RevisionError> {
    let mut inputs = serde_json::Map::new();
    if rule_ids.is_empty() {
        return Err(RevisionError::Invalid(
            "nonempty validator rule collection required",
        ));
    }
    for name in rule_ids {
        let mut chars = name.bytes();
        if !chars.next().is_some_and(|byte| byte.is_ascii_lowercase())
            || !chars.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || inputs.contains_key(name)
        {
            return Err(RevisionError::Invalid(
                "distinct validator rule input names required",
            ));
        }
        let mut contract = rule_contract();
        contract["required"] = json!(true);
        inputs.insert(name.clone(), contract);
    }
    ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"collect-validator-checks",
            "description":"Preserve every complete typed rule result supplied by pinned result bindings, without interpreting it or writing review evidence.",
            "content":COLLECT_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":inputs,
            "result_contract":{"type":"list","checks":[],"items":rule_contract()},
            "preload":{"format":"python-preload/2","exports":{
                "collect_validator_checks":{"symbol":"collect_validator_checks","parameters":["inputs"],"mapping":true}
            },"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"collect_validator_checks"}
        },"dependencies":[],"association":null
    }).to_string())
}

/// Aggregate an exact pinned validator's required rules and observed results.
/// The caller must bind required IDs from that validator, never the candidate.
/// This function produces data; only the trusted executor/evidence owner can
/// establish its provenance and whether all required checks actually ran.
pub fn aggregate_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let string = nonempty_string();
    let check = rule_contract();
    ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"aggregate-validator-checks",
            "description":"Require complete unique pinned rule coverage and consistent typed results; preserve rule-correlated diagnostics without writing evidence or approval.",
            "content":AGGREGATE_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{
                "required_rule_ids":{"type":"list","required":true,"checks":[],"items":string},
                "checks":{"type":"list","required":true,"checks":[],"items":check}
            },
            "result_contract":{"type":"object","checks":[],"allow_extra_fields":false,"fields":{
                "pass":{"type":"boolean","required":true,"checks":[]},
                "errors":{"type":"list","required":true,"checks":[],"items":{
                    "type":"object","checks":[],"allow_extra_fields":false,"fields":{
                        "rule_id":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},
                        "errors":{ "type":"list","required":true,"checks":[],"items":string }
                    }
                }}
            }},
            "preload":{"format":"python-preload/2","exports":{
                "aggregate_validator_checks":{"symbol":"aggregate_validator_checks","parameters":["inputs"],"mapping":true}
            },"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"aggregate_validator_checks"}
        },"dependencies":[],"association":null
    }).to_string())
}

/// One mechanical parser usage with a canonical execution Recipe. The caller
/// supplies an actually registered Tool identity and retains its implementation
/// separately. These drafts create neither that Tool nor approval/activation.
pub fn source_inspection_drafts(
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
            "distinct source-inspection usage identities required",
        ));
    }
    let inputs = json!({"source":{"type":"string","required":true,"checks":[]}});
    let result = json!({"type":"string"});
    let export = "inspect_python_source";
    let preload = json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":export});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed", "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association = json!({"format":"skill-association/1","skill_uuid":ids.skill,"python_code_uuid":ids.python_code,
        "tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,"callable":"host.source_inspection",
        "inputs":inputs,"arguments":{"source":"source"},"code_arguments":{},"result":result,"failure":failure}).to_string();
    let make = |id, class_code, document, dependencies, association| {
        ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":class_code,"document":document,"dependencies":dependencies,"association":association,
    }).to_string())
    };
    Ok(vec![
        make(
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,
            "callable":"host.source_inspection","capability_id":"builtin.source_inspection"}}),
            vec![ids.tool],
            serde_json::Value::Null,
        )?,
        make(
            ids.python_code,
            22,
            json!({"name":"inspect-python-source","description":"Forward complete typed source to one nonexecuting parser primitive and return exact fact bytes.",
            "content":"def inspect_python_source(inputs):\n    return host.source_inspection(source=inputs['source'])\n",
            "input_contract":inputs,"result_contract":result,"preload":preload}),
            vec![],
            serde_json::Value::Null,
        )?,
        make(
            ids.skill,
            1,
            json!({"body":format!(
            "Purpose\nInspect Python source text without executing it. This is one parser usage, not a class validator.\n\nComponents and interface\nTool {}; ToolSkill {}; PythonCode {}. Pinned export inspect_python_source(inputs) calls host.source_inspection(source=inputs['source']) exactly once. No helpers, constants or dependencies. Loading has no effects.\n\nInput and result\nsource is a required non-null string, including empty source, quotes, Unicode, newlines and template-looking text. It remains data and is never substituted into executable source. Return exact python-source-facts/1 JSON bytes with preparation revision, retained worker checksum and mechanical parser observations. A parse success is not a validation verdict or proof of behavior.\n\nPrerequisites and failure\nRetain the actual registered native implementation and worker artifact. Current instance Tool policy, cancellation, attempt fencing and acknowledged preparation limits apply at every dispatch. Missing/null/nonstring inputs, unavailable backend, syntax/parser/worker errors and capacity rejection stop the usage. One initial attempt, no automatic retry or approval grant. Never turn a failure into No-Match.\n\nExecution Recipe and command\nRecipe {}, variant selected, command: inspect Python source %. Position zero is source:string, required, non-null and verbatim. Examples: inspect Python source value = 1; inspect Python source raise RuntimeError('data only'). Matching commands do not authorize or execute their source. This deterministic usage is Tier 0 only after complete qualification and activation. The consuming validator Recipe chooses rules and produces its own verdict; this usage writes no evidence, approval, reply or component state.",ids.tool,ids.tool_skill,ids.python_code,ids.recipe),
            "interface":{"format":"skill-interface/1","python_code_uuid":ids.python_code,"exports":preload["exports"],"inputs":inputs,
                "result":result,"failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        )?,
        make(
            ids.recipe,
            21,
            json!({"name":"inspect-python-source","description":"Canonical single-usage execution Recipe for nonexecuting Python parser facts; it does not validate a component.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["inspect Python source %"],
                "variable_patterns":[{"name":"source","pattern":null,"description":"Complete literal Python text, never executed"}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Inspect Python text","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"rust","goal":"Prepare source inspection binding","content":"","type":"component","include":[ids.tool_skill],
                    "tool_bindings":[{"tool_id":ids.tool,"tool_name":"source_inspection","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Invoke pinned parser usage","content":"","type":"component","include":[ids.python_code]}
            ]}],"input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,
                "steps":{"0:2":{"source":{"kind":"task_input","reference":"{{vars.source}}"}}}}},
            "invocations":{"selected":{"0:2":export}}}),
            vec![ids.tool_skill, ids.python_code, ids.skill],
            serde_json::Value::Null,
        )?,
    ])
}
