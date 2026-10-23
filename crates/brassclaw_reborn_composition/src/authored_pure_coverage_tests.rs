//! Assembly-level occurrence policy over real typed IBS. These authoring drafts
//! have no approval or native artifact and are never dispatched/activated here.
//! Ordinary database/global acceptance is supplied by the native catalogue cases.
use super::*;
use brassclaw_engine::memory::{
    retained_instruction::compile_retained_recipe, retained_tools::prepare_retained_tool_program,
};
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot},
    global_bootstrap_components::{GlobalBootstrapUsage, UsageComponentIds, usage_drafts},
};

fn graph(same_variant: bool) -> (Arc<RetainedComponentSnapshot>, Uuid, Uuid) {
    let ids = UsageComponentIds {
        recipe: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        tool: Uuid::new_v4(),
        skill: Uuid::new_v4(),
        formatter: None,
    };
    let helper = Uuid::new_v4();
    let mut drafts = Vec::new();
    let inputs = json!({"answer":{"type":"string","required":true,"checks":[]}});
    let local = json!({"answer":{"kind":"task_input","reference":"{{vars.answer}}"}});
    for original in usage_drafts(ids, GlobalBootstrapUsage::PostReply).unwrap() {
        let mut document = original.document().clone();
        let mut dependencies = original.dependencies().clone();
        if original.uuid() == ids.python_code {
            document["content"] = json!(
                "def publish_turn_reply(inputs):\n    return host.post_reply(answer=answer_identity(inputs))\n"
            );
            document["preload"]["dependencies"] = json!([helper]);
            dependencies.insert(helper);
        } else if original.uuid() == ids.skill {
            document["interface"]["dependencies"] = json!([helper]);
        } else if original.uuid() == ids.recipe {
            dependencies.insert(helper);
            if same_variant {
                document["step_descriptions"][0]["steps"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"stepnumber":3,"knowledge":"orchestrator",
                        "goal":"Independently reuse the helper","content":"",
                        "type":"component","include":[helper]}));
                document["input_layouts"]["selected"]["steps"]["0:3"] = local.clone();
            } else {
                let mut variant = document["variants"][0].clone();
                variant["variant_key"] = json!("pure-only");
                variant["step_link"] = json!("1:1-1:E");
                variant["intent_examples"] = json!(["show literal answer %"]);
                document["variants"].as_array_mut().unwrap().push(variant);
                document["step_descriptions"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({
                    "desc_idx":1,"label":"Independent helper","yaml_source":"","steps":[{
                        "stepnumber":1,"knowledge":"orchestrator","goal":"Reuse the helper",
                        "content":"","type":"component","include":[helper]}]}));
                document["input_layouts"]["pure-only"] = json!({
                    "format":"recipe-input-layout/1","task_inputs":inputs,
                    "steps":{"1:1":local}});
            }
        }
        drafts.push(
            ComponentRevisionDraft::from_json(
                &json!({
                    "format":"component-revision/1","uuid":original.uuid(),
                    "class_code":original.class_code(),"document":document,
                    "dependencies":dependencies,
                    "association":original.association().map(|value|value.exact_bytes())
                })
                .to_string(),
            )
            .unwrap(),
        );
    }
    for (id, class, document) in [
        (
            ids.tool,
            0,
            json!({"capability_id":"host.post_reply","callable":"host.post_reply","input_contract":inputs}),
        ),
        (
            helper,
            22,
            json!({"content":"def answer_identity(inputs):\n    return inputs['answer']\n",
            "input_contract":inputs,"result_contract":{"type":"string"},
            "preload":{"format":"python-preload/2","exports":{"identity":{"symbol":"answer_identity","parameters":["inputs"],"mapping":true}},
                "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"identity"}}),
        ),
    ] {
        drafts.push(
            ComponentRevisionDraft::from_json(
                &json!({
                    "format":"component-revision/1","uuid":id,"class_code":class,
                    "document":document,"dependencies":[],"association":null
                })
                .to_string(),
            )
            .unwrap(),
        );
    }
    (
        Arc::new(
            RetainedComponentSnapshot::new(
                &[ids.recipe],
                drafts
                    .into_iter()
                    .map(|value| value.at_version(1).unwrap())
                    .collect(),
            )
            .unwrap(),
        ),
        ids.recipe,
        helper,
    )
}
fn selected(
    snapshot: Arc<RetainedComponentSnapshot>,
    recipe: Uuid,
    variant: &str,
) -> RetainedToolProgram {
    prepare_retained_tool_program(
        compile_retained_recipe(snapshot, recipe, variant, WorkflowClass::Deterministic).unwrap(),
    )
    .unwrap()
}

#[test]
fn bound_usage_dependency_does_not_require_independent_invocation_approval() {
    let (snapshot, recipe, helper) = graph(false);
    let program = selected(snapshot, recipe, "selected");
    assert!(
        program.bindings()["0:2"]
            .combination()
            .iter()
            .any(|r| r.uuid == helper)
    );
    require_independent_execution_coverage(&program, &BTreeSet::new()).unwrap();
}

#[test]
fn same_variant_dependency_membership_cannot_approve_an_independent_step() {
    let (snapshot, recipe, helper) = graph(true);
    let program = selected(snapshot, recipe, "selected");
    assert!(
        program.bindings()["0:2"]
            .combination()
            .iter()
            .any(|r| r.uuid == helper)
    );
    assert!(
        matches!(require_independent_execution_coverage(&program, &BTreeSet::new()),
        Err(RecipeStoreError::Invalid(reason)) if reason=="standalone execution lacks exact pure-component approval")
    );
    // Internal owner coverage must identify the selected pure component. The
    // set is not an operator input or an approval capability exposed by an API.
    assert!(
        require_independent_execution_coverage(&program, &BTreeSet::from([Uuid::new_v4()]))
            .is_err()
    );
    require_independent_execution_coverage(&program, &BTreeSet::from([helper])).unwrap();
}

#[test]
fn another_variants_usage_closure_cannot_approve_a_pure_only_variant() {
    let (snapshot, recipe, helper) = graph(false);
    let bound = selected(snapshot.clone(), recipe, "selected");
    let pure = selected(snapshot, recipe, "pure-only");
    assert!(
        bound.bindings()["0:2"]
            .combination()
            .iter()
            .any(|r| r.uuid == helper)
    );
    assert!(pure.bindings().is_empty());
    require_independent_execution_coverage(&bound, &BTreeSet::new()).unwrap();
    assert!(
        matches!(require_independent_execution_coverage(&pure, &BTreeSet::new()),
        Err(RecipeStoreError::Invalid(reason)) if reason=="standalone execution lacks exact pure-component approval")
    );
}
