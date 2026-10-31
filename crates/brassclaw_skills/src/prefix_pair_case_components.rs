//! Reusable Recipe over existing digest/original-template encoding usages.
//! These constructors are unapproved candidates, not installed workflows or
//! target-design/provider/cache acceptance. Never replace the original template.
use crate::{
    component_revision::{ComponentRevisionDraft, REVISION_LIMITS, RevisionError},
    global_bootstrap_components::UsageComponentIds,
    value_contract::{InputContract, ValueContract},
};
use serde_json::{Value, json};
use uuid::Uuid;

pub const CASE_SOURCE: &str = include_str!("../../../scripts/prefix/prefix_pair_cases.py");

#[derive(Clone, Copy)]
pub struct PairCaseIds {
    pub recipe: Uuid,
    pub library: Uuid,
    pub prepare: Uuid,
    pub compare: Uuid,
}
impl PairCaseIds {
    pub fn allocate(mut allocate: impl FnMut(&str) -> Uuid) -> Self {
        Self {
            recipe: allocate("prefix-pair-cases:recipe"),
            library: allocate("prefix-pair-cases:library"),
            prepare: allocate("prefix-pair-cases:prepare"),
            compare: allocate("prefix-pair-cases:compare"),
        }
    }
}
fn string() -> Value {
    json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]})
}
fn strings() -> Value {
    json!({"type":"list","required":true,"checks":[],"items":{"type":"string","checks":[]}})
}
fn pure(
    id: Uuid,
    name: &str,
    source: String,
    inputs: Value,
    result: Value,
    dependencies: Vec<Uuid>,
    private: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    InputContract::from_value(&inputs, REVISION_LIMITS)?;
    ValueContract::result_from_value(&result, REVISION_LIMITS)?;
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":name,"description":"Pure complete case preparation/comparison; no Tool calls, rendering, source/design approval or activation.",
            "content":source,"includes":[],"dependency_registry":null,"input_contract":inputs,"result_contract":result,
            "preload":{"format":"python-preload/2","exports":{name:{"symbol":name,"parameters":["inputs"],"mapping":true}},
                "private_functions":private,"constants":[],"imports":["json"],"dependencies":dependencies,"default_export":name}},
        "dependencies":dependencies,"association":null}).to_string())
}

/// Prepare all cases -> hash exact expected/context/prefix strings -> original
/// template encoding -> compare complete bytes/tokens and measured prefix IDs.
/// Two independent Tool calls have separate binding/execution steps. All
/// original usages and actual native Tool definitions are supplied by the owner.
pub fn pair_case_drafts(
    ids: PairCaseIds,
    digest: UsageComponentIds,
    encoding: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut names = vec![
        ids.recipe,
        ids.library,
        ids.prepare,
        ids.compare,
        digest.recipe,
        digest.skill,
        digest.python_code,
        digest.tool_skill,
        digest.tool,
        encoding.recipe,
        encoding.skill,
        encoding.python_code,
        encoding.tool_skill,
        encoding.tool,
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
        || encoding.formatter.is_some()
    {
        return Err(RevisionError::Invalid(
            "distinct original-template case workflow identities required",
        ));
    }
    let library = pure(
        ids.library,
        "pair_case_stage",
        CASE_SOURCE.into(),
        json!({"phase":string(),"data_bytes":string()}),
        json!({"type":"string"}),
        vec![],
        json!({"pair_case_json":["value"],"pair_case_decode":["text"],"pair_case_fields":["value","fields"],
            "pair_case_hash":["value"],"pair_case_uint":["value"],"pair_case_packet":["text"],
            "pair_case_prepare":["inputs"],"pair_case_compare":["inputs"]}),
    )?;
    let prepare=pure(ids.prepare,"prepare_pair_case_probes",
        "def prepare_pair_case_probes(inputs):\n    import json\n    return json.loads(pair_case_stage({'phase':'prepare','data_bytes':json.dumps(inputs, ensure_ascii=False, sort_keys=True, separators=(',', ':'))}))\n".into(),
        json!({"case_packet_bytes":string()}),
        json!({"type":"object","allow_extra_fields":false,"fields":{"bank_bytes":string(),"texts":strings(),"probes_json":string(),"specification":string(),"artifact_set":string()}}),
        vec![ids.library],json!({}))?;
    let compare=pure(ids.compare,"compare_pair_case_measurement",
        "def compare_pair_case_measurement(inputs):\n    import json\n    return pair_case_stage({'phase':'compare','data_bytes':json.dumps(inputs, ensure_ascii=False, sort_keys=True, separators=(',', ':'))})\n".into(),
        json!({"bank_bytes":string(),"digests":strings(),"observation_bytes":{"type":"string","required":true,"checks":[]}}),json!({"type":"string"}),vec![ids.library],json!({}))?;
    let component = |number, id, goal| json!({"stepnumber":number,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[id]});
    let binding = |number, usage: UsageComponentIds, name| json!({"stepnumber":number,"knowledge":"rust","goal":"Prepare exact retained primitive binding","content":"","type":"component","include":[usage.tool_skill],"tool_bindings":[{"tool_id":usage.tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]});
    let result =
        |number, path| json!({"kind":"result","step_id":format!("0:{number}"),"path":path});
    let dependencies = vec![
        ids.prepare,
        ids.compare,
        digest.skill,
        digest.tool_skill,
        digest.python_code,
        encoding.skill,
        encoding.tool_skill,
        encoding.python_code,
    ];
    let pattern = "compare prefix pair cases packet=%";
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
    let recipe=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.recipe,"class_code":21,
        "document":{"name":"compare-prefix-pair-default-template-cases",
            "description":"Compare complete literal worked cases with retained original-template rendering/encoding. Does not render a serving prompt, accept a design or prove provider wire, capacity, cache or answer quality.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,
                "variable_patterns":[{"name":"case_packet_bytes","pattern":null,"description":"Exact canonical unqualified complete case projection"}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Compare all retained default-template cases","yaml_source":"","steps":[
                component(1,ids.prepare,"Prepare complete original-context and expected-text probe pairs"),
                binding(2,digest,"json"),component(3,digest.python_code,"Hash exact packet/expected/context/prefix texts"),
                binding(4,encoding,"model_encoding"),component(5,encoding.python_code,"Encode complete cases with original model template"),
                component(6,ids.compare,"Compare full bytes/token IDs and measured fixed-prefix partitions")]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"case_packet_bytes":string()},"steps":{
                "0:1":{"case_packet_bytes":{"kind":"task_input","reference":"{{vars.case_packet_bytes}}"}},
                "0:3":{"texts":result(1,vec!["texts"])},
                "0:5":{"specification":result(1,vec!["specification"]),"artifact_set":result(1,vec!["artifact_set"]),"probes_json":result(1,vec!["probes_json"])},
                "0:6":{"bank_bytes":result(1,vec!["bank_bytes"]),"digests":result(3,vec!["digests"]),"observation_bytes":result(5,Vec::<&str>::new())}}}},
            "invocations":{"selected":{"0:1":"prepare_pair_case_probes","0:3":"digest_utf8_texts","0:5":"encode_model_probes","0:6":"compare_pair_case_measurement"}}},
        "dependencies":dependencies,"association":null}).to_string())?;
    Ok(vec![library, prepare, compare, recipe])
}
