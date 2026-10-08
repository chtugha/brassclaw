//! Actual global Python -> admitted Reborn host -> model/transcript ports.
//! PostgreSQL supplies real admission, claims, transcript and interceptor rows.
//! Recording provider fixtures match the original composition acceptance tests;
//! this isolated caller does not certify application boot or ordinary Recipes.
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use brassclaw_host_api::{AgentId, ProjectId, TenantId, ThreadId, UserId};
use brassclaw_interceptor::{InterceptorStore, PacketStatus, PgInterceptorStore};
use brassclaw_loop_support::{
    HostManagedModelError, HostManagedModelGateway, HostManagedModelMessageRole,
    HostManagedModelRequest, HostManagedModelResponse, PgCapabilityIo, SystemBundleSource,
};
use brassclaw_monty_host::{
    service::{TaskInput, TaskOutcome, TaskPorts},
    transport_actor::{ActorLimits, StopKind},
};
use brassclaw_pg::PgPool;
use brassclaw_reborn::{
    loop_driver_host::{
        RebornLoopDriverHostFactory, RebornLoopDriverHostRequest, TextOnlyLoopHostConfig,
    },
    monty_task_host::MontyTaskHost,
};
use brassclaw_resources::{LiveMontyTaskSettings, MontyTaskLimits, MontyTaskSettingsRevision};
use brassclaw_threads::{
    AcceptInboundMessageRequest, AppendAssistantDraftRequest, EnsureThreadRequest, MessageContent,
    PgSessionThreadService, SessionThreadService, SubmittedUserMessageRequest,
    ThreadHistoryRequest, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, DefaultTurnCoordinator, IdempotencyKey, InMemoryCheckpointStateStore,
    InMemoryLoopCheckpointStore, PgTurnStateStore, ReplyTargetBindingRef, SourceBindingRef,
    SubmitTurnRequest, SubmitTurnResponse, TurnActor, TurnCoordinator, TurnLeaseToken,
    TurnRunnerId, TurnScope,
    run_profile::{
        AgentLoopDriverRunRequest, InMemoryLoopHostMilestoneSink, InstructionSafetyContext,
        LoopCapabilityPort, LoopModelGatewayError, LoopModelPolicyGuard, LoopRunContext,
        ModelWorkRequest, MontyTaskAttempt, MontyTaskHandoff, MontyTurnDriverPort, NoOpPolicyGuard,
        ProviderToolCall,
    },
    runner::{ClaimRunRequest, TurnRunTransitionPort},
};
use serde_json::{Value, json};
use sha2::Digest;

#[path = "support/admission_integrity.rs"]
mod admission_integrity;
#[path = "support/capabilities.rs"]
mod capabilities;
#[path = "../../../crates/brassclaw_reborn_composition/src/global_monty_driver.rs"]
mod global_monty_driver;
#[path = "../../../crates/brassclaw_reborn_composition/src/global_monty_owner.rs"]
mod global_monty_owner;
#[path = "../../../crates/brassclaw_reborn_composition/src/global_recipe_ports.rs"]
mod global_recipe_ports;
#[path = "../../../crates/brassclaw_reborn_composition/src/global_root_seed.rs"]
mod global_root_seed;
#[path = "../../../crates/brassclaw_reborn_composition/src/global_task_factory.rs"]
mod global_task_factory;
#[path = "../../../crates/brassclaw_reborn_composition/src/monty_instance_owner.rs"]
mod monty_instance_owner;
#[path = "../../../crates/brassclaw_reborn_composition/src/monty_task_input.rs"]
mod monty_task_input;
#[path = "../../../crates/brassclaw_reborn/tests/common/native_pg.rs"]
pub(crate) mod native_pg;
#[path = "support/reply_workflows.rs"]
mod reply_workflows;
#[path = "support/task_catalogue.rs"]
mod task_catalogue;
// Existing owner cases continue to use the actual native PostgreSQL fixture.
mod runtime {
    pub(crate) mod test_pg {
        pub(crate) use crate::native_pg;
    }
}
#[path = "../../../crates/brassclaw_reborn_composition/src/pg_monty_admission.rs"]
mod pg_monty_admission;
#[path = "support/runtime.rs"]
mod support;
const SOURCE: &str = include_str!("../../../crates/brassclaw_engine/orchestrator/global_mode.py");

fn assert_settlement_outcome(mut actual: Value, expected: Value) -> Value {
    let report = actual
        .as_object_mut()
        .unwrap()
        .remove("execution")
        .expect("actual quiescent execution report");
    assert_eq!(actual, expected);
    assert_eq!(report["format"], "monty-task-execution/1");
    assert_eq!(report["semantic_approval"], false);
    assert_eq!(report["catalogue_activation"], false);
    assert_eq!(report["root_completed"], actual["status"] == "completed");
    if actual["status"] == "completed" {
        assert_eq!(report["all_selected_recipes_complete"], true);
        assert!(report["accounting"]["compute_time"].is_object());
        assert!(report["accounting"]["failure"].is_null());
    }
    report
}

#[derive(Default)]
struct RecordingProvider {
    requests: Mutex<Vec<HostManagedModelRequest>>,
    policy_work: Mutex<Vec<ModelWorkRequest>>,
    use_tools: bool,
    hold: Option<Arc<ProviderHold>>,
}

#[derive(Default)]
struct ProviderHold {
    entered: tokio::sync::Notify,
    released: tokio::sync::Notify,
}

// Observe the actual policy envelope while retaining the fixture's default
// model policy. This wrapper does not dispatch or grant Tools.
struct ObservedModelPolicy(Arc<RecordingProvider>);
#[async_trait]
impl LoopModelPolicyGuard for ObservedModelPolicy {
    async fn check_model_work_policy(
        &self,
        context: &LoopRunContext,
        work: &ModelWorkRequest,
    ) -> Result<(), LoopModelGatewayError> {
        NoOpPolicyGuard
            .check_model_work_policy(context, work)
            .await?;
        self.0.policy_work.lock().unwrap().push(work.clone());
        Ok(())
    }
}
#[async_trait]
impl HostManagedModelGateway for RecordingProvider {
    async fn stream_model(
        &self,
        request: HostManagedModelRequest,
    ) -> Result<HostManagedModelResponse, HostManagedModelError> {
        self.requests.lock().unwrap().push(request);
        if let Some(hold) = &self.hold {
            hold.entered.notify_one();
            hold.released.notified().await;
        }
        Ok(HostManagedModelResponse::assistant_reply(
            "actual scoped reply",
        ))
    }

    async fn stream_model_with_capabilities(
        &self,
        request: HostManagedModelRequest,
        capabilities: Arc<dyn LoopCapabilityPort>,
    ) -> Result<HostManagedModelResponse, HostManagedModelError> {
        if !self.use_tools {
            return self.stream_model(request).await;
        }
        let has_result = request
            .messages
            .iter()
            .any(|message| message.role == HostManagedModelMessageRole::ToolResult);
        self.requests.lock().unwrap().push(request.clone());
        if has_result {
            let result = request
                .messages
                .iter()
                .find(|message| message.role == HostManagedModelMessageRole::ToolResult)
                .unwrap();
            assert!(result.content.contains("workspace-sentinel.txt"));
            let replay = result.tool_result_provider_call.as_ref().unwrap();
            assert_eq!(replay.provider_call_id, "native-call-1");
            assert_eq!(replay.capability_id.as_str(), "builtin.list_dir");
            assert_eq!(
                replay.signature.as_deref(),
                Some("native-provider-signature")
            );
            assert_eq!(
                replay.reasoning.as_deref(),
                Some("inspect the mounted workspace")
            );
            return Ok(HostManagedModelResponse::assistant_reply(
                "actual scoped reply",
            ));
        }
        let tool = capabilities
            .tool_definitions()
            .unwrap()
            .into_iter()
            .find(|tool| tool.capability_id.as_str() == "builtin.list_dir")
            .unwrap();
        let candidate = capabilities
            .register_provider_tool_call(ProviderToolCall {
                provider_id: "native-provider".into(),
                provider_model_id: "native-model".into(),
                turn_id: Some("native-provider-turn".into()),
                id: "native-call-1".into(),
                name: tool.name,
                arguments: json!({"path": "/workspace"}),
                response_reasoning: None,
                reasoning: Some("inspect the mounted workspace".into()),
                signature: Some("native-provider-signature".into()),
            })
            .await
            .unwrap();
        Ok(HostManagedModelResponse::capability_calls(
            vec![candidate],
            "",
        ))
    }
}

// Immutable bytes loaded from an actual file before creating this task host.
struct SelectedPrefix(String);
#[async_trait]
impl SystemBundleSource for SelectedPrefix {
    async fn get_system_bundle(&self, _user_id: &str, _project_id: &str) -> String {
        self.0.clone()
    }
}

struct NativeTaskPortsFactory {
    last_host: Mutex<Option<Arc<MontyTaskHost>>>,
    last_receipt: Mutex<Option<Arc<brassclaw_monty_host::service::TaskReceipt>>>,
    settlement_hold: Mutex<Option<Arc<ProviderHold>>>,
    inner: global_task_factory::OwnedGlobalTaskFactory,
}
struct DraftCatalogueProvider {
    pool: Arc<PgPool>,
    draft: Option<Arc<brassclaw_engine::memory::retained_tools::RetainedToolProgram>>,
    reply: Arc<brassclaw_engine::memory::retained_tools::RetainedToolProgram>,
    block_reply: bool,
}
#[async_trait]
impl global_task_factory::MontyCatalogueProvider for DraftCatalogueProvider {
    async fn capture(
        &self,
        host: Arc<MontyTaskHost>,
        admission: Arc<pg_monty_admission::PgMontyAdmission>,
        _input: &TaskInput,
    ) -> Result<
        Arc<dyn global_recipe_ports::MontyTaskCatalogue>,
        brassclaw_turns::run_profile::AgentLoopDriverError,
    > {
        Ok(Arc::new(
            task_catalogue::ValidationCatalogue::new(
                self.pool.clone(),
                host,
                self.draft.clone(),
                admission,
                self.reply.clone(),
                self.block_reply,
            )
            .await,
        ))
    }
}
impl NativeTaskPortsFactory {
    async fn new(
        pool: Arc<PgPool>,
        ownership: global_monty_owner::GlobalOwnerCheck,
        draft: Option<Arc<brassclaw_engine::memory::retained_tools::RetainedToolProgram>>,
    ) -> Self {
        let reply = reply_workflows::reply_program(pool.clone()).await;
        Self::with_catalogue(
            pool.clone(),
            ownership,
            Arc::new(DraftCatalogueProvider {
                pool,
                draft,
                reply,
                block_reply: false,
            }),
        )
    }

