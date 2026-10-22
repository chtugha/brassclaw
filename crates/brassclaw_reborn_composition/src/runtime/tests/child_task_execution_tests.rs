//! A closed native child-usage observation with a real worker parent, immutable
//! intent journal, claimed child coordinator and ordinary global Monty child.
//! This does not approve or activate the candidate parent usage/Recipes.
use super::*;

#[path = "postgres_child_commit_transport.rs"]
mod child_commit_transport;
use child_commit_transport::{ChildCommitTransport, ChildWrite};
use secrecy::ExposeSecret;

use crate::RebornRuntime;
use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
};
use brassclaw_host_api::{
    CapabilitySet, ExecutionContext, ExtensionId, MountView, ResourceEstimate, RuntimeKind,
    TrustClass,
};
use brassclaw_host_runtime::{RuntimeCapabilityOutcome, RuntimeCapabilityRequest};
use brassclaw_monty_host::process::PortAnswer;
use brassclaw_product_workflow::MontyVmSettingsStore;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use brassclaw_turns::CancelRunRequest;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[path = "child_parent_recipe_tests.rs"]
mod parent_recipe_tests;

enum ChildCandidateShape {
    NativeCalls(usize),
    ParentRecipe,
}

async fn child_usage_candidate(
    pool: Arc<brassclaw_pg::PgPool>,
    identity: brassclaw_host_runtime::NativeImplementationRef,
    shape: ChildCandidateShape,
) -> Arc<RetainedToolProgram> {
    let (recipe, tool, binding, code, skill) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let text = json!({"type":"string","required":true,"checks":[]});
    let workflow = json!({"type":"object","required":true,"checks":[],"allow_extra_fields":false,
        "fields":{"recipe_id":text,"variant":text}});
    let mut optional_text = text.clone();
    optional_text["required"] = json!(false);
    let inputs = json!({"workflow":workflow,"inputs":{"type":"object","required":true,"checks":[],
        "allow_extra_fields":false,"fields":{"answer":text,
        "user_input":optional_text,"reply_ref":optional_text}}});
    let result = json!({"type":"object","checks":[],"allow_extra_fields":false,
        "fields":{"status":text,"child_run_id":text,"thread_id":text,
        "reply_ref":optional_text,"content":optional_text,
        "failure":{"type":"object","required":false,"nullable":true,"checks":[],
        "allow_extra_fields":false,"fields":{"category":text}}}});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed",
        "idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let exports =
        json!({"delegate":{"symbol":"delegate_child","parameters":["inputs"],"mapping":true}});
    let association = json!({"format":"skill-association/1","skill_uuid":skill,"python_code_uuid":code,
        "tool_skill_uuid":binding,"tool_uuid":tool,"callable":"host.spawn_subagent","inputs":inputs,
        "arguments":{"workflow":"workflow","inputs":"inputs"},"code_arguments":{},"result":result,"failure":failure}).to_string();
    let (repetitions, completion) = match shape {
        ChildCandidateShape::NativeCalls(count) => (count, None),
        ChildCandidateShape::ParentRecipe => (1, Some(Uuid::new_v4())),
    };
    let mut steps = Vec::new();
    let mut layout = serde_json::Map::new();
    for n in 0..repetitions {
        steps.push(json!({"stepnumber":n*2+1,"knowledge":"rust","goal":"Bind one native child operation","content":"",
            "type":"component","include":[binding],"tool_bindings":[{"tool_id":tool,"tool_name":"spawn_subagent",
            "params":{},"error_policy":{"policy":"fail"}}]}));
        steps.push(json!({"stepnumber":n*2+2,"knowledge":"orchestrator","goal":"Admit and observe one selected child",
            "content":"","type":"component","include":[code]}));
        layout.insert(
            format!("0:{}", n * 2 + 2),
            json!({
                "workflow":{"kind":"task_input","reference":"{{vars.workflow}}"},
                "inputs":{"kind":"task_input","reference":"{{vars.inputs}}"},
            }),
        );
    }
    if let Some(completion) = completion {
        steps.push(json!({"stepnumber":3,"knowledge":"orchestrator","goal":"Require the actual child result and return its private review receipt",
            "content":"","type":"component","include":[completion]}));
        layout.insert(
            "0:3".into(),
            json!({"child":{"kind":"result","step_id":"0:2","path":[]}}),
        );
    }
    let mut dependencies = vec![binding, code, skill];
    dependencies.extend(completion);
    let mut documents = vec![
        (
            tool,
            0,
            json!({"capability_id":"builtin.spawn_subagent","callable":"host.spawn_subagent",
            "input_contract":inputs,"implementation":{"identity":identity}}),
            Vec::new(),
            Value::Null,
        ),
        (
            binding,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":tool,
            "callable":"host.spawn_subagent","capability_id":"builtin.spawn_subagent"}}),
            vec![tool],
            Value::Null,
        ),
        (
            code,
            22,
            json!({"content":"def delegate_child(inputs):\n    return host.spawn_subagent(workflow=inputs['workflow'], inputs=inputs['inputs'])",
            "input_contract":inputs,"result_contract":result,"preload":{"format":"python-preload/2",
            "exports":exports,"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"delegate"}}),
            Vec::new(),
            Value::Null,
        ),
        (
            skill,
            1,
            json!({"body":"One explicit native child workflow admission and correlated terminal observation; no authorization receipt is completion.",
            "interface":{"format":"skill-interface/1","python_code_uuid":code,"exports":exports,"inputs":inputs,"result":result,
            "failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![tool, binding, code],
            json!(association),
        ),
        (
            recipe,
            21,
            json!({"variants":[{"variant_key":"native-child-candidate","step_link":"0:1-0:E",
            "intent_examples":["candidate child % %"],"variable_patterns":[{"name":"workflow","pattern":null,"description":null},{"name":"inputs","pattern":null,"description":null}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Closed native candidate","yaml_source":"","steps":steps}],
            "input_layouts":{"native-child-candidate":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":layout}}}),
            dependencies,
            Value::Null,
        ),
    ];
    if let Some(completion) = completion {
        let mut child_result = result;
        child_result["required"] = json!(true);
        documents.push((completion, 22, json!({
            "content":"def complete_child_review(inputs):\n    child = inputs['child']\n    if child['status'] != 'completed':\n        raise RuntimeError('child_review_failed')\n    return 'child-review:' + child['child_run_id']",
            "input_contract":{"child":child_result},"result_contract":{"type":"string","checks":[]},
            "preload":{"format":"python-preload/2","exports":{"complete":{"symbol":"complete_child_review","parameters":["inputs"],"mapping":true}},
                "private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"complete"}
        }), Vec::new(), Value::Null));
    }
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut references = Vec::new();
    for (uuid, class_code, document, dependencies, association) in documents {
        let draft = ComponentRevisionDraft::from_json(
            &json!({"format":"component-revision/1","uuid":uuid,"class_code":class_code,
            "document":document,"dependencies":dependencies,"association":association})
            .to_string(),
        )
        .unwrap();
        references.push(store.stage(&draft, 0).await.unwrap());
    }
    pool.get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1)",
            &[&tool],
        )
        .await
        .unwrap();
    let snapshot = Arc::new(store.read_exact(&[recipe], &references).await.unwrap());
    Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                snapshot,
                recipe,
                "native-child-candidate",
                WorkflowClass::RequiresModel,
            )
            .unwrap(),
        )
        .unwrap(),
    )
}

fn child_execution_context(host: &MontyTaskHost) -> ExecutionContext {
    let context = host.run_context();
    let mut execution = ExecutionContext::local_default(
        context.actor.as_ref().unwrap().user_id.clone(),
        ExtensionId::new("native-child-candidate").unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::FirstParty,
        CapabilitySet::default(),
        MountView::default(),
    )
    .unwrap();
    execution.tenant_id = context.scope.tenant_id.clone();
    execution.agent_id = context.scope.agent_id.clone();
    execution.project_id = context.scope.project_id.clone();
    execution.thread_id = Some(context.thread_id.clone());
    execution.resource_scope.tenant_id = execution.tenant_id.clone();
    execution.resource_scope.agent_id = execution.agent_id.clone();
    execution.resource_scope.project_id = execution.project_id.clone();
    execution.resource_scope.thread_id = execution.thread_id.clone();
    execution
}

/// Execute one draft export with the existing contained behavioral runner. This
/// is component review, not another production orchestrator or an activation.
/// The actual admitted child still runs in the instance's global Monty service.
async fn run_child_export_for_review(
    runtime: &RebornRuntime,
    host: Arc<MontyTaskHost>,
    admission: Arc<crate::pg_monty_admission::PgMontyAdmission>,
    program: Arc<RetainedToolProgram>,
    supplied: &Value,
    step: &str,
) -> Value {
    use brassclaw_engine::executor::{
        retained_recipe::{RetainedProgram, RetainedRecipeExecution},
        retained_source::InspectedRetainedProgram,
    };
    use brassclaw_monty_host::{
        global::GlobalBounds,
        process::{ProcessLimits, RootBoot, TaskSettings, installed_worker},
        utility::UtilityLimits,
    };
    use sha2::{Digest, Sha256};

    let settings = runtime
        .monty_settings_owner
        .store()
        .get("default", "default")
        .await
        .unwrap();
    let client = runtime.global_monty_owner.client();
    let live = client.live_task_settings().current();
    assert_eq!(settings.revision, live.revision);
    let memory = client.worker_memory_status();
    let worker = installed_worker().unwrap();
    let inspected = Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(program.clone()),
            &worker,
            UtilityLimits::from_execution(settings.execution_limits).unwrap(),
        )
        .await
        .unwrap(),
    );
    let tools = runtime
        .installed_catalogue
        .child_usage_tools_for_test(host.clone(), admission, program.clone())
        .unwrap();
    let context = host.run_context();
    let input = crate::monty_task_input::load_monty_task_input(
        runtime.thread_service.as_ref(),
        &context.scope,
        context.accepted_message_ref.as_ref(),
        context.turn_id,
        context.run_id,
    )
    .await
    .unwrap();
    assert!(
        input.prior_context.messages.is_empty(),
        "this fixture has no prior turns"
    );
    let task_input = json!({
        "conversation_id":context.thread_id, "message_id":input.message.message_id,
        "turn_id":context.turn_id, "run_id":context.run_id,
        "user_input":input.message.content.unwrap(), "history":[],
    });
    let source = crate::bootstrap_reply_validation::ROOT;
    let boot = RootBoot {
        adapter_reserve_bytes: memory.allocator.adapter_reserve_bytes,
        heap_settings: Some(client.heap_observation().status.effective.unwrap()),
        source: source.into(),
        checksum: Sha256::digest(source.as_bytes()).into(),
        aliases: ["await_next_task", "enter_task", "validation_hold"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        bounds: GlobalBounds {
            values: client.vm_bounds(),
            workers: 1,
            max_pending_calls: usize::try_from(client.pending_call_capacity().limit).unwrap(),
        },
        startup_timeout: Duration::from_millis(settings.execution_limits.startup_timeout_millis),
        task_settings: TaskSettings {
            revision: live.revision,
            max_compute_time: live.limits.max_compute_time,
            token_budgets_enabled: live.limits.token_budgets_enabled,
        },
        max_recipe_contexts: settings.execution_limits.max_recipe_contexts,
    };
    let (mut owner, transport, _, task) = crate::bootstrap_validation::start(
        &worker,
        boot,
        ProcessLimits {
            hard_memory_bytes: memory.allocator.memory_budget_bytes,
            max_frame_bytes: memory.frame.effective_capacity_bytes,
            response_timeout: Duration::from_millis(
                settings.execution_limits.response_timeout_millis,
            ),
        },
        crate::live_monty_settings::actor_limits(settings.execution_limits).unwrap(),
        task_input,
    )
    .await
    .unwrap();
    let input = program.inputs().bind_task_inputs(supplied).unwrap();
    let mut execution =
        RetainedRecipeExecution::new_for_behavioral_validation(task, inspected).unwrap();
    let result = execution
        .run_step(&transport, step, &input, Some(tools.as_ref()))
        .await
        .unwrap();
    let observation = execution.observations().get(step).unwrap();
    assert!(observation.settled() && observation.observation_error().is_none());
    assert_eq!(
        execution.is_complete(),
        program.bindings().len() == 1,
        "only the single-usage candidate can complete after one export"
    );
    owner.request_termination();
    let exit = owner.join().await.unwrap();
    assert!(
        exit.exit_status.is_some() && exit.containment_error.is_none() && exit.reap_error.is_none()
    );
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_child_uses_claimed_parent_old_catalogue_and_same_global_vm() {
    let rig = super::super::test_pg::pg_rig().await;
    let _database = rig.lock_db().await;
    let directory = tempfile::tempdir().unwrap();
    let transport = ChildCommitTransport::start(rig.url.expose_secret(), ChildWrite::rules())
        .await
        .unwrap();
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                crate::RebornBuildInput::postgres_with_reborn_home(
                    "native-child-owner",
                    (*transport.pool).clone(),
                    brassclaw_secrets::SecretMaterial::from(
                        transport.connection_string().to_owned(),
                    ),
                    directory.path().to_path_buf(),
                )
                .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_poll_settings(PollSettings {
                interval: Duration::from_millis(10),
                max_total: Duration::from_secs(60),
            }),
        ))
        .await
        .unwrap(),
    );
    let catalogue = runtime.installed_catalogue.clone();
    let settings = runtime.monty_settings_owner.store();
    let before = settings.get("default", "default").await.unwrap();
    let mut limits = before.execution_limits;
    // Completion must wake the real waiter even with a deliberately slow
    // fallback. This edits the actual acknowledged settings owner.
    limits.status_poll_interval_millis = 30_000;
    settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(json!({
                "expected_revision":before.revision,"execution_limits":limits,
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    let original = catalogue.active.read().await.clone();
    let root_identity = runtime.global_monty_owner.client().root_identity();
    let captured = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *catalogue.capture_gate.lock().await = Some((captured.clone(), release.clone()));
    let conversation = runtime.new_conversation().await.unwrap();
    let correlation = Uuid::new_v4();
    let run = TurnRunId::from_uuid(correlation);
    let running = runtime.clone();
    let chat = conversation.clone();
    let parent = tokio::spawn(async move {
        running
            .send_correlated_user_message(
                &chat,
                "publish literal reply parent kept separate",
                correlation,
                CancellationToken::new(),
            )
            .await
            .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(30), captured.notified())
        .await
        .unwrap();
    // Pause only this actual parent's transcript mutation. A new child thread
    // remains writable, and the original parent VM has a real pending Tool.
    let mut blocker = rig.pool.get().await.unwrap();
    let transaction = blocker.transaction().await.unwrap();
    transaction
        .query_one(
            "SELECT id FROM brassclaw_session_threads WHERE id=$1 FOR UPDATE",
            &[&conversation.0.as_str()],
        )
        .await
        .unwrap();
    release.notify_one();
    let (host, admission) = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(host) = runtime
                .monty_test_controls
                .0
                .retained_host_for_run(run)
                .unwrap()
                && let Ok(admission) = catalogue.retained_child_parent_for_test(&host)
                && admission.check_live_dependency().await.is_ok()
            {
                break (host, admission);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(host.claimed_run().is_some());
    // A genuine coherent publication changes latest while this parent is
    // already bound. The child must retain the original generation.
    let replacement = catalogue.activate_authored("native-child-owner",serde_json::from_value(json!({
        "activation_id":Uuid::new_v4(),"expected_generation":original.generation,"recipes":[],
    })).unwrap()).await.unwrap();
    assert_ne!(replacement.generation, original.generation.to_string());
    let capability = CapabilityId::new("builtin.spawn_subagent").unwrap();
    let ordinary = catalogue.kernel.builtin(&capability, None).unwrap();
    let program = child_usage_candidate(
        rig.pool.clone(),
        ordinary.native_identity().unwrap(),
        ChildCandidateShape::NativeCalls(5),
    )
    .await;
    let failed_program = child_usage_candidate(
        rig.pool.clone(),
        ordinary.native_identity().unwrap(),
        ChildCandidateShape::NativeCalls(1),
    )
    .await;
    let binding = &program.bindings()["0:2"];
    let policy = Arc::new(crate::monty_kernel::PgSelectedToolPolicy {
        host: host.clone(),
        pool: rig.pool.clone(),
        tool: binding.tool().uuid,
        capability,
        effects: ordinary.descriptor().effects.clone(),
        mounts: MountView::default(),
    });
    let native = catalogue
        .kernel
        .child_with_policy(
            ordinary.native_identity().unwrap(),
            catalogue.child_task_port(host.clone(), admission.clone()),
            policy,
        )
        .unwrap();
    let workflow = catalogue.reply.program().inputs().instruction();
    let literal = "child ' ü🙂 {{vars.goal}}\r\n```python\nraise RuntimeError('data')\n```\nnull\n";
    let mut child_run = None;
    for (step, valid, cancel, internal, reject_release, block_reply) in [
        ("0:2", true, false, false, false, false),
        ("0:4", false, false, false, false, false),
        ("0:6", true, true, false, false, false),
        ("0:8", false, false, true, false, false),
        ("0:10", true, false, false, true, false),
        ("0:12", true, false, false, false, true),
    ] {
        let active_program = if block_reply {
            &failed_program
        } else {
            &program
        };
        let export_step = if block_reply { "0:2" } else { step };
        let selected_workflow = if internal {
            catalogue.history.program().inputs().instruction()
        } else {
            workflow
        };
        let inputs = if internal {
            json!({"user_input":"must not append history from a child goal", "answer":literal,
                "reply_ref":"msg:not-a-published-child-reply"})
        } else {
            json!({"answer":if valid {json!(literal)} else {Value::Null}})
        };
        // The helper's actual recursive inputs are valid. Rejection must
        // happen because it is internal finalization, before any admission or
        // history effect, rather than after executing it as an ordinary child.
        if internal {
            catalogue
                .history
                .program()
                .inputs()
                .bind_task_inputs_with_limits(
                    &inputs,
                    brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
                )
                .unwrap();
        }
        let arguments = json!({"workflow":{"recipe_id":selected_workflow.recipe().uuid,
            "variant":selected_workflow.variant().variant_key}, "inputs":inputs});
        let execution = (!matches!(step, "0:2" | "0:12")).then(|| child_execution_context(&host));
        let manual_invocation = execution.as_ref().map(|context| context.invocation_id);
        let record = if let Some(invocation) = manual_invocation {
            Some(
                admission
                    .begin_tool_invocation(
                        invocation,
                        active_program,
                        export_step,
                        &arguments,
                        brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
                    )
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        let barriers = valid.then(|| catalogue.pause_next_child_for_test());
        let interrupted_write = match step {
            "0:2" => Some(ChildWrite::AdmitRun),
            "0:6" => Some(ChildWrite::EnsureThread),
            "0:10" => Some(ChildWrite::AcceptInput),
            _ => None,
        };
        let cuts_before = transport.cut_count();
        if let Some(phase) = interrupted_write {
            transport.arm(phase as u8).unwrap();
        }
        let native_call = tokio::time::timeout(Duration::from_secs(20), async {
            let Some(execution) = execution else {
                return Ok(run_child_export_for_review(
                    runtime.as_ref(),
                    host.clone(),
                    admission.clone(),
                    active_program.clone(),
                    &arguments,
                    export_step,
                )
                .await);
            };
            match host
                .invoke_retained_tool(
                    &native,
                    RuntimeCapabilityRequest::new(
                        execution,
                        native.descriptor().id.clone(),
                        ResourceEstimate::default(),
                        arguments,
                        TrustDecision {
                            effective_trust: EffectiveTrustClass::user_trusted(),
                            authority_ceiling: AuthorityCeiling {
                                allowed_effects: native.descriptor().effects.clone(),
                                max_resource_ceiling: None,
                            },
                            provenance: TrustProvenance::AdminConfig,
                            evaluated_at: Utc::now(),
                        },
                    ),
                )
                .await
                .unwrap()
            {
                RuntimeCapabilityOutcome::Completed(done) => Ok(done.output),
                RuntimeCapabilityOutcome::Failed(failure) => Err(failure),
                unexpected => panic!("unexpected actual native outcome: {}", unexpected.kind()),
            }
        });
        let release_child = async {
            if let Some((captured, waiting, release)) = barriers {
                tokio::time::timeout(Duration::from_secs(15), captured.notified())
                    .await
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(15), waiting.notified())
                    .await
                    .unwrap();
                if reject_release {
                    // Reject the real aggregate write only when this child's
                    // reservation marker changes. Its Recipe/reply commits
                    // normally; no effect or terminal result is simulated.
                    let invocation = manual_invocation.unwrap();
                    let source = format!("monty-child-v1:{invocation}");
                    rig.pool.get().await.unwrap().batch_execute(&format!(
                        "CREATE FUNCTION reject_child_reservation_release() RETURNS trigger LANGUAGE plpgsql AS $$
                         BEGIN
                           IF EXISTS(SELECT 1 FROM jsonb_array_elements(NEW.payload->'runs') AS r
                             WHERE r->>'source_binding_ref'='{source}'
                               AND r->>'claimed_child_reservation_released'='true') THEN
                             RAISE EXCEPTION 'deliberate child reservation rejection';
                           END IF;
                           RETURN NEW;
                         END $$;
                         CREATE TRIGGER reject_child_reservation_release BEFORE INSERT OR UPDATE
                         ON brassclaw_turns FOR EACH ROW EXECUTE FUNCTION reject_child_reservation_release();"
                    )).await.unwrap();
                }
                if block_reply {
                    // Change the actual live instance policy after the child
                    // captured its workflow, before its first real dispatch.
                    // The parent is already waiting on its original reply.
                    let brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(reply) =
                        catalogue.reply.program()
                    else {
                        panic!("real reply Tool binding")
                    };
                    let reply_tool = reply.bindings()["0:2"].tool().uuid;
                    assert_eq!(rig.pool.get().await.unwrap().execute(
                        "UPDATE brassclaw_instance_tool_settings SET enabled=false,revision=revision+1 WHERE tool_id=$1 AND enabled=true",
                        &[&reply_tool],
                    ).await.unwrap(), 1);
                }
                if cancel {
                    let invocation = manual_invocation.unwrap();
                    // Cancel the actual admitted/claimed child while its
                    // capture is paused before any Recipe effect. The parent
                    // remains live and must receive a classified cancellation,
                    // never an empty reply or another child attempt.
                    let state = brassclaw_turns::PgTurnStateStore::new(
                        rig.pool.clone(),
                        host.run_context().scope.tenant_id.as_str(),
                    );
                    let mut children = brassclaw_turns::TurnSpawnTreeStateStore::children_of(
                        &state,
                        &host.run_context().scope,
                        run,
                    )
                    .await
                    .unwrap()
                    .into_iter()
                    .filter(|child| {
                        child.source_binding_ref.as_str() == format!("monty-child-v1:{invocation}")
                    });
                    let child = children.next().unwrap();
                    assert!(children.next().is_none());
                    assert!(!child.status.is_terminal());
                    runtime
                        .turn_coordinator
                        .cancel_tree(CancelRunRequest {
                            scope: child.scope,
                            actor: host.run_context().actor.clone().unwrap(),
                            run_id: child.run_id,
                            reason: SanitizedCancelReason::UserRequested,
                            idempotency_key: IdempotencyKey::new("native-child-cancellation")
                                .unwrap(),
                        })
                        .await
                        .unwrap();
                }
                release.notify_one();
            }
        };
        let (outcome, ()) = tokio::join!(native_call, release_child);
        let outcome = outcome.unwrap();
        assert_eq!(
            transport.cut_count(),
            cuts_before + usize::from(interrupted_write.is_some()),
            "each selected write must lose its actual database acknowledgement once"
        );
        let invocation = if let Some(invocation) = manual_invocation {
            invocation
        } else {
            let id = rig.pool.get().await.unwrap().query_one(
                "SELECT invocation_id FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND recipe_id=$2 AND step_id=$3",
                &[&correlation, &active_program.inputs().instruction().recipe().uuid, &export_step],
            ).await.unwrap().get::<_, Uuid>(0);
            brassclaw_host_api::InvocationId::from_uuid(id)
        };
        let answer = match outcome {
            Ok(output) if valid && !cancel && !reject_release && !block_reply => {
                assert_eq!(output["status"], "completed");
                assert_eq!(output["content"], literal);
                let id = Uuid::parse_str(output["child_run_id"].as_str().unwrap()).unwrap();
                child_run = Some(id);
                let client = rig.pool.get().await.unwrap();
                let row = client.query_one("SELECT catalogue_id,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
                    &[&id]).await.unwrap();
                assert_eq!(row.get::<_, Uuid>(0), original.generation);
                let evidence: Value = row.get(1);
                assert_eq!(evidence["execution"]["intent_outcome"], "child_recipe");
                assert_eq!(
                    evidence["execution"]["reply"]["reference"],
                    output["reply_ref"]
                );
                let state = brassclaw_turns::PgTurnStateStore::new(
                    rig.pool.clone(),
                    host.run_context().scope.tenant_id.as_str(),
                );
                let child_scope = brassclaw_turns::TurnScope {
                    thread_id: ThreadId::new(output["thread_id"].as_str().unwrap()).unwrap(),
                    ..host.run_context().scope.clone()
                };
                let actual = brassclaw_turns::TurnSpawnTreeStateStore::get_run_record(
                    &state,
                    &child_scope,
                    TurnRunId::from_uuid(id),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(actual.parent_run_id, Some(run));
                assert_eq!(actual.spawn_tree_root_run_id, Some(run));
                assert_eq!(actual.subagent_depth, 1);
                assert_eq!(actual.claimed_child_reservation_released, Some(true));
                assert_eq!(
                    actual.source_binding_ref.as_str(),
                    format!("monty-child-v1:{invocation}")
                );
                assert_eq!(
                    runtime.global_monty_owner.client().root_identity(),
                    root_identity
                );
                PortAnswer::Return {
                    value: output.into(),
                }
            }
            Ok(output) if block_reply => {
                assert_eq!(output["status"], "failed");
                assert!(output.get("reply_ref").is_none());
                assert!(output.get("content").is_none());
                let id = Uuid::parse_str(output["child_run_id"].as_str().unwrap()).unwrap();
                let scope = brassclaw_turns::TurnScope {
                    thread_id: ThreadId::new(output["thread_id"].as_str().unwrap()).unwrap(),
                    ..host.run_context().scope.clone()
                };
                let state = brassclaw_turns::PgTurnStateStore::new(
                    rig.pool.clone(),
                    scope.tenant_id.as_str(),
                );
                let child = brassclaw_turns::TurnSpawnTreeStateStore::get_run_record(
                    &state,
                    &scope,
                    TurnRunId::from_uuid(id),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(child.status, TurnStatus::Failed);
                assert_eq!(child.claim_count, 1);
                assert_eq!(child.claimed_child_reservation_released, Some(true));
                assert_eq!(child.parent_run_id, Some(run));
                assert_eq!(
                    output["failure"],
                    serde_json::to_value(child.failure).unwrap()
                );
                assert!(output["failure"]["category"].as_str().is_some());
                let client = rig.pool.get().await.unwrap();
                let evidence: Value = client.query_one(
                    "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
                    &[&id],
                ).await.unwrap().get(0);
                assert_eq!(evidence["status"], "failed");
                assert_eq!(evidence["execution"]["intent_outcome"], "child_recipe");
                assert_eq!(
                    evidence["execution"]["all_selected_recipes_complete"],
                    false
                );
                assert!(evidence["execution"]["reply"].is_null());
                assert_eq!(evidence["execution"]["model_exchange_count"], 0);
                assert_eq!(client.query_one(
                    "SELECT count(*)::bigint FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                    &[&id],
                ).await.unwrap().get::<_, i64>(0), 1, "one denied dispatch intent, no retry");
                let reply = runtime
                    .thread_service
                    .finalized_assistant_message_by_run(
                        brassclaw_threads::FinalizedAssistantMessageByRunRequest {
                            scope: brassclaw_threads::ThreadScope {
                                tenant_id: scope.tenant_id.clone(),
                                agent_id: scope.agent_id.clone().unwrap(),
                                project_id: scope.project_id.clone(),
                                owner_user_id: scope.explicit_owner_user_id().cloned(),
                            },
                            thread_id: scope.thread_id,
                            turn_run_id: id.to_string(),
                        },
                    )
                    .await
                    .unwrap();
                assert!(reply.is_none(), "policy denial cannot produce a reply");
                let brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(reply) =
                    catalogue.reply.program()
                else {
                    panic!("real reply Tool binding")
                };
                assert_eq!(client.execute(
                    "UPDATE brassclaw_instance_tool_settings SET enabled=true,revision=revision+1 WHERE tool_id=$1 AND enabled=false",
                    &[&reply.bindings()["0:2"].tool().uuid],
                ).await.unwrap(), 1);
                assert_eq!(
                    runtime.global_monty_owner.client().root_identity(),
                    root_identity
                );
                PortAnswer::Return {
                    value: output.into(),
                }
            }
            Ok(output) if cancel => {
                assert_eq!(output["status"], "cancelled");
                assert!(output.get("reply_ref").is_none());
                assert!(output.get("content").is_none());
                let id = Uuid::parse_str(output["child_run_id"].as_str().unwrap()).unwrap();
                assert_ne!(Some(id), child_run);
                let state = brassclaw_turns::PgTurnStateStore::new(
                    rig.pool.clone(),
                    host.run_context().scope.tenant_id.as_str(),
                );
                let child_scope = brassclaw_turns::TurnScope {
                    thread_id: ThreadId::new(output["thread_id"].as_str().unwrap()).unwrap(),
                    ..host.run_context().scope.clone()
                };
                let actual = brassclaw_turns::TurnSpawnTreeStateStore::get_run_record(
                    &state,
                    &child_scope,
                    TurnRunId::from_uuid(id),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(actual.status, TurnStatus::Cancelled);
                assert_eq!(actual.claimed_child_reservation_released, Some(true));
                assert_eq!(actual.claim_count, 1);
                let client = rig.pool.get().await.unwrap();
                let observation = client
                    .query_one(
                        "SELECT outcome,started_at,\
                         (SELECT count(*)::bigint FROM brassclaw_monty_tool_invocations WHERE run_id=$1) \
                         FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
                        &[&id],
                    )
                    .await
                    .unwrap();
                let evidence: Value = observation.get(0);
                assert_eq!(evidence["status"], "failed");
                assert_eq!(evidence["reason_kind"], "task_cancelled");
                assert!(
                    observation
                        .get::<_, Option<chrono::DateTime<Utc>>>(1)
                        .is_none()
                );
                assert_eq!(observation.get::<_, i64>(2), 0);
                assert_eq!(
                    runtime.global_monty_owner.client().root_identity(),
                    root_identity
                );
                assert!(host.check_dispatch_open().is_ok());
                PortAnswer::Return {
                    value: output.into(),
                }
            }
            Err(_) if !valid => PortAnswer::TerminalError {
                reason_kind: "invalid_child_input".into(),
            },
            Err(_) if reject_release => {
                let state = brassclaw_turns::PgTurnStateStore::new(
                    rig.pool.clone(),
                    host.run_context().scope.tenant_id.as_str(),
                );
                let children = brassclaw_turns::TurnSpawnTreeStateStore::children_of(
                    &state,
                    &host.run_context().scope,
                    run,
                )
                .await
                .unwrap();
                let child = children
                    .iter()
                    .find(|child| {
                        child.source_binding_ref.as_str() == format!("monty-child-v1:{invocation}")
                    })
                    .unwrap();
                assert_eq!(child.status, TurnStatus::Completed);
                assert_eq!(child.claimed_child_reservation_released, Some(false));
                assert_eq!(child.claim_count, 1);
                use crate::global_task_factory::MontyCatalogueProvider;
                assert!(
                    catalogue
                        .reconcile_received_child_results(host.attempt())
                        .await
                        .is_err()
                );
                rig.pool
                    .get()
                    .await
                    .unwrap()
                    .batch_execute(
                        "DROP TRIGGER reject_child_reservation_release ON brassclaw_turns;
                     DROP FUNCTION reject_child_reservation_release();",
                    )
                    .await
                    .unwrap();
                // This acknowledges the received observation, not another Tool
                // call or child admission. Repetition cannot decrement a sibling.
                catalogue
                    .reconcile_received_child_results(host.attempt())
                    .await
                    .unwrap();
                catalogue
                    .reconcile_received_child_results(host.attempt())
                    .await
                    .unwrap();
                let confirmed = brassclaw_turns::TurnSpawnTreeStateStore::get_run_record(
                    &state,
                    &child.scope,
                    child.run_id,
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(confirmed.claimed_child_reservation_released, Some(true));
                assert_eq!(confirmed.claim_count, 1);
                let client = rig.pool.get().await.unwrap();
                let evidence: Value = client.query_one(
                    "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
                    &[&child.run_id.as_uuid()],
                ).await.unwrap().get(0);
                assert_eq!(evidence["status"], "completed");
                assert_eq!(evidence["execution"]["intent_outcome"], "child_recipe");
                let message = runtime
                    .thread_service
                    .finalized_assistant_message_by_run(
                        brassclaw_threads::FinalizedAssistantMessageByRunRequest {
                            scope: brassclaw_threads::ThreadScope {
                                tenant_id: child.scope.tenant_id.clone(),
                                agent_id: child.scope.agent_id.clone().unwrap(),
                                project_id: child.scope.project_id.clone(),
                                owner_user_id: child.scope.explicit_owner_user_id().cloned(),
                            },
                            thread_id: child.scope.thread_id.clone(),
                            turn_run_id: child.run_id.to_string(),
                        },
                    )
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(message.content.as_deref(), Some(literal));
                assert_eq!(client.query_one(
                    "SELECT count(*)::bigint FROM brassclaw_monty_tool_invocations \
                     WHERE run_id=$1 AND selection_bytes::jsonb->>'capability_id'='host.post_reply'",
                    &[&child.run_id.as_uuid()],
                ).await.unwrap().get::<_, i64>(0), 1);
                PortAnswer::TerminalError {
                    reason_kind: "retained_tool_failed".into(),
                }
            }
            unexpected => panic!("unexpected actual child native result: {unexpected:?}"),
        };
        if let Some(record) = record {
            record.record_answer(&answer).await.unwrap();
        } else {
            // The preloaded export traversed the actual SelectedTools adapter;
            // this test must not manufacture its journal acknowledgement.
            let row = rig.pool.get().await.unwrap().query_one(
                "SELECT answer_bytes,attempt_count::bigint FROM brassclaw_monty_tool_invocations WHERE invocation_id=$1 AND phase='answered'",
                &[&invocation.as_uuid()],
            ).await.unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&row.get::<_, String>(0)).unwrap(),
                serde_json::to_value(&answer).unwrap()
            );
            assert_eq!(row.get::<_, i64>(1), 1);
        }
        if reject_release {
            let row = rig.pool.get().await.unwrap().query_one(
                "SELECT answer_bytes,attempt_count::bigint FROM brassclaw_monty_tool_invocations WHERE invocation_id=$1",
                &[&invocation.as_uuid()],
            ).await.unwrap();
            let recorded: Value = serde_json::from_str(&row.get::<_, String>(0)).unwrap();
            assert_eq!(
                recorded,
                json!({"kind":"terminal_error", "reason_kind":"retained_tool_failed"})
            );
            assert_eq!(row.get::<_, i64>(1), 1);
        }
    }
    assert!(child_run.is_some());
    assert_eq!(
        catalogue.retained_child_operations_for_test(host.attempt()),
        4
    );
    let client = rig.pool.get().await.unwrap();
    assert_eq!(client.query_one("SELECT count(*)::bigint FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND selection_bytes::jsonb->>'capability_id'='builtin.spawn_subagent'",
        &[&correlation]).await.unwrap().get::<_,i64>(0),6);
    assert_eq!(client.query_one("SELECT count(*)::bigint FROM brassclaw_session_threads WHERE tenant_id=$1 AND id LIKE 'monty-child-v1-%'",
        &[&host.run_context().scope.tenant_id.as_str()]).await.unwrap().get::<_,i64>(0),4,"invalid input must create no additional child thread");
    transaction.commit().await.unwrap();
    let finished = tokio::time::timeout(Duration::from_secs(30), parent)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finished.status, TurnStatus::Completed);
    assert_eq!(finished.text.as_deref(), Some("parent kept separate"));
    assert_eq!(
        catalogue.retained_child_operations_for_test(host.attempt()),
        1,
        "completed/cancelled observations are freed; a failed child retains its original recovery owner"
    );
    assert!(admission.check_live_dependency().await.is_err());
    let rejected = host.invoke_retained_tool(
            &native,
            RuntimeCapabilityRequest::new(
                child_execution_context(&host),
                native.descriptor().id.clone(),
                ResourceEstimate::default(),
                json!({"workflow":{"recipe_id":workflow.recipe().uuid,"variant":workflow.variant().variant_key},"inputs":{"answer":"stale parent must not run"}}),
                TrustDecision {
                    effective_trust: EffectiveTrustClass::user_trusted(),
                    authority_ceiling: AuthorityCeiling {
                        allowed_effects: native.descriptor().effects.clone(),
                        max_resource_ceiling: None,
                    },
                    provenance: TrustProvenance::AdminConfig,
                    evaluated_at: Utc::now()
                }
            ),
        )
        .await;
    assert!(
        matches!(rejected, Err(_) | Ok(RuntimeCapabilityOutcome::Failed(_))),
        "a fenced parent cannot dispatch another child"
    );
    shutdown_shared_runtime(runtime).await.unwrap();
    transport.shutdown().await.unwrap();
}
