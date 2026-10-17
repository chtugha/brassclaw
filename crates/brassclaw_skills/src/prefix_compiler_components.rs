//! Reusable effect-free compiler library and small typed stage entry points.
//! Drafts are not qualified/activated merely because their code is packaged.
use crate::component_revision::{ComponentRevisionDraft, RevisionError};
use serde_json::{Value, json};
use uuid::Uuid;

pub const UNIT_SOURCE: &str = include_str!("../../../scripts/prefix/prefix_units.py");
pub const COMPILER_SOURCE: &str = include_str!("../../../scripts/prefix/prefix_compile_logic.py");
fn draft(
    id: Uuid,
    document: Value,
    dependencies: Vec<Uuid>,
) -> Result<ComponentRevisionDraft, RevisionError> {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":document,"dependencies":dependencies,"association":null})
        .to_string(),
    )
}
fn string() -> Value {
    json!({"type":"string","required":true,"checks":[]})
}
fn strings() -> Value {
    json!({"type":"list","required":true,"checks":[],"items":{"type":"string","checks":[]}})
}
pub fn unit_segmentation_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let source = format!(
        "{UNIT_SOURCE}\n\ndef segment_prefix_units(inputs):\n    return [{{'line_start':unit[0],'line_end':unit[1],'text':unit[2],'parent':unit[3]}} for unit in source_units_core(inputs['text'], inputs['suffix'])]\n"
    );
    draft(
        id,
        json!({"name":"segment-prefix-source-units","description":"Complete literal documentation units, preserving fenced blocks and whole executable files. No source selection or effects.",
        "content":source,"input_contract":{"text":string(),"suffix":{"type":"string","required":false,"default":".md","checks":[]}},
        "result_contract":{"type":"list","items":{"type":"object","allow_extra_fields":false,"fields":{"line_start":{"type":"integer","required":true},"line_end":{"type":"integer","required":true},"text":{"type":"string","required":true},"parent":{"type":"string","required":true}}}},
        "preload":{"format":"python-preload/2","exports":{"segment_prefix_units":{"symbol":"segment_prefix_units","parameters":["inputs"],"mapping":true}},
            "private_functions":{"source_units_core":["text","suffix"]},"constants":[],"imports":["re"],"dependencies":[],"default_export":"segment_prefix_units"}}),
        vec![],
    )
}

/// Internal dependencies provide actual retained definitions. Do not substitute
/// mutable Python imports, caller-selected scripts or latest dependency lookup.
pub fn compiler_library_draft(
    id: Uuid,
    units: Uuid,
    renderer: Uuid,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let declarations = json!({"prefix_json":["value"],"prefix_decode":["text"],"prefix_fields":["value","names"],"prefix_path":["name"],"prefix_digest_list":["values","count"],
        "prefix_prepare":["inputs"],"prefix_reference":["inputs"],"prefix_render":["inputs"],"prefix_identity":["inputs"],"prefix_finalize":["inputs"]});
    draft(
        id,
        json!({"name":"prefix-source-compiler-library","description":"Effect-free source-only compiler stages. Hashing, source reads, retention, target tokenization and job decisions remain explicit external Recipe steps.",
        "content":COMPILER_SOURCE,"input_contract":{"phase":string(),"snapshot_bytes":{"type":"string","required":false,"default":"","checks":[]},"state_bytes":{"type":"string","required":false,"default":"","checks":[]},"digests":{"type":"list","required":false,"default":[],"checks":[],"items":{"type":"string","checks":[]}}},
        "result_contract":{"type":"string"},
        "preload":{"format":"python-preload/2","exports":{"compile_stage":{"symbol":"compile_stage","parameters":["inputs"],"mapping":true}},
            "private_functions":declarations,"constants":[],"imports":["json","base64"],"dependencies":[units,renderer],"default_export":"compile_stage"}}),
        vec![units, renderer],
    )
}

