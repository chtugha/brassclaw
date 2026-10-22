//! Actual retained kernel + PostgreSQL + operator-selected live model candidate.
//! Not catalogue activation, a global Recipe cutover or installed-system QA.
use crate::test_loopback_http as loopback_http;

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
        ModelRoute, ModelRoutePolicy, ModelSelectionMode, ModelSlot, StaticModelRouteResolver,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires BRASSCLAW_TEST_MODEL_URL and BRASSCLAW_TEST_MODEL_ID for an actual operator-selected model"]
async fn native_prepared_model_call_retains_backend_before_accounting_and_keeps_live_limits() {
    use axum::{extract::State, routing::post};
    use brassclaw_llm::{RegistryProviderConfig, SwappableLlmProvider};
    use brassclaw_loop_support::ThreadBackedLoopContextPort;
    use brassclaw_reborn::model_gateway::ThreadBackedLoopModelGateway;
    use brassclaw_turns::run_profile::{
        AgentLoopHostErrorKind, HostManagedLoopPromptPort, InMemoryInstructionMaterializationStore,
        LoopModelDispatchPolicy, LoopModelGateway, LoopModelGatewayRequest, LoopModelRequest,
        LoopPromptBundleRequest, LoopPromptPort, PromptMode,
    };
    struct Relay {
        endpoint: String,
        model: String,
        client: reqwest::Client,
        calls: tokio::sync::Mutex<Vec<String>>,
    }
    async fn relay(
        State(state): State<Arc<Relay>>,
        path: axum::http::Uri,
        axum::Json(body): axum::Json<Value>,
    ) -> ([(axum::http::HeaderName, &'static str); 1], Vec<u8>) {
        assert_eq!(body["model"], state.model);
        state.calls.lock().await.push(path.path().into());
        let response = state
            .client
            .post(format!(
                "{}/chat/completions",
                state.endpoint.trim_end_matches('/')
            ))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let bytes = response.bytes().await.unwrap();
        let actual: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(actual["choices"][0]["finish_reason"], "stop");
        assert!(actual["choices"][0]["message"]["content"].is_string());
        // Forward the actual upstream answer unchanged; no simulated provider
        // success or invented token observation establishes this regression.
        (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            bytes.to_vec(),
        )
    }
    let state = Arc::new(Relay {
        endpoint: std::env::var("BRASSCLAW_TEST_MODEL_URL").expect("explicit live endpoint"),
        model: std::env::var("BRASSCLAW_TEST_MODEL_ID").expect("explicit live model"),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap(),
        calls: tokio::sync::Mutex::new(Vec::new()),
    });
    let router = axum::Router::new()
        .route("/a/v1/chat/completions", post(relay))
        .route("/b/v1/chat/completions", post(relay))
        .with_state(state.clone());
    let (address, stop, server) = loopback_http::spawn(router).await;
    let provider = |role: &str| {
        brassclaw_llm::create_registry_provider(
            &RegistryProviderConfig::generic(
                brassclaw_llm::ProviderProtocol::OpenAiCompletions,
                format!("retained-{role}"),
                None,
                format!("http://{address}/{role}/v1"),
                state.model.clone(),
            ),
            120,
        )
        .unwrap()
    };
    let selected = Arc::new(SwappableLlmProvider::new_with_identity(
        provider("a"),
        "retained-a",
    ));
    let input_limit = brassclaw_agent_loop::LiveTokenBudget::new(None);
    let rig = crate::runtime::test_pg::pg_rig().await;
    let admitted = admission_fixture::reserve(
        rig.pool.clone(),
        "Answer with one short sentence describing a triangle.",
    )
    .await;
    let context = admitted.context;
    let scope = ThreadScope {
        tenant_id: context.scope.tenant_id.clone(),
        agent_id: context.scope.agent_id.clone().unwrap(),
        project_id: context.scope.project_id.clone(),
        owner_user_id: None,
    };
    let threads = Arc::new(PgSessionThreadService::new(
        rig.pool.clone(),
        context.scope.tenant_id.as_str(),
    ));
    let model_gateway = Arc::new(
        LlmProviderModelGateway::with_provider_identity(
            "retained-a",
            selected.clone(),
            LlmModelProfilePolicy::new()
                .allow_model_profile(context.resolved_run_profile.model_profile_id.clone(), None),
        )
        .with_total_input_tokens(Some(input_limit.clone())),
    );
    let gateway = ThreadBackedLoopModelGateway::new(
        threads.clone(),
        scope.clone(),
        model_gateway.clone(),
        16,
        InstructionSafetyContext::local_development_noop(),
    );
    let prompt = HostManagedLoopPromptPort::new(
        context.clone(),
        Arc::new(ThreadBackedLoopContextPort::new(
            threads,
            scope,
            context.clone(),
            16,
        )),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
    )
    .with_safety_context(InstructionSafetyContext::local_development_noop())
    .with_instruction_materialization_store(Arc::new(
        InMemoryInstructionMaterializationStore::default(),
    ))
    .build_prompt_bundle(LoopPromptBundleRequest {
        mode: PromptMode::TextOnly,
        context_cursor: None,
        surface_version: None,
        checkpoint_state_ref: None,
        max_messages: Some(16),
        inline_messages: Vec::new(),
        capability_view: None,
        recipe_hint: None,
    })
    .await
    .unwrap();
    let request = || LoopModelGatewayRequest {
        context: context.clone(),
        request: LoopModelRequest {
            dispatch_policy: LoopModelDispatchPolicy::SingleSubmission,
            messages: prompt.messages.clone(),
            ..LoopModelRequest::default()
        },
    };
    let first = gateway
        .prepare_model_call(request())
        .await
        .unwrap()
        .unwrap();
    selected.swap_with_identity(provider("b"), "retained-b");
    assert!(
        state.calls.lock().await.is_empty(),
        "retention has no provider I/O"
    );
    let answer = first.dispatch().await.unwrap();
    assert!(matches!(
        answer.output,
        brassclaw_turns::run_profile::ParentLoopOutput::AssistantReply(_)
    ));
    assert!(
        answer.usage.is_some(),
        "retain actual upstream token observation"
    );
    assert_eq!(*state.calls.lock().await, ["/a/v1/chat/completions"]);

    let second = gateway
        .prepare_model_call(request())
        .await
        .unwrap()
        .unwrap();
    input_limit.set(Some(1));
    let (rejected, submission) = second.dispatch_with_submission().await;
    assert_eq!(
        rejected.unwrap_err().kind,
        AgentLoopHostErrorKind::BudgetExceeded
    );
    assert_eq!(
        submission,
        brassclaw_turns::run_profile::ModelSubmission::NotSubmitted
    );
    assert_eq!(
        state.calls.lock().await.len(),
        1,
        "live rejection precedes I/O"
    );
    input_limit.set(None);
    let third = gateway
        .prepare_model_call(request())
        .await
        .unwrap()
        .unwrap();
    third.dispatch().await.unwrap();
    assert_eq!(
        *state.calls.lock().await,
        ["/a/v1/chat/completions", "/b/v1/chat/completions"]
    );
    // Pause the actual guarded system caller before its policy/real-governor
    // accounting completes. The barrier delegates the ordinary policy result;
    // it supplies neither a fake provider response nor an accounting success.
    use brassclaw_loop_support::{
        GuardedSystemInferencePort, ModelGatewayBackedSystemInferencePort,
    };
    use brassclaw_turns::run_profile::{
        LoopModelGatewayError, LoopModelPolicyGuard, LoopRunContext, ModelWorkRequest,
        NoOpPolicyGuard, SystemInferenceIdentity, SystemInferencePort, SystemInferenceRequest,
        SystemInferenceTaskId, SystemPromptSource, SystemTaskKind,
    };
    struct PausedPolicy {
        inner: NoOpPolicyGuard,
        entered: tokio::sync::Notify,
        resume: tokio::sync::Notify,
    }
    #[async_trait::async_trait]
    impl LoopModelPolicyGuard for PausedPolicy {
        async fn check_model_work_policy(
            &self,
            context: &LoopRunContext,
            work: &ModelWorkRequest,
        ) -> Result<(), LoopModelGatewayError> {
            self.entered.notify_one();
            self.resume.notified().await;
            self.inner.check_model_work_policy(context, work).await
        }
    }
    let paused = Arc::new(PausedPolicy {
        inner: NoOpPolicyGuard,
        entered: Default::default(),
        resume: Default::default(),
    });
    let governor = Arc::new(brassclaw_resources::PersistentResourceGovernor::new(
        brassclaw_resources::PgResourceGovernorStore::new(
            rig.pool.clone(),
            format!("system-prepared-{}", Uuid::new_v4()),
        ),
    ));
    let accountant = Arc::new(brassclaw_loop_support::GovernorBackedAccountant::new(
        governor.clone(),
        Arc::new(brassclaw_loop_support::ZeroCostTable),
    ));
    let direct: Arc<dyn SystemInferencePort> = Arc::new(
        ModelGatewayBackedSystemInferencePort::new(model_gateway, context.clone()),
    );
    let guarded = Arc::new(GuardedSystemInferencePort::new(
        direct.clone(),
        context.clone(),
        accountant,
        paused.clone(),
    ));
    let system_request = SystemInferenceRequest {
        task_id: SystemInferenceTaskId::new(),
        identity: SystemInferenceIdentity {
            task_kind: SystemTaskKind::Compaction,
            prompt_source: SystemPromptSource::Static {
                prompt_id: "prepared_system".to_string().try_into().unwrap(),
            },
            system_prompt: "Answer in one short sentence.".into(),
        },
        input_text: "Briefly define a square.".into(),
        max_input_tokens: 1000,
        deadline_ms: 120000,
    };
    let running = tokio::spawn({
        let guarded = guarded.clone();
        let request = system_request.clone();
        async move { guarded.call_system_inference(request).await }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        paused.entered.notified(),
    )
    .await
    .unwrap();
    assert_eq!(
        state.calls.lock().await.len(),
        2,
        "selection and policy preparation perform no model I/O"
    );
    selected.swap_with_identity(provider("a"), "retained-a");
    paused.resume.notify_one();
    let system_answer = running.await.unwrap().unwrap();
    assert_eq!(system_answer.task_id, system_request.task_id);
    assert!(!system_answer.output_text.is_empty());
    let actual = system_answer.usage.expect("actual upstream system meter");
    use brassclaw_resources::ResourceGovernor;
    let ledger = governor
        .account_snapshot(&brassclaw_resources::ResourceAccount::tenant(
            context.scope.tenant_id.clone(),
        ))
        .unwrap()
        .unwrap()
        .ledger;
    assert_eq!(ledger.spent.input_tokens, u64::from(actual.input_tokens));
    assert_eq!(ledger.spent.output_tokens, u64::from(actual.output_tokens));
    assert_eq!(ledger.reserved.input_tokens, 0);
    assert_eq!(ledger.reserved.output_tokens, 0);
    assert_eq!(
        *state.calls.lock().await,
        [
            "/a/v1/chat/completions",
            "/b/v1/chat/completions",
            "/b/v1/chat/completions"
        ]
    );
    // These are new internal operations, not retries of the completed request.
    // A native live-input rejection and a local input preflight both prove
    // provider non-entry. Release their actual PostgreSQL holds, retaining all
    // previously observed spend and without another upstream request.
    for local_preflight in [false, true] {
        input_limit.set(if local_preflight { None } else { Some(1) });
        let mut rejected_request = system_request.clone();
        rejected_request.task_id = SystemInferenceTaskId::new();
        if local_preflight {
            rejected_request.max_input_tokens = 0;
        }
        paused.resume.notify_one();
        let (rejected, submission) = guarded
            .call_system_inference_with_submission(rejected_request)
            .await;
        assert!(rejected.is_err());
        if local_preflight {
            assert_eq!(
                rejected.unwrap_err(),
                brassclaw_turns::run_profile::SystemInferenceError::InputTooLarge
            );
        }
        assert_eq!(
            submission,
            brassclaw_turns::run_profile::ModelSubmission::NotSubmitted
        );
        let after = governor
            .account_snapshot(&brassclaw_resources::ResourceAccount::tenant(
                context.scope.tenant_id.clone(),
            ))
            .unwrap()
            .unwrap()
            .ledger;
        assert_eq!(after.spent, ledger.spent);
        assert_eq!(after.reserved, ledger.reserved);
        assert_eq!(state.calls.lock().await.len(), 3);
    }
    input_limit.set(None);
    // A retained system selection cannot be repurposed for another task or text.
    let retained = direct
        .retain_model_selection(&system_request)
        .unwrap()
        .unwrap();
    let mut different = system_request.clone();
    different.input_text.push_str(" changed");
    assert!(retained.call_system_inference(different).await.is_err());
    let mut different = system_request.clone();
    different.task_id = SystemInferenceTaskId::new();
    assert!(retained.call_system_inference(different).await.is_err());
    input_limit.set(Some(1));
    assert!(
        retained
            .call_system_inference(system_request)
            .await
            .is_err()
    );
    input_limit.set(None);
    assert_eq!(state.calls.lock().await.len(), 3);
    stop.cancel();
    server.await.unwrap();
}

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
    for n in 0..7 {
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
    // A real upstream model completes before the relay deliberately loses its
    // first answer. This must remain one uncertain submission, not a hidden
    // configured retry after a charged/possibly completed external operation.
    use axum::{Json, extract::State, response::IntoResponse, routing::post};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Relay {
        endpoint: String,
        model: String,
        client: reqwest::Client,
        lose_answer: AtomicBool,
        omit_usage: AtomicBool,
        calls: AtomicUsize,
        actual_responses: tokio::sync::Mutex<Vec<Value>>,
    }
    async fn relay(
        State(state): State<Arc<Relay>>,
        Json(body): Json<Value>,
    ) -> axum::response::Response {
        state.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            body["model"], state.model,
            "a pinned task cannot use the configured cheap model"
        );
        assert!(
            body.get("max_completion_tokens").is_none() && body.get("max_tokens").is_none(),
            "natural-completion custody fixture has no artificial output cap"
        );
        let actual = state
            .client
            .post(format!(
                "{}/chat/completions",
                state.endpoint.trim_end_matches('/')
            ))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(
            actual.status().is_success(),
            "actual selected model must complete"
        );
        let bytes = actual.bytes().await.unwrap();
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            parsed["choices"][0]["finish_reason"], "stop",
            "successful-answer fixture must actually complete"
        );
        assert!(parsed["choices"][0]["message"]["content"].is_string());
        state.actual_responses.lock().await.push(parsed.clone());
        if state.omit_usage.load(Ordering::SeqCst) {
            // Fault injection removes only metadata from a real received model
            // answer; no content, counters or success is manufactured.
            let mut delivered = parsed;
            delivered.as_object_mut().unwrap().remove("usage");
            return Json(delivered).into_response();
        }
        if state.lose_answer.load(Ordering::SeqCst) {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error":{"message":"upstream completion was lost"}})),
            )
                .into_response();
        }
        (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            bytes,
        )
            .into_response()
    }
    let relay_state = Arc::new(Relay {
        endpoint,
        model: model.clone(),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap(),
        lose_answer: AtomicBool::new(true),
        omit_usage: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
        actual_responses: tokio::sync::Mutex::new(Vec::new()),
    });
    let router = axum::Router::new()
        .route("/v1/chat/completions", post(relay))
        .with_state(relay_state.clone());
    let (address, stop_relay, relay_server) = loopback_http::spawn(router).await;
    let mut config = brassclaw_llm::build_llm_config_with_provider_policy(
        brassclaw_llm::ResolvedProviderConfig::Registry(RegistryProviderConfig::generic(
            ProviderProtocol::OpenAiCompletions,
            "native-model",
            None,
            format!("http://{address}/v1"),
            model.clone(),
        )),
        brassclaw_llm::ProviderRequestPolicy {
            max_retries: 3,
            circuit_breaker_threshold: Some(10),
            ..Default::default()
        },
        brassclaw_llm::ResponseCachePolicy::default(),
    )
    .unwrap();
    config.cheap_model = Some("must-not-select-this-unrelated-model".into());
    config.smart_routing_cascade = true;
    let session_directory = tempfile::tempdir().unwrap();
    let session = Arc::new(brassclaw_llm::SessionManager::new(
        brassclaw_llm::SessionConfig {
            auth_base_url: format!("http://{address}/unused-auth"),
            session_path: session_directory.path().join("session.json"),
        },
    ));
    let provider = brassclaw_llm::build_static_provider_chain(&config, session)
        .await
        .unwrap();
    let gateway = Arc::new(
        LlmProviderModelGateway::with_provider_identity(
            "native-model",
            provider,
            LlmModelProfilePolicy::new().allow_model_profile(profile, Some(model)),
        )
        .with_max_output_tokens(None),
    );
    let system_gateway = gateway.clone();
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
    .with_hook_dispatcher_builder_factory(|| {
        Ok(brassclaw_hooks::dispatch::HookDispatcherBuilder::new(
            brassclaw_hooks::registry::HookRegistry::new(),
        ))
    })
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
    let mut uncertain_args = args.clone();
    uncertain_args["prompt"]["user_query"] =
        json!("Return a short confirmation for the transport-loss case.");
    let (uncertain, failed) = invoke(
        &host,
        &admitted.admission,
        &program,
        &runtime,
        "0:14",
        uncertain_args.clone(),
    )
    .await;
    assert!(matches!(failed, RuntimeCapabilityOutcome::Failed(_)));
    assert_eq!(
        relay_state.calls.load(Ordering::SeqCst),
        1,
        "configured retry/cascade must not resubmit an uncertain model completion"
    );
    assert_eq!(
        relay_state.actual_responses.lock().await.len(),
        1,
        "one real model response was lost at the transport boundary"
    );
    let unknown = rig.pool.get().await.unwrap().query_one("SELECT attempt_count::bigint,phase FROM brassclaw_monty_tool_invocations WHERE invocation_id=$1", &[&uncertain.as_uuid()]).await.unwrap();
    assert_eq!(unknown.get::<_, i64>(0), 1);
    assert_eq!(unknown.get::<_, String>(1), "answered"); // error receipt, never proof of no external effect
    assert!(
        admitted
            .admission
            .begin_tool_invocation(
                uncertain,
                &program,
                "0:14",
                &uncertain_args,
                EXECUTION_DATA_LIMITS
            )
            .await
            .is_err(),
        "a terminal transport error does not authorize replay"
    );
    assert!(
        rig.pool
            .get()
            .await
            .unwrap()
            .query_opt(
                "SELECT 1 FROM brassclaw_monty_model_responses WHERE invocation_id=$1",
                &[&uncertain.as_uuid()]
            )
            .await
            .unwrap()
            .is_none(),
        "never manufacture a received native answer from relay-only observations"
    );
    relay_state.lose_answer.store(false, Ordering::SeqCst);
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
    let brassclaw_turns::run_profile::ParentLoopOutput::AssistantReply(reply) = &actual.output
    else {
        panic!("this assistant-only candidate requires the real assistant output profile")
    };
    crate::validator_component_tests::assert_model_answer_projection(
        serde_json::to_value(&actual).unwrap(),
        &reply.content,
    )
    .await;
    let captured = packets.list_recent(1).await.unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].run_id, context.run_id.to_string());
    let request_evidence: Value = serde_json::from_str(
        captured[0]
            .model_exchange
            .as_ref()
            .unwrap()
            .effective_request_bytes
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(request_evidence["dispatch_policy"], "single_submission");
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
    // Observe the existing system-inference adapter separately: one real model
    // response must retain its actual input/output/cache meter through the
    // neutral work handoff and real PostgreSQL governor. This does not qualify
    // this legacy internal path as a canonical Recipe/kernel/custody cutover.
    {
        use brassclaw_loop_support::{
            GovernorBackedAccountant, ModelGatewayBackedSystemInferencePort, ZeroCostTable,
        };
        use brassclaw_resources::{
            BudgetPeriod, PersistentResourceGovernor, PgResourceGovernorStore, ResourceAccount,
            ResourceGovernor, ResourceLimits,
        };
        use brassclaw_turns::run_profile::{
            LoopModelBudgetAccountant, ModelWorkOutcome, ModelWorkRequest, SystemInferenceIdentity,
            SystemInferencePort, SystemInferenceRequest, SystemInferenceTaskId, SystemPromptSource,
            SystemTaskKind,
        };
        let governor = Arc::new(PersistentResourceGovernor::new(
            PgResourceGovernorStore::new(rig.pool.clone(), context.scope.tenant_id.as_str()),
        ));
        let account = ResourceAccount::user(
            context.scope.tenant_id.clone(),
            context.actor.as_ref().unwrap().user_id.clone(),
        );
        governor
            .set_limit(
                account.clone(),
                ResourceLimits {
                    max_usd: Some(rust_decimal::Decimal::ONE),
                    period: BudgetPeriod::Rolling24h,
                    ..Default::default()
                },
            )
            .unwrap();
        // This explicitly free local fixture tests token observations, not a
        // production provider price catalogue or an inferred LAN discount.
        let accountant = GovernorBackedAccountant::new(governor.clone(), Arc::new(ZeroCostTable));
        let request = SystemInferenceRequest {
            task_id: SystemInferenceTaskId::new(),
            identity: SystemInferenceIdentity {
                task_kind: SystemTaskKind::Compaction,
                prompt_source: SystemPromptSource::Static {
                    prompt_id: "native_system_meter".to_string().try_into().unwrap(),
                },
                system_prompt: "Answer with one short word. Do not request tools.".into(),
            },
            input_text: "Confirm receipt.".into(),
            max_input_tokens: 1000,
            deadline_ms: 120_000,
        };
        let work = ModelWorkRequest::for_system_inference(&context, &request);
        accountant.pre_model_work(&context, &work).await.unwrap();
        accountant.model_work_dispatching(&context, &work).unwrap();
        let before = relay_state.calls.load(Ordering::SeqCst);
        let system = ModelGatewayBackedSystemInferencePort::new(system_gateway, context.clone());
        let mut unknown_request = request.clone();
        unknown_request.task_id = SystemInferenceTaskId::new();
        let result = system.call_system_inference(request).await;
        let response = result.as_ref().unwrap();
        let meter = response
            .usage
            .expect("actual provider meter must reach system inference");
        let actual = relay_state
            .actual_responses
            .lock()
            .await
            .last()
            .unwrap()
            .clone();
        assert_eq!(
            u64::from(meter.input_tokens),
            actual["usage"]["prompt_tokens"].as_u64().unwrap()
        );
        assert_eq!(
            u64::from(meter.output_tokens),
            actual["usage"]["completion_tokens"].as_u64().unwrap()
        );
        assert_eq!(meter.cache_read_input_tokens, 0);
        assert_eq!(meter.cache_creation_input_tokens, 0);
        assert_eq!(relay_state.calls.load(Ordering::SeqCst), before + 1);
        accountant
            .post_model_work(
                &context,
                &work,
                ModelWorkOutcome::from_system_inference_result(&result),
            )
            .await
            .unwrap();
        let reopened = PersistentResourceGovernor::new(PgResourceGovernorStore::new(
            rig.pool.clone(),
            context.scope.tenant_id.as_str(),
        ));
        let state = reopened.account_snapshot(&account).unwrap().unwrap();
        assert_eq!(
            state.ledger.spent.input_tokens,
            u64::from(meter.input_tokens)
        );
        assert_eq!(
            state.ledger.spent.output_tokens,
            u64::from(meter.output_tokens)
        );
        assert_eq!(
            state.ledger.spent.output_bytes,
            response.output_text.len() as u64
        );
        assert_eq!(state.ledger.spent.wall_clock_ms, response.elapsed_ms);
        assert_eq!(
            state.ledger.reserved,
            brassclaw_resources::ResourceTally::default()
        );
        // This is a gateway/DTO observation, not an accounting or canonical
        // activation certificate for unmetered work. The legacy estimated debit
        // path remains separately documented and must not fabricate counters.
        relay_state.omit_usage.store(true, Ordering::SeqCst);
        let before = relay_state.calls.load(Ordering::SeqCst);
        let unmetered = system.call_system_inference(unknown_request).await.unwrap();
        relay_state.omit_usage.store(false, Ordering::SeqCst);
        assert_eq!(relay_state.calls.load(Ordering::SeqCst), before + 1);
        assert_eq!(unmetered.usage, None);
        assert!(!unmetered.output_text.trim().is_empty());
        let actual = relay_state
            .actual_responses
            .lock()
            .await
            .last()
            .unwrap()
            .clone();
        assert!(actual["usage"]["prompt_tokens"].as_u64().unwrap() > 0);
        assert!(actual["usage"]["completion_tokens"].as_u64().unwrap() > 0);
    }
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
    assert_eq!(host.review_dispatch_counts().0, 3);
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
    assert_eq!(host.review_dispatch_counts().0, 4);
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_native_model_response ON brassclaw_monty_model_responses; DROP FUNCTION reject_native_model_response();").await.unwrap();
    responses.reconcile_received_response(third).await.unwrap();
    responses.reconcile_received_response(third).await.unwrap();
    assert!(!responses.has_uncommitted_results());
    assert_eq!(
        host.review_dispatch_counts().0,
        4,
        "reconciliation cannot replay the provider"
    );
    let mut bad = args.clone();
    bad["prompt"]["prefix_reference"] = json!("attempted override");
    let (_, failed) = invoke(&host, &admitted.admission, &program, &runtime, "0:8", bad).await;
    assert!(matches!(failed, RuntimeCapabilityOutcome::Failed(_)));
    assert_eq!(host.review_dispatch_counts().0, 4);
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
    assert_eq!(host.review_dispatch_counts().0, 4);
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
        // Wait for the exact Loop-response write, not an earlier raw-wire or
        // adapter observation now owned by the same native occurrence.
        while !rig.pool.get().await.unwrap().query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND classid=0 AND objid=771888601 AND NOT granted)", &[]
        ).await.unwrap().get::<_, bool>(0) {
            assert!(
                !late_call.is_finished(),
                "model call stopped before its received-response barrier"
            );
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("actual received model response reaches native persistence barrier");
    assert!(responses.has_uncommitted_results());
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
    assert!(
        !pending.calls_settled(),
        "blocked native persistence is not quiescence"
    );
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
    assert_eq!(host.review_dispatch_counts().0, 5);
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
    assert_eq!(relay_state.calls.load(Ordering::SeqCst), 7);
    assert_eq!(relay_state.actual_responses.lock().await.len(), 7);
    stop_relay.cancel();
    relay_server.await.unwrap();
}

