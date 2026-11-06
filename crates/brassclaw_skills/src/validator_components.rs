//! Reusable pure checking components. Packaged drafts are neither complete
//! class validators nor qualification, activation, or successful Q1 evidence.
use crate::component_revision::{ComponentRevisionDraft, RevisionError};
use crate::global_bootstrap_components::UsageComponentIds;
use serde_json::json;
use uuid::Uuid;

#[path = "validator_source_inventory.rs"]
mod source_inventory;
pub use source_inventory::{
    SOURCE_ENTRIES_SOURCE, source_inventory_drafts, source_inventory_qualification_draft,
};

#[path = "validator_python_structure.rs"]
mod python_structure;
pub use python_structure::{python_definitions_draft, python_graph_draft, python_structure_drafts};

#[path = "validator_python_pure.rs"]
mod python_pure;
pub use python_pure::{python_pure_calls_draft, python_pure_structure_drafts};

pub const AGGREGATE_SOURCE: &str = include_str!("validator_aggregate.py");
pub const COLLECT_SOURCE: &str = include_str!("validator_collect.py");
pub const STRUCTURAL_PRESENCE_SOURCE: &str = include_str!("validator_structural_presence.py");
pub const PRELOAD_DEFINITION_SOURCE: &str = include_str!("validator_preload_definition.py");
pub const SUBJECT_SOURCES_SOURCE: &str = include_str!("validator_subject_sources.py");
pub const REVIEW_SUBJECT_READER_SOURCE: &str = include_str!("validator_review_subject.py");
pub const SKILL_CALL_TARGETS_SOURCE: &str = include_str!("validator_skill_calls.py");
pub const SKILL_INTERFACE_SOURCE: &str = include_str!("validator_skill_interface.py");
pub const RECURSIVE_CONTRACT_SOURCE: &str = include_str!("validator_recursive_contract.py");

/// Private dependency shared by subject-consuming checks. It returns borrowed
/// parsed data inside the VM; it exposes no command, invocation or approval API.
pub fn review_subject_reader_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"review-subject-reader",
            "description":"Shared private parsing of complete authored single-root /1, authored function /2 and explicitly separate installation-validator-subject/1 inputs. Preserve each format's exact fields and meaning; installation subject_id is never an authored submission_id. Validate complete envelope/role/revision identities, include each selected partner once and leave comparison bases out of executable dependencies. Return original parsed objects within the invocation without source substitution or serialization copies. This is shape/data handling, not checksum/provenance authentication, prior approval, semantic review, candidate execution or Q1/Q2. Only the trusted owner supplies the original corresponding sealed subject/custody.",
            "content":REVIEW_SUBJECT_READER_SOURCE,"includes":[],"dependency_registry":null,
            "preload":{"format":"python-preload/2","exports":{},
                "private_functions":{"_validator_review_entry":["entry"],"_validator_review_base":["base","candidate"],"_validator_review_subject":["inputs"]},
                "constants":[],"imports":["json"],"dependencies":[],"default_export":null}},
        "dependencies":[],"association":null
    }).to_string())
}

/// One reusable declaration check, not complete joint Skill/code validation.
/// Source/graph, variance, effect/behavior and prose meaning remain other rules.
pub fn skill_interface_draft(
    id: Uuid,
    recursive_rule: Uuid,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let reader = crate::global_bootstrap_components::packaged_component_id(
        "validator:review-subject-reader",
    );
    let definition =
        crate::global_bootstrap_components::packaged_component_id("validator:preload-definition");
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"check-review-skill-interface",
            "description":"Check the strict Skill interface against its actual associated implementation in a complete retained function subject. Require usage prose and closed format/UUID/export/signature/private/dependency declarations; reuse recursive input/result grammar and compare usage inputs/result/failure to the exact association with boolean/number distinctions. Extra implementation exports and compatible schema changes are not rejected merely for differing from usage declarations. Source definitions, reachable private completeness, variance/consumer compatibility, actual calls/effects/errors/retries, isolation, behavior and prose meaning remain separately required checks. Context receives no new approval. This component executes no candidate, grants no Q1/Q2 and is not a complete class validator.",
            "content":SKILL_INTERFACE_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{"subject_json":{"type":"string","required":true,"checks":[]}},
            "result_contract":rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_review_skill_interface":{"symbol":"check_review_skill_interface","parameters":["inputs"],"mapping":true}},
                "private_functions":{"_validator_skill_interface_equal":["left","right"]},
                "constants":[],"imports":["json"],"dependencies":[recursive_rule,reader,definition],"default_export":"check_review_skill_interface"}},
        "dependencies":[recursive_rule,reader,definition],"association":null
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

