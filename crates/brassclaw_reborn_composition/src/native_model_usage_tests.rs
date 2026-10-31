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
use sha2::Digest as _;
use uuid::Uuid;

use crate::{native_admission_fixture as admission_fixture, pg_monty_admission};

/// Observe the actual gateway's Rust-owned evidence; all dispatch, retention,
/// custody and capability behavior is delegated unchanged to the real gateway.
type ObservedRejectionSummaries = Arc<std::sync::Mutex<Vec<(brassclaw_turns::TurnRunId, String)>>>;
struct ObservedSubmissionGateway {
    inner: Arc<dyn brassclaw_loop_support::HostManagedModelGateway>,
    errors: Arc<std::sync::Mutex<Vec<(brassclaw_turns::TurnRunId, bool)>>>,
    rejection_summaries: Option<ObservedRejectionSummaries>,
}
#[async_trait::async_trait]
impl brassclaw_loop_support::HostManagedModelGateway for ObservedSubmissionGateway {
    async fn capture_task_model_selection(
        &self,
        parent: Option<Arc<dyn brassclaw_turns::run_profile::LoopTaskModelSelection>>,
        capture: Option<Arc<dyn brassclaw_turns::run_profile::LoopModelCaptureContext>>,
    ) -> Result<
        Option<brassclaw_loop_support::HostManagedTaskModelSelection>,
        brassclaw_loop_support::HostManagedModelError,
    > {
        Ok(self
            .inner
            .capture_task_model_selection(parent, capture.clone())
            .await?
            .map(
                |selected| brassclaw_loop_support::HostManagedTaskModelSelection {
                    binding: selected.binding,
                    gateway: Arc::new(Self {
                        inner: selected.gateway,
                        errors: self.errors.clone(),
                        rejection_summaries: self.rejection_summaries.clone(),
                    }),
                },
            ))
    }

    async fn preflight_task_model_usage(
        &self,
        input: Option<&brassclaw_turns::run_profile::LoopModelInputReference>,
    ) -> Result<(), brassclaw_loop_support::HostManagedModelError> {
        self.inner.preflight_task_model_usage(input).await
    }

