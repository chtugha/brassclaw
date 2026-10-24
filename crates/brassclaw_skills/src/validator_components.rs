//! Reusable pure checking components. Packaged drafts are neither complete
//! class validators nor qualification, activation, or successful Q1 evidence.
use crate::component_revision::{ComponentRevisionDraft, RevisionError};
use crate::global_bootstrap_components::UsageComponentIds;
use serde_json::json;
use uuid::Uuid;

#[path = "validator_source_inventory.rs"]
mod source_inventory;
pub use source_inventory::{SOURCE_ENTRIES_SOURCE, source_inventory_drafts};

#[path = "validator_python_structure.rs"]
mod python_structure;
pub use python_structure::{python_definitions_draft, python_graph_draft, python_structure_drafts};

pub const AGGREGATE_SOURCE: &str = include_str!("validator_aggregate.py");
pub const COLLECT_SOURCE: &str = include_str!("validator_collect.py");
pub const STRUCTURAL_PRESENCE_SOURCE: &str = include_str!("validator_structural_presence.py");
pub const PRELOAD_DEFINITION_SOURCE: &str = include_str!("validator_preload_definition.py");
pub const SUBJECT_SOURCES_SOURCE: &str = include_str!("validator_subject_sources.py");
pub const RECURSIVE_CONTRACT_SOURCE: &str = include_str!("validator_recursive_contract.py");