/// One mechanical own-function rule. It consumes actual prior checks and native
/// facts; joint admission, behavior, provenance and approval stay with the owner.
pub fn skill_call_targets_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let reader = crate::global_bootstrap_components::packaged_component_id(
        "validator:review-subject-reader",
    );
    let calls =
        crate::global_bootstrap_components::packaged_component_id("validator:python-pure-calls");
    if id.is_nil() || id == reader || id == calls {
        return Err(RevisionError::Invalid(
            "distinct Skill call rule and helper identities required",
        ));
    }
    let mut check = rule_contract();
    check["required"] = json!(true);
    let mut profile = python_structure::profile_contract();
    profile["required"] = json!(true);
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"check-review-skill-call-targets",
            "description":"After actual graph and Skill-interface checks, compare native Host sites reachable through the declared public Skill exports and selected helpers with the exact associated callable. Reuse the pure checker private function-closure walk without invoking its pure-only rule. Source ranges are original parser facts; unused private functions are not usage roots. No additional one-call budget, argument/result compatibility engine, candidate invocation, behavior/prose interpretation, component approval or provenance claim. Native single-class joint preparation remains unsupported. This is one private incomplete own-function check, never a new combination review.",
            "content":SKILL_CALL_TARGETS_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{"subject_json":{"type":"string","required":true,"checks":[]},
                "inventory_json":{"type":"string","required":true,"checks":[]},"profile":profile,
                "graph":check,"interface":check},"result_contract":rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_review_skill_call_targets":{"symbol":"check_review_skill_call_targets","parameters":["inputs"],"mapping":true}},
                "private_functions":{"_validator_skill_call_targets_result":["errors"]},
                "constants":[],"imports":["json"],"dependencies":[reader,calls],"default_export":"check_review_skill_call_targets"}},
        "dependencies":[reader,calls],"association":null
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
    let subject_reader = crate::global_bootstrap_components::packaged_component_id(
        "validator:review-subject-reader",
    );
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
            },"private_functions":{},"constants":[],"imports":["json"],"dependencies":[recursive_rule,subject_reader],"default_export":"check_review_python_contracts"}
        },"dependencies":[recursive_rule,subject_reader],"association":null
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
    let subject_reader = crate::global_bootstrap_components::packaged_component_id(
        "validator:review-subject-reader",
    );
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"extract-review-python-sources",
            "description":"Extract every exact class-22 content string from complete single-root or Skill/code function subjects through the shared private reader into a UUID-keyed inventory, preserving empty text and excluding comparison bases. Include the exact partner once; no context is reapproved. Non-Python records remain available in the original subject for separate checks. No candidate executes, source is not normalized or truncated, and this pure transformation establishes no integrity, review provenance, verdict or approval. The trusted owner must supply the sealed subject and observe its actual typed input/result edges.",
            "content":SUBJECT_SOURCES_SOURCE,"includes":[],"dependency_registry":null,
            "input_contract":{"subject_json":{"type":"string","required":true,"checks":[]}},
            "result_contract":{"type":"object","checks":[],"fields":{},"allow_extra_fields":true,
                "extra_fields":{"type":"string","checks":[]}},
            "preload":{"format":"python-preload/2","exports":{
                "extract_review_python_sources":{"symbol":"extract_review_python_sources","parameters":["inputs"],"mapping":true}
            },"private_functions":{},"constants":[],"imports":[],"dependencies":[subject_reader],"default_export":"extract_review_python_sources"}
        },"dependencies":[subject_reader],"association":null
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
            },"private_functions":{"_validator_preload_identifier":["name"],"_validator_preload_export_shape":["value"]},"constants":[],"imports":[],"dependencies":[],"default_export":"check_preload_definition"}
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

