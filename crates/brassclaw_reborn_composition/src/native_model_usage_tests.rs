//! Actual retained kernel + PostgreSQL + operator-selected live model candidate.
//! Not catalogue activation, a global Recipe cutover or installed-system QA.
use std::{collections::HashMap, sync::Arc};

use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
    ToolExecutionRules,
};
use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
};
use brassclaw_extensions::ExtensionRegistry;
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, EffectKind, ExecutionContext, ExtensionId, InvocationId,
    MountView, NetworkPolicy, PackageId, ResourceCeiling, ResourceEstimate, RuntimeKind,
    TrustClass,
};
use brassclaw_host_runtime::{
    FirstPartyCapabilityRegistry, HostRuntimeServices, RetainedFirstPartyCapability,
    RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
};
use brassclaw_interceptor::InterceptorStore;
use brassclaw_reborn::{
    loop_driver_host::{
        RebornLoopDriverHostFactory, RebornLoopDriverHostRequest, TextOnlyLoopHostConfig,
    },
    model_gateway::{LlmModelProfilePolicy, LlmProviderModelGateway},
    model_routes::{
        ModelRoute, ModelRoutePolicy, ModelSelectionMode, ModelSlot,
        StaticModelRouteResolver,
    },
    monty_model_capability::RecipeModelPrefix,
    monty_task_host::MontyTaskHost,
};
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
    value_contract::EXECUTION_DATA_LIMITS,
};
use brassclaw_threads::{PgSessionThreadService, ThreadScope};
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
use brassclaw_turns::{
    InMemoryCheckpointStateStore, InMemoryLoopCheckpointStore, TurnStateStore,
    run_profile::{
        AgentLoopDriverRunRequest, InMemoryLoopHostMilestoneSink, InstructionSafetyContext,
        LoopModelResponse, LoopModelRouteSnapshot, MontyTaskHandoff,
    },
    runner::{RecordModelRouteSnapshotRequest, TurnRunTransitionPort},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{native_admission_fixture as admission_fixture, pg_monty_admission};

async fn program(store: &PgComponentRevisionStore) -> Arc<RetainedToolProgram> {
    let (root, code, binding, tool, skill) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let text = json!({"type":"string","required":true,"checks":[]});
    let inputs = json!({"prompt":{"type":"object","required":true,"checks":[],"allow_extra_fields":false,"fields":{
        "user_query":text,"chat_history":{"type":"list","required":true,"checks":[],"items":{"type":"object","checks":[],"allow_extra_fields":false,"fields":{"role":text,"content":text}}}
    }}});
    let result = json!({"type":"string","checks":[]});
    let failure = json!({"action":"stop","max_attempts":1,"idempotency":"not_assumed","idempotency_evidence_ref":null,"retryable_outcomes":[]});
    let exports = json!({"complete":{"symbol":"complete_model_usage","parameters":["inputs"],"mapping":true}});
    let association=json!({"format":"skill-association/1","skill_uuid":skill,"python_code_uuid":code,"tool_skill_uuid":binding,"tool_uuid":tool,
        "callable":"host.kohai_complete","inputs":inputs,"arguments":{"prompt":"prompt"},"code_arguments":{},"result":result,"failure":failure}).to_string();
    let mut steps = Vec::new();
    let mut layouts = serde_json::Map::new();
    for n in 0..6 {
        steps.push(json!({"stepnumber":n*2+1,"knowledge":"rust","goal":"Bind one model usage","content":"","type":"component","include":[binding],"tool_bindings":[{"tool_id":tool,"tool_name":"kohai_complete","params":{},"error_policy":{"policy":"fail"}}]}));
        steps.push(json!({"stepnumber":n*2+2,"knowledge":"orchestrator","goal":"Call one model operation","content":"","type":"component","include":[code]}));
        layouts.insert(
            format!("0:{}", n * 2 + 2),
            json!({"prompt":{"kind":"task_input","reference":"{{vars.prompt}}"}}),
        );
    }
    let docs = vec![
        (
            tool,
            0,
            json!({"capability_id":"host.kohai_complete","callable":"host.kohai_complete","input_contract":inputs}),
            vec![],
            Value::Null,
        ),
        (
            binding,
            13,
            json!({"binding":{"format":"tool-skill-binding/1","tool_uuid":tool,"callable":"host.kohai_complete","capability_id":"host.kohai_complete"}}),
            vec![tool],
            Value::Null,
        ),
        (
            code,
            22,
            json!({"content":"def complete_model_usage(inputs):\n    return host.kohai_complete(prompt=inputs['prompt'])", "input_contract":inputs,"result_contract":result,
            "preload":{"format":"python-preload/2","exports":exports,"private_functions":{},"constants":[],"imports":[],"dependencies":[],"default_export":"complete"}}),
            vec![],
            Value::Null,
        ),
        (
            skill,
            1,
            json!({"body":"One selected provider model operation; return its complete response as JSON text, without executing proposals.","interface":{"format":"skill-interface/1","python_code_uuid":code,"exports":exports,"inputs":inputs,"result":result,"failure":failure,"dependencies":[],"private_symbols":[]}}),
            vec![code, binding, tool],
            json!(association),
        ),
        (
            root,
            21,
            json!({"variants":[{"variant_key":"selected","step_link":"0:1-0:E","intent_examples":["model %"],"variable_patterns":[{"name":"prompt","pattern":null,"description":null}]}],
            "step_descriptions":[{"desc_idx":0,"label":"Independent candidate occurrences","yaml_source":"","steps":steps}],
            "input_layouts":{"selected":{"format":"recipe-input-layout/1","task_inputs":inputs,"steps":layouts}}}),
            vec![binding, code, skill],
            Value::Null,
        ),
    ];
    let mut references = Vec::new();
    for (id, class, doc, deps, association) in docs {
        let draft=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id,"class_code":class,"document":doc,"dependencies":deps,"association":association}).to_string()).unwrap();
        references.push(store.stage(&draft, 0).await.unwrap());
    }
    let retained = Arc::new(store.read_exact(&[root], &references).await.unwrap());
    Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(retained, root, "selected", WorkflowClass::RequiresModel)
                .unwrap(),
        )
        .unwrap(),
    )
}
fn policy(
    revision: u64,
    tool: Uuid,
    enabled: bool,
    output: Option<u64>,
) -> StableToolPolicySnapshot {
    StableToolPolicySnapshot {
        revision,
        capabilities: HashMap::from([(CapabilityId::new("host.kohai_complete").unwrap(), tool)]),
        tools: HashMap::from([(
            tool,
            InstanceToolRule {
                enabled,
                revision,
                execution: ToolExecutionRules {
                    allowed_effects: vec![EffectKind::ExternalWrite],
                    mounts: MountView::default(),
                    network: NetworkPolicy::default(),
                    secrets: vec![],
                    resource_ceiling: output.map(|limit| ResourceCeiling {
                        max_usd: None,
                        max_input_tokens: None,
                        max_output_tokens: None,
                        max_wall_clock_ms: None,
                        max_output_bytes: Some(limit),
                        sandbox: None,
                    }),
                },
            },
        )]),
    }
}
fn trust() -> TrustDecision {
    TrustDecision {
        effective_trust: EffectiveTrustClass::user_trusted(),
        authority_ceiling: AuthorityCeiling {
            allowed_effects: vec![EffectKind::ExternalWrite],
            max_resource_ceiling: None,
        },
        provenance: TrustProvenance::AdminConfig,
        evaluated_at: chrono::Utc::now(),
    }
}