pub fn compiler_stage_draft(
    id: Uuid,
    library: Uuid,
    stage: &str,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let (export, inputs, result) = match stage {
        "prepare" => (
            "prepare_prefix_sources",
            json!({"snapshot_bytes":string()}),
            json!({"type":"object","allow_extra_fields":false,"fields":{"state_bytes":{"type":"string","required":true},"texts":{"type":"list","required":true,"items":{"type":"string"}}}}),
        ),
        "reference" => (
            "prepare_prefix_reference",
            json!({"state_bytes":string(),"digests":strings()}),
            json!({"type":"object","allow_extra_fields":false,"fields":{"state_bytes":{"type":"string","required":true},"texts":{"type":"list","required":true,"items":{"type":"string"}}}}),
        ),
        "render" => (
            "render_prefix_sources",
            json!({"state_bytes":string(),"digests":strings()}),
            json!({"type":"object","allow_extra_fields":false,"fields":{"state_bytes":{"type":"string","required":true},"texts":{"type":"list","required":true,"items":{"type":"string"}}}}),
        ),
        "identity" => (
            "prepare_prefix_identity",
            json!({"state_bytes":string(),"digests":strings()}),
            json!({"type":"object","allow_extra_fields":false,"fields":{"state_bytes":{"type":"string","required":true},"texts":{"type":"list","required":true,"items":{"type":"string"}}}}),
        ),
        "finalize" => (
            "finalize_prefix_package",
            json!({"state_bytes":string(),"digests":strings()}),
            json!({"type":"string","checks":[{"kind":"min_length","value":1}]}),
        ),
        _ => return Err(RevisionError::Invalid("unsupported compiler stage")),
    };
    let function = format!("prefix_{stage}");
    let body = format!("def {export}(inputs):\n    return {function}(inputs)\n");
    draft(
        id,
        json!({"name":format!("prefix-compiler-{stage}"),"description":format!("One typed effect-free prefix {stage} transformation; no Tool dispatch or activation."),
        "content":body,"input_contract":inputs,"result_contract":result,
        "preload":{"format":"python-preload/2","exports":{export:{"symbol":export,"parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":[],"dependencies":[library],"default_export":export}}),
        vec![library],
    )
}

/// Compose existing one-Tool usages with the pure stages. The caller must
/// retain and qualify the complete graph before exposing it to admission.
pub fn source_compile_recipe_draft(
    id: Uuid,
    stages: [Uuid; 5],
    receipt: Uuid,
    digest: crate::global_bootstrap_components::UsageComponentIds,
    retain: crate::global_bootstrap_components::UsageComponentIds,
) -> Result<ComponentRevisionDraft, RevisionError> {
    let revision = retain.formatter.ok_or(RevisionError::Invalid(
        "compile Recipe requires revision formatter",
    ))?;
    let mut identities = vec![
        id,
        receipt,
        revision,
        digest.recipe,
        digest.tool,
        digest.skill,
        digest.tool_skill,
        digest.python_code,
        retain.recipe,
        retain.tool,
        retain.skill,
        retain.tool_skill,
        retain.python_code,
    ];
    identities.extend(stages);
    if identities.iter().any(Uuid::is_nil)
        || identities
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != identities.len()
    {
        return Err(RevisionError::Invalid(
            "compile Recipe identities must be distinct and non-nil",
        ));
    }
    let mut steps = Vec::new();
    let mut layouts = serde_json::Map::new();
    let mut invocations = serde_json::Map::new();
    let component = |number: usize, code: Uuid, goal: &str| json!({"stepnumber":number,"knowledge":"orchestrator","goal":goal,"content":"","type":"component","include":[code]});
    let task = |name: &str| json!({"kind":"task_input","reference":format!("{{{{vars.{name}}}}}")});
    let result = |number: usize, path: Vec<&str>| json!({"kind":"result","step_id":format!("0:{number}"),"path":path});
    let binding = |number: usize,
                   ids: crate::global_bootstrap_components::UsageComponentIds,
                   name: &str| json!({"stepnumber":number,"knowledge":"rust","goal":format!("Bind {name}"),"content":"","type":"component","include":[ids.tool_skill],"tool_bindings":[{"tool_id":ids.tool,"tool_name":name,"params":{},"error_policy":{"policy":"fail"}}]});
    steps.push(component(1, revision, "Validate retained profile revision"));
    layouts.insert("0:1".into(), json!({"revision":task("revision")}));
    invocations.insert("0:1".into(), json!("prepare_generation_revision"));
    steps.push(component(2, stages[0], "Prepare complete pinned sources"));
    layouts.insert(
        "0:2".into(),
        json!({"snapshot_bytes":task("snapshot_bytes")}),
    );
    invocations.insert("0:2".into(), json!("prepare_prefix_sources"));
    let names = [
        "prepare_prefix_reference",
        "render_prefix_sources",
        "prepare_prefix_identity",
        "finalize_prefix_package",
    ];
    let mut previous = 2;
    for index in 0..4 {
        let bind = 3 + index * 3;
        let hash = bind + 1;
        let pure = bind + 2;
        steps.push(binding(bind, digest, "json"));
        steps.push(component(
            hash,
            digest.python_code,
            "Hash complete ordered stage data",
        ));
        layouts.insert(
            format!("0:{hash}"),
            json!({"texts":result(previous,vec!["texts"])}),
        );
        invocations.insert(format!("0:{hash}"), json!("digest_utf8_texts"));
        steps.push(component(
            pure,
            stages[index + 1],
            "Transform exact pinned stage data",
        ));
        layouts.insert(format!("0:{pure}"),json!({"state_bytes":result(previous,vec!["state_bytes"]),"digests":result(hash,vec!["digests"])}));
        invocations.insert(format!("0:{pure}"), json!(names[index]));
        previous = pure;
    }
    steps.push(binding(15, retain, "prefix_generation"));
    steps.push(component(
        16,
        retain.python_code,
        "Retain quarantined immutable source generation",
    ));
    layouts.insert("0:16".into(),json!({"profile":task("profile"),"expected_profile_revision":result(1,vec![]),"generation_bytes":result(14,vec![])}));
    invocations.insert("0:16".into(), json!("retain_prefix_generation"));
    steps.push(component(
        17,
        receipt,
        "Format the actual quarantined generation receipt",
    ));
    layouts.insert("0:17".into(),json!({"profile":result(16,vec!["profile"]),"expected_profile":task("profile"),"generation":result(16,vec!["generation"]),"qualification":result(16,vec!["qualification"])}));
    invocations.insert("0:17".into(), json!("format_prefix_generation_receipt"));
    let mut dependencies = vec![
        receipt,
        revision,
        digest.skill,
        digest.tool_skill,
        digest.python_code,
        retain.skill,
        retain.tool_skill,
        retain.python_code,
    ];
    dependencies.extend(stages);
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":21,
        "document":{"name":"compile-prefix-source-generation","description":"Tier-0 source-only compilation from a pinned complete source snapshot: five pure PythonCode stages, four explicit JSON hashing usages and one immutable retention. No collection, tokenization, quality approval, activation or provider request.",
        "variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["compile prefix profile=% revision=% snapshot=%"],"variable_patterns":[{"name":"profile","pattern":null,"description":null},{"name":"revision","pattern":null,"description":null},{"name":"snapshot_bytes","pattern":null,"description":null}]}],
        "step_descriptions":[{"desc_idx":0,"label":"Compile and retain a source generation","yaml_source":"","steps":steps}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"profile":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},"revision":{"type":"string","required":true,"checks":[{"kind":"min_length","value":1}]},"snapshot_bytes":string()},"steps":layouts}},"invocations":{"selected":invocations}},"dependencies":dependencies,"association":null}).to_string())
}

