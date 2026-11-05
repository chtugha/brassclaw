//! Private pure-source review drafts derived from the existing structure Recipe.
//! This is authoring only, not a new runner, provenance seal or Q1 writer.
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    global_bootstrap_components::{UsageComponentIds, packaged_component_id},
    value_contract::{InputContract, ValueContract},
};

pub fn python_pure_calls_draft(
    id: Uuid,
    source: &str,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let mut check = super::rule_contract();
    check["required"] = json!(true);
    let mut profile = super::python_structure::profile_contract();
    profile["required"] = json!(true);
    let subject_reader = packaged_component_id("validator:review-subject-reader");
    let draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"check-review-python-pure-calls",
            "description":"After actual selected definition/contract/graph checks, require no host call anywhere in the complete pure closure, including private/dead/unselected code. Walk actual native call and function-value edges from every public root export. Reject recursive reachable graphs unsupported by retained invocation and calling immutable constants as functions. Preserve unused pure helper definitions without inventing Tool usages. Inspect source only; no candidate invocation, complete behavior/semantic approval, caller authentication, trusted evidence or activation. The existing trusted owner must bind original native facts/profile/prior results and separately observe every required export behavior.",
            "content":source,"includes":[],"dependency_registry":null,
            "input_contract":{"subject_json":{"type":"string","required":true,"checks":[]},
                "inventory_json":{"type":"string","required":true,"checks":[]},"profile":profile,"graph":check},
            "result_contract":super::rule_contract(),
            "preload":{"format":"python-preload/2","exports":{
                "check_review_python_pure_calls":{"symbol":"check_review_python_pure_calls","parameters":["inputs"],"mapping":true}
            },"private_functions":{"_validator_python_pure_calls_result":["errors"],
                "_validator_python_reachable_functions":["scopes","symbols","builtins","constants"]},"constants":[],"imports":["json"],"dependencies":[subject_reader],"default_export":"check_review_python_pure_calls"}
        },"dependencies":[subject_reader],"association":null
    }).to_string())?;
    validate(&draft)?;
    Ok(draft)
}

