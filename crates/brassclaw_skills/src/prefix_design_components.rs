//! Shared deterministic design selection. These constructors do not approve,
//! install or activate a model design. Exact selection/usage qualification stays
//! with the existing catalogue/bootstrap owners.
use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    global_bootstrap_components::UsageComponentIds,
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use uuid::Uuid;

pub const DESIGN_SOURCE: &str = include_str!("../../../scripts/prefix/prefix_design_selection.py");

#[path = "prefix_budgeted_components.rs"]
mod budgeted;
pub use budgeted::{BudgetedDesignIds, budgeted_design_selection_drafts};

#[derive(Clone, Copy)]
pub struct DesignSelectionIds {
    pub recipe: Uuid,
    pub library: Uuid,
    pub prepare: Uuid,
    pub select: Uuid,
}
impl DesignSelectionIds {
    pub fn allocate(mut allocate: impl FnMut(&str) -> Uuid) -> Self {
        Self {
            recipe: allocate("prefix-design-selection:recipe"),
            library: allocate("prefix-design-selection:library"),
            prepare: allocate("prefix-design-selection:prepare"),
            select: allocate("prefix-design-selection:select"),
        }
    }
}
fn string() -> Value {
    json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]})
}
fn strings() -> Value {
    json!({"type":"list","required":true,"checks":[],"items":{"type":"string","checks":[]}})
}
fn prepared() -> Value {
    json!({"type":"object","allow_extra_fields":false,"fields":{"bank_bytes":string(),"texts":strings(),"probes_json":string()}})
}
fn preparation_inputs() -> Value {
    json!({"design_bytes":string(),"sources_bytes":string(),"policy_ref":string(),"special_tokens":strings(),"prefix_budget":{"type":"integer","required":true,"checks":[{"kind":"min","value":0},{"kind":"max","value":9007199254740991i64}]}})
}
fn selection_inputs() -> Value {
    json!({"bank_bytes":string(),"digests":strings(),"observation_bytes":{"type":"string","required":true,"checks":[]},"expected_specification":string(),"expected_artifact_set":string()})
}
fn pure(
    id: Uuid,
    name: &str,
    body: String,
    inputs: Value,
    result: Value,
    dependencies: Vec<Uuid>,
    private: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    InputContract::from_value(&inputs, REVISION_LIMITS)?;
    ValueContract::result_from_value(&result, REVISION_LIMITS)?;
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":name,"description":"Pure deterministic whole-unit design preparation/selection. No Tool calls, design approval, envelope qualification or activation.","content":body,"includes":[],"dependency_registry":null,"input_contract":inputs,"result_contract":result,
        "preload":{"format":"python-preload/2","exports":{name:{"symbol":name,"parameters":["inputs"],"mapping":true}},"private_functions":private,"constants":[],"imports":["json"],"dependencies":dependencies,"default_export":name}},"dependencies":dependencies,"association":null}).to_string())
}

