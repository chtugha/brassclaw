//! Actual retained PostgreSQL/IBS -> global Monty -> child -> kernel Tool path.
//! These unapproved drafts exercise behavioral validation, not activation or
//! ordinary application boot. No provider or Tool success is manufactured.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
    ToolExecutionRules,
};
use brassclaw_engine::{
    executor::retained_recipe::{
        RetainedExecutionError, RetainedProgram, RetainedRecipeExecution, RetainedToolPort,
        RetainedTransportEvidence,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolBinding, RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_extensions::ExtensionRegistry;
use brassclaw_filesystem::LocalFilesystem;
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, EffectKind, ExecutionContext, ExtensionId, MountView,
    NetworkPolicy, PackageId, ResourceEstimate, RuntimeKind, TrustClass, UserId,
};
use brassclaw_host_runtime::{
    CapabilitySurfaceVersion, FirstPartyCapabilityRegistry, HostRuntime, HostRuntimeServices,
    RuntimeCapabilityOutcome, RuntimeCapabilityRequest, RuntimeFailureKind,
    builtin_first_party_handlers, builtin_first_party_package,
};
use brassclaw_monty_host::{
    process::{
        PortAnswer, ProcessBoundary, ProcessSnapshot, RecipeBoundary, RecipeEvent, WorkerCommand,
    },
    transport_actor::{ActorLimits, TransportClient, TransportOwner},
};
use brassclaw_resources::InMemoryResourceGovernor;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
};
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
mod native_pg;
#[path = "support/runtime.rs"]
mod support;

fn snapshot(revision: u64, enabled: bool, tool: Uuid) -> StableToolPolicySnapshot {
    StableToolPolicySnapshot {
        revision,
        tools: HashMap::from([(
            tool,
            InstanceToolRule {
                enabled,
                revision,
                execution: ToolExecutionRules {
                    allowed_effects: vec![EffectKind::DispatchCapability],
                    mounts: MountView::default(),
                    network: NetworkPolicy::default(),
                    secrets: vec![],
                    resource_ceiling: None,
                },
            },
        )]),
        capabilities: HashMap::from([(CapabilityId::new("builtin.json").unwrap(), tool)]),
    }
}
fn trust() -> TrustDecision {
    TrustDecision {
        effective_trust: EffectiveTrustClass::user_trusted(),
        authority_ceiling: AuthorityCeiling {
            allowed_effects: vec![EffectKind::DispatchCapability],
            max_resource_ceiling: None,
        },
        provenance: TrustProvenance::AdminConfig,
        evaluated_at: chrono::Utc::now(),
    }
}

struct KernelPort {
    runtime: Arc<dyn HostRuntime>,
    policy: Arc<LiveStableToolPolicy>,
    deny_second: bool,
    tool: Uuid,
    calls: Mutex<usize>,
    outcomes: Mutex<Vec<&'static str>>,
}
impl KernelPort {
    fn new(deny_second: bool, tool: Uuid) -> Self {
        let package = builtin_first_party_package().unwrap();
        let mut registry = ExtensionRegistry::new();
        registry.insert(package).unwrap();
        let registrations = builtin_first_party_handlers(Arc::new(
            brassclaw_triggers::InMemoryTriggerRepository::default(),
        ))
        .unwrap();
        let id = CapabilityId::new("builtin.json").unwrap();
        let selected = registrations.retain_binding(&id).unwrap();
        let retained = FirstPartyCapabilityRegistry::new().with_handler(id, Arc::new(selected));
        // The actual selected handler stays alive after the source registry dies.
        drop(registrations);
        let policy = Arc::new(LiveStableToolPolicy::new(snapshot(1, true, tool)).unwrap());
        let trust_policy = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
            AdminEntry::for_local_manifest(
                PackageId::new("builtin").unwrap(),
                "/system/extensions/builtin/manifest.toml".into(),
                None,
                HostTrustAssignment::first_party(),
                vec![EffectKind::DispatchCapability],
                None,
            ),
        ]))])
        .unwrap();
        let runtime = HostRuntimeServices::new(
            Arc::new(registry),
            Arc::new(LocalFilesystem::new()),
            Arc::new(InMemoryResourceGovernor::new()),
            Arc::new(InstanceToolAuthorizer::new(policy.clone())),
            brassclaw_processes::ProcessServices::in_memory(),
            CapabilitySurfaceVersion::new("retained-draft-kernel").unwrap(),
        )
        .with_first_party_capabilities(Arc::new(retained))
        .with_trust_policy(Arc::new(trust_policy))
        .host_runtime_for_local_testing();
        Self {
            runtime: Arc::new(runtime),
            policy,
            deny_second,
            tool,
            calls: Mutex::new(0),
            outcomes: Mutex::new(Vec::new()),
        }
    }
}
#[async_trait]
impl RetainedToolPort for KernelPort {
    async fn dispatch(&self, binding: &RetainedToolBinding, arguments: Value) -> PortAnswer {
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
                .publish(1, snapshot(2, false, self.tool))
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
            .invoke_capability(RuntimeCapabilityRequest::new(
                context,
                CapabilityId::new(binding.capability_id()).unwrap(),
                ResourceEstimate::default(),
                arguments,
                trust(),
            ))
            .await
            .unwrap();
        match outcome {
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
        }
    }
}