/// One mechanical parser usage with a separate reference Recipe. The caller
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
            "Purpose\nInspect Python source text without executing it. This is one parser usage, not a class validator.\n\nComponents and interface\nTool {}; ToolSkill {}; PythonCode {}. Pinned export inspect_python_source(inputs) calls host.source_inspection(source=inputs['source']) exactly once. No helpers, constants or dependencies. Loading has no effects.\n\nInput and result\nsource is a required non-null string, including empty source, quotes, Unicode, newlines and template-looking text. It remains data and is never substituted into executable source. Return exact python-source-facts/1 JSON bytes with preparation revision, retained worker checksum and mechanical parser observations. A parse success is not a validation verdict or proof of behavior.\n\nPrerequisites and failure\nRetain the actual registered native implementation and worker artifact. Current instance Tool policy, cancellation, attempt fencing and acknowledged preparation limits apply at every dispatch. Missing/null/nonstring inputs, unavailable backend, syntax/parser/worker errors and capacity rejection stop the usage. One initial attempt, no automatic retry or approval grant. Never turn a failure into No-Match.\n\nOptional consuming/reference Recipe and command\nThe referenced workflow is a separate consumer/test contract, not a prerequisite for Skill completeness or reuse. MCP exposure requires a separately qualified declaration.\nRecipe {}, variant selected, command: inspect Python source %. Position zero is source:string, required, non-null and verbatim. Examples: inspect Python source value = 1; inspect Python source raise RuntimeError('data only'). Matching commands do not authorize or execute their source. This deterministic usage is Tier 0 only after complete qualification and activation. The consuming validator Recipe chooses rules and produces its own verdict; this usage writes no evidence, approval, reply or component state.",ids.tool,ids.tool_skill,ids.python_code,ids.recipe),
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

/// Stable packaged ID for the structural-check PythonCode for a validator class.
/// These IDs are allocated in the shared installation namespace so both the legacy
/// mutable-recipe seeder and the new immutable-revision path use the same identity.
pub fn structural_validator_pc_id(class_code: i16) -> Uuid {
    crate::global_bootstrap_components::packaged_component_id(&format!(
        "validator:structural:pc:class-{class_code}"
    ))
}

/// Stable packaged ID for the collect step of a class validator Recipe.
pub fn structural_validator_collect_id(class_code: i16) -> Uuid {
    crate::global_bootstrap_components::packaged_component_id(&format!(
        "validator:structural:collect:class-{class_code}"
    ))
}

/// Stable packaged ID for the aggregate step of a class validator Recipe.
pub fn structural_validator_aggregate_id(class_code: i16) -> Uuid {
    crate::global_bootstrap_components::packaged_component_id(&format!(
        "validator:structural:aggregate:class-{class_code}"
    ))
}

/// Stable packaged ID for the receipt encoder step of a class validator Recipe.
pub fn structural_validator_receipt_id(class_code: i16) -> Uuid {
    crate::global_bootstrap_components::packaged_component_id(&format!(
        "validator:structural:receipt:class-{class_code}"
    ))
}

/// Stable packaged ID for a class validator Recipe (immutable revision path).
pub fn structural_validator_recipe_id(class_code: i16) -> Uuid {
    crate::global_bootstrap_components::packaged_component_id(&format!(
        "validator:structural:recipe:class-{class_code}"
    ))
}

/// Supported class codes for the eight packaged structural validator Recipes.
pub const VALIDATOR_CLASS_CODES: &[i16] = &[0, 1, 2, 3, 13, 21, 22, 23];

