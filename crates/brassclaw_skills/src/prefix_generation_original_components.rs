//! Archived-original projection composed with the existing generation read.
//! Definitions do not authenticate custody, publish health or activate a prefix.
use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    global_bootstrap_components::UsageComponentIds,
    prefix_generation_components::{GenerationUsage, generation_usage_drafts},
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use uuid::Uuid;

pub const ORIGINAL_SOURCE: &str =
    include_str!("../../../scripts/prefix/prefix_generation_original.py");

fn string() -> Value {
    json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]})
}

/// Reuse the original canonical read variant and add one indexed-source variant.
/// Only its real storage result supplies `detail`; it is not a task input.
pub fn generation_original_drafts(
    read: UsageComponentIds,
    projection: Uuid,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut drafts = generation_usage_drafts(read, GenerationUsage::Read)?;
    if projection.is_nil()
        || [
            read.recipe,
            read.tool,
            read.tool_skill,
            read.skill,
            read.python_code,
        ]
        .contains(&projection)
    {
        return Err(RevisionError::Invalid(
            "distinct original projection identity required",
        ));
    }
    let usage = drafts
        .iter()
        .find(|draft| draft.uuid() == read.python_code)
        .ok_or(RevisionError::Invalid("generation read usage missing"))?;
    // Derive this contract from the actual producer, not another DTO/schema copy.
    let mut detail = usage.document()["result_contract"].clone();
    detail["required"] = json!(true);
    // Result schemas may omit checks to mean no extra constraints. Top-level
    // input schemas require that same meaning explicitly; retain any producer
    // checks rather than replacing them with a weaker declaration.
    if detail.get("checks").is_none() {
        detail["checks"] = json!([]);
    }
    let mut task_inputs = usage.document()["input_contract"].clone();
    task_inputs["reference"] = string();
    let mut inputs = task_inputs.clone();
    inputs["detail"] = detail;
    let mut text = string();
    text["checks"] = json!([]);
    let result = json!({"type":"object","allow_extra_fields":false,"fields":{
        "profile":string(),"generation":string(),"reference":string(),"original_sha256":string(),
        "original_text":text,"disposition":string(),"reason":string(),"generation_qualification":string(),"qualification":string()}});
    let input_contract = InputContract::from_value(&inputs, REVISION_LIMITS)?;
    ValueContract::result_from_value(&usage.document()["result_contract"], REVISION_LIMITS)?
        .require_compatible_input(&input_contract, "detail")?;
    ValueContract::result_from_value(&result, REVISION_LIMITS)?;
    let pure = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":projection,"class_code":22,
        "document":{"name":"project-prefix-generation-source","description":"Project one complete UTF-8 original by exact source index from the real immutable generation-reader result. Resolve modern and historical indexed originals by raw SHA without following old input-package artifact names. Preserve disposition and generation qualification. Reject legacy/no-index, different profile/generation, missing/duplicate references and malformed artifacts; no current-file fallback, hash computation, fetching, availability or knowledge approval.",
        "content":ORIGINAL_SOURCE,"includes":[],"dependency_registry":null,"input_contract":inputs,"result_contract":result,
        "preload":{"format":"python-preload/2","exports":{"project_prefix_generation_source":{"symbol":"project_prefix_generation_source","parameters":["inputs"],"mapping":true}},
        "private_functions":{"original_nonempty":["value"],"original_digest":["value"],"original_artifact":["package","name"]},
        "constants":[],"imports":["json","base64"],"dependencies":[],"default_export":"project_prefix_generation_source"}},
        "dependencies":[],"association":null}).to_string())?;
    let index = drafts
        .iter()
        .position(|draft| draft.uuid() == read.recipe)
        .ok_or(RevisionError::Invalid("generation read Recipe missing"))?;
    let recipe = drafts.remove(index);
    let mut document = recipe.document().clone();
    let mut steps = document["step_descriptions"][0]["steps"]
        .as_array()
        .ok_or(RevisionError::Invalid("generation read steps missing"))?
        .clone();
    let projection_step = steps.len() + 1;
    steps.push(json!({"stepnumber":projection_step,"knowledge":"orchestrator","goal":"Project the exact indexed whole original without rereading current sources","content":"","type":"component","include":[projection]}));
    let layouts = document["input_layouts"]["selected"]["steps"].clone();
    let invocations = document["invocations"]["selected"].clone();
    let mut rebased_layouts = json!({});
    let mut rebased_invocations = json!({});
    for (key, value) in layouts
        .as_object()
        .ok_or(RevisionError::Invalid("generation read layouts missing"))?
    {
        let suffix = key.strip_prefix("0:").ok_or(RevisionError::Invalid(
            "generation read layout reference invalid",
        ))?;
        rebased_layouts[format!("1:{suffix}")] = value.clone();
    }
    let mut read_result = None;
    for (key, value) in invocations.as_object().ok_or(RevisionError::Invalid(
        "generation read invocations missing",
    ))? {
        let suffix = key.strip_prefix("0:").ok_or(RevisionError::Invalid(
            "generation read invocation reference invalid",
        ))?;
        rebased_invocations[format!("1:{suffix}")] = value.clone();
        if value == "read_prefix_generation" {
            read_result = Some(format!("1:{suffix}"));
        }
    }
    let read_result =
        read_result.ok_or(RevisionError::Invalid("generation read export missing"))?;
    let projection_key = format!("1:{projection_step}");
    rebased_layouts[&projection_key] = json!({
        "profile":{"kind":"task_input","reference":"{{vars.profile}}"},
        "generation":{"kind":"task_input","reference":"{{vars.generation}}"},
        "reference":{"kind":"task_input","reference":"{{vars.reference}}"},
        "detail":{"kind":"result","step_id":read_result,"path":[]}});
    rebased_invocations[&projection_key] = json!("project_prefix_generation_source");
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
    .map(|lead| format!("{lead}read prefix source profile=% generation=% reference=%"))
    .collect();
    let variables: Vec<_> = ["profile", "generation", "reference"]
        .iter()
        .map(|name| json!({"name":name,"pattern":null,"description":"Exact immutable original-source input"}))
        .collect();
    document["variants"]
        .as_array_mut()
        .ok_or(RevisionError::Invalid("generation read variants missing"))?
        .push(json!({
        "variant_key":"source","step_link":"1:1-1:E","intent_examples":examples,
        "variable_patterns":variables }));
    document["step_descriptions"].as_array_mut().ok_or(RevisionError::Invalid("generation read descriptions missing"))?.push(json!({"desc_idx":1,"label":"Read one exact archived original","yaml_source":"","steps":steps}));
    document["input_layouts"]["source"] =
        json!({"format":"recipe-input-layout/1","task_inputs":task_inputs,"steps":rebased_layouts});
    document["invocations"]["source"] = rebased_invocations;
    let mut dependencies = recipe.dependencies().clone();
    dependencies.insert(projection);
    drafts.push(ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":read.recipe,"class_code":21,
        "document":document,"dependencies":dependencies,"association":null})
        .to_string(),
    )?);
    drafts.push(pure);
    Ok(drafts)
}
