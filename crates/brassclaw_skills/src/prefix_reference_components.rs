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
            "private_functions":{"prefix_literal":["text"],"prefix_escape_controls":["text","special_tokens"],"prefix_source_inventory":["header","sources","special_tokens"],"render_prefix_reference":["header","sources","special_tokens"],"render_deduplicated_prefix_reference":["header","sources","special_tokens"]},
            "constants":[],"imports":[],"dependencies":[],"default_export":"compile_reference"}},
        "dependencies":[],"association":null}).to_string())
}
