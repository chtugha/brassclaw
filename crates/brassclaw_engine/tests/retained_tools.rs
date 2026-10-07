//! Actual PostgreSQL retention -> IBS -> explicit Tool/usage binding requests.
//! Drafts exercise preparation only: no activation, permission or Tool effect.
#![cfg(feature = "skills-db")]

use std::{collections::BTreeMap, sync::Arc};

use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_reviews::{RetainedReviewError, retain_prepared_tool_reviews},
    retained_tools::{RetainedToolError, prepare_retained_tool_program},
};
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot},
    revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "../../brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;

fn draft(
    id: Uuid,
    class: i32,
    document: Value,
    deps: &[Uuid],
    association: Value,
) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1", "uuid":id,
        "class_code":class,"document":document,"dependencies":deps,"association":association})
        .to_string(),
    )
    .unwrap()
}
fn compile(
    snapshot: Arc<RetainedComponentSnapshot>,
    root: Uuid,
) -> Result<brassclaw_engine::memory::retained_tools::RetainedToolProgram, RetainedToolError> {
    prepare_retained_tool_program(
        compile_retained_recipe(snapshot, root, "selected", WorkflowClass::Deterministic).unwrap(),
    )
}

#[tokio::test]
async fn prepared_binding_retains_exact_usage_and_checks_actual_arguments_without_dispatching() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let (recipe, code, tool_skill, tool, skill) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let inputs = json!({"answer":{"type":"string","required":true,"checks":[]}});
    let result = json!({"type":"string"});
    let assoc = json!({"format":"skill-association/1", "skill_uuid":skill, "python_code_uuid":code,
        "tool_skill_uuid":tool_skill,"tool_uuid":tool,"callable":"host.post_reply","inputs":inputs,
        "arguments":{"answer":"answer"},"code_arguments":{},"result":result,
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed",
            "idempotency_evidence_ref":null,"retryable_outcomes":[]}})
    .to_string();
    let tool_document = json!({"capability_id":"post_reply", "callable":"host.post_reply", "input_contract":inputs});
    let descriptor_document = json!({"name":"bind-post-reply", "binding":{"format":"tool-skill-binding/1",
        "tool_uuid":tool,"callable":"host.post_reply","capability_id":"post_reply"}});
    let code_document = json!({"content":"result = host.post_reply(answer=inputs['answer'])", "input_contract":inputs, "result_contract":result});
    let recipe_document = json!({"variants":[{"variant_key":"selected","step_link":"0:1-0:E",
        "intent_examples":["reply %"],"variable_patterns":[{"name":"answer","pattern":null,"description":null}]}],
        "step_descriptions":[{"desc_idx":0,"label":"reply","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"rust","goal":"Bind reply","content":"","type":"component","include":[tool_skill],
                "tool_bindings":[{"tool_id":tool,"tool_name":"post_reply","params":{},"error_policy":{"policy":"fail"}}]},
            {"stepnumber":2,"knowledge":"orchestrator","goal":"Post the reply","content":"","type":"component","include":[code]}]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,
            "steps":{"0:2":{"answer":{"kind":"task_input","reference":"{{vars.answer}}"}}}}}});
    let tool_ref = store
        .stage(&draft(tool, 0, tool_document.clone(), &[], Value::Null), 0)
        .await
        .unwrap();
    let descriptor_ref = store
        .stage(
            &draft(
                tool_skill,
                13,
                descriptor_document.clone(),
                &[tool],
                Value::Null,
            ),
            0,
        )
        .await
        .unwrap();
    let code_ref = store
        .stage(&draft(code, 22, code_document.clone(), &[], Value::Null), 0)
        .await
        .unwrap();
    let skill_ref = store
        .stage(
            &draft(
                skill,
                1,
                json!({"body":"Post the supplied answer and return the reply reference."}),
                &[code, tool_skill, tool],
                json!(assoc),
            ),
            0,
        )
        .await
        .unwrap();
    let root_ref = store
        .stage(
            &draft(
                recipe,
                21,
                recipe_document.clone(),
                &[tool_skill, code, skill],
                Value::Null,
            ),
            0,
        )
        .await
        .unwrap();
    let refs = [root_ref, tool_ref, descriptor_ref, code_ref, skill_ref];
    let old = Arc::new(store.read_exact(&[recipe], &refs).await.unwrap());
    let prepared = compile(old.clone(), recipe).unwrap();
    assert_eq!(prepared.program().steplist.len(), 1);
    assert!(
        prepared.program().rust_directives.is_empty(),
        "no legacy mutable artifact loading directives"
    );
    assert!(prepared.program().variables.is_empty());
    assert!(prepared.program().assembled_program.is_empty());
    assert_eq!(
        prepared.program().steplist[0].executable_code,
        code_document["content"]
    );
    let binding = &prepared.bindings()["0:2"];
    assert_eq!(binding.tool(), tool_ref);
    assert_eq!(binding.tool_skill(), descriptor_ref);
    assert_eq!(binding.skill(), skill_ref);
    assert_eq!(binding.python(), code_ref);
    assert_eq!(binding.capability_id(), "post_reply");
    assert_eq!(binding.association().exact_bytes(), assoc);
    let mut combination = binding.combination().to_vec();
    combination.sort_by_key(|r| r.uuid);
    let mut expected = vec![tool_ref, descriptor_ref, code_ref, skill_ref];
    expected.sort_by_key(|r| r.uuid);
    assert_eq!(combination, expected);
    assert!(!combination.iter().any(|r| r.uuid == recipe));
    // IBS binding/complete selection is not combination review. Missing IDs
    // and a nonexistent actual record remain explicit failures before effects.
    let mut selecting_client = rig.pool.get().await.unwrap();
    let selecting = selecting_client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    assert!(matches!(
        retain_prepared_tool_reviews(&selecting, &prepared, &BTreeMap::new()).await,
        Err(RetainedReviewError::Selection)
    ));
    assert!(matches!(
        retain_prepared_tool_reviews(
            &selecting,
            &prepared,
            &BTreeMap::from([(skill, Uuid::new_v4())])
        )
        .await,
        Err(RetainedReviewError::Records(_))
    ));
    selecting.commit().await.unwrap();
    let data = json!({"answer":"'quoted' Ü\n{{vars.answer}}"});
    assert_eq!(binding.bind_tool_arguments(&data, &data).unwrap(), data);
    for bad in [
        json!({"answer":"other"}),
        json!({"answer":null}),
        json!({"answer":12}),
        json!({"answer":data["answer"],"grant":true}),
    ] {
        assert!(binding.bind_tool_arguments(&data, &bad).is_err());
    }
    let newer = store.stage(&draft(tool,0,json!({"capability_id":"changed-reply", "callable":"host.changed_reply","input_contract":inputs}),&[],Value::Null),1).await.unwrap();
    assert!(matches!(
        compile(
            Arc::new(
                store
                    .read_exact(
                        &[recipe],
                        &[root_ref, newer, descriptor_ref, code_ref, skill_ref]
                    )
                    .await
                    .unwrap()
            ),
            recipe
        ),
        Err(RetainedToolError::Invalid(
            "Tool/ToolSkill/Recipe identities disagree"
        ))
    ));
    assert_eq!(
        compile(old, recipe).unwrap().bindings()["0:2"].tool(),
        tool_ref
    );

    // Every rejected replacement comes from real append-only revision storage.
    // None can mutate the retained original or manufacture an executable grant.
    let mut changes = Vec::new();
    let mut missing = descriptor_document.clone();
    missing.as_object_mut().unwrap().remove("binding");
    changes.push((
        tool_skill,
        13,
        missing,
        vec![tool],
        "explicit retained ToolSkill binding required",
    ));
    let mut extra = descriptor_document.clone();
    extra["binding"]["grant"] = json!(true);
    changes.push((
        tool_skill,
        13,
        extra,
        vec![tool],
        "explicit retained ToolSkill binding required",
    ));
    changes.push((
        tool_skill,
        13,
        descriptor_document.clone(),
        vec![],
        "ToolSkill must declare its Tool dependency",
    ));
    let mut parameters = recipe_document.clone();
    parameters["step_descriptions"][0]["steps"][0]["tool_bindings"][0]["params"] =
        json!({"answer":"{{vars.answer}}"});
    changes.push((
        recipe,
        21,
        parameters,
        vec![tool_skill, code, skill],
        "legacy invocation parameters or error policy cannot prepare a v3 binding",
    ));
    let mut ignore = recipe_document.clone();
    ignore["step_descriptions"][0]["steps"][0]["tool_bindings"][0]["error_policy"] =
        json!({"policy":"ignore"});
    changes.push((
        recipe,
        21,
        ignore,
        vec![tool_skill, code, skill],
        "legacy invocation parameters or error policy cannot prepare a v3 binding",
    ));
    let mut schema = code_document.clone();
    schema["input_contract"]["answer"]["type"] = json!("integer");
    changes.push((
        code,
        22,
        schema,
        vec![],
        "PythonCode contracts differ from its usage association",
    ));
    let mut tool_contract = tool_document.clone();
    tool_contract["input_contract"]["answer"]["type"] = json!("integer");
    let incompatible = store
        .stage(&draft(tool, 0, tool_contract, &[], Value::Null), 2)
        .await
        .unwrap();
    assert!(matches!(
        compile(
            Arc::new(
                store
                    .read_exact(
                        &[recipe],
                        &[root_ref, incompatible, descriptor_ref, code_ref, skill_ref]
                    )
                    .await
                    .unwrap()
            ),
            recipe
        ),
        Err(RetainedToolError::Association(_))
    ));
    let mut versions = std::collections::BTreeMap::from([(tool_skill, 1), (recipe, 1), (code, 1)]);
    for (id, class, document, deps, reason) in changes {
        let previous = versions[&id];
        let replacement = store
            .stage(&draft(id, class, document, &deps, Value::Null), previous)
            .await
            .unwrap();
        versions.insert(id, previous + 1);
        let refs: Vec<_> = refs
            .iter()
            .map(|r| if r.uuid == id { replacement } else { *r })
            .collect();
        assert!(
            matches!(compile(Arc::new(store.read_exact(&[recipe],&refs).await.unwrap()),recipe),Err(RetainedToolError::Invalid(actual)) if actual == reason)
        );
    }
    assert!(
        compile(
            Arc::new(store.read_exact(&[recipe], &refs).await.unwrap()),
            recipe
        )
        .is_ok()
    );
}