/// Build the immutable `ComponentRevisionDraft` set for one structural class
/// validator Recipe and its dependencies. This is the retained-revision path
/// for `RetainedValidatorCoverage::from_inputs()`; it is separate from the
/// legacy mutable `reborn_recipes` seeder in `seed_validator_recipes`.
///
/// Returns five drafts (in dependency-first order):
/// 1. PythonCode — the structural check body (pure-logic, `inputs["subject_json"]`).
/// 2. PythonCode — collect step (single-rule wrapper).
/// 3. PythonCode — aggregate step.
/// 4. PythonCode — receipt encoder step (serialises results into the receipt JSON string).
/// 5. Recipe (class 21) — the validator Recipe with `validator_coverage`.
///
/// The PythonCode body must already be published via the existing
/// `PC_VALIDATOR_STRUCTURAL_BODY` constant in `builtin_bootstrap.rs`; the
/// IDs here must match those used by the bootstrap seeder.
///
/// None of these drafts establish approval, Q1 evidence, activation or Tool
/// permission. They supply preparation metadata for the retained validator path.
pub fn structural_validator_recipe_drafts(
    class_code: i16,
    class_label: &str,
    pc_body: &str,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    if !VALIDATOR_CLASS_CODES.contains(&class_code) {
        return Err(RevisionError::Invalid("unsupported validator class code"));
    }
    let pc_id = structural_validator_pc_id(class_code);
    let collect_id = structural_validator_collect_id(class_code);
    let aggregate_id = structural_validator_aggregate_id(class_code);
    let receipt_id = structural_validator_receipt_id(class_code);
    let recipe_id = structural_validator_recipe_id(class_code);

    // Rule name for the single structural check.
    let rule_id = "structural_presence";
    let rule_step = "0:1";
    let collect_step = "0:2";
    let aggregate_step = "0:3";
    let receipt_step = "0:4";

    // 1. PythonCode for the structural-check body.
    // The input contract uses `subject_json` (a single required string containing
    // the serialised submission subject JSON) so that
    // RetainedValidatorCoverage::subject_task_inputs finds a step binding for
    // `subject_json` and satisfies its "consume its complete subject input" check.
    let pc_description = format!(
        "Shipped shallow structural-checking function for class {class_code} ({class_label}). \
         Receives subject_json as typed string data and inspects the candidate document. \
         Checks name/description and the class-specific presence markers in its actual body. \
         Pure logic with no host calls; returns {{rule_id: structural_presence, ok: bool, errors: list[string]}}. \
         Own-export qualification proves only this checking behavior, never complete candidate-class Q1, \
         semantic agreement, primitive registration, workflow validity or component activation."
    );
    let rule_contract = rule_contract();
    let pc_draft = ComponentRevisionDraft::from_json(
        &json!({
            "format": "component-revision/1",
            "uuid": pc_id,
            "class_code": 22,
            "document": {
                "name": format!("check-structural-presence-class-{class_code}"),
                "description": pc_description,
                "content": pc_body,
                "includes": [],
                "dependency_registry": null,
                "input_contract": {
                    "subject_json": {"type": "string", "required": true, "checks": []}
                },
                "result_contract": rule_contract,
                "preload": {
                    "format": "python-preload/2",
                    "exports": {
                        "check_structural_presence": {
                            "symbol": "check_structural_presence",
                            "parameters": ["inputs"],
                            "mapping": true
                        }
                    },
                    "private_functions": {},
                    "constants": [],
                    "imports": ["json"],
                    "dependencies": [],
                    "default_export": "check_structural_presence"
                }
            },
            "dependencies": [],
            "association": null
        })
        .to_string(),
    )?;

    // 2. Collect step (single rule).
    let collect_draft = collect_draft(collect_id, &[rule_id.to_string()])?;

    // 3. Aggregate step.
    let aggregate_draft = aggregate_draft(aggregate_id)?;

    // 4. Receipt encoder step — serialises the per-step results into the
    //    `validator-structural-receipt/1` JSON string that becomes the task's
    //    internal receipt. All executed step results are included so that
    //    observe_subject can verify each step's fingerprint against the receipt.
    //    Pure-logic; no host calls; imports json only.
    let receipt_encoder_body = concat!(
        "def encode_structural_receipt(inputs):\n",
        "    import json as _json\n",
        "    _receipt = _json.dumps({\n",
        "        \"format\": \"validator-structural-receipt/1\",\n",
        "        \"results\": {\n",
        "            \"0:1\": inputs[\"rule_result\"],\n",
        "            \"0:2\": inputs[\"collect_result\"],\n",
        "            \"0:3\": inputs[\"aggregate\"],\n",
        "        }\n",
        "    })\n",
        "    return _receipt\n",
    );
    let receipt_draft = ComponentRevisionDraft::from_json(
        &json!({
            "format": "component-revision/1",
            "uuid": receipt_id,
            "class_code": 22,
            "document": {
                "name": format!("encode-structural-receipt-class-{class_code}"),
                "description": format!(
                    "Encode class-{class_code} structural validator step results into \
                     the validator-structural-receipt/1 JSON string. Pure-logic, no host calls."
                ),
                "content": receipt_encoder_body,
                "includes": [],
                "dependency_registry": null,
                "input_contract": {
                    "rule_result": {
                        "type": "object", "required": true, "checks": [], "allow_extra_fields": false,
                        "fields": rule_contract["fields"].clone()
                    },
                    "collect_result": {"type": "list", "required": true, "checks": [], "items": rule_contract.clone()},
                    "aggregate": {
                        "type": "object", "required": true, "checks": [], "allow_extra_fields": false,
                        "fields": aggregate_draft.document()["result_contract"]["fields"].clone()
                    }
                },
                "result_contract": {"type": "string"},
                "preload": {
                    "format": "python-preload/2",
                    "exports": {
                        "encode_structural_receipt": {
                            "symbol": "encode_structural_receipt",
                            "parameters": ["inputs"],
                            "mapping": true
                        }
                    },
                    "private_functions": {},
                    "constants": [],
                    "imports": ["json"],
                    "dependencies": [],
                    "default_export": "encode_structural_receipt"
                }
            },
            "dependencies": [],
            "association": null
        })
        .to_string(),
    )?;

    // 5. Recipe with validator_coverage.
    // Build document using Value directly because the variant_key is dynamic.
    let recipe_description = format!(
        "Private incomplete shallow structural-check workflow for class {class_code} ({class_label}). \
         Collects actual presence-check diagnostics; neither helper qualification nor this draft \
         supplies complete candidate-class Q1 or approval for its other pure steps. \
         Consumer_tags=[05:validator]; no public activation is implied."
    );
    let variant_key = format!("validator-class-{class_code}");
    // The task input is the full sealed subject JSON string. The PC body parses
    // it internally; no source substitution occurs — subject_json is typed data.
    let pc_input_contract = json!({
        "subject_json": {"type": "string", "required": true, "checks": []}
    });

    // Verify the aggregate result contract has the expected shape required by
    // RetainedValidatorCoverage::from_inputs (pass:bool + errors:list).
    let agg = &aggregate_draft.document()["result_contract"];
    if agg["type"] != "object"
        || agg["allow_extra_fields"] != false
        || agg["fields"]["pass"]["type"] != "boolean"
        || agg["fields"]["errors"]["type"] != "list"
    {
        return Err(RevisionError::Invalid(
            "aggregate result contract does not match required shape",
        ));
    }

    // Build the input_layouts and invocations with dynamic keys.
    // Step 0:1 receives subject_json as its sole task_input; the PC body parses
    // it internally. This satisfies RetainedValidatorCoverage::subject_task_inputs
    // which checks that at least one step binding references a task input named
    // "subject_json".
    let layout_steps = {
        let mut steps = serde_json::Map::new();
        steps.insert(
            "0:1".into(),
            json!({
                "subject_json": {"kind": "task_input", "reference": "{{vars.subject_json}}"}
            }),
        );
        let mut collect_inputs = serde_json::Map::new();
        collect_inputs.insert(
            rule_id.to_string(),
            json!({"kind": "result", "step_id": rule_step, "path": []}),
        );
        steps.insert("0:2".into(), serde_json::Value::Object(collect_inputs));
        steps.insert(
            "0:3".into(),
            json!({
                "required_rule_ids": {"kind": "constant", "value": [rule_id]},
                "checks": {"kind": "result", "step_id": collect_step, "path": []}
            }),
        );
        // Receipt encoder: takes the rule result, collect result and aggregate result.
        steps.insert(
            receipt_step.to_string(),
            json!({
                "rule_result":    {"kind": "result", "step_id": rule_step,      "path": []},
                "collect_result": {"kind": "result", "step_id": collect_step,   "path": []},
                "aggregate":      {"kind": "result", "step_id": aggregate_step, "path": []}
            }),
        );
        serde_json::Value::Object(steps)
    };
    let layout = json!({
        "format": "recipe-input-layout/1",
        "task_inputs": pc_input_contract,
        "steps": layout_steps
    });
    let input_layouts = {
        let mut m = serde_json::Map::new();
        m.insert(variant_key.clone(), layout);
        serde_json::Value::Object(m)
    };
    let invocations = {
        let mut inv = serde_json::Map::new();
        inv.insert(
            variant_key.clone(),
            json!({
                "0:1": "check_structural_presence",
                "0:2": "collect_validator_checks",
                "0:3": "aggregate_validator_checks",
                "0:4": "encode_structural_receipt"
            }),
        );
        serde_json::Value::Object(inv)
    };

    let recipe_doc = json!({
        "name": format!("validator-class-{class_code}"),
        "description": recipe_description,
        "validates_class_code": class_code,
        "consumer_tags": ["05:validator"],
        "validator_coverage": {
            "format": "validator-coverage/1",
            "rules": [{"rule_id": rule_id, "step_id": rule_step}],
            "aggregate_step": aggregate_step,
            "receipt_step": receipt_step
        },
        "variants": [{
            "variant_key": variant_key,
            "step_link": "0:1-0:E",
            "intent_examples": [],
            "variable_patterns": []
        }],
        "step_descriptions": [{"desc_idx": 0, "label": format!("Structural check class {class_code}"), "yaml_source": "", "steps": [
            {"stepnumber": 1, "knowledge": "orchestrator", "goal": "Run structural presence check", "content": "", "type": "component", "include": [pc_id]},
            {"stepnumber": 2, "knowledge": "orchestrator", "goal": "Collect rule results", "content": "", "type": "component", "include": [collect_id]},
            {"stepnumber": 3, "knowledge": "orchestrator", "goal": "Aggregate required rule set", "content": "", "type": "component", "include": [aggregate_id]},
            {"stepnumber": 4, "knowledge": "orchestrator", "goal": "Encode step results into receipt JSON string", "content": "", "type": "component", "include": [receipt_id]}
        ]}],
        "input_layouts": input_layouts,
        "invocations": invocations
    });

    let recipe_draft = ComponentRevisionDraft::from_json(
        &json!({
            "format": "component-revision/1",
            "uuid": recipe_id,
            "class_code": 21,
            "document": recipe_doc,
            "dependencies": [pc_id, collect_id, aggregate_id, receipt_id],
            "association": null
        })
        .to_string(),
    )?;

    Ok(vec![
        pc_draft,
        collect_draft,
        aggregate_draft,
        receipt_draft,
        recipe_draft,
    ])
}

