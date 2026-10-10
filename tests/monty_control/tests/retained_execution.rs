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

#[tokio::test]
async fn blocked_dispatch_intent_allows_cancel_and_rolls_back_before_effects() {
    use brassclaw_turns::{
        CancelRunRequest, DefaultTurnCoordinator, IdempotencyKey, SanitizedCancelReason,
        TurnCoordinator, TurnStatus,
    };
    use std::time::Duration;

    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = retained_program::program(&store, false).await;
    let admitted = Arc::new(admission::reserve(rig.pool.clone(), "cancel held intent").await);
    admitted
        .admission
        .retain_recipe_selection(program.inputs().instruction())
        .await
        .unwrap();
    let mut lock_client = rig.pool.get().await.unwrap();
    let lock = lock_client.transaction().await.unwrap();
    lock.batch_execute("LOCK TABLE brassclaw_monty_tool_invocations IN SHARE MODE")
        .await
        .unwrap();
    let writing = tokio::spawn({
        let admission = admitted.admission.clone();
        let program = program.clone();
        async move {
            admission
                .begin_tool_invocation(&program, "0:2", &json!({"data":"{\"text\":\"held\"}"}))
                .await
        }
    });
    let observation = rig.pool.get().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = observation
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' \
                     AND query LIKE '%INSERT INTO brassclaw_monty_tool_invocations%' \
                     AND pid<>pg_backend_pid())",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let cancelled = tokio::time::timeout(
        Duration::from_millis(500),
        DefaultTurnCoordinator::new(admitted.state.clone()).cancel_run(CancelRunRequest {
            scope: admitted.context.scope.clone(),
            actor: admitted.context.actor.clone().unwrap(),
            run_id: admitted.context.run_id,
            reason: SanitizedCancelReason::UserRequested,
            idempotency_key: IdempotencyKey::new("cancel-blocked-dispatch-intent").unwrap(),
        }),
    )
    .await
    .expect("effect-free journal preparation must not lock out cancellation")
    .unwrap();
    assert_eq!(cancelled.status, TurnStatus::CancelRequested);
    assert!(!writing.is_finished());
    lock.rollback().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), writing)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    let intents: i64 = observation
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(intents, 0);
    // The earlier exact selection remains; cancellation cannot mutate it, and
    // another call under the stale attempt cannot create an intent or effect.
    assert!(
        admitted
            .admission
            .begin_tool_invocation(&program, "0:2", &json!({"data":"{}"}))
            .await
            .is_err()
    );
    let selections: i64 = observation
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_recipe_selections",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(selections, 1);
}

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
#[path = "support/retained_program_variants.rs"]
mod retained_program_variants;
use retained_program_variants::{program_with_preload_source, program_with_source};

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
    async fn new(
        deny_second: bool,
        tool: Uuid,
        task: TaskHandle,
        admitted: Arc<admission::Admitted>,
        prepared: Arc<RetainedToolProgram>,
        cancel_before_record: bool,
    ) -> Self {
        let (runtime, policy) = retained_kernel::runtime(&prepared, "0:2").await;
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
        let prepared = if invalid_output {
            retained_program::program(&store, true).await
        } else {
            // Exercise an actual caught exception before both retained Tool
            // occurrences. The local alias must not become a dependency, and
            // the error's text remains typed data passed to the JSON usage.
            program_with_preload_source(
                &store,
                "def parse_usage(inputs):\n    try:\n        raise ValueError(inputs['data'])\n    except ValueError as error:\n        data = str(error)\n    return _parse_data({'data': data})",
                None,
            )
            .await
        };
        assert_eq!(prepared.bindings()["0:2"].combination().len(), 5);
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
        )
        .await;
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
        assert_eq!(inspected.source_checks().len(), 2);
        assert_eq!(inspected.preload_order().len(), 2);
        assert_eq!(
            inspected.invocation("0:2"),
            Some("result = parse_usage(inputs=inputs)")
        );
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected).unwrap();
        assert!(execution.observations().is_empty());
        assert!(!execution.is_complete());
        // Reject an out-of-order requested feed before allocating a child or
        // recording an invocation; the original first occurrence remains usable.
        assert!(
            execution
                .run_step(
                    &transport,
                    &prepared.program().steplist[1].step_id,
                    &json!({}),
                    Some(&port)
                )
                .await
                .is_err()
        );
        assert!(execution.completed_step_ids().next().is_none());
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
            assert_eq!(
                execution.completed_step_ids().last(),
                Some(step.step_id.as_str())
            );
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
            assert_eq!(selection["components"].as_array().unwrap().len(), 6);
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
    let program = program_with_source(&store, false, Some(valid)).await;
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
        let program = program_with_source(&store, false, Some(source)).await;
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
    let program = program_with_source(&store, false, Some("def broken(:")).await;
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

