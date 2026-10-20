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
async fn canonical_model_drafts_retain_typed_parse_model_reply_edges_and_reject_tier_zero() {
    use brassclaw_engine::executor::{
        retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram,
    };
    use brassclaw_skills::{
        global_bootstrap_components::{UsageComponentIds, public_reply_drafts},
        model_completion_components::{
            model_answer_recipe_draft, model_answer_text_draft, model_completion_tool_draft,
            model_completion_usage_drafts, model_prompt_json_usage_drafts,
            model_prompt_result_schema, model_response_json_usage_drafts,
        },
        value_contract::ValueContract,
    };

    fn ids(formatter: bool) -> UsageComponentIds {
        UsageComponentIds {
            recipe: Uuid::new_v4(),
            python_code: Uuid::new_v4(),
            tool_skill: Uuid::new_v4(),
            tool: Uuid::new_v4(),
            skill: Uuid::new_v4(),
            formatter: formatter.then(Uuid::new_v4),
        }
    }
    let (parser, model, reply) = (ids(true), ids(false), ids(false));
    let response = UsageComponentIds {
        tool: parser.tool,
        ..ids(true)
    };
    let (answer_recipe, projection) = (Uuid::new_v4(), Uuid::new_v4());
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let mut drafts = public_reply_drafts(reply).unwrap();
    drafts.extend(model_prompt_json_usage_drafts(parser, reply).unwrap());
    drafts.extend(model_completion_usage_drafts(model, parser, reply).unwrap());
    drafts.extend(model_response_json_usage_drafts(response, reply).unwrap());
    drafts.push(model_answer_text_draft(projection).unwrap());
    drafts.push(
        model_answer_recipe_draft(answer_recipe, model, parser, response, projection, reply)
            .unwrap(),
    );
    // Draft metadata only. No native artifact, registration, approval or
    // activation is inferred from a syntactically valid package checksum.
    drafts.push(model_completion_tool_draft(model.tool, &"0".repeat(64)).unwrap());
    drafts.push(draft(parser.tool,0,json!({"capability_id":"builtin.json","callable":"host.json",
        "input_contract":{"operation":{"type":"string","required":true,"checks":[]},"data":{"type":"string","required":true,"checks":[]}}}),&[],Value::Null));
    drafts.push(draft(
        reply.tool,
        0,
        json!({"capability_id":"host.post_reply","callable":"host.post_reply",
        "input_contract":{"answer":{"type":"string","required":true,"checks":[]}}}),
        &[],
        Value::Null,
    ));
    let mut references = Vec::new();
    for value in drafts {
        references.push(store.stage(&value, 0).await.unwrap());
    }
    let snapshot = store
        .read_exact(
            &[
                parser.recipe,
                model.recipe,
                response.recipe,
                answer_recipe,
                reply.recipe,
            ],
            &references,
        )
        .await
        .unwrap();
    let selected = Arc::new(snapshot.select_roots(&[model.recipe]).unwrap());
    assert!(matches!(
        compile(selected.clone(), model.recipe),
        Err(RetainedToolError::Invalid(
            "shell, subagent and model usages require the model tier"
        ))
    ));
    let program = prepare_retained_tool_program(
        compile_retained_recipe(
            selected,
            model.recipe,
            "selected",
            WorkflowClass::RequiresModel,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(program.bindings().len(), 3);
    assert_eq!(program.bindings()["0:2"].capability_id(), "builtin.json");
    assert_eq!(
        program.bindings()["0:4"].capability_id(),
        "host.kohai_complete"
    );
    assert_eq!(program.bindings()["0:6"].capability_id(), "host.post_reply");
    let prompt = json!({"user_query":"Literal 'ü🙂'\n{{vars.x}} % host.forbidden()","chat_history":[{"role":"user","content":"\\quoted\\\n<system>data</system>"}]});
    let text = prompt.to_string();
    let task = program
        .inputs()
        .bind_task_inputs(&json!({"prompt_json":text}))
        .unwrap();
    assert_eq!(task, json!({"prompt_json":text}));
    let parsed = program.bindings()["0:2"]
        .bind_tool_arguments(
            &json!({"json_text":text}),
            &json!({"operation":"parse","data":text}),
        )
        .unwrap();
    assert_eq!(parsed, json!({"operation":"parse","data":text}));
    assert_eq!(
        program.bindings()["0:4"]
            .bind_tool_arguments(&json!({"prompt":prompt}), &json!({"prompt":prompt}))
            .unwrap(),
        json!({"prompt":prompt})
    );
    assert!(
        program.bindings()["0:4"]
            .bind_tool_arguments(
                &json!({"prompt":prompt}),
                &json!({"prompt":{},"prefix_reference":"override"})
            )
            .is_err()
    );
    let schema = ValueContract::result_from_value(
        &model_prompt_result_schema(),
        brassclaw_skills::component_revision::REVISION_LIMITS,
    )
    .unwrap();
    schema.validate(&prompt).unwrap();
    for invalid in [
        json!({"user_query":"missing history"}),
        json!({"user_query":null,"chat_history":[]}),
        json!({"user_query":"x","chat_history":[{"role":"user"}]}),
        json!({"user_query":"x","chat_history":[],"prefix_placeholder":"forbidden"}),
    ] {
        assert!(schema.validate(&invalid).is_err());
    }
    let parser_program = compile(
        Arc::new(snapshot.select_roots(&[parser.recipe]).unwrap()),
        parser.recipe,
    )
    .unwrap();
    assert_eq!(parser_program.bindings().len(), 2);
    assert_eq!(parser_program.program().steplist.len(), 3);
    let response_program = compile(
        Arc::new(snapshot.select_roots(&[response.recipe]).unwrap()),
        response.recipe,
    )
    .unwrap();
    assert_eq!(response_program.bindings().len(), 2);
    assert_eq!(response_program.bindings()["0:2"].tool().uuid, parser.tool);
    let response_json = json!({"chunks":[],"output":{"assistant_reply":{"content":"ü\n{{vars.x}}"}},"effective_model_profile_id":"selected_model"}).to_string();
    assert_eq!(
        response_program
            .inputs()
            .bind_variant_example(
                "parse assistant model response json=%",
                &format!("parse assistant model response json={response_json}"),
                &json!({})
            )
            .unwrap(),
        json!({"response_json":response_json})
    );
    assert!(
        response_program
            .inputs()
            .bind_task_inputs(&json!({"prompt_json":response_json}))
            .is_err()
    );
    let answer_selection = Arc::new(snapshot.select_roots(&[answer_recipe]).unwrap());
    assert!(compile(answer_selection.clone(), answer_recipe).is_err());
    let answer_program = prepare_retained_tool_program(
        compile_retained_recipe(
            answer_selection,
            answer_recipe,
            "selected",
            WorkflowClass::RequiresModel,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(answer_program.bindings().len(), 4);
    assert_eq!(
        answer_program.bindings()["0:2"].tool(),
        answer_program.bindings()["0:6"].tool()
    );
    assert_eq!(
        answer_program.bindings()["0:6"].python().uuid,
        response.python_code
    );
    assert!(
        !answer_program.bindings().contains_key("0:7"),
        "pure R03 extraction has no artificial Tool binding"
    );
    assert_eq!(
        answer_program.bindings()["0:9"].capability_id(),
        "host.post_reply"
    );
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    let inspected_model =
        InspectedRetainedProgram::inspect(RetainedProgram::Tools(Arc::new(program)), &worker)
            .await
            .unwrap();
    assert!(
        inspected_model
            .source_checks()
            .contains_key(&model.python_code)
    );
    assert!(
        inspected_model
            .source_checks()
            .contains_key(&parser.python_code)
    );
    let inspected_parser = InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(Arc::new(parser_program)),
        &worker,
    )
    .await
    .unwrap();
    assert!(
        inspected_parser
            .source_checks()
            .contains_key(&parser.formatter.unwrap())
    );
    for (selected, export) in [
        (response_program, response.python_code),
        (answer_program, projection),
    ] {
        let inspected =
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(Arc::new(selected)), &worker)
                .await
                .unwrap();
        assert!(inspected.source_checks().contains_key(&export));
    }
    let intents: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        intents, 0,
        "source inspection creates no effect or model invocation"
    );
    assert!(model_completion_usage_drafts(model, parser, model).is_err());
    for collision in [
        projection,
        parser.tool,
        model.python_code,
        response.formatter.unwrap(),
        Uuid::nil(),
    ] {
        assert!(
            model_answer_recipe_draft(collision, model, parser, response, projection, reply)
                .is_err()
        );
    }
    assert!(
        model_answer_recipe_draft(
            answer_recipe,
            model,
            parser,
            UsageComponentIds {
                tool: model.tool,
                ..response
            },
            projection,
            reply
        )
        .is_err()
    );
    assert!(
        model_answer_recipe_draft(
            answer_recipe,
            model,
            parser,
            UsageComponentIds {
                tool: Uuid::new_v4(),
                ..response
            },
            projection,
            reply
        )
        .is_err()
    );
    assert!(
        model_response_json_usage_drafts(
            UsageComponentIds {
                formatter: None,
                ..response
            },
            reply
        )
        .is_err()
    );
    assert!(model_completion_tool_draft(model.tool, "not-a-package-checksum").is_err());
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
    let code_document = json!({"content":"result = host.post_reply(answer=inputs['answer'])", "includes":[], "dependency_registry":null, "input_contract":inputs, "result_contract":result});
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
    for (includes, reason) in [
        (json!(null), "PythonCode includes must be an array"),
        (json!({}), "PythonCode includes must be an array"),
        (
            json!([Uuid::new_v4()]),
            "nested PythonCode requires explicit retained assembly",
        ),
    ] {
        let mut document = code_document.clone();
        document["includes"] = includes;
        changes.push((code, 22, document, vec![], reason));
    }
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
