//! Development-only finite response consumer through the actual retained global
//! Recipe flow, JSON kernel and PostgreSQL journal. No model, reply, approval or
//! catalogue activation is manufactured by this observation.
use super::*;
use crate::{
    bootstrap_recipe_flow, bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
};
use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{RetainedProgram, RetainedRecipeExecution, RetainedToolPort},
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskOutcome},
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft,
    global_bootstrap_components::{UsageComponentIds, public_reply_drafts},
    model_completion_components::{model_answer_text_draft, model_response_json_usage_drafts},
    revision_store::PgComponentRevisionStore,
};
use brassclaw_turns::run_profile::{
    AssistantReply, LoopModelResponse, LoopModelUsage, ModelProfileId, ModelStreamChunk,
    ParentLoopOutput,
};
use serde_json::{Value, json};
use uuid::Uuid;

struct Invocation {
    reference: String,
    inputs: Value,
    expected: Value,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "development-model-response"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.host_answers().len() != 1
            || execution.transport_failure().is_some()
            || serde_json::from_str::<Value>(receipt).ok().as_ref() != Some(&self.expected)
        {
            return Err(PortFailure::new("response_receipt_invalid").unwrap());
        }
        Ok(())
    }
}

fn ids(tool: Uuid, formatter: bool) -> UsageComponentIds {
    UsageComponentIds {
        recipe: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        tool,
        skill: Uuid::new_v4(),
        formatter: formatter.then(Uuid::new_v4),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_model_response_parser_preserves_wire_data_and_stops_before_projection_on_failure() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(RebornRuntimeInput::from_services(
        rig.build_input("response-parser-owner", directory.path())
            .with_runtime_policy(local_dev_runtime_policy()),
    )))
    .await
    .unwrap();
    let catalogue = &runtime.installed_catalogue;
    let RetainedProgram::Tools(json_program) = catalogue._measurement_candidate.program() else {
        panic!("actual installed JSON selection required");
    };
    let json_tool = json_program.bindings()["0:5"].tool();
    let RetainedProgram::Tools(reply_program) = catalogue.reply.program() else {
        panic!("actual retained reply identity required");
    };
    let reply_tool = reply_program.bindings()["0:2"].tool();
    let parser = ids(json_tool.uuid, true);
    let reply = ids(reply_tool.uuid, false);
    let (root, projection, render) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let text = json!({"type":"string","required":true,"checks":[]});
    let renderer = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":render,"class_code":22,
        "document":{"content":"def render_response_observation(inputs):\n    import json\n    return json.dumps({'answer': inputs['answer']}, ensure_ascii=False)\n",
            "input_contract":{"answer":text},"result_contract":{"type":"string","checks":[]},
            "preload":{"format":"python-preload/2","exports":{"render_response_observation":{"symbol":"render_response_observation","parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],"imports":["json"],"dependencies":[],"default_export":"render_response_observation"}},
        "dependencies":[],"association":null}).to_string()).unwrap();
    // A private observation consumer reuses the exact packaged parser and R03.
    // The final pure renderer makes an empty answer a nonempty internal receipt;
    // it is neither a public reply nor a replacement model-response envelope.
    let consumer = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":root,"class_code":21,
        "document":{"variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["observe response %"],"variable_patterns":[{"name":"response_json","pattern":null,"description":null}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Private parser and projection observation","yaml_source":"","steps":[
                {"stepnumber":1,"knowledge":"rust","goal":"Bind one JSON usage","content":"","type":"component","include":[parser.tool_skill],"tool_bindings":[{"tool_id":parser.tool,"tool_name":"json","params":{},"error_policy":{"policy":"fail"}}]},
                {"stepnumber":2,"knowledge":"orchestrator","goal":"Validate complete response","content":"","type":"component","include":[parser.python_code]},
                {"stepnumber":3,"knowledge":"orchestrator","goal":"Extract exact answer text","content":"","type":"component","include":[projection]},
                {"stepnumber":4,"knowledge":"orchestrator","goal":"Render internal observation","content":"","type":"component","include":[render]}]}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":{"response_json":text},"steps":{
                "0:2":{"json_text":{"kind":"task_input","reference":"{{vars.response_json}}"}},
                "0:3":{"response":{"kind":"result","step_id":"0:2","path":[]}},
                "0:4":{"answer":{"kind":"result","step_id":"0:3","path":[]}}}}},
            "invocations":{"selected":{"0:2":"parse_assistant_model_response","0:3":"extract_model_answer","0:4":"render_response_observation"}}},
        "dependencies":[parser.skill,parser.tool_skill,parser.python_code,projection,render],"association":null}).to_string()).unwrap();
    let mut drafts = public_reply_drafts(reply).unwrap();
    drafts.extend(model_response_json_usage_drafts(parser, reply).unwrap());
    drafts.extend([
        model_answer_text_draft(projection).unwrap(),
        renderer,
        consumer,
    ]);
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let mut refs = vec![json_tool, reply_tool];
    for draft in drafts {
        refs.push(store.stage(&draft, 0).await.unwrap());
    }
    let all = store
        .read_exact(&[root, parser.recipe, reply.recipe], &refs)
        .await
        .unwrap();
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                Arc::new(all.select_roots(&[root]).unwrap()),
                root,
                "selected",
                WorkflowClass::Deterministic,
            )
            .unwrap(),
        )
        .unwrap(),
    );
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(program.clone()),
            &catalogue.worker,
        )
        .await
        .unwrap(),
    );
    let root_definition = crate::global_root_seed::retain_packaged_global_root(
        rig.pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .unwrap();
    let answer = "' ü🙂\n{{vars.answer}} % {\"ok\":false}\nraise RuntimeError('data')";
    let mut response = LoopModelResponse {
        chunks: vec![ModelStreamChunk {
            safe_text_delta: "independent chunk data".into(),
        }],
        safe_reasoning_deltas: vec![],
        output: ParentLoopOutput::AssistantReply(AssistantReply {
            content: answer.into(),
        }),
        effective_model_profile_id: ModelProfileId::new("selected_model").unwrap(),
        usage: None,
    };
    let wire = serde_json::to_value(&response).unwrap();
    response.output = ParentLoopOutput::AssistantReply(AssistantReply { content: "".into() });
    response.safe_reasoning_deltas = vec!["reasoning observation".into()];
    response.usage = Some(LoopModelUsage {
        input_tokens: 17,
        output_tokens: 0,
        cache_read_input_tokens: u32::MAX,
        cache_creation_input_tokens: 0,
    });
    let empty = serde_json::to_value(&response).unwrap();
    response.output = ParentLoopOutput::CapabilityCalls(vec![]);
    let proposals = serde_json::to_value(&response).unwrap();
    let mut missing = wire.clone();
    missing.as_object_mut().unwrap().remove("output");
    let mut null_usage = wire.clone();
    null_usage["usage"] = Value::Null;
    let mut extra = wire.clone();
    extra["unknown"] = json!(true);
    let evidence_before: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    for (value, expected, blocked) in [
        (wire.clone(), Some(answer), false),
        (empty, Some(""), false),
        (proposals, None, false),
        (missing, None, false),
        (null_usage, None, false),
        (extra, None, false),
        (wire.clone(), None, true),
    ] {
        // Change the real instance-wide rule after retaining/inspecting the
        // program. No per-call grant or alternate Tool is introduced.
        assert_eq!(rig.pool.get().await.unwrap().execute("UPDATE brassclaw_instance_tool_settings SET enabled=$2,revision=revision+1 WHERE tool_id=$1", &[&parser.tool, &!blocked]).await.unwrap(), 1);
        let original = value.to_string();
        let reference = format!("development-model-response:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            rig.pool.clone(),
            "response-parser",
            &reference,
        ))
        .await
        .unwrap();
        let tools = crate::installed_monty_catalogue::response_parser_candidate_tools(
            rig.pool.clone(),
            catalogue.kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
        )
        .unwrap();
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            bootstrap_recipe_flow::Request::contained(
                catalogue.worker.clone(),
                inspected.preparation_limits(),
                &root_definition,
                Arc::new(Invocation {
                    reference,
                    inputs: json!({"response_json":original}),
                    expected: json!({"answer":expected}),
                    inspected: inspected.clone(),
                    tools,
                }),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ),
            expected.is_some(),
            "{:?}",
            observed.receipt.outcome
        );
        assert_eq!(observed.report["model_exchange_count"], 0);
        assert!(observed.report["reply"].is_null());
        assert_eq!(observed.report["semantic_approval"], false);
        observed.ports.observe_internal_execution(|execution| {
            assert_eq!(execution.host_answers().len(), 1);
            if blocked {
                use brassclaw_engine::executor::retained_recipe::RetainedTransportEvidence;
                use brassclaw_monty_host::{VmFailure, global::Lifecycle, process::{ProcessFailure, RecipeCommand, WorkerCommand}};
                let Some(RetainedTransportEvidence::Process {error, command, ..}) = execution.transport_failure() else {
                    panic!("a denied native call must retain its actual worker error");
                };
                assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::Python));
                assert!(error.snapshot.as_ref().is_some_and(|snapshot| snapshot.lifecycle == Lifecycle::Ready && snapshot.root.as_ref().is_some_and(|root| root.vm_id() == observed.receipt.root.vm_id())));
                assert!(matches!(command.as_ref().unwrap().as_ref(), WorkerCommand::Recipe {command:RecipeCommand::ResumeHost {..}}));
            } else {
                assert!(execution.transport_failure().is_none(), "schema rejection is distinct from a worker/transport error");
                if expected.is_none() {
                    assert!(execution.observations()["0:2"].failure() == Some(brassclaw_engine::executor::retained_recipe::RetainedStepFailure::ResultContract));
                }
            }
            if let Some(answer) = expected {
                assert!(execution.is_complete());
                assert_eq!(execution.completed_step_ids().collect::<Vec<_>>(), ["0:2", "0:3", "0:4"]);
                assert!(execution.observations().values().all(|o| o.settled() && o.failure().is_none() && o.observation_error().is_none()));
                assert_eq!(execution.observations()["0:3"].result_checksum(), Some(brassclaw_engine::executor::retained_recipe::typed_value_checksum(&json!(answer)).unwrap()));
            } else {
                assert!(!execution.is_complete());
                assert_eq!(execution.failed_step_id(), Some("0:2"));
                assert!(execution.completed_step_ids().next().is_none());
                assert_eq!(execution.observations().len(), 1, "projection never begins on invalid/blocked response");
            }
            if blocked {
                assert!(matches!(&execution.host_answers()[0].1, PortAnswer::TerminalError { reason_kind } if reason_kind == "retained_tool_authorization"));
            } else {
                assert!(matches!(&execution.host_answers()[0].1, PortAnswer::Return { value: actual } if *actual == value));
            }
        }).await.unwrap();
        assert!(admitted.host.finalized_reply_ref().is_none());
        assert!(observed.exit.failure.is_none());
        let exit = observed.exit.transport.unwrap();
        assert!(
            exit.exit_status.is_some()
                && exit.containment_error.is_none()
                && exit.reap_error.is_none()
        );
        bootstrap_validation::settle_closed(&admitted)
            .await
            .unwrap();
        let rows = rig.pool.get().await.unwrap().query("SELECT step_id,attempt_count,phase,arguments_bytes,answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&admitted.attempt.run_id.as_uuid()]).await.unwrap();
        assert_eq!(
            rows.len(),
            1,
            "exactly one native parse, no retry or Tier-2 replay"
        );
        assert_eq!(rows[0].get::<_, String>(0), "0:2");
        assert_eq!(rows[0].get::<_, i16>(1), 1);
        assert_eq!(rows[0].get::<_, String>(2), "answered");
        assert_eq!(
            serde_json::from_str::<Value>(&rows[0].get::<_, String>(3)).unwrap(),
            json!({"operation":"parse","data":original})
        );
        let native: PortAnswer =
            serde_json::from_str(&rows[0].get::<_, Option<String>>(4).unwrap()).unwrap();
        if !blocked {
            assert!(matches!(native, PortAnswer::Return { value: actual } if actual == value));
        }
    }
    assert!(!wire.as_object().unwrap().contains_key("usage"));
    let evidence_after: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM reborn_component_review_evidence", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        evidence_before, evidence_after,
        "candidate execution never fabricates approval"
    );
    runtime.shutdown().await.unwrap();
}