#[tokio::test]
async fn recipe_children_share_one_task_parent_with_explicit_result_handoff() {
    use brassclaw_engine::executor::retained_recipe::RetainedTaskContext;
    use brassclaw_monty_host::process::RecipeCommand;

    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let first = retained_program::program(&store, false).await;
    let second = retained_program::program(&store, false).await;
    let expected = json!({"text":"'quotes'\n Ü {{vars.data}} host.forbidden()"});
    let first_value = json!({"text":expected.to_string()});
    let initial = json!({"data":first_value.to_string()});
    let admitted_fixture =
        Arc::new(admission::reserve(rig.pool.clone(), initial["data"].as_str().unwrap()).await);
    let definitions = include_str!("../../../crates/brassclaw_engine/orchestrator/global_mode.py")
        .strip_suffix("asyncio.run(_global_main())\n")
        .unwrap();
    // Actual Monty sequences both constrained draft workflows. Rust responds
    // only to the requested composition/step; runtime values remain data.
    let source = format!(
        "{definitions}\nasync def validate_children():\n    task = await host.await_next_task(0)\n    host.enter_task(task['task_token'])\n    first = await _execute_recipe(task['task_token'], 'first', '0:1-0:E', {{'data': task['user_input']}})\n    second = await _execute_recipe(task['task_token'], 'second', '0:1-0:E', {{'data': first['text']}})\n    await host.validation_result(task['task_token'], second)\nasyncio.run(validate_children())\n"
    );
    let mut boot = support::boot(&source);
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
    let mut root = progress(&transport, admitted).await;
    let mut parent_owner = RetainedTaskContext::new(task);
    let mut opening = Box::pin(parent_owner.open(&transport));
    assert!(futures::poll!(opening.as_mut()).is_pending());
    drop(opening);
    let pending = transport.completions().outstanding().unwrap();
    assert_eq!(pending.len(), 1);
    let parent = parent_owner.open(&transport).await.unwrap();
    let original_request = parent_owner.receipt().unwrap().id;
    assert_eq!(pending, vec![original_request]);
    assert_eq!(parent_owner.open(&transport).await.unwrap(), parent);
    assert_eq!(parent_owner.receipt().unwrap().id, original_request);
    for (name, prepared, inputs, value) in [
        ("first", first.clone(), initial, first_value),
        (
            "second",
            second.clone(),
            json!({"data":expected.to_string()}),
            expected.clone(),
        ),
    ] {
        let Some(ProcessBoundary::HostCall {
            key,
            name: operation,
            args,
            kwargs,
        }) = root.boundary
        else {
            panic!("Monty composition request required")
        };
        assert_eq!(operation, "compose_orchestrator");
        assert!(kwargs.is_empty());
        assert_eq!(args[1], name);
        assert_eq!(args[3], inputs);
        admitted_fixture
            .admission
            .retain_recipe_selection(prepared.inputs().instruction())
            .await
            .unwrap();
        exchange(&transport, WorkerCommand::Defer { key }).await;
        let reference = Uuid::new_v4().to_string();
        root = progress(&transport, exchange(&transport, WorkerCommand::Resolve { key, answer: PortAnswer::Return {
            value: json!({"ok":true,"program_ref":reference,
            "steps":prepared.program().steplist.iter().map(|s| json!({"step_id":s.step_id})).collect::<Vec<_>>(),
            "inputs":inputs,"flow":prepared.inputs().monty_flow().unwrap()})
        }}).await).await;
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect(
                RetainedProgram::Tools(prepared.clone()),
                support::worker(),
            )
            .await
            .unwrap(),
        );
        let mut child = RetainedRecipeExecution::new_child(task, parent, inspected).unwrap();
        let port = KernelPort::new(
            false,
            prepared.bindings()["0:2"].tool().uuid,
            task,
            admitted_fixture.clone(),
            prepared.clone(),
            false,
        )
        .await;
        for step in &prepared.program().steplist {
            let Some(ProcessBoundary::HostCall {
                key,
                name,
                args,
                kwargs,
            }) = root.boundary
            else {
                panic!("Monty child step request required")
            };
            assert_eq!(name, "run_program");
            assert_eq!(args[1], reference);
            assert_eq!(args[2], step.step_id);
            assert!(kwargs.is_empty());
            exchange(&transport, WorkerCommand::Defer { key }).await;
            let actual = child
                .run_step(&transport, &step.step_id, &args[3]["inputs"], Some(&port))
                .await
                .unwrap();
            assert_eq!(actual, value);
            root = progress(
                &transport,
                exchange(
                    &transport,
                    WorkerCommand::Resolve {
                        key,
                        answer: PortAnswer::Return {
                            value: json!({"ok":true,"return_value":actual}),
                        },
                    },
                )
                .await,
            )
            .await;
        }
        assert_eq!(*port.calls.lock().unwrap(), 2);
        assert_eq!(child.host_answers().len(), 2);
    }
    let Some(ProcessBoundary::HostCall {
        key, name, args, ..
    }) = root.boundary
    else {
        panic!("actual draft validation result required")
    };
    assert_eq!(name, "validation_result");
    assert_eq!(args[1], expected);
    exchange(&transport, WorkerCommand::Defer { key }).await;
    let released = exchange(
        &transport,
        WorkerCommand::Recipe {
            command: RecipeCommand::CancelContext { context: parent },
        },
    )
    .await;
    let Some(RecipeEvent::Released {
        task: None,
        contexts,
    }) = released.recipe
    else {
        panic!("actual task context release required")
    };
    assert_eq!(released.lifecycle, Lifecycle::Ready);
    assert_eq!(
        contexts.len(),
        3,
        "one parent and two explicit Recipe children"
    );
    // Local child release cannot reset the still-active root task account or
    // turn this draft validation into product completion.
    let premature = transport
        .try_submit(WorkerCommand::Recipe {
            command: RecipeCommand::CloseTask { task },
        })
        .unwrap()
        .wait()
        .await
        .unwrap()
        .outcome
        .unwrap_err();
    assert_eq!(premature.kind, ProcessFailure::Vm(VmFailure::WrongBoundary));
    assert_eq!(premature.snapshot.unwrap().task_accounting.len(), 1);
    let client = rig.pool.get().await.unwrap();
    let rows = client.query("SELECT recipe_id,attempt_count,phase FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
        &[&admitted_fixture.context.run_id.as_uuid()]).await.unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let recipe: Uuid = row.get(0);
        assert!(
            [
                first.inputs().instruction().recipe().uuid,
                second.inputs().instruction().recipe().uuid
            ]
            .contains(&recipe)
        );
        assert_eq!(row.get::<_, i16>(1), 1);
        assert_eq!(row.get::<_, &str>(2), "answered");
    }
    drop(client);
    // This constrained validator has no product reply/finish contract. Its
    // observed child results never fabricate whole-task completion or approval.
    owner.request_termination();
    let exit = owner.join().await.unwrap();
    assert!(exit.exit_status.is_some());
    assert!(exit.containment_error.is_none());
    assert!(exit.reap_error.is_none());
}

