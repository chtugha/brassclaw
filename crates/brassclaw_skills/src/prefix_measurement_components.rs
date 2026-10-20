//! Candidate Recipe composition over existing usages and pure PythonCode.
//! Construction neither qualifies nor activates a workflow or target.
use crate::{
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct MeasurementIds {
    pub recipe: Uuid,
    pub parse: Uuid,
    pub template: Uuid,
    pub texts: Uuid,
    pub verify: Uuid,
    pub capacity: Uuid,
    pub receipt: Uuid,
    pub digest: UsageComponentIds,
    pub encoding: UsageComponentIds,
}
impl MeasurementIds {
    pub fn allocate(
        digest: UsageComponentIds,
        encoding: UsageComponentIds,
        mut allocate: impl FnMut(&str) -> Uuid,
    ) -> Self {
        Self {
            recipe: allocate("prefix-measurement:recipe"),
            parse: allocate("prefix-measurement:parse"),
            template: allocate("prefix-measurement:template"),
            texts: allocate("prefix-measurement:texts"),
            verify: allocate("prefix-measurement:verify"),
            capacity: allocate("prefix-measurement:capacity"),
            receipt: allocate("prefix-measurement:receipt"),
            digest,
            encoding,
        }
    }
}
fn string() -> Value {
    json!({"type":"string","required":true,"checks":[]})
}
fn strings() -> Value {
    json!({"type":"list","required":true,"items":{"type":"string","checks":[]},"checks":[]})
}
fn request_fields() -> Value {
    let mut fields = json!({"reference":string(),"original_template":string(),"probes_json":string(),"specification":string(),"artifact_set":string(),"limits_ref":string()});
    for name in ["specification", "artifact_set", "probes_json", "limits_ref"] {
        fields[name]["checks"] = json!([{"kind":"min_length","value":1}]);
    }
    for (name, minimum) in [
        ("context_window_tokens", 1),
        ("reserved_output_tokens", 1),
        ("reserved_additional_input_tokens", 0),
    ] {
        fields[name] = json!({"type":"integer","required":true,"checks":[{"kind":"min","value":minimum},{"kind":"max","value":9007199254740991i64}]});
    }
    fields
}
fn pure(
    id: Uuid,
    name: &str,
    content: &str,
    inputs: Value,
    result: Value,
) -> Result<ComponentRevisionDraft, RevisionError> {
    crate::value_contract::InputContract::from_value(
        &inputs,
        crate::component_revision::REVISION_LIMITS,
    )?;
    crate::value_contract::ValueContract::result_from_value(
        &result,
        crate::component_revision::REVISION_LIMITS,
    )?;
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":name,"description":"Pure typed prefix measurement preparation/verification; no Tool call, source selection, approval or activation.","content":content,"includes":[],"dependency_registry":null,"input_contract":inputs,"result_contract":result,
            "preload":{"format":"python-preload/2","exports":{name:{"symbol":name,"parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":if content.contains("import json") {vec!["json"]} else {vec![]},"dependencies":[],"default_export":name}},"dependencies":[],"association":null}).to_string())
}