    fn with_catalogue(
        pool: Arc<PgPool>,
        ownership: global_monty_owner::GlobalOwnerCheck,
        catalogue: Arc<dyn global_task_factory::MontyCatalogueProvider>,
    ) -> Self {
        Self {
            last_host: Mutex::new(None),
            last_receipt: Mutex::new(None),
            settlement_hold: Mutex::new(None),
            inner: global_task_factory::OwnedGlobalTaskFactory::new(
                pool.clone(),
                ownership,
                catalogue,
                8,
            )
            .unwrap(),
        }
    }
}
#[async_trait]
impl global_monty_driver::GlobalTaskPortsFactory for NativeTaskPortsFactory {
    async fn build(
        &self,
        host: Arc<MontyTaskHost>,
        input: &TaskInput,
    ) -> Result<Arc<dyn TaskPorts>, brassclaw_turns::run_profile::AgentLoopDriverError> {
        *self.last_host.lock().unwrap() = Some(host.clone());
        global_monty_driver::GlobalTaskPortsFactory::build(&self.inner, host, input).await
    }

    async fn settle(
        &self,
        host: Arc<MontyTaskHost>,
        receipt: Arc<brassclaw_monty_host::service::TaskReceipt>,
    ) -> Result<(), brassclaw_turns::run_profile::AgentLoopDriverError> {
        *self.last_receipt.lock().unwrap() = Some(receipt.clone());
        let hold = self.settlement_hold.lock().unwrap().take();
        if let Some(hold) = hold {
            assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
            hold.entered.notify_one();
            hold.released.notified().await;
        }
        global_monty_driver::GlobalTaskPortsFactory::settle(&self.inner, host, receipt).await
    }
}
async fn admitted(
    pool: Arc<PgPool>,
    provider: Arc<RecordingProvider>,
    name: &str,
    prefix: Arc<SelectedPrefix>,
    history_count: usize,
    settings: LiveMontyTaskSettings,
    tool_root: Option<&std::path::Path>,
) -> (
    TaskInput,
    MontyTaskHandoff,
    Arc<PgSessionThreadService>,
    ThreadScope,
    Arc<PgInterceptorStore>,
) {
    admitted_with_text(
        pool,
        provider,
        AdmissionInput {
            name,
            text: "'quoted' Ü {{vars.literal}}",
        },
        prefix,
        history_count,
        settings,
        tool_root,
    )
    .await
}

struct AdmissionInput<'a> {
    name: &'a str,
    text: &'a str,
}

