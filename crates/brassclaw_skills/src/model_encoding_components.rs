//! One reusable offline encoding usage. These are unapproved drafts; native
//! registration, exact worker retention and behavioral qualification are separate.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};
use serde_json::{Value, json};

/// The two shipped usages keep distinct identities while sharing one primitive.
pub fn usage_ids(
    tool: uuid::Uuid,
    effective: bool,
    mut allocate: impl FnMut(&str) -> uuid::Uuid,
) -> UsageComponentIds {
    let name = if effective { "template" } else { "original" };
    UsageComponentIds {
        recipe: allocate(&format!("prefix-encoding:{name}:recipe")),
        python_code: allocate(&format!("prefix-encoding:{name}:code")),
        skill: allocate(&format!("prefix-encoding:{name}:skill")),
        tool_skill: allocate(&format!("prefix-encoding:{name}:binding")),
        tool,
        formatter: None,
    }
}

fn inputs() -> Value {
    let string =
        json!({"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]});
    json!({"specification":string,"artifact_set":string,"probes_json":string})
}
pub fn model_encoding_tool_draft(
    id: uuid::Uuid,
    identity: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let mut contract = inputs();
    contract["template_utf8"] = json!({"type":"string","required":false,"checks":[]});
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":0,
        "document":{"capability_id":"builtin.model_encoding","callable":"host.model_encoding","input_contract":contract,
            "implementation":{"format":"native-first-party/1","identity":identity}},"dependencies":[],"association":null}).to_string())
}
pub fn model_encoding_usage_drafts(
    ids: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    usage_drafts(ids, false)
}
/// Same primitive, with an exact effective template supplied by the consuming
/// selected adapter. This measures a candidate; it never accepts its design.
pub fn model_template_encoding_usage_drafts(
    ids: UsageComponentIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    usage_drafts(ids, true)
}
fn usage_drafts(
    ids: UsageComponentIds,
    with_template: bool,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let unique = std::collections::BTreeSet::from([
        ids.recipe,
        ids.python_code,
        ids.tool_skill,
        ids.tool,
        ids.skill,
    ]);
    if ids.formatter.is_some() || unique.len() != 5 || unique.contains(&uuid::Uuid::nil()) {
        return Err(RevisionError::Invalid(
            "distinct model encoding usage identities required",
        ));
    }
    let mut inputs = inputs();
    if with_template {
        inputs["template_utf8"] = json!({"type":"string","required":true,"checks":[]});
    }
    let mut arguments = json!({"specification":"specification","artifact_set":"artifact_set","probes_json":"probes_json"});
    let mut bindings = json!({"specification":{"kind":"task_input","reference":"{{vars.specification}}"},"artifact_set":{"kind":"task_input","reference":"{{vars.artifact_set}}"},"probes_json":{"kind":"task_input","reference":"{{vars.probes_json}}"}});
    let mut variables = vec![
        json!({"name":"specification","pattern":null,"description":"Exact model specification hash"}),
        json!({"name":"artifact_set","pattern":null,"description":"Exact immutable model artifact set hash"}),
        json!({"name":"probes_json","pattern":null,"description":"Complete ordered probe array JSON"}),
    ];
    if with_template {
        arguments["template_utf8"] = json!("template_utf8");
        bindings["template_utf8"] =
            json!({"kind":"task_input","reference":"{{vars.template_utf8}}"});
        variables.push(json!({"name":"template_utf8","pattern":null,"description":"Exact literal effective Jinja template; measurement is not approval"}));
    }
    let name = if with_template {
        "encode-model-template-probes"
    } else {
        "encode-model-probes"
    };
    let pattern = if with_template {
        "encode model specification=% artifacts=% probes=% template=%"
    } else {
        "encode model specification=% artifacts=% probes=%"
    };
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
    let template_contract = if with_template {
        "The additional required non-null template_utf8 string is the exact effective template emitted by the consuming pinned adapter. It is Jinja data for this contained rendering operation, never Python source. Preserve original model template and effective template identities separately. template_origin=supplied-template and its report hash are observations, not acceptance of code or design. No fallback to the original template occurs if a supplied template fails."
    } else {
        "This usage fixes the model-artifact template by omitting template_utf8. A serving injection adapter can have a different effective template: use the separate template-encoding usage to measure it; do not infer server compatibility from this original-template observation."
    };
    let variable_names = if with_template {
        "specification:string, artifact_set:string, probes_json:string, template_utf8:string"
    } else {
        "specification:string, artifact_set:string, probes_json:string"
    };
    let result = json!({"type":"string"});
    let export = if with_template {
        "encode_model_template_probes"
    } else {
        "encode_model_probes"
    };
    let preload = json!({"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},
        "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":export});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let association=json!({"format":"skill-association/1","skill_uuid":ids.skill,"python_code_uuid":ids.python_code,
        "tool_skill_uuid":ids.tool_skill,"tool_uuid":ids.tool,"callable":"host.model_encoding","inputs":inputs,
        "arguments":arguments,"code_arguments":{},"result":result,"failure":failure}).to_string();
    let code = if with_template {
        "def encode_model_template_probes(inputs):\n    return host.model_encoding(specification=inputs['specification'], artifact_set=inputs['artifact_set'], probes_json=inputs['probes_json'], template_utf8=inputs['template_utf8'])\n"
    } else {
        "def encode_model_probes(inputs):\n    return host.model_encoding(specification=inputs['specification'], artifact_set=inputs['artifact_set'], probes_json=inputs['probes_json'])\n"
    };
    let prose = format!(
        "Purpose\nRender and encode exact supplied probes with one retained offline model-artifact set. This measures bytes/token IDs; it never chooses knowledge, redesigns a prefix or accepts cache/model quality.\n\nComponents and interface\nTool {}; ToolSkill {}; associated PythonCode {}. Preload {export}(inputs); invoke it once on demand. The function calls host.model_encoding exactly once. No helpers, constants, mutable defaults, imports or dependencies. Loading has no effects. Canonical execution Recipe {}.\n\nInputs\nRequired non-null string inputs: specification and artifact_set are exact lower-case SHA-256 identifiers; probes_json is one nonempty JSON array of at most 64 probes. A text probe has kind=text and text. A chat probe has kind=chat and context_json: the complete ordered JSON render context, including messages, tools and generation/thinking flags actually used by the consuming provider adapter. Preserve nested key order. Context/data never becomes Python source. Do not substitute a model name, latest revision, file path, remote code or arbitrary worker. {template_contract}\n\nResults\nReturn exact brassclaw-model-encoding-observation/1 JSON bytes: immutable specification/artifact identity, acknowledged preparation revision, retained worker checksum and report containing tokenizer/config/effective-template hashes and separately preserved original model-template identity, rendered-byte and token-ID hashes/counts per probe, full-array common-prefix length/hash and disabled-padding/truncation facts. Token-ID hashes cover consecutive little-endian u32 IDs without a header. Empty common prefix is a valid observation. Full arrays/prompts are not returned. qualification=offline-measurement-only: no acceptance, activation, runtime compatibility, cache residency/reuse or recurrent-state reuse claim follows from it.\n\nWorkflow\nA separate Recipe selects accepted model design and exact model/serving identity, freezes sources, renders candidate content and constructs representative request probes. This usage observes them. Subsequent pure PythonCode compares observed counts/prefix IDs and technical context/output headroom, then the workflow chooses whether further cache/behavioral acceptance is needed. Generate must retain the accepted design; this primitive cannot redesign it. Offline tokenizer equivalence must be qualified for each supported target; unsupported templates/configurations fail closed. AWQ INT4 weights, attention KV dtype and linear/recurrent-state representation are separate properties.\n\nAuthority and failure\nPin actual native implementation, model artifact binding and contained worker. Current instance Tool policy, task/attempt fencing, cancellation and acknowledged preparation limits still apply. One initial attempt; no automatic retry or whole-Recipe/Tier-2 fallback. Invalid/missing assets, malformed contexts, duplicate keys, unavailable worker, template failure, nondeterminism, insufficient capacity or timeout stop the usage. Never shorten/truncate input, select latest, replace tokenizer configuration, increase resource limits or manufacture an acceptance result after failure. This usage changes no catalogue/model design, prefix generation, serving pointer, evidence approval or cache.\n\nCommand\n{pattern}. Variables in order: {variable_names}, all required and verbatim. Matching or binding grants nothing. Deterministic Tier 0 only after supported exact-combination qualification and activation.",
        ids.tool, ids.tool_skill, ids.python_code, ids.recipe
    );
    let drafts = [
        (
            ids.tool_skill,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":ids.tool,"callable":"host.model_encoding","capability_id":"builtin.model_encoding"}}),
            vec![ids.tool],
            Value::Null,
        ),
        (
            ids.python_code,
            22,
            json!({"name":name,"description":"Forward exact typed probes to one contained offline model encoding primitive.","content":code,"input_contract":inputs,"result_contract":result,"preload":preload}),
            vec![],
            Value::Null,
        ),
        (
            ids.skill,
            1,
            json!({"body":prose,"interface":{"format":"skill-interface/1","python_code_uuid":ids.python_code,"exports":preload["exports"],"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![ids.python_code, ids.tool_skill, ids.tool],
            json!(association),
        ),
        (
            ids.recipe,
            21,
            json!({"name":name,"description":"Canonical single-usage offline model encoding; acceptance decisions belong to consuming Recipes.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":variables}],
            "step_descriptions":[{"desc_idx":0,"label":"Offline model encoding","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"rust","goal":"Prepare model encoding binding","content":"","type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":"model_encoding","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Invoke pinned encoding export","content":"","type":"component","include":[ids.python_code]}]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":{"0:2":bindings}}},"invocations":{"selected":{"0:2":export}}}),
            vec![ids.tool_skill, ids.python_code, ids.skill],
            Value::Null,
        ),
    ];
    drafts.into_iter().map(|(uuid,class_code,document,dependencies,association)|ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":uuid,"class_code":class_code,"document":document,"dependencies":dependencies,"association":association}).to_string())).collect()
}