/// Six explicit steps: prepare complete candidates; bind/hash exact texts;
/// bind/encode reference-text probes; select the fullest fitting priority prefix.
/// The two existing Tool usages are reused, never replaced by a workflow Tool.
pub fn design_selection_drafts(
    ids: DesignSelectionIds,
    units: Uuid,
    renderer: Uuid,
    digest: UsageComponentIds,
    encoding: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut names = vec![
        ids.recipe,
        ids.library,
        ids.prepare,
        ids.select,
        units,
        renderer,
        digest.tool,
        digest.recipe,
        digest.skill,
        digest.tool_skill,
        digest.python_code,
        encoding.tool,
        encoding.recipe,
        encoding.skill,
        encoding.tool_skill,
        encoding.python_code,
    ];
    names.extend(digest.formatter);
    names.extend(encoding.formatter);
    if names.iter().any(Uuid::is_nil)
        || names
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != names.len()
    {
        return Err(RevisionError::Invalid(
            "design graph identities must be distinct and non-nil",
        ));
    }
    let library = pure(
        ids.library,
        "prefix_design_stage",
        DESIGN_SOURCE.into(),
        json!({"phase":string(),"data_bytes":string()}),
        json!({"type":"string"}),
        vec![units, renderer],
        json!({"design_json":["value"],"design_decode":["text"],"design_fields":["value","fields"],"design_integer":["value","minimum"],"design_name":["value"],"design_hash":["value"],"design_closure":["roots","groups"],"design_groups":["band","units"],"design_source_units":["source_bytes"],"design_selector_units":["selector","units"],"design_resolve_rules":["design","units"],"validate_prefix_design_coverage":["inputs"],"prepare_prefix_design_candidates":["inputs"],"select_prefix_design_candidate":["inputs"]}),
    )?;
    let prepare = pure(ids.prepare, "prepare_design_candidates", "def prepare_design_candidates(inputs):\n    import json\n    return json.loads(prefix_design_stage({'phase':'prepare','data_bytes':json.dumps(inputs, ensure_ascii=False, sort_keys=True, separators=(',', ':'))}))\n".into(), preparation_inputs(), prepared(), vec![ids.library], json!({}))?;
    let select = pure(ids.select, "select_measured_design", "def select_measured_design(inputs):\n    import json\n    return prefix_design_stage({'phase':'select','data_bytes':json.dumps(inputs, ensure_ascii=False, sort_keys=True, separators=(',', ':'))})\n".into(), selection_inputs(), json!({"type":"string"}), vec![ids.library], json!({}))?;
    let component = |number, id, goal| json!({"stepnumber":number,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[id]});
    let binding = |number, usage: UsageComponentIds, name| json!({"stepnumber":number,"knowledge":"rust","goal":"Prepare retained native usage","content":"","type":"component","include":[usage.tool_skill],"tool_bindings":[{"tool_id":usage.tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]});
    let task = |name: &str| json!({"kind":"task_input","reference":format!("{{{{vars.{name}}}}}")});
    let result = |step, path| json!({"kind":"result","step_id":format!("0:{step}"),"path":path});
    let mut task_inputs = preparation_inputs();
    task_inputs["specification"] = string();
    task_inputs["artifact_set"] = string();
    let mut prepare_layout = serde_json::Map::new();
    for name in [
        "design_bytes",
        "sources_bytes",
        "prefix_budget",
        "policy_ref",
        "special_tokens",
    ] {
        prepare_layout.insert(name.into(), task(name));
    }
    let layouts = json!({
        "0:1":prepare_layout,
        "0:3":{"texts":result(1,vec!["texts"])},
        "0:5":{"specification":task("specification"),"artifact_set":task("artifact_set"),"probes_json":result(1,vec!["probes_json"])},
        "0:6":{"bank_bytes":result(1,vec!["bank_bytes"]),"digests":result(3,vec!["digests"]),"observation_bytes":result(5,vec![]),"expected_specification":task("specification"),"expected_artifact_set":task("artifact_set")}});
    let pattern = "select prefix design=% sources=% budget=% policy=% tokens=% model=% artifacts=%";
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
    let variables: Vec<_> = [
        "design_bytes",
        "sources_bytes",
        "prefix_budget",
        "policy_ref",
        "special_tokens",
        "specification",
        "artifact_set",
    ]
    .iter()
    .map(|name| json!({"name":name,"pattern":null,"description":"Pinned typed model-design input"}))
    .collect();
    let dependencies = vec![
        ids.prepare,
        ids.select,
        digest.skill,
        digest.tool_skill,
        digest.python_code,
        encoding.skill,
        encoding.tool_skill,
        encoding.python_code,
    ];
    let recipe = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.recipe,"class_code":21,
        "document":{"name":"select-prefix-design-by-measurement","description":"Model-free deterministic complete-unit design selection using existing native hashing and exact reference-text encoding. Does not measure semantic prefix partitions, approve designs or activate serving.",
        "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":variables}],
        "step_descriptions":[{"desc_idx":0,"label":"Prepare, measure and select complete design candidates","yaml_source":"","steps":[component(1,ids.prepare,"Prepare dependency-complete candidates"),binding(2,digest,"json"),component(3,digest.python_code,"Hash original inputs and complete candidates"),binding(4,encoding,"model_encoding"),component(5,encoding.python_code,"Measure exact reference text without truncation"),component(6,ids.select,"Select a fitting priority prefix or report capacity blocked")]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":task_inputs,"steps":layouts}},
        "invocations":{"selected":{"0:1":"prepare_design_candidates","0:3":"digest_utf8_texts","0:5":"encode_model_probes","0:6":"select_measured_design"}}},
        "dependencies":dependencies,"association":null}).to_string())?;
    Ok(vec![library, prepare, select, recipe])
}

/// Additional pure stages for deriving an independent profile floor from the
/// original capture. No identity here grants source authority or activation.
#[derive(Clone, Copy)]
pub struct CapturedDesignSelectionIds {
    pub compiler_library: Uuid,
    pub source_prepare: Uuid,
    pub design_inputs: Uuid,
    pub coverage: Uuid,
    pub select: Uuid,
}

