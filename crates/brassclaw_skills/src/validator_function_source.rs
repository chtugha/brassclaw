//! Canonical private joint-source graph. These drafts grant no qualification,
//! admission, whole-function Q1, behavior, human Q2 or component activation.
use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    global_bootstrap_components::{UsageComponentIds, packaged_component_id},
    value_contract::{InputContract, ValueContract},
};

pub const FUNCTION_SOURCE_RECEIPT_SOURCE: &str =
    include_str!("validator_function_source_receipt.py");

/// Each primary class has a private route; source checks always consume both
/// exact physical partners and their complete original dependency selection.
pub fn function_source_recipe_id(primary_class: i16) -> Uuid {
    packaged_component_id(&format!(
        "validator:function-source:class-{primary_class}:recipe"
    ))
}

pub fn function_source_drafts(
    inventory: UsageComponentIds,
    primary_class: i16,
    profile: Value,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    if !matches!(primary_class, 1..=3 | 22) {
        return Err(RevisionError::Invalid(
            "function source primary class is unsupported",
        ));
    }
    let id = |part: &str| match part {
        "recipe" => function_source_recipe_id(primary_class),
        // The qualified aggregate already accepts any exact nonempty rule set.
        "aggregate" => packaged_component_id("validator:python-pure-structure:aggregate"),
        _ => packaged_component_id(&format!("validator:function-source:{part}")),
    };
    let shared = |part: &str| packaged_component_id(&format!("validator:{part}"));
    let identities = BTreeSet::from([
        id("recipe"),
        id("collect"),
        id("aggregate"),
        id("receipt"),
        shared("subject-sources"),
        shared("python-definitions"),
        shared("python-contracts"),
        shared("python-graph"),
        shared("skill-interface"),
        shared("skill-call-targets"),
        inventory.recipe,
        inventory.tool,
        inventory.tool_skill,
        inventory.skill,
        inventory.python_code,
    ]);
    if identities.len() != 15 || identities.contains(&Uuid::nil()) {
        return Err(invalid());
    }
    // Reuse the actual existing inventory binding, typed profile and original
    // structure steps. Only this new workflow's collector/receipt identities
    // change; no alternate source is constructed under an existing UUID.
    let base = super::python_structure_drafts(
        inventory,
        include_str!("validator_structure_receipt.py"),
        profile,
    )?;
    let base_recipe = base
        .iter()
        .find(|draft| draft.class_code() == 21)
        .ok_or_else(invalid)?;
    let mut recipe: Value =
        serde_json::from_str(base_recipe.exact_bytes()).map_err(|_| invalid())?;
    recipe["uuid"] = json!(id("recipe"));
    let collection = super::collect_draft(
        id("collect"),
        &super::FUNCTION_SOURCE_RULE_IDS.map(str::to_owned),
    )?;
    let aggregation = super::aggregate_draft(id("aggregate"))?;
    let mut receipt: Value = serde_json::from_str(
        base.iter()
            .find(|draft| draft.uuid() == shared("python-structure:receipt"))
            .ok_or_else(invalid)?
            .exact_bytes(),
    )
    .map_err(|_| invalid())?;
    receipt["uuid"] = json!(id("receipt"));
    receipt["document"]["name"] = json!("encode-function-source-results");
    receipt["document"]["description"] = json!(
        "Encode complete original sources, native inventory, five actual rule results, collection and aggregation for the sealed observer. No candidate code execution, verdict qualification, approval or activation."
    );
    receipt["document"]["content"] = json!(FUNCTION_SOURCE_RECEIPT_SOURCE);
    let mut rule = super::rule_contract();
    rule["required"] = json!(true);
    receipt["document"]["input_contract"]["interface"] = rule.clone();
    receipt["document"]["input_contract"]["targets"] = rule;
    receipt["document"]["preload"]["exports"] = json!({
        "encode_function_source_results":{"symbol":"encode_function_source_results","parameters":["inputs"],"mapping":true}
    });
    receipt["document"]["preload"]["default_export"] = json!("encode_function_source_results");
    let receipt = ComponentRevisionDraft::from_json(&receipt.to_string())?;

    let document = &mut recipe["document"];
    document["name"] = json!(format!("validator-function-source-class-{primary_class}"));
    document["description"] = json!(
        "Private joint Skill/implementation source observations: one native inventory, actual definitions/contracts/graph, exact Skill interface and reachable declared Host targets. Includes both physical partners and unchanged context. This is not complete argument/result/failure compatibility, behavior, whole-function Q1, human Q2, approval or activation."
    );
    document["validates_class_code"] = json!(primary_class);
    document["validator_coverage"] = json!({"format":"validator-coverage/2","subject_kind":"function",
        "rules":super::FUNCTION_SOURCE_RULE_IDS.iter().enumerate().map(|(index, rule)|
            json!({"rule_id":rule,"step_id":format!("0:{}", index+4)})).collect::<Vec<_>>(),
        "aggregate_step":"0:10","receipt_step":"0:11"});
    let steps = document["step_descriptions"][0]["steps"]
        .as_array_mut()
        .ok_or_else(invalid)?;
    for (index, step) in steps.iter_mut().enumerate() {
        if index >= 6 {
            step["stepnumber"] = json!(index + 3);
        }
        for part in ["collect", "aggregate", "receipt"] {
            if step["include"] == json!([shared(&format!("python-structure:{part}"))]) {
                step["include"] = json!([id(part)]);
            }
        }
    }
    steps.insert(6, json!({"stepnumber":7,"knowledge":"orchestrator","goal":"Check the exact Skill and implementation interface","content":"","type":"component","include":[shared("skill-interface")]}));
    steps.insert(7, json!({"stepnumber":8,"knowledge":"orchestrator","goal":"Check reachable Host targets against the associated Tool usage","content":"","type":"component","include":[shared("skill-call-targets")]}));
    let layout = document["input_layouts"]["selected"]["steps"]
        .as_object_mut()
        .ok_or_else(invalid)?;
    for (from, to) in [("0:9", "0:11"), ("0:8", "0:10"), ("0:7", "0:9")] {
        let bindings = layout.remove(from).ok_or_else(invalid)?;
        layout.insert(to.into(), bindings);
    }
    let subject = json!({"kind":"task_input","reference":"{{vars.subject_json}}"});
    let result = |step| json!({"kind":"result","step_id":step,"path":[]});
    let profile = layout
        .get("0:6")
        .and_then(|inputs| inputs.get("profile"))
        .cloned()
        .ok_or_else(invalid)?;
    layout.insert("0:7".into(), json!({"subject_json":subject}));
    layout.insert(
        "0:8".into(),
        json!({"subject_json":subject,"inventory_json":result("0:3"),
        "profile":profile,"graph":result("0:6"),"interface":result("0:7")}),
    );
    let collection_inputs = layout.get_mut("0:9").ok_or_else(invalid)?;
    collection_inputs["skill_interface"] = result("0:7");
    collection_inputs["skill_call_targets"] = result("0:8");
    let aggregate_inputs = layout.get_mut("0:10").ok_or_else(invalid)?;
    aggregate_inputs["required_rule_ids"]["value"] = json!(super::FUNCTION_SOURCE_RULE_IDS);
    aggregate_inputs["checks"] = result("0:9");
    let receipt_inputs = layout.get_mut("0:11").ok_or_else(invalid)?;
    receipt_inputs["interface"] = result("0:7");
    receipt_inputs["targets"] = result("0:8");
    receipt_inputs["checks"] = result("0:9");
    receipt_inputs["aggregation"] = result("0:10");
    let invocations = document["invocations"]["selected"]
        .as_object_mut()
        .ok_or_else(invalid)?;
    for (from, to) in [("0:9", "0:11"), ("0:8", "0:10"), ("0:7", "0:9")] {
        let invocation = invocations.remove(from).ok_or_else(invalid)?;
        invocations.insert(to.into(), invocation);
    }
    invocations.insert("0:7".into(), json!("check_review_skill_interface"));
    invocations.insert("0:8".into(), json!("check_review_skill_call_targets"));
    invocations.insert("0:11".into(), json!("encode_function_source_results"));
    let dependencies = recipe["dependencies"].as_array_mut().ok_or_else(invalid)?;
    for dependency in dependencies.iter_mut() {
        for part in ["collect", "aggregate", "receipt"] {
            if *dependency == json!(shared(&format!("python-structure:{part}"))) {
                *dependency = json!(id(part));
            }
        }
    }
    dependencies.extend([
        json!(shared("skill-interface")),
        json!(shared("skill-call-targets")),
    ]);
    let recipe = ComponentRevisionDraft::from_json(&recipe.to_string())?;
    super::ValidatorCoverageDeclaration::from_value(
        &recipe.document()["validator_coverage"],
        i32::from(primary_class),
    )
    .map_err(RevisionError::Invalid)?;
    for draft in [&collection, &aggregation, &receipt] {
        InputContract::from_value(&draft.document()["input_contract"], REVISION_LIMITS)?;
        ValueContract::result_from_value(&draft.document()["result_contract"], REVISION_LIMITS)?;
    }
    Ok(vec![collection, aggregation, receipt, recipe])
}

fn invalid() -> RevisionError {
    RevisionError::Invalid("invalid canonical function source graph")
}
