//! Installation drafts for the source/definition/contract part of private review.
//! They deliberately supply neither the complete class check set nor Q1 evidence.
use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    global_bootstrap_components::{UsageComponentIds, packaged_component_id},
    value_contract::{InputContract, ValueContract},
};

pub fn python_definitions_draft(
    id: Uuid,
    definition_rule: Uuid,
    source: &str,
) -> Result<ComponentRevisionDraft, RevisionError> {
    if id.is_nil() || definition_rule.is_nil() || id == definition_rule {
        return Err(RevisionError::Invalid(
            "distinct Python definition rule identities required",
        ));
    }
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{
            "name":"check-review-python-definitions",
            "description":"Compare every class-22 candidate/dependency declaration from the complete strict sealed subject with the existing native inventory. Require complete matching source IDs; preserve compiler-profile rejections and every signature/symbol/import/load-shape diagnostic from the pinned check_preload_definition export. Validate closed declaration shapes before internal calls and compare actual retained dependency declarations. Never execute candidate source, invent native facts, authenticate caller JSON, infer dependency/call/binding/semantic agreement, approve components or write evidence. The trusted owner must observe original subject/native Tool inputs/results and verify the retained interpreter and admitted attempt before evidence.",
            "content":source,"includes":[],"dependency_registry":null,
            "input_contract":{
                "subject_json":{"type":"string","required":true,"checks":[]},
                "inventory_json":{"type":"string","required":true,"checks":[]}
            },"result_contract":super::rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_review_python_definitions":{"symbol":"check_review_python_definitions","parameters":["inputs"],"mapping":true}
            },"private_functions":{"_validator_review_declaration_shape":["value"]},"constants":[],"imports":["json"],"dependencies":[definition_rule],"default_export":"check_review_python_definitions"}
        },"dependencies":[definition_rule],"association":null
    }).to_string())?;
    validate(&draft)?;
    Ok(draft)
}

