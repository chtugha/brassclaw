//! Actual retained PostgreSQL/IBS -> global Monty -> child -> kernel Tool path.
//! These unapproved drafts exercise behavioral validation, not activation or
//! ordinary application boot. No provider or Tool success is manufactured.
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use brassclaw_authorization::LiveStableToolPolicy;
use brassclaw_engine::{
    executor::retained_recipe::{
        RetainedExecutionError, RetainedProgram, RetainedRecipeExecution, RetainedStepFailure,
        RetainedToolInvocation, RetainedToolPort, RetainedTransportEvidence, port_answer_checksum,
        typed_value_checksum,
    },
    executor::retained_source::{InspectedRetainedProgram, RetainedSourceError},
    memory::retained_tools::{RetainedToolBinding, RetainedToolProgram},
};
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, MountView, ResourceEstimate,
    RuntimeKind, TrustClass, UserId,
};
use brassclaw_host_runtime::{
    RetainedFirstPartyCapability, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
    RuntimeFailureKind,
};
use brassclaw_monty_host::{
    VmFailure,
    global::Lifecycle,
    process::{
        PortAnswer, ProcessBoundary, ProcessFailure, ProcessSnapshot, RecipeBoundary, RecipeEvent,
        TaskHandle, WorkerCommand,
    },
    transport_actor::{ActorLimits, TransportClient, TransportOwner},
    utility::UtilityRequest,
};
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "support/admission.rs"]
mod admission;
#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;
#[path = "../../../crates/brassclaw_reborn_composition/src/pg_monty_admission.rs"]
mod pg_monty_admission;
#[path = "support/retained_kernel.rs"]
mod retained_kernel;
#[path = "support/retained_program.rs"]
mod retained_program;
#[path = "support/runtime.rs"]
mod support;