/// Format a task receipt from the actual retention result. The admission owner
/// must independently verify its durable row; this pure string grants no trust.
pub fn generation_receipt_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let content = "def format_prefix_generation_receipt(inputs):\n    profile = inputs['profile']\n    generation = inputs['generation']\n    if profile != inputs['expected_profile'] or not profile or any(ch not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for ch in profile) or profile[0] not in 'abcdefghijklmnopqrstuvwxyz':\n        raise ValueError('retained profile differs')\n    if len(generation) != 64 or any(ch not in '0123456789abcdef' for ch in generation) or inputs['qualification'] != 'quarantined':\n        raise ValueError('quarantined immutable generation receipt required')\n    return 'prefix-generation:' + profile + ':' + generation\n";
    draft(
        id,
        json!({"name":"format-prefix-generation-receipt","description":"Format the exact quarantined storage receipt for internal task completion. Durable verification belongs to the admission owner; no activation or approval.","content":content,"input_contract":{"profile":string(),"expected_profile":string(),"generation":string(),"qualification":string()},"result_contract":{"type":"string"},"preload":{"format":"python-preload/2","exports":{"format_prefix_generation_receipt":{"symbol":"format_prefix_generation_receipt","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"format_prefix_generation_receipt"}}),
        vec![],
    )
}

/// Stable component identities supplied by the installation/catalogue owner.
/// Tool identities are resolved existing primitives, never allocated per Recipe.
#[derive(Clone, Copy)]
pub struct SourceCompilerIds {
    pub recipe: Uuid,
    pub units: Uuid,
    pub renderer: Uuid,
    pub library: Uuid,
    pub stages: [Uuid; 5],
    pub receipt: Uuid,
    pub digest: crate::global_bootstrap_components::UsageComponentIds,
    pub retain: crate::global_bootstrap_components::UsageComponentIds,
}
impl SourceCompilerIds {
    pub fn allocate(
        json_tool: Uuid,
        generation_tool: Uuid,
        mut allocate: impl FnMut(&str) -> Uuid,
    ) -> Self {
        use crate::global_bootstrap_components::UsageComponentIds;
        Self {
            recipe: allocate("prefix-source-compiler:recipe"),
            units: allocate("prefix-source-compiler:units"),
            renderer: allocate("prefix-source-compiler:renderer"),
            library: allocate("prefix-source-compiler:library"),
            stages: ["prepare", "reference", "render", "identity", "finalize"]
                .map(|stage| allocate(&format!("prefix-source-compiler:{stage}"))),
            receipt: allocate("prefix-source-compiler:receipt"),
            digest: UsageComponentIds {
                recipe: allocate("utf8-digest:recipe"),
                python_code: allocate("utf8-digest:code"),
                skill: allocate("utf8-digest:skill"),
                tool_skill: allocate("utf8-digest:binding"),
                tool: json_tool,
                formatter: Some(allocate("utf8-digest:argument-parser")),
            },
            // Reuse the shipped canonical storage usage identities and exact
            // definitions; do not create a second Retain Skill/Recipe.
            retain: UsageComponentIds {
                recipe: allocate("prefix-generation:retain:recipe"),
                python_code: allocate("prefix-generation:retain:code"),
                skill: allocate("prefix-generation:retain:skill"),
                tool_skill: allocate("prefix-generation:retain:binding"),
                tool: generation_tool,
                formatter: Some(allocate("prefix-generation:revision-formatter")),
            },
        }
    }
}

/// One definition source for installation candidates and behavioral validation.
/// The caller separately retains the actual two existing Tool revisions, pins
/// the complete graph and obtains its applicable closed bootstrap approvals.
pub fn source_compiler_drafts(
    ids: SourceCompilerIds,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    let mut identities = vec![
        ids.recipe,
        ids.units,
        ids.renderer,
        ids.library,
        ids.receipt,
        ids.digest.recipe,
        ids.digest.python_code,
        ids.digest.skill,
        ids.digest.tool_skill,
        ids.digest.tool,
        ids.retain.recipe,
        ids.retain.python_code,
        ids.retain.skill,
        ids.retain.tool_skill,
        ids.retain.tool,
    ];
    identities.extend(ids.stages);
    identities.extend(ids.retain.formatter);
    identities.extend(ids.digest.formatter);
    if identities.iter().any(Uuid::is_nil)
        || identities
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != identities.len()
    {
        return Err(RevisionError::Invalid(
            "compiler graph identities must be distinct and non-nil",
        ));
    }
    let mut drafts = vec![
        unit_segmentation_draft(ids.units)?,
        crate::prefix_reference_components::reference_render_draft(ids.renderer)?,
        compiler_library_draft(ids.library, ids.units, ids.renderer)?,
        generation_receipt_draft(ids.receipt)?,
    ];
    for (id, stage) in
        ids.stages
            .into_iter()
            .zip(["prepare", "reference", "render", "identity", "finalize"])
    {
        drafts.push(compiler_stage_draft(id, ids.library, stage)?);
    }
    drafts.extend(crate::utf8_digest_components::utf8_digest_usage_drafts(
        ids.digest,
    )?);
    drafts.extend(
        crate::prefix_generation_components::generation_usage_drafts(
            ids.retain,
            crate::prefix_generation_components::GenerationUsage::Retain,
        )?,
    );
    drafts.push(source_compile_recipe_draft(
        ids.recipe,
        ids.stages,
        ids.receipt,
        ids.digest,
        ids.retain,
    )?);
    Ok(drafts)
}