/// Reuse the actual inventory usage/native identity. This Recipe has no public
/// matching command and remains incomplete/unreviewed until its owning validator
/// plan finishes mandatory coverage and trusted evidence. No activation occurs.
pub fn python_structure_drafts(
    inventory: UsageComponentIds,
    receipt_source: &str,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let recipe = packaged_component_id("validator:python-structure:recipe");
    let extractor = packaged_component_id("validator:subject-sources");
    let definitions = packaged_component_id("validator:python-definitions");
    let contracts = packaged_component_id("validator:python-contracts");
    let collector = packaged_component_id("validator:python-structure:collect");
    let aggregate = packaged_component_id("validator:python-structure:aggregate");
    let receipt = packaged_component_id("validator:python-structure:receipt");
    let identities = BTreeSet::from([
        recipe,
        extractor,
        definitions,
        contracts,
        collector,
        aggregate,
        receipt,
        inventory.recipe,
        inventory.python_code,
        inventory.skill,
        inventory.tool_skill,
        inventory.tool,
    ]);
    if identities.len() != 12 || identities.contains(&Uuid::nil()) || inventory.formatter.is_some()
    {
        return Err(RevisionError::Invalid(
            "distinct Python structure/usage identities required",
        ));
    }
    let collection = super::collect_draft(
        collector,
        &["python_definitions".into(), "python_contracts".into()],
    )?;
    let aggregation = super::aggregate_draft(aggregate)?;
    let text = json!({"type":"string","checks":[]});
    let sources = json!({"type":"object","checks":[],"fields":{},"allow_extra_fields":true,"extra_fields":text});
    let required = |mut schema: Value| {
        schema["required"] = json!(true);
        schema
    };
    let receipt = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":receipt,"class_code":22,
        "document":{"name":"encode-python-structure-results",
            "description":"Return all actual source-map, native inventory, rule, collection and aggregate results as JSON string data for the existing trusted observer. No verdict, evidence or approval is written; the owner verifies every actual input/result edge and native custody separately.",
            "content":receipt_source,"includes":[],"dependency_registry":null,
            "input_contract":{
                "sources":required(sources),"inventory_json":required(text.clone()),
                "definitions":required(super::rule_contract()),"contracts":required(super::rule_contract()),
                "checks":required(collection.document()["result_contract"].clone()),
                "aggregation":required(aggregation.document()["result_contract"].clone())
            },"result_contract":text,
            "preload":{"format":"python-preload/2","exports":{
                "encode_python_structure_results":{"symbol":"encode_python_structure_results","parameters":["inputs"],"mapping":true}
            },"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"encode_python_structure_results"}
        },"dependencies":[],"association":null
    }).to_string())?;
    let recipe = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":recipe,"class_code":21,
        "document":{"name":"validator-python-structure",
            "description":"Private incomplete review draft: inspect the complete original Python source inventory once, check definitions and recursive public contracts, collect all actual diagnostics and return them for sealed observation. This is not complete class-22 Q1, behavior/semantic evidence, trusted approval or an active route.",
            "validates_class_code":22,"consumer_tags":["05:validator"],
            "validator_coverage":{"format":"validator-coverage/1","rules":[
                {"rule_id":"python_definitions","step_id":"0:4"},{"rule_id":"python_contracts","step_id":"0:5"}
            ],"aggregate_step":"0:7"},
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":[],"variable_patterns":[]}],
            "step_descriptions":[{"desc_idx":0,"label":"Inspect original definitions and contracts","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"orchestrator","goal":"Extract all selected Python sources","content":"","type":"component","include":[extractor]},
                {"stepnumber":2,"knowledge":"rust","goal":"Prepare the existing inventory usage binding","content":"","type":"component","include":[inventory.tool_skill],
                    "tool_bindings":[{"tool_id":inventory.tool,"tool_name":"source_inspection","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":3,"knowledge":"orchestrator","goal":"Inspect the complete source graph without executing candidates","content":"","type":"component","include":[inventory.python_code]},
                {"stepnumber":4,"knowledge":"orchestrator","goal":"Compare every original declaration and actual native facts","content":"","type":"component","include":[definitions]},
                {"stepnumber":5,"knowledge":"orchestrator","goal":"Check every selected public contract","content":"","type":"component","include":[contracts]},
                {"stepnumber":6,"knowledge":"orchestrator","goal":"Collect both complete rule results","content":"","type":"component","include":[collector]},
                {"stepnumber":7,"knowledge":"orchestrator","goal":"Aggregate the pinned required rule set","content":"","type":"component","include":[aggregate]},
                {"stepnumber":8,"knowledge":"orchestrator","goal":"Return complete results for the sealed observer","content":"","type":"component","include":[receipt.uuid()]}
            ]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"subject_json":{"type":"string","required":true,"checks":[]}},"steps":{
                "0:1":{"subject_json":{"kind":"task_input","reference":"{{vars.subject_json}}"}},
                "0:3":{"sources":{"kind":"result","step_id":"0:1","path":[]}},
                "0:4":{"subject_json":{"kind":"task_input","reference":"{{vars.subject_json}}"},"inventory_json":{"kind":"result","step_id":"0:3","path":[]}},
                "0:5":{"subject_json":{"kind":"task_input","reference":"{{vars.subject_json}}"}},
                "0:6":{"python_definitions":{"kind":"result","step_id":"0:4","path":[]},"python_contracts":{"kind":"result","step_id":"0:5","path":[]}},
                "0:7":{"required_rule_ids":{"kind":"constant","value":["python_definitions","python_contracts"]},"checks":{"kind":"result","step_id":"0:6","path":[]}},
                "0:8":{"sources":{"kind":"result","step_id":"0:1","path":[]},"inventory_json":{"kind":"result","step_id":"0:3","path":[]},
                    "definitions":{"kind":"result","step_id":"0:4","path":[]},"contracts":{"kind":"result","step_id":"0:5","path":[]},
                    "checks":{"kind":"result","step_id":"0:6","path":[]},"aggregation":{"kind":"result","step_id":"0:7","path":[]}}
            }}},"invocations":{"selected":{"0:1":"extract_review_python_sources","0:3":"inspect_python_inventory",
                "0:4":"check_review_python_definitions","0:5":"check_review_python_contracts","0:6":"collect_validator_checks","0:7":"aggregate_validator_checks","0:8":"encode_python_structure_results"}}
        },"dependencies":[extractor,inventory.tool_skill,inventory.python_code,inventory.skill,definitions,contracts,collector,aggregate,receipt.uuid()],"association":null
    }).to_string())?;
    for draft in [&collection, &aggregation, &receipt] {
        validate(draft)?;
    }
    Ok(vec![collection, aggregation, receipt, recipe])
}

fn validate(draft: &ComponentRevisionDraft) -> Result<(), RevisionError> {
    InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS)?;
    ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)?;
    Ok(())
}