    async fn retain_model_selection(
        &self,
        request: &brassclaw_loop_support::HostManagedModelRequest,
    ) -> Result<
        Option<Arc<dyn brassclaw_loop_support::HostManagedModelGateway>>,
        brassclaw_loop_support::HostManagedModelError,
    > {
        Ok(self
            .inner
            .retain_model_selection(request)
            .await?
            .map(|inner| {
                Arc::new(Self {
                    inner,
                    errors: self.errors.clone(),
                    rejection_summaries: self.rejection_summaries.clone(),
                }) as Arc<dyn brassclaw_loop_support::HostManagedModelGateway>
            }))
    }
    fn supports_provider_observation(&self) -> bool {
        self.inner.supports_provider_observation()
    }
    fn supports_tool_exchange(&self) -> bool {
        self.inner.supports_tool_exchange()
    }
    async fn stream_model(
        &self,
        request: brassclaw_loop_support::HostManagedModelRequest,
    ) -> Result<
        brassclaw_loop_support::HostManagedModelResponse,
        brassclaw_loop_support::HostManagedModelError,
    > {
        self.inner.stream_model(request).await
    }
    async fn stream_model_with_capabilities(
        &self,
        request: brassclaw_loop_support::HostManagedModelRequest,
        capabilities: Arc<dyn brassclaw_turns::run_profile::LoopCapabilityPort>,
    ) -> Result<
        brassclaw_loop_support::HostManagedModelResponse,
        brassclaw_loop_support::HostManagedModelError,
    > {
        self.inner
            .stream_model_with_capabilities(request, capabilities)
            .await
    }
    async fn stream_model_observed(
        &self,
        request: brassclaw_loop_support::HostManagedModelRequest,
        capabilities: Option<Arc<dyn brassclaw_turns::run_profile::LoopCapabilityPort>>,
        custodian: Arc<dyn brassclaw_turns::run_profile::LoopModelResponseCustodian>,
    ) -> Result<
        brassclaw_loop_support::HostManagedModelResponse,
        brassclaw_loop_support::HostManagedModelError,
    > {
        let run = request.run_id;
        let result = self
            .inner
            .stream_model_observed(request, capabilities, custodian)
            .await;
        if let Err(error) = &result {
            self.errors
                .lock()
                .unwrap()
                .push((run, error.confirmed_unsubmitted()));
            if let Some(summaries) = &self.rejection_summaries {
                summaries
                    .lock()
                    .unwrap()
                    .push((run, error.safe_summary.clone()));
            }
        }
        result
    }
}

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
        .await
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
    program_with_model_input(store, None).await
}
pub(crate) async fn program_with_model_input(
    store: &PgComponentRevisionStore,
    model_input: Option<brassclaw_skills::recipe_model_input::RecipeModelInput>,
) -> Arc<RetainedToolProgram> {
    let (root, code, binding, tool, skill) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let mut prompt_schema =
        brassclaw_skills::model_completion_components::model_prompt_result_schema();
    prompt_schema["required"] = json!(true);
    let inputs = json!({"prompt":prompt_schema});
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
    for (id, class, mut doc, deps, association) in docs {
        if class == 21
            && let Some(model_input) = &model_input
        {
            doc["model_input_bindings"] = json!({"format":"recipe-model-input-bindings/1",
                "variants":{"selected":model_input}});
        }
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
    let submission_errors = Arc::new(std::sync::Mutex::new(Vec::new()));
    let gateway = Arc::new(ObservedSubmissionGateway {
        inner: gateway,
        errors: submission_errors.clone(),
        rejection_summaries: None,
    });
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
        submission_errors.lock().unwrap().as_slice(),
        &[(context.run_id, false)],
        "a real transmitted/charged completion cannot be marked unsubmitted after its answer is lost"
    );
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
    // A separate actual run reaches the final submission fence after all
    // prompt/provider preparation. Hold its invocation row, cancel the real
    // claim while that preparation waits, then release it. No upstream model
    // request may start, and no response/counter is manufactured for this case.
    let (fenced, mut fenced_claim) = admission_fixture::reserve_with_claim(
        rig.pool.clone(),
        "cancel before actual provider submission",
    )
    .await;
    let route = context.resolved_model_route.clone().unwrap();
    fenced_claim.state = fenced
        .state
        .record_model_route_snapshot(RecordModelRouteSnapshotRequest {
            run_id: fenced.context.run_id,
            runner_id: fenced_claim.runner_id,
            lease_token: fenced_claim.lease_token,
            snapshot: route.clone(),
        })
        .await
        .unwrap();
    let fenced_context = fenced.context.clone().with_resolved_model_route(route);
    let fenced_host = Arc::new(MontyTaskHost::new(
        MontyTaskHandoff::new(
            AgentLoopDriverRunRequest {
                turn_id: fenced_context.turn_id,
                run_id: fenced_context.run_id,
                resolved_run_profile: fenced_context.resolved_run_profile.clone(),
            },
            brassclaw_turns::run_profile::MontyTaskAttempt {
                run_id: fenced_context.run_id,
                runner_id: fenced_claim.runner_id,
                lease_token: fenced_claim.lease_token,
            },
            Arc::new(
                factory
                    .build_text_only_host(RebornLoopDriverHostRequest {
                        claimed_run: fenced_claim,
                        loop_run_context: fenced_context.clone(),
                    })
                    .await
                    .unwrap(),
            ),
        )
        .unwrap(),
    ));
    let fenced_responses = fenced.admission.model_response_store(prefix_ref.into());
    let fenced_runtime = crate::monty_kernel::capture(&services)
        .unwrap()
        .model(
            fenced_host.clone(),
            RecipeModelPrefix::new(prefix_ref.into(), prefix.clone()).unwrap(),
            fenced_responses.clone(),
            live.clone(),
        )
        .unwrap();
    let execution = execution_context(&fenced_host);
    let fenced_id = execution.invocation_id;
    let fenced_args = json!({"prompt":{"user_query":"This cancelled input must never reach the provider.","chat_history":[]}});
    let fenced_record = fenced
        .admission
        .begin_tool_invocation(
            fenced_id,
            &program,
            "0:2",
            &fenced_args,
            EXECUTION_DATA_LIMITS,
        )
        .await
        .unwrap();
    let mut gate = rig.pool.get().await.unwrap();
    let gate_pid: i32 = gate
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let held_row = gate.transaction().await.unwrap();
    held_row.query_one("SELECT invocation_id FROM brassclaw_monty_tool_invocations WHERE invocation_id=$1 FOR UPDATE",
        &[&fenced_id.as_uuid()]).await.unwrap();
    let pending = {
        let host = fenced_host.clone();
        tokio::spawn(async move {
            host.invoke_retained_tool(
                &fenced_runtime,
                RuntimeCapabilityRequest::new(
                    execution,
                    fenced_runtime.descriptor().id.clone(),
                    ResourceEstimate::default(),
                    fenced_args,
                    trust(),
                ),
            )
            .await
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !rig.pool.get().await.unwrap().query_one(
            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE query LIKE 'SELECT phase FROM brassclaw_monty_tool_invocations%' AND $1=ANY(pg_blocking_pids(pid)))",
            &[&gate_pid],
        ).await.unwrap().get::<_, bool>(0) {
            assert!(!pending.is_finished(), "native model call must reach its real pre-submission row barrier");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("native transport reaches final claim check after preparation");
    fenced
        .state
        .request_cancel(brassclaw_turns::CancelRunRequest {
            scope: fenced_context.scope.clone(),
            actor: fenced_context.actor.clone().unwrap(),
            run_id: fenced_context.run_id,
            reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
            idempotency_key: brassclaw_turns::IdempotencyKey::new(
                "native-model-cancel-before-wire",
            )
            .unwrap(),
        })
        .await
        .unwrap();
    held_row.commit().await.unwrap();
    drop(gate);
    match pending.await.unwrap() {
        Ok(RuntimeCapabilityOutcome::Failed(_)) => {}
        Err(error) => assert_eq!(
            error.kind,
            brassclaw_turns::run_profile::AgentLoopHostErrorKind::Cancelled
        ),
        _ => panic!("cancelled native model submission cannot complete"),
    }
    assert!(!fenced_responses.has_uncommitted_results());
    assert!(
        submission_errors
            .lock()
            .unwrap()
            .contains(&(fenced_context.run_id, true)),
        "only the denied final transport barrier supplies confirmed-unsubmitted evidence"
    );
    let observations: i64 = rig.pool.get().await.unwrap().query_one(
        "SELECT (SELECT count(*) FROM brassclaw_monty_provider_wire_observations WHERE invocation_id=$1) + (SELECT count(*) FROM brassclaw_monty_provider_observations WHERE invocation_id=$1) + (SELECT count(*) FROM brassclaw_monty_model_responses WHERE invocation_id=$1)",
        &[&fenced_id.as_uuid()],
    ).await.unwrap().get(0);
    assert_eq!(observations, 0);
    assert_eq!(
        relay_state.calls.load(Ordering::SeqCst),
        7,
        "cancel during preparation prevents any new HTTP submission"
    );
    assert_eq!(relay_state.actual_responses.lock().await.len(), 7);
    fenced_record
        .record_answer(&brassclaw_monty_host::process::PortAnswer::TerminalError {
            reason_kind: "model_attempt_cancelled_before_submission".into(),
        })
        .await
        .unwrap();
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

/// Ordinary task-host/kernel entry with no legacy route resolver or manually
/// written run route. The actual configured HTTP adapter receives the pinned
/// database selection; the controlled peer is transport evidence, not inference.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[tracing_test::traced_test]
async fn native_task_model_usage_captures_database_selection_without_legacy_route() {
    native_task_model_input_case(None, None, None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[tracing_test::traced_test]
async fn native_recipe_selected_prefix_uses_original_pair_without_base_fallback() {
    native_task_model_input_case(Some("sempai"), None, None).await;
}

/// The values distinguish an actual per-invocation parameter from a hidden
/// constant or the accepted Prefix default. This controlled peer is SDK
/// transport evidence, not an inference-quality or activation certificate.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_recipe_output_setting_reaches_sdk_and_guard_with_token_budgets_disabled() {
    for tokens in [37, 79] {
        native_task_model_input_case(None, Some(tokens), None).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_review_accounting_distinguishes_unsubmitted_and_observed_budget_failure() {
    for known_unsubmitted in [true, false] {
        native_task_model_input_case(None, None, Some(known_unsubmitted)).await;
    }
}

async fn native_task_model_input_case(
    selected_profile: Option<&str>,
    requested_output_tokens: Option<u32>,
    review_accounting_case: Option<bool>,
) {
    use crate::task_model_provider::TaskProviderConfiguration;
    use axum::{Json, extract::State, routing::post};
    use brassclaw_loop_support::HostManagedModelGateway as _;
    fn sdk_text(content: &Value) -> &str {
        match content {
            Value::String(text) => text,
            Value::Array(parts) => {
                let [part] = parts.as_slice() else {
                    panic!("complete SDK content must be one text part: {parts:?}")
                };
                assert_eq!(part["type"], "text");
                part["text"].as_str().expect("complete SDK text part")
            }
            other => panic!("unexpected SDK content: {other:?}"),
        }
    }
    let calls: Arc<tokio::sync::Mutex<Vec<Value>>> = Default::default();
    async fn completion(
        State(calls): State<Arc<tokio::sync::Mutex<Vec<Value>>>>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        calls.lock().await.push(body.clone());
        let mut finish_reason = "stop";
        let content = if body["model"] == "captured-sempai-model" {
            let message = body["messages"].as_array().unwrap().last().unwrap();
            assert_eq!(message["role"], "user");
            let audit = sdk_text(&message["content"]);
            let volatile: Vec<(String, String)> = serde_json::from_str(audit).unwrap();
            if volatile
                .iter()
                .any(|(_, text)| text.contains("related-review-length-failure"))
            {
                finish_reason = "length";
            }
            serde_json::to_string(&brassclaw_interceptor::SempaiReviewOutcome {
                adjusted_volatile_messages: volatile,
                bridge_messages: vec![],
                composition_summary: "retained role transport".into(),
                proposed_recipe_updates: vec![],
                proposed_intent_examples: vec![],
                settings_adjustments: vec![],
                proposed_components: vec![],
            })
            .unwrap()
        } else {
            "database selected native answer".into()
        };
        Json(
            json!({"id":"native-task-selection","object":"chat.completion","created":1,
            "model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":content},"finish_reason":finish_reason}],
            "usage":{"prompt_tokens":8,"completion_tokens":4,"total_tokens":12}}),
        )
    }
    let (address, stop, server) = loopback_http::spawn(
        axum::Router::new()
            .route("/v1/chat/completions", post(completion))
            .with_state(calls.clone()),
    )
    .await;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = program_with_model_input(
        &store,
        selected_profile.map(|profile| {
            brassclaw_skills::recipe_model_input::RecipeModelInput::Selected {
                profile: profile.into(),
            }
        }),
    )
    .await;
    let (admitted, claimed) = admission_fixture::reserve_unstarted_with_claim(
        rig.pool.clone(),
        "selected database model",
    )
    .await;
    let context = admitted.context.clone();
    assert!(context.resolved_model_route.is_none());
    let tenant = context.scope.tenant_id.as_str();
    let captured_model = format!("modèle/模型/{}", "long_model_".repeat(25));
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "native-task-selection-authenticated-master-fixture".into(),
            tenant,
        )
        .unwrap(),
    ));
    let definition: brassclaw_llm::ProviderDefinition = serde_json::from_value(json!({"id":"task-selected","protocol":"open_ai_completions",
        "default_base_url":format!("http://{address}/v1"),"model_env":"BRASSCLAW_NATIVE_TASK_MODEL",
        "default_model":captured_model,"api_key_required":false,"description":"native task model transport fixture",
        "context_window_tokens":200000,"model_token_limits":{(captured_model.clone()):{"max_prefix_tokens":100000,"max_prompt_tokens":100000}},
        "request_policy":brassclaw_llm::ProviderRequestPolicy { max_retries:0,..Default::default() }})).unwrap();
    crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant)
        .replace_revision(definition.clone(), None)
        .await
        .unwrap();
    let mut reviewer_definition = definition.clone();
    reviewer_definition.id = "task-reviewer".into();
    reviewer_definition.default_model = "captured-sempai-model".into();
    reviewer_definition.model_token_limits =
        serde_json::from_value(json!({"captured-sempai-model":{
        "max_prefix_tokens":100000,"max_prompt_tokens":100000}}))
        .unwrap();
    crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant)
        .replace_revision(reviewer_definition, None)
        .await
        .unwrap();
    let measurement_scope = brassclaw_host_api::ResourceScope {
        tenant_id: context.scope.tenant_id.clone(),
        user_id: context.actor.as_ref().unwrap().user_id.clone(),
        agent_id: context.scope.agent_id.clone(),
        project_id: context.scope.project_id.clone(),
        thread_id: Some(context.scope.thread_id.clone()),
        invocation_id: InvocationId::new(),
    };
    let target = |provider: &str, model: &str| brassclaw_product_workflow::PrefixModelTarget {
        provider_id: provider.into(),
        provider_revision: 1,
        model: model.into(),
        consumer: "native-task-fixture".into(),
        configured_context_window: Some(200000),
    };
    let primary_prefix = crate::prefix_acceptances::tests::qualify_for_existing_provider(
        rig.pool.clone(),
        measurement_scope.clone(),
        target("task-selected", &captured_model),
        selected_profile.unwrap_or("base-prompt"),
    )
    .await;
    let reviewer_prefix = crate::prefix_acceptances::tests::qualify_for_existing_provider(
        rig.pool.clone(),
        measurement_scope,
        target("task-reviewer", "captured-sempai-model"),
        "sempai",
    )
    .await;
    crate::db_config::save_config_changes(
        rig.pool.as_ref(),
        tenant,
        &[
            ("llm.default.provider_id", Some("task-selected")),
            ("llm.default.model", Some(captured_model.as_str())),
            ("llm.sempai.provider_id", Some("task-reviewer")),
            ("llm.sempai.model", Some("captured-sempai-model")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let sources = TaskProviderConfiguration::new(rig.pool.as_ref().clone(), tenant, keys)
        .with_final_encoding(
            Some(primary_prefix.worker.clone()),
            primary_prefix.governor.clone(),
        )
        .with_session(Arc::new(brassclaw_llm::SessionManager::new(
            Default::default(),
        )));
    let live_output_limit = brassclaw_agent_loop::LiveTokenBudget::new(None);
    let gateway = Arc::new(
        LlmProviderModelGateway::new(
            Arc::new(crate::runtime::PlaceholderLlmProvider),
            LlmModelProfilePolicy::new()
                .allow_model_profile(context.resolved_run_profile.model_profile_id.clone(), None),
        )
        .with_task_source(sources.role(brassclaw_llm::ProviderRole::Kohai))
        .with_max_output_tokens(Some(live_output_limit.clone())),
    );
    let rejected_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let gateway = Arc::new(ObservedSubmissionGateway {
        inner: gateway,
        errors: Default::default(),
        rejection_summaries: Some(rejected_requests.clone()),
    });
    let sempai = Arc::new(
        LlmProviderModelGateway::new(
            Arc::new(crate::runtime::PlaceholderLlmProvider),
            LlmModelProfilePolicy::new().allow_model_profile(
                brassclaw_turns::run_profile::ModelProfileId::new("sempai_model").unwrap(),
                None,
            ),
        )
        .with_task_source(sources.role(brassclaw_llm::ProviderRole::Sempai))
        .with_total_input_tokens(
            (review_accounting_case == Some(true))
                .then(|| brassclaw_agent_loop::LiveTokenBudget::new(Some(1))),
        ),
    );
    let live_mode = brassclaw_interceptor::SharedInterceptorMode::new();
    live_mode.set_rerouting();
    brassclaw_reborn::loop_driver_host::init_sempai_persona(
        include_str!("../../brassclaw_engine/prompts/sempai_audit.md").into(),
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
    let accounting = review_accounting_case.map(|_| {
        let events = Arc::new(brassclaw_resources::InMemoryBudgetEventSink::new());
        let governor = Arc::new(
            brassclaw_resources::InMemoryResourceGovernor::new().with_event_sink(events.clone()),
        );
        (governor, events)
    });
    let mut factory = RebornLoopDriverHostFactory::new(
        Arc::new(PgSessionThreadService::new(rig.pool.clone(), tenant)),
        thread_scope,
        gateway,
        Arc::new(InMemoryCheckpointStateStore::default()),
        admitted.state.clone(),
        Arc::new(InMemoryLoopCheckpointStore::default()),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
        TextOnlyLoopHostConfig {
            max_messages: 8,
            require_model_route_snapshot: false,
        },
        InstructionSafetyContext::local_development_noop(),
    )
    .with_token_budget_mode(move || review_accounting_case.is_some())
    .with_sempai_gateway(sempai)
    .with_interceptor_mode(live_mode.clone())
    .with_hook_dispatcher_builder_factory(|| {
        Ok(brassclaw_hooks::dispatch::HookDispatcherBuilder::new(
            brassclaw_hooks::registry::HookRegistry::new(),
        ))
    });
    if let Some((governor, _)) = &accounting {
        factory = factory.with_model_budget_accountant(Arc::new(
            brassclaw_loop_support::GovernorBackedAccountant::new(
                governor.clone(),
                Arc::new(brassclaw_loop_support::ZeroCostTable),
            ),
        ));
    }
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
    // Same capture/admission calls used by OwnedGlobalTaskFactory, through the
    // real hook wrapper, host-managed port, thread resolver and interceptor.
    let selected = host
        .capture_task_model_selection(None, None)
        .await
        .unwrap()
        .unwrap();
    admitted
        .admission
        .bind_task_models(Some(selected.retained_metadata().unwrap()))
        .unwrap();
    // GlobalRecipePorts starts the bound admission before the native effect.
    // This direct native caller must perform that same real transition.
    admitted.admission.check_and_start().await.unwrap();
    let prefix = crate::task_model_provider::recipe_prefix_for_instruction(
        &host,
        brassclaw_llm::ProviderRole::Kohai,
        program.inputs().instruction(),
    )
    .unwrap()
    .expect("actual retained Recipe prefix binding");
    let input = prefix.resolve().await.unwrap().input_reference();
    assert_eq!(
        input.selection,
        if selected_profile.is_some() {
            brassclaw_turns::run_profile::RecipePrefixSelection::Selected
        } else {
            brassclaw_turns::run_profile::RecipePrefixSelection::Default
        }
    );
    let conflicting = LlmProviderModelGateway::new(
        Arc::new(crate::runtime::PlaceholderLlmProvider),
        LlmModelProfilePolicy::new().allow_model_profile(
            context.resolved_run_profile.model_profile_id.clone(),
            Some("unrelated-profile-model".into()),
        ),
    )
    .with_task_source(sources.role(brassclaw_llm::ProviderRole::Kohai))
    .capture_task_model_selection(Some(selected.clone()), None)
    .await
    .unwrap()
    .unwrap();
    assert!(
        conflicting
            .gateway
            .preflight_task_model_usage(Some(&input))
            .await
            .is_err(),
        "a profile override cannot replace the original selected task model"
    );
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), tenant);
    let mut incompatible = definition.clone();
    incompatible.default_base_url = Some("http://127.0.0.1:1/v1".into());
    repo.replace_revision(incompatible.clone(), Some(1))
        .await
        .unwrap();
    assert!(
        host.preflight_task_model_usage(Some(&input)).await.is_err(),
        "incompatible live target must reject before any Recipe effect or model submission"
    );
    assert!(calls.lock().await.is_empty());
    repo.replace_revision(definition, Some(2)).await.unwrap();
    host.preflight_task_model_usage(Some(&input)).await.unwrap();
    assert_eq!(
        host.selected_model_route().unwrap().model_id,
        captured_model
    );
    assert!(host.run_context().resolved_model_route.is_none());
    crate::db_config::save_config_key(
        rig.pool.as_ref(),
        tenant,
        "llm.default.model",
        "replacement-model",
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    crate::db_config::save_config_key(
        rig.pool.as_ref(),
        tenant,
        "llm.sempai.model",
        "replacement-sempai",
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    live_mode.set_routing();
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
        brassclaw_host_runtime::CapabilitySurfaceVersion::new("native-task-selected").unwrap(),
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
    host.preflight_task_model_usage(Some(&input)).await.unwrap();
    let missing_program = program_with_model_input(
        &store,
        Some(
            brassclaw_skills::recipe_model_input::RecipeModelInput::Selected {
                profile: "absent-profile".into(),
            },
        ),
    )
    .await;
    let missing = crate::task_model_provider::recipe_prefix_for_instruction(
        &host,
        brassclaw_llm::ProviderRole::Kohai,
        missing_program.inputs().instruction(),
    )
    .unwrap()
    .unwrap();
    assert!(
        missing.resolve().await.is_err(),
        "missing Selected cannot fall back to a ready Base"
    );
    let free_program = program_with_model_input(
        &store,
        Some(
            brassclaw_skills::recipe_model_input::RecipeModelInput::PrefixFreeWorkflow {
                purpose: brassclaw_skills::recipe_model_input::PrefixFreePurpose::PrefixGeneration,
            },
        ),
    )
    .await;
    assert!(!crate::global_recipe_ports::model_input_supported(
        free_program.inputs().instruction()
    ));
    assert!(
        crate::task_model_provider::recipe_prefix_for_instruction(
            &host,
            brassclaw_llm::ProviderRole::Kohai,
            free_program.inputs().instruction(),
        )
        .is_err(),
        "unsupported Free is recognized and blocked, never treated as Default"
    );
    let mut foreign = input.clone();
    foreign.selection = brassclaw_turns::run_profile::RecipePrefixSelection::Selected;
    foreign.reference = format!(
        "prefix-generation:sempai:foreign:qualification:{}",
        Uuid::new_v4()
    );
    assert!(
        host.preflight_task_model_usage(Some(&foreign))
            .await
            .is_err(),
        "a qualification outside the original captured map is not selectable"
    );
    let mut changed_generation = input.clone();
    changed_generation.reference =
        input
            .reference
            .replacen("prefix-generation:", "changed-generation:", 1);
    assert!(
        host.preflight_task_model_usage(Some(&changed_generation))
            .await
            .is_err(),
        "the whole canonical reference must match, not just its approved UUID suffix"
    );
    assert!(
        calls.lock().await.is_empty(),
        "input preflight cannot submit model work"
    );
    if selected_profile.is_some() {
        assert!(
            host.preflight_task_model_usage(None).await.is_err(),
            "the fixture has no Base: explicit Selected must not require or manufacture it"
        );
    }
    let responses = admitted
        .admission
        .selected_model_response_store(prefix.clone(), host.task_model_selection());
    let runtime = crate::monty_kernel::capture(&services)
        .unwrap()
        .model(host.clone(), prefix, responses.clone(), live)
        .unwrap();
    let mut arguments = json!({"prompt":{"user_query":"selected task input","chat_history":[]}});
    if review_accounting_case == Some(false) {
        arguments["prompt"]["user_query"] = json!("related-review-length-failure");
    }
    if let Some(tokens) = requested_output_tokens {
        arguments["prompt"]["max_output_tokens"] = json!(tokens);
    }
    let original_arguments = arguments.clone();
    let (invocation, outcome) = invoke(
        &host,
        &admitted.admission,
        &program,
        &runtime,
        "0:2",
        arguments,
    )
    .await;
    if let Some(known_unsubmitted) = review_accounting_case {
        assert!(!matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
        let (governor, events) = accounting.as_ref().unwrap();
        let account = brassclaw_resources::ResourceAccount::tenant(context.scope.tenant_id.clone());
        let client = rig.pool.get().await.unwrap();
        let row = client.query_one(
            "SELECT work_id,dispatch_count::bigint FROM brassclaw_monty_related_model_work WHERE invocation_id=$1",
            &[&invocation.as_uuid()],
        ).await.unwrap();
        let work_id: Uuid = row.get(0);
        assert_ne!(work_id, invocation.as_uuid());
        let event_log = events.snapshot();
        let reserved: std::collections::BTreeSet<_> = event_log
            .iter()
            .filter_map(|event| match event {
                brassclaw_resources::BudgetEvent::Reserved { reservation, .. } => {
                    Some(reservation.scope.invocation_id.as_uuid())
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            reserved,
            [invocation.as_uuid(), work_id].into_iter().collect(),
            "the actual accountant reserves primary and review separately"
        );
        let released: std::collections::BTreeSet<_> = event_log
            .iter()
            .filter_map(|event| match event {
                brassclaw_resources::BudgetEvent::Released { receipt, .. } => {
                    Some(receipt.scope.invocation_id.as_uuid())
                }
                _ => None,
            })
            .collect();
        assert!(
            released.contains(&invocation.as_uuid()),
            "primary was never submitted"
        );
        assert_eq!(
            released.contains(&work_id),
            known_unsubmitted,
            "only native no-submission evidence can release review spend"
        );
        assert_eq!(
            governor.usage_for(&account),
            Default::default(),
            "no fabricated completed spend"
        );
        if known_unsubmitted {
            assert_eq!(governor.reserved_for(&account), Default::default());
            assert_eq!(row.get::<_, i64>(1), 0);
            assert!(
                calls.lock().await.is_empty(),
                "actual preparation guard rejects before SDK transport"
            );
        } else {
            assert!(
                governor.reserved_for(&account).input_tokens > 0,
                "an actually submitted budget failure keeps its unresolved reservation"
            );
            assert_eq!(row.get::<_, i64>(1), 1);
            assert_eq!(
                calls.lock().await.len(),
                1,
                "only review reached the real SDK peer"
            );
            let reason: String = client.query_one(
                "SELECT o.response_bytes::jsonb#>>'{response,finish_reason}'
                 FROM brassclaw_monty_related_model_observations o JOIN brassclaw_monty_related_model_work w USING(work_id)
                 WHERE w.work_id=$1 AND o.format='provider-model-observation/1'", &[&work_id],
            ).await.unwrap().get(0);
            assert_eq!(
                reason, "length",
                "same budget error category after a real provider response"
            );
        }
        assert!(!responses.has_uncommitted_results());
        stop.cancel();
        server.await.unwrap();
        return;
    }
    let RuntimeCapabilityOutcome::Completed(answer) = outcome else {
        panic!(
            "native model operation must complete with captured database selection: {outcome:?}; submitted requests: {}",
            calls.lock().await.len()
        )
    };
    assert!(
        answer
            .output
            .to_string()
            .contains("database selected native answer")
    );
    let client = rig.pool.get().await.unwrap();
    let original = client.query_one(
        "SELECT arguments_bytes,attempt_count::bigint FROM brassclaw_monty_tool_invocations WHERE invocation_id=$1",
        &[&invocation.as_uuid()],
    ).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(original.get::<_, &str>(0)).unwrap(),
        original_arguments
    );
    assert_eq!(original.get::<_, i64>(1), 1);
    let row = client.query_one("SELECT selection_bytes FROM brassclaw_monty_recipe_selections WHERE run_id=$1 AND recipe_id=$2",
        &[&context.run_id.as_uuid(), &program.inputs().instruction().recipe().uuid]).await.unwrap();
    let persisted: Value = serde_json::from_str(row.get::<_, &str>(0)).unwrap();
    if let Some(profile) = selected_profile {
        assert_eq!(
            persisted["model_input_binding"],
            json!({"format":"recipe-model-input-selection/1",
            "binding":{"selection":"selected","profile":profile}})
        );
    } else {
        assert!(
            persisted.get("model_input_binding").is_none(),
            "old Default metadata stays original"
        );
    }
    for table in [
        "brassclaw_monty_model_responses",
        "brassclaw_monty_provider_observations",
        "brassclaw_monty_provider_wire_observations",
    ] {
        let row = client
            .query_one(
                &format!("SELECT prefix_reference FROM {table} WHERE invocation_id=$1"),
                &[&invocation.as_uuid()],
            )
            .await
            .unwrap();
        assert_eq!(
            row.get::<_, &str>(0),
            input.reference,
            "all native stages use the original selected reference"
        );
    }
    let related = client
        .query(
        "SELECT work_id,work_bytes,request_bytes,prefix_reference,dispatch_count::bigint,phase,request_checksum
         FROM brassclaw_monty_related_model_work WHERE invocation_id=$1",
            &[&invocation.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(
        related.len(),
        1,
        "the review owns a distinct native work record"
    );
    let related = &related[0];
    let work_id: Uuid = related.get(0);
    assert_ne!(work_id, invocation.as_uuid());
    let work: Value = serde_json::from_str(related.get::<_, &str>(1)).unwrap();
    let review_request: Value = serde_json::from_str(related.get::<_, &str>(2)).unwrap();
    assert_eq!(
        related.get::<_, String>(6),
        hex::encode(sha2::Sha256::digest(related.get::<_, &str>(2).as_bytes())),
        "native address retains the checksum of the complete immutable original request"
    );
    assert_eq!(work["work_id"], work_id.to_string());
    assert_eq!(work["kind"]["kind"], "sempai_review");
    assert_eq!(work["resolved_model_route"]["provider_id"], "task-reviewer");
    assert_eq!(
        work["resolved_model_route"]["model_id"],
        "captured-sempai-model"
    );
    assert_eq!(
        review_request["resolved_model_route"],
        work["resolved_model_route"]
    );
    assert_eq!(review_request["run_id"], context.run_id.to_string());
    assert_eq!(review_request["turn_id"], context.turn_id.to_string());
    assert!(
        review_request["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| {
                message["content"]
                    .as_str()
                    .is_some_and(|text| text.contains("selected task input"))
            })
    );
    assert!(
        related
            .get::<_, &str>(3)
            .starts_with("prefix-generation:sempai:")
    );
    assert_ne!(related.get::<_, &str>(3), input.reference);
    assert_eq!(related.get::<_, i64>(4), 1);
    assert_eq!(related.get::<_, &str>(5), "dispatch_intent");
    let observations = client.query(
        "SELECT format,response_bytes,response_checksum FROM brassclaw_monty_related_model_observations
         WHERE work_id=$1 ORDER BY format", &[&work_id],
    ).await.unwrap();
    assert_eq!(observations.len(), 3);
    for observation in observations {
        let format: &str = observation.get(0);
        let bytes: &str = observation.get(1);
        assert_eq!(
            observation.get::<_, String>(2),
            hex::encode(sha2::Sha256::digest(bytes.as_bytes()))
        );
        let response: Value = serde_json::from_str(bytes).unwrap();
        if format == "loop-model-response/1" {
            assert_eq!(response["effective_model_profile_id"], "sempai_model");
        } else {
            assert_eq!(response["format"], format);
            assert_eq!(response["provider_id"], "task-reviewer");
            assert_eq!(response["model_id"], "captured-sempai-model");
        }
    }
    for sql in [
        "UPDATE brassclaw_monty_related_model_work SET dispatch_count=0 WHERE work_id=$1",
        "UPDATE brassclaw_monty_related_model_work SET request_bytes='{}' WHERE work_id=$1",
        "DELETE FROM brassclaw_monty_related_model_work WHERE work_id=$1",
        "DELETE FROM brassclaw_monty_related_model_observations WHERE work_id=$1",
    ] {
        let failure = client.execute(sql, &[&work_id]).await.unwrap_err();
        assert_eq!(
            failure.code(),
            Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
        );
    }
    drop(client);
    let sent = calls.lock().await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["model"], "captured-sempai-model");
    assert_eq!(sent[1]["model"], captured_model);
    if let Some(tokens) = requested_output_tokens {
        let sdk_output = sent[1]
            .get("max_completion_tokens")
            .or_else(|| sent[1].get("max_tokens"));
        assert_eq!(
            sdk_output,
            Some(&json!(tokens)),
            "the original native argument must reach the actual SDK request unchanged"
        );
        let review_output = sent[0]
            .get("max_completion_tokens")
            .or_else(|| sent[0].get("max_tokens"))
            .and_then(Value::as_u64)
            .expect("review retains its own actual accepted Prefix output setting");
        assert!(review_output > 0);
        assert_ne!(
            review_output,
            u64::from(tokens),
            "never copy the Kohai output request into review work"
        );
    }
    let primary_system = sdk_text(&sent[1]["messages"][0]["content"]);
    let review_system = sdk_text(&sent[0]["messages"][0]["content"]);
    assert!(primary_system.starts_with(&primary_prefix.reference));
    assert_eq!(
        primary_system.matches(&primary_prefix.reference).count(),
        1,
        "Kohai preserves its whole original reference exactly once before dynamic system material"
    );
    assert!(review_system.starts_with(&reviewer_prefix.reference));
    assert_eq!(
        review_system.matches(&reviewer_prefix.reference).count(),
        1,
        "Sempai preserves its own qualified reference exactly once"
    );
    assert!(
        review_system
            .strip_prefix(&reviewer_prefix.reference)
            .unwrap()
            .contains(brassclaw_reborn::loop_driver_host::sempai_persona()),
        "SDK-joined review instructions remain after the unchanged fixed reference"
    );
    drop(sent);
    let pending_review = if requested_output_tokens.is_none() {
        let before = calls.lock().await.len();
        let (failed, outcome) = invoke(
            &host,
            &admitted.admission,
            &program,
            &runtime,
            "0:4",
            json!({"prompt":{"user_query":"related-review-length-failure","chat_history":[]}}),
        )
        .await;
        assert!(!matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
        assert_eq!(
            calls.lock().await.len(),
            before + 1,
            "failed review must not submit Kohai"
        );
        let client = rig.pool.get().await.unwrap();
        let counts = client
            .query_one(
                "SELECT (SELECT count(*) FROM brassclaw_monty_related_model_observations o
              JOIN brassclaw_monty_related_model_work w USING(work_id) WHERE w.invocation_id=$1),
              (SELECT count(*) FROM brassclaw_monty_model_responses WHERE invocation_id=$1)",
                &[&failed.as_uuid()],
            )
            .await
            .unwrap();
        assert_eq!(
            counts.get::<_, i64>(0),
            2,
            "actual wire and adapter original survive length rejection"
        );
        assert_eq!(
            counts.get::<_, i64>(1),
            0,
            "never fabricate a primary answer"
        );
        client.batch_execute("CREATE FUNCTION reject_related_provider_original() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.format='provider-model-observation/1' THEN RAISE EXCEPTION 'deliberate related provider write failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_related_provider_original BEFORE INSERT ON brassclaw_monty_related_model_observations FOR EACH ROW EXECUTE FUNCTION reject_related_provider_original();").await.unwrap();
        drop(client);
        let (pending, outcome) = invoke(
            &host, &admitted.admission, &program, &runtime, "0:6",
            json!({"prompt":{"user_query":"retain actual related provider original","chat_history":[]}}),
        ).await;
        assert!(!matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
        assert_eq!(calls.lock().await.len(), before + 2);
        assert!(responses.has_uncommitted_results());
        rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_related_provider_original ON brassclaw_monty_related_model_observations; DROP FUNCTION reject_related_provider_original();").await.unwrap();
        Some(pending)
    } else {
        None
    };
    if requested_output_tokens.is_some() {
        // A schema-valid, positive value that cannot fit the real captured
        // context must fail the existing final request guard before transport.
        // This is a fresh retained occurrence, never a replay of 0:2.
        let (_, outcome) = invoke(
            &host, &admitted.admission, &program, &runtime, "0:4",
            json!({"prompt":{"user_query":"must not send","chat_history":[],"max_output_tokens":u32::MAX}}),
        ).await;
        assert!(!matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
        assert!(
            rejected_requests
                .lock()
                .unwrap()
                .iter()
                .any(|(run, summary)| {
                    *run == context.run_id && summary.starts_with("model input rejected:")
                }),
            "the negative case must reach the actual final request token check, not pass on an unrelated rejection"
        );
        assert_eq!(
            calls
                .lock()
                .await
                .iter()
                .filter(|body| body["model"] == captured_model)
                .count(),
            1,
            "the actual guard rejects an impossible output reservation before the primary peer"
        );
    }
    if let Some(tokens) = requested_output_tokens {
        // A later, separately admitted occurrence sees a newly enabled limit.
        // Reject the explicit original value; never silently send a smaller one.
        live_output_limit.set(Some(usize::try_from(tokens - 1).unwrap()));
        let (_, outcome) = invoke(
            &host, &admitted.admission, &program, &runtime, "0:6",
            json!({"prompt":{"user_query":"must not clamp","chat_history":[],"max_output_tokens":tokens}}),
        ).await;
        assert!(!matches!(outcome, RuntimeCapabilityOutcome::Completed(_)));
        assert!(
            rejected_requests
                .lock()
                .unwrap()
                .iter()
                .any(|(run, summary)| {
                    *run == context.run_id
                        && summary == "explicit output reservation exceeds the enabled token budget"
                }),
            "the existing live limit must reject this original request setting"
        );
        assert_eq!(
            calls
                .lock()
                .await
                .iter()
                .filter(|body| body["model"] == captured_model)
                .count(),
            1,
            "an enabled cap must not clamp and submit the request"
        );
        live_output_limit.set(None);
    }
    let calls_before_incompatible = calls.lock().await.len();
    repo.replace_revision(incompatible, Some(3)).await.unwrap();
    assert!(
        host.preflight_task_model_usage(Some(&input)).await.is_err(),
        "cached clients must receive the same live preflight check"
    );
    assert_eq!(
        calls.lock().await.len(),
        calls_before_incompatible,
        "preflight never performs inference"
    );
    if let Some(pending) = pending_review {
        admitted
            .state
            .request_cancel(brassclaw_turns::CancelRunRequest {
                scope: context.scope.clone(),
                actor: context.actor.clone().unwrap(),
                run_id: context.run_id,
                reason: brassclaw_turns::SanitizedCancelReason::UserRequested,
                idempotency_key: brassclaw_turns::IdempotencyKey::new(
                    "related-original-after-cancellation",
                )
                .unwrap(),
            })
            .await
            .unwrap();
        host.fence_dispatch();
        assert!(host.check_dispatch_open().is_err());
        responses
            .reconcile_received_response(pending)
            .await
            .unwrap();
        responses
            .reconcile_received_response(pending)
            .await
            .unwrap();
        assert!(!responses.has_uncommitted_results());
        assert_eq!(
            calls.lock().await.len(),
            calls_before_incompatible,
            "late original persistence cannot repeat provider work after cancellation"
        );
        let client = rig.pool.get().await.unwrap();
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_related_model_observations o
             JOIN brassclaw_monty_related_model_work w USING(work_id) WHERE w.invocation_id=$1",
                &[&pending.as_uuid()],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            count, 2,
            "received original is reconciled; missing loop success stays absent"
        );
    }
    stop.cancel();
    server.await.unwrap();
}

/// Real component selection + encrypted provider capture + admission SQL.
/// The Prefix table is deliberately unavailable to prove capture independence;
/// no model call or prefix-free activation is accepted by this fixture.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_prefix_free_capture_retains_original_pair_without_prefix_table_or_dispatch_grant() {
    use brassclaw_engine::executor::{
        retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram,
    };
    use brassclaw_skills::recipe_model_input::{PrefixFreePurpose, RecipeModelInput};
    use brassclaw_turns::run_profile::{LoopModelInputReference, RecipePrefixSelection};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = program_with_model_input(
        &store,
        Some(RecipeModelInput::PrefixFreeWorkflow {
            purpose: PrefixFreePurpose::PrefixGeneration,
        }),
    )
    .await;
    let inspected = InspectedRetainedProgram::inspect(
        RetainedProgram::Tools(program.clone()),
        &brassclaw_monty_host::process::installed_worker().unwrap(),
    )
    .await
    .unwrap();
    let (admitted, _claim) = admission_fixture::reserve_unstarted_with_claim(
        rig.pool.clone(),
        "prefix-free capture only",
    )
    .await;
    let tenant = admitted.context.scope.tenant_id.as_str();
    let keys = crate::task_model_provider::tests::keys(&rig.pool, tenant);
    crate::task_model_provider::tests::provider(
        &rig.pool,
        tenant,
        "first-pair",
        "http://127.0.0.1:1/v1",
        None,
    )
    .await;
    crate::task_model_provider::tests::select(
        &rig.pool,
        tenant,
        "kohai",
        "first-pair",
        "original-model",
    )
    .await;
    let configuration = crate::task_model_provider::TaskProviderConfiguration::new(
        rig.pool.as_ref().clone(),
        tenant,
        keys,
    )
    .with_session(Arc::new(brassclaw_llm::SessionManager::new(
        Default::default(),
    )));
    let source = configuration.role(brassclaw_llm::ProviderRole::Kohai);
    let capture = crate::task_model_capture::PrefixFreeCapture::from_program(&inspected, tenant)
        .unwrap()
        .unwrap();
    let other =
        crate::task_model_capture::PrefixFreeCapture::from_program(&inspected, "other-namespace")
            .unwrap()
            .unwrap();
    let normal = source.capture(None, None).await.unwrap();
    assert_eq!(
        normal.retained_metadata().unwrap()["format"],
        "brassclaw-task-provider-binding/2"
    );
    assert!(!normal.matches_capture_context(capture.as_ref()));
    assert!(
        source
            .capture(Some(normal), Some(capture.clone()))
            .await
            .is_err()
    );
    assert!(
        source.capture(None, Some(other)).await.is_err(),
        "native namespace must agree"
    );
    struct ForeignCapture;
    impl brassclaw_turns::run_profile::LoopModelCaptureContext for ForeignCapture {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    assert!(
        source
            .capture(None, Some(Arc::new(ForeignCapture)))
            .await
            .is_err()
    );
    rig.pool.get().await.unwrap().batch_execute(
        "ALTER TABLE reborn_prefix_target_bindings RENAME TO temporarily_unavailable_prefix_bindings"
    ).await.unwrap();
    assert!(
        source.capture(None, None).await.is_err(),
        "ordinary capture still needs its binding owner"
    );
    let selected = source.capture(None, Some(capture.clone())).await.unwrap();
    assert!(selected.matches_capture_context(capture.as_ref()));
    let metadata = selected.retained_metadata().unwrap();
    assert_eq!(metadata["format"], "brassclaw-task-provider-binding/3");
    assert_eq!(metadata["prefix_bindings"], json!([]));
    assert_eq!(metadata["prefix_capture"]["purpose"], "prefix_generation");
    assert_eq!(
        metadata["prefix_capture"]["recipe_uuid"],
        json!(program.inputs().instruction().recipe().uuid)
    );
    assert_eq!(
        metadata["prefix_capture"]["selection_checksum"],
        hex::encode(
            program
                .inputs()
                .instruction()
                .retained_selection()
                .unwrap()
                .checksum()
        )
    );
    crate::task_model_provider::tests::select(
        &rig.pool,
        tenant,
        "kohai",
        "first-pair",
        "replacement-model",
    )
    .await;
    let child = source.capture(Some(selected.clone()), None).await.unwrap();
    assert!(Arc::ptr_eq(&selected, &child));
    let review = configuration
        .role(brassclaw_llm::ProviderRole::Sempai)
        .capture(Some(selected.clone()), Some(capture.clone()))
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&selected, &review));
    assert_eq!(selected.model_route().unwrap().model_id, "original-model");
    assert_eq!(child.retained_metadata().unwrap(), metadata);
    assert_eq!(review.retained_metadata().unwrap(), metadata);
    // Recompute bad subjects so rejection cannot be credited merely to a stale
    // outer checksum. These are observations, not native declaration objects.
    fn rehash(mut value: Value) -> Value {
        use sha2::Digest;
        let mut selection = value["selection"].clone();
        selection.sort_all_objects();
        let base = hex::encode(sha2::Sha256::digest(
            serde_json::to_vec(&selection).unwrap(),
        ));
        let mut capture = value["prefix_capture"].clone();
        capture.sort_all_objects();
        let mut prefixes = value["prefix_bindings"].clone();
        prefixes.sort_all_objects();
        value["checksum"] = json!(hex::encode(sha2::Sha256::digest(
            serde_json::to_vec(&json!([base, prefixes, capture])).unwrap()
        )));
        value
    }
    for (field, value) in [
        ("purpose", json!("unqualified-purpose")),
        ("namespace", json!("foreign-namespace")),
        ("recipe_version", json!(0)),
        ("recipe_checksum", json!("invalid")),
        ("variant", json!("")),
        ("unexpected", json!(true)),
    ] {
        let mut bad = metadata.clone();
        bad["prefix_capture"][field] = value;
        assert!(
            admitted
                .admission
                .bind_task_models(Some(rehash(bad)))
                .is_err(),
            "{field}"
        );
    }
    let mut bad = metadata.clone();
    bad["prefix_bindings"] = json!([{"qualification":Uuid::new_v4()}]);
    assert!(
        admitted
            .admission
            .bind_task_models(Some(rehash(bad)))
            .is_err()
    );
    admitted
        .admission
        .bind_task_models(Some(metadata.clone()))
        .unwrap();
    admitted.admission.check_and_start().await.unwrap();
    let client = rig.pool.get().await.unwrap();
    let persisted: Value = client
        .query_one(
            "SELECT task_model_selection FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&admitted.context.run_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(persisted, metadata);
    assert!(client.execute(
        "UPDATE brassclaw_monty_task_admissions SET task_model_selection=jsonb_set(task_model_selection,'{prefix_capture,purpose}','\"model_design\"') WHERE run_id=$1",
        &[&admitted.context.run_id.as_uuid()],
    ).await.is_err(), "original family is immutable");
    drop(client);
    let free_input = LoopModelInputReference {
        selection: RecipePrefixSelection::PrefixFreeWorkflow,
        reference: "capture-is-not-qualification".into(),
    };
    assert!(
        source
            .preflight_compatibility(&selected, None, None)
            .await
            .is_err()
    );
    assert!(
        source
            .preflight_compatibility(&selected, None, Some(&free_input))
            .await
            .is_err()
    );
    assert!(!crate::global_recipe_ports::model_input_supported(
        program.inputs().instruction()
    ));
    assert!(
        crate::installed_monty_catalogue::program_for_committed_match(
            std::iter::once(&inspected),
            &brassclaw_engine::memory::intent_system::IntentResolution::NoMatch,
        )
        .unwrap()
        .is_none()
    );
}