async fn admitted_with_text(
    pool: Arc<PgPool>,
    provider: Arc<RecordingProvider>,
    input: AdmissionInput<'_>,
    prefix: Arc<SelectedPrefix>,
    history_count: usize,
    settings: LiveMontyTaskSettings,
    tool_root: Option<&std::path::Path>,
) -> (
    TaskInput,
    MontyTaskHandoff,
    Arc<PgSessionThreadService>,
    ThreadScope,
    Arc<PgInterceptorStore>,
) {
    let name = input.name;
    let user_input = input.text;
    let tenant = TenantId::new("native-global-host").unwrap();
    let agent = AgentId::new("global-agent").unwrap();
    let project = ProjectId::new("global-project").unwrap();
    let user = UserId::new("global-operator").unwrap();
    let thread = ThreadId::new(format!("opaque-conversation-{name}")).unwrap();
    let thread_scope = ThreadScope {
        tenant_id: tenant.clone(),
        agent_id: agent.clone(),
        project_id: Some(project.clone()),
        owner_user_id: None,
    };
    let service = Arc::new(PgSessionThreadService::new(pool.clone(), tenant.as_str()));
    service
        .ensure_thread(EnsureThreadRequest {
            scope: thread_scope.clone(),
            thread_id: Some(thread.clone()),
            created_by_actor_id: user.to_string(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    // Persist actual historical transcript fixtures through the same writer.
    // These records are context setup, not simulated Tool/provider effects.
    for index in 0..history_count {
        let content = MessageContent::text(format!("historical context {index}"));
        let draft = service
            .append_assistant_draft(AppendAssistantDraftRequest {
                scope: thread_scope.clone(),
                thread_id: thread.clone(),
                turn_run_id: format!("historical-fixture-{index}"),
                content: content.clone(),
            })
            .await
            .unwrap();
        service
            .finalize_assistant_message(&thread_scope, &thread, draft.message_id, content)
            .await
            .unwrap();
    }
    let accepted = service
        .accept_inbound_message(AcceptInboundMessageRequest {
            scope: thread_scope.clone(),
            thread_id: thread.clone(),
            actor_id: user.to_string(),
            source_binding_id: Some("source-web".into()),
            reply_target_binding_id: Some("reply-web".into()),
            external_event_id: Some(format!("native-event-{name}")),
            content: MessageContent::text(user_input),
        })
        .await
        .unwrap();
    let turn_scope = TurnScope::new(tenant.clone(), Some(agent), Some(project), thread.clone());
    let state = Arc::new(PgTurnStateStore::new(pool.clone(), tenant.as_str()));
    let coordinator = DefaultTurnCoordinator::new(state.clone());
    let submitted = coordinator
        .submit_turn(SubmitTurnRequest {
            scope: turn_scope.clone(),
            actor: TurnActor::new(user.clone()),
            accepted_message_ref: AcceptedMessageRef::new(format!("msg:{}", accepted.message_id))
                .unwrap(),
            source_binding_ref: SourceBindingRef::new("source-web").unwrap(),
            reply_target_binding_ref: ReplyTargetBindingRef::new("reply-web").unwrap(),
            requested_run_profile: None,
            idempotency_key: IdempotencyKey::new(format!("native-{name}")).unwrap(),
            received_at: chrono::Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        })
        .await
        .unwrap();
    let claimed = state
        .claim_next_run(ClaimRunRequest {
            runner_id: TurnRunnerId::new(),
            lease_token: TurnLeaseToken::new(),
            scope_filter: Some(turn_scope.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    let SubmitTurnResponse::Accepted { run_id, .. } = submitted;
    assert_eq!(claimed.state.run_id, run_id);
    service
        .mark_message_submitted(
            &thread_scope,
            &thread,
            accepted.message_id,
            claimed.state.turn_id.to_string(),
            claimed.state.run_id.to_string(),
        )
        .await
        .unwrap();
    let context = LoopRunContext::new(
        turn_scope,
        claimed.state.turn_id,
        claimed.state.run_id,
        claimed.resolved_run_profile.clone(),
    )
    .with_actor(claimed.state.actor.clone().unwrap())
    .with_accepted_message_ref(claimed.state.accepted_message_ref.clone());
    let packets = Arc::new(PgInterceptorStore::new(pool.clone(), tenant.as_str()));
    let io = Arc::new(PgCapabilityIo::new(pool.clone()));
    let mut factory = RebornLoopDriverHostFactory::new(
        service.clone(),
        thread_scope.clone(),
        provider.clone(),
        Arc::new(InMemoryCheckpointStateStore::default()),
        state,
        Arc::new(InMemoryLoopCheckpointStore::default()),
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
        TextOnlyLoopHostConfig {
            max_messages: 8,
            require_model_route_snapshot: false,
        },
        InstructionSafetyContext::local_development_noop(),
    )
    .with_system_bundle_source(prefix)
    .with_token_budget_mode(move || settings.current().limits.token_budgets_enabled)
    .with_interceptor_store(packets.clone());
    factory = factory.with_model_policy_guard(Arc::new(ObservedModelPolicy(provider)));
    if tool_root.is_some() {
        factory = factory.with_tool_result_source(io.clone());
    }
    let request = RebornLoopDriverHostRequest {
        claimed_run: claimed.clone(),
        loop_run_context: context.clone(),
    };
    let host = if let Some(root) = tool_root {
        factory
            .build_text_only_host_with_capabilities(request, capabilities::port(&context, root, io))
            .await
    } else {
        factory.build_text_only_host(request).await
    }
    .unwrap();
    let admitted_input = service
        .submitted_turn_input(
            SubmittedUserMessageRequest {
                scope: thread_scope.clone(),
                thread_id: thread,
                message_id: accepted.message_id,
                turn_id: context.turn_id.to_string(),
                turn_run_id: context.run_id.to_string(),
            },
            None,
        )
        .await
        .unwrap();
    let input = TaskInput {
        conversation_id: context.thread_id.to_string(),
        message_id: admitted_input.message.message_id.to_string(),
        turn_id: context.turn_id.to_string(),
        run_id: context.run_id.to_string(),
        user_input: admitted_input.message.content.unwrap(),
        history: admitted_input
            .prior_context
            .messages
            .into_iter()
            .map(|message| json!({"content": message.content}))
            .collect(),
    };
    let handoff = MontyTaskHandoff::new(
        AgentLoopDriverRunRequest {
            turn_id: context.turn_id,
            run_id: context.run_id,
            resolved_run_profile: context.resolved_run_profile.clone(),
        },
        MontyTaskAttempt {
            run_id: context.run_id,
            runner_id: claimed.runner_id,
            lease_token: claimed.lease_token,
        },
        Arc::new(host),
    )
    .unwrap();
    (input, handoff, service, thread_scope, packets)
}

#[tokio::test]
async fn global_no_match_uses_actual_scoped_model_and_persisted_reply_ports() {
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let directory = tempfile::tempdir().unwrap();
    let prefix_file = directory.path().join("selected-system-prefix.md");
    std::fs::write(&prefix_file, "selected prefix with Ü and {{literal.data}}").unwrap();
    let prefix = Arc::new(SelectedPrefix(
        std::fs::read_to_string(&prefix_file).unwrap(),
    ));
    let provider = Arc::new(RecordingProvider::default());
    let live = LiveMontyTaskSettings::new(MontyTaskSettingsRevision {
        revision: 1,
        limits: MontyTaskLimits {
            max_compute_time: Duration::from_secs(600),
            token_budgets_enabled: false,
        },
    })
    .unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot: support::boot(SOURCE),
            process: support::limits(),
            live: live.clone(),
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: support::limits().max_frame_bytes * 16,
                max_control_unclaimed: 8,
                max_control_reserved_frame_bytes: support::limits().max_frame_bytes * 16,
            },
            queue_capacity: 4,
        },
    )
    .await
    .unwrap();
    let factory =
        NativeTaskPortsFactory::new(database.pool.clone(), owner.ownership_check(), None).await;
    for name in ["A", "B"] {
        let history_count = if name == "A" { 140 } else { 0 };
        let (input, handoff, threads, scope, packets) = admitted(
            database.pool.clone(),
            provider.clone(),
            name,
            prefix.clone(),
            history_count,
            live.clone(),
            None,
        )
        .await;
        let task_host = Arc::new(MontyTaskHost::new(handoff));
        let ports =
            global_monty_driver::GlobalTaskPortsFactory::build(&factory, task_host.clone(), &input)
                .await
                .unwrap();
        if name == "A" {
            threads
                .accept_inbound_message(AcceptInboundMessageRequest {
                    scope: scope.clone(),
                    thread_id: ThreadId::new(input.conversation_id.clone()).unwrap(),
                    actor_id: "global-operator".to_owned(),
                    source_binding_id: None,
                    reply_target_binding_id: None,
                    external_event_id: Some("later-input".to_owned()),
                    content: MessageContent::text("future input must stay out"),
                })
                .await
                .unwrap();
            let surface = task_host
                .dispatch_port("visible_capabilities", json!({}))
                .await
                .unwrap();
            let request = json!({"mode": "text_only", "surface_version": surface["version"],
                "max_messages": 2, "inline_messages": [],
                "capability_view": {"visible_capability_ids": []}});
            for (revision, enabled, expected_count) in
                [(2, false, 141), (3, true, 2), (4, false, 141)]
            {
                let effective = owner
                    .client()
                    .publish_settings(
                        revision - 1,
                        brassclaw_monty_host::process::TaskSettings {
                            revision,
                            max_compute_time: Duration::from_secs(600),
                            token_budgets_enabled: enabled,
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(effective.effective_settings.revision, revision);
                assert_eq!(live.current().revision, revision);
                let bundle = task_host
                    .dispatch_port("build_prompt_bundle", request.clone())
                    .await
                    .unwrap();
                assert_eq!(
                    bundle["compaction_message_index"].as_array().unwrap().len(),
                    expected_count
                );
            }
        }
        let run_id = input.run_id.clone();
        let conversation = ThreadId::new(input.conversation_id.clone()).unwrap();
        let ticket = owner.client().submit(input, ports).unwrap();
        let receipt = tokio::time::timeout(Duration::from_secs(10), ticket.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(&receipt.outcome, TaskOutcome::Completed { .. }));
        assert!(receipt.withheld.is_empty());
        global_monty_driver::GlobalTaskPortsFactory::settle(&factory, task_host, receipt.clone())
            .await
            .unwrap();
        let history = threads
            .list_thread_history(ThreadHistoryRequest {
                scope,
                thread_id: conversation,
            })
            .await
            .unwrap();
        assert_eq!(
            history.messages.len(),
            history_count + 2 + usize::from(name == "A")
        );
        assert_eq!(
            history.messages.last().unwrap().content.as_deref(),
            Some("actual scoped reply")
        );
        let packet = packets
            .list_recent(10)
            .await
            .unwrap()
            .into_iter()
            .find(|packet| packet.run_id == run_id)
            .unwrap();
        assert_eq!(packet.status, PacketStatus::Complete);
        assert!(packet.completed_at.is_some());
    }
    {
        let requests = provider.requests.lock().unwrap();
        assert_work_matches_provider(&provider, &requests);
        assert_eq!(requests.len(), 2);
        for request in requests.iter() {
            assert!(
                request
                    .messages
                    .iter()
                    .any(|message| message.content == prefix.0)
            );
            assert!(
                request
                    .messages
                    .iter()
                    .any(|message| message.content == "'quoted' Ü {{vars.literal}}")
            );
            assert!(
                !request
                    .messages
                    .iter()
                    .any(|message| message.content == "future input must stay out")
            );
        }
        let first = &requests[0];
        assert_eq!(
            first
                .messages
                .iter()
                .filter(|message| message.content.starts_with("historical context "))
                .count(),
            140
        );
    }
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    let exit = exit.service.unwrap();
    assert_eq!(exit.failure, None);
    assert!(exit.tasks.is_empty());
    assert_eq!(exit.transport.unwrap().kind, StopKind::Graceful);
}

#[tokio::test]
async fn global_model_tool_followup_uses_real_mount_kernel_and_retained_output() {
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("workspace-sentinel.txt"),
        "actual workspace bytes",
    )
    .unwrap();
    let provider = Arc::new(RecordingProvider {
        use_tools: true,
        ..Default::default()
    });
    let settings = LiveMontyTaskSettings::new(MontyTaskSettingsRevision {
        revision: 1,
        limits: MontyTaskLimits {
            max_compute_time: Duration::from_secs(600),
            token_budgets_enabled: false,
        },
    })
    .unwrap();
    let (input, handoff, threads, scope, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "tools",
        Arc::new(SelectedPrefix("native tool prefix".into())),
        0,
        settings.clone(),
        Some(root.path()),
    )
    .await;
    let task_host = Arc::new(MontyTaskHost::new(handoff));
    let context = task_host.run_context().clone();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot: support::boot(SOURCE),
            process: support::limits(),
            live: settings,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * 256 * 1024,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * 256 * 1024,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    let factory =
        NativeTaskPortsFactory::new(database.pool.clone(), owner.ownership_check(), None).await;
    let ports =
        global_monty_driver::GlobalTaskPortsFactory::build(&factory, task_host.clone(), &input)
            .await
            .unwrap();
    let receipt = tokio::time::timeout(
        Duration::from_secs(10),
        owner.client().submit(input, ports).unwrap().wait(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(matches!(receipt.outcome, TaskOutcome::Completed { .. }));
    assert!(receipt.withheld.is_empty());
    global_monty_driver::GlobalTaskPortsFactory::settle(&factory, task_host, receipt.clone())
        .await
        .unwrap();
    let history = threads
        .list_thread_history(ThreadHistoryRequest {
            scope: scope.clone(),
            thread_id: context.thread_id.clone(),
        })
        .await
        .unwrap();
    let result = history
        .messages
        .iter()
        .find_map(|message| message.tool_result_ref.as_ref())
        .unwrap();
    assert!(
        history
            .messages
            .iter()
            .all(|message| message.tool_result_provider_call.is_none())
    );
    let stored_message = history
        .messages
        .iter()
        .find(|message| message.tool_result_ref.is_some())
        .unwrap();
    let private_context = threads
        .load_context_messages(brassclaw_threads::LoadContextMessagesRequest {
            scope: scope.clone(),
            thread_id: context.thread_id.clone(),
            message_ids: vec![stored_message.message_id],
        })
        .await
        .unwrap();
    let provider_call = private_context.messages[0]
        .tool_result_provider_call
        .clone()
        .unwrap();
    let envelope = brassclaw_threads::ToolResultReferenceEnvelope::from_json_str(
        stored_message.content.as_deref().unwrap(),
    )
    .unwrap();
    let replay = brassclaw_threads::AppendToolResultReferenceRequest {
        scope: scope.clone(),
        thread_id: context.thread_id.clone(),
        turn_run_id: context.run_id.to_string(),
        result_ref: result.clone(),
        safe_summary: envelope.safe_summary,
        provider_call: Some(provider_call.clone()),
        model_observation: envelope.model_observation,
    };
    assert_eq!(
        threads
            .append_tool_result_reference(replay.clone())
            .await
            .unwrap()
            .message_id,
        stored_message.message_id
    );
    let mut conflicting = replay;
    conflicting.provider_call.as_mut().unwrap().provider_call_id = "different-provider-call".into();
    assert!(
        threads
            .append_tool_result_reference(conflicting)
            .await
            .is_err()
    );
    let result = brassclaw_turns::LoopResultRef::new(result.clone()).unwrap();
    // Recreate the store, without an in-memory cache, and read the actual value.
    let recreated = PgCapabilityIo::new(database.pool.clone());
    let output = recreated
        .result_output(&context.scope, context.run_id, &result)
        .await
        .unwrap();
    assert!(
        serde_json::to_string(&output)
            .unwrap()
            .contains("workspace-sentinel.txt")
    );
    let mut other = context.scope.clone();
    other.thread_id = ThreadId::new("another-opaque-conversation").unwrap();
    assert!(
        recreated
            .result_output(&other, context.run_id, &result)
            .await
            .is_err()
    );
    assert!(
        recreated
            .result_output(&context.scope, brassclaw_turns::TurnRunId::new(), &result)
            .await
            .is_err()
    );
    {
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_work_matches_provider(&provider, &requests);
    }
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    let exit = exit.service.unwrap();
    assert_eq!(exit.failure, None);
    assert!(exit.tasks.is_empty());
    assert_eq!(exit.transport.unwrap().kind, StopKind::Graceful);
}

fn assert_work_matches_provider(
    provider: &RecordingProvider,
    requests: &[HostManagedModelRequest],
) {
    let work = provider.policy_work.lock().unwrap();
    assert_eq!(work.len(), requests.len());
    for (work, request) in work.iter().zip(requests) {
        let expected = request
            .messages
            .iter()
            .fold(0u64, |total, message| {
                total.saturating_add(
                    brassclaw_loop_support::estimate_tokens_from_chars(&message.content).as_u64(),
                )
            })
            .max(64);
        assert_eq!(work.estimated_input_tokens, expected);
        assert_eq!(work.model_profile_id, request.model_profile_id);
    }
}

#[tokio::test]
async fn global_driver_hands_opaque_admitted_tasks_to_one_existing_service() {
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let provider = Arc::new(RecordingProvider::default());
    let prefix = Arc::new(SelectedPrefix("selected driver prefix".into()));
    // Root bytes come from an actual immutable class-10 definition. This is
    // draft validation; no seed/root approval is inferred from source integrity.
    let store =
        brassclaw_skills::revision_store::PgComponentRevisionStore::new(database.pool.clone());
    let definition = global_root_seed::retain_packaged_global_root(database.pool.clone(), SOURCE)
        .await
        .unwrap();
    let root_ref = definition.reference();
    let root_uuid = root_ref.uuid;
    assert_eq!(root_ref.version, 1);
    assert_eq!(definition.source(), SOURCE);
    assert!(!definition.ports().contains("post_reply"));
    let root_review =
        global_root_seed::retain_packaged_root_review(database.pool.clone(), &definition)
            .await
            .unwrap();
    assert_eq!(root_review.candidate, root_ref);
    assert!(root_review.base.is_none());
    // Concurrent boots retain exactly one original revision, not new versions
    // or trust records. Use real pool transactions rather than a fake store.
    for retained in futures::future::join_all(
        (0..8)
            .map(|_| global_root_seed::retain_packaged_global_root(database.pool.clone(), SOURCE)),
    )
    .await
    {
        assert_eq!(retained.unwrap().reference(), root_ref);
    }
    let root_draft = brassclaw_skills::orchestrator_contract::global_root_draft(
        root_uuid,
        SOURCE,
        definition.ports(),
    )
    .unwrap();
    let mut malformed: Value = serde_json::from_str(root_draft.exact_bytes()).unwrap();
    malformed["document"]["input_symbols"] = json!(["host", "thread"]);
    let malformed = brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(
        &malformed.to_string(),
    )
    .unwrap()
    .at_version(1)
    .unwrap();
    assert!(
        brassclaw_skills::orchestrator_contract::GlobalRootDefinition::from_revision(&malformed)
            .is_err()
    );
    let mut bad_digest: Value = serde_json::from_str(root_draft.exact_bytes()).unwrap();
    bad_digest["document"]["source_checksum"] = json!("0".repeat(64));
    let bad_digest = brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(
        &bad_digest.to_string(),
    )
    .unwrap()
    .at_version(1)
    .unwrap();
    assert!(
        brassclaw_skills::orchestrator_contract::GlobalRootDefinition::from_revision(&bad_digest)
            .is_err()
    );
    let mut boot = support::boot(definition.source());
    boot.aliases = definition.ports().clone();
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    // Staging a newer definition cannot replace this generation or the source
    // reference retained before startup. New roots require their own review.
    let replacement = brassclaw_skills::orchestrator_contract::global_root_draft(
        root_uuid,
        &format!("{SOURCE}\n# staged successor\n"),
        definition.ports(),
    )
    .unwrap();
    let successor_ref = store.stage(&replacement, root_ref.version).await.unwrap();
    let successors = store
        .read_exact(&[root_uuid], &[successor_ref])
        .await
        .unwrap();
    let successor = brassclaw_skills::orchestrator_contract::GlobalRootDefinition::from_revision(
        &successors.revisions()[&root_uuid],
    )
    .unwrap();
    assert_eq!(successor.reference().version, root_ref.version + 1);
    // Boot after an authored successor must retain the original package without
    // moving its allocation head backward or replacing the successor's bytes.
    assert_eq!(
        global_root_seed::retain_packaged_global_root(database.pool.clone(), SOURCE)
            .await
            .unwrap()
            .reference(),
        root_ref
    );
    assert_eq!(
        store.retain_packaged_draft(&replacement).await.unwrap(),
        successor_ref
    );
    let head: i64 = database
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT last_version FROM reborn_component_revision_heads WHERE component_id=$1",
            &[&root_uuid],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(head as u64, successor_ref.version);
    let package_upgrade = brassclaw_skills::orchestrator_contract::global_root_draft(
        root_uuid,
        &format!("{SOURCE}\n# packaged successor\n"),
        definition.ports(),
    )
    .unwrap();
    let upgrade_ref = store.retain_packaged_draft(&package_upgrade).await.unwrap();
    assert_eq!(upgrade_ref.version, successor_ref.version + 1);
    assert_eq!(
        store.retain_packaged_draft(&package_upgrade).await.unwrap(),
        upgrade_ref
    );
    let upgrade = store
        .read_revision(upgrade_ref.uuid, upgrade_ref.version)
        .await
        .unwrap();
    let upgrade_definition =
        brassclaw_skills::orchestrator_contract::GlobalRootDefinition::from_revision(&upgrade)
            .unwrap();
    let upgrade_review =
        global_root_seed::retain_packaged_root_review(database.pool.clone(), &upgrade_definition)
            .await
            .unwrap();
    assert_eq!(upgrade_review.candidate, upgrade_ref);
    assert_eq!(upgrade_review.base, Some(successor_ref));
    assert_ne!(root_review.id, upgrade_review.id);
    // Attaching or retrying a review cannot allocate a revision, select latest,
    // change the subject after an authored edit or turn its actor into approval.
    let repeated =
        global_root_seed::retain_packaged_root_review(database.pool.clone(), &definition)
            .await
            .unwrap();
    assert_eq!(repeated.id, root_review.id);
    assert_eq!(repeated.subject_bytes, root_review.subject_bytes);
    let heads: i64 = database
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT last_version FROM reborn_component_revision_heads WHERE component_id=$1",
            &[&root_uuid],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(heads as u64, upgrade_ref.version);
    let reviews = brassclaw_skills::review_submission_store::PgReviewSubmissionStore::new(
        database.pool.clone(),
    );
    let changed_actor = brassclaw_skills::review_submission_store::ReviewSubmissionDraft::new(
        root_review.id,
        "different-author",
        root_draft.clone(),
        None,
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        reviews.submit_retained(&changed_actor).await,
        Err(brassclaw_skills::review_submission_store::SubmissionStoreError::Conflict)
    ));
    let missing_draft = brassclaw_skills::orchestrator_contract::global_root_draft(
        uuid::Uuid::new_v4(),
        SOURCE,
        definition.ports(),
    )
    .unwrap();
    let missing_id = uuid::Uuid::new_v4();
    let missing = brassclaw_skills::review_submission_store::ReviewSubmissionDraft::new(
        missing_id,
        "packaged-global-root",
        missing_draft,
        None,
        Vec::new(),
    )
    .unwrap();
    assert!(reviews.submit_retained(&missing).await.is_err());
    assert!(reviews.read(missing_id).await.unwrap().is_none());
    // Reusing an identity for a different class is still a conflict, not a seed
    // override. This rejected retention must leave the existing root readable.
    let wrong_class = brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":root_uuid,"class_code":22,
            "document":{"body":"result = None"},"dependencies":[],"association":null})
        .to_string(),
    )
    .unwrap();
    assert!(matches!(
        store.retain_packaged_draft(&wrong_class).await,
        Err(brassclaw_skills::revision_store::RevisionStoreError::Conflict)
    ));
    let old = store.read_exact(&[root_uuid], &[root_ref]).await.unwrap();
    assert_eq!(
        old.revisions()[&root_uuid].draft().exact_bytes(),
        root_draft.exact_bytes()
    );
    let factory = Arc::new(
        NativeTaskPortsFactory::new(database.pool.clone(), owner.ownership_check(), None).await,
    );
    owner.ownership_check().check().await.unwrap();
    assert!(matches!(
        monty_instance_owner::PgMontyOwner::acquire(&database.pool).await,
        Err(monty_instance_owner::OwnershipError::AlreadyOwned)
    ));
    // Capacity one proves that healthy completed attempts release their slot;
    // neither conversation constructs a new root or a legacy Engine Thread.
    let threads = Arc::new(PgSessionThreadService::new(
        database.pool.clone(),
        "native-global-host",
    ));
    let driver =
        global_monty_driver::GlobalMontyDriver::new(owner.client(), threads, factory.clone(), 1)
            .unwrap();
    let mut observed_root = None;
    for name in ["driver-first", "driver-second"] {
        let (input, handoff, threads, scope, _) = admitted(
            database.pool.clone(),
            provider.clone(),
            name,
            prefix.clone(),
            5,
            owner.client().live_task_settings(),
            None,
        )
        .await;
        let exit = tokio::time::timeout(Duration::from_secs(10), driver.drive_turn(handoff))
            .await
            .unwrap()
            .unwrap();
        let brassclaw_turns::LoopExit::Completed(exit) = exit else {
            panic!("expected actual completion")
        };
        assert_eq!(
            exit.completion_kind,
            brassclaw_turns::LoopCompletionKind::FinalReply
        );
        assert_eq!(exit.reply_message_refs.len(), 1);
        let actual_host = factory.last_host.lock().unwrap().clone().unwrap();
        let attempt = actual_host.attempt();
        driver.stop_attempt(attempt).await.unwrap();
        assert!(driver.take_settlement(attempt).unwrap().is_none());
        let admission = database
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&attempt.run_id.as_uuid()],
            )
            .await
            .unwrap();
        assert_eq!(admission.get::<_, String>(0), "settled");
        let report = assert_settlement_outcome(
            admission.get::<_, Value>(1),
            json!({"status":"completed", "reply_ref":exit.reply_message_refs[0].as_str()}),
        );
        let root = factory.last_receipt.lock().unwrap().as_ref().unwrap().root;
        assert_eq!(root.source_checksum(), support::boot(SOURCE).checksum);
        assert!(root.matches_definition(definition.source_checksum(), definition.ports()));
        assert!(!root.matches_definition(successor.source_checksum(), successor.ports()));
        let mut wrong_ports = definition.ports().clone();
        wrong_ports.insert("unreviewed_port".into());
        assert!(!root.matches_definition(definition.source_checksum(), &wrong_ports));
        assert_eq!(report["root"]["format"], "monty-root-execution/1");
        assert_eq!(report["root"]["vm_id"], root.vm_id().to_string());
        assert_eq!(
            report["root"]["source_checksum"],
            format!("{:x}", sha2::Sha256::digest(SOURCE.as_bytes()))
        );
        assert_eq!(report["root"]["workers"], 2);
        if let Some(original) = observed_root {
            assert_eq!(root, original);
        }
        observed_root = Some(root);
        let duplicate = pg_monty_admission::PgMontyAdmission::reserve(
            database.pool.clone(),
            actual_host.run_context(),
            attempt,
        )
        .await;
        assert!(
            matches!(duplicate, Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_admission_replay_requires_recovery")
        );
        let stale = MontyTaskAttempt {
            lease_token: TurnLeaseToken::new(),
            ..attempt
        };
        assert!(
            matches!(pg_monty_admission::PgMontyAdmission::reserve(database.pool.clone(), actual_host.run_context(), stale).await,
            Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_admission_fenced")
        );
        let history = threads
            .list_thread_history(ThreadHistoryRequest {
                scope,
                thread_id: ThreadId::new(input.conversation_id).unwrap(),
            })
            .await
            .unwrap();
        let reply = history.messages.last().unwrap();
        assert_eq!(
            exit.reply_message_refs[0].as_str(),
            format!("msg:{}", reply.message_id)
        );
        assert_eq!(reply.content.as_deref(), Some("actual scoped reply"));
        assert_eq!(history.messages.len(), 7);
    }
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    owner.request_shutdown();
    assert!(owner.ownership_check().check().await.is_err());
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.ownership_failure.is_none());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(guard) => {
            guard.check().await.unwrap();
            panic!("clean shutdown must establish quiescence");
        }
    }
    let service = exit.service.unwrap();
    assert_eq!(Some(service.root), observed_root);
    assert_eq!(service.failure, None);
    assert!(service.tasks.is_empty());
    assert_eq!(service.transport.unwrap().kind, StopKind::Graceful);
    let replacement = monty_instance_owner::PgMontyOwner::acquire(&database.pool)
        .await
        .unwrap();
    replacement.check().await.unwrap();
    replacement.release().await.unwrap();
}