fn execution_context(host: &MontyTaskHost) -> ExecutionContext {
    let c = host.run_context();
    let mut execution = ExecutionContext::local_default(
        c.actor.as_ref().unwrap().user_id.clone(),
        ExtensionId::new("model-candidate").unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::FirstParty,
        CapabilitySet::default(),
        MountView::default(),
    )
    .unwrap();
    execution.tenant_id = c.scope.tenant_id.clone();
    execution.agent_id = c.scope.agent_id.clone();
    execution.project_id = c.scope.project_id.clone();
    execution.thread_id = Some(c.thread_id.clone());
    execution.resource_scope.tenant_id = execution.tenant_id.clone();
    execution.resource_scope.agent_id = execution.agent_id.clone();
    execution.resource_scope.project_id = execution.project_id.clone();
    execution.resource_scope.thread_id = execution.thread_id.clone();
    execution
}

async fn invoke(
    host: &Arc<MontyTaskHost>,
    admission: &Arc<pg_monty_admission::PgMontyAdmission>,
    program: &RetainedToolProgram,
    runtime: &RetainedFirstPartyCapability,
    step: &str,
    arguments: Value,
) -> (InvocationId, RuntimeCapabilityOutcome) {
    let execution = execution_context(host);
    let id = execution.invocation_id;
    let record = admission
        .begin_tool_invocation(id, program, step, &arguments, EXECUTION_DATA_LIMITS)
        .await
        .unwrap();
    let outcome = host
        .invoke_retained_tool(
            runtime,
            RuntimeCapabilityRequest::new(
                execution,
                runtime.descriptor().id.clone(),
                ResourceEstimate::default(),
                arguments,
                trust(),
            ),
        )
        .await
        .unwrap();
    let answer = match &outcome {
        RuntimeCapabilityOutcome::Completed(done) => {
            brassclaw_monty_host::process::PortAnswer::Return {
                value: done.output.clone().into(),
            }
        }
        _ => brassclaw_monty_host::process::PortAnswer::TerminalError {
            reason_kind: "model_candidate_failed".into(),
        },
    };
    record.record_answer(&answer).await.unwrap();
    (id, outcome)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires BRASSCLAW_TEST_MODEL_URL and BRASSCLAW_TEST_MODEL_ID for an actual operator-selected model"]
async fn native_model_usage_retains_real_response_before_output_validation_and_reconciles_without_replay()
 {
    use brassclaw_llm::{config::RegistryProviderConfig, registry::ProviderProtocol};
    let endpoint = std::env::var("BRASSCLAW_TEST_MODEL_URL").expect("explicit live endpoint");
    let model = std::env::var("BRASSCLAW_TEST_MODEL_ID").expect("explicit live model");
    let rig = crate::runtime::test_pg::pg_rig().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = program(&store).await;
    let (admitted, mut claimed) =
        admission_fixture::reserve_with_claim(rig.pool.clone(), "real selected model candidate")
            .await;
    assert_eq!(
        admitted.input["run_id"],
        admitted.context.run_id.to_string()
    );
    let route = LoopModelRouteSnapshot::try_new(
        "native-model",
        &model,
        "config:native-model-v1",
        "auth:none",
    )
    .unwrap();
    claimed.state = admitted
        .state
        .record_model_route_snapshot(RecordModelRouteSnapshotRequest {
            run_id: admitted.context.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
            snapshot: route.clone(),
        })
        .await
        .unwrap();
    let selected_route = ModelRoute::new("native-model", &model).unwrap();
    let resolver = Arc::new(
        StaticModelRouteResolver::new(
            ModelRoutePolicy::new(ModelSelectionMode::ManagedOnly)
                .with_approved_route(selected_route.clone()),
        )
        .with_route(ModelSlot::Default, selected_route),
    );
    let context = admitted.context.clone().with_resolved_model_route(route);
    // The actual runner normally maintains this durable lease during provider
    // waits. The native adapter fixture uses that same heartbeat API.
    let (stop_heartbeat, mut heartbeat_stop) = tokio::sync::watch::channel(false);
    let heartbeat_store = admitted.state.clone();
    let heartbeat_request = brassclaw_turns::runner::HeartbeatRequest {
        run_id: admitted.context.run_id,
        runner_id: claimed.runner_id,
        lease_token: claimed.lease_token,
    };
    let heartbeat = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = heartbeat_stop.changed() => {
                    if changed.is_err() || *heartbeat_stop.borrow() {
                        return Ok::<(), brassclaw_turns::TurnError>(());
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                    heartbeat_store.heartbeat(heartbeat_request.clone()).await?;
                }
            }
        }
    }));
    let profile = context.resolved_run_profile.model_profile_id.clone();
    let provider = brassclaw_llm::create_registry_provider(
        &RegistryProviderConfig::generic(
            ProviderProtocol::OpenAiCompletions,
            "native-model",
            None,
            endpoint,
            model.clone(),
        ),
        120,
    )
    .unwrap();
    let gateway = Arc::new(
        LlmProviderModelGateway::with_provider_identity(
            "native-model",
            provider,
            LlmModelProfilePolicy::new().allow_model_profile(profile, Some(model)),
        )
        .with_max_output_tokens(Some(brassclaw_agent_loop::LiveTokenBudget::new(Some(96)))),
    );
    let thread_scope = ThreadScope {
        tenant_id: context.scope.tenant_id.clone(),
        agent_id: context.scope.agent_id.clone().unwrap(),
        project_id: context.scope.project_id.clone(),
        owner_user_id: None,
    };
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!("../../brassclaw_loop_support/prompts/compaction_summarizer_fresh.md").into(),
    );
    let packets = Arc::new(brassclaw_interceptor::PgInterceptorStore::new(
        rig.pool.clone(),
        context.scope.tenant_id.as_str(),
    ));
    let factory = RebornLoopDriverHostFactory::new(
        Arc::new(PgSessionThreadService::new(
            rig.pool.clone(),
            context.scope.tenant_id.as_str(),
        )),
        thread_scope,
        gateway,
        Arc::new(InMemoryCheckpointStateStore::default()),
        admitted.state.clone(),
        Arc::new(InMemoryLoopCheckpointStore::default()),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
        TextOnlyLoopHostConfig {
            max_messages: 8,
            require_model_route_snapshot: true,
        },
        InstructionSafetyContext::local_development_noop(),
    )
    .with_model_route_resolver(resolver)
    .with_interceptor_store(packets.clone())
    .with_token_budget_mode(|| false);
    let host = Arc::new(MontyTaskHost::new(
        MontyTaskHandoff::new(
            AgentLoopDriverRunRequest {
                turn_id: context.turn_id,
                run_id: context.run_id,
                resolved_run_profile: context.resolved_run_profile.clone(),
            },
            brassclaw_turns::run_profile::MontyTaskAttempt {
                run_id: context.run_id,
                runner_id: claimed.runner_id,
                lease_token: claimed.lease_token,
            },
            Arc::new(
                factory
                    .build_text_only_host(RebornLoopDriverHostRequest {
                        claimed_run: claimed,
                        loop_run_context: context.clone(),
                    })
                    .await
                    .unwrap(),
            ),
        )
        .unwrap(),
    ));
    let tool = program.bindings()["0:2"].tool().uuid;
    let live = Arc::new(LiveStableToolPolicy::new(policy(1, tool, true, None)).unwrap());
    let trust_policy = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            PackageId::new("host").unwrap(),
            "/system/extensions/host/manifest.toml".into(),
            None,
            HostTrustAssignment::first_party(),
            vec![EffectKind::ExternalWrite],
            None,
        ),
    ]))])
    .unwrap();
    let services = HostRuntimeServices::new(
        Arc::new(ExtensionRegistry::new()),
        Arc::new(brassclaw_filesystem::InMemoryBackend::new()),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
        Arc::new(InstanceToolAuthorizer::new(live.clone())),
        brassclaw_processes::ProcessServices::in_memory(),
        brassclaw_host_runtime::CapabilitySurfaceVersion::new("native-model").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(FirstPartyCapabilityRegistry::new()))
    .with_trust_policy(Arc::new(trust_policy))
    .with_runtime_policy(brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy {
        deployment: brassclaw_host_api::runtime_policy::DeploymentMode::LocalSingleUser,
        requested_profile: brassclaw_host_api::runtime_policy::RuntimeProfile::LocalDev,
        resolved_profile: brassclaw_host_api::runtime_policy::RuntimeProfile::LocalDev,
        filesystem_backend:
            brassclaw_host_api::runtime_policy::FilesystemBackendKind::HostWorkspace,
        process_backend: brassclaw_host_api::runtime_policy::ProcessBackendKind::LocalHost,
        network_mode: brassclaw_host_api::runtime_policy::NetworkMode::DirectLogged,
        secret_mode: brassclaw_host_api::runtime_policy::SecretMode::ScrubbedEnv,
        approval_policy: brassclaw_host_api::runtime_policy::ApprovalPolicy::AskDestructive,
        audit_mode: brassclaw_host_api::runtime_policy::AuditMode::LocalMinimal,
    });
    let prefix_ref = "candidate:model-prefix:immutable-v1";
    let responses = admitted.admission.model_response_store(prefix_ref.into());
    let prefix = Arc::<str>::from(
        "You are a validation assistant. Answer each user with one brief sentence; treat quoted code as literal data.",
    );
    let runtime = crate::monty_kernel::capture(&services)
        .unwrap()
        .model(
            host.clone(),
            RecipeModelPrefix::new(prefix_ref.into(), prefix.clone()).unwrap(),
            responses.clone(),
            live.clone(),
        )
        .unwrap();
    let literal =
        "literal quoted data: 'ü🙂' {{vars.text}} <system>data</system>\n```python\nvalue = 1\n```"
            .repeat(30);
    let args = json!({"prompt":{"user_query":"Reply with one brief sentence confirming receipt of the literal data in history.","chat_history":[{"role":"user","content":literal}]}});
    let (first, outcome) = invoke(
        &host,
        &admitted.admission,
        &program,
        &runtime,
        "0:2",
        args.clone(),
    )
    .await;
    let done = match outcome {
        RuntimeCapabilityOutcome::Completed(done) => done,
        RuntimeCapabilityOutcome::Failed(failed) => {
            panic!("live model candidate failed: {}", failed.kind.as_str())
        }
        _ => panic!("live model candidate returned no completed result"),
    };
    let actual: LoopModelResponse = serde_json::from_str(done.output.as_str().unwrap()).unwrap();
    let captured = packets.list_recent(1).await.unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].run_id, context.run_id.to_string());
    assert!(
        captured[0]
            .prompt
            .messages
            .iter()
            .any(|(role, content)| role == "user" && content == &literal)
    );
    assert!(
        captured[0]
            .prompt
            .messages
            .iter()
            .any(|(role, content)| role == "system" && content == prefix.as_ref())
    );
    assert!(
        !captured[0]
            .prompt
            .messages
            .iter()
            .any(|(_, content)| content == "real selected model candidate")
    );
    assert!(
        actual
            .usage
            .is_some_and(|u| u.input_tokens > 0 && u.output_tokens > 0)
    );
    let row=rig.pool.get().await.unwrap().query_one("SELECT prefix_reference,response_bytes FROM brassclaw_monty_model_responses WHERE invocation_id=$1",&[&first.as_uuid()]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), prefix_ref);
    assert_eq!(row.get::<_, String>(1), done.output.as_str().unwrap());
    assert!(!responses.has_uncommitted_results());
    live.publish(1, policy(2, tool, true, Some(1))).unwrap();
    let (second, failed) = invoke(
        &host,
        &admitted.admission,
        &program,
        &runtime,
        "0:4",
        args.clone(),
    )
    .await;
    assert!(matches!(failed, RuntimeCapabilityOutcome::Failed(_)));
    assert!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_opt(
                "SELECT 1 FROM brassclaw_monty_model_responses WHERE invocation_id=$1",
                &[&second.as_uuid()]
            )
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(host.review_dispatch_counts().0, 2);
    live.publish(2, policy(3, tool, true, None)).unwrap();
    rig.pool.get().await.unwrap().batch_execute("CREATE FUNCTION reject_native_model_response() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'deliberate native response rejection'; END $$; CREATE TRIGGER reject_native_model_response BEFORE INSERT ON brassclaw_monty_model_responses FOR EACH ROW EXECUTE FUNCTION reject_native_model_response();").await.unwrap();
    let (third, failed) = invoke(
        &host,
        &admitted.admission,
        &program,
        &runtime,
        "0:6",
        args.clone(),
    )
    .await;
    assert!(matches!(failed, RuntimeCapabilityOutcome::Failed(_)));
    assert!(responses.has_uncommitted_results());
    assert_eq!(host.review_dispatch_counts().0, 3);
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_native_model_response ON brassclaw_monty_model_responses; DROP FUNCTION reject_native_model_response();").await.unwrap();
    responses.reconcile_received_response(third).await.unwrap();
    responses.reconcile_received_response(third).await.unwrap();
    assert!(!responses.has_uncommitted_results());
    assert_eq!(
        host.review_dispatch_counts().0,
        3,
        "reconciliation cannot replay the provider"
    );
    let mut bad = args.clone();
    bad["prompt"]["prefix_reference"] = json!("attempted override");
    let (_, failed) = invoke(&host, &admitted.admission, &program, &runtime, "0:8", bad).await;
    assert!(matches!(failed, RuntimeCapabilityOutcome::Failed(_)));
    assert_eq!(host.review_dispatch_counts().0, 3);
    live.publish(3, policy(4, tool, false, None)).unwrap();
    let (_, failed) = invoke(
        &host,
        &admitted.admission,
        &program,
        &runtime,
        "0:10",
        args.clone(),
    )
    .await;
    assert!(matches!(failed, RuntimeCapabilityOutcome::Failed(_)));
    assert_eq!(host.review_dispatch_counts().0, 3);
    assert!(
        responses
            .prepare(first, &json!({}), prefix_ref)
            .await
            .is_err()
    );
    let foreign =
        admission_fixture::reserve(rig.pool.clone(), "foreign native model receipt owner").await;
    assert!(
        foreign
            .admission
            .model_response_store(prefix_ref.into())
            .prepare(first, &json!({}), prefix_ref)
            .await
            .is_err()
    );
    // Hold the real native-result write after a real provider response. Fence
    // the original attempt while its actual response is owned but not delivered.
    live.publish(4, policy(5, tool, true, None)).unwrap();
    let gate = rig.pool.get().await.unwrap();
    gate.query_one("SELECT pg_advisory_lock(771888601::bigint)", &[])
        .await
        .unwrap();
    rig.pool.get().await.unwrap().batch_execute("CREATE FUNCTION hold_late_native_model_response() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(771888601::bigint); RETURN NEW; END $$; CREATE TRIGGER hold_late_native_model_response BEFORE INSERT ON brassclaw_monty_model_responses FOR EACH ROW EXECUTE FUNCTION hold_late_native_model_response();").await.unwrap();
    let execution = execution_context(&host);
    let late_id = execution.invocation_id;
    let late_record = admitted
        .admission
        .begin_tool_invocation(late_id, &program, "0:12", &args, EXECUTION_DATA_LIMITS)
        .await
        .unwrap();
    let late_host = host.clone();
    let late_runtime = runtime.clone();
    let late_call = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        late_host
            .invoke_retained_tool(
                &late_runtime,
                RuntimeCapabilityRequest::new(
                    execution,
                    late_runtime.descriptor().id.clone(),
                    ResourceEstimate::default(),
                    args,
                    trust(),
                ),
            )
            .await
    }));
    tokio::time::timeout(std::time::Duration::from_secs(120), async {
        while !responses.has_uncommitted_results() {
            assert!(
                !late_call.is_finished(),
                "model call stopped before its received-response barrier"
            );
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("actual received model response reaches native persistence barrier");
    stop_heartbeat.send(true).unwrap();
    heartbeat.await.unwrap().unwrap();
    admitted
        .state
        .request_cancel(brassclaw_turns::CancelRunRequest {
            scope: context.scope.clone(),
            actor: context.actor.clone().unwrap(),
            run_id: context.run_id,
            reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
            idempotency_key: brassclaw_turns::IdempotencyKey::new(
                "native-model-cancel-after-receive",
            )
            .unwrap(),
        })
        .await
        .unwrap();
    // The barrier returns a receipt at its deadline, including pending calls;
    // an Ok receipt alone never acknowledges completed cancellation.
    let pending = host
        .fence_handle()
        .fence_and_wait(host.attempt(), std::time::Duration::ZERO)
        .await
        .unwrap();
    assert!(!pending.calls_settled(), "blocked native persistence is not quiescence");
    assert!(pending.pending_calls > 0);
    assert!(pending.abandoned_calls.is_empty());
    gate.query_one("SELECT pg_advisory_unlock(771888601::bigint)", &[])
        .await
        .unwrap();
    drop(gate);
    let late_error = late_call.await.unwrap().unwrap_err();
    assert_eq!(
        late_error.kind,
        brassclaw_turns::run_profile::AgentLoopHostErrorKind::Cancelled
    );
    assert!(
        host.fence_handle()
            .fence_and_wait(host.attempt(), std::time::Duration::ZERO)
            .await
            .unwrap()
            .calls_settled()
    );
    let late_bytes: String = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT response_bytes FROM brassclaw_monty_model_responses WHERE invocation_id=$1",
            &[&late_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    let withheld = host.take_withheld_results();
    assert!(withheld.iter().any(|result| matches!(&result.value, brassclaw_reborn::monty_task_host::WithheldTaskPortValue::RecordedModelResponse {invocation_id, response_bytes} if *invocation_id == late_id && response_bytes == &late_bytes)));
    assert!(!responses.has_uncommitted_results());
    assert_eq!(host.review_dispatch_counts().0, 4);
    late_record
        .record_answer(&brassclaw_monty_host::process::PortAnswer::TerminalError {
            reason_kind: "model_attempt_cancelled".into(),
        })
        .await
        .unwrap();
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER hold_late_native_model_response ON brassclaw_monty_model_responses; DROP FUNCTION hold_late_native_model_response();").await.unwrap();
    for sql in [
        "UPDATE brassclaw_monty_model_responses SET prefix_reference='changed'",
        "DELETE FROM brassclaw_monty_model_responses",
        "TRUNCATE brassclaw_monty_model_responses",
    ] {
        assert!(
            rig.pool
                .get()
                .await
                .unwrap()
                .batch_execute(sql)
                .await
                .is_err()
        );
    }
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_monty_model_responses", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 4);
}