/// One explicit workflow: parse -> template -> digest preparation -> JSON usage
/// -> encoding usage -> template-identity verification -> capacity comparison -> receipt serialization.
/// The caller supplies the exact existing Tool identities and retains the graph.
pub fn measurement_drafts(
    ids: MeasurementIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut identities = vec![
        ids.recipe,
        ids.parse,
        ids.template,
        ids.texts,
        ids.verify,
        ids.capacity,
        ids.receipt,
        ids.digest.recipe,
        ids.digest.python_code,
        ids.digest.skill,
        ids.digest.tool_skill,
        ids.digest.tool,
        ids.encoding.recipe,
        ids.encoding.python_code,
        ids.encoding.skill,
        ids.encoding.tool_skill,
        ids.encoding.tool,
    ];
    identities.extend(ids.digest.formatter);
    identities.extend(ids.encoding.formatter);
    if identities.iter().any(Uuid::is_nil)
        || identities
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != identities.len()
    {
        return Err(RevisionError::Invalid(
            "measurement graph identities must be distinct and non-nil",
        ));
    }
    let fields = request_fields();
    let parse = "def parse_prefix_measurement(inputs):\n    import json\n    value = json.loads(inputs['request_json'])\n    names = ['reference', 'original_template', 'probes_json', 'specification', 'artifact_set', 'limits_ref', 'context_window_tokens', 'reserved_output_tokens', 'reserved_additional_input_tokens']\n    if not isinstance(value, dict) or sorted(value) != sorted(names):\n        raise ValueError('complete measurement request required')\n    if json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')) != inputs['request_json']:\n        raise ValueError('canonical measurement request required')\n    return value\n";
    let texts = "def prepare_template_texts(inputs):\n    return [inputs['effective_template'], inputs['original_template']]\n";
    let verify = "def verify_template_measurement(inputs):\n    import json\n    digests = inputs['digests']\n    if len(digests) != 2 or any(len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value) for value in digests):\n        raise ValueError('exact effective/original template digest pair required')\n    observed = json.loads(inputs['observation_bytes'])\n    if observed['model_chat_template_sha256'] != digests[1] or observed['report']['chat_template_sha256'] != digests[0] or observed['template_origin'] != 'supplied-template':\n        raise ValueError('measurement template identities differ')\n    return {'effective_template_sha256': digests[0], 'observation_bytes': inputs['observation_bytes']}\n";
    let capacity = crate::prefix_compiler_components::capacity_draft(ids.capacity)?;
    let mut comparison_input = capacity.document()["result_contract"].clone();
    comparison_input["required"] = json!(true);
    comparison_input["checks"] = json!([]);
    let receipt = pure(
        ids.receipt,
        "format_prefix_capacity_result",
        "def format_prefix_capacity_result(inputs):\n    import json\n    return json.dumps(inputs['comparison'], ensure_ascii=False, sort_keys=True, separators=(',', ':'))\n",
        json!({"comparison":comparison_input}),
        json!({"type":"string","checks":[]}),
    )?;
    let mut drafts = vec![
        pure(
            ids.parse,
            "parse_prefix_measurement",
            parse,
            json!({"request_json":string()}),
            json!({"type":"object","allow_extra_fields":false,"fields":fields}),
        )?,
        crate::prefix_reference_components::template_adapter_draft(ids.template)?,
        pure(
            ids.texts,
            "prepare_template_texts",
            texts,
            json!({"effective_template":string(),"original_template":string()}),
            json!({"type":"list","checks":[],"items":{"type":"string","checks":[]}}),
        )?,
        pure(
            ids.verify,
            "verify_template_measurement",
            verify,
            json!({"digests":strings(),"observation_bytes":string()}),
            json!({"type":"object","allow_extra_fields":false,"fields":{"effective_template_sha256":string(),"observation_bytes":string()}}),
        )?,
        capacity,
        receipt,
    ];
    drafts.extend(crate::utf8_digest_components::utf8_digest_usage_drafts(
        ids.digest,
    )?);
    drafts.extend(
        crate::model_encoding_components::model_template_encoding_usage_drafts(ids.encoding)?,
    );
    let task = json!({"kind":"task_input","reference":"{{vars.request_json}}"});
    let result = |step: usize, path: Vec<&str>| json!({"kind":"result","step_id":format!("0:{step}"),"path":path});
    let component = |step: usize, id: Uuid, goal: &str| json!({"stepnumber":step,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[id]});
    let binding = |step: usize, usage: UsageComponentIds, name: &str| json!({"stepnumber":step,"knowledge":"rust","goal":format!("Prepare {name} binding"),"content":"","type":"component","include":[usage.tool_skill],"tool_bindings":[{"tool_id":usage.tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]});
    let steps = vec![
        component(1, ids.parse, "Parse complete captured measurement inputs"),
        component(
            2,
            ids.template,
            "Prepare exact supported effective template",
        ),
        component(
            3,
            ids.texts,
            "Prepare ordered effective/original template texts",
        ),
        binding(4, ids.digest, "json"),
        component(5, ids.digest.python_code, "Hash the exact template pair"),
        binding(6, ids.encoding, "model_encoding"),
        component(
            7,
            ids.encoding.python_code,
            "Measure complete frozen provider probes",
        ),
        component(
            8,
            ids.verify,
            "Verify both template identities against the actual observation",
        ),
        component(
            9,
            ids.capacity,
            "Report capacity without truncation or approval",
        ),
        component(
            10,
            ids.receipt,
            "Serialize the exact calculation for internal completion",
        ),
    ];
    let mut layouts = json!({
        "0:1":{"request_json":task},
        "0:2":{"reference":result(1,vec!["reference"]),"original_template":result(1,vec!["original_template"])},
        "0:3":{"effective_template":result(2,vec![]),"original_template":result(1,vec!["original_template"])},
        "0:5":{"texts":result(3,vec![])},
        "0:7":{"specification":result(1,vec!["specification"]),"artifact_set":result(1,vec!["artifact_set"]),"probes_json":result(1,vec!["probes_json"]),"template_utf8":result(2,vec![])},
        "0:8":{"digests":result(5,vec!["digests"]),"observation_bytes":result(7,vec![])},
        "0:9":{"observation_bytes":result(8,vec!["observation_bytes"]),"expected_specification":result(1,vec!["specification"]),"expected_artifact_set":result(1,vec!["artifact_set"]),"expected_template_sha256":result(8,vec!["effective_template_sha256"])}
    });
    layouts["0:10"] = json!({"comparison":result(9,vec![])});
    for name in [
        "limits_ref",
        "context_window_tokens",
        "reserved_output_tokens",
        "reserved_additional_input_tokens",
    ] {
        layouts["0:9"][name] = result(1, vec![name]);
    }
    let pattern = "measure prefix json=%";
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
    let dependencies = vec![
        ids.parse,
        ids.template,
        ids.texts,
        ids.verify,
        ids.capacity,
        ids.receipt,
        ids.digest.skill,
        ids.digest.tool_skill,
        ids.digest.python_code,
        ids.encoding.skill,
        ids.encoding.tool_skill,
        ids.encoding.python_code,
    ];
    drafts.push(ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":ids.recipe,"class_code":21,
        "document":{"name":"measure-prefix-template-and-capacity","description":"Model-free candidate measurement over captured source/model/design inputs. Two independent native usages with pure preparation and identity/capacity checks. This does not choose sources/designs, verify deployed limits, approve quality/cache behavior or activate a generation.",
            "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":examples,"variable_patterns":[{"name":"request_json","pattern":null,"description":"Complete canonical JSON measurement request"}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Measure an exact candidate prefix","yaml_source":"","steps":steps}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"request_json":string()},"steps":layouts}},
            "invocations":{"selected":{"0:1":"parse_prefix_measurement","0:2":"build_prefix_template","0:3":"prepare_template_texts","0:5":"digest_utf8_texts","0:7":"encode_model_template_probes","0:8":"verify_template_measurement","0:9":"assess_prefix_capacity","0:10":"format_prefix_capacity_result"}}},
        "dependencies":dependencies,"association":null}).to_string())?);
    Ok(drafts)
}
