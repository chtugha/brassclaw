//! Immutable draft usages for actual reply/history behavioral validation.
//! These records are not approval evidence or an activated catalogue.
use std::sync::Arc;

use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use serde_json::json;
use uuid::Uuid;

use brassclaw_skills::global_bootstrap_components::{
    GlobalBootstrapUsage, UsageComponentIds, usage_drafts, verify_packaged_usage,
};

pub(super) async fn program(
    store: &PgComponentRevisionStore,
    history: bool,
) -> Arc<RetainedToolProgram> {
    let (root, code, descriptor, tool, skill, formatter) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let usage = if history {
        GlobalBootstrapUsage::SaveHistory
    } else {
        GlobalBootstrapUsage::PostReply
    };
    let ids = UsageComponentIds {
        recipe: root,
        python_code: code,
        tool_skill: descriptor,
        tool,
        skill,
        formatter: history.then_some(formatter),
    };
    let drafts = usage_drafts(ids, usage).unwrap();
    let mut tool_inputs = drafts
        .iter()
        .find(|draft| draft.uuid() == code)
        .unwrap()
        .document()["input_contract"]
        .clone();
    if history {
        tool_inputs["target"] = json!({"type":"string","required":true,"checks":[]});
    }
    let (capability, callable) = if history {
        ("builtin.memory_write", "host.memory_write")
    } else {
        ("host.post_reply", "host.post_reply")
    };
    // Real primitive metadata for this constrained validation kernel. Packaged
    // usage drafts deliberately do not create or approve a replacement Tool.
    let tool_draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1", "uuid":tool,"class_code":0,
        "document":{"capability_id":capability,"callable":callable,"input_contract":tool_inputs},
        "dependencies":[],"association":null,
    }).to_string()).unwrap();
    let mut refs = Vec::new();
    for draft in std::iter::once(tool_draft).chain(drafts) {
        refs.push(store.stage(&draft, 0).await.unwrap());
    }
    let snapshot = Arc::new(store.read_exact(&[root], &refs).await.unwrap());
    let integrity = verify_packaged_usage(ids, usage, &snapshot).unwrap();
    assert_eq!(integrity.tool().uuid, tool);
    assert_eq!(integrity.components().len(), refs.len());
    Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(snapshot, root, "selected", WorkflowClass::Deterministic)
                .unwrap(),
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn packaged_usage_integrity_rejects_changed_code_and_retains_the_original_revision() {
    let database = crate::native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(database.pool.clone());
    let reply = program(&store, false).await;
    let instruction = reply.inputs().instruction();
    let original = instruction.snapshot();
    let binding = &reply.bindings()["0:2"];
    let ids = UsageComponentIds {
        recipe: instruction.recipe().uuid,
        python_code: binding.python().uuid,
        tool_skill: binding.tool_skill().uuid,
        tool: binding.tool().uuid,
        skill: binding.skill().uuid,
        formatter: None,
    };
    let integrity = verify_packaged_usage(ids, GlobalBootstrapUsage::PostReply, original).unwrap();
    let answer = "quotes ' Ü {{vars.answer}} host.forbidden()";
    for template in
        original.revisions()[&ids.recipe].draft().document()["variants"][0]["intent_examples"]
            .as_array()
            .unwrap()
    {
        let template = template.as_str().unwrap();
        let query = template.replace('%', answer);
        let bound = reply
            .inputs()
            .bind_variant_example(template, &query, &json!({}))
            .unwrap();
        assert_eq!(bound, json!({"answer":answer}));
    }
    let old_code = &original.revisions()[&ids.python_code];
    let mut document: serde_json::Value =
        serde_json::from_str(old_code.draft().exact_bytes()).unwrap();
    document["document"]["content"] = json!("result = 'different implementation'");
    let changed = ComponentRevisionDraft::from_json(&document.to_string()).unwrap();
    let changed_ref = store
        .stage(&changed, old_code.reference().version)
        .await
        .unwrap();
    let changed_refs: Vec<_> = integrity
        .components()
        .iter()
        .map(|reference| {
            if reference.uuid == ids.python_code {
                changed_ref
            } else {
                *reference
            }
        })
        .collect();
    let changed_snapshot = store
        .read_exact(&[ids.recipe], &changed_refs)
        .await
        .unwrap();
    assert!(
        verify_packaged_usage(ids, GlobalBootstrapUsage::PostReply, &changed_snapshot).is_err()
    );
    let retained = store
        .read_exact(&[ids.recipe], integrity.components())
        .await
        .unwrap();
    assert_eq!(
        retained.revisions()[&ids.python_code].draft().exact_bytes(),
        old_code.draft().exact_bytes()
    );
    assert!(verify_packaged_usage(ids, GlobalBootstrapUsage::PostReply, &retained).is_ok());
    assert_eq!(changed_ref.version, old_code.reference().version + 1);
}
