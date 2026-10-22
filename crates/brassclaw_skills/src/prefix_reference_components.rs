//! Pure prefix rendering building block. Packaged draft construction is not
//! source approval, bootstrap qualification, catalogue activation or a Tool grant.
use crate::component_revision::{ComponentRevisionDraft, RevisionError};
use serde_json::json;
use uuid::Uuid;

pub const REFERENCE_RENDER_SOURCE: &str = include_str!("../../../scripts/prefix/prefix_render.py");

/// Render already verified and canonically ordered typed source data. The exact
/// same effect-free definitions serve the offline compiler and Monty preload.
pub fn reference_render_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let string = json!({"type":"string","required":true,"checks":[]});
    let source = json!({"type":"object","checks":[],"allow_extra_fields":false,"fields":{
        "path":string,"disposition":string,"reason":string,"text":string,"evidence_id":string}});
    let body = format!(
        "{REFERENCE_RENDER_SOURCE}\n\ndef compile_reference(inputs):\n    return render_prefix_reference(inputs['header'], inputs['sources'], inputs['special_tokens'])\n"
    );
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"prefix-render-reference","description":"Literal rendering of complete verified originals with stable evidence labels; no source selection, hashing, tokenization or effects.",
        "content":body,"includes":[],"dependency_registry":null,
        "input_contract":{"header":string,"sources":{"type":"list","required":true,"checks":[],"items":source},"special_tokens":{"type":"list","required":true,"checks":[],"items":{"type":"string","checks":[]}}},
        "result_contract":{"type":"string"},
        "preload":{"format":"python-preload/2","exports":{"compile_reference":{"symbol":"compile_reference","parameters":["inputs"],"mapping":true}},
            "private_functions":{"prefix_literal":["text"],"prefix_escape_controls":["text","special_tokens"],"prefix_source_inventory":["header","sources","special_tokens"],"render_prefix_reference":["header","sources","special_tokens"],"render_deduplicated_prefix_reference":["header","sources","special_tokens"],"render_trailing_inventory_prefix_reference":["header","sources","special_tokens"],"prefix_render_units":["header","sources","special_tokens","inventory_last"],"render_stable_sharing_prefix_reference":["header","sources","special_tokens"],"prefix_progressive_units":["source","groups","emitted","special_tokens","scope"],"prefix_source_context":["source"],"prefix_context_caption":["scope"],"render_contextual_prefix_reference":["header","sources","special_tokens"]},
            "constants":[],"imports":[],"dependencies":[],"default_export":"compile_reference"}},
        "dependencies":[],"association":null}).to_string())
}

/// Exact shared offline/Recipe adapter. This prepares literal template data;
/// supported-target qualification and activation are separate.
pub fn template_adapter_draft(id: Uuid) -> Result<ComponentRevisionDraft, RevisionError> {
    let string = json!({"type":"string","required":true,"checks":[]});
    let body = format!(
        "{}\n\ndef build_prefix_template(inputs):\n    return inject_reference(inputs['reference'], inputs['original_template'])\n",
        include_str!("../../../scripts/prefix/prefix_template.py")
    );
    ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":22,
        "document":{"name":"build-ornith-prefix-template","description":"Prepare exact Ornith/Qwen stable-leading reference injection bytes with the shared offline adapter. Reject changed template anchors; no fallback, tokenization, serving edit or cache claim.",
            "content":body,"includes":[],"dependency_registry":null,"input_contract":{"reference":string,"original_template":string},"result_contract":{"type":"string"},
            "preload":{"format":"python-preload/2","exports":{"build_prefix_template":{"symbol":"build_prefix_template","parameters":["inputs"],"mapping":true}},"private_functions":{"inject_reference":["full","original"]},"constants":[],"imports":["json"],"dependencies":[],"default_export":"build_prefix_template"}},
        "dependencies":[],"association":null}).to_string())
}