fn draft(
    id: Uuid,
    class: i32,
    document: Value,
    dependencies: &[Uuid],
    association: Value,
) -> ComponentRevisionDraft {
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":id,"class_code":class,
        "document":document,"dependencies":dependencies,"association":association})
        .to_string(),
    )
    .unwrap()
}
async fn program(
    store: &PgComponentRevisionStore,
    invalid_output: bool,
) -> Arc<RetainedToolProgram> {
    let (root, code, descriptor, tool, skill) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let inputs = json!({"data":{"type":"string","required":true,"checks":[]}});
    let result = json!({"type":"object","allow_extra_fields":false,"fields":{"text":{"type":"string","required":true}}});
    let association = json!({"format":"skill-association/1","skill_uuid":skill,"python_code_uuid":code,"tool_skill_uuid":descriptor,"tool_uuid":tool,
        "callable":"host.json","inputs":inputs,"arguments":{"data":"data"},"code_arguments":{"operation":{"type":"string","checks":[],"depends_on":[],"meaning":"Fixed parse operation for this usage"}},"result":result,
        "failure":{"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]}}).to_string();
    let body = if invalid_output {
        "value = host.json(operation='parse', data=inputs['data'])\nresult = 'invalid output'"
    } else {
        "result = host.json(operation='parse', data=inputs['data'])"
    };
    let recipe = json!({"variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["parse %"],"variable_patterns":[{"name":"data","pattern":null,"description":null}]}],
        "step_descriptions":[{"desc_idx":0,"label":"two explicit usages","yaml_source":"","steps":[
            {"stepnumber":1,"knowledge":"rust","goal":"Bind JSON","content":"","type":"component","include":[descriptor],"tool_bindings":[{"tool_id":tool,"tool_name":"json","params":{},"error_policy":{"policy":"fail"}}]},
            {"stepnumber":2,"knowledge":"orchestrator","goal":"Parse JSON","content":"","type":"component","include":[code]},
            {"stepnumber":3,"knowledge":"rust","goal":"Bind JSON","content":"","type":"component","include":[descriptor],"tool_bindings":[{"tool_id":tool,"tool_name":"json","params":{},"error_policy":{"policy":"fail"}}]},
            {"stepnumber":4,"knowledge":"orchestrator","goal":"Parse JSON again","content":"","type":"component","include":[code]}]}],
        "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":{
            "0:2":{"data":{"kind":"task_input","reference":"{{vars.data}}"}},"0:4":{"data":{"kind":"task_input","reference":"{{vars.data}}"}}}}}});
    let mut refs = Vec::new();
    for d in [
        draft(
            tool,
            0,
            json!({"capability_id":"builtin.json","callable":"host.json","input_contract":{
            "operation":{"type":"string","required":true,"checks":[]},"data":{"type":"string","required":true,"checks":[]}}}),
            &[],
            Value::Null,
        ),
        draft(
            descriptor,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":tool,"callable":"host.json","capability_id":"builtin.json"}}),
            &[tool],
            Value::Null,
        ),
        draft(
            code,
            22,
            json!({"content":body,"input_contract":inputs,"result_contract":result}),
            &[],
            Value::Null,
        ),
        draft(
            skill,
            1,
            json!({"body":"Parse the supplied JSON text and return its object."}),
            &[code, descriptor, tool],
            json!(association),
        ),
        draft(root, 21, recipe, &[descriptor, code, skill], Value::Null),
    ] {
        refs.push(store.stage(&d, 0).await.unwrap());
    }
    let retained = Arc::new(store.read_exact(&[root], &refs).await.unwrap());
    Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(retained, root, "selected", WorkflowClass::Deterministic)
                .unwrap(),
        )
        .unwrap(),
    )
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
        let prepared = program(&store, invalid_output).await;
        assert_eq!(prepared.bindings()["0:2"].combination().len(), 4);
        assert!(std::ptr::eq(
            prepared.bindings()["0:2"].combination(),
            prepared.bindings()["0:4"].combination()
        ));
        let data = json!({"text":"'quotes'\n Ü {{vars.data}} host.forbidden()"});
        let inputs = json!({"data":data.to_string()});
        let port = KernelPort::new(!invalid_output, prepared.bindings()["0:2"].tool().uuid);
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
        let admitted = exchange(&transport, WorkerCommand::Admit { key: ready.work_waits[0].1,
            task: json!({"conversation_id":"opaque-draft", "message_id":"draft-message", "turn_id":"draft-turn", "run_id":"draft-run", "user_input":inputs["data"], "history":[]}) }).await;
        let task = admitted.admitted_task.unwrap();
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
        exchange(&transport, WorkerCommand::Defer { key }).await;
        let root = exchange(&transport, WorkerCommand::Resolve { key, answer: PortAnswer::Return { value: json!({"ok":true,"program_ref":reference,
            "steps":prepared.program().steplist.iter().map(|s| json!({"step_id":s.step_id})).collect::<Vec<_>>(),"inputs":inputs,"flow":prepared.inputs().monty_flow().unwrap()}) } }).await;
        let mut root = progress(&transport, root).await;
        let mut execution =
            RetainedRecipeExecution::new(task, RetainedProgram::Tools(prepared.clone())).unwrap();
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
            if invalid_output || step.step_id == "0:4" {
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
                break;
            }
            let value = result.unwrap();
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
        // Failure never gets a fabricated product completion or effect-free
        // receipt. Retain actual evidence and reap the worker explicitly.
        owner.request_termination();
        let exit = owner.join().await.unwrap();
        assert!(exit.exit_status.is_some());
        assert!(exit.containment_error.is_none());
        assert!(exit.reap_error.is_none());
    }
}