/// Review every selected PythonCode's public contracts using the existing pure
/// grammar export. The exact original subject and helper selection are supplied
/// by the review owner; this code grants no provenance or component approval.
pub fn python_contracts_draft(
    id: Uuid,
    recursive_rule: Uuid,
    source: &str,
) -> Result<ComponentRevisionDraft, RevisionError> {
    if id.is_nil() || recursive_rule.is_nil() || id == recursive_rule {
        return Err(RevisionError::Invalid(
            "distinct Python contract rule identities required",
        ));
    }
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"check-review-python-contracts",
            "description":"Inspect all class-22 candidate/dependency public input/result contracts from the complete strict retained subject using the pinned check_recursive_contract export with fixed input/result positions. Preserve every negative diagnostic; exclude the comparison base and leave non-Python records for their own checks. Private helper-only libraries need no invented public boundary, but any present contract is checked. Do not execute candidate source, infer binding/call/semantic agreement, establish native resource admission/provenance, write evidence or approve components. The original sealed subject and actual result/input edges must be observed by the existing trusted review owner.",
            "content":source,"includes":[],"dependency_registry":null,
            "input_contract":{"subject_json":{"type":"string","required":true,"checks":[]}},
            "result_contract":rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_review_python_contracts":{"symbol":"check_review_python_contracts","parameters":["inputs"],"mapping":true}
            },"private_functions":{},"constants":[],"imports":["json"],"dependencies":[recursive_rule],"default_export":"check_review_python_contracts"}
        },"dependencies":[recursive_rule],"association":null
    }).to_string())?;
    crate::value_contract::InputContract::from_value(
        &draft.document()["input_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    crate::value_contract::ValueContract::result_from_value(
        &draft.document()["result_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    Ok(draft)
}

/// Check the current recursive consumer/result/Tool-parameter grammar as pure
/// Recipe logic. The owner supplies JSON encoded from the strict retained reader;
/// this rule is not duplicate-key/integrity provenance or behavioral approval.
/// Computed arguments, handoff compatibility and resource admission stay separate.
pub fn recursive_contract_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"check-recursive-contract",
            "description":"Check one complete recursively declared contract in the current input, parameters or result position, supplied as JSON string data encoded from the strict retained subject reader. Verify closed fields, required/null/default rules, items/object fields/extras, supported checks, exact decimal bounds and cross-input targets; inspect defaults without filling or changing data. Defaults may fill missing consumer fields only. Do not execute candidates, infer compatibility or behavior, establish provenance/duplicate-key validation, approve components or write evidence. Computed arguments require their separate owner; unsupported positions fail. Existing task/data limits apply without a new checker cap. Integer range constants describe the native JSON wire representation, not a task budget.",
            "content":RECURSIVE_CONTRACT_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{
                "contract_json":{"type":"string","required":true,"checks":[]},
                "position":{"type":"string","required":true,"checks":[]}
            },"result_contract":rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_recursive_contract":{"symbol":"check_recursive_contract","parameters":["inputs"],"mapping":true}
            },"private_functions":{
                "_validator_contract_name":["name"],"_validator_contract_number":["value"],
                "_validator_contract_parts":["value"],"_validator_contract_compare":["left","right"],
                "_validator_contract_default":["schema","value"]
            },"constants":[],"imports":["json"],"dependencies":[],"default_export":"check_recursive_contract"}
        },"dependencies":[],"association":null
    }).to_string())?;
    crate::value_contract::InputContract::from_value(
        &draft.document()["input_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    crate::value_contract::ValueContract::result_from_value(
        &draft.document()["result_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    Ok(draft)
}

/// Extract all class-22 sources from the sealed reader's complete subject. The
/// comparison base is excluded; non-Python dependencies remain in the original
/// subject for their separate checks. This transformation is not a provenance
/// verifier: the review owner must bind/observe the actual retained subject.
pub fn subject_sources_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"extract-review-python-sources",
            "description":"Extract every exact class-22 content string from the complete retained candidate/dependency subject into a UUID-keyed inventory, preserving empty text and excluding the comparison base. Non-Python records remain available in the original subject for separate checks. No candidate executes, source is not normalized or truncated, and this pure transformation establishes no integrity, review provenance, verdict or approval. The trusted owner must supply the sealed subject and observe its actual typed input/result edges.",
            "content":SUBJECT_SOURCES_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{"subject_json":{"type":"string","required":true,"checks":[]}},
            "result_contract":{"type":"object","checks":[],"fields":{},"allow_extra_fields":true,
                "extra_fields":{"type":"string","checks":[]}},
            "preload":{"format":"python-preload/2","exports":{
                "extract_review_python_sources":{"symbol":"extract_review_python_sources","parameters":["inputs"],"mapping":true}
            },"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"extract_review_python_sources"}
        },"dependencies":[],"association":null
    }).to_string())?;
    crate::value_contract::InputContract::from_value(
        &draft.document()["input_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    crate::value_contract::ValueContract::result_from_value(
        &draft.document()["result_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    Ok(draft)
}

/// Compare one declared function library with actual mechanical parser facts.
/// This rule neither executes the candidate nor resolves/approves its dependency
/// graph, reachable Tool usage, behavior or provenance. The review owner must
/// supply the exact original declaration and matching native observations.
pub fn preload_definition_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let text = json!({"type":"string"});
    let strings = json!({"type":"list","items":text});
    let count = json!({"type":"integer","checks":[{"kind":"min","value":0}]});
    let mapping = |values| json!({"type":"object","fields":{},"allow_extra_fields":true,"extra_fields":values});
    let required = |mut schema: serde_json::Value| {
        schema["required"] = json!(true);
        schema
    };
    let export = json!({"type":"object","allow_extra_fields":false,"fields":{
        "symbol":required(text.clone()),"parameters":required(strings.clone()),
        "mapping":{"type":"boolean","required":true}
    }});
    let declaration = json!({"type":"object","required":true,"nullable":true,"checks":[],"allow_extra_fields":false,"fields":{
        "format":required(text.clone()),"exports":required(mapping(export)),
        "private_functions":required(mapping(strings.clone())),"constants":required(strings.clone()),
        "imports":required(strings.clone()),"dependencies":required(strings.clone()),
        "default_export":{"type":"string","required":false,"nullable":true}
    }});
    let scope = json!({"type":"object","allow_extra_fields":false,"fields":{
        "references":required(strings.clone()),"calls":required(mapping(count.clone())),
        "value_references":required(strings.clone()),"nested_functions":{"type":"boolean","required":true},
        "start":required(count.clone()),"end":required(count.clone())
    }});
    let observed_preload = json!({"type":"object","required":true,"nullable":true,"allow_extra_fields":false,"fields":{
        "scopes":required(mapping(scope)),"functions":required(mapping(strings.clone())),
        "constants":required(strings.clone()),"references":required(strings.clone())
    }});
    let callsite = json!({"type":"object","allow_extra_fields":false,"fields":{
        "attribute":required(text.clone()),"start":required(count.clone()),"end":required(count.clone()),
        "repeatable":{"type":"boolean","required":true}
    }});
    let observations = json!({"type":"object","required":true,"checks":[],"allow_extra_fields":false,"fields":{
        "source_checksum":required(text),"preload":observed_preload,
        "direct_host_calls":{"type":"list","required":true,"items":callsite},
        "imports":required(strings.clone()),"relative_imports":{"type":"boolean","required":true},
        "host_value_references":required(count.clone()),"reserved_name_references":required(strings),
        "result_store_sites":required(count.clone()),"inspected_nodes":required(count)
    }});
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"check-preload-definition",
            "description":"Compare one exact preload declaration with actual nonexecuting source observations: declared signatures, symbols, constants, imports and effect-free definition shape. Dependency resolution, invocation contracts, Tool effects and trusted provenance require separate review checks; this rule writes no evidence or approval.",
            "content":PRELOAD_DEFINITION_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{"declaration":declaration,"observations":observations},
            "result_contract":rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_preload_definition":{"symbol":"check_preload_definition","parameters":["inputs"],"mapping":true}
            },"private_functions":{"_validator_preload_identifier":["name"]},"constants":[],"imports":[],"dependencies":[],"default_export":"check_preload_definition"}
        },"dependencies":[],"association":null
    }).to_string())?;
    crate::value_contract::InputContract::from_value(
        &draft.document()["input_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    crate::value_contract::ValueContract::result_from_value(
        &draft.document()["result_contract"],
        crate::component_revision::REVISION_LIMITS,
    )?;
    Ok(draft)
}

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