#[cfg(test)]
mod structural_validator_recipe_tests {
    use super::*;
    use crate::global_bootstrap_components::packaged_component_id;

    const MINIMAL_PC_BODY: &str = concat!(
        "def check_structural_presence(inputs):\n",
        "    import json as _json\n",
        "    _subj = _json.loads(inputs[\"subject_json\"])\n",
        "    _doc = _subj[\"candidate\"][\"revision\"][\"document\"]\n",
        "    _name = _doc.get(\"name\", \"\")\n",
        "    _desc = _doc.get(\"description\", \"\")\n",
        "    _content = _doc.get(\"content\", \"\")\n",
        "    _errors = []\n",
        "    if not _name or not str(_name).strip(): _errors.append(\"name is empty\")\n",
        "    if not _desc or not str(_desc).strip(): _errors.append(\"description is empty\")\n",
        "    if not _content or not str(_content).strip(): _errors.append(\"content/steps is empty\")\n",
        "    return {\"rule_id\": \"structural_presence\", \"ok\": len(_errors) == 0, \"errors\": _errors}\n",
    );

    #[test]
    fn all_eight_class_codes_produce_five_drafts() {
        for &class_code in VALIDATOR_CLASS_CODES {
            let label = format!("class-{class_code}");
            let drafts = structural_validator_recipe_drafts(class_code, &label, MINIMAL_PC_BODY)
                .unwrap_or_else(|err| {
                    panic!("structural_validator_recipe_drafts({class_code}) failed: {err}")
                });
            assert_eq!(
                drafts.len(),
                5,
                "expected 5 drafts for class {class_code}, got {}",
                drafts.len()
            );
        }
    }