#[tokio::test]
async fn selected_export_preflight_rejects_unsafe_libraries_before_execution() {
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    for source in [
        "effect = host.json(data='{}')\ndef parse_usage(inputs):\n    return _parse_data(inputs)",
        "def parse_usage(inputs=host.json(data='{}')):\n    return _parse_data(inputs)",
        "def parse_usage(inputs):\n    return _missing_dependency(inputs)",
        "def parse_usage(inputs):\n    first = _parse_data(inputs)\n    return host.json(data=inputs['data'], operation='parse')",
        "def parse_usage(inputs):\n    first = _parse_data(inputs)\n    return _parse_data(inputs)",
        "def parse_usage(inputs):\n    for item in [inputs]:\n        _parse_data(item)\n    return _parse_data(inputs)",
        "def parse_usage(inputs):\n    alias = _parse_data\n    return alias(inputs)",
        "def parse_usage(inputs):\n    call = lambda: host.json(operation='parse', data=inputs['data'])\n    return call()",
        "def parse_usage(inputs):\n    values = [_parse_data(item) for item in [inputs, inputs]]\n    return values[0]",
        "def parse_usage(inputs):\n    values = [(lambda item: _parse_data(item))(item) for item in [inputs, inputs]]\n    return values[0]",
        "def parse_usage(inputs):\n    values = {(lambda item: _parse_data(item))(item)['text'] for item in [inputs, inputs]}\n    return {'text': next(iter(values))}",
        "def parse_usage(inputs):\n    values = {index: (lambda item: _parse_data(item))(item) for index, item in enumerate([inputs, inputs])}\n    return values[0]",
        "def parse_usage(inputs):\n    values = list((lambda item: _parse_data(item))(item) for item in [inputs, inputs])\n    return values[0]",
        "def parse_usage(inputs):\n    values = [[(lambda item: _parse_data(item))(item) for item in group] for group in [[inputs, inputs]]]\n    return values[0][0]",
        "STATE = []\ndef parse_usage(inputs):\n    STATE.append(inputs['data'])\n    return _parse_data(inputs)",
        "def parse_usage(inputs):\n    global STATE\n    STATE = inputs\n    return _parse_data(inputs)",
    ] {
        let program = program_with_preload_source(&store, source, None).await;
        assert!(matches!(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(program), support::worker())
                .await,
            Err(RetainedSourceError::Invalid { .. })
        ));
    }
    let client = rig.pool.get().await.unwrap();
    let count: i64 = client
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count, 0,
        "invalid libraries must not record a dispatch intent or invoke a Tool"
    );
}
