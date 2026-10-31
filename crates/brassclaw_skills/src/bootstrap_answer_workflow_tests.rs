use super::*;
use crate::{
    component_revision::REVISION_LIMITS,
    recipe_model_input::{PrefixFreePurpose, RecipeModelInput},
    value_contract::InputContract,
};
use std::collections::BTreeSet;

fn usage(tool: Uuid) -> UsageComponentIds {
    UsageComponentIds {
        recipe: Uuid::nil(), // Separate consuming Recipes are not dependencies.
        python_code: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        skill: Uuid::new_v4(),
        tool,
        formatter: None,
    }
}
fn inputs() -> (
    BootstrapAnswerWorkflowIds,
    UsageComponentIds,
    UsageComponentIds,
    StrictJsonUsageIds,
    Uuid,
    Uuid,
) {
    let response = usage(Uuid::new_v4());
    (
        BootstrapAnswerWorkflowIds {
            recipe: Uuid::new_v4(),
            prompt: Uuid::new_v4(),
            protocol: Uuid::new_v4(),
        },
        usage(Uuid::new_v4()),
        response,
        StrictJsonUsageIds {
            tool: response.tool,
            tool_skill: Uuid::new_v4(),
            python_code: Uuid::new_v4(),
            skill: Uuid::new_v4(),
        },
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
}

#[test]
fn bootstrap_protocol_graph_keeps_native_strict_boundary_and_original_packet_edges() {
    let (ids, model, response, strict, text, answer) = inputs();
    let drafts =
        bootstrap_answer_workflow_drafts(ids, model, response, strict, text, answer).unwrap();
    assert_eq!(drafts.len(), 3);
    assert_eq!(
        drafts[0].exact_bytes(),
        bootstrap_prompt_draft(ids.prompt).unwrap().exact_bytes()
    );
    let recipe = drafts
        .iter()
        .find(|draft| draft.uuid() == ids.recipe)
        .unwrap();
    let doc = recipe.document();
    assert_eq!(doc["variants"][0]["intent_examples"], json!([]));
    assert_eq!(doc["variants"][0]["variable_patterns"], json!([]));
    let steps = doc["step_descriptions"][0]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 9);
    for step in steps {
        assert_eq!(step["include"].as_array().unwrap().len(), 1);
    }
    for (index, binding, code) in [
        (1, model.tool_skill, model.python_code),
        (3, response.tool_skill, response.python_code),
        (6, strict.tool_skill, strict.python_code),
    ] {
        assert_eq!(steps[index]["knowledge"], "rust");
        assert_eq!(steps[index]["include"], json!([binding]));
        assert_eq!(steps[index + 1]["knowledge"], "orchestrator");
        assert_eq!(steps[index + 1]["include"], json!([code]));
    }
    assert_eq!(
        doc["input_layouts"]["selected"]["steps"]["0:8"]["json_text"]["step_id"],
        "0:6"
    );
    assert_eq!(
        doc["input_layouts"]["selected"]["steps"]["0:9"]["answer_bytes"]["step_id"],
        "0:8"
    );
    assert_eq!(
        doc["input_layouts"]["selected"]["steps"]["0:9"]["packet_bytes"]["reference"],
        "{{vars.packet_bytes}}"
    );
    assert_eq!(
        RecipeModelInput::for_variant(doc, &BTreeSet::from(["selected".into()]), "selected")
            .unwrap(),
        RecipeModelInput::PrefixFreeWorkflow {
            purpose: PrefixFreePurpose::BasicProviderConfigurationValues
        }
    );
    let deps = drafts
        .iter()
        .find(|draft| draft.uuid() == ids.protocol)
        .unwrap()
        .dependencies();
    assert_eq!(deps, &BTreeSet::from([answer]));
    assert!(!recipe.dependencies().contains(&Uuid::nil()));
}

#[test]
fn bootstrap_output_reservation_has_no_default_or_coercion() {
    let (ids, model, response, strict, text, answer) = inputs();
    let drafts =
        bootstrap_answer_workflow_drafts(ids, model, response, strict, text, answer).unwrap();
    let doc = drafts[0].document();
    let contract = InputContract::from_value(&doc["input_contract"], REVISION_LIMITS).unwrap();
    for value in [json!(1), json!(u32::MAX)] {
        contract
            .bind(&json!({"packet_bytes":"{}","max_output_tokens":value}))
            .unwrap();
    }
    for value in [
        json!(null),
        json!(0),
        json!(true),
        json!(1.5),
        json!("37"),
        json!(u64::from(u32::MAX) + 1),
    ] {
        assert!(
            contract
                .bind(&json!({"packet_bytes":"{}","max_output_tokens":value}))
                .is_err()
        );
    }
    assert!(contract.bind(&json!({"packet_bytes":"{}"})).is_err());
    assert!(
        contract
            .bind(&json!({"packet_bytes":"{}","max_output_tokens":37,"prefix_reference":"caller"}))
            .is_err()
    );
}

#[test]
fn bootstrap_identity_aliases_do_not_create_another_tool_or_hidden_dependency() {
    let (ids, model, response, strict, text, answer) = inputs();
    let mut bad = ids;
    bad.prompt = answer;
    assert!(bootstrap_answer_workflow_drafts(bad, model, response, strict, text, answer).is_err());
    bad = ids;
    bad.protocol = Uuid::nil();
    assert!(bootstrap_answer_workflow_drafts(bad, model, response, strict, text, answer).is_err());
    let mut wrong = strict;
    wrong.tool = Uuid::new_v4();
    assert!(bootstrap_answer_workflow_drafts(ids, model, response, wrong, text, answer).is_err());
    wrong = strict;
    wrong.python_code = model.python_code;
    assert!(bootstrap_answer_workflow_drafts(ids, model, response, wrong, text, answer).is_err());
    let mut same_tool = model;
    same_tool.tool = response.tool;
    assert!(
        bootstrap_answer_workflow_drafts(ids, same_tool, response, strict, text, answer).is_err()
    );
}