#[tokio::test]
async fn global_no_match_reply_obeys_live_tool_policy_without_model_replay() {
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let provider = Arc::new(RecordingProvider::default());
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    let reply = reply_workflows::reply_program(database.pool.clone()).await;
    let factory = Arc::new(NativeTaskPortsFactory::with_catalogue(
        database.pool.clone(),
        owner.ownership_check(),
        Arc::new(DraftCatalogueProvider {
            pool: database.pool.clone(),
            draft: None,
            reply,
            block_reply: true,
        }),
    ));
    let driver = global_monty_driver::GlobalMontyDriver::new(
        owner.client(),
        Arc::new(PgSessionThreadService::new(
            database.pool.clone(),
            "native-global-host",
        )),
        factory.clone(),
        1,
    )
    .unwrap();
    let (_, handoff, threads, scope, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "blocked-no-match-reply",
        Arc::new(SelectedPrefix("selected reply-policy prefix".into())),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let (request, attempt, host) = handoff.into_parts();
    let failed = tokio::time::timeout(
        Duration::from_secs(20),
        driver.drive_turn(MontyTaskHandoff::new(request, attempt, host).unwrap()),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(matches!(failed,
        brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }
        if reason_kind == "recipe_execution_failed"));
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    let (host, receipt, control) = driver.take_settlement(attempt).unwrap().unwrap();
    assert!(control.receipt().unwrap().is_ok());
    assert!(receipt.withheld.is_empty());
    assert!(receipt.accounting.is_some());
    assert!(host.finalized_reply_ref().is_none());
    let history = threads
        .list_thread_history(ThreadHistoryRequest {
            scope,
            thread_id: host.run_context().thread_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(history.messages.len(), 1);
    let client = database.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "settled");
    let report = assert_settlement_outcome(
        row.get(1),
        json!({"status":"failed","reason_kind":"recipe_execution_failed"}),
    );
    assert_eq!(report["all_selected_recipes_complete"], false);
    assert_eq!(report["recipes"].as_array().unwrap().len(), 1);
    assert_eq!(report["recipes"][0]["failed_step"], "0:2");
    assert_eq!(report["recipes"][0]["completed_steps"], json!([]));
    let rows = client.query(
        "SELECT phase,attempt_count,answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
        &[&attempt.run_id.as_uuid()],
    ).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, &str>(0), "answered");
    assert_eq!(rows[0].get::<_, i16>(1), 1);
    let answer: Value = serde_json::from_str(rows[0].get(2)).unwrap();
    assert_eq!(
        answer,
        json!({"kind":"terminal_error","reason_kind":"retained_tool_authorization"})
    );
    drop(client);
    let (retained_host, admission, ports, retained_receipt) = factory
        .inner
        .take_failed_settlement(attempt)
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&retained_host, &host));
    assert!(Arc::ptr_eq(&retained_receipt, &receipt));
    ports.fence();
    assert!(admission.check_and_start().await.is_err());
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    let service = exit.service.unwrap();
    assert!(service.tasks.is_empty());
    assert_eq!(service.failure, None);
    assert_eq!(service.transport.unwrap().kind, StopKind::Graceful);
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => {
            panic!("actual clean worker exit required")
        }
    }
}