struct KernelPort {
    runtime: Arc<RetainedFirstPartyCapability>,
    policy: Arc<LiveStableToolPolicy>,
    deny_second: bool,
    tool: Uuid,
    calls: Mutex<usize>,
    task: TaskHandle,
    admitted: Arc<admission::Admitted>,
    prepared: Arc<RetainedToolProgram>,
    cancel_before_record: bool,
    outcomes: Mutex<Vec<&'static str>>,
}
impl KernelPort {
    fn new(
        deny_second: bool,
        tool: Uuid,
        task: TaskHandle,
        admitted: Arc<admission::Admitted>,
        prepared: Arc<RetainedToolProgram>,
        cancel_before_record: bool,
    ) -> Self {
        let (runtime, policy) = retained_kernel::runtime(tool);
        Self {
            runtime,
            policy,
            deny_second,
            tool,
            calls: Mutex::new(0),
            task,
            admitted,
            prepared,
            cancel_before_record,
            outcomes: Mutex::new(Vec::new()),
        }
    }
}
#[async_trait]
impl RetainedToolPort for KernelPort {
    async fn dispatch(
        &self,
        invocation: RetainedToolInvocation<'_>,
        binding: &RetainedToolBinding,
        arguments: Value,
    ) -> PortAnswer {
        assert_eq!(invocation.task(), self.task);
        let selected = &self.prepared.bindings()[invocation.step_id()];
        assert_eq!(selected.tool(), binding.tool());
        assert_eq!(selected.python(), binding.python());
        let record = self
            .admitted
            .admission
            .begin_tool_invocation(&self.prepared, invocation.step_id(), &arguments)
            .await
            .unwrap();
        assert_eq!(binding.capability_id(), "builtin.json");
        assert_eq!(binding.association().callable(), "host.json");
        assert_eq!(
            self.policy
                .tool_identity(&CapabilityId::new(binding.capability_id()).unwrap())
                .unwrap(),
            Some(binding.tool().uuid)
        );
        let count = {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            *calls
        };
        if self.deny_second && count == 2 {
            self.policy
                .publish(1, retained_kernel::snapshot(2, false, self.tool))
                .unwrap();
        }
        let context = ExecutionContext::local_default(
            UserId::new("draft-operator").unwrap(),
            ExtensionId::new("draft-caller").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .unwrap();
        let outcome = self
            .runtime
            .invoke(RuntimeCapabilityRequest::new(
                context,
                CapabilityId::new(binding.capability_id()).unwrap(),
                ResourceEstimate::default(),
                arguments,
                retained_kernel::trust(),
            ))
            .await
            .unwrap();
        let answer = match outcome {
            RuntimeCapabilityOutcome::Completed(completed) => {
                self.outcomes.lock().unwrap().push("completed");
                PortAnswer::Return {
                    value: completed.output,
                }
            }
            RuntimeCapabilityOutcome::Failed(failure) => {
                assert_eq!(failure.kind, RuntimeFailureKind::Authorization);
                self.outcomes.lock().unwrap().push("authorization");
                PortAnswer::TerminalError {
                    reason_kind: "tool_policy_denied".into(),
                }
            }
            other => panic!("unexpected real kernel outcome: {other:?}"),
        };
        if self.cancel_before_record {
            use brassclaw_turns::{
                CancelRunRequest, DefaultTurnCoordinator, IdempotencyKey, SanitizedCancelReason,
                TurnCoordinator,
            };
            DefaultTurnCoordinator::new(self.admitted.state.clone())
                .cancel_run(CancelRunRequest {
                    scope: self.admitted.context.scope.clone(),
                    actor: self.admitted.context.actor.clone().unwrap(),
                    run_id: self.admitted.context.run_id,
                    reason: SanitizedCancelReason::Policy,
                    idempotency_key: IdempotencyKey::new("late-invocation-answer").unwrap(),
                })
                .await
                .unwrap();
        }
        record.record_answer(&answer).await.unwrap();
        record.record_answer(&answer).await.unwrap(); // exact repetition is idempotent
        // A changed answer cannot replace the retained real result.
        let rejected = PortAnswer::TerminalError {
            reason_kind: "forged_answer".into(),
        };
        assert!(record.record_answer(&rejected).await.is_err());
        answer
    }
}

async fn exchange(transport: &TransportClient, command: WorkerCommand) -> ProcessSnapshot {
    transport
        .try_submit(command)
        .unwrap()
        .wait()
        .await
        .unwrap()
        .outcome
        .unwrap()
}
async fn progress(transport: &TransportClient, mut state: ProcessSnapshot) -> ProcessSnapshot {
    while let Some(ProcessBoundary::ControlYield { key }) = state.boundary {
        state = exchange(transport, WorkerCommand::ResumeControl { key }).await;
    }
    state
}

#[tokio::test]
async fn retained_steps_use_real_kernel_policy_and_keep_success_before_output_failure() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    for invalid_output in [false, true] {
        let prepared = retained_program::program(&store, invalid_output).await;
        assert_eq!(prepared.bindings()["0:2"].combination().len(), 4);
        assert!(std::ptr::eq(
            prepared.bindings()["0:2"].combination(),
            prepared.bindings()["0:4"].combination()
        ));
        let data = json!({"text":"'quotes'\n Ü {{vars.data}} host.forbidden()"});
        let inputs = json!({"data":data.to_string()});
        let admitted_fixture =
            Arc::new(admission::reserve(rig.pool.clone(), inputs["data"].as_str().unwrap()).await);
        let definitions =
            include_str!("../../../crates/brassclaw_engine/orchestrator/global_mode.py")
                .strip_suffix("asyncio.run(_global_main())\n")
                .unwrap();
        let source = format!(
            "{definitions}\nasync def validate_draft():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    value = await _execute_recipe(task['task_token'], 'retained-draft', '0:1-0:E', {{'user_input': task['user_input']}})\n    await host.validation_result(task['task_token'], value)\nasyncio.run(validate_draft())\n"
        );
        let mut boot = support::boot(&source);
        // This validation entry has one coroutine/initial work wait. The
        // production helper's normal entry creates its configured worker set.
        boot.bounds.workers = 1;
        boot.aliases.insert("validation_result".into());
        let (mut owner, ready) = TransportOwner::start(
            support::worker(),
            boot,
            support::limits(),
            ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * 1024 * 1024,
                max_control_unclaimed: 8,
                max_control_reserved_frame_bytes: 4 * 1024 * 1024,
            },
        )
        .await
        .unwrap();
        let transport = owner.client();
        let admitted = exchange(
            &transport,
            WorkerCommand::Admit {
                key: ready.work_waits[0].1,
                task: admitted_fixture.input.clone(),
            },
        )
        .await;
        let task = admitted.admitted_task.unwrap();
        let port = KernelPort::new(
            !invalid_output,
            prepared.bindings()["0:2"].tool().uuid,
            task,
            admitted_fixture.clone(),
            prepared.clone(),
            invalid_output,
        );
        let root = progress(&transport, admitted).await;
        let Some(ProcessBoundary::HostCall {
            key,
            name,
            args,
            kwargs,
        }) = root.boundary
        else {
            panic!("composition request required")
        };
        assert_eq!(name, "compose_orchestrator");
        assert_eq!(args.len(), 4);
        assert!(kwargs.is_empty());
        assert_eq!(args[1], "retained-draft");
        assert_eq!(args[2], "0:1-0:E");
        let token = args[0].clone();
        let reference = Uuid::new_v4().to_string();
        admitted_fixture
            .admission
            .retain_recipe_selection(prepared.inputs().instruction())
            .await
            .unwrap();
        exchange(&transport, WorkerCommand::Defer { key }).await;
        let root = exchange(&transport, WorkerCommand::Resolve { key, answer: PortAnswer::Return { value: json!({"ok":true,"program_ref":reference,
            "steps":prepared.program().steplist.iter().map(|s| json!({"step_id":s.step_id})).collect::<Vec<_>>(),"inputs":inputs,"flow":prepared.inputs().monty_flow().unwrap()}) } }).await;
        let mut root = progress(&transport, root).await;
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect(
                RetainedProgram::Tools(prepared.clone()),
                support::worker(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(inspected.source_checks().len(), 1);
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected).unwrap();
        assert!(execution.observations().is_empty());
        for step in &prepared.program().steplist {
            let Some(ProcessBoundary::HostCall {
                key,
                name,
                args,
                kwargs,
            }) = root.boundary
            else {
                panic!("step request required")
            };
            assert_eq!(name, "run_program");
            assert_eq!(args.len(), 4);
            assert!(kwargs.is_empty());
            assert_eq!(args[0], token);
            assert_eq!(args[1], reference);
            assert_eq!(args[2], step.step_id);
            exchange(&transport, WorkerCommand::Defer { key }).await;
            let result = execution
                .run_step(&transport, &step.step_id, &args[3]["inputs"], Some(&port))
                .await;
            let observation = &execution.observations()[&step.step_id];
            assert!(observation.settled());
            assert!(observation.observation_error().is_none());
            assert_eq!(
                observation.input_checksum(),
                typed_value_checksum(&args[3]["inputs"]).unwrap()
            );
            assert_eq!(
                observation.arguments_checksum(),
                Some(
                    typed_value_checksum(&json!({"data":inputs["data"],"operation":"parse"}))
                        .unwrap()
                )
            );
            assert_eq!(
                observation.answer_checksum(),
                Some(port_answer_checksum(&execution.host_answers().last().unwrap().1).unwrap())
            );
            if invalid_output || step.step_id == "0:4" {
                assert!(observation.result_checksum().is_none());
                assert!(
                    observation.failure()
                        == Some(if invalid_output {
                            RetainedStepFailure::ResultContract
                        } else {
                            RetainedStepFailure::Process
                        })
                );
                if invalid_output {
                    assert!(matches!(result, Err(RetainedExecutionError::Inputs(_))));
                    let Some(RecipeEvent::Progress {
                        boundary: RecipeBoundary::Complete { value },
                        ..
                    }) = execution.latest_snapshot().unwrap().recipe.as_ref()
                    else {
                        panic!("actual invalid result retained")
                    };
                    assert_eq!(value, "invalid output");
                    assert_eq!(*port.outcomes.lock().unwrap(), ["completed"]);
                } else {
                    let Err(RetainedExecutionError::Process(error)) = &result else {
                        panic!("actual worker failure required")
                    };
                    let Some(RetainedTransportEvidence::Process {
                        error: retained,
                        command,
                        ..
                    }) = execution.transport_failure()
                    else {
                        panic!("private transport failure must survive port error conversion")
                    };
                    assert!(Arc::ptr_eq(error, retained));
                    assert!(matches!(
                        command.as_ref().unwrap().as_ref(),
                        WorkerCommand::Recipe {
                            command: brassclaw_monty_host::process::RecipeCommand::ResumeHost { .. }
                        }
                    ));
                    assert!(retained.snapshot.is_some());
                    assert_eq!(
                        *port.outcomes.lock().unwrap(),
                        ["completed", "authorization"]
                    );
                }
                assert!(
                    execution
                        .run_step(&transport, &step.step_id, &args[3]["inputs"], Some(&port))
                        .await
                        .is_err()
                );
                let PortAnswer::Return { value } = &execution.host_answers()[0].1 else {
                    panic!("real success retained")
                };
                assert_eq!(value, &data);
                assert_eq!(
                    *port.calls.lock().unwrap(),
                    if invalid_output { 1 } else { 2 }
                );
                // Feed the observed child failure back to the actual waiting
                // root. This validation entry deliberately has no task error
                // handler, so its uncaught failure is an instance failure.
                let receipt = transport
                    .try_submit(WorkerCommand::Resolve {
                        key,
                        answer: PortAnswer::DomainError {
                            reason_kind: "recipe_execution_failed".into(),
                        },
                    })
                    .unwrap()
                    .wait()
                    .await
                    .unwrap();
                assert!(receipt.transport_started);
                let failure = receipt.outcome.unwrap_err();
                assert_eq!(failure.kind, ProcessFailure::Vm(VmFailure::Python));
                assert_eq!(failure.snapshot.unwrap().lifecycle, Lifecycle::Failed);
                break;
            }
            let value = result.unwrap();
            assert!(observation.failure().is_none());
            assert_eq!(
                observation.result_checksum(),
                Some(typed_value_checksum(&value).unwrap())
            );
            assert_eq!(value, data);
            let resolved = exchange(
                &transport,
                WorkerCommand::Resolve {
                    key,
                    answer: PortAnswer::Return {
                        value: json!({"ok":true,"return_value":value}),
                    },
                },
            )
            .await;
            root = progress(&transport, resolved).await;
        }
        let rows = rig
            .pool
            .get()
            .await
            .unwrap()
            .query(
                "SELECT step_id,attempt_count,phase,selection_bytes,arguments_bytes,answer_bytes
             FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id",
                &[&admitted_fixture.context.run_id.as_uuid()],
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), if invalid_output { 1 } else { 2 });
        for row in &rows {
            assert_eq!(row.get::<_, i16>(1), 1);
            assert_eq!(row.get::<_, &str>(2), "answered");
            let selection: Value = serde_json::from_str(row.get(3)).unwrap();
            assert_eq!(
                selection["recipe"]["uuid"],
                prepared.inputs().instruction().recipe().uuid.to_string()
            );
            assert_eq!(selection["variant_key"], "selected");
            assert_eq!(selection["components"].as_array().unwrap().len(), 5);
            let arguments: Value = serde_json::from_str(row.get(4)).unwrap();
            assert_eq!(arguments["data"], inputs["data"]);
        }
        let answer: Value = serde_json::from_str(rows[0].get(5)).unwrap();
        assert_eq!(answer, json!({"kind":"return","value":data}));
        assert!(
            admitted_fixture
                .admission
                .begin_tool_invocation(
                    &prepared,
                    "0:2",
                    &json!({"data":inputs["data"],"operation":"parse"})
                )
                .await
                .is_err(),
            "completed/cancelled invocation cannot replay outside the Rust executor either"
        );
        if invalid_output {
            assert!(
                admitted_fixture
                    .admission
                    .begin_tool_invocation(
                        &prepared,
                        "0:4",
                        &json!({"data":inputs["data"],"operation":"parse"})
                    )
                    .await
                    .is_err(),
                "cancelled claim cannot record intent for a new effect"
            );
        } else {
            let answer: Value = serde_json::from_str(rows[1].get(5)).unwrap();
            assert_eq!(
                answer,
                json!({"kind":"terminal_error","reason_kind":"tool_policy_denied"})
            );
        }
        // Failure never gets a fabricated product completion or effect-free
        // receipt. Retain actual evidence and reap the worker explicitly.
        owner.request_termination();
        let exit = owner.join().await.unwrap();
        assert!(exit.exit_status.is_some());
        assert!(exit.containment_error.is_none());
        assert!(exit.reap_error.is_none());
        admitted_fixture
            .admission
            .settle(json!({"status":"failed","reason_kind":"recipe_execution_failed"}))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn retained_source_preflight_rejects_unbound_code_before_any_execution() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let valid = "# host.forbidden() import os\ntext = 'host.fake() eval(1) __execute_action__'\nresult = host.json(operation='parse', data=inputs['data'])";
    let program = retained_program::program_with_source(&store, false, Some(valid)).await;
    let inspected = InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(program.clone()),
        support::worker(),
    )
    .await
    .unwrap();
    // One actual observation is shared by both selected uses of this revision.
    assert_eq!(inspected.source_checks().len(), 1);
    let reference = program.inputs().components()["0:2"];
    let observed = &inspected.source_checks()[&reference.uuid];
    assert_eq!(observed.component(), reference);
    assert_eq!(observed.observations().direct_host_calls.len(), 1);
    assert!(observed.observations().imports.is_empty());
    assert!(observed.observations().reserved_name_references.is_empty());
    for source in [
        "result = host.unbound(data=inputs['data'])",
        "receiver = host\nresult = receiver.json(data=inputs['data'])",
        "first = host.json(data=inputs['data'])\nresult = host.json(data=inputs['data'])",
        "result = __execute_action__('json')",
        "import os\nresult = host.json(data=inputs['data'])",
        "value = host.json(data=inputs['data'])",
        "result = {'text': inputs['data']}",
    ] {
        let program = retained_program::program_with_source(&store, false, Some(source)).await;
        let error = match InspectedRetainedProgram::inspect(
            RetainedProgram::Tools(program),
            support::worker(),
        )
        .await
        {
            Ok(_) => panic!("unsupported selected source must fail preflight"),
            Err(error) => error,
        };
        assert!(matches!(error, RetainedSourceError::Invalid { .. }));
        assert!(!format!("{error:?}").contains(source));
    }
    let program = retained_program::program_with_source(&store, false, Some("def broken(:")).await;
    let error =
        match InspectedRetainedProgram::inspect(RetainedProgram::Tools(program), support::worker())
            .await
        {
            Ok(_) => panic!("malformed source must fail contained compilation"),
            Err(error) => error,
        };
    let RetainedSourceError::Inspection { error, .. } = error else {
        panic!("actual worker error evidence required");
    };
    assert_eq!(error.kind, ProcessFailure::Vm(VmFailure::Python));
    assert!(error.exit_status.unwrap().success());
    let UtilityRequest::InspectSource { source, .. } = error.request.as_ref() else {
        panic!("original rejected source required");
    };
    assert_eq!(source, "def broken(:");
}