/// Reuse the whole structure definition; add the pure rule only to a distinct
/// private workflow. Tool-using Python must not be rejected by a generic check
/// merely for being effectful. Neither workflow is activated by retention.
pub fn python_pure_structure_drafts(
    inventory: UsageComponentIds,
    receipt_source: &str,
    profile: Value,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let base = super::python_structure_drafts(inventory, receipt_source, profile)?;
    let old_id = |part| packaged_component_id(&format!("validator:python-structure:{part}"));
    let new_id = |part| packaged_component_id(&format!("validator:python-pure-structure:{part}"));
    let pure_calls = packaged_component_id("validator:python-pure-calls");
    let identities = std::collections::BTreeSet::from([
        new_id("recipe"),
        new_id("collect"),
        new_id("aggregate"),
        new_id("receipt"),
        pure_calls,
        packaged_component_id("validator:subject-sources"),
        packaged_component_id("validator:python-definitions"),
        packaged_component_id("validator:python-contracts"),
        packaged_component_id("validator:python-graph"),
        inventory.recipe,
        inventory.python_code,
        inventory.skill,
        inventory.tool_skill,
        inventory.tool,
    ]);
    if identities.len() != 14 || identities.contains(&Uuid::nil()) {
        return Err(RevisionError::Invalid(
            "distinct private pure structure identities required",
        ));
    }
    let collection = super::collect_draft(
        new_id("collect"),
        &[
            "python_definitions".into(),
            "python_contracts".into(),
            "python_graph".into(),
            "python_pure_calls".into(),
        ],
    )?;
    let aggregation = super::aggregate_draft(new_id("aggregate"))?;
    let base_draft = |id| {
        base.iter()
            .find(|draft| draft.uuid() == id)
            .ok_or_else(invalid)
    };
    let mut receipt = value(base_draft(old_id("receipt"))?)?;
    receipt["uuid"] = json!(new_id("receipt"));
    receipt["document"]["name"] = json!("encode-python-pure-structure-results");
    let mut rule = super::rule_contract();
    rule["required"] = json!(true);
    receipt["document"]["input_contract"]["pure_calls"] = rule;
    let receipt = ComponentRevisionDraft::from_json(&receipt.to_string())?;
    validate(&receipt)?;
    let mut recipe = value(base_draft(old_id("recipe"))?)?;
    recipe["uuid"] = json!(new_id("recipe"));
    let document = &mut recipe["document"];
    document["name"] = json!("validator-python-pure-structure");
    document["description"] = json!(
        "Private incomplete pure-review draft: reuse original-source/native inventory/definition/contract/graph checks, then require a host-free closure and qualify static function references for every public root export. Collect all actual results for sealed observation. No complete class Q1, behavioral/semantic approval, production review admission, trusted evidence, activation or permission is supplied."
    );
    document["validator_coverage"]["rules"]
        .as_array_mut()
        .ok_or_else(invalid)?
        .push(json!({"rule_id":"python_pure_calls","step_id":"0:7"}));
    document["validator_coverage"]["aggregate_step"] = json!("0:9");
    document["validator_coverage"]["receipt_step"] = json!("0:10");
    let steps = document["step_descriptions"][0]["steps"]
        .as_array_mut()
        .ok_or_else(invalid)?;
    for (index, step) in steps.iter_mut().enumerate() {
        if index >= 6 {
            step["stepnumber"] = json!(index + 2);
        }
        for part in ["collect", "aggregate", "receipt"] {
            if step["include"] == json!([old_id(part)]) {
                step["include"] = json!([new_id(part)]);
            }
        }
    }
    steps.insert(
        6,
        json!({"stepnumber":7,"knowledge":"orchestrator",
        "goal":"Check the entire pure closure and every public root function graph",
        "content":"","type":"component","include":[pure_calls]}),
    );
    let layout = document["input_layouts"]["selected"]["steps"]
        .as_object_mut()
        .ok_or_else(invalid)?;
    for (from, to) in [("0:9", "0:10"), ("0:8", "0:9"), ("0:7", "0:8")] {
        let inputs = layout.remove(from).ok_or_else(invalid)?;
        layout.insert(to.into(), inputs);
    }
    let selected_profile = layout["0:6"]["profile"].clone();
    layout.insert(
        "0:7".into(),
        json!({
            "subject_json":{"kind":"task_input","reference":"{{vars.subject_json}}"},
            "inventory_json":{"kind":"result","step_id":"0:3","path":[]},
            "profile":selected_profile,"graph":{"kind":"result","step_id":"0:6","path":[]}
        }),
    );
    layout.get_mut("0:8").ok_or_else(invalid)?["python_pure_calls"] =
        json!({"kind":"result","step_id":"0:7","path":[]});
    let aggregate_inputs = layout.get_mut("0:9").ok_or_else(invalid)?;
    aggregate_inputs["required_rule_ids"]["value"] = json!([
        "python_definitions",
        "python_contracts",
        "python_graph",
        "python_pure_calls"
    ]);
    aggregate_inputs["checks"]["step_id"] = json!("0:8");
    let receipt_inputs = layout.get_mut("0:10").ok_or_else(invalid)?;
    receipt_inputs["pure_calls"] = json!({"kind":"result","step_id":"0:7","path":[]});
    receipt_inputs["checks"]["step_id"] = json!("0:8");
    receipt_inputs["aggregation"]["step_id"] = json!("0:9");
    let invocations = document["invocations"]["selected"]
        .as_object_mut()
        .ok_or_else(invalid)?;
    for (from, to) in [("0:9", "0:10"), ("0:8", "0:9"), ("0:7", "0:8")] {
        let export = invocations.remove(from).ok_or_else(invalid)?;
        invocations.insert(to.into(), export);
    }
    invocations.insert("0:7".into(), json!("check_review_python_pure_calls"));
    let dependencies = recipe["dependencies"].as_array_mut().ok_or_else(invalid)?;
    for dependency in dependencies.iter_mut() {
        for part in ["collect", "aggregate", "receipt"] {
            if *dependency == json!(old_id(part)) {
                *dependency = json!(new_id(part));
            }
        }
    }
    dependencies.push(json!(pure_calls));
    let recipe = ComponentRevisionDraft::from_json(&recipe.to_string())?;
    Ok(vec![collection, aggregation, receipt, recipe])
}

fn invalid() -> RevisionError {
    RevisionError::Invalid("complete shared private structure definition required")
}
fn value(draft: &ComponentRevisionDraft) -> Result<Value, RevisionError> {
    serde_json::from_str(draft.exact_bytes()).map_err(|_| invalid())
}
fn validate(draft: &ComponentRevisionDraft) -> Result<(), RevisionError> {
    InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS)?;
    ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)?;
    Ok(())
}