#[tokio::test]
async fn owned_task_factory_fences_failed_preparation_without_replacing_its_admission() {
    use global_monty_driver::GlobalTaskPortsFactory;

    struct FailedCapture;
    #[async_trait]
    impl global_task_factory::MontyCatalogueProvider for FailedCapture {
        async fn capture(
            &self,
            _host: Arc<MontyTaskHost>,
            _admission: Arc<pg_monty_admission::PgMontyAdmission>,
            _input: &TaskInput,
        ) -> Result<
            Arc<dyn global_recipe_ports::MontyTaskCatalogue>,
            brassclaw_turns::run_profile::AgentLoopDriverError,
        > {
            Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed {
                reason_kind: "catalogue_capture_failed".into(),
            })
        }
    }
    let database = native_pg::NativePostgres::start().await;
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    let factory = global_task_factory::OwnedGlobalTaskFactory::new(
        database.pool.clone(),
        owner.ownership_check(),
        Arc::new(FailedCapture),
        1,
    )
    .unwrap();
    let provider = Arc::new(RecordingProvider::default());
    let prefix = Arc::new(SelectedPrefix("actual preparation prefix".into()));
    let (mut input, handoff, _, _, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "preparation-failure",
        prefix.clone(),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let host = Arc::new(MontyTaskHost::new(handoff));
    let original_conversation = input.conversation_id.clone();
    input.conversation_id = "another-opaque-conversation".into();
    assert!(matches!(factory.build(host.clone(), &input).await,
        Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_admission_identity_invalid"));
    let client = database.pool.get().await.unwrap();
    assert!(
        !client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM brassclaw_monty_task_admissions WHERE run_id=$1)",
                &[&host.attempt().run_id.as_uuid()]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    input.conversation_id = original_conversation;
    assert!(matches!(factory.build(host.clone(), &input).await,
        Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "catalogue_capture_failed"));
    let original = client.query_one("SELECT phase,admission_key,claim_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&host.attempt().run_id.as_uuid()]).await.unwrap();
    assert_eq!(original.get::<_, &str>(0), "reserved");
    admission_integrity::reject_reserved_rewrite(&database.pool, host.attempt().run_id).await;
    assert!(factory.take_failed_settlement(host.attempt()).is_err());
    assert!(matches!(factory.build(host.clone(), &input).await,
        Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_admission_replay_requires_recovery"));
    let repeated = client.query_one("SELECT admission_key,claim_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&host.attempt().run_id.as_uuid()]).await.unwrap();
    assert_eq!(original.get::<_, Vec<u8>>(1), repeated.get::<_, Vec<u8>>(0));
    assert_eq!(original.get::<_, Vec<u8>>(2), repeated.get::<_, Vec<u8>>(1));
    let (next_input, handoff, _, _, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "preparation-capacity",
        prefix,
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let next = Arc::new(MontyTaskHost::new(handoff));
    assert!(matches!(
        factory.build(next.clone(), &next_input).await,
        Err(brassclaw_turns::run_profile::AgentLoopDriverError::Unavailable { .. })
    ));
    assert!(
        !client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM brassclaw_monty_task_admissions WHERE run_id=$1)",
                &[&next.attempt().run_id.as_uuid()]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert!(provider.requests.lock().unwrap().is_empty());
    drop(client);
    owner.request_shutdown();
    let exit = owner.join().await.unwrap();
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => panic!("no task was submitted"),
    }
    let service = exit.service.unwrap();
    assert!(service.tasks.is_empty());
    assert_eq!(service.failure, None);
    assert_eq!(service.transport.unwrap().kind, StopKind::Graceful);
}

#[tokio::test]
async fn owned_task_factory_retains_cancelled_catalogue_preparation() {
    use global_monty_driver::GlobalTaskPortsFactory;

    struct WaitingCapture(Arc<tokio::sync::Notify>);
    #[async_trait]
    impl global_task_factory::MontyCatalogueProvider for WaitingCapture {
        async fn capture(
            &self,
            _host: Arc<MontyTaskHost>,
            _admission: Arc<pg_monty_admission::PgMontyAdmission>,
            _input: &TaskInput,
        ) -> Result<
            Arc<dyn global_recipe_ports::MontyTaskCatalogue>,
            brassclaw_turns::run_profile::AgentLoopDriverError,
        > {
            self.0.notify_one();
            std::future::pending().await
        }
    }
    let database = native_pg::NativePostgres::start().await;
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let factory = global_task_factory::OwnedGlobalTaskFactory::new(
        database.pool.clone(),
        owner.ownership_check(),
        Arc::new(WaitingCapture(entered.clone())),
        1,
    )
    .unwrap();
    let provider = Arc::new(RecordingProvider::default());
    let (input, handoff, _, _, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "cancelled-preparation",
        Arc::new(SelectedPrefix("selected prefix".into())),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let host = Arc::new(MontyTaskHost::new(handoff));
    let mut preparation = Box::pin(factory.build(host.clone(), &input));
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            _ = entered.notified() => (),
            _ = &mut preparation => panic!("catalogue capture must still be pending"),
        }
    })
    .await
    .unwrap();
    // This is a real dropped preparation future, not a fabricated service
    // cancellation receipt. No Python task or external work was submitted.
    drop(preparation);
    host.fence_dispatch();
    assert!(factory.take_failed_settlement(host.attempt()).is_err());
    assert!(matches!(factory.build(host.clone(), &input).await,
        Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_admission_replay_requires_recovery"));
    let client = database.pool.get().await.unwrap();
    let row = client.query_one("SELECT phase,octet_length(admission_key),outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1", &[&host.attempt().run_id.as_uuid()]).await.unwrap();
    assert_eq!(row.get::<_, &str>(0), "reserved");
    assert_eq!(row.get::<_, i32>(1), 32);
    assert_eq!(row.get::<_, Option<Value>>(2), None);
    assert!(provider.requests.lock().unwrap().is_empty());
    drop(client);
    owner.request_shutdown();
    let exit = owner.join().await.unwrap();
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => panic!("no task was submitted"),
    }
    assert!(exit.service.unwrap().tasks.is_empty());
}

#[tokio::test]
async fn global_owner_loses_real_database_session_and_fences_new_task_dispatch() {
    let database = native_pg::NativePostgres::start().await;
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    owner.ownership_check().check().await.unwrap();
    let client = database.pool.get().await.unwrap();
    let pid: i32 = client
        .query_one(
            "SELECT pid FROM pg_locks WHERE locktype='advisory'
        AND classid::bigint=$1 AND objid::bigint=$2 AND objsubid=2 AND granted",
            &[&i64::from(0x4252_434c_i32), &i64::from(0x4d4f_4e54_i32)],
        )
        .await
        .unwrap()
        .get(0);
    assert!(
        client
            .query_one("SELECT pg_terminate_backend($1)", &[&pid])
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    assert!(owner.ownership_check().check().await.is_err());
    let provider = Arc::new(RecordingProvider::default());
    let (_, handoff, threads, _, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "lost-owner",
        Arc::new(SelectedPrefix("selected prefix".into())),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let factory = Arc::new(
        NativeTaskPortsFactory::new(database.pool.clone(), owner.ownership_check(), None).await,
    );
    let driver =
        global_monty_driver::GlobalMontyDriver::new(owner.client(), threads, factory, 1).unwrap();
    assert!(matches!(driver.drive_turn(handoff).await,
        Err(brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_instance_ownership_failed"));
    assert!(provider.requests.lock().unwrap().is_empty());
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.ownership_failure.is_some());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => {
            assert!(release.is_err())
        }
        global_monty_owner::OwnershipSettlement::Quarantined(guard) => {
            assert!(guard.check().await.is_err());
            panic!("actual root and host quiescence must be observed");
        }
    }
    let service = exit.service.unwrap();
    assert!(service.tasks.is_empty());
    assert!(service.transport.unwrap().exit_status.is_some());
    let replacement = monty_instance_owner::PgMontyOwner::acquire(&database.pool)
        .await
        .unwrap();
    replacement.release().await.unwrap();
}

#[tokio::test]
async fn cancelled_boot_retains_instance_ownership_until_actual_worker_settlement() {
    let database = native_pg::NativePostgres::start().await;
    // Actual VM boot computation keeps the startup future pending. It performs
    // no Tool effects and never fabricates worker readiness or host results.
    let source = format!(
        "import time\n_boot_start = time.monotonic()\nwhile time.monotonic() - _boot_start < 0.5:\n    _boot_padding = 1\n{SOURCE}"
    );
    let boot = support::boot(&source);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let pool = database.pool.clone();
    let startup = tokio::spawn(async move {
        global_monty_owner::GlobalMontyOwner::start(
            &pool,
            support::worker(),
            global_monty_owner::GlobalServiceConfig {
                boot,
                process: support::limits(),
                live,
                actor: ActorLimits {
                    max_unclaimed: 8,
                    max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                    max_control_unclaimed: 4,
                    max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
                },
                queue_capacity: 8,
            },
        )
        .await
    });
    let client = database.pool.get().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let owned: bool = client
                .query_one(
                    "SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype='advisory'
                 AND classid::bigint=$1 AND objid::bigint=$2 AND objsubid=2 AND granted)",
                    &[&i64::from(0x4252_434c_i32), &i64::from(0x4d4f_4e54_i32)],
                )
                .await
                .unwrap()
                .get(0);
            if owned {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        !startup.is_finished(),
        "regression must cancel during real boot"
    );
    startup.abort();
    match startup.await {
        Err(error) => assert!(error.is_cancelled()),
        Ok(_) => panic!("startup unexpectedly completed before cancellation"),
    }
    assert!(
        matches!(
            monty_instance_owner::PgMontyOwner::acquire(&database.pool).await,
            Err(monty_instance_owner::OwnershipError::AlreadyOwned)
        ),
        "abandoning startup must not release the live worker's ownership"
    );
    let replacement = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match monty_instance_owner::PgMontyOwner::acquire(&database.pool).await {
                Ok(owner) => break owner,
                Err(monty_instance_owner::OwnershipError::AlreadyOwned) => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => panic!("ownership check failed: {error}"),
            }
        }
    })
    .await
    .expect("actual boot and worker shutdown must release ownership");
    replacement.release().await.unwrap();
}

#[path = "support/admission.rs"]
mod invocation_admission;
#[path = "support/retained_kernel.rs"]
mod retained_kernel;
#[path = "support/retained_program.rs"]
mod retained_program;

#[tokio::test]
async fn native_invocation_journal_keeps_uncertainty_and_late_real_answer_without_replay() {
    use brassclaw_engine::memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    };
    use brassclaw_host_api::{
        CapabilityId, CapabilitySet, ExecutionContext, ExtensionId, MountView, ResourceEstimate,
        RuntimeKind, TrustClass,
    };
    use brassclaw_host_runtime::{RuntimeCapabilityOutcome, RuntimeCapabilityRequest};
    use brassclaw_monty_host::process::PortAnswer;
    use brassclaw_skills::{
        component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
    };
    use brassclaw_turns::{CancelRunRequest, SanitizedCancelReason};
    let rig = native_pg::NativePostgres::start().await;
    let store = PgComponentRevisionStore::new(rig.pool.clone());
    let program = retained_program::program(&store, false).await;
    let admitted = invocation_admission::reserve(rig.pool.clone(), "actual journal input").await;
    admitted
        .admission
        .retain_recipe_selection(program.inputs().instruction())
        .await
        .unwrap();
    admitted
        .admission
        .retain_recipe_selection(program.inputs().instruction())
        .await
        .unwrap();
    // A real immutable replacement can coexist in the store, but this task
    // cannot switch to it, even before its first Tool invocation.
    let old = program.inputs().instruction();
    let root = old.recipe().uuid;
    let old_draft = old.snapshot().revisions()[&root].draft();
    let mut document = old_draft.document().clone();
    document["variants"][0]["description"] = json!("replacement workflow");
    let dependencies: Vec<_> = old_draft.dependencies().iter().copied().collect();
    let replacement = ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":root,"class_code":21,
        "document":document,"dependencies":dependencies,"association":null})
        .to_string(),
    )
    .unwrap();
    let newer = store
        .stage(&replacement, old.recipe().version)
        .await
        .unwrap();
    let references: Vec<_> = old
        .snapshot()
        .revisions()
        .values()
        .map(|revision| {
            if revision.reference().uuid == root {
                newer
            } else {
                revision.reference()
            }
        })
        .collect();
    let replacement_program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(store.read_exact(&[root], &references).await.unwrap()),
            root,
            "selected",
            WorkflowClass::Deterministic,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        admitted
            .admission
            .retain_recipe_selection(replacement_program.inputs().instruction())
            .await
            .is_err()
    );
    assert!(
        admitted.input["conversation_id"]
            .as_str()
            .unwrap()
            .starts_with("opaque-draft-")
    );
    let arguments = json!({"operation":"parse","data":"{\"text\":\"observed\"}"});
    assert!(
        admitted
            .admission
            .begin_tool_invocation(&replacement_program, "0:2", &arguments)
            .await
            .is_err(),
        "a replacement cannot slip into an unexecuted step of the retained workflow"
    );
    let pending = admitted
        .admission
        .begin_tool_invocation(&program, "0:4", &arguments)
        .await
        .unwrap();
    // No Tool was called for this record. Losing the Rust handle leaves a real
    // persisted intent, not a fabricated effect-free or completed answer.
    drop(pending);
    assert!(
        admitted
            .admission
            .begin_tool_invocation(&program, "0:4", &arguments)
            .await
            .is_err()
    );
    // Step IDs are Recipe-local. A different retained workflow in the same
    // task can use 0:2 without replaying this Recipe's 0:2 invocation.
    let follow_on = retained_program::program(&store, false).await;
    let follow_on_record = admitted
        .admission
        .begin_tool_invocation(&follow_on, "0:2", &arguments)
        .await
        .unwrap();
    drop(follow_on_record); // no Tool called; retain uncertainty honestly
    let record = admitted
        .admission
        .begin_tool_invocation(&program, "0:2", &arguments)
        .await
        .unwrap();
    let (runtime, policy) = retained_kernel::runtime(program.bindings()["0:2"].tool().uuid);
    assert_eq!(
        policy
            .tool_identity(&CapabilityId::new("builtin.json").unwrap())
            .unwrap(),
        Some(program.bindings()["0:2"].tool().uuid)
    );
    let context = ExecutionContext::local_default(
        UserId::new("draft-operator").unwrap(),
        ExtensionId::new("draft-caller").unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::FirstParty,
        CapabilitySet::default(),
        MountView::default(),
    )
    .unwrap();
    let outcome = runtime
        .invoke(RuntimeCapabilityRequest::new(
            context,
            CapabilityId::new("builtin.json").unwrap(),
            ResourceEstimate::default(),
            arguments.clone(),
            retained_kernel::trust(),
        ))
        .await
        .unwrap();
    let RuntimeCapabilityOutcome::Completed(completed) = outcome else {
        panic!("actual JSON kernel result required: {outcome:?}");
    };
    assert_eq!(completed.output, json!({"text":"observed"}));
    let answer = PortAnswer::Return {
        value: completed.output,
    };
    let client = rig.pool.get().await.unwrap();
    client
        .batch_execute(&format!(
            "ALTER TABLE brassclaw_monty_tool_invocations
        ADD CONSTRAINT reject_test_answer CHECK (run_id <> '{}'::uuid OR phase <> 'answered')",
            admitted.context.run_id
        ))
        .await
        .unwrap();
    assert!(record.record_answer(&answer).await.is_err());
    // A real database failure after the actual operation cannot grant replay.
    assert!(
        admitted
            .admission
            .begin_tool_invocation(&program, "0:2", &arguments)
            .await
            .is_err()
    );
    let row = client
        .query_one(
            "SELECT phase,answer_bytes FROM brassclaw_monty_tool_invocations
        WHERE run_id=$1 AND recipe_id=$2 AND step_id='0:2'",
            &[
                &admitted.context.run_id.as_uuid(),
                &program.inputs().instruction().recipe().uuid,
            ],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "dispatch_intent");
    assert!(row.get::<_, Option<String>>(1).is_none());
    client
        .batch_execute(
            "ALTER TABLE brassclaw_monty_tool_invocations DROP CONSTRAINT reject_test_answer",
        )
        .await
        .unwrap();
    DefaultTurnCoordinator::new(admitted.state.clone())
        .cancel_run(CancelRunRequest {
            scope: admitted.context.scope.clone(),
            actor: admitted.context.actor.clone().unwrap(),
            run_id: admitted.context.run_id,
            reason: SanitizedCancelReason::Policy,
            idempotency_key: IdempotencyKey::new("late-record-cancel").unwrap(),
        })
        .await
        .unwrap();
    record.record_answer(&answer).await.unwrap();
    record.record_answer(&answer).await.unwrap();
    let rows = client
        .query(
            "SELECT step_id,phase,attempt_count,answer_bytes FROM brassclaw_monty_tool_invocations
        WHERE run_id=$1 AND recipe_id=$2 ORDER BY step_id",
            &[
                &admitted.context.run_id.as_uuid(),
                &program.inputs().instruction().recipe().uuid,
            ],
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<_, &str>(0), "0:2");
    assert_eq!(rows[0].get::<_, &str>(1), "answered");
    let actual: Value = serde_json::from_str(rows[0].get(3)).unwrap();
    assert_eq!(actual, json!({"kind":"return","value":{"text":"observed"}}));
    assert_eq!(rows[1].get::<_, &str>(0), "0:4");
    assert_eq!(rows[1].get::<_, &str>(1), "dispatch_intent");
    assert!(rows[1].get::<_, Option<String>>(3).is_none());
    for row in rows {
        assert_eq!(row.get::<_, i16>(2), 1);
    }
    let row = client
        .query_one(
            "SELECT phase,answer_bytes FROM brassclaw_monty_tool_invocations
             WHERE run_id=$1 AND recipe_id=$2 AND step_id='0:2'",
            &[
                &admitted.context.run_id.as_uuid(),
                &follow_on.inputs().instruction().recipe().uuid,
            ],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "dispatch_intent");
    assert!(row.get::<_, Option<String>>(1).is_none());
    for statement in [
        "UPDATE brassclaw_monty_tool_invocations SET phase='dispatch_intent',answer_bytes=NULL,answer_checksum=NULL,answered_at=NULL WHERE step_id='0:2'",
        "UPDATE brassclaw_monty_tool_invocations SET attempt_count=2",
        "UPDATE brassclaw_monty_tool_invocations SET step_id='replacement'",
        "DELETE FROM brassclaw_monty_tool_invocations",
        "TRUNCATE brassclaw_monty_tool_invocations",
        "UPDATE brassclaw_monty_recipe_selections SET selection_bytes='{}'",
        "DELETE FROM brassclaw_monty_recipe_selections",
        "TRUNCATE brassclaw_monty_recipe_selections CASCADE",
    ] {
        let error = client.batch_execute(statement).await.unwrap_err();
        assert_eq!(error.as_db_error().unwrap().code().code(), "23514");
    }
    assert!(
        admitted
            .admission
            .begin_tool_invocation(&program, "0:4", &arguments)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn global_recipe_ports_retain_actual_ibs_and_effects_without_model_replay() {
    use brassclaw_engine::memory::intent_system::{
        InputClass, IntentScope, IntentSource, seed_intent_input,
    };
    let database = native_pg::NativePostgres::start().await;
    let store =
        brassclaw_skills::revision_store::PgComponentRevisionStore::new(database.pool.clone());
    let program = retained_program::program(&store, false).await;
    let recipe = program.inputs().instruction().recipe().uuid;
    seed_intent_input(
        &database.pool,
        &IntentScope {
            tenant_id: "native-global-host".into(),
            user_id: "global-operator".into(),
            agent_id: "global-agent".into(),
            project_id: "global-project".into(),
        },
        "parse %",
        InputClass::Partial,
        recipe,
        21,
        IntentSource::Seeded,
        Some("0:1-0:E"),
    )
    .await
    .unwrap();
    // A higher-scored draft outside this validator's selected catalogue must
    // be excluded before ranking, rather than hide the actual matched Recipe.
    let excluded = retained_program::program(&store, false).await;
    let excluded_id = excluded.inputs().instruction().recipe().uuid;
    seed_intent_input(
        &database.pool,
        &IntentScope {
            tenant_id: "native-global-host".into(),
            user_id: "global-operator".into(),
            agent_id: "global-agent".into(),
            project_id: "global-project".into(),
        },
        "parse %",
        InputClass::Partial,
        excluded_id,
        21,
        IntentSource::Seeded,
        Some("0:1-0:E"),
    )
    .await
    .unwrap();
    database
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE reborn_intent_inputs SET score=100 WHERE component_id=$1",
            &[&excluded_id],
        )
        .await
        .unwrap();
    let provider = Arc::new(RecordingProvider::default());
    let prefix = Arc::new(SelectedPrefix("actual selected validation prefix".into()));
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    let factory = Arc::new(
        NativeTaskPortsFactory::new(
            database.pool.clone(),
            owner.ownership_check(),
            Some(program.clone()),
        )
        .await,
    );
    let threads = Arc::new(PgSessionThreadService::new(
        database.pool.clone(),
        "native-global-host",
    ));
    let driver =
        global_monty_driver::GlobalMontyDriver::new(owner.client(), threads, factory.clone(), 1)
            .unwrap();
    let data = json!({"text":"'quotes'\n Ü {{vars.data}} host.forbidden()"});
    let query = format!("parse {data}");
    let (_, handoff, _, _, _) = admitted_with_text(
        database.pool.clone(),
        provider.clone(),
        AdmissionInput {
            name: "recipe-failure",
            text: &query,
        },
        prefix.clone(),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let (request, attempt, host) = handoff.into_parts();
    let handoff = MontyTaskHandoff::new(request, attempt, host).unwrap();
    let failed = tokio::time::timeout(Duration::from_secs(20), driver.drive_turn(handoff))
        .await
        .unwrap()
        .unwrap_err();
    assert!(
        matches!(failed, brassclaw_turns::run_profile::AgentLoopDriverError::Failed { reason_kind }
        if reason_kind == "recipe_reply_invalid")
    );
    // Both JSON effects really completed. This draft deliberately returns a
    // JSON object, not a published reply; the root must fail without Tier 2.
    let client = database.pool.get().await.unwrap();
    let rows = client.query("SELECT phase, answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id",
        &[&attempt.run_id.as_uuid()]).await.unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.get::<_, &str>(0), "answered");
        let answer: Value = serde_json::from_str(row.get(1)).unwrap();
        assert_eq!(answer, json!({"kind":"return","value":data}));
    }
    let selected: String = client.query_one("SELECT selection_bytes FROM brassclaw_monty_recipe_selections WHERE run_id=$1 AND recipe_id=$2",
        &[&attempt.run_id.as_uuid(), &recipe]).await.unwrap().get(0);
    assert_eq!(
        serde_json::from_str::<Value>(&selected).unwrap()["variant"]["variant_key"],
        "selected"
    );
    drop(client);
    assert!(provider.requests.lock().unwrap().is_empty());
    let (actual_host, receipt, retained_control) =
        driver.take_settlement(attempt).unwrap().unwrap();
    assert!(
        matches!(receipt.outcome, TaskOutcome::Failed { ref reason_kind } if reason_kind == "recipe_reply_invalid")
    );
    assert!(retained_control.receipt().unwrap().is_ok());
    let (factory_host, retained_admission, retained_ports, factory_receipt) = factory
        .inner
        .take_failed_settlement(attempt)
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&factory_host, &actual_host));
    assert!(Arc::ptr_eq(&factory_receipt, &receipt));
    // The caller now owns both the actual child/Tool state and its durable
    // admission address. Taking settlement cannot grant another dispatch.
    retained_ports.fence();
    assert!(retained_admission.check_and_start().await.is_err());
    assert!(
        factory
            .inner
            .take_failed_settlement(attempt)
            .unwrap()
            .is_none()
    );
    // The same global root handles an actual No-Match after the failed Recipe.
    let (_, handoff, _, _, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "after-recipe-failure",
        prefix,
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    assert!(matches!(
        driver.drive_turn(handoff).await.unwrap(),
        brassclaw_turns::LoopExit::Completed(_)
    ));
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.service.unwrap().tasks.is_empty());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => {
            panic!("actual clean exit required")
        }
    }
}