/// Separate admitted run: accounting uncertainty must not be cleared merely to
/// continue the independent cases in the preceding native custody fixture.
mod accounting {
    use super::*;
    use brassclaw_loop_support::{GovernorBackedAccountant, ZeroCostTable};
    use brassclaw_resources::{
        PersistentResourceGovernor, PgResourceGovernorStore, ResourceAccount, ResourceGovernor,
    };
    use brassclaw_turns::run_profile::{
        LoopModelBudgetAccountant, LoopModelGatewayError, LoopModelRequest, LoopRunContext,
        ModelCallOutcome, ModelWorkOutcome, ModelWorkRequest,
    };
    use std::sync::Mutex;

    #[derive(Default)]
    struct Observations {
        request: Option<ModelWorkRequest>,
        response: Option<LoopModelResponse>,
    }
    /// Instrument the supported accounting boundary without fabricating any
    /// admission, provider response, usage or settlement result.
    struct ObservedAccountant {
        inner: Arc<GovernorBackedAccountant>,
        observed: Mutex<Observations>,
    }
    impl ObservedAccountant {
        fn response(&self, outcome: &ModelCallOutcome<'_>) {
            if let ModelCallOutcome::Success(response) = outcome {
                self.observed
                    .lock()
                    .unwrap()
                    .response
                    .get_or_insert_with(|| LoopModelResponse::clone(response));
            }
        }
    }
    #[async_trait::async_trait]
    impl LoopModelBudgetAccountant for ObservedAccountant {
        fn isolated_work_accountant(&self) -> Option<Arc<dyn LoopModelBudgetAccountant>> {
            self.inner.isolated_work_accountant()
        }
        async fn pre_model_work(
            &self,
            context: &LoopRunContext,
            request: &ModelWorkRequest,
        ) -> Result<(), LoopModelGatewayError> {
            self.inner.pre_model_work(context, request).await?;
            self.observed
                .lock()
                .unwrap()
                .request
                .get_or_insert_with(|| request.clone());
            Ok(())
        }
        fn model_work_dispatching(
            &self,
            context: &LoopRunContext,
            request: &ModelWorkRequest,
        ) -> Result<(), LoopModelGatewayError> {
            self.inner.model_work_dispatching(context, request)
        }
        async fn post_model_work(
            &self,
            context: &LoopRunContext,
            request: &ModelWorkRequest,
            outcome: ModelWorkOutcome,
        ) -> Result<(), LoopModelGatewayError> {
            self.inner.post_model_work(context, request, outcome).await
        }
        async fn post_model_work_result(
            &self,
            context: &LoopRunContext,
            request: &ModelWorkRequest,
            outcome: ModelCallOutcome<'_>,
        ) -> Result<(), LoopModelGatewayError> {
            self.response(&outcome);
            self.inner
                .post_model_work_result(context, request, outcome)
                .await
        }
        async fn post_model_call(
            &self,
            context: &LoopRunContext,
            request: &LoopModelRequest,
            outcome: ModelCallOutcome<'_>,
        ) -> Result<(), LoopModelGatewayError> {
            self.response(&outcome);
            self.inner.post_model_call(context, request, outcome).await
        }
        async fn reconcile_retained_model_work(
            &self,
            context: &LoopRunContext,
            request: &ModelWorkRequest,
            response: &LoopModelResponse,
        ) -> Option<Result<(), LoopModelGatewayError>> {
            self.inner
                .reconcile_retained_model_work(context, request, response)
                .await
        }
        fn release_in_flight(&self, context: &LoopRunContext, request: &ModelWorkRequest) {
            self.inner.release_in_flight(context, request);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires the explicit operator-selected real model endpoint and model ID"]
    async fn native_model_accounting_failure_retains_answer_and_settles_original_hold_without_replay()
     {
        use brassclaw_llm::{config::RegistryProviderConfig, registry::ProviderProtocol};
        let endpoint = std::env::var("BRASSCLAW_TEST_MODEL_URL").expect("explicit live endpoint");
        let model = std::env::var("BRASSCLAW_TEST_MODEL_ID").expect("explicit live model");
        let rig = crate::runtime::test_pg::pg_rig().await;
        let store = PgComponentRevisionStore::new(rig.pool.clone());
        let program = program(&store).await;
        let (admitted, mut claimed) = admission_fixture::reserve_with_claim(
            rig.pool.clone(),
            "real native model accounting candidate",
        )
        .await;
        let route = LoopModelRouteSnapshot::try_new(
            "native-model",
            &model,
            "config:native-accounting-v1",
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
        let context = admitted.context.clone().with_resolved_model_route(route);
        let (stop_heartbeat, mut heartbeat_stop) = tokio::sync::watch::channel(false);
        let heartbeat_store = admitted.state.clone();
        let heartbeat_request = brassclaw_turns::runner::HeartbeatRequest {
            run_id: context.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
        };
        let heartbeat = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = heartbeat_stop.changed() => {
                        if changed.is_err() || *heartbeat_stop.borrow() { return Ok::<(), brassclaw_turns::TurnError>(()); }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                        heartbeat_store.heartbeat(heartbeat_request.clone()).await?;
                    }
                }
            }
        }));
        let config = brassclaw_llm::build_llm_config_with_provider_policy(
            brassclaw_llm::ResolvedProviderConfig::Registry(RegistryProviderConfig::generic(
                ProviderProtocol::OpenAiCompletions,
                "native-model",
                None,
                endpoint.clone(),
                model.clone(),
            )),
            brassclaw_llm::ProviderRequestPolicy {
                max_retries: 3,
                ..Default::default()
            },
            brassclaw_llm::ResponseCachePolicy::default(),
        )
        .unwrap();
        let session_directory = tempfile::tempdir().unwrap();
        let session = Arc::new(brassclaw_llm::SessionManager::new(
            brassclaw_llm::SessionConfig {
                auth_base_url: format!("{}/unused-auth", endpoint.trim_end_matches('/')),
                session_path: session_directory.path().join("session.json"),
            },
        ));
        let provider = brassclaw_llm::build_static_provider_chain(&config, session)
            .await
            .unwrap();
        let gateway = Arc::new(
            LlmProviderModelGateway::with_provider_identity(
                "native-model",
                provider,
                LlmModelProfilePolicy::new().allow_model_profile(
                    context.resolved_run_profile.model_profile_id.clone(),
                    Some(model.clone()),
                ),
            )
            .with_max_output_tokens(None),
        );
        let selected = ModelRoute::new("native-model", &model).unwrap();
        let resolver = Arc::new(
            StaticModelRouteResolver::new(
                ModelRoutePolicy::new(ModelSelectionMode::ManagedOnly)
                    .with_approved_route(selected.clone()),
            )
            .with_route(ModelSlot::Default, selected),
        );
        let governor = Arc::new(PersistentResourceGovernor::new(
            PgResourceGovernorStore::new(rig.pool.clone(), context.scope.tenant_id.as_str()),
        ));
        // The explicit local/free pricing policy still reserves and records
        // token usage; it is not a claim about third-party provider billing.
        let accountant = Arc::new(GovernorBackedAccountant::new(
            governor.clone(),
            Arc::new(ZeroCostTable),
        ));
        let observed = Arc::new(ObservedAccountant {
            inner: accountant.clone(),
            observed: Mutex::new(Observations::default()),
        });
        let thread_scope = ThreadScope {
            tenant_id: context.scope.tenant_id.clone(),
            agent_id: context.scope.agent_id.clone().unwrap(),
            project_id: context.scope.project_id.clone(),
            owner_user_id: None,
        };
        brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
            include_str!("../../brassclaw_loop_support/prompts/compaction_summarizer_fresh.md")
                .into(),
        );
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
        .with_model_budget_accountant(observed.clone())
        .with_hook_dispatcher_builder_factory(|| {
            Ok(brassclaw_hooks::dispatch::HookDispatcherBuilder::new(
                brassclaw_hooks::registry::HookRegistry::new(),
            ))
        })
        .with_interceptor_store(Arc::new(brassclaw_interceptor::PgInterceptorStore::new(
            rig.pool.clone(),
            context.scope.tenant_id.as_str(),
        )))
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
            brassclaw_host_runtime::CapabilitySurfaceVersion::new("native-accounting").unwrap(),
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
        let prefix_ref = "candidate:accounting-prefix:immutable-v1";
        let responses = admitted.admission.model_response_store(prefix_ref.into());
        let runtime = crate::monty_kernel::capture(&services)
            .unwrap()
            .model(
                host.clone(),
                RecipeModelPrefix::new(
                    prefix_ref.into(),
                    Arc::from("Answer the user with one brief sentence."),
                )
                .unwrap(),
                responses.clone(),
                live,
            )
            .unwrap();
        rig.pool.get().await.unwrap().batch_execute("CREATE FUNCTION reject_observed_model_settlement() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF EXISTS (SELECT 1 FROM jsonb_array_elements(NEW.payload->'state'->'reservations') AS r WHERE r->1->>'status'='Reconciled') THEN RAISE EXCEPTION 'deliberate observed model settlement rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_observed_model_settlement BEFORE UPDATE ON brassclaw_resource_accounts FOR EACH ROW EXECUTE FUNCTION reject_observed_model_settlement();").await.unwrap();
        let args = json!({"prompt":{"user_query":"Give one short sentence for this accounting test.","chat_history":[]}});
        let (invocation, outcome) = invoke(
            &host,
            &admitted.admission,
            &program,
            &runtime,
            "0:2",
            args.clone(),
        )
        .await;
        assert!(
            matches!(outcome, RuntimeCapabilityOutcome::Failed(_)),
            "actual settlement rejection must fail the operation"
        );
        let (work, response) = {
            let observed = observed.observed.lock().unwrap();
            (
                observed.request.clone().unwrap(),
                observed.response.clone().unwrap(),
            )
        };
        assert_eq!(
            work.work_id.unwrap().as_uuid(),
            invocation.as_uuid(),
            "budget work keeps the exact admitted native invocation identity"
        );
        let usage = response.usage.expect("actual model-reported usage");
        assert_eq!(response.effective_model_profile_id, work.model_profile_id);
        assert!(usage.input_tokens > 0 && usage.output_tokens > 0);
        let row = rig.pool.get().await.unwrap().query_one(
            "SELECT prefix_reference,response_bytes FROM brassclaw_monty_model_responses WHERE invocation_id=$1", &[&invocation.as_uuid()],
        ).await.unwrap();
        assert_eq!(row.get::<_, String>(0), prefix_ref);
        assert_eq!(
            serde_json::from_str::<LoopModelResponse>(&row.get::<_, String>(1)).unwrap(),
            response
        );
        assert!(!responses.has_uncommitted_results());
        let account = ResourceAccount::tenant(context.scope.tenant_id.clone());
        let held = governor.account_snapshot(&account).unwrap().unwrap().ledger;
        assert!(held.reserved.input_tokens > 0 && held.reserved.output_tokens > 0);
        assert_eq!(held.spent.input_tokens, 0);
        assert_eq!(held.spent.output_tokens, 0);
        assert_eq!(host.review_dispatch_counts().0, 1);
        assert!(
            admitted
                .admission
                .begin_tool_invocation(
                    InvocationId::new(),
                    &program,
                    "0:2",
                    &args,
                    EXECUTION_DATA_LIMITS
                )
                .await
                .is_err()
        );
        assert_eq!(host.review_dispatch_counts().0, 1);
        rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_observed_model_settlement ON brassclaw_resource_accounts; DROP FUNCTION reject_observed_model_settlement();").await.unwrap();
        accountant
            .reconcile_retained_model_work(&context, &work, &response)
            .await
            .unwrap()
            .unwrap();
        let settled = governor.account_snapshot(&account).unwrap().unwrap().ledger;
        assert_eq!(settled.reserved.input_tokens, 0);
        assert_eq!(settled.reserved.output_tokens, 0);
        assert_eq!(settled.spent.input_tokens, u64::from(usage.input_tokens));
        assert_eq!(settled.spent.output_tokens, u64::from(usage.output_tokens));
        assert_eq!(settled.spent.usd, rust_decimal::Decimal::ZERO);
        let reopened = PersistentResourceGovernor::new(PgResourceGovernorStore::new(
            rig.pool.clone(),
            context.scope.tenant_id.as_str(),
        ));
        let after = reopened.account_snapshot(&account).unwrap().unwrap().ledger;
        assert_eq!(after.spent, settled.spent);
        assert_eq!(after.reserved, settled.reserved);
        responses
            .reconcile_received_response(invocation)
            .await
            .unwrap();
        assert_eq!(host.review_dispatch_counts().0, 1);
        host.fence_dispatch();
        stop_heartbeat.send(true).unwrap();
        heartbeat.await.unwrap().unwrap();
    }
}