/// Extend the same candidate Recipe with a captured-source variant. The
/// original six-step variant and shared selection/native usage components are
/// reused. Callers must supply the authenticated capture and canonical target
/// policy through their existing admission owner before qualifying this graph.
pub fn captured_design_selection_drafts(
    ids: DesignSelectionIds,
    captured: CapturedDesignSelectionIds,
    units: Uuid,
    renderer: Uuid,
    digest: UsageComponentIds,
    encoding: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut drafts = design_selection_drafts(ids, units, renderer, digest, encoding)?;
    let extra = [
        captured.compiler_library,
        captured.source_prepare,
        captured.design_inputs,
        captured.coverage,
        captured.select,
    ];
    let existing = [
        ids.recipe,
        ids.library,
        ids.prepare,
        ids.select,
        units,
        renderer,
        digest.tool,
        digest.recipe,
        digest.skill,
        digest.tool_skill,
        digest.python_code,
        encoding.tool,
        encoding.recipe,
        encoding.skill,
        encoding.tool_skill,
        encoding.python_code,
    ];
    if extra.iter().any(|id| {
        id.is_nil()
            || existing.contains(id)
            || digest.formatter == Some(*id)
            || encoding.formatter == Some(*id)
    }) || extra
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != extra.len()
    {
        return Err(RevisionError::Invalid(
            "captured selection identities must be distinct and non-nil",
        ));
    }
    let recipe_index = drafts
        .iter()
        .position(|draft| draft.uuid() == ids.recipe)
        .ok_or(RevisionError::Invalid("candidate selection Recipe missing"))?;
    let recipe = drafts.remove(recipe_index);
    let mut document = recipe.document().clone();
    let task = |name: &str| json!({"kind":"task_input","reference":format!("{{{{vars.{name}}}}}")});
    let result =
        |step, field| json!({"kind":"result","step_id":format!("1:{step}"),"path":[field]});
    let component = |number, id, goal| json!({"stepnumber":number,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[id]});
    let mut steps = vec![
        component(
            1,
            captured.source_prepare,
            "Prepare complete original profile sources",
        ),
        document["step_descriptions"][0]["steps"][1].clone(),
        component(
            3,
            digest.python_code,
            "Hash original sources, units and compiler procedures",
        ),
        component(
            4,
            captured.design_inputs,
            "Derive profile-owned coverage floor and included originals",
        ),
        component(
            5,
            captured.coverage,
            "Check every proposed size band against independent coverage",
        ),
    ];
    steps[1]["stepnumber"] = json!(2);
    let offset = steps.len();
    let mut tail = document["step_descriptions"][0]["steps"]
        .as_array()
        .ok_or(RevisionError::Invalid("candidate steps missing"))?
        .clone();
    for (index, step) in tail.iter_mut().enumerate() {
        step["stepnumber"] = json!(offset + index + 1);
    }
    steps.extend(tail);
    let mut layouts = json!({
        "1:1":{"snapshot_bytes":task("snapshot_bytes")},
        "1:3":{"texts":result(1,"texts")},
        "1:4":{"snapshot_bytes":task("snapshot_bytes"),"digests":result(3,"digests")},
        "1:5":{"design_bytes":task("design_bytes"),"floor_bytes":result(4,"floor_bytes"),"sources_bytes":result(4,"sources_bytes")}
    });
    // Rebase the existing layout, not a separately maintained selection workflow.
    for (key, value) in document["input_layouts"]["selected"]["steps"]
        .as_object()
        .ok_or(RevisionError::Invalid("candidate layouts missing"))?
    {
        let number = key
            .strip_prefix("0:")
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or(RevisionError::Invalid("candidate step reference invalid"))?;
        let mut value = value.clone();
        for binding in value
            .as_object_mut()
            .ok_or(RevisionError::Invalid("candidate binding layout invalid"))?
            .values_mut()
        {
            if binding["kind"] == "result" {
                let source = binding["step_id"]
                    .as_str()
                    .and_then(|id| id.strip_prefix("0:"))
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or(RevisionError::Invalid("candidate result reference invalid"))?;
                binding["step_id"] = json!(format!("1:{}", source + offset));
            }
        }
        layouts[format!("1:{}", number + offset)] = value;
    }
    let prepare_key = format!("1:{}", offset + 1);
    layouts[&prepare_key]["design_bytes"] = result(5, "design_bytes");
    layouts[&prepare_key]["sources_bytes"] = result(5, "sources_bytes");
    let selection_key = format!("1:{}", steps.len());
    layouts[&selection_key]["comparison_bytes"] = result(5, "comparison_bytes");
    layouts[&selection_key]["snapshot_bytes"] = result(4, "snapshot_bytes");
    steps
        .last_mut()
        .ok_or(RevisionError::Invalid("candidate selection step missing"))?["include"] =
        json!([captured.select]);
    let mut invocations = json!({"1:1":"prepare_prefix_sources","1:3":"digest_utf8_texts", "1:4":"prepare_prefix_design_inputs","1:5":"compare_prefix_coverage"});
    for (key, value) in document["invocations"]["selected"]
        .as_object()
        .ok_or(RevisionError::Invalid("candidate invocations missing"))?
    {
        let number = key
            .strip_prefix("0:")
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or(RevisionError::Invalid(
                "candidate invocation reference invalid",
            ))?;
        invocations[format!("1:{}", number + offset)] = value.clone();
    }
    invocations[&selection_key] = json!("select_captured_design");
    let mut inputs = document["input_layouts"]["selected"]["task_inputs"].clone();
    inputs
        .as_object_mut()
        .ok_or(RevisionError::Invalid("candidate task inputs missing"))?
        .remove("sources_bytes");
    inputs["snapshot_bytes"] = string();
    let variables = [
        "design_bytes",
        "snapshot_bytes",
        "prefix_budget",
        "policy_ref",
        "special_tokens",
        "specification",
        "artifact_set",
    ];
    let examples: Vec<_> = ["", "please ", "now ", "please now ", "execute ", "run ", "please execute ", "please run ", "perform ", "please perform "]
        .iter().map(|prefix| format!("{prefix}select captured prefix design=% snapshot=% budget=% policy=% tokens=% model=% artifacts=%")).collect();
    document["variants"].as_array_mut().ok_or(RevisionError::Invalid("candidate variants missing"))?.push(json!({
        "variant_key":"captured","step_link":"1:1-1:E","intent_examples":examples,
        "variable_patterns":variables.iter().map(|name|json!({"name":name,"pattern":null,"description":"Pinned captured-profile model-design input"})).collect::<Vec<_>>()
    }));
    document["step_descriptions"].as_array_mut().ok_or(RevisionError::Invalid("candidate step descriptions missing"))?.push(json!({
        "desc_idx":1,"label":"Derive profile coverage then measure complete candidates","yaml_source":"","steps":steps
    }));
    document["input_layouts"]["captured"] =
        json!({"format":"recipe-input-layout/1","task_inputs":inputs,"steps":layouts});
    document["invocations"]["captured"] = invocations;
    let mut dependencies = recipe.dependencies().clone();
    dependencies.extend([
        captured.source_prepare,
        captured.design_inputs,
        captured.coverage,
        captured.select,
    ]);
    drafts.push(ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":ids.recipe,"class_code":21,
        "document":document,"dependencies":dependencies,"association":null})
        .to_string(),
    )?);
    drafts.push(crate::prefix_compiler_components::compiler_library_draft(
        captured.compiler_library,
        units,
        renderer,
    )?);
    drafts.push(crate::prefix_compiler_components::compiler_stage_draft(
        captured.source_prepare,
        captured.compiler_library,
        "prepare",
    )?);
    drafts.push(crate::prefix_compiler_components::compiler_stage_draft(
        captured.design_inputs,
        captured.compiler_library,
        "design-inputs",
    )?);
    drafts.push(pure(captured.coverage, "compare_prefix_coverage",
        "def compare_prefix_coverage(inputs):\n    import json\n    comparison = prefix_design_stage({'phase':'coverage','data_bytes':json.dumps(inputs, ensure_ascii=False, sort_keys=True, separators=(',', ':'))})\n    return {'comparison_bytes':comparison,'design_bytes':inputs['design_bytes'],'sources_bytes':inputs['sources_bytes']}\n".into(),
        json!({"design_bytes":string(),"floor_bytes":string(),"sources_bytes":string()}),
        json!({"type":"object","allow_extra_fields":false,"fields":{"comparison_bytes":string(),"design_bytes":string(),"sources_bytes":string()}}),
        vec![ids.library], json!({}))?);
    let mut selection = selection_inputs();
    selection["comparison_bytes"] = string();
    selection["snapshot_bytes"] = string();
    drafts.push(pure(captured.select, "select_captured_design",
        "def select_captured_design(inputs):\n    import json\n    selected = select_measured_design({key:inputs[key] for key in ['bank_bytes','digests','observation_bytes','expected_specification','expected_artifact_set']})\n    return json.dumps({'format':'brassclaw-captured-prefix-design-selection/1','selection_bytes':selected,'comparison_bytes':inputs['comparison_bytes'],'snapshot_bytes':inputs['snapshot_bytes'],'qualification':'captured-reference-candidate-only; no accepted design, envelope fit or activation'}, ensure_ascii=False, sort_keys=True, separators=(',', ':'))\n".into(),
        selection, json!({"type":"string","checks":[{"kind":"min_length","value":1}]}), vec![ids.select], json!({}))?);
    Ok(drafts)
}