    #[test]
    fn unsupported_class_code_is_rejected() {
        for &bad in &[4i16, 5, 7, 8, 9, 10, 11, 12, 14, 15, 16, 24, 50] {
            assert!(
                structural_validator_recipe_drafts(bad, "bad", MINIMAL_PC_BODY).is_err(),
                "class code {bad} should be rejected"
            );
        }
    }

    #[test]
    fn pc_draft_is_class_22() {
        let drafts = structural_validator_recipe_drafts(22, "PythonCode", MINIMAL_PC_BODY).unwrap();
        let pc = &drafts[0];
        assert_eq!(
            pc.class_code(),
            22,
            "first draft must be PythonCode (class 22)"
        );
    }

    #[test]
    fn recipe_draft_is_class_21_with_validator_coverage() {
        for &class_code in VALIDATOR_CLASS_CODES {
            let drafts = structural_validator_recipe_drafts(
                class_code,
                &format!("class-{class_code}"),
                MINIMAL_PC_BODY,
            )
            .unwrap();
            let recipe = &drafts[4];
            assert_eq!(
                recipe.class_code(),
                21,
                "fifth draft must be Recipe (class 21) for class {class_code}"
            );
            let doc = recipe.document();
            let coverage = doc.get("validator_coverage").unwrap_or_else(|| {
                panic!("validator_coverage missing in class {class_code} recipe doc")
            });
            assert_eq!(
                coverage["format"], "validator-coverage/1",
                "validator_coverage format mismatch for class {class_code}"
            );
            assert_eq!(
                doc["validates_class_code"].as_i64().unwrap(),
                i64::from(class_code),
                "validates_class_code mismatch"
            );
            assert_eq!(
                doc["consumer_tags"],
                serde_json::json!(["05:validator"]),
                "consumer_tags must be exactly [\"05:validator\"] for class {class_code}"
            );
            // Verify there is exactly one rule in the coverage.
            let rules = coverage["rules"].as_array().unwrap();
            assert_eq!(
                rules.len(),
                1,
                "expected exactly one rule for class {class_code}"
            );
            assert_eq!(rules[0]["rule_id"], "structural_presence");
            assert_eq!(rules[0]["step_id"], "0:1");
        }
    }