#[tokio::test]
async fn dropped_turn_waiter_requires_worker_and_durable_cancellation_acknowledgement() {
    use brassclaw_turns::run_profile::AgentLoopDriverError;
    use brassclaw_turns::runner::CancelRunCompletionRequest;
    use brassclaw_turns::{CancelRunRequest, SanitizedCancelReason};

    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let hold = Arc::new(ProviderHold::default());
    let provider = Arc::new(RecordingProvider {
        hold: Some(hold.clone()),
        ..Default::default()
    });
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    let factory = Arc::new(
        NativeTaskPortsFactory::new(database.pool.clone(), owner.ownership_check(), None).await,
    );
    let prefix = Arc::new(SelectedPrefix("cancellation validation prefix".into()));
    let (_, handoff, threads, scope, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "dropped-waiter",
        prefix.clone(),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let (request, attempt, host) = handoff.into_parts();
    let context = host.run_context().clone();
    let handoff = MontyTaskHandoff::new(request, attempt, host).unwrap();
    let driver = global_monty_driver::GlobalMontyDriver::new(
        owner.client(),
        threads.clone(),
        factory.clone(),
        1,
    )
    .unwrap();
    let mut drive = Box::pin(driver.drive_turn(handoff));
    tokio::time::timeout(Duration::from_secs(10), async {
        tokio::select! {
            _ = hold.entered.notified() => {},
            result = &mut drive => panic!("provider must remain in progress: {result:?}"),
        }
    })
    .await
    .unwrap();
    drop(drive);
    assert!(matches!(
        driver.stop_attempt(attempt).await,
        Err(AgentLoopDriverError::Unavailable { .. })
    ));
    assert!(driver.take_settlement(attempt).is_err());
    let client = database.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT phase,outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, &str>(0), "started");
    assert!(row.get::<_, Option<Value>>(1).is_none());

    // Dispatch remains fenced. Recording the original service outcome after a
    // terminal claim transition is audit persistence, not permission to resume.
    let state = Arc::new(PgTurnStateStore::new(
        database.pool.clone(),
        "native-global-host",
    ));
    DefaultTurnCoordinator::new(state.clone())
        .cancel_run(CancelRunRequest {
            scope: context.scope.clone(),
            actor: context.actor.clone().unwrap(),
            run_id: attempt.run_id,
            reason: SanitizedCancelReason::Policy,
            idempotency_key: IdempotencyKey::new("dropped-waiter-cancel").unwrap(),
        })
        .await
        .unwrap();
    client
        .batch_execute(&format!(
            "CREATE FUNCTION reject_settlement_commit() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN IF NEW.run_id = '{}'::uuid AND NEW.phase = 'settled' THEN
           RAISE EXCEPTION 'test settlement commit failure'; END IF; RETURN NEW; END $$;
         CREATE CONSTRAINT TRIGGER reject_settlement_commit AFTER UPDATE
         ON brassclaw_monty_task_admissions DEFERRABLE INITIALLY DEFERRED
         FOR EACH ROW EXECUTE FUNCTION reject_settlement_commit()",
            attempt.run_id,
        ))
        .await
        .unwrap();
    hold.released.notify_one();
    assert!(matches!(driver.stop_attempt(attempt).await,
        Err(AgentLoopDriverError::Failed { reason_kind }) if reason_kind == "monty_admission_database_failed"));
    assert!(driver.take_settlement(attempt).is_err());
    assert!(factory.inner.take_failed_settlement(attempt).is_err());
    let phase: String = client
        .query_one(
            "SELECT phase FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(phase, "started");
    // The actual service receipt is now available, even though its audit write
    // failed. Terminal cancellation releases the claim only after that worker
    // acknowledgement; the original durable outcome must remain recordable.
    TurnRunTransitionPort::cancel_run(
        state.as_ref(),
        CancelRunCompletionRequest {
            run_id: attempt.run_id,
            runner_id: attempt.runner_id,
            lease_token: attempt.lease_token,
        },
    )
    .await
    .unwrap();
    client
        .batch_execute(
            "DROP TRIGGER reject_settlement_commit ON brassclaw_monty_task_admissions;
         DROP FUNCTION reject_settlement_commit()",
        )
        .await
        .unwrap();
    // Two callers acknowledge one retained receipt. Only the same idempotent
    // database write is retried; the actual model request is never replayed.
    let (first, second) = tokio::join!(driver.stop_attempt(attempt), driver.stop_attempt(attempt));
    first.unwrap();
    second.unwrap();
    let (actual_host, receipt, control) = driver.take_settlement(attempt).unwrap().unwrap();
    assert!(
        matches!(receipt.outcome, TaskOutcome::Failed { ref reason_kind } if reason_kind == "task_cancelled")
    );
    assert!(Arc::ptr_eq(&receipt, &control.receipt().unwrap().unwrap()));
    assert!(driver.take_settlement(attempt).unwrap().is_none());
    let (factory_host, admission, ports, factory_receipt) = factory
        .inner
        .take_failed_settlement(attempt)
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&factory_host, &actual_host));
    assert!(Arc::ptr_eq(&factory_receipt, &receipt));
    assert!(admission.check_and_start().await.is_err());
    ports.fence();
    let outcome: Value = client
        .query_one(
            "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_settlement_outcome(
        outcome,
        json!({"status":"failed", "reason_kind":"task_cancelled"}),
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    let history = threads
        .list_thread_history(ThreadHistoryRequest {
            scope,
            thread_id: context.thread_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(history.messages.len(), 1);
    assert!(actual_host.has_withheld_results() || !receipt.withheld.is_empty());
    drop(client);

    let next_provider = Arc::new(RecordingProvider::default());
    let (_, handoff, _, _, _) = admitted(
        database.pool.clone(),
        next_provider.clone(),
        "after-dropped-waiter",
        prefix,
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    assert!(matches!(
        driver.drive_turn(handoff).await.unwrap(),
        brassclaw_turns::LoopExit::Completed(_)
    ));
    assert_eq!(next_provider.requests.lock().unwrap().len(), 1);
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.service.unwrap().tasks.is_empty());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => {
            panic!("actual clean settlement required")
        }
    }
}

#[tokio::test]
async fn completed_reply_survives_waiter_fencing_before_durable_settlement() {
    brassclaw_reborn::loop_driver_host::init_compaction_summarizer(
        include_str!(
            "../../../crates/brassclaw_loop_support/prompts/compaction_summarizer_fresh.md"
        )
        .to_owned(),
    );
    let database = native_pg::NativePostgres::start().await;
    let provider = Arc::new(RecordingProvider::default());
    let boot = support::boot(SOURCE);
    let live = LiveMontyTaskSettings::new(boot.task_settings.into()).unwrap();
    let mut owner = global_monty_owner::GlobalMontyOwner::start(
        &database.pool,
        support::worker(),
        global_monty_owner::GlobalServiceConfig {
            boot,
            process: support::limits(),
            live,
            actor: ActorLimits {
                max_unclaimed: 8,
                max_reserved_frame_bytes: 4 * support::limits().max_frame_bytes,
                max_control_unclaimed: 4,
                max_control_reserved_frame_bytes: 2 * support::limits().max_frame_bytes,
            },
            queue_capacity: 8,
        },
    )
    .await
    .unwrap();
    let factory = Arc::new(
        NativeTaskPortsFactory::new(database.pool.clone(), owner.ownership_check(), None).await,
    );
    let hold = Arc::new(ProviderHold::default());
    *factory.settlement_hold.lock().unwrap() = Some(hold.clone());
    let prefix = Arc::new(SelectedPrefix("completed settlement prefix".into()));
    let (_, handoff, threads, scope, _) = admitted(
        database.pool.clone(),
        provider.clone(),
        "completed-before-fence",
        prefix.clone(),
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    let (request, attempt, task_host) = handoff.into_parts();
    let handoff = MontyTaskHandoff::new(request, attempt, task_host).unwrap();
    let driver = global_monty_driver::GlobalMontyDriver::new(
        owner.client(),
        threads.clone(),
        factory.clone(),
        1,
    )
    .unwrap();
    let mut drive = Box::pin(driver.drive_turn(handoff));
    tokio::time::timeout(Duration::from_secs(10), async {
        tokio::select! {
            _ = hold.entered.notified() => {},
            result = &mut drive => panic!("actual settlement must be held: {result:?}"),
        }
    })
    .await
    .unwrap();
    let host = factory.last_host.lock().unwrap().clone().unwrap();
    let reference = host.finalized_reply_ref().unwrap();
    assert_eq!(
        host.published_reply_content(&reference).unwrap(),
        "actual scoped reply"
    );
    let client = database.pool.get().await.unwrap();
    let phase: String = client
        .query_one(
            "SELECT phase FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(phase, "started");
    drop(drive);
    // Active calls are fenced, but the supervisor retains the actual published
    // reference. A late stop records Completed, never fabricates cancellation.
    assert!(host.published_reply_content(&reference).is_err());
    assert_eq!(host.finalized_reply_ref(), Some(reference.clone()));
    assert!(driver.take_settlement(attempt).is_err());
    hold.released.notify_one();
    driver.stop_attempt(attempt).await.unwrap();
    let (settled_host, receipt, control) = driver.take_settlement(attempt).unwrap().unwrap();
    assert!(Arc::ptr_eq(&host, &settled_host));
    assert!(Arc::ptr_eq(&receipt, &control.receipt().unwrap().unwrap()));
    assert_eq!(
        receipt.outcome,
        TaskOutcome::Completed {
            reply_ref: reference.as_str().to_owned()
        }
    );
    assert!(receipt.withheld.is_empty());
    assert!(!host.has_withheld_results());
    assert!(driver.take_settlement(attempt).unwrap().is_none());
    assert!(
        factory
            .inner
            .take_failed_settlement(attempt)
            .unwrap()
            .is_none()
    );
    let outcome: Value = client
        .query_one(
            "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&attempt.run_id.as_uuid()],
        )
        .await
        .unwrap()
        .get(0);
    assert_settlement_outcome(
        outcome,
        json!({"status":"completed", "reply_ref":reference.as_str()}),
    );
    let history = threads
        .list_thread_history(ThreadHistoryRequest {
            scope,
            thread_id: host.run_context().thread_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(history.messages.len(), 2);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    drop(client);

    let next_provider = Arc::new(RecordingProvider::default());
    let (_, handoff, _, _, _) = admitted(
        database.pool.clone(),
        next_provider.clone(),
        "after-completed-fence",
        prefix,
        0,
        owner.client().live_task_settings(),
        None,
    )
    .await;
    assert!(matches!(
        driver.drive_turn(handoff).await.unwrap(),
        brassclaw_turns::LoopExit::Completed(_)
    ));
    assert_eq!(next_provider.requests.lock().unwrap().len(), 1);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    owner.request_shutdown();
    let exit = tokio::time::timeout(Duration::from_secs(10), owner.join())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.service.unwrap().tasks.is_empty());
    match exit.ownership {
        global_monty_owner::OwnershipSettlement::ReleaseAttempt(release) => release.unwrap(),
        global_monty_owner::OwnershipSettlement::Quarantined(_) => {
            panic!("actual clean settlement required")
        }
    }
}
