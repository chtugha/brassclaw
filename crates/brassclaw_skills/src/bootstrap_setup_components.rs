//! One configuration-only pure workflow. Draft bytes are not qualification or
//! independent design acceptance; only its closed native owner may select it.
use crate::{
    component_revision::REVISION_LIMITS,
    component_revision::{ComponentRevisionDraft, RevisionError},
    value_contract::{InputContract, ValueContract},
};
use serde_json::json;
use uuid::Uuid;
pub const SOURCE: &str = include_str!("../../../scripts/prefix/bootstrap_setup.py");
pub fn drafts(recipe: Uuid, code: Uuid) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    if recipe.is_nil() || code.is_nil() || recipe == code {
        return Err(RevisionError::Invalid(
            "distinct bootstrap setup identities required",
        ));
    }
    let text = json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]});
    let inputs = json!({"capture_bytes":text});
    let result = json!({"type":"string"});
    InputContract::from_value(&inputs, REVISION_LIMITS)?;
    ValueContract::result_from_value(&result, REVISION_LIMITS)?;
    let code_draft=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":code,"class_code":22,
        "document":{"name":"bootstrap_setup","description":"Present exact original configuration/setup, preserve manual null/zero, and report endpoint/design gaps without models or effects.","content":SOURCE,"includes":[],"dependency_registry":null,
        "input_contract":inputs,"result_contract":result,"preload":{"format":"python-preload/2","exports":{"bootstrap_setup":{"symbol":"bootstrap_setup","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"bootstrap_setup"}},"dependencies":[],"association":null}).to_string())?;
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
    .into_iter()
    .map(|lead| format!("{lead}prepare original provider setup capture=%"))
    .collect();
    let recipe_draft=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":recipe,"class_code":21,
        "document":{"name":"bootstrap-provider-configuration-values","description":"Technical configuration-only prefill; no endpoint proof, model call, design acceptance, prefix generation or activation.",
        "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":[{"name":"capture_bytes","pattern":null,"description":"Exact native original capture"}]}],
        "step_descriptions":[{"desc_idx":0,"label":"Retain exact technical setup and explicit gaps","yaml_source":"","steps":[{"stepnumber":1,"knowledge":"orchestrator","goal":"Present native original setup and scoped missing facts","content":"","type":"component","include":[code]}]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":{"0:1":{"capture_bytes":{"kind":"task_input","reference":"{{vars.capture_bytes}}"}}}}},
        "invocations":{"selected":{"0:1":"bootstrap_setup"}}},"dependencies":[code],"association":null}).to_string())?;
    Ok(vec![code_draft, recipe_draft])
}