    #[test]
    fn stable_packaged_ids_are_consistent() {
        // ID functions should be deterministic and distinct per class.
        let ids_22 = (
            structural_validator_pc_id(22),
            structural_validator_collect_id(22),
            structural_validator_aggregate_id(22),
            structural_validator_recipe_id(22),
        );
        let ids_0 = (
            structural_validator_pc_id(0),
            structural_validator_collect_id(0),
            structural_validator_aggregate_id(0),
            structural_validator_recipe_id(0),
        );
        // Per-class IDs are distinct from each other within the same class.
        assert_ne!(ids_22.0, ids_22.1);
        assert_ne!(ids_22.0, ids_22.2);
        assert_ne!(ids_22.0, ids_22.3);
        assert_ne!(ids_22.1, ids_22.2);
        assert_ne!(ids_22.1, ids_22.3);
        assert_ne!(ids_22.2, ids_22.3);
        // Class-22 and class-0 IDs are distinct.
        assert_ne!(ids_22.0, ids_0.0);
        // IDs are in the packaged namespace (v5 UUID based on a fixed namespace).
        assert_eq!(
            ids_22.0,
            packaged_component_id("validator:structural:pc:class-22")
        );
        assert_eq!(
            ids_22.3,
            packaged_component_id("validator:structural:recipe:class-22")
        );
    }

    #[test]
    fn recipe_draft_dependencies_match_pc_collect_aggregate_receipt() {
        let drafts = structural_validator_recipe_drafts(22, "PythonCode", MINIMAL_PC_BODY).unwrap();
        let pc_id = drafts[0].uuid();
        let collect_id = drafts[1].uuid();
        let aggregate_id = drafts[2].uuid();
        let receipt_id = drafts[3].uuid();
        let recipe_deps = drafts[4].dependencies();
        assert!(recipe_deps.contains(&pc_id), "recipe deps must include pc");
        assert!(
            recipe_deps.contains(&collect_id),
            "recipe deps must include collect"
        );
        assert!(
            recipe_deps.contains(&aggregate_id),
            "recipe deps must include aggregate"
        );
        assert!(
            recipe_deps.contains(&receipt_id),
            "recipe deps must include receipt encoder"
        );
        assert_eq!(
            recipe_deps.len(),
            4,
            "recipe should have exactly 4 dependencies"
        );
    }

    #[test]
    fn input_layouts_uses_variant_key() {
        let drafts = structural_validator_recipe_drafts(0, "Tool", MINIMAL_PC_BODY).unwrap();
        let doc = drafts[4].document();
        let expected_key = "validator-class-0";
        assert!(
            doc["input_layouts"]
                .as_object()
                .unwrap()
                .contains_key(expected_key),
            "input_layouts should use variant key '{expected_key}'"
        );
        assert!(
            doc["invocations"]
                .as_object()
                .unwrap()
                .contains_key(expected_key),
            "invocations should use variant key '{expected_key}'"
        );
    }

    #[test]
    fn aggregate_result_shape_is_validated() {
        // structural_validator_recipe_drafts rejects if aggregate result doesn't
        // match pass:bool + errors:list shape (enforced inside the factory).
        // All eight classes use aggregate_draft() which produces the correct shape.
        for &class_code in VALIDATOR_CLASS_CODES {
            let result = structural_validator_recipe_drafts(class_code, "label", MINIMAL_PC_BODY);
            assert!(
                result.is_ok(),
                "class {class_code} should succeed with correct aggregate shape"
            );
        }
    }
}
