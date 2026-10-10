//! Assembled Reborn runtime: substrate + drivers + worker, started as one.
//!
//! This module is the "later slice" the crate-level docstring promises:
//! product-level wiring on top of the substrate facades exposed by
//! `build_reborn_services`. It is the **only** place in the workspace where
//! `brassclaw_reborn` (drivers, host factory, model gateway bridge),
//! `brassclaw_threads` (session thread service), and (under the
//! `root-llm-provider` feature) `brassclaw_llm` are composed into a running
//! agent.
//!
//! Downstream callers (the CLI, future channel adapters, e2e harnesses) reach
//! this assembly only through:
//!
//! - [`build_reborn_runtime`] — construct + start the runtime
//! - [`RebornRuntime`] — task-level handle (`new_conversation`,
//!   `send_user_message`, `shutdown`)
//!
//! They never name the underlying `TurnCoordinator`, `SessionThreadService`,
//! `LoopExitApplier`, `HostManagedModelGateway`, etc. directly. That is the
//! property that satisfies the "narrow Reborn public surface" requirement
//! pinned by `crates/brassclaw_architecture/tests/reborn_dependency_boundaries.rs`.

// arch-exempt: large_file, needs Reborn runtime helper extraction, plan #4471
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use brassclaw_events::{DurableAuditLog, DurableEventLog, InMemoryAuditSink, RuntimeEvent};
use brassclaw_first_party_extension_ports::SelectableSkillContextSource;
use brassclaw_host_api::{
    ActionResultSummary, ActionSummary, AgentId, AuditEnvelope, AuditEventId, AuditStage,
    CapabilityId, CorrelationId, DecisionSummary, EffectKind, InvocationId, ProjectId,
    ResourceScope, TenantId, ThreadId, UserId,
};
use brassclaw_loop_support::{
    CapabilityAllowSet, CapabilityResolveError, CapabilitySurfaceProfileResolver,
    JsonSpawnSubagentInputCodec, ModelGatewayBackedSystemInferencePort,
};
use brassclaw_product_adapters::ProjectionStream;
use brassclaw_product_workflow::{
    ApprovalBlockedTurnRun, ApprovalInteractionScope, ApprovalInteractionService,
    ApprovalResolverPort, ApprovalTurnRunLocator, AuthInteractionService,
    DefaultApprovalInteractionService, DefaultAuthInteractionService,
    RunStateApprovalInteractionReadModel,
};
use brassclaw_reborn::loop_exit_applier::ThreadCheckpointLoopExitEvidencePort;
use brassclaw_reborn::milestone_events::{
    DurableLoopHostMilestoneScope, DurableLoopHostMilestoneSink,
};
use brassclaw_reborn::runtime::{
    DefaultPlannedRuntimeBuildError, DefaultPlannedRuntimeConfig, DefaultPlannedRuntimeParts,
    build_default_planned_runtime,
};
#[cfg(not(feature = "postgres"))]
use brassclaw_reborn::subagent::goal_store::InMemoryBoundedSubagentGoalStore;
use brassclaw_reborn::subagent::{
    flavors::StaticSubagentDefinitionResolver, gate_resolution::BoundedSubagentGateResolutionStore,
};
use brassclaw_reborn::turn_runner::{TurnRunnerWakeSender, TurnRunnerWorkerConfig};
use brassclaw_threads::{
    AcceptInboundMessageRequest, EnsureThreadRequest, MessageContent, MessageKind, MessageStatus,
    SessionThreadService, ThreadHistoryRequest, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, CancelRunRequest, CancelRunResponse, GetRunStateRequest, IdempotencyKey,
    ReplyTargetBindingRef, RunProfileResolutionRequest, SanitizedCancelReason, SourceBindingRef,
    SubmitTurnRequest, SubmitTurnResponse, TurnActor, TurnCoordinator, TurnError,
    TurnEventProjectionSource, TurnId, TurnPersistenceSnapshot, TurnRunId, TurnRunRecord,
    TurnScope, TurnSpawnTreeStateStore, TurnStatus,
    run_profile::{LoopHostMilestoneSink, LoopRunContext},
};

use crate::default_system_prompt::DefaultSystemPromptIdentitySource;
use crate::factory::{LocalDevRootFilesystem, LocalDevTurnStateStore, builtin_extension_registry};
use crate::local_dev_capability_policy::local_dev_capability_policy;
use crate::projection::{RebornProjectionServices, build_reborn_projection_services};
use crate::runtime_input::{
    PollSettings, RebornRuntimeIdentity, RebornRuntimeInput, TriggerPollerAuthorizerConfig,
    TriggerPollerSettings,
};
#[cfg(any(test, feature = "test-support"))]
use crate::trigger_poller::TenantScopedTrustedTriggerFireAuthorizer;
use crate::trigger_poller::{
    AccessCheckerTriggerFireAuthorizer, ConversationContentRefMaterializer,
    LocalTriggerTurnSnapshotSource, SnapshotActiveRunLookup, TRIGGER_POLLER_SHUTDOWN_TIMEOUT,
    TriggerPollerCompositionDeps, TriggerPollerRuntimeHandle, TriggerTurnSnapshotSource,
    spawn_trigger_poller,
};
use crate::{RebornBuildError, RebornProductAuthServices, RebornServices, build_reborn_services};

const MAX_DESCENDANT_CANCEL_NODES: usize = 1_000;

mod approval;
mod auth_interaction;
#[cfg(test)]
#[path = "runtime/tests/auth_interaction.rs"]
mod auth_interaction_tests;
#[cfg(test)]
#[path = "runtime/tests/default_system_prompt.rs"]
mod default_system_prompt_tests;
mod local_dev;
mod run_state_read_source;
// v3 Phase H.12.5: re-export the per-run Tier-0 EffectExecutor builder so the
// `orchestrator_lookup_impl` bridge (crate-root module) can hold/call it
// without widening the whole `local_dev` module's visibility.
#[cfg(feature = "skills-db")]
pub(crate) use local_dev::TierZeroEffectExecutorBuilder;
#[cfg(test)]
#[path = "runtime/test_pg.rs"]
pub(crate) mod test_pg;

#[cfg(feature = "root-llm-provider")]
use crate::runtime_input::ResolvedRebornLlm;

/// Stable identifier for a Reborn CLI conversation. Wraps a `ThreadId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConversationId(pub ThreadId);

/// Final-form assistant reply read back from the session thread service after
/// a `send_user_message` completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantReply {
    pub conversation: ConversationId,
    pub run_id: TurnRunId,
    pub status: TurnStatus,
    pub text: Option<String>,
}

/// Controls the trusted internal-turn path. The current implementation only
/// supports hard-fail Tier-0 execution; ordinary turns must use the public
/// user-message methods.
#[derive(Debug, Clone, Copy)]
pub struct InternalTurnOptions {
    pub hard_fail_on_recipe_miss: bool,
    pub allow_tier_two: bool,
}

impl AssistantReply {
    /// True when a caller can treat the reply as a successful single-shot
    /// response. Recovery/failed/cancelled runs may still produce diagnostics,
    /// but they did not produce the requested assistant text.
    pub fn is_successful_final_reply(&self) -> bool {
        self.status == TurnStatus::Completed && self.text.is_some()
    }
}

/// Errors returned by `RebornRuntime` methods.
#[derive(Debug, Error)]
pub enum RebornRuntimeError {
    #[error("reborn runtime build failed: {0}")]
    Build(#[from] RebornBuildError),
    #[error("turn coordinator unavailable for assembled runtime")]
    TurnCoordinatorUnavailable,
    #[error("host runtime unavailable for assembled runtime")]
    HostRuntimeUnavailable,
    #[error("turn submission failed: {0}")]
    TurnSubmission(String),
    #[error("turn submission rejected: {reason}")]
    TurnRejected { reason: String },
    #[error("session thread service error: {0}")]
    ThreadService(String),
    #[error("turn coordinator error: {0}")]
    TurnCoordinator(String),
    #[error("run did not reach a terminal state within {timeout:?}")]
    RunTimeout { timeout: Duration },
    #[error("run cancelled by caller")]
    OperationCancelled,
    #[error("invalid scope or identifier: {reason}")]
    InvalidArgument { reason: String },
    #[cfg(feature = "root-llm-provider")]
    #[error("llm provider construction failed: {0}")]
    LlmProvider(String),
    #[error("turn-runner worker is no longer running")]
    WorkerStopped,
}

impl From<TurnError> for RebornRuntimeError {
    fn from(value: TurnError) -> Self {
        Self::TurnCoordinator(value.to_string())
    }
}

impl From<DefaultPlannedRuntimeBuildError> for RebornRuntimeError {
    fn from(value: DefaultPlannedRuntimeBuildError) -> Self {
        Self::InvalidArgument {
            reason: value.to_string(),
        }
    }
}

/// Started, running Reborn agent runtime.
///
/// `RebornRuntime` is the single user-facing handle returned by
/// [`build_reborn_runtime`]. Downstream code never reaches into the substrate
/// or worker machinery: it talks to the runtime through task-level methods.
pub struct RebornRuntime {
    services: RebornServices,
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    global_monty_owner: crate::global_monty_owner::GlobalMontyOwner,
    #[cfg(all(
        feature = "postgres",
        feature = "skills-db",
        feature = "root-llm-provider"
    ))]
    completed_turn_review_owner: crate::completed_turn_review::ReviewOwner,
    #[cfg(feature = "skills-db")]
    mcp_recipe_discovery: Arc<crate::mcp_recipe_catalogue::McpRecipeDiscovery>,
    #[cfg(feature = "skills-db")]
    mcp_chat_bridge: std::sync::OnceLock<Arc<crate::mcp_chat_bridge::McpChatBridge>>,
    #[cfg(feature = "skills-db")]
    mcp_listener_status: std::sync::OnceLock<Arc<dyn crate::mcp_server_service::McpListenerStatus>>,
    #[cfg(feature = "skills-db")]
    mcp_provider_binding: Arc<crate::mcp_provider_gateway::McpProviderBinding>,
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    monty_settings_owner: crate::live_monty_settings::MontySettingsOwner,
    #[cfg(all(test, feature = "postgres", feature = "skills-db"))]
    monty_test_controls: (
        Arc<crate::global_monty_driver::GlobalMontyDriver>,
        Arc<crate::global_task_factory::OwnedGlobalTaskFactory>,
    ),
    turn_coordinator: Arc<dyn TurnCoordinator>,
    turn_tree_store: Arc<dyn TurnSpawnTreeStateStore>,
    thread_service: Arc<dyn SessionThreadService>,
    thread_scope: ThreadScope,
    worker_handle: JoinHandle<()>,
    worker_cancel: CancellationToken,
    trigger_poller_handle: Option<TriggerPollerRuntimeHandle>,
    #[cfg(any(test, feature = "test-support"))]
    trigger_conversation_pairing:
        Option<Arc<dyn brassclaw_conversations::ConversationActorPairingService>>,
    budget_event_projection: Option<crate::budget_events::BudgetEventProjection>,
    #[cfg(any(test, feature = "test-support"))]
    resource_governor: Arc<dyn brassclaw_resources::ResourceGovernor>,
    #[cfg(any(test, feature = "test-support"))]
    budget_gate_store: Arc<dyn brassclaw_resources::BudgetGateStore>,
    broadcast_budget_sink: Arc<brassclaw_resources::BroadcastBudgetEventSink>,
    #[cfg(any(test, feature = "test-support"))]
    in_memory_budget_sink: Arc<brassclaw_resources::InMemoryBudgetEventSink>,
    poll_settings: PollSettings,
    actor_user_id: UserId,
    source_binding_ref: SourceBindingRef,
    reply_target_binding_ref: ReplyTargetBindingRef,
    projection_services: RebornProjectionServices,
    approval_interaction_service: Arc<dyn ApprovalInteractionService>,
    auth_interaction_service: Arc<dyn AuthInteractionService>,
    #[cfg(test)]
    approval_audit_sink: Arc<InMemoryAuditSink>,
    webui_event_log: Arc<dyn DurableEventLog>,
    default_run_profile_id: String,
    wake_sender: TurnRunnerWakeSender,
    send_locks: Mutex<HashMap<ConversationId, Arc<Mutex<()>>>>,
    internal_conversation_scopes:
        Mutex<HashMap<ConversationId, (brassclaw_threads::ThreadScope, TurnScope)>>,
    skill_activation_source: Option<Arc<SelectableSkillContextSource>>,
    /// Plan library processor: active when `plan_library_enabled = true`.
    /// After each completed turn, scores the session and persists plan docs.
    plan_library: Option<Arc<crate::plan_library::PlanLibraryService<LocalDevRootFilesystem>>>,
    /// Shared plan-state slot written by the post-turn bridge (reserved for
    /// future use when a state-port decorator feeds it without checkpoint I/O).
    #[allow(dead_code)]
    plan_state_slot: Option<crate::plan_library::CurrentPlanStateSlot>,
    /// Hot-swap handle for the live LLM provider, when one was wired at boot.
    #[cfg(feature = "root-llm-provider")]
    llm_reload: Option<RebornLlmReloadParts>,
    /// Shared interceptor mode flag.  Flipped by the settings service when the
    /// operator connects or disconnects a Sempai provider; consumed by
    /// `RebornLoopDriverHost` on every turn to decide routing vs rerouting.
    /// `None` in DB-less mode or when `root-llm-provider` is not active.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    interceptor_mode: Option<brassclaw_interceptor::SharedInterceptorMode>,
}

// Known tech-debt (#4416): when a second test-only handle is
// needed off the trigger poller seam (e.g. trusted_submitter,
// materializer, active_run_lookup for cleanup-state tests), consolidate
// the cfg-gated fields into a dedicated `TriggerPollerTestHandles`
// struct exposed via a single `RebornRuntime::trigger_poller_test_handles()`
// accessor. That removes the current `TriggerPollerServices` /
// `TriggerPollerServicesInner` split (review f-ptr-1/f-ptr-2) without
// inventing cfg-gated function parameters. Premature today: only one
// test-only handle exists, so the shape isn't proven yet.
struct TriggerPollerServices {
    materializer: Arc<dyn brassclaw_triggers::TriggerPromptMaterializer>,
    trusted_submitter: Arc<dyn brassclaw_triggers::TrustedTriggerFireSubmitter>,
    /// Test-support handle on the SAME conversation services instance the
    /// poller-side materializer/submitter use, so integration tests can call
    /// the production `pair_external_actor` API to seed the trigger
    /// creator's actor pairing before driving the poller. Without this
    /// pre-seed, real `ConversationContentRefMaterializer` fails closed with
    /// `BindingRequired` — by design — and the trusted-ingress turn is
    /// never submitted.
    #[cfg(any(test, feature = "test-support"))]
    pairing_service: Arc<dyn brassclaw_conversations::ConversationActorPairingService>,
}

#[allow(clippy::too_many_arguments)]
async fn build_trigger_poller_services(
    local_runtime: Option<&crate::factory::RebornLocalRuntimeServices>,
    #[cfg(feature = "postgres")] pg_pool: Option<&Arc<deadpool_postgres::Pool>>,
    turn_coordinator: Arc<dyn TurnCoordinator>,
    thread_service: Arc<dyn SessionThreadService>,
    authorizer_config: TriggerPollerAuthorizerConfig,
    access_checker: Option<Arc<dyn crate::runtime_input::TriggerFireAccessChecker>>,
    tenant_id: TenantId,
    default_agent_id: AgentId,
) -> Result<TriggerPollerServices, RebornRuntimeError> {
    let authorizer =
        build_trigger_fire_authorizer(authorizer_config, access_checker, tenant_id.clone())?;

    // Resolve conversation services: filesystem-backed for LocalDev, PG-backed
    // for the pure-Postgres path.
    #[cfg(feature = "postgres")]
    if local_runtime.is_none() {
        let pool = pg_pool.ok_or_else(|| RebornRuntimeError::InvalidArgument {
            reason: "trigger poller on pure-postgres path requires a Postgres pool".to_string(),
        })?;
        let conversations = brassclaw_conversations::InMemoryConversationServices::with_pg_store(
            (**pool).clone(),
            tenant_id.as_str(),
        )
        .await
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: format!("pg conversation services unavailable: {error}"),
        })?;
        #[cfg(any(test, feature = "test-support"))]
        let pairing_service: Arc<
            dyn brassclaw_conversations::ConversationActorPairingService,
        > = Arc::new(conversations.clone());
        let TriggerPollerServicesInner {
            materializer,
            trusted_submitter,
        } = build_trigger_poller_services_from_conversation_services(
            conversations.clone(),
            conversations,
            turn_coordinator,
            thread_service,
            default_agent_id,
            authorizer,
        );
        return Ok(TriggerPollerServices {
            materializer,
            trusted_submitter,
            #[cfg(any(test, feature = "test-support"))]
            pairing_service,
        });
    }

    let local_runtime = local_runtime.ok_or_else(|| RebornRuntimeError::InvalidArgument {
        reason: "trigger poller requires a local-dev runtime substrate or a Postgres pool for \
                 conversation services"
            .to_string(),
    })?;
    let conversations = local_runtime
        .durable_trigger_conversation_services()
        .await
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: format!("trigger conversation services unavailable: {error}"),
        })?;
    #[cfg(any(test, feature = "test-support"))]
    let pairing_service: Arc<dyn brassclaw_conversations::ConversationActorPairingService> =
        Arc::new(conversations.clone());
    let TriggerPollerServicesInner {
        materializer,
        trusted_submitter,
    } = build_trigger_poller_services_from_conversation_services(
        conversations.clone(),
        conversations,
        turn_coordinator,
        thread_service,
        default_agent_id,
        authorizer,
    );
    Ok(TriggerPollerServices {
        materializer,
        trusted_submitter,
        #[cfg(any(test, feature = "test-support"))]
        pairing_service,
    })
}

fn trigger_poller_authorization_required_error() -> RebornRuntimeError {
    RebornRuntimeError::InvalidArgument {
        reason: "trigger poller cannot be enabled without a fire-time creator access checker"
            .to_string(),
    }
}

/// Validate the temporary trigger-poller authorizer shape after the caller has
/// already decided to enable the poller.
fn validate_trigger_poller_authorization(
    trigger_poller: &TriggerPollerSettings,
    access_checker: Option<&Arc<dyn crate::runtime_input::TriggerFireAccessChecker>>,
) -> Result<(), RebornRuntimeError> {
    debug_assert!(trigger_poller.enabled);
    match trigger_poller.authorizer {
        #[cfg(any(test, feature = "test-support"))]
        TriggerPollerAuthorizerConfig::TenantScopedPlaceholderForTest => Ok(()),
        TriggerPollerAuthorizerConfig::CreatorAccessRequired => access_checker
            .map(|_| ())
            .ok_or_else(trigger_poller_authorization_required_error),
    }
}

fn build_trigger_fire_authorizer(
    authorizer_config: TriggerPollerAuthorizerConfig,
    access_checker: Option<Arc<dyn crate::runtime_input::TriggerFireAccessChecker>>,
    tenant_id: TenantId,
) -> Result<Arc<dyn crate::trigger_poller_trusted_submit::TriggerFireAuthorizer>, RebornRuntimeError>
{
    #[cfg(not(any(test, feature = "test-support")))]
    let _ = tenant_id;
    match authorizer_config {
        #[cfg(any(test, feature = "test-support"))]
        TriggerPollerAuthorizerConfig::TenantScopedPlaceholderForTest => Ok(Arc::new(
            TenantScopedTrustedTriggerFireAuthorizer::new(tenant_id),
        )),
        TriggerPollerAuthorizerConfig::CreatorAccessRequired => access_checker
            .map(|checker| {
                Arc::new(AccessCheckerTriggerFireAuthorizer::new(checker))
                    as Arc<dyn crate::trigger_poller_trusted_submit::TriggerFireAuthorizer>
            })
            .ok_or_else(trigger_poller_authorization_required_error),
    }
}

struct TriggerPollerServicesInner {
    materializer: Arc<dyn brassclaw_triggers::TriggerPromptMaterializer>,
    trusted_submitter: Arc<dyn brassclaw_triggers::TrustedTriggerFireSubmitter>,
}

fn build_trigger_poller_services_from_conversation_services<B, S>(
    binding_service: B,
    session_thread_service: S,
    turn_coordinator: Arc<dyn TurnCoordinator>,
    thread_service: Arc<dyn SessionThreadService>,
    default_agent_id: AgentId,
    authorizer: Arc<dyn crate::trigger_poller_trusted_submit::TriggerFireAuthorizer>,
) -> TriggerPollerServicesInner
where
    B: brassclaw_conversations::ConversationBindingService + Clone + 'static,
    S: brassclaw_conversations::SessionThreadService + 'static,
{
    let materializer = Arc::new(ConversationContentRefMaterializer::new(
        binding_service.clone(),
        Arc::clone(&thread_service),
        default_agent_id.clone(),
        authorizer,
    ));
    let trusted_submitter = brassclaw_conversations::trusted_trigger_fire_submitter(
        binding_service,
        session_thread_service,
        turn_coordinator,
    );
    TriggerPollerServicesInner {
        materializer,
        trusted_submitter,
    }
}

fn build_trigger_active_run_lookup(
    local_dev_turn_state: Option<Arc<LocalDevTurnStateStore>>,
) -> Arc<dyn brassclaw_triggers::TriggerActiveRunLookup> {
    let snapshot_source: Arc<dyn TriggerTurnSnapshotSource> = match local_dev_turn_state {
        Some(store) => Arc::new(LocalTriggerTurnSnapshotSource::new(store)),
        // Pure-PG path: no in-process snapshot. Trigger poller handles
        // duplicate-fire via DB-level trigger locking / idempotency keys.
        None => Arc::new(EmptyTriggerTurnSnapshotSource),
    };
    Arc::new(SnapshotActiveRunLookup::new(snapshot_source))
}

/// No-op snapshot source for the pure-PG path.
///
/// The PG trigger poller handles duplicate-fire via DB-level trigger
/// locking and idempotency keys rather than an in-process turn snapshot.
struct EmptyTriggerTurnSnapshotSource;

#[async_trait::async_trait]
impl TriggerTurnSnapshotSource for EmptyTriggerTurnSnapshotSource {
    async fn snapshot(
        &self,
    ) -> Result<brassclaw_turns::TurnPersistenceSnapshot, brassclaw_triggers::TriggerError> {
        Ok(brassclaw_turns::TurnPersistenceSnapshot::default())
    }
}

/// Uses the same scoped durable/local state reader as auth interactions.
/// An approval record alone cannot identify a parked run.
struct RuntimeApprovalTurnRunLocator {
    turn_state: Arc<dyn run_state_read_source::RunStateReadSource>,
}

impl RuntimeApprovalTurnRunLocator {
    async fn snapshot(
        &self,
        scope: &TurnScope,
    ) -> Result<TurnPersistenceSnapshot, brassclaw_product_workflow::ProductWorkflowError> {
        self.turn_state
            .snapshot_for_scope(scope)
            .await
            .map_err(|error| {
                tracing::error!(%error, "approval run state read failed");
                brassclaw_product_workflow::ProductWorkflowError::Transient {
                    reason: "approval run state unavailable".to_owned(),
                }
            })
    }
}

#[async_trait::async_trait]
impl ApprovalTurnRunLocator for RuntimeApprovalTurnRunLocator {
    async fn blocked_approval_runs(
        &self,
        scope: &ApprovalInteractionScope,
    ) -> Result<Vec<ApprovalBlockedTurnRun>, brassclaw_product_workflow::ProductWorkflowError> {
        let turn_scope = scope.turn_scope();
        let actor = TurnActor::new(scope.user_id.clone());
        let snapshot = self.snapshot(&turn_scope).await?;
        let mut runs = snapshot
            .runs
            .iter()
            .filter(|run| {
                run.scope == turn_scope
                    && run.status == TurnStatus::BlockedApproval
                    && run.gate_ref.is_some()
                    && snapshot_run_actor_matches(&snapshot, run, &actor)
            })
            .filter_map(|run| {
                run.gate_ref.clone().map(|gate_ref| ApprovalBlockedTurnRun {
                    run_id: run.run_id,
                    gate_ref,
                })
            })
            .collect::<Vec<_>>();
        runs.sort_by_key(|run| run.run_id.as_uuid());
        Ok(runs)
    }

    async fn approval_run_for_gate(
        &self,
        scope: &ApprovalInteractionScope,
        gate_ref: &brassclaw_turns::GateRef,
    ) -> Result<Option<TurnRunId>, brassclaw_product_workflow::ProductWorkflowError> {
        let turn_scope = scope.turn_scope();
        let actor = TurnActor::new(scope.user_id.clone());
        let snapshot = self.snapshot(&turn_scope).await?;
        let active = snapshot
            .runs
            .iter()
            .find(|run| {
                run.scope == turn_scope
                    && run.status == TurnStatus::BlockedApproval
                    && run.gate_ref.as_ref() == Some(gate_ref)
                    && snapshot_run_actor_matches(&snapshot, run, &actor)
            })
            .map(|run| run.run_id);
        if active.is_some() {
            return Ok(active);
        }

        let mut historical = snapshot
            .checkpoints
            .iter()
            .filter(|checkpoint| {
                checkpoint.status == TurnStatus::BlockedApproval
                    && &checkpoint.gate_ref == gate_ref
                    && checkpoint
                        .scope
                        .as_ref()
                        .is_none_or(|stored| stored == &turn_scope)
            })
            .filter_map(|checkpoint| {
                snapshot
                    .runs
                    .iter()
                    .find(|run| {
                        run.run_id == checkpoint.run_id
                            && run.scope == turn_scope
                            && snapshot_run_actor_matches(&snapshot, run, &actor)
                    })
                    .map(|run| run.run_id)
            })
            .collect::<Vec<_>>();
        historical.sort_by_key(|run_id| run_id.as_uuid());
        historical.dedup();
        Ok(historical.into_iter().next())
    }
}

fn snapshot_run_actor_matches(
    snapshot: &TurnPersistenceSnapshot,
    run: &TurnRunRecord,
    actor: &TurnActor,
) -> bool {
    snapshot
        .turns
        .iter()
        .any(|turn| turn.turn_id == run.turn_id && turn.scope == run.scope && turn.actor == *actor)
}

impl RebornRuntime {
    /// Read-only discovery from the same qualified catalogue as ordinary chat.
    /// This supplies no chat execution adapter or provider connection.
    #[cfg(feature = "skills-db")]
    pub fn mcp_recipe_discovery(&self) -> Arc<crate::mcp_recipe_catalogue::McpRecipeDiscovery> {
        self.mcp_recipe_discovery.clone()
    }

    /// Attach the ordinary-chat transport to this runtime without a strong
    /// ownership cycle. Products retain one bridge for the listener lifetime.
    #[cfg(feature = "skills-db")]
    pub fn mcp_chat_bridge(
        self: &Arc<Self>,
    ) -> Result<Arc<crate::mcp_chat_bridge::McpChatBridge>, RebornRuntimeError> {
        let pool = self
            .services
            .pg_pool
            .as_ref()
            .ok_or(RebornRuntimeError::HostRuntimeUnavailable)?
            .clone();
        let owner_scope = serde_json::to_value(&self.thread_scope).map_err(|error| {
            RebornRuntimeError::InvalidArgument {
                reason: error.to_string(),
            }
        })?;
        Ok(self
            .mcp_chat_bridge
            .get_or_init(|| {
                crate::mcp_chat_bridge::McpChatBridge::new(
                    Arc::downgrade(self),
                    pool,
                    self.mcp_recipe_discovery(),
                    owner_scope,
                )
            })
            .clone())
    }

    #[cfg(feature = "skills-db")]
    pub fn attach_mcp_listener_status(
        &self,
        status: Arc<dyn crate::mcp_server_service::McpListenerStatus>,
    ) -> Result<(), RebornRuntimeError> {
        let observation = status.status();
        if observation.state != crate::mcp_server_service::McpServerState::Running {
            return Err(RebornRuntimeError::HostRuntimeUnavailable);
        }
        let endpoint = observation
            .endpoint_url
            .as_deref()
            .ok_or(RebornRuntimeError::HostRuntimeUnavailable)?;
        let bridge = self
            .mcp_chat_bridge
            .get()
            .ok_or(RebornRuntimeError::HostRuntimeUnavailable)?;
        self.mcp_provider_binding
            .activate(bridge, endpoint)
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: error.to_string(),
            })?;
        self.mcp_listener_status
            .set(status)
            .map_err(|_| RebornRuntimeError::InvalidArgument {
                reason: "MCP listener is already attached".into(),
            })
    }

    #[cfg(feature = "skills-db")]
    pub(crate) fn mcp_listener_status(
        &self,
    ) -> Option<Arc<dyn crate::mcp_server_service::McpListenerStatus>> {
        self.mcp_listener_status.get().cloned()
    }

    /// Snapshot of the substrate facades produced by `build_reborn_services`.
    /// Exposed for diagnostics / readiness reporting; **not** for traffic.
    pub fn services(&self) -> &RebornServices {
        &self.services
    }

    /// The runtime's NEAR AI session manager, when an LLM seam is wired. The
    /// LLM-config service uses it so a completed NEAR AI login applies to the
    /// live provider on reload.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn webui_llm_session(&self) -> Option<Arc<brassclaw_llm::SessionManager>> {
        self.llm_reload
            .as_ref()
            .map(|parts| Arc::clone(&parts.session))
    }

    /// Shared NEAR AI login-state store. The authenticated start endpoint
    /// issues states and the public callback consumes them.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn webui_nearai_login_states(
        &self,
    ) -> Option<Arc<crate::llm_config_service::NearAiLoginStateStore>> {
        self.llm_reload
            .as_ref()
            .map(|parts| Arc::clone(&parts.nearai_login_states))
    }

    /// Public NEAR AI login callback mount for the host ingress to merge via
    /// [`crate::webui_serve::WebuiServeConfig::with_public_route_mount`]. Built
    /// from the runtime's private session/reload/boot so those stay internal.
    /// `None` when no LLM seam or boot config was wired.
    #[cfg(feature = "root-llm-provider")]
    pub fn nearai_login_callback_mount(&self) -> Option<crate::webui_serve::PublicRouteMount> {
        let session = self.webui_llm_session()?;
        let reload = self.webui_llm_reload_trigger()?;
        let states = self.webui_nearai_login_states()?;
        Some(crate::nearai_login_serve::nearai_login_callback_mount(
            session, reload, states,
        ))
    }

    /// Live LLM-provider reload trigger for the settings service. Returns the
    /// bare adapter (not yet Arc-wrapped) when an LLM provider was wired at
    /// boot and a Postgres pool is available; otherwise `None`.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn webui_llm_reload_adapter(
        &self,
    ) -> Option<crate::llm_reload::RebornLlmReloadAdapter> {
        let parts = self.llm_reload.as_ref()?;
        #[cfg(feature = "postgres")]
        let pool = self.services.pg_pool.as_ref()?;
        #[cfg(not(feature = "postgres"))]
        return None;
        #[cfg(feature = "postgres")]
        {
            let tenant_id = self.thread_scope.tenant_id.as_str().to_string();
            let adapter = crate::llm_reload::RebornLlmReloadAdapter::new(
                Arc::clone(&parts.reload_handle),
                Arc::clone(&parts.session),
                crate::LlmKeyStore::new(self.services.secret_store()),
                Arc::clone(pool),
                tenant_id,
            );
            Some(adapter)
        }
    }

    /// Convenience wrapper that returns the adapter already `Arc`-wrapped.
    /// Use `webui_llm_reload_adapter()` when you need to attach a callback first.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn webui_llm_reload_trigger(&self) -> Option<Arc<dyn crate::LlmReloadTrigger>> {
        self.webui_llm_reload_adapter()
            .map(|adapter| Arc::new(adapter) as Arc<dyn crate::LlmReloadTrigger>)
    }

    /// The Sempai live-swap wrapper, when allocated at boot.  The LLM-config
    /// settings service uses this to hot-swap the Sempai provider without a
    /// restart.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn sempai_swappable(&self) -> Option<Arc<brassclaw_llm::SwappableLlmProvider>> {
        self.llm_reload
            .as_ref()
            .and_then(|r| r.sempai_swappable.clone())
    }

    /// The Sempai model gateway, when allocated at boot.  The interceptor host
    /// reads this via the composition's `build_reborn_runtime` path so the
    /// `on_prompt_assembled` hook can call the Sempai provider directly.
    #[cfg(feature = "root-llm-provider")]
    pub(crate) fn sempai_gateway(
        &self,
    ) -> Option<Arc<dyn brassclaw_loop_support::HostManagedModelGateway>> {
        self.llm_reload
            .as_ref()
            .and_then(|r| r.sempai_gateway.clone())
    }

    /// The shared interceptor mode flag.  The LLM-config settings service flips
    /// this when the operator activates or deactivates the Sempai provider.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    pub(crate) fn interceptor_mode(&self) -> Option<brassclaw_interceptor::SharedInterceptorMode> {
        self.interceptor_mode.clone()
    }

    /// Diagnostic id for the no-profile run profile selected by this runtime.
    pub fn default_run_profile_id(&self) -> &str {
        &self.default_run_profile_id
    }

    /// Test-only accessor for the composition-owned trigger repository so
    /// integration tests can seed `TriggerRecord` rows that the spawned
    /// trigger poller will observe through its production read path. Returns
    /// `None` when the runtime was built without a local-runtime substrate
    /// (e.g. production-shape profiles that haven't been wired end-to-end
    /// yet). Gated behind `test-support` so the substrate handle never leaks
    /// into production builds. Mirrors the production read path exercised by
    /// the spawned trigger poller worker, which calls
    /// `TriggerRepository::list_due_triggers` on every tick and the
    /// per-trigger `claim_due_fire` / `mark_fire_*` mutation methods.
    #[cfg(any(test, feature = "test-support"))]
    pub fn trigger_repository(&self) -> Option<Arc<dyn brassclaw_triggers::TriggerRepository>> {
        self.services
            .local_runtime
            .as_ref()
            .map(|local_runtime| Arc::clone(&local_runtime.trigger_repository))
    }

    /// Test-only accessor for the SAME `ConversationActorPairingService`
    /// instance the spawned trigger poller's
    /// [`ConversationContentRefMaterializer`] consults. Integration tests
    /// use this to call the production `pair_external_actor` API and seed
    /// the trigger creator's actor pairing — without it, the materializer
    /// fails closed with `BindingRequired` (by design: trigger fires never
    /// auto-pair unknown actors). Returns `None` when the trigger poller
    /// wasn't built for this runtime (poller disabled). Gated behind
    /// `test-support` so the conversation handle never leaks into
    /// production builds.
    #[cfg(any(test, feature = "test-support"))]
    pub fn trigger_conversation_pairing(
        &self,
    ) -> Option<Arc<dyn brassclaw_conversations::ConversationActorPairingService>> {
        self.trigger_conversation_pairing.as_ref().map(Arc::clone)
    }

    /// Open the canonical Reborn identity resolver backed by the Postgres pool
    /// threaded from `build_postgres_production`. Returns `None` when no pool
    /// is wired (in-memory / disabled-profile runtimes), so callers fail
    /// closed.
    #[cfg(feature = "postgres")]
    pub async fn open_reborn_identity_resolver(
        &self,
        _tenant_id: &TenantId,
    ) -> Option<
        Result<
            Arc<dyn brassclaw_reborn_identity::RebornIdentityResolver>,
            brassclaw_reborn_identity::RebornIdentityError,
        >,
    > {
        let pool = self.services.pg_pool.as_ref()?;
        let store = brassclaw_reborn_identity::PgRebornIdentityStore::new((**pool).clone());
        Some(Ok(Arc::new(store)
            as Arc<
                dyn brassclaw_reborn_identity::RebornIdentityResolver,
            >))
    }

    /// The tenant ID used by this runtime. Needed by WebUI services that
    /// write to `brassclaw_config` (interceptor config, recipe store, etc.).
    #[cfg(feature = "postgres")]
    pub(crate) fn webui_tenant_id(&self) -> &str {
        self.thread_scope.tenant_id.as_str()
    }

    /// Agent-id string for the runtime's thread scope.  Used when wiring
    /// per-scope Postgres stores (e.g. `PgMontyVmSettingsStore`).
    #[cfg(feature = "postgres")]
    pub(crate) fn webui_agent_id(&self) -> &str {
        self.thread_scope.agent_id.as_str()
    }

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    pub(crate) fn webui_monty_settings_store(
        &self,
    ) -> Arc<dyn brassclaw_product_workflow::MontyVmSettingsStore> {
        self.monty_settings_owner.store()
    }

    pub(crate) fn webui_thread_service(&self) -> Arc<dyn SessionThreadService> {
        self.thread_service.clone()
    }

    pub(crate) fn webui_turn_coordinator(&self) -> Arc<dyn TurnCoordinator> {
        self.turn_coordinator.clone()
    }

    pub(crate) fn webui_event_stream(&self) -> Arc<dyn ProjectionStream> {
        self.projection_services.webui_event_stream()
    }

    pub(crate) fn webui_approval_interaction_service(&self) -> Arc<dyn ApprovalInteractionService> {
        self.approval_interaction_service.clone()
    }

    pub(crate) fn webui_auth_interaction_service(&self) -> Arc<dyn AuthInteractionService> {
        self.auth_interaction_service.clone()
    }

    #[cfg(test)]
    fn webui_approval_audit_sink(&self) -> Arc<InMemoryAuditSink> {
        self.approval_audit_sink.clone()
    }

    pub(crate) fn webui_skill_activation_source(
        &self,
    ) -> Option<Arc<SelectableSkillContextSource>> {
        self.skill_activation_source.clone()
    }

    /// Test-only handle on the resource governor backing the budget
    /// accountant. Exposed under `test-support` so integration tests can
    /// assert ledger state after a `send_user_message` round-trip.
    #[cfg(any(test, feature = "test-support"))]
    pub fn budget_resource_governor(
        &self,
    ) -> Option<Arc<dyn brassclaw_resources::ResourceGovernor>> {
        Some(Arc::clone(&self.resource_governor))
    }

    /// Test-only handle on the in-memory budget event sink wired to the
    /// governor. Tests use `.drain()` / `.snapshot()` to inspect the
    /// audit-event stream produced by a run.
    #[cfg(any(test, feature = "test-support"))]
    pub fn budget_event_sink(&self) -> Option<Arc<brassclaw_resources::InMemoryBudgetEventSink>> {
        Some(Arc::clone(&self.in_memory_budget_sink))
    }

    /// Broadcast sink that fans every emitted `BudgetEvent` to any
    /// subscriber. The runtime always spawns its own subscriber — the
    /// [`crate::budget_events::BudgetEventProjection`] task wired by
    /// `build_reborn_runtime` and shut down via [`Self::shutdown`] —
    /// so this sink is never a no-op even when the caller does not
    /// install a custom observer (review feedback Thermo-Nuclear #3
    /// / follow-up A2). Callers that need a richer projection
    /// (multi-channel fan-out, telemetry exporters) should pass an
    /// observer through
    /// [`crate::RebornRuntimeInput::with_budget_event_observer`]
    /// rather than re-subscribing here; spawning a second long-lived
    /// receiver risks one of them lagging while the other drains.
    pub fn broadcast_budget_event_sink(
        &self,
    ) -> Option<Arc<brassclaw_resources::BroadcastBudgetEventSink>> {
        Some(Arc::clone(&self.broadcast_budget_sink))
    }

    /// Test-only handle on the budget approval-gate store. Tests resolve
    /// pending gates here (Approve / Cancel / let-expire) to drive the
    /// F3/F4/F5 approval-flow scenarios.
    #[cfg(any(test, feature = "test-support"))]
    pub fn budget_gate_store(&self) -> Option<Arc<dyn brassclaw_resources::BudgetGateStore>> {
        Some(Arc::clone(&self.budget_gate_store))
    }

    /// Apply the outcome of a resolved [`BudgetApprovalGate`]: when the
    /// gate is approved, raise the affected account's limit so a
    /// subsequent `send_user_message` can re-issue the reservation that
    /// previously crossed the pause threshold. Returns the resolved
    /// gate.
    ///
    /// Production wires this through a gate-resolution route on the web
    /// gateway; the test-only accessor lets E2E tests drive F3 / F4 / F5
    /// without booting that surface.
    #[cfg(any(test, feature = "test-support"))]
    pub fn apply_resolved_budget_gate(
        &self,
        scope: &brassclaw_host_api::ResourceScope,
        gate_id: brassclaw_resources::BudgetGateId,
    ) -> Result<brassclaw_resources::BudgetApprovalGate, RebornRuntimeError> {
        let gate = self
            .budget_gate_store
            .get(scope, gate_id)
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: format!("budget gate read failed: {error}"),
            })?
            .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                reason: format!("unknown budget gate: {gate_id}"),
            })?;
        if let brassclaw_resources::BudgetGateStatus::Approved {
            increased_limit, ..
        } = &gate.status
        {
            self.resource_governor
                .set_limit(gate.needed.account.clone(), increased_limit.clone())
                .map_err(|error| RebornRuntimeError::InvalidArgument {
                    reason: format!("failed to apply approved budget limit: {error}"),
                })?;
        }
        Ok(gate)
    }

    /// Create a fresh conversation. Returns the opaque conversation id used
    /// in subsequent `send_user_message` calls.
    ///
    /// The thread is materialized inside the session thread service so
    /// `accept_inbound_message` does not error on the first send.
    pub async fn new_conversation(&self) -> Result<ConversationId, RebornRuntimeError> {
        self.ensure_correlated_conversation(Uuid::new_v4()).await
    }

    /// Materialize the ordinary chat identified by an already persisted UUID.
    pub async fn ensure_correlated_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<ConversationId, RebornRuntimeError> {
        let thread_id =
            ThreadId::new(format!("reborn-conv-{conversation_id}")).map_err(|reason| {
                RebornRuntimeError::InvalidArgument {
                    reason: reason.to_string(),
                }
            })?;
        self.thread_service
            .ensure_thread(EnsureThreadRequest {
                scope: self.thread_scope.clone(),
                thread_id: Some(thread_id.clone()),
                created_by_actor_id: self.actor_user_id.as_str().to_string(),
                title: None,
                metadata_json: None,
            })
            .await
            .map_err(|error| RebornRuntimeError::ThreadService(error.to_string()))?;
        Ok(ConversationId(thread_id))
    }

    /// Create an internal conversation whose data scope is derived from the
    /// authenticated caller and requested project reference.
    pub async fn new_internal_conversation(
        &self,
        tenant_id: &str,
        user_id: &str,
        project_id: &str,
    ) -> Result<ConversationId, RebornRuntimeError> {
        let tenant = TenantId::new(tenant_id).map_err(|e| RebornRuntimeError::InvalidArgument {
            reason: e.to_string(),
        })?;
        let user = UserId::new(user_id).map_err(|e| RebornRuntimeError::InvalidArgument {
            reason: e.to_string(),
        })?;
        let project =
            ProjectId::new(project_id).map_err(|e| RebornRuntimeError::InvalidArgument {
                reason: e.to_string(),
            })?;
        let thread_id =
            ThreadId::new(format!("reborn-internal-{}", Uuid::new_v4())).map_err(|reason| {
                RebornRuntimeError::InvalidArgument {
                    reason: reason.to_string(),
                }
            })?;
        let thread_scope = brassclaw_threads::ThreadScope {
            tenant_id: tenant.clone(),
            agent_id: self.thread_scope.agent_id.clone(),
            project_id: Some(project.clone()),
            owner_user_id: Some(user.clone()),
        };
        self.thread_service
            .ensure_thread(EnsureThreadRequest {
                scope: thread_scope.clone(),
                thread_id: Some(thread_id.clone()),
                created_by_actor_id: user.as_str().to_string(),
                title: None,
                metadata_json: None,
            })
            .await
            .map_err(|error| RebornRuntimeError::ThreadService(error.to_string()))?;
        let scope = TurnScope::new_with_owner(
            tenant,
            Some(self.thread_scope.agent_id.clone()),
            Some(project),
            thread_id.clone(),
            Some(user),
        );
        let conversation = ConversationId(thread_id);
        self.internal_conversation_scopes
            .lock()
            .await
            .insert(conversation.clone(), (thread_scope, scope));
        Ok(conversation)
    }

    /// Submit a user message into the conversation, wait for the run to
    /// reach a terminal state, and return the assistant reply read back
    /// from the session thread service.
    ///
    /// Without an LLM gateway wired in (i.e. when this crate is built
    /// without the `root-llm-provider` feature or an LLM config is not
    /// provided), the run will fail and the returned reply will surface
    /// that failure via `status = Failed` and `text = None`.
    pub async fn send_user_message(
        &self,
        conversation: &ConversationId,
        text: &str,
    ) -> Result<AssistantReply, RebornRuntimeError> {
        self.send_user_message_with_cancellation(conversation, text, CancellationToken::new())
            .await
    }

    /// Submit a user message with a cooperative cancellation token. If the
    /// token fires while waiting for completion, the runtime cancels the run
    /// before returning.
    pub async fn send_user_message_with_cancellation(
        &self,
        conversation: &ConversationId,
        text: &str,
        cancellation: CancellationToken,
    ) -> Result<AssistantReply, RebornRuntimeError> {
        self.send_user_message_internal(conversation, text, cancellation, None, None)
            .await
    }

    /// Submit a host-created internal message using a reserved source binding.
    /// The marker reaches the loop through persisted turn state, never through
    /// message text, and causes Tier-0 misses/errors to stop before Tier 2.
    pub async fn send_internal_user_message(
        &self,
        conversation: &ConversationId,
        text: &str,
        options: InternalTurnOptions,
    ) -> Result<AssistantReply, RebornRuntimeError> {
        if !options.hard_fail_on_recipe_miss || options.allow_tier_two {
            return Err(RebornRuntimeError::InvalidArgument {
                reason: "internal turns require hard-fail Tier-0 options".into(),
            });
        }
        let source =
            SourceBindingRef::new(brassclaw_turns::run_profile::TRUSTED_INTERNAL_SOURCE_BINDING)
                .map_err(|reason| RebornRuntimeError::InvalidArgument { reason })?;
        self.send_user_message_internal(
            conversation,
            text,
            CancellationToken::new(),
            Some(source),
            None,
        )
        .await
    }

    /// Submit an ordinary chat turn using a transport's previously persisted
    /// correlation UUID. Repeating the exact submission retains message and run
    /// identity; this grants no special matching or execution authority.
    pub async fn send_correlated_user_message(
        &self,
        conversation: &ConversationId,
        text: &str,
        correlation_id: Uuid,
        cancellation: CancellationToken,
    ) -> Result<AssistantReply, RebornRuntimeError> {
        self.send_user_message_internal(
            conversation,
            text,
            cancellation,
            None,
            Some(correlation_id),
        )
        .await
    }

    /// Read the original run after an interrupted transport operation. This
    /// never submits a message or restarts a Recipe.
    pub async fn recover_correlated_user_message(
        &self,
        conversation: &ConversationId,
        correlation_id: Uuid,
    ) -> Result<Option<AssistantReply>, RebornRuntimeError> {
        let run_id = TurnRunId::from_uuid(correlation_id);
        let state = self
            .turn_coordinator
            .get_run_state(GetRunStateRequest {
                scope: self.turn_scope_for(&conversation.0),
                run_id,
            })
            .await?;
        if !state.status.is_terminal() {
            return Ok(None);
        }
        let text = self
            .read_latest_assistant_text(&self.thread_scope, &conversation.0, run_id)
            .await?;
        Ok(Some(AssistantReply {
            conversation: conversation.clone(),
            run_id,
            status: state.status,
            text,
        }))
    }

    #[cfg(feature = "skills-db")]
    pub(crate) async fn correlated_chat_message_refs(
        &self,
        conversation: &ConversationId,
        correlation_id: Uuid,
        expected_command: &str,
    ) -> Result<(String, Option<String>), RebornRuntimeError> {
        let history = self
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: self.thread_scope.clone(),
                thread_id: conversation.0.clone(),
            })
            .await
            .map_err(|error| RebornRuntimeError::ThreadService(error.to_string()))?;
        let run_id = correlation_id.to_string();
        let mut users = history.messages.iter().filter(|message| {
            message.kind == MessageKind::User
                && message.turn_run_id.as_deref() == Some(run_id.as_str())
        });
        let user = users.next().ok_or_else(|| {
            RebornRuntimeError::ThreadService("original submitted message is missing".into())
        })?;
        if user.content.as_deref() != Some(expected_command) {
            return Err(RebornRuntimeError::ThreadService(
                "original command differs from its retained message".into(),
            ));
        }
        if users.next().is_some() {
            return Err(RebornRuntimeError::ThreadService(
                "original submitted message is ambiguous".into(),
            ));
        }
        let mut replies = history.messages.iter().filter(|message| {
            message.kind == MessageKind::Assistant
                && message.status == MessageStatus::Finalized
                && message.turn_run_id.as_deref() == Some(run_id.as_str())
        });
        let reply = replies
            .next()
            .map(|message| format!("msg:{}", message.message_id));
        if replies.next().is_some() {
            return Err(RebornRuntimeError::ThreadService(
                "original reply is ambiguous".into(),
            ));
        }
        Ok((format!("msg:{}", user.message_id), reply))
    }

    /// Close a transport-owned chat only after its exact run is terminal.
    /// Transcript, immutable selections and effect evidence remain retained.
    pub async fn close_correlated_conversation(
        &self,
        conversation: &ConversationId,
        correlation_id: Uuid,
    ) -> Result<(), RebornRuntimeError> {
        let lock = self.send_lock_for(conversation).await;
        let _guard = lock.lock().await;
        let state = self
            .turn_coordinator
            .get_run_state(GetRunStateRequest {
                scope: self.turn_scope_for(&conversation.0),
                run_id: TurnRunId::from_uuid(correlation_id),
            })
            .await?;
        if !state.status.is_terminal() {
            return Err(RebornRuntimeError::InvalidArgument {
                reason: "cannot close a chat with an unsettled run".into(),
            });
        }
        self.thread_service
            .close_thread(&self.thread_scope, &conversation.0)
            .await
            .map_err(|error| RebornRuntimeError::ThreadService(error.to_string()))
    }

    async fn send_user_message_internal(
        &self,
        conversation: &ConversationId,
        text: &str,
        cancellation: CancellationToken,
        source_binding_override: Option<SourceBindingRef>,
        correlation_id: Option<Uuid>,
    ) -> Result<AssistantReply, RebornRuntimeError> {
        if cancellation.is_cancelled() {
            return Err(RebornRuntimeError::OperationCancelled);
        }
        let send_lock = self.send_lock_for(conversation).await;
        let _send_guard = send_lock.lock().await;
        if self.worker_handle.is_finished() {
            return Err(RebornRuntimeError::WorkerStopped);
        }
        let is_internal_turn = source_binding_override.is_some();
        let internal_scope = if is_internal_turn {
            Some(
                self.internal_conversation_scopes
                    .lock()
                    .await
                    .remove(conversation)
                    .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                        reason: "internal conversation scope is missing or already consumed".into(),
                    })?,
            )
        } else {
            None
        };
        let scope = internal_scope
            .as_ref()
            .map(|(_, scope)| scope.clone())
            .unwrap_or_else(|| self.turn_scope_for(&conversation.0));
        let thread_scope = internal_scope
            .as_ref()
            .map(|(thread_scope, _)| thread_scope.clone())
            .unwrap_or_else(|| self.thread_scope.clone());
        let actor_id = scope
            .explicit_owner_user_id()
            .cloned()
            .unwrap_or_else(|| self.actor_user_id.clone());
        if cancellation.is_cancelled() {
            return Err(RebornRuntimeError::OperationCancelled);
        }
        let source_binding =
            source_binding_override.unwrap_or_else(|| self.source_binding_ref.clone());
        let accepted = self
            .thread_service
            .accept_inbound_message(AcceptInboundMessageRequest {
                scope: thread_scope.clone(),
                thread_id: conversation.0.clone(),
                actor_id: actor_id.as_str().to_string(),
                source_binding_id: Some(source_binding.as_str().to_string()),
                reply_target_binding_id: Some(self.reply_target_binding_ref.as_str().to_string()),
                // Transport correlation uses its durable event UUID. Ordinary
                // uncorrelated callers mint a fresh source-scoped event.
                external_event_id: Some(format!(
                    "{}:{}",
                    source_binding.as_str(),
                    correlation_id.unwrap_or_else(Uuid::new_v4)
                )),
                content: MessageContent::text(text.to_string()),
            })
            .await
            .map_err(|error| RebornRuntimeError::ThreadService(error.to_string()))?;

        let accepted_message_ref = AcceptedMessageRef::new(format!("msg:{}", accepted.message_id))
            .map_err(|reason| RebornRuntimeError::InvalidArgument { reason })?;
        let idempotency_key = IdempotencyKey::new(format!(
            "{}-{}",
            source_binding.as_str(),
            correlation_id.unwrap_or_else(Uuid::new_v4)
        ))
        .map_err(|reason| RebornRuntimeError::InvalidArgument { reason })?;

        if !is_internal_turn && let Some(skill_activation_source) = &self.skill_activation_source {
            skill_activation_source
                .record_user_message(scope.clone(), accepted_message_ref.clone(), text)
                .map_err(|error| RebornRuntimeError::TurnSubmission(error.to_string()))?;
        }

        if cancellation.is_cancelled() {
            return Err(RebornRuntimeError::OperationCancelled);
        }
        let response = match self
            .turn_coordinator
            .submit_turn(SubmitTurnRequest {
                scope: scope.clone(),
                actor: TurnActor::new(actor_id),
                accepted_message_ref: accepted_message_ref.clone(),
                source_binding_ref: source_binding,
                reply_target_binding_ref: self.reply_target_binding_ref.clone(),
                requested_run_profile: None,
                idempotency_key,
                received_at: Utc::now(),
                requested_run_id: correlation_id.map(TurnRunId::from_uuid),
                parent_run_id: None,
                subagent_depth: 0,
                spawn_tree_root_run_id: None,
            })
            .await
        {
            Ok(response) => response,
            Err(error) => {
                if let Some(skill_activation_source) = &self.skill_activation_source {
                    skill_activation_source
                        .clear_accepted_message(&scope, &accepted_message_ref)
                        .map_err(|clear_error| {
                            RebornRuntimeError::TurnSubmission(clear_error.to_string())
                        })?;
                }
                return Err(error.into());
            }
        };

        let SubmitTurnResponse::Accepted {
            run_id, turn_id, ..
        } = response;
        if let Err(error) = self
            .thread_service
            .mark_message_submitted(
                &thread_scope,
                &conversation.0,
                accepted.message_id,
                turn_id.to_string(),
                run_id.to_string(),
            )
            .await
        {
            // A run without a committed input must not remain queued forever.
            self.cancel_run(
                &scope,
                run_id,
                SanitizedCancelReason::Policy,
                "input-admission-failed",
            )
            .await?;
            return Err(RebornRuntimeError::ThreadService(error.to_string()));
        }
        if cancellation.is_cancelled() {
            if let Some(skill_activation_source) = &self.skill_activation_source {
                skill_activation_source
                    .clear_accepted_message(&scope, &accepted_message_ref)
                    .map_err(|error| RebornRuntimeError::TurnSubmission(error.to_string()))?;
            }
            self.cancel_run(
                &scope,
                run_id,
                SanitizedCancelReason::UserRequested,
                "caller-cancel",
            )
            .await?;
            return Err(RebornRuntimeError::OperationCancelled);
        }
        self.wake_sender.wake();

        let reply = async {
            let terminal_status = self
                .wait_for_terminal(&scope, run_id, &cancellation)
                .await?;
            let assistant_text = self
                .read_latest_assistant_text(&thread_scope, &conversation.0, run_id)
                .await?;

            Ok(AssistantReply {
                conversation: conversation.clone(),
                run_id,
                status: terminal_status,
                text: assistant_text,
            })
        }
        .await;

        if let Some(skill_activation_source) = &self.skill_activation_source {
            skill_activation_source
                .clear_accepted_message(&scope, &accepted_message_ref)
                .map_err(|error| RebornRuntimeError::TurnSubmission(error.to_string()))?;
        }

        // Post-turn plan library processing — awaited so the plan doc is persisted
        // before the CLI process exits. Errors are swallowed inside the hook.
        if let (Ok(ok_reply), Some(library)) = (reply.as_ref(), self.plan_library.as_ref())
            && let Some(ref local_runtime) = self.services.local_runtime
        {
            let library = Arc::clone(library);
            let fs = Arc::clone(&local_runtime.extension_filesystem);
            let scope_clone = scope.clone();
            let run_id_clone = ok_reply.run_id;
            Self::run_plan_library_post_turn(library, fs, &scope_clone, run_id_clone).await;
        }

        reply
    }

    /// Stop the turn-runner worker and the budget-event projection.
    /// Awaits both tasks before returning so background state is fully
    /// drained when the runtime drops.
    pub async fn shutdown(self) -> Result<(), RebornRuntimeError> {
        if let Some(trigger_poller) = self.trigger_poller_handle {
            trigger_poller
                .shutdown(TRIGGER_POLLER_SHUTDOWN_TIMEOUT)
                .await;
        }
        #[cfg(all(
            feature = "postgres",
            feature = "skills-db",
            feature = "root-llm-provider"
        ))]
        self.completed_turn_review_owner.request_shutdown();
        self.worker_cancel.cancel();
        if let Some(projection) = self.budget_event_projection {
            projection.shutdown().await;
        }
        if let Err(error) = self.worker_handle.await {
            if error.is_panic() {
                tracing::error!(%error, "reborn worker task panicked during shutdown");
            } else {
                tracing::debug!(%error, "reborn worker task was cancelled during shutdown");
            }
        }
        #[cfg(all(feature = "postgres", feature = "skills-db"))]
        {
            let settings_result = self.monty_settings_owner.shutdown().await;
            let mut owner = self.global_monty_owner;
            owner.request_shutdown();
            let exit = owner
                .join()
                .await
                .map_err(|error| RebornRuntimeError::InvalidArgument {
                    reason: error.to_string(),
                })?;
            #[cfg(feature = "root-llm-provider")]
            self.completed_turn_review_owner.join().await?;
            crate::global_monty_startup::check_shutdown(exit)?;
            settings_result.map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: error.to_string(),
            })?;
        }
        Ok(())
    }

    fn turn_scope_for(&self, thread_id: &ThreadId) -> TurnScope {
        TurnScope::new_with_owner(
            self.thread_scope.tenant_id.clone(),
            Some(self.thread_scope.agent_id.clone()),
            self.thread_scope.project_id.clone(),
            thread_id.clone(),
            self.thread_scope.owner_user_id.clone(),
        )
    }
    async fn send_lock_for(&self, conversation: &ConversationId) -> Arc<Mutex<()>> {
        let mut locks = self.send_locks.lock().await;
        Arc::clone(
            locks
                .entry(conversation.clone())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    async fn wait_for_terminal(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        cancellation: &CancellationToken,
    ) -> Result<TurnStatus, RebornRuntimeError> {
        let start = std::time::Instant::now();
        loop {
            if self.worker_handle.is_finished() {
                return Err(RebornRuntimeError::WorkerStopped);
            }
            let state = self
                .turn_coordinator
                .get_run_state(GetRunStateRequest {
                    scope: scope.clone(),
                    run_id,
                })
                .await?;
            if state.status.is_terminal() {
                return Ok(state.status);
            }
            // TurnStatus::RecoveryRequired is now terminal (is_terminal() returns true)
            // so the branch above handles it; no special cancel-to-release-lock is needed.
            if start.elapsed() > self.poll_settings.max_total {
                self.cancel_run(
                    scope,
                    run_id,
                    SanitizedCancelReason::Timeout,
                    "timeout-cancel",
                )
                .await?;
                return Err(RebornRuntimeError::RunTimeout {
                    timeout: self.poll_settings.max_total,
                });
            }
            tokio::select! {
                _ = cancellation.cancelled() => {
                    self.cancel_run(
                        scope,
                        run_id,
                        SanitizedCancelReason::UserRequested,
                        "caller-cancel",
                    )
                    .await?;
                    return Err(RebornRuntimeError::OperationCancelled);
                }
                _ = tokio::time::sleep(self.poll_settings.interval) => {}
            }
        }
    }

    async fn cancel_run(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        reason: SanitizedCancelReason,
        idempotency_suffix: &str,
    ) -> Result<CancelRunResponse, RebornRuntimeError> {
        let response = self
            .turn_coordinator
            .cancel_run(CancelRunRequest {
                scope: scope.clone(),
                actor: TurnActor::new(self.actor_user_id.clone()),
                run_id,
                reason,
                idempotency_key: IdempotencyKey::new(format!(
                    "{}-{}-{}",
                    self.source_binding_ref.as_str(),
                    idempotency_suffix,
                    run_id
                ))
                .map_err(|reason| RebornRuntimeError::InvalidArgument { reason })?,
            })
            .await?;
        let cancellation_accepted = matches!(
            response.status,
            TurnStatus::CancelRequested | TurnStatus::Cancelled
        );
        if cancellation_accepted {
            self.append_webui_loop_cancelled(scope, run_id).await?;
            // Cancel descendants BEFORE waking the scheduler so children in
            // Queued state are marked Cancelled before they can be picked up.
            self.cancel_descendant_runs(scope, run_id, reason, idempotency_suffix)
                .await?;
        }
        self.wake_sender.wake();
        Ok(response)
    }

    async fn cancel_descendant_runs(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
        reason: SanitizedCancelReason,
        idempotency_suffix: &str,
    ) -> Result<(), RebornRuntimeError> {
        let mut stack = self.turn_tree_store.children_of(scope, run_id).await?;
        let mut visited = HashSet::new();
        let mut visited_count = 0usize;
        while let Some(child) = stack.pop() {
            if !visited.insert(child.run_id) {
                continue;
            }
            visited_count += 1;
            if visited_count > MAX_DESCENDANT_CANCEL_NODES {
                tracing::debug!(
                    scope = ?scope,
                    run_id = %run_id,
                    max_nodes = MAX_DESCENDANT_CANCEL_NODES,
                    "stopped descendant cancellation traversal after node budget was reached"
                );
                break;
            }
            if child.status.is_terminal() {
                continue;
            }
            let grandchildren = self
                .turn_tree_store
                .children_of(&child.scope, child.run_id)
                .await?;
            stack.extend(grandchildren);
            let idempotency_key = IdempotencyKey::new(format!(
                "{}-{}-descendant-{}",
                self.source_binding_ref.as_str(),
                idempotency_suffix,
                child.run_id
            ))
            .map_err(|reason| RebornRuntimeError::InvalidArgument { reason })?;
            let child_scope = child.scope.clone();
            let child_run_id = child.run_id;
            let response = self
                .turn_coordinator
                .cancel_run(CancelRunRequest {
                    scope: child_scope.clone(),
                    actor: TurnActor::new(self.actor_user_id.clone()),
                    run_id: child_run_id,
                    reason,
                    idempotency_key,
                })
                .await;
            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    let state = self
                        .turn_coordinator
                        .get_run_state(GetRunStateRequest {
                            scope: child_scope,
                            run_id: child_run_id,
                        })
                        .await?;
                    if matches!(
                        state.status,
                        TurnStatus::CancelRequested | TurnStatus::Cancelled
                    ) {
                        self.wake_sender.wake();
                        continue;
                    }
                    return Err(error.into());
                }
            };
            if matches!(
                response.status,
                TurnStatus::CancelRequested | TurnStatus::Cancelled
            ) {
                self.append_webui_loop_cancelled(&child.scope, child_run_id)
                    .await?;
            }
            self.wake_sender.wake();
        }
        Ok(())
    }

    async fn append_webui_loop_cancelled(
        &self,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) -> Result<(), RebornRuntimeError> {
        let capability_id = CapabilityId::new(LOOP_RUN_CAPABILITY_ID).map_err(|reason| {
            RebornRuntimeError::InvalidArgument {
                reason: format!("loop-run capability id: {reason}"),
            }
        })?;
        self.webui_event_log
            .append(RuntimeEvent::loop_cancelled(
                ResourceScope {
                    tenant_id: scope.tenant_id.clone(),
                    user_id: self.actor_user_id.clone(),
                    agent_id: scope.agent_id.clone(),
                    project_id: scope.project_id.clone(),
                    thread_id: Some(scope.thread_id.clone()),
                    invocation_id: InvocationId::from_uuid(run_id.as_uuid()),
                },
                capability_id,
            ))
            .await
            .map(|_| ())
            .map_err(|error| RebornRuntimeError::TurnCoordinator(error.to_string()))
    }

    async fn read_latest_assistant_text(
        &self,
        scope: &ThreadScope,
        thread_id: &ThreadId,
        run_id: TurnRunId,
    ) -> Result<Option<String>, RebornRuntimeError> {
        let history = self
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: scope.clone(),
                thread_id: thread_id.clone(),
            })
            .await
            .map_err(|error| RebornRuntimeError::ThreadService(error.to_string()))?;
        let run_id_str = run_id.to_string();
        let mut replies = history.messages.into_iter().filter(|message| {
            matches!(message.kind, MessageKind::Assistant)
                && matches!(message.status, MessageStatus::Finalized)
                && message.turn_run_id.as_deref() == Some(run_id_str.as_str())
        });
        let reply = replies.next().and_then(|message| message.content);
        if replies.next().is_some() {
            return Err(RebornRuntimeError::ThreadService(
                "run has ambiguous finalized replies".into(),
            ));
        }
        Ok(reply)
    }

    /// Post-turn hook for the plan library.
    ///
    /// Scans the checkpoint-state filesystem directly for the Final checkpoint
    /// belonging to `run_id`, deserializes `LoopExecutionState`, and calls
    /// `PlanLibraryService::process_session`. All errors are logged at DEBUG
    /// and swallowed — the plan library is best-effort.
    async fn run_plan_library_post_turn(
        library: Arc<crate::plan_library::PlanLibraryService<LocalDevRootFilesystem>>,
        filesystem: Arc<LocalDevRootFilesystem>,
        scope: &TurnScope,
        run_id: TurnRunId,
    ) {
        use brassclaw_agent_loop::state::{CheckpointKind, LoopExecutionState};
        use brassclaw_filesystem::RootFilesystem;
        use brassclaw_host_api::VirtualPath;

        // Build the directory path where checkpoint state files for this thread live.
        // FilesystemCheckpointStateStore stores state records at:
        //   /tenants/{tenant}/users/__system__/checkpoint-state/agents/{agent}/threads/{thread}/states/checkpoint/{uuid}.json
        // The state_ref is "checkpoint:{uuid}" and stored as "checkpoint/{uuid}" under states/.
        // We list the "states/checkpoint" subdirectory directly to get the .json files.
        let dir_path_str = {
            let mut p = format!(
                "/tenants/{}/users/__system__/checkpoint-state",
                scope.tenant_id.as_str()
            );
            if let Some(agent_id) = &scope.agent_id {
                p.push_str("/agents/");
                p.push_str(agent_id.as_str());
            }
            if let Some(project_id) = &scope.project_id {
                p.push_str("/projects/");
                p.push_str(project_id.as_str());
            }
            p.push_str("/threads/");
            p.push_str(scope.thread_id.as_str());
            p.push_str("/states/checkpoint");
            p
        };
        let dir_path = match VirtualPath::new(&dir_path_str) {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!(%run_id, error = %e, "plan library: invalid checkpoint dir path");
                return;
            }
        };

        // List entries in the states directory and find the Final checkpoint for run_id.
        let entries = match filesystem.list_dir(&dir_path).await {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!(%run_id, error = %e, "plan library: could not list checkpoint dir");
                return;
            }
        };

        // Read each .json file, decode the stored record, find the one matching
        // run_id + kind == Final.
        #[derive(serde::Deserialize)]
        struct StoredRecord {
            run_id: TurnRunId,
            kind: brassclaw_turns::LoopCheckpointKind,
            payload_hex: String,
        }

        let run_id_str = run_id.to_string();
        for entry in &entries {
            if entry.file_type == brassclaw_filesystem::FileType::Directory {
                continue;
            }
            let file_path_str = format!("{}/{}", dir_path_str, entry.name);
            let file_path = match VirtualPath::new(&file_path_str) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let bytes = match filesystem.read_file(&file_path).await {
                Ok(b) => b,
                Err(_) => continue,
            };
            let record: StoredRecord = match serde_json::from_slice(&bytes) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if record.run_id.to_string() != run_id_str {
                continue;
            }
            if record.kind != brassclaw_turns::LoopCheckpointKind::Final {
                continue;
            }
            // Decode payload.
            let payload_bytes = match hex::decode(&record.payload_hex) {
                Ok(b) => b,
                Err(e) => {
                    tracing::debug!(%run_id, error = %e, "plan library: payload hex decode failed");
                    return;
                }
            };
            let exec_state = match LoopExecutionState::from_checkpoint_payload(
                &payload_bytes,
                CheckpointKind::Final,
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!(%run_id, error = %e, "plan library: state deserialization failed");
                    return;
                }
            };
            let tool_outcomes: Vec<brassclaw_agent_loop::plan_scoring::ToolOutcome> = exec_state
                .recent_call_signatures
                .iter()
                .map(|sig| brassclaw_agent_loop::plan_scoring::ToolOutcome {
                    tool_id: sig.name.as_str().to_string(),
                    success: true,
                })
                .collect();
            library.process_session(&exec_state, &tool_outcomes).await;
            return;
        }
        tracing::debug!(%run_id, "plan library: no Final checkpoint found for run");
    }
}

/// Build and start a Reborn agent runtime.
///
/// On return, the turn-runner worker is already running in the background and
/// the returned `RebornRuntime` is ready to accept `send_user_message` calls.
///
/// **Supported paths:**
/// - `RebornStorageInput::LocalDev` without a PG pool — pure local-dev filesystem substrate.
/// - `RebornStorageInput::LocalDev` with a PG pool — local-dev filesystem substrate + PG pool;
///   all runtime stores (turn state, thread service, approval, etc.) are PG-backed.
/// - `RebornServices { local_runtime: None, pg_pool: Some(_) }` (pure-PG path) — all stores
///   built from the PG pool via `build_pg_runtime_stores`.
pub async fn build_reborn_runtime(
    input: RebornRuntimeInput,
) -> Result<RebornRuntime, RebornRuntimeError> {
    #[cfg(feature = "skills-db")]
    let mcp_provider_binding = Arc::new(crate::mcp_provider_gateway::McpProviderBinding::default());

    let RebornRuntimeInput {
        #[cfg(feature = "skills-db")]
        inbound_mcp_required,
        services: services_input,
        #[cfg(feature = "root-llm-provider")]
        llm,
        runner,
        trigger_poller,
        trigger_fire_access_checker,
        poll,
        identity,
        default_project_id,
        regex_skill_activation_enabled: _regex_skill_activation_enabled,
        conversation_context_tokens,
        skill_context_tokens: _skill_context_tokens,
        identity_token_ceiling,
        capability_surface_tokens,
        capability_focus_enabled,
        planning_mode_enabled,
        content_cache_threshold,
        plan_library_enabled,
        skill_promotion_threshold,
        hooks: hooks_config,
        budget_defaults,
        budget_event_observer,
        #[cfg(any(test, feature = "test-support"))]
        model_gateway_override,
        #[cfg(any(test, feature = "test-support"))]
        model_cost_table_override,
    } = input;

    let services_input = services_input.ok_or(RebornRuntimeError::InvalidArgument {
        reason: "RebornRuntimeInput.services is required".to_string(),
    })?;

    if services_input.runtime_policy().is_none() {
        return Err(RebornRuntimeError::InvalidArgument {
            reason: "RebornRuntimeInput.services must include a resolved runtime policy"
                .to_string(),
        });
    }

    let trusted_laptop_access = services_input.grants_trusted_laptop_access();
    let owner_id = services_input.owner_id().to_string();
    // Extract `reborn_home` before consuming services_input — needed by the pure-PG
    // path to derive the system-prompt storage root via build_pg_runtime_stores.
    #[cfg(feature = "postgres")]
    let pg_reborn_home: Option<std::path::PathBuf> =
        services_input.pg_reborn_home().map(|p| p.to_path_buf());
    #[cfg(feature = "postgres")]
    if pg_reborn_home.is_some() {
        let supported = tokio::runtime::Handle::try_current().is_ok_and(|handle| {
            handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
        });
        if !supported {
            return Err(RebornRuntimeError::InvalidArgument {
                reason: "PostgreSQL runtime accounting requires a multithread Tokio runtime".into(),
            });
        }
    }
    // Extract the runtime tenant_id before consuming `identity` — the PG store
    // constructors need it before `validate_runtime_identity` is called later.
    #[cfg(feature = "postgres")]
    let pg_tenant_id: String = identity.tenant_id.clone();
    let mut services = build_reborn_services(services_input).await?;

    // Resolve the runtime substrate: either the local-dev filesystem slab or,
    // on the pure-postgres path, PG-backed equivalents for every store that
    // `build_reborn_runtime` needs.  Both paths expose the same set of named
    // locals so the rest of the function is path-agnostic.
    //
    // Hybrid path (brassclaw serve): LocalDev profile + RebornStorageInput::Postgres
    //   → local_runtime is Some, pg_pool is Some.  Uses local_runtime stores for
    //   turn-state and checkpoints; upgrades thread service to PG.
    //
    // Pure-PG path (build_postgres_production → hosted/production profile):
    //   → local_runtime is None, pg_pool is Some.  Builds PgRuntimeStores here.
    #[cfg(feature = "postgres")]
    let pg_stores: Option<crate::factory::PgRuntimeStores>;
    #[cfg(feature = "postgres")]
    #[allow(clippy::type_complexity)]
    let (
        turn_state_store,
        checkpoint_state_store,
        loop_checkpoint_store,
        thread_service,
        budget_event_sink_for_accountant,
        resource_governor_for_accountant,
        budget_gate_store_for_accountant,
        event_log,
        audit_log,
        storage_root,
        system_prompt_path,
    ): (
        Arc<brassclaw_turns::TurnStateDriverBox>,
        Arc<dyn brassclaw_turns::CheckpointStateStore>,
        Arc<dyn brassclaw_turns::LoopCheckpointStore>,
        Arc<dyn brassclaw_threads::SessionThreadService>,
        Arc<dyn brassclaw_resources::BudgetEventSink>,
        Arc<dyn brassclaw_resources::ResourceGovernor>,
        Arc<dyn brassclaw_resources::BudgetGateStore>,
        Arc<dyn brassclaw_events::DurableEventLog>,
        Arc<dyn brassclaw_events::DurableAuditLog>,
        std::path::PathBuf,
        std::path::PathBuf,
    ) = if let Some(lr) = services.local_runtime.as_ref() {
        // Hybrid path: when a PG pool is present, upgrade all runtime stores to
        // PG-backed implementations so state survives process restart.  Without a
        // pool (pure local-dev / test), fall back to the in-memory stores.
        if let Some(pool) = services.pg_pool.as_ref() {
            let reborn_home = pg_reborn_home
                .as_deref()
                .filter(|p| !p.as_os_str().is_empty())
                .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                    reason: "hybrid runtime path requires a reborn_home to derive \
                             the system-prompt storage root"
                        .to_string(),
                })?;
            let stores = crate::factory::build_pg_runtime_stores(
                Arc::clone(pool),
                reborn_home,
                &pg_tenant_id,
            )
            .await
            .map_err(RebornRuntimeError::Build)?;
            let thread_svc: Arc<dyn brassclaw_threads::SessionThreadService> =
                Arc::new(brassclaw_threads::PgSessionThreadService::new(
                    Arc::clone(pool),
                    pg_tenant_id.as_str(),
                ));
            let event_sink = Arc::clone(&stores.budget_event_sink);
            let result = (
                Arc::new(brassclaw_turns::TurnStateDriverBox::new(
                    Arc::clone(&stores.turn_state) as Arc<dyn brassclaw_turns::TurnStateDriver>,
                )),
                Arc::clone(&stores.checkpoint_state_store),
                Arc::clone(&stores.loop_checkpoint_store),
                thread_svc,
                event_sink,
                Arc::clone(&stores.resource_governor),
                Arc::clone(&stores.budget_gate_store),
                Arc::clone(&stores.event_log),
                Arc::clone(&stores.audit_log),
                stores.local_dev_storage_root.clone(),
                stores.default_system_prompt_path.clone(),
            );
            pg_stores = Some(stores);
            result
        } else {
            pg_stores = None;
            let thread_svc: Arc<dyn brassclaw_threads::SessionThreadService> =
                Arc::clone(&lr.thread_service);
            let event_sink = Arc::clone(&lr.budget_event_sink);
            (
                Arc::new(brassclaw_turns::TurnStateDriverBox::new(
                    Arc::clone(&lr.turn_state) as Arc<dyn brassclaw_turns::TurnStateDriver>,
                )),
                Arc::clone(&lr.checkpoint_state_store),
                Arc::clone(&lr.loop_checkpoint_store),
                thread_svc,
                event_sink,
                Arc::clone(&lr.resource_governor),
                Arc::clone(&lr.budget_gate_store),
                Arc::clone(&lr.event_log),
                Arc::clone(&lr.audit_log),
                lr.local_dev_storage_root.clone(),
                lr.default_system_prompt_path.clone(),
            )
        }
    } else {
        // Pure-PG path: build PG-backed equivalents.
        let pool =
            services
                .pg_pool
                .as_ref()
                .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                    reason: "neither a local-dev substrate nor a Postgres pool is available; \
                         cannot assemble runtime"
                        .to_string(),
                })?;
        let reborn_home = pg_reborn_home
            .as_deref()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                reason: "pure-postgres runtime path requires a reborn_home to derive \
                         the system-prompt storage root"
                    .to_string(),
            })?;
        let stores =
            crate::factory::build_pg_runtime_stores(Arc::clone(pool), reborn_home, &pg_tenant_id)
                .await
                .map_err(RebornRuntimeError::Build)?;
        let event_sink = Arc::clone(&stores.budget_event_sink);
        let thread_svc: Arc<dyn brassclaw_threads::SessionThreadService> = Arc::new(
            brassclaw_threads::PgSessionThreadService::new(Arc::clone(pool), pg_tenant_id.as_str()),
        );
        let result = (
            Arc::new(brassclaw_turns::TurnStateDriverBox::new(
                Arc::clone(&stores.turn_state) as Arc<dyn brassclaw_turns::TurnStateDriver>,
            )),
            Arc::clone(&stores.checkpoint_state_store),
            Arc::clone(&stores.loop_checkpoint_store),
            thread_svc,
            event_sink,
            Arc::clone(&stores.resource_governor),
            Arc::clone(&stores.budget_gate_store),
            Arc::clone(&stores.event_log),
            Arc::clone(&stores.audit_log),
            stores.local_dev_storage_root.clone(),
            stores.default_system_prompt_path.clone(),
        );
        pg_stores = Some(stores);
        result
    };
    #[cfg(not(feature = "postgres"))]
    let _pg_stores: Option<()> = None;
    #[cfg(not(feature = "postgres"))]
    let local_runtime =
        services
            .local_runtime
            .as_ref()
            .ok_or(RebornRuntimeError::InvalidArgument {
                reason: "local-dev RebornServices did not provide runtime substrate".to_string(),
            })?;
    #[cfg(not(feature = "postgres"))]
    let turn_state_store: Arc<brassclaw_turns::TurnStateDriverBox> =
        Arc::new(brassclaw_turns::TurnStateDriverBox::new(
            Arc::clone(&local_runtime.turn_state) as Arc<dyn brassclaw_turns::TurnStateDriver>,
        ));
    #[cfg(not(feature = "postgres"))]
    let checkpoint_state_store = Arc::clone(&local_runtime.checkpoint_state_store);
    #[cfg(not(feature = "postgres"))]
    let loop_checkpoint_store = Arc::clone(&local_runtime.loop_checkpoint_store);
    #[cfg(not(feature = "postgres"))]
    let thread_service = Arc::clone(&local_runtime.thread_service);
    #[cfg(not(feature = "postgres"))]
    let budget_event_sink_for_accountant = Arc::clone(&local_runtime.budget_event_sink);
    #[cfg(not(feature = "postgres"))]
    let resource_governor_for_accountant = Arc::clone(&local_runtime.resource_governor);
    #[cfg(not(feature = "postgres"))]
    let budget_gate_store_for_accountant = Arc::clone(&local_runtime.budget_gate_store);
    #[cfg(not(feature = "postgres"))]
    let event_log = Arc::clone(&local_runtime.event_log);
    #[cfg(not(feature = "postgres"))]
    let audit_log = Arc::clone(&local_runtime.audit_log);
    #[cfg(not(feature = "postgres"))]
    let storage_root = local_runtime.local_dev_storage_root.clone();
    #[cfg(not(feature = "postgres"))]
    let system_prompt_path = local_runtime.default_system_prompt_path.clone();

    // Keep the typed local store only when durable stores were not selected.
    // Auth and approval interactions use the scoped reader selected below;
    // trigger snapshots currently still require the typed local store.
    let local_dev_turn_state: Option<Arc<crate::factory::LocalDevTurnStateStore>> = {
        #[cfg(feature = "postgres")]
        {
            if pg_stores.is_some() {
                None
            } else {
                services
                    .local_runtime
                    .as_ref()
                    .map(|lr| Arc::clone(&lr.turn_state))
            }
        }
        #[cfg(not(feature = "postgres"))]
        services
            .local_runtime
            .as_ref()
            .map(|lr| Arc::clone(&lr.turn_state))
    };

    // Concrete BroadcastBudgetEventSink — needed by BudgetEventProjection::spawn
    // which takes `&BroadcastBudgetEventSink`.  Extracted separately from the
    // tuple because the tuple uses Arc<dyn BudgetEventSink> for the accountant.
    let broadcast_budget_sink: Arc<brassclaw_resources::BroadcastBudgetEventSink> = {
        #[cfg(feature = "postgres")]
        {
            // Prefer PG stores (set on hybrid-with-pool and pure-PG paths) over the
            // in-memory sink so event subscriptions use the same sink that the accountant
            // writes to.
            if let Some(pg) = pg_stores.as_ref() {
                Arc::clone(&pg.broadcast_budget_event_sink)
            } else if let Some(lr) = services.local_runtime.as_ref() {
                Arc::clone(&lr.broadcast_budget_event_sink)
            } else {
                Arc::new(brassclaw_resources::BroadcastBudgetEventSink::default())
            }
        }
        #[cfg(not(feature = "postgres"))]
        Arc::clone(&local_runtime.broadcast_budget_event_sink)
    };

    #[cfg(any(test, feature = "test-support"))]
    let in_memory_budget_sink = {
        #[cfg(feature = "postgres")]
        {
            if let Some(pg) = pg_stores.as_ref() {
                Arc::clone(&pg.in_memory_budget_event_sink)
            } else {
                Arc::clone(
                    &services
                        .local_runtime
                        .as_ref()
                        .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                            reason: "budget event substrate missing".into(),
                        })?
                        .in_memory_budget_event_sink,
                )
            }
        }
        #[cfg(not(feature = "postgres"))]
        Arc::clone(&local_runtime.in_memory_budget_event_sink)
    };

    // Extract approval_requests and capability_leases from the substrate.
    // These are `Arc<dyn ...>` trait objects so they work for both paths.
    // Priority: PG stores (set on both the hybrid-with-pool and pure-PG paths) win over
    // local-dev in-memory stores so that approvals survive process restart on the serve path.
    let substrate_approval_requests: Arc<dyn brassclaw_run_state::ApprovalRequestStore> = {
        #[cfg(feature = "postgres")]
        {
            if let Some(pg) = pg_stores.as_ref() {
                Arc::clone(&pg.approval_requests)
                    as Arc<dyn brassclaw_run_state::ApprovalRequestStore>
            } else if let Some(lr) = services.local_runtime.as_ref() {
                Arc::clone(&lr.approval_requests)
                    as Arc<dyn brassclaw_run_state::ApprovalRequestStore>
            } else {
                return Err(RebornRuntimeError::InvalidArgument {
                    reason: "approval store not available (no substrate)".to_string(),
                });
            }
        }
        #[cfg(not(feature = "postgres"))]
        {
            Arc::clone(&local_runtime.approval_requests)
                as Arc<dyn brassclaw_run_state::ApprovalRequestStore>
        }
    };
    let substrate_capability_leases: Arc<dyn brassclaw_authorization::CapabilityLeaseStore> = {
        #[cfg(feature = "postgres")]
        {
            if let Some(pg) = pg_stores.as_ref() {
                Arc::clone(&pg.capability_leases)
                    as Arc<dyn brassclaw_authorization::CapabilityLeaseStore>
            } else if let Some(lr) = services.local_runtime.as_ref() {
                Arc::clone(&lr.capability_leases)
                    as Arc<dyn brassclaw_authorization::CapabilityLeaseStore>
            } else {
                return Err(RebornRuntimeError::InvalidArgument {
                    reason: "lease store not available (no substrate)".to_string(),
                });
            }
        }
        #[cfg(not(feature = "postgres"))]
        {
            Arc::clone(&local_runtime.capability_leases)
                as Arc<dyn brassclaw_authorization::CapabilityLeaseStore>
        }
    };
    // Extract workspace/skill/memory mounts — empty on the pure-PG path.
    let substrate_workspace_mounts: brassclaw_host_api::MountView = services
        .local_runtime
        .as_ref()
        .map(|lr| lr.workspace_mounts.clone())
        .unwrap_or_default();
    let substrate_skill_mounts: brassclaw_host_api::MountView = services
        .local_runtime
        .as_ref()
        .map(|lr| lr.skill_mounts.clone())
        .unwrap_or_default();
    let substrate_memory_mounts: brassclaw_host_api::MountView = services
        .local_runtime
        .as_ref()
        .map(|lr| lr.memory_mounts.clone())
        .unwrap_or_default();
    // Extract trigger repository from the substrate.
    #[cfg(feature = "postgres")]
    let substrate_trigger_repository: Arc<dyn brassclaw_triggers::TriggerRepository> = {
        if let Some(lr) = services.local_runtime.as_ref() {
            Arc::clone(&lr.trigger_repository)
        } else if let Some(pg) = pg_stores.as_ref() {
            Arc::clone(&pg.trigger_repository) as Arc<dyn brassclaw_triggers::TriggerRepository>
        } else {
            // Trigger poller disabled if no repository; it should not be enabled.
            Arc::new(brassclaw_triggers::InMemoryTriggerRepository::default())
        }
    };
    #[cfg(not(feature = "postgres"))]
    let substrate_trigger_repository: Arc<dyn brassclaw_triggers::TriggerRepository> =
        Arc::clone(&local_runtime.trigger_repository);
    // Extract broadcast_budget_event_sink for the budget projection task.
    // Uses the substrate variable already extracted above (broadcast_budget_event_sink_for_accountant).

    // Read the legacy startup VM settings. Missing rows use the store's explicit
    // defaults; read failures must not silently disable limits. The duration is
    // an executing-VM/task budget, never a Rust wall-clock limit over provider
    // waits. Shared live revision uptake remains part of the global cutover.
    #[cfg(not(all(feature = "postgres", feature = "skills-db")))]
    let startup_monty_settings: Option<brassclaw_product_workflow::MontyVmSettings> = {
        #[cfg(feature = "postgres")]
        {
            if let Some(pool) = services.pg_pool.as_ref() {
                Some(read_startup_monty_settings(pool).await?)
            } else {
                None
            }
        }
        #[cfg(not(feature = "postgres"))]
        {
            None
        }
    };
    #[cfg(not(all(feature = "postgres", feature = "skills-db")))]
    let resolved_token_budgets_enabled = startup_monty_settings
        .as_ref()
        .is_some_and(|settings| settings.token_budgets_enabled);

    let resolved_max_output_tokens: Option<u32> = None;
    let resolved_inline_control_tokens: Option<usize> = None;
    let resolved_total_input_tokens: Option<usize> = None;

    // DB-first: read context_window_tokens from the provider row seeded into
    // brassclaw_llm_providers.  Falls back to the compiled-in registry when the
    // DB pool is unavailable (e.g. postgres feature disabled — db-less paths are
    // not supported in production but must compile).
    #[cfg(all(feature = "root-llm-provider", feature = "postgres"))]
    let resolved_context_window_tokens: Option<u32> = {
        let db_val = if let (Some(pool), Some(l)) = (services.pg_pool.as_ref(), llm.as_ref()) {
            let repo = crate::pg_provider_repo::PgProviderRepo::new(
                (**pool).clone(),
                identity.tenant_id.clone(),
            );
            repo.get(l.provider_id())
                .await
                .ok()
                .flatten()
                .and_then(|def| def.context_window_tokens)
        } else {
            None
        };
        db_val.or_else(|| {
            llm.as_ref().and_then(|l| {
                brassclaw_llm::ProviderRegistry::try_load_from_path(None)
                    .ok()
                    .and_then(|reg| {
                        reg.find(l.provider_id())
                            .and_then(|d| d.context_window_tokens)
                    })
            })
        })
    };
    // Non-postgres builds have no provider DB; context window is unavailable.
    #[cfg(all(feature = "root-llm-provider", not(feature = "postgres")))]
    let resolved_context_window_tokens: Option<u32> = None;
    #[cfg(not(feature = "root-llm-provider"))]
    let resolved_context_window_tokens: Option<u32> = None;

    #[cfg(all(feature = "root-llm-provider", feature = "postgres"))]
    let resolved_cache_retention_final: Option<String> = {
        let db_val = if let (Some(pool), Some(l)) = (services.pg_pool.as_ref(), llm.as_ref()) {
            let repo = crate::pg_provider_repo::PgProviderRepo::new(
                (**pool).clone(),
                identity.tenant_id.clone(),
            );
            repo.get(l.provider_id())
                .await
                .ok()
                .flatten()
                .and_then(|def| def.cache_retention)
        } else {
            None
        };
        db_val
            .or_else(|| {
                std::env::var("LLM_CACHE_RETENTION").ok().and_then(|raw| {
                    match raw.parse::<brassclaw_llm::CacheRetention>() {
                        Ok(cr) => Some(cr.to_string()),
                        Err(error) => {
                            tracing::debug!(
                                cache_retention = %raw,
                                error = %error,
                                "ignoring unparseable LLM_CACHE_RETENTION; using DB value"
                            );
                            None
                        }
                    }
                })
            })
            .or_else(|| {
                llm.as_ref().and_then(|l| {
                    brassclaw_llm::ProviderRegistry::try_load_from_path(None)
                        .ok()
                        .and_then(|reg| {
                            reg.find(l.provider_id())
                                .and_then(|d| d.cache_retention.clone())
                        })
                })
            })
    };
    // Non-postgres builds have no provider DB; cache retention is unavailable.
    #[cfg(all(feature = "root-llm-provider", not(feature = "postgres")))]
    #[allow(unused_variables)]
    let resolved_cache_retention_final: Option<String> = None;
    #[cfg(not(feature = "root-llm-provider"))]
    #[allow(unused_variables)]
    let resolved_cache_retention_final: Option<String> = None;

    #[allow(unused_variables)]
    let live_context_budget: Option<brassclaw_agent_loop::LiveTokenBudget> =
        conversation_context_tokens.map(|n| brassclaw_agent_loop::LiveTokenBudget::new(Some(n)));
    #[allow(unused_variables)]
    let live_max_output: Option<brassclaw_agent_loop::LiveTokenBudget> = resolved_max_output_tokens
        .map(|n| brassclaw_agent_loop::LiveTokenBudget::new(Some(n as usize)));
    #[allow(unused_variables)]
    let live_total_input: Option<brassclaw_agent_loop::LiveTokenBudget> =
        resolved_total_input_tokens.map(|n| brassclaw_agent_loop::LiveTokenBudget::new(Some(n)));
    #[allow(unused_variables)]
    let live_inline_control: Option<brassclaw_agent_loop::LiveTokenBudget> =
        resolved_inline_control_tokens.map(|n| brassclaw_agent_loop::LiveTokenBudget::new(Some(n)));
    #[allow(unused_variables)]
    let live_context_window: Option<brassclaw_agent_loop::LiveTokenBudget> =
        resolved_context_window_tokens
            .map(|n| brassclaw_agent_loop::LiveTokenBudget::new(Some(n as usize)));

    let validated_identity = validate_runtime_identity(identity)?;
    // Build the message-text recorder used by intent matching (InputStage).
    // The VFS-based SKILL.md loading path was removed in Phase P.1 Step C;
    // this is now a plain in-memory message recorder.
    #[cfg(not(feature = "postgres"))]
    let has_local_runtime = true;
    #[cfg(feature = "postgres")]
    let has_local_runtime = services.local_runtime.is_some();
    let skill_activation_source: Option<Arc<SelectableSkillContextSource>> = if has_local_runtime {
        Some(Arc::new(SelectableSkillContextSource::new()))
    } else {
        None
    };

    let tenant_id = validated_identity.tenant_id.clone();
    let agent_id = validated_identity.agent_id.clone();
    let actor_user_id =
        UserId::new(owner_id.clone()).map_err(|reason| RebornRuntimeError::InvalidArgument {
            reason: format!("user id: {reason}"),
        })?;

    // Step 6.5: Migrate legacy brassclaw_memory_docs rows into the class-specific
    // component tables (V036–V043) at boot.  This is idempotent — rows with an
    // unchanged content_hash are skipped.  Runs only when both `postgres` and
    // `skills-db` features are active and a PG pool is available.
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    if let Some(pool) = services.pg_pool.as_ref() {
        match crate::component_import::run_component_import(
            pool,
            validated_identity.agent_id.as_str(),
            validated_identity.tenant_id.as_str(),
        )
        .await
        {
            Ok(summary) => {
                if summary.inserted > 0 || summary.updated > 0 {
                    tracing::debug!(
                        inserted = summary.inserted,
                        updated = summary.updated,
                        skipped = summary.skipped,
                        failed = summary.failed.len(),
                        "component_import: legacy MemoryDoc rows migrated to component tables"
                    );
                }
                for (key, reason) in &summary.failed {
                    tracing::debug!(key, reason, "component_import: row migration failed");
                }
            }
            Err(e) => {
                // Non-fatal: log and continue.  Import will be retried on next boot.
                tracing::debug!(
                    error = %e,
                    "component_import: boot-time migration encountered an error (non-fatal)"
                );
            }
        }
    }

    // Shared boot prerequisites belong to the runtime, including REPL and
    // non-WebUI entries. Failure returns before any turn worker or producer is
    // spawned. The factory has completed migrations on this pool.
    #[cfg(feature = "postgres")]
    if let Some(pool) = services.pg_pool.as_ref() {
        let booted_db = crate::booted_db::BootedDb::from_migrated_pool(Arc::clone(pool));
        // Component seeding carries large typed documents across awaits. Keep
        // that state on the heap rather than enlarging every runtime startup
        // future and overflowing a normal executor/test thread's stack.
        Box::pin(crate::component_boot::initialize_runtime_components(
            &booted_db,
            validated_identity.tenant_id.as_str(),
        ))
        .await?;
    }

    let thread_scope = ThreadScope {
        tenant_id,
        agent_id,
        project_id: default_project_id,
        // Keep local-dev runtime threads aligned with WebUI's owner-scoped
        // facade so both entrypoints drive the same runner/evidence path.
        owner_user_id: Some(actor_user_id.clone()),
    };

    // Resolve the model gateway in three flat steps so the cfg gates
    // don't multiply into a 4-way permutation:
    //
    // 1. Normalize the test-only override into a plain `Option`.
    //    Off-feature builds get a hard `None` so downstream control flow
    //    stays plain.
    // 2. Build the production gateway + cost table from the LLM config
    //    (cfg-gated helper); without `root-llm-provider` the helper
    //    short-circuits to a stub.
    // 3. The test override wins over the production gateway when set;
    //    the LLM-derived cost table is kept regardless so the
    //    accountant can fire against a stub gateway too.
    // 3. A test gateway override short-circuits the production build entirely:
    //    building a real gateway only to discard it wastes startup work (and, on
    //    the cold-boot path, an LLM session manager), which made
    //    timeout-sensitive tests flaky. When no override is set, build normally.
    #[cfg(all(feature = "root-llm-provider", any(test, feature = "test-support")))]
    let (model_gateway, llm_cost_table, llm_reload) = match model_gateway_override {
        Some(override_gateway) => (override_gateway, None, None),
        None => {
            build_production_model_gateway(
                llm,
                live_max_output.clone(),
                live_total_input.clone(),
                resolved_cache_retention_final.clone(),
            )
            .await?
        }
    };
    #[cfg(all(
        feature = "root-llm-provider",
        not(any(test, feature = "test-support"))
    ))]
    let (model_gateway, llm_cost_table, llm_reload) = build_production_model_gateway(
        llm,
        live_max_output.clone(),
        live_total_input.clone(),
        resolved_cache_retention_final.clone(),
    )
    .await?;
    #[cfg(all(
        not(feature = "root-llm-provider"),
        any(test, feature = "test-support")
    ))]
    let (model_gateway, llm_cost_table) = match model_gateway_override {
        Some(override_gateway) => (override_gateway, None),
        None => build_production_model_gateway()?,
    };
    #[cfg(all(
        not(feature = "root-llm-provider"),
        not(any(test, feature = "test-support"))
    ))]
    let (model_gateway, llm_cost_table) = build_production_model_gateway()?;

    // Resolved cost table is either: the LLM-policy-derived table (real
    // LLM wired), a test override (so tests can drive deterministic
    // prices through stub gateways), or None — in which case the
    // accountant uses its conservative unknown-pricing fallback. The test
    // override (when set) wins over the LLM-derived table — the test is
    // being explicit about the prices it wants.
    let llm_cost_table_arc: Option<Arc<dyn brassclaw_loop_support::ModelCostTable>> =
        llm_cost_table
            .map(|table| Arc::new(table) as Arc<dyn brassclaw_loop_support::ModelCostTable>);
    #[cfg(any(test, feature = "test-support"))]
    let resolved_cost_table = model_cost_table_override.or(llm_cost_table_arc);
    #[cfg(not(any(test, feature = "test-support")))]
    let resolved_cost_table = llm_cost_table_arc;

    // Build the model budget accountant from the resolved cost table plus
    // the local-dev governor. `local-dev-yolo` is the explicit local
    // exception: it inherits host trust and must not pause on budget gates.
    // Token mode never removes the accountant: reservations and actual usage
    // survive live edits. The governor masks only token enforcement using the
    // acknowledged settings source; independent cost/resource limits remain.
    // Unknown pricing retains the accountant's conservative cost fallback.
    //
    // The accountant is wired with a seeding policy derived from the
    // caller-supplied `BudgetDefaults` (or `compiled_defaults().with_env()`
    // as the composition-root fallback when no caller pre-resolves them)
    // so a fresh user / project account picks up the default daily cap on
    // the first model call. Without this seeding step the local-dev
    // governor starts empty and `reserve_with_outcome_in_state` skips
    // accounts that have no configured limit — model calls would record
    // usage but never enforce a cap (review feedback High #2 + Thermo-
    // Nuclear #1: defaults resolve once at the composition root with
    // explicit precedence and a `validate()` call instead of being
    // re-read by the wiring helper).
    let model_budget_accountant: Option<
        Arc<dyn brassclaw_turns::run_profile::LoopModelBudgetAccountant>,
    > = match trusted_laptop_access {
        // Skip budget enforcement for trusted-laptop-access (yolo) profiles —
        // the local user has full host access and budget limits are counterproductive.
        true => None,
        false => {
            let cost_table = resolved_cost_table
                .unwrap_or_else(|| Arc::new(brassclaw_loop_support::StaticModelCostTable::new()));
            let resolved_budget_defaults = match budget_defaults {
                Some(defaults) => {
                    defaults
                        .validate()
                        .map_err(|error| RebornRuntimeError::InvalidArgument {
                            reason: format!("supplied budget defaults invalid: {error}"),
                        })?;
                    defaults
                }
                None => {
                    let defaults = brassclaw_reborn_config::BudgetDefaults::compiled_defaults()
                        .with_env()
                        .map_err(|error| RebornRuntimeError::InvalidArgument {
                            reason: format!("budget defaults env-override invalid: {error}"),
                        })?;
                    defaults
                        .validate()
                        .map_err(|error| RebornRuntimeError::InvalidArgument {
                            reason: format!("resolved budget defaults invalid: {error}"),
                        })?;
                    defaults
                }
            };
            // Shared helper — same wiring shape used by any production
            // loop composer that wants the accountant.
            // The accountant uses the same broadcast-backed sink that
            // the governor writes to, so `BudgetEvent::GateOpened`
            // (emitted by the accountant) lands on the same downstream
            // projection as the governor's `Warned` / `Denied` events.
            let accountant = crate::build_default_budget_accountant(
                Arc::clone(&resource_governor_for_accountant),
                cost_table,
                Arc::clone(&budget_gate_store_for_accountant),
                Arc::clone(&budget_event_sink_for_accountant),
                &resolved_budget_defaults,
            );
            Some(accountant)
        }
    };

    let loop_exit_evidence = Arc::new(ThreadCheckpointLoopExitEvidencePort::new_with_thread_scope(
        Arc::clone(&thread_service),
        Arc::clone(&turn_state_store) as Arc<dyn brassclaw_turns::TurnStateStore>,
        Arc::clone(&loop_checkpoint_store),
        thread_scope.clone(),
    ));
    // `event_log` and `audit_log` are substrate variables extracted above.
    let milestone_thread_scope = ThreadScope {
        owner_user_id: Some(actor_user_id.clone()),
        ..thread_scope.clone()
    };
    let milestone_scope = DurableLoopHostMilestoneScope::from_thread_scope(&milestone_thread_scope)
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: error.to_string(),
        })?;
    let durable_milestone_sink: Arc<dyn LoopHostMilestoneSink> = Arc::new(
        DurableLoopHostMilestoneSink::new(Arc::clone(&event_log), milestone_scope),
    );
    // Postgres is mandatory — subagent goals must survive process restarts.
    // Fail hard if the pool is not available rather than silently losing goals.
    #[cfg(feature = "postgres")]
    let subagent_goal_store: Arc<dyn brassclaw_reborn::runtime::RuntimeSubagentGoalStore> = {
        let pool =
            services
                .pg_pool
                .as_ref()
                .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                    reason: "Postgres pool is required for PgSubagentGoalStore \
                         (postgres is mandatory; in-memory fallback removed)"
                        .to_string(),
                })?;
        Arc::new(brassclaw_reborn::subagent::goal_store::PgSubagentGoalStore::new((**pool).clone()))
    };
    #[cfg(not(feature = "postgres"))]
    let subagent_goal_store: Arc<dyn brassclaw_reborn::runtime::RuntimeSubagentGoalStore> =
        Arc::new(InMemoryBoundedSubagentGoalStore::new());
    if trusted_laptop_access {
        append_trusted_laptop_access_audit(&audit_log, &thread_scope, &actor_user_id).await?;
    }
    // Postgres is mandatory — outbound notification targets must survive restarts.
    // Fail hard if the pool is not available rather than silently using in-memory state.
    #[cfg(feature = "postgres")]
    let outbound_store: Arc<dyn brassclaw_outbound::OutboundStateStore> = {
        let pool =
            services
                .pg_pool
                .as_ref()
                .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                    reason: "Postgres pool is required for PgOutboundStateStore \
                         (postgres is mandatory; in-memory fallback removed)"
                        .to_string(),
                })?;
        Arc::new(brassclaw_outbound::PgOutboundStateStore::new(
            (**pool).clone(),
        ))
    };
    #[cfg(not(feature = "postgres"))]
    let outbound_store: Arc<dyn brassclaw_outbound::OutboundStateStore> =
        Arc::new(brassclaw_outbound::InMemoryOutboundStateStore::default());
    let projection_services = build_reborn_projection_services(
        Arc::clone(&event_log),
        validated_identity.reply_target_binding_ref.clone(),
        outbound_store,
    );
    let live_projection_publisher =
        projection_services.live_projection_publisher(actor_user_id.clone());
    // v3 Phase P.1 Step C: snapshot a clone of the publisher before it is
    // consumed by the milestone sink, so the skills-db block below can build the
    // skill-activation observer and wire it into PgRetrievalLookup.
    #[cfg(feature = "skills-db")]
    let skill_activation_observer_arc =
        projection_services.skill_activation_observer(Arc::clone(&live_projection_publisher));
    let milestone_sink = projection_services.with_live_progress_milestone_sink_for_publisher(
        durable_milestone_sink,
        live_projection_publisher,
    );
    // Capability wiring: on the local-dev / hybrid path use the full
    // `local_dev::capability_wiring` which wires workspace/skill/memory
    // mounts, extension surface discovery, display previews, and skill
    // activation.  On the pure-PG path (`services.local_runtime.is_none()`),
    // use `local_dev::pg_capability_wiring` which wires the same IO surface
    // against `HostRuntimeLoopCapabilityPortFactory` with empty mounts and
    // allow-all policy.
    let local_dev_capabilities = if services.local_runtime.is_some() {
        let local_dev_capability_policy =
            Arc::new(local_dev_capability_policy().map_err(|error| {
                tracing::error!(%error, "local-dev capability policy is invalid");
                RebornRuntimeError::InvalidArgument {
                    reason: format!("local-dev capability policy is invalid: {error}"),
                }
            })?);
        local_dev::capability_wiring(
            &services,
            Arc::clone(&thread_service) as Arc<dyn SessionThreadService>,
            thread_scope.clone(),
            actor_user_id.clone(),
            Arc::clone(&local_dev_capability_policy),
            model_gateway,
            milestone_sink.clone(),
        )
        .ok_or(RebornRuntimeError::HostRuntimeUnavailable)?
    } else {
        local_dev::pg_capability_wiring(
            &services,
            Arc::clone(&thread_service) as Arc<dyn SessionThreadService>,
            thread_scope.clone(),
            actor_user_id.clone(),
            model_gateway,
            milestone_sink.clone(),
        )
        .ok_or(RebornRuntimeError::HostRuntimeUnavailable)?
    };
    #[cfg(all(test, feature = "postgres", feature = "skills-db"))]
    let monty_test_controls;
    // Global Monty is ready before any turn worker or trigger producer starts.
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    let (global_monty_owner, installed_catalogue, monty_driver, monty_settings_owner) = {
        let pool = services
            .pg_pool
            .clone()
            .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                reason: "global Monty requires PostgreSQL".into(),
            })?;
        let scope = brassclaw_engine::memory::intent_system::IntentScope {
            tenant_id: validated_identity.tenant_id.to_string(),
            user_id: actor_user_id.to_string(),
            agent_id: validated_identity.agent_id.to_string(),
            project_id: thread_scope
                .project_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "default".into()),
        };
        let (owner, catalogue, initial_settings, memory_measurement_status) =
            crate::global_monty_startup::start(
                pool.clone(),
                services.monty_kernel.clone().ok_or_else(|| {
                    RebornRuntimeError::InvalidArgument {
                        reason: "global Monty requires its captured kernel".into(),
                    }
                })?,
                &scope,
                substrate_memory_mounts.clone(),
            )
            .await?;
        let settings_owner = crate::live_monty_settings::MontySettingsOwner::start(
            pool.clone(),
            owner.client(),
            owner.ownership_check(),
            initial_settings,
            memory_measurement_status,
        )
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: error.to_string(),
        })?;
        let ports = Arc::new(
            crate::global_task_factory::OwnedGlobalTaskFactory::new(
                pool,
                owner.ownership_check(),
                Arc::new(catalogue.clone()),
                owner.client(),
                settings_owner.store().recipe_capacity_source(),
                settings_owner.store().cancellation_policy_source(),
                Some(mcp_provider_binding.clone()),
            )
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: error.to_string(),
            })?,
        );
        let driver = Arc::new(
            crate::global_monty_driver::GlobalMontyDriver::new(
                owner.client(),
                thread_service.clone(),
                ports.clone(),
            )
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: error.to_string(),
            })?,
        );
        #[cfg(test)]
        {
            monty_test_controls = (driver.clone(), ports);
        }
        (
            owner,
            catalogue,
            Some(driver as Arc<dyn brassclaw_turns::run_profile::MontyTurnDriverPort>),
            settings_owner,
        )
    };
    #[cfg(not(all(feature = "postgres", feature = "skills-db")))]
    let monty_driver: Option<Arc<dyn brassclaw_turns::run_profile::MontyTurnDriverPort>> = None;

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    resource_governor_for_accountant
        .bind_task_budget_settings(global_monty_owner.client().live_task_settings())
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: error.to_string(),
        })?;

    #[cfg(feature = "skills-db")]
    let retrieval_lookup: Option<Arc<dyn brassclaw_turns::run_profile::RetrievalLookup>> =
        services.pg_pool.as_ref().map(|pool| {
            Arc::new(
                crate::retrieval_lookup_impl::PgRetrievalLookup::new(Arc::new(
                    brassclaw_engine::memory::PostgresSource::new(Arc::clone(pool)),
                ))
                .with_token_settings(monty_settings_owner.store().effective_view())
                .with_skill_activation_observer(Arc::clone(&skill_activation_observer_arc)),
            ) as Arc<dyn brassclaw_turns::run_profile::RetrievalLookup>
        });
    // v3 Phase H.12.5: clone the per-run Tier-0 EffectExecutor builder out of
    // the capability wiring before any of its sibling fields are moved, so the
    // OrchestratorLookup bridge (built below near `retrieval_lookup`) can hand
    // it to `PgOrchestratorLookup`. Skills-db-gated: the builder (and the whole
    // Tier-0 deterministic path) only exists under that feature.
    #[cfg(feature = "skills-db")]
    let tier_zero_executor_builder = local_dev_capabilities.tier_zero_executor_builder.clone();
    let capability_factory = {
        // Wrap the capability factory with the content-cache decorator when
        // content_cache_threshold is configured.
        let base_factory = local_dev_capabilities.capability_factory;
        if let Some(threshold_tokens) = content_cache_threshold {
            tracing::debug!(
                threshold_tokens,
                "content cache enabled: large tool results will be stubbed and cached"
            );
            // Use the local-dev content-cache slot when available; fall back to a
            // new default slot on the pure-PG path (no local-dev substrate).
            let slot = services
                .local_runtime
                .as_ref()
                .map(|lr| lr.content_cache_slot.clone())
                .unwrap_or_default();
            let decorator = brassclaw_reborn::content_cache_port::ContentCachingPortDecorator::new(
                slot,
                threshold_tokens,
            );
            Arc::new(
                brassclaw_loop_support::DecoratingLoopCapabilityPortFactory::new(base_factory)
                    .with_decorator(Arc::new(decorator)),
            ) as Arc<dyn brassclaw_loop_support::LoopCapabilityPortFactory>
        } else {
            base_factory
        }
    };
    let capability_input_resolver = local_dev_capabilities.capability_input_resolver;
    let capability_result_writer = local_dev_capabilities.capability_result_writer;
    let model_gateway = local_dev_capabilities.model_gateway;
    #[cfg(feature = "skills-db")]
    let capability_factory: Arc<dyn brassclaw_loop_support::LoopCapabilityPortFactory> =
        Arc::new(crate::mcp_provider_gateway::McpCommandPortFactory {
            inner: capability_factory,
            binding: mcp_provider_binding.clone(),
            writer: capability_result_writer.clone(),
        });
    #[cfg(feature = "skills-db")]
    let model_gateway: Arc<dyn brassclaw_loop_support::HostManagedModelGateway> =
        Arc::new(crate::mcp_provider_gateway::McpProviderModelGateway {
            inner: model_gateway,
            binding: mcp_provider_binding.clone(),
        });
    // Hook framework activation (#3934 + third-party projection), gated behind
    // the typed `HooksActivationConfig` carried in `RebornRuntimeInput` (master
    // flag default OFF; third-party sub-flag also default OFF). The env vars
    // (`HOOKS_ENABLED`, `HOOKS_THIRD_PARTY_ENABLED`) are resolved ONCE at the
    // edge that builds the input (the CLI / ingress adapter); this composition
    // root consumes the typed config and never reads the environment itself.
    //
    // Hook-only projection containment: third-party `[[hooks]]` are discovered
    // and projected into a `HookProjectionRegistry` that carries ONLY hook
    // metadata (no `ExtensionRegistry`, no `ExtensionPackage`) and reaches ONLY
    // this hook factory, not the capability catalog or surface resolver.
    let hook_dispatcher_builder_factory = {
        // On the pure-PG path there is no local extension filesystem for
        // third-party hook discovery — pass None to disable it.
        // The two branches must be separate (type-level: the generic F differs
        // between LocalDevRootFilesystem and the no-filesystem None case).
        let projection_registry = if let Some(lr) = services.local_runtime.as_ref() {
            let third_party_input = crate::hooks::ThirdPartyDiscoveryInput {
                filesystem: lr.extension_filesystem.as_ref(),
                tenant_id: &validated_identity.tenant_id,
            };
            crate::hooks::build_hook_projection_registry(
                builtin_extension_registry()?,
                Some(third_party_input),
                hooks_config,
            )
            .await
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: format!("hook projection registry assembly failed: {error}"),
            })?
        } else {
            // Pure-PG path: builtin-only registry, no third-party discovery.
            crate::hooks::build_hook_projection_registry::<brassclaw_filesystem::LocalFilesystem>(
                builtin_extension_registry()?,
                None,
                hooks_config,
            )
            .await
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: format!("hook projection registry assembly failed: {error}"),
            })?
        };
        // Pass the Postgres pool when available so the hooks predicate-state
        // backend uses PostgresPredicateStateBackend instead of the in-memory
        // fallback (L6 fix). In local-dev the pool is None until embedded PG
        // is wired (Phase 6); in production the pool flows from
        // build_postgres_production via services.pg_pool.
        #[cfg(feature = "postgres")]
        let hooks_pg_pool = services.pg_pool.as_ref().map(Arc::clone);
        #[cfg(not(feature = "postgres"))]
        let hooks_pg_pool: Option<std::sync::Arc<deadpool_postgres::Pool>> = None;
        crate::hooks::build_hook_dispatcher_builder_factory_for_tenant(
            hooks_config,
            &projection_registry,
            &validated_identity.tenant_id,
            hooks_pg_pool,
        )
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: format!("hook framework activation failed: {error}"),
        })?
    };

    // Step 5.3: Use PgRecipeLibrary (reads from reborn_recipes).
    // Postgres is mandatory — the MemoryDoc-backed fallback has been removed.
    // When the pool is None, recipe_lookup is None (RecipeStage falls through to
    // Tier 2, which is the correct explicit behaviour when PG is unavailable).
    #[cfg(feature = "postgres")]
    let recipe_lookup: Option<Arc<dyn brassclaw_turns::run_profile::RecipeLookup>> =
        services.pg_pool.as_ref().map(|pool| {
            Arc::new(crate::pg_recipe_store::PgRecipeLibrary::local_dev(
                Arc::clone(pool),
            )) as Arc<dyn brassclaw_turns::run_profile::RecipeLookup>
        });
    #[cfg(not(feature = "postgres"))]
    let recipe_lookup: Option<Arc<dyn brassclaw_turns::run_profile::RecipeLookup>> = None;

    // v3 Phase E.0 / plan §H4: wire PgRetrievalLookup (engine
    // `PostgresSource`-backed) when the `skills-db` feature is active and a
    // Postgres pool is available. Postgres is mandatory at runtime; the
    // `#[cfg(not(feature = "skills-db"))]` fallback below is for test builds
    // that compile without the feature. v3 Phase P.1 Step C wires the
    // skill-activation observer so intent matches emit live WebUI projection
    // events via the same publisher as the milestone sink.
    #[cfg(not(feature = "skills-db"))]
    let retrieval_lookup: Option<Arc<dyn brassclaw_turns::run_profile::RetrievalLookup>> = None;

    // v3 Phase H.12.5: wire the production OrchestratorLookup bridge
    // (PgOrchestratorLookup) when the skills-db feature is active. The bridge
    // holds the engine TierZeroOrchestrator facade (deterministic Tier-0
    // channel; LlmBackend = the always-erroring TierZeroLlmGuard so a
    // mis-compiled recipe surfaces loudly instead of silently calling a model),
    // the PG-backed engine Store (PgThreadEngineStore — loads the live Thread
    // via SessionThreadService::read_thread), and the per-run
    // TierZeroEffectExecutorBuilder (cloned out of the capability wiring above).
    // No Postgres pool needed — the thread store reads via the loop thread
    // service, not a raw pool. When the feature is off the slot stays `None` →
    // `NoOrchestrator` → Tier-2 degrade.
    #[cfg(feature = "skills-db")]
    let orchestrator_lookup: Option<Arc<dyn brassclaw_turns::run_profile::OrchestratorLookup>> = {
        use brassclaw_engine::{LlmBackend, TierZeroOrchestrator};
        let thread_store: Arc<dyn brassclaw_engine::Store> =
            Arc::new(crate::pg_thread_engine_store::PgThreadEngineStore::new(
                Arc::clone(&thread_service) as Arc<dyn SessionThreadService>,
                validated_identity.tenant_id.as_str(),
            ));
        let llm_guard =
            Arc::new(crate::tier_zero_llm_guard::TierZeroLlmGuard::new()) as Arc<dyn LlmBackend>;
        let runtime = TierZeroOrchestrator::builder()
            .llm(llm_guard)
            .build()
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: error.to_string(),
            })?;
        // Retain the legacy lookup adapter for its explicit consumers. Ordinary
        // turns are dispatched through the global Monty service below.
        let executor_builder_for_lookup = Arc::clone(&tier_zero_executor_builder);
        Some(Arc::new(
            crate::orchestrator_lookup_impl::PgOrchestratorLookup::new(
                Arc::new(runtime),
                thread_store,
                executor_builder_for_lookup,
            )
            .with_retrieval_lookup(retrieval_lookup.clone()),
        )
            as Arc<dyn brassclaw_turns::run_profile::OrchestratorLookup>)
    };
    #[cfg(not(feature = "skills-db"))]
    let orchestrator_lookup: Option<Arc<dyn brassclaw_turns::run_profile::OrchestratorLookup>> =
        None;

    // v3 Phase E.0 / plan §H3: wire SkillActivationMessageTextResolver so the
    // production host can resolve the raw accepted-message body via the
    // non-consuming `messages_by_run` read (intent matching sees unsanitized
    // text). Not engine-gated — raw-text resolution needs no Postgres; the
    // slot is `None` only when no local-dev skill activation source was
    // configured (the pure-PG path has no `messages_by_run` store).
    let message_text_resolver: Option<Arc<dyn brassclaw_turns::run_profile::MessageTextResolver>> =
        skill_activation_source.as_ref().map(|source| {
            Arc::new(
                crate::retrieval_lookup_impl::SkillActivationMessageTextResolver::new(Arc::clone(
                    source,
                )),
            ) as Arc<dyn brassclaw_turns::run_profile::MessageTextResolver>
        });

    // Wire PgInterceptorStore when a Postgres pool is available.
    // In DB-less mode the interceptor store remains None and the
    // `on_prompt_assembled` hook is a no-op.
    #[cfg(feature = "postgres")]
    let interceptor_store: Option<Arc<dyn brassclaw_interceptor::InterceptorStore>> =
        services.pg_pool.as_ref().map(|pool| {
            Arc::new(brassclaw_interceptor::PgInterceptorStore::new(
                Arc::clone(pool),
                validated_identity.tenant_id.as_str(),
            )) as Arc<dyn brassclaw_interceptor::InterceptorStore>
        });
    #[cfg(not(feature = "postgres"))]
    let interceptor_store: Option<Arc<dyn brassclaw_interceptor::InterceptorStore>> = None;

    // Create the SharedInterceptorMode before building DefaultPlannedRuntimeParts.
    // The same Arc<AtomicBool> is shared between the host factory (per-turn decisions)
    // and RebornRuntime (settings service flips it).  No DB pool required — the mode
    // is an atomic bool, but rerouting only ever fires when the host has both a store
    // (postgres) and a gateway (root-llm-provider).
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    let interceptor_mode: Option<brassclaw_interceptor::SharedInterceptorMode> = services
        .pg_pool
        .as_ref()
        .map(|_| brassclaw_interceptor::SharedInterceptorMode::new());

    // Wire PgSempaiProposalSink when Postgres pool is available.
    // Routes Sempai-proposed component updates and intent examples to Q1
    // validation queue so operators can review before they take effect.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    let proposal_sink: Option<Arc<dyn brassclaw_interceptor::SempaiProposalSink>> =
        services.pg_pool.as_ref().map(|pool| {
            Arc::new(crate::sempai_proposal_sink::PgSempaiProposalSink::new(
                Arc::clone(pool),
                validated_identity.tenant_id.as_str(),
                validated_identity.agent_id.as_str(),
            )) as Arc<dyn brassclaw_interceptor::SempaiProposalSink>
        });

    #[cfg(all(
        feature = "postgres",
        feature = "root-llm-provider",
        feature = "skills-db"
    ))]
    let system_bundle_source: Option<Arc<dyn brassclaw_loop_support::SystemBundleSource>> =
        Some(installed_catalogue.clone());
    #[cfg(all(
        feature = "postgres",
        feature = "root-llm-provider",
        not(feature = "skills-db")
    ))]
    let system_bundle_source: Option<Arc<dyn brassclaw_loop_support::SystemBundleSource>> =
        services.pg_pool.as_ref().map(|pool| {
            Arc::new(crate::pg_basic_prompt_store::PgBasicPromptStore::new(
                pool.clone(),
                validated_identity.tenant_id.as_str(),
                validated_identity.agent_id.as_str(),
            )) as Arc<dyn brassclaw_loop_support::SystemBundleSource>
        });

    #[cfg(all(
        feature = "postgres",
        feature = "skills-db",
        feature = "root-llm-provider"
    ))]
    let review_accountant = model_budget_accountant.clone();
    let composition = build_default_planned_runtime(DefaultPlannedRuntimeParts {
        turn_state: Arc::clone(&turn_state_store),
        thread_service: Arc::clone(&thread_service),
        thread_scope: thread_scope.clone(),
        model_gateway: Arc::clone(&model_gateway),
        checkpoint_state_store: Arc::clone(&checkpoint_state_store)
            as Arc<dyn brassclaw_turns::CheckpointStateStore>,
        loop_checkpoint_store: Arc::clone(&loop_checkpoint_store)
            as Arc<dyn brassclaw_turns::LoopCheckpointStore>,
        milestone_sink,
        capability_factory,
        capability_surface_resolver: Arc::new(AllowAllCapabilitySurfaceResolver),
        capability_result_writer,
        subagent_goal_store,
        subagent_gate_store: Arc::new(BoundedSubagentGateResolutionStore::new()),
        subagent_definition_resolver: Arc::new(StaticSubagentDefinitionResolver),
        subagent_spawn_input_codec: Arc::new(JsonSpawnSubagentInputCodec::new(
            capability_input_resolver,
        )),
        subagent_spawn_limits: brassclaw_loop_support::SubagentSpawnLimits::default(),
        loop_exit_evidence,
        config: DefaultPlannedRuntimeConfig {
            worker: TurnRunnerWorkerConfig {
                heartbeat_interval: runner.heartbeat_interval,
                poll_interval: runner.poll_interval,
                scope_filter: None,
                max_driver_wall_time: None,
            },
            context_token_budget: live_context_budget.clone(),
            identity_token_ceiling,
            capability_surface_tokens,
            context_window_tokens: live_context_window.clone(),
            max_output_tokens: live_max_output.clone(),
            inline_control_tokens: live_inline_control.clone(),
            total_input_tokens: live_total_input.clone(),
            capability_focus_enabled,
            planning_mode_enabled,
            ..DefaultPlannedRuntimeConfig::default()
        },
        model_route_resolver: None,
        cancellation_factory: None,
        input_queue: None,
        identity_context_source: Arc::new(
            // `storage_root` and `system_prompt_path` are extracted from the
            // substrate (local-dev or PG) at the top of this function.
            DefaultSystemPromptIdentitySource::try_new(
                storage_root.clone(),
                system_prompt_path.clone(),
            )
            .map_err(|error| RebornRuntimeError::InvalidArgument {
                reason: error.to_string(),
            })?,
        ),
        model_policy_guard: None,
        model_budget_accountant,
        token_budget_mode: {
            #[cfg(all(feature = "postgres", feature = "skills-db"))]
            {
                let live = global_monty_owner.client().live_task_settings();
                Some(
                    Arc::new(move || live.current().limits.token_budgets_enabled)
                        as Arc<dyn Fn() -> bool + Send + Sync>,
                )
            }
            #[cfg(not(all(feature = "postgres", feature = "skills-db")))]
            {
                Some(Arc::new(move || resolved_token_budgets_enabled)
                    as Arc<dyn Fn() -> bool + Send + Sync>)
            }
        },
        safety_context: None,
        hook_security_audit_sink: Some(Arc::new(brassclaw_events::TracingSecurityAuditSink)),
        turn_event_sink: None,
        hook_dispatcher_builder_factory,
        recipe_lookup,
        retrieval_lookup,
        orchestrator_lookup,
        message_text_resolver,
        interceptor_store,
        #[cfg(feature = "root-llm-provider")]
        sempai_gateway: llm_reload.as_ref().and_then(|r| r.sempai_gateway.clone()),
        #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
        interceptor_mode: {
            // Create the SharedInterceptorMode here (before llm_reload is moved).
            // It will be cloned into both DefaultPlannedRuntimeParts and RebornRuntime.
            interceptor_mode.clone()
        },
        #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
        proposal_sink,
        #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
        system_bundle_source,
        monty_driver,
    })?;
    let default_resolved_run_profile = composition
        .run_profile_resolver
        .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
        .await
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: format!("could not resolve default run profile: {error}"),
        })?;
    let default_run_profile_id = default_resolved_run_profile.profile_id.as_str().to_string();
    #[cfg(all(
        feature = "postgres",
        feature = "skills-db",
        feature = "root-llm-provider"
    ))]
    let completed_turn_review_owner = crate::completed_turn_review::ReviewOwner::start(
        services
            .pg_pool
            .clone()
            .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                reason: "review journal requires PostgreSQL".into(),
            })?,
        services
            .monty_kernel
            .clone()
            .ok_or_else(|| RebornRuntimeError::InvalidArgument {
                reason: "review requires captured kernel".into(),
            })?,
        global_monty_owner.client(),
        global_monty_owner.ownership_check(),
        thread_service.clone(),
        thread_scope.clone(),
        crate::completed_turn_review::ReviewSource {
            provider: llm_reload.as_ref().and_then(|r| r.sempai_swappable.clone()),
            accountant: review_accountant,
            profile: default_resolved_run_profile.clone(),
        },
    )
    .await?;

    let failure_explanation_thread_id =
        ThreadId::new("failure-explanation-system").map_err(|reason| {
            RebornRuntimeError::InvalidArgument {
                reason: format!("failure explanation thread id: {reason}"),
            }
        })?;
    let failure_explanation_scope = TurnScope::new(
        thread_scope.tenant_id.clone(),
        Some(thread_scope.agent_id.clone()),
        thread_scope.project_id.clone(),
        failure_explanation_thread_id,
    );
    let failure_explanation_profile = default_resolved_run_profile.clone();
    let failure_explanation_model_gateway = Arc::clone(&model_gateway);
    let failure_explanation_inference = Arc::new(move || {
        Arc::new(ModelGatewayBackedSystemInferencePort::new(
            Arc::clone(&failure_explanation_model_gateway),
            LoopRunContext::new(
                failure_explanation_scope.clone(),
                TurnId::new(),
                TurnRunId::new(),
                failure_explanation_profile.clone(),
            ),
        )) as Arc<dyn brassclaw_turns::run_profile::SystemInferencePort>
    });
    let planned_turn_coordinator: Arc<dyn TurnCoordinator> = composition.coordinator.clone();
    let interaction_turn_state: Option<Arc<dyn run_state_read_source::RunStateReadSource>> = {
        #[cfg(feature = "postgres")]
        {
            if let Some(stores) = pg_stores.as_ref() {
                Some(stores.turn_state.clone() as Arc<dyn run_state_read_source::RunStateReadSource>)
            } else {
                local_dev_turn_state
                    .clone()
                    .map(|store| store as Arc<dyn run_state_read_source::RunStateReadSource>)
            }
        }
        #[cfg(not(feature = "postgres"))]
        {
            local_dev_turn_state
                .clone()
                .map(|store| store as Arc<dyn run_state_read_source::RunStateReadSource>)
        }
    };
    let approval_turn_runs: Arc<dyn ApprovalTurnRunLocator> =
        Arc::new(RuntimeApprovalTurnRunLocator {
            turn_state: interaction_turn_state.clone().ok_or_else(|| {
                RebornRuntimeError::InvalidArgument {
                    reason: "approval run state reader unavailable".to_owned(),
                }
            })?,
        });
    let approval_read_model = Arc::new(RunStateApprovalInteractionReadModel::new(
        Arc::clone(&substrate_approval_requests),
        approval_turn_runs,
    ));
    let approval_audit_sink = Arc::new(InMemoryAuditSink::new());
    let approval_resolver = Arc::new(
        ApprovalResolverPort::new(
            Arc::clone(&substrate_approval_requests),
            Arc::clone(&substrate_capability_leases),
        )
        .with_audit_sink(approval_audit_sink.clone()),
    );
    let approval_interaction_service: Arc<dyn ApprovalInteractionService> = {
        let local_dev_capability_policy =
            Arc::new(local_dev_capability_policy().map_err(|error| {
                tracing::error!(%error, "local-dev capability policy is invalid for approval");
                RebornRuntimeError::InvalidArgument {
                    reason: format!("local-dev capability policy is invalid: {error}"),
                }
            })?);
        Arc::new(DefaultApprovalInteractionService::new(
            approval_read_model,
            Arc::new(approval::LocalDevApprovalLeaseTermsProvider::new(
                local_dev_capability_policy,
                substrate_workspace_mounts,
                substrate_skill_mounts,
                substrate_memory_mounts,
            )),
            approval_resolver,
            Arc::clone(&planned_turn_coordinator),
        ))
    };
    let auth_interaction_service = build_webui_auth_interaction_service(
        services.product_auth.as_deref(),
        interaction_turn_state,
        Arc::clone(&planned_turn_coordinator),
    );
    let turn_event_source: Arc<dyn TurnEventProjectionSource> = turn_state_store.clone();
    let projection_services = projection_services
        .with_turn_events(turn_event_source, Arc::clone(&planned_turn_coordinator))
        .with_model_failure_explainer_factory(failure_explanation_inference)
        .with_display_previews(Arc::clone(&local_dev_capabilities.display_previews));
    // Wire auth-challenge enrichment when the product-auth bundle exposes a
    // flow record source (local-dev / test mode). Production deployments without
    // a wired flow_record_source fall back to the plain 4-field AuthPromptView.
    let projection_services = if let Some(provider) = services
        .product_auth
        .as_ref()
        .and_then(|pa| pa.as_auth_challenge_provider())
    {
        projection_services.with_auth_challenges(provider)
    } else {
        projection_services
    };
    services.turn_coordinator = Some(Arc::clone(&planned_turn_coordinator));

    // Both `trigger_poller_handle` and the test-support
    // `trigger_conversation_pairing_value` are produced atomically inside
    // a single `if trigger_poller.enabled` expression.
    #[cfg(any(test, feature = "test-support"))]
    // Safety: assigned in every branch below; clippy's needless_late_init does not
    // support #[cfg]-gated assignments so the late-init form is necessary here.
    #[allow(clippy::needless_late_init)]
    let trigger_conversation_pairing_value: Option<
        Arc<dyn brassclaw_conversations::ConversationActorPairingService>,
    >;
    let trigger_poller_handle: Option<TriggerPollerRuntimeHandle> = if trigger_poller.enabled {
        validate_trigger_poller_authorization(
            &trigger_poller,
            trigger_fire_access_checker.as_ref(),
        )?;
        let trigger_poller_services = build_trigger_poller_services(
            services.local_runtime.as_deref(),
            #[cfg(feature = "postgres")]
            services.pg_pool.as_ref(),
            Arc::clone(&planned_turn_coordinator),
            Arc::clone(&thread_service),
            trigger_poller.authorizer,
            trigger_fire_access_checker.clone(),
            thread_scope.tenant_id.clone(),
            validated_identity.agent_id.clone(),
        )
        .await?;
        let active_run_lookup = build_trigger_active_run_lookup(local_dev_turn_state.clone());
        // Stash clones for the automation facade (fire_now path) before the
        // services are moved into TriggerPollerCompositionDeps.
        services.trigger_repository = Some(Arc::clone(&substrate_trigger_repository));
        services.trusted_submitter = Some(Arc::clone(&trigger_poller_services.trusted_submitter));
        services.trigger_materializer = Some(Arc::clone(&trigger_poller_services.materializer));
        #[cfg(any(test, feature = "test-support"))]
        {
            trigger_conversation_pairing_value =
                Some(Arc::clone(&trigger_poller_services.pairing_service));
        }
        spawn_trigger_poller(
            trigger_poller,
            TriggerPollerCompositionDeps {
                repository: Arc::clone(&substrate_trigger_repository),
                materializer: trigger_poller_services.materializer,
                trusted_submitter: trigger_poller_services.trusted_submitter,
                active_run_lookup,
            },
        )
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: format!("trigger poller could not be started: {error}"),
        })?
    } else {
        #[cfg(any(test, feature = "test-support"))]
        {
            trigger_conversation_pairing_value = None;
        }
        None
    };
    let worker_cancel = CancellationToken::new();
    let worker = Arc::clone(&composition.worker);
    let worker_cancel_clone = worker_cancel.clone();
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    let worker_monty_client = global_monty_owner.client();
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    let worker_mcp_binding = mcp_provider_binding.clone();
    let worker_handle = tokio::spawn(async move {
        #[cfg(all(feature = "postgres", feature = "skills-db"))]
        if inbound_mcp_required {
            tokio::select! {
                () = worker_cancel_clone.cancelled() => return,
                () = worker_mcp_binding.wait_ready() => {}
            }
        }
        #[cfg(all(feature = "postgres", feature = "skills-db"))]
        worker
            .run_with_admission_capacity(worker_cancel_clone, move || {
                // One additional transport claim lets a nested ordinary chat reach
                // Monty's own admission check and fail explicitly at capacity,
                // rather than deadlock behind its waiting parent. This grants no
                // additional VM slot or resource budget.
                worker_monty_client
                    .admission_observation()
                    .limits
                    .max_tasks
                    .min(worker_monty_client.retained_attempts().limit)
                    .saturating_add(1) as usize
            })
            .await;
        #[cfg(not(all(feature = "postgres", feature = "skills-db")))]
        worker.run(worker_cancel_clone).await;
    });
    services.readiness.workers.turn_runner = true;
    services.readiness.workers.trigger_poller = trigger_poller_handle.is_some();
    let turn_coordinator = planned_turn_coordinator;
    let wake_sender = composition.wake_sender;

    // Spawn the budget-event projection task as the production owner
    // of the broadcast sink — review feedback Thermo-Nuclear #3
    // (#3841 follow-up A2). The runtime's `broadcast_budget_event_sink`
    // accessor used to expose a sink that no one subscribed to; with
    // this projection the runtime always has at least the tracing
    // observer attached, and callers can install a richer observer
    // (SSE projection, telemetry export) through
    // `RebornRuntimeInput::with_budget_event_observer`.
    let budget_event_projection = {
        let observer = budget_event_observer.unwrap_or_else(|| {
            Arc::new(crate::TracingBudgetEventObserver) as Arc<dyn crate::BudgetEventObserver>
        });
        Some(crate::budget_events::BudgetEventProjection::spawn(
            broadcast_budget_sink.as_ref(),
            observer,
        ))
    };

    // Wire plan library when enabled. The service holds an Arc to the root
    // filesystem so it can write plan documents and SKILL.md files.
    let (plan_library, plan_state_slot) = if plan_library_enabled {
        if let Some(ref local_runtime) = services.local_runtime {
            let fs = Arc::clone(&local_runtime.extension_filesystem);
            let library = Arc::new(crate::plan_library::PlanLibraryService::new(
                fs,
                skill_promotion_threshold,
            ));
            let slot = local_runtime.plan_state_slot.clone();
            (Some(library), Some(slot))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    Ok(RebornRuntime {
        #[cfg(feature = "skills-db")]
        mcp_recipe_discovery: installed_catalogue.mcp_discovery(),
        #[cfg(feature = "skills-db")]
        mcp_chat_bridge: std::sync::OnceLock::new(),
        #[cfg(feature = "skills-db")]
        mcp_listener_status: std::sync::OnceLock::new(),
        #[cfg(feature = "skills-db")]
        mcp_provider_binding,
        services,
        #[cfg(all(feature = "postgres", feature = "skills-db"))]
        global_monty_owner,
        #[cfg(all(
            feature = "postgres",
            feature = "skills-db",
            feature = "root-llm-provider"
        ))]
        completed_turn_review_owner,
        #[cfg(all(feature = "postgres", feature = "skills-db"))]
        monty_settings_owner,
        #[cfg(all(test, feature = "postgres", feature = "skills-db"))]
        monty_test_controls,
        turn_coordinator,
        turn_tree_store: turn_state_store,
        thread_service,
        thread_scope,
        worker_handle,
        worker_cancel,
        trigger_poller_handle,
        #[cfg(any(test, feature = "test-support"))]
        trigger_conversation_pairing: trigger_conversation_pairing_value,
        budget_event_projection,
        #[cfg(any(test, feature = "test-support"))]
        resource_governor: resource_governor_for_accountant,
        #[cfg(any(test, feature = "test-support"))]
        budget_gate_store: budget_gate_store_for_accountant,
        broadcast_budget_sink,
        #[cfg(any(test, feature = "test-support"))]
        in_memory_budget_sink,
        poll_settings: poll,
        actor_user_id,
        source_binding_ref: validated_identity.source_binding_ref,
        reply_target_binding_ref: validated_identity.reply_target_binding_ref,
        projection_services,
        approval_interaction_service,
        auth_interaction_service,
        #[cfg(test)]
        approval_audit_sink,
        webui_event_log: event_log,
        default_run_profile_id,
        wake_sender,
        send_locks: Mutex::new(HashMap::new()),
        internal_conversation_scopes: Mutex::new(HashMap::new()),
        skill_activation_source,
        plan_library,
        plan_state_slot,
        #[cfg(feature = "root-llm-provider")]
        llm_reload,
        #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
        interceptor_mode,
    })
}

/// Legacy settings lookup shared with startup failure acceptance. The global
/// revision cutover must replace this startup snapshot, not reuse it as a wall
/// timer. A missing row is the store's explicit default; a failed read is fatal.
#[cfg(all(feature = "postgres", any(test, not(feature = "skills-db"))))]
async fn read_startup_monty_settings(
    pool: &Arc<brassclaw_pg::PgPool>,
) -> Result<brassclaw_product_workflow::MontyVmSettings, RebornRuntimeError> {
    use brassclaw_product_workflow::MontyVmSettingsStore as _;
    crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(Arc::clone(pool), "default", "default")
        .get("default", "default")
        .await
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: format!("cannot load Monty VM settings at startup: {error}"),
        })
}

fn build_webui_auth_interaction_service(
    product_auth: Option<&RebornProductAuthServices>,
    turn_state_store: Option<Arc<dyn run_state_read_source::RunStateReadSource>>,
    turn_coordinator: Arc<dyn TurnCoordinator>,
) -> Arc<dyn AuthInteractionService> {
    // `AuthFlowRecordSource` is optional on the product-auth bundle because
    // production may supply a durable read projection that is not the flow
    // manager itself. Local-dev can render pending WebUI auth interactions only
    // when the bundle explicitly exposes this scoped projection; otherwise the
    // WebUI surface fails closed with a stable unavailable error.
    // Both durable and local stores provide the same scoped run projection.
    let Some(product_auth) = product_auth else {
        return Arc::new(auth_interaction::UnavailableAuthInteractionService);
    };
    let Some(flow_records) = product_auth.flow_record_source() else {
        return Arc::new(auth_interaction::UnavailableAuthInteractionService);
    };
    let Some(turn_state_store) = turn_state_store else {
        return Arc::new(auth_interaction::UnavailableAuthInteractionService);
    };
    Arc::new(DefaultAuthInteractionService::new(
        Arc::new(auth_interaction::RunStateAuthInteractionReadModel::new(
            turn_state_store,
            flow_records,
        )),
        product_auth.flow_manager(),
        turn_coordinator,
    ))
}

const LOOP_RUN_CAPABILITY_ID: &str = "loop.run";
const TRUSTED_LAPTOP_ACCESS_AUDIT_KIND: &str = "local_dev_trusted_laptop_access";
const TRUSTED_LAPTOP_ACCESS_AUDIT_TARGET: &str = "filesystem=host_workspace_and_home;process=local_host;network=direct;secrets=inherited_env;host_home_mount=/host";
const TRUSTED_LAPTOP_ACCESS_AUDIT_STATUS: &str = "host_home_mounted_read_write";

async fn append_trusted_laptop_access_audit(
    audit_log: &Arc<dyn DurableAuditLog>,
    thread_scope: &ThreadScope,
    actor_user_id: &UserId,
) -> Result<(), RebornRuntimeError> {
    let invocation_id = InvocationId::new();
    audit_log
        .append(AuditEnvelope {
            event_id: AuditEventId::new(),
            correlation_id: CorrelationId::new(),
            stage: AuditStage::After,
            timestamp: Utc::now(),
            tenant_id: thread_scope.tenant_id.clone(),
            user_id: actor_user_id.clone(),
            agent_id: Some(thread_scope.agent_id.clone()),
            project_id: thread_scope.project_id.clone(),
            thread_id: None,
            invocation_id,
            process_id: None,
            approval_request_id: None,
            extension_id: None,
            action: ActionSummary {
                kind: TRUSTED_LAPTOP_ACCESS_AUDIT_KIND.to_string(),
                target: Some(TRUSTED_LAPTOP_ACCESS_AUDIT_TARGET.to_string()),
                effects: vec![
                    EffectKind::ReadFilesystem,
                    EffectKind::WriteFilesystem,
                    EffectKind::SpawnProcess,
                    EffectKind::Network,
                    EffectKind::UseSecret,
                ],
            },
            decision: DecisionSummary {
                kind: "allowed".to_string(),
                reason: None,
                actor: None,
            },
            result: Some(ActionResultSummary {
                success: true,
                status: Some(TRUSTED_LAPTOP_ACCESS_AUDIT_STATUS.to_string()),
                output_bytes: None,
            }),
        })
        .await
        .map(|_| ())
        .map_err(|error| RebornRuntimeError::InvalidArgument {
            reason: format!("could not record trusted laptop access audit event: {error}"),
        })
}

struct ValidatedRuntimeIdentity {
    tenant_id: TenantId,
    agent_id: AgentId,
    source_binding_ref: SourceBindingRef,
    reply_target_binding_ref: ReplyTargetBindingRef,
}

fn validate_runtime_identity(
    identity: RebornRuntimeIdentity,
) -> Result<ValidatedRuntimeIdentity, RebornRuntimeError> {
    let tenant_id = TenantId::new(identity.tenant_id).map_err(|reason| {
        RebornRuntimeError::InvalidArgument {
            reason: format!("tenant id: {reason}"),
        }
    })?;
    let agent_id =
        AgentId::new(identity.agent_id).map_err(|reason| RebornRuntimeError::InvalidArgument {
            reason: format!("agent id: {reason}"),
        })?;
    let source_binding_ref =
        SourceBindingRef::new(identity.source_binding_id).map_err(|reason| {
            RebornRuntimeError::InvalidArgument {
                reason: format!("source binding id: {reason}"),
            }
        })?;
    let reply_target_binding_ref = ReplyTargetBindingRef::new(identity.reply_target_binding_id)
        .map_err(|reason| RebornRuntimeError::InvalidArgument {
            reason: format!("reply target binding id: {reason}"),
        })?;
    Ok(ValidatedRuntimeIdentity {
        tenant_id,
        agent_id,
        source_binding_ref,
        reply_target_binding_ref,
    })
}

struct AllowAllCapabilitySurfaceResolver;

#[async_trait::async_trait]
impl CapabilitySurfaceProfileResolver for AllowAllCapabilitySurfaceResolver {
    async fn resolve(
        &self,
        _run_context: &LoopRunContext,
    ) -> Result<CapabilityAllowSet, CapabilityResolveError> {
        Ok(CapabilityAllowSet::All)
    }
}

/// Build the production model gateway and its (optional) LLM-derived
/// cost table. Cfg-gated so off-feature builds short-circuit to the
/// no-op fallback without referencing types that don't exist.
#[cfg(feature = "root-llm-provider")]
async fn build_production_model_gateway(
    llm: Option<crate::runtime_input::ResolvedRebornLlm>,
    max_output_tokens: Option<brassclaw_agent_loop::LiveTokenBudget>,
    total_input_tokens: Option<brassclaw_agent_loop::LiveTokenBudget>,
    cache_retention: Option<String>,
) -> Result<
    (
        Arc<dyn brassclaw_loop_support::HostManagedModelGateway>,
        Option<brassclaw_loop_support::StaticModelCostTable>,
        Option<RebornLlmReloadParts>,
    ),
    RebornRuntimeError,
> {
    let parsed_cache_retention = cache_retention
        .as_deref()
        .map(str::parse::<brassclaw_llm::CacheRetention>)
        .transpose();
    // Unknown values must not break boot — fall back to None silently
    // rather than poisoning the runtime.
    let cache_retention = match parsed_cache_retention {
        Ok(cr) => cr,
        Err(error) => {
            tracing::debug!(
                cache_retention = ?cache_retention,
                error = %error,
                "ignoring unparseable CacheRetention override; using provider default"
            );
            None
        }
    };
    // Even with no LLM configured at boot we build a real swappable gateway
    // around a placeholder provider (which errors until swapped) plus a reload
    // handle. That way the FIRST configuration made through the settings UI
    // hot-swaps the placeholder into a working provider without a restart —
    // otherwise a cold boot would wire a dead stub with no reload seam.
    match llm {
        Some(cfg) => {
            let LlmGatewayBundle {
                gateway,
                policy,
                reload,
            } = build_llm_gateway(cfg, max_output_tokens, total_input_tokens, cache_retention)
                .await?;
            Ok((gateway, Some(policy.build_cost_table()), Some(reload)))
        }
        None => {
            let LlmGatewayBundle {
                gateway, reload, ..
            } = build_placeholder_llm_gateway().await?;
            // No cost table for the placeholder: there is no real model to cost,
            // and a synthetic table would gate budgets against a model that
            // isn't actually in use. The budget cost table is (re)derived when a
            // real provider is configured + the binary restarts.
            Ok((gateway, None, Some(reload)))
        }
    }
}

#[cfg(not(feature = "root-llm-provider"))]
fn build_production_model_gateway() -> Result<
    (
        Arc<dyn brassclaw_loop_support::HostManagedModelGateway>,
        Option<brassclaw_loop_support::StaticModelCostTable>,
    ),
    RebornRuntimeError,
> {
    Ok((build_no_llm_gateway(), None))
}

#[cfg(feature = "root-llm-provider")]
struct LlmGatewayBundle {
    gateway: Arc<dyn brassclaw_loop_support::HostManagedModelGateway>,
    /// Policy used to derive the budget accountant's cost table — kept
    /// alongside the gateway so the composer doesn't re-derive the
    /// `ModelProfileId → provider-model` mapping in two places.
    policy: brassclaw_reborn::model_gateway::LlmModelProfilePolicy,
    /// Hot-swap handle + session for the live-reload path. The model gateway
    /// wraps a [`SwappableLlmProvider`], so the settings service can rebuild
    /// the provider chain from updated config and atomically swap the inner
    /// backend without rebuilding the gateway or restarting the binary.
    reload: RebornLlmReloadParts,
}

/// The pieces the LLM-config settings service needs to hot-swap the running
/// provider: the reload handle wrapping the live `SwappableLlmProvider`, and
/// the session manager to rebuild the chain against.
///
/// The `sempai_swappable` field holds the live [`SwappableLlmProvider`] for the
/// Sempai (teaching/auditing) role.  It is allocated at startup so the settings
/// service can call `swap()` on it whenever the operator changes the Sempai
/// selection, without requiring a restart or a gateway rebuild.
#[cfg(feature = "root-llm-provider")]
pub(crate) struct RebornLlmReloadParts {
    pub(crate) reload_handle: Arc<brassclaw_llm::LlmReloadHandle>,
    pub(crate) session: Arc<brassclaw_llm::SessionManager>,
    pub(crate) nearai_login_states: Arc<crate::llm_config_service::NearAiLoginStateStore>,
    /// Live hot-swap wrapper for the Sempai provider.  Allocated at startup
    /// wrapping a `PlaceholderLlmProvider`.  The settings service swaps the
    /// inner provider when the operator activates a Sempai selection via
    /// `set_active(Sempai, ...)`.
    pub(crate) sempai_swappable: Option<Arc<brassclaw_llm::SwappableLlmProvider>>,
    /// Sempai model gateway — wraps `sempai_swappable` in a
    /// `LlmProviderModelGateway` so the interceptor host can call it via the
    /// `HostManagedModelGateway` trait without knowing the concrete type.
    pub(crate) sempai_gateway: Option<Arc<dyn brassclaw_loop_support::HostManagedModelGateway>>,
}

#[cfg(feature = "root-llm-provider")]
async fn build_llm_gateway(
    llm: ResolvedRebornLlm,
    max_output_tokens: Option<brassclaw_agent_loop::LiveTokenBudget>,
    total_input_tokens: Option<brassclaw_agent_loop::LiveTokenBudget>,
    cache_retention: Option<brassclaw_llm::CacheRetention>,
) -> Result<LlmGatewayBundle, RebornRuntimeError> {
    let model = llm.model().to_string();
    let session = brassclaw_llm::create_session_manager(llm.config.session.clone()).await;
    let mut raw = brassclaw_llm::build_static_provider_chain(&llm.config, Arc::clone(&session))
        .await
        .map_err(|error| RebornRuntimeError::LlmProvider(error.to_string()))?;
    if let Some(retention) = cache_retention {
        raw = brassclaw_llm::apply_cache_retention(raw, retention);
    }
    wrap_swappable_gateway(
        raw,
        Some(model),
        session,
        max_output_tokens,
        total_input_tokens,
    )
}

/// Cold-boot gateway: no LLM configured yet. Wraps a placeholder provider (which
/// errors until swapped) so the model-gateway + reload seam exist from the
/// start; the first configuration applied through the settings UI swaps the
/// placeholder for a real provider chain with no restart.
#[cfg(feature = "root-llm-provider")]
async fn build_placeholder_llm_gateway() -> Result<LlmGatewayBundle, RebornRuntimeError> {
    let session =
        brassclaw_llm::create_session_manager(brassclaw_llm::SessionConfig::default()).await;
    let raw: Arc<dyn brassclaw_llm::LlmProvider> = Arc::new(PlaceholderLlmProvider);
    // No max_output_tokens or total_input guard for the placeholder — it will error on every call anyway.
    wrap_swappable_gateway(raw, None, session, None, None)
}

/// Wrap a raw provider in a [`SwappableLlmProvider`] + reload handle and build
/// the model gateway. Shared by the real and placeholder boot paths so both get
/// an identical live-reload seam.
///
/// Also allocates the Sempai [`SwappableLlmProvider`] (initially wrapping
/// a `PlaceholderLlmProvider`) so the settings service can swap it in place
/// when the operator configures a Sempai selection, without a restart.
#[cfg(feature = "root-llm-provider")]
fn wrap_swappable_gateway(
    raw: Arc<dyn brassclaw_llm::LlmProvider>,
    model: Option<String>,
    session: Arc<brassclaw_llm::SessionManager>,
    max_output_tokens: Option<brassclaw_agent_loop::LiveTokenBudget>,
    total_input_tokens: Option<brassclaw_agent_loop::LiveTokenBudget>,
) -> Result<LlmGatewayBundle, RebornRuntimeError> {
    use brassclaw_llm::{LlmProvider, LlmReloadHandle, SwappableLlmProvider};
    use brassclaw_reborn::model_gateway::{LlmModelProfilePolicy, LlmProviderModelGateway};
    use brassclaw_turns::run_profile::ModelProfileId;

    let swappable = Arc::new(SwappableLlmProvider::new(raw));
    let reload_handle = Arc::new(LlmReloadHandle::new(Arc::clone(&swappable), None));
    let provider: Arc<dyn LlmProvider> = swappable;

    // Allocate the Sempai swappable starting with a placeholder (no Sempai
    // configured yet).  The settings service swaps the inner provider when
    // the operator connects a Sempai selection.
    let sempai_inner: Arc<dyn LlmProvider> = Arc::new(PlaceholderLlmProvider);
    let sempai_swappable = Arc::new(SwappableLlmProvider::new(sempai_inner));

    // Build the Sempai gateway using a dedicated "sempai_model" profile.
    // The gateway is backed by the swappable so a live-swap of the inner
    // provider is picked up immediately without rebuilding the gateway.
    let sempai_model_profile_id = ModelProfileId::new("sempai_model").map_err(|reason| {
        RebornRuntimeError::LlmProvider(format!("invalid sempai model profile id: {reason}"))
    })?;
    let sempai_policy =
        LlmModelProfilePolicy::new().allow_model_profile(sempai_model_profile_id, None);
    let sempai_provider: Arc<dyn LlmProvider> =
        Arc::clone(&sempai_swappable) as Arc<dyn LlmProvider>;
    let sempai_gateway: Arc<dyn brassclaw_loop_support::HostManagedModelGateway> =
        Arc::new(LlmProviderModelGateway::new(sempai_provider, sempai_policy));

    let model_profile_id = ModelProfileId::new("interactive_model").map_err(|reason| {
        RebornRuntimeError::LlmProvider(format!("invalid interactive model profile id: {reason}"))
    })?;
    let policy = LlmModelProfilePolicy::new().allow_model_profile(model_profile_id, model);
    let gateway = LlmProviderModelGateway::new(provider, policy.clone())
        .with_max_output_tokens(max_output_tokens)
        .with_total_input_tokens(total_input_tokens);
    Ok(LlmGatewayBundle {
        gateway: Arc::new(gateway),
        policy,
        reload: RebornLlmReloadParts {
            reload_handle,
            session,
            nearai_login_states: Arc::new(crate::llm_config_service::NearAiLoginStateStore::new()),
            sempai_swappable: Some(Arc::clone(&sempai_swappable)),
            sempai_gateway: Some(sempai_gateway),
        },
    })
}

/// Stand-in provider used before any LLM is configured. Every call fails with a
/// clear, user-safe message; it exists only so the gateway/reload seam is live
/// from a cold boot and the first configuration can swap it out.
///
/// Also used by `RebornLlmConfigService` when the Sempai slot is cleared, to
/// swap the existing Sempai provider back to a no-op stub.
#[cfg(feature = "root-llm-provider")]
#[derive(Debug)]
pub(crate) struct PlaceholderLlmProvider;

#[cfg(feature = "root-llm-provider")]
#[async_trait::async_trait]
impl brassclaw_llm::LlmProvider for PlaceholderLlmProvider {
    fn model_name(&self) -> &str {
        "unconfigured"
    }

    fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
        (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
    }

    async fn complete(
        &self,
        _request: brassclaw_llm::CompletionRequest,
    ) -> Result<brassclaw_llm::CompletionResponse, brassclaw_llm::LlmError> {
        Err(placeholder_unconfigured_error())
    }

    async fn complete_with_tools(
        &self,
        _request: brassclaw_llm::ToolCompletionRequest,
    ) -> Result<brassclaw_llm::ToolCompletionResponse, brassclaw_llm::LlmError> {
        Err(placeholder_unconfigured_error())
    }
}

#[cfg(feature = "root-llm-provider")]
fn placeholder_unconfigured_error() -> brassclaw_llm::LlmError {
    brassclaw_llm::LlmError::RequestFailed {
        provider: "unconfigured".to_string(),
        reason: "no LLM provider is configured yet; choose one in Settings → Inference".to_string(),
    }
}

// Substrate-only build (without the `root-llm-provider` feature) wires a no-op gateway
// that returns Unavailable on every call. With the LLM provider compiled in, a cold boot
// uses a placeholder-backed swappable gateway instead (see `build_placeholder_llm_gateway`).
#[cfg(not(feature = "root-llm-provider"))]
fn build_no_llm_gateway() -> Arc<dyn brassclaw_loop_support::HostManagedModelGateway> {
    use async_trait::async_trait;
    use brassclaw_loop_support::{
        HostManagedModelError, HostManagedModelErrorKind, HostManagedModelGateway,
        HostManagedModelRequest, HostManagedModelResponse,
    };

    #[derive(Debug, Default)]
    struct NoLlmGateway;

    #[async_trait]
    impl HostManagedModelGateway for NoLlmGateway {
        async fn stream_model(
            &self,
            _request: HostManagedModelRequest,
        ) -> Result<HostManagedModelResponse, HostManagedModelError> {
            Err(HostManagedModelError::safe(
                HostManagedModelErrorKind::Unavailable,
                "no LLM gateway wired (build with `root-llm-provider` feature)",
            ))
        }
    }

    Arc::new(NoLlmGateway)
}

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    mod mcp_provider_acceptance {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/common/mcp_provider_acceptance.rs"
        ));
    }
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use brassclaw_auth::{GOOGLE_CALENDAR_EVENTS_SCOPE, GOOGLE_CALENDAR_READONLY_SCOPE};
    use brassclaw_authorization::CapabilityLeaseStore;
    use brassclaw_events::{DurableAuditLog, EventStreamKey, ReadScope};
    use brassclaw_host_api::{
        Action, AgentId, ApprovalRequest, ApprovalRequestId, AuditStage, CapabilityId,
        CorrelationId, EffectKind, InvocationFingerprint, InvocationId, Principal,
        ResourceEstimate, ResourceScope, TenantId, ThreadId, UserId,
        runtime_policy::{
            ApprovalPolicy, AuditMode, DeploymentMode, EffectiveRuntimePolicy,
            FilesystemBackendKind, NetworkMode, ProcessBackendKind, RuntimeProfile, SecretMode,
        },
    };
    use brassclaw_loop_support::{
        HostManagedModelError, HostManagedModelErrorKind, HostManagedModelGateway,
        HostManagedModelMessageRole, HostManagedModelRequest, HostManagedModelResponse, ModelCost,
        SpawnSubagentMode, SubagentKindId, SubagentThreadKind, SubagentThreadMetadata,
    };
    use brassclaw_product_adapters::{ProductOutboundPayload, ProductProjectionItem};
    use brassclaw_product_workflow::{
        LifecyclePackageKind, LifecyclePackageRef, LifecyclePhase, LifecycleProductPayload,
        LifecycleReadinessBlocker, RebornExtensionCredentialSetup, RebornServicesErrorCode,
        RebornServicesErrorKind, RebornStreamEventsRequest, RebornSubmitTurnResponse,
        WebUiAuthenticatedCaller, WebUiCreateThreadRequest, WebUiListAutomationsRequest,
        WebUiResolveGateRequest, WebUiSendMessageRequest, WebUiSetupExtensionRequest,
        approval_gate_ref,
    };
    use brassclaw_run_state::ApprovalRequestStore;
    use brassclaw_threads::{
        AppendToolResultReferenceRequest, EnsureThreadRequest, LoadContextMessagesRequest,
        MessageKind, ThreadHistoryRequest, ThreadScope, ToolResultSafeSummary,
    };
    use brassclaw_turns::{
        AcceptedMessageRef, AllowAllTurnAdmissionPolicy, BlockedReason, GetRunStateRequest,
        IdempotencyKey, LoopResultRef, ReplyTargetBindingRef, SanitizedCancelReason,
        SourceBindingRef, SubmitChildRunRequest, SubmitTurnRequest, SubmitTurnResponse, TurnActor,
        TurnCheckpointId, TurnId, TurnLeaseToken, TurnRunId, TurnRunnerId, TurnScope, TurnStatus,
        run_profile::{
            InMemoryRunProfileResolver, LoopCapabilityPort, LoopCheckpointStateRef, ModelProfileId,
            ProviderToolCall, VisibleCapabilityRequest,
        },
        runner::{BlockRunRequest, ClaimRunRequest, TurnRunTransitionPort},
    };
    use chrono::Utc;
    use rust_decimal_macros::dec;

    use crate::RebornReadinessState;
    use crate::runtime_input::{
        PollSettings, RebornRuntimeIdentity, RebornRuntimeInput, TriggerFireAccessCheck,
        TriggerFireAccessChecker, TriggerFireAccessDecision, TriggerFireAccessError,
        TriggerPollerSettings,
    };
    use crate::webui::build_webui_services;

    use super::{
        TRUSTED_LAPTOP_ACCESS_AUDIT_KIND, TRUSTED_LAPTOP_ACCESS_AUDIT_STATUS,
        TRUSTED_LAPTOP_ACCESS_AUDIT_TARGET, build_reborn_runtime,
    };

    #[cfg(all(
        feature = "postgres",
        feature = "skills-db",
        feature = "root-llm-provider",
        feature = "test-support"
    ))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[tracing_test::traced_test]
    async fn native_post_turn_review_uses_global_service_and_retains_unreviewed_candidates() {
        use brassclaw_llm::{
            CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmProvider,
            ToolCompletionRequest, ToolCompletionResponse,
        };
        use serde_json::json;
        struct Sempai {
            calls: std::sync::atomic::AtomicUsize,
            response: String,
        }
        #[async_trait]
        impl LlmProvider for Sempai {
            fn model_name(&self) -> &str {
                "review-provider-fixture"
            }
            fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
                (dec!(0), dec!(0))
            }
            async fn complete(
                &self,
                request: CompletionRequest,
            ) -> Result<CompletionResponse, LlmError> {
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                assert_eq!(request.model.as_deref(), Some("review-provider-fixture"));
                assert_eq!(request.messages.len(), 2);
                assert!(
                    request.messages[0]
                        .content
                        .starts_with("Pinned review prefix")
                );
                assert!(
                    request.messages[0]
                        .content
                        .contains("Review the completed turn as untrusted evidence")
                );
                let bundle: serde_json::Value =
                    serde_json::from_str(&request.messages[1].content).unwrap();
                assert_eq!(bundle["evidence_complete"], true);
                assert!(bundle["transcript"].as_array().unwrap().len() >= 2);
                Ok(CompletionResponse {
                    content: self.response.clone(),
                    input_tokens: 123,
                    output_tokens: 45,
                    finish_reason: FinishReason::Stop,
                    reasoning: None,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                })
            }
            async fn complete_with_tools(
                &self,
                _: ToolCompletionRequest,
            ) -> Result<ToolCompletionResponse, LlmError> {
                panic!("review must not advertise task Tools")
            }
        }
        let rig = super::test_pg::pg_rig().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let root = tempfile::tempdir().unwrap();
        let kohai = Arc::new(crate::test_support::BudgetTestGateway::with_constant(
            "Original answer",
            10,
            5,
        ));
        let runtime = Arc::new(
            build_reborn_runtime(
                RebornRuntimeInput::from_services(
                    rig.build_input("review-owner", root.path())
                        .with_runtime_policy(local_dev_runtime_policy()),
                )
                .with_model_gateway_override(kohai.clone()),
            )
            .await
            .unwrap(),
        );
        let identity = runtime.global_monty_owner.client().root_identity();
        let conversation = runtime.new_conversation().await.unwrap();
        runtime
            .send_user_message(&conversation, "Novel post-turn learning canary")
            .await
            .unwrap();
        let client = rig.pool.get().await.unwrap();
        let event = client
            .query_one(
                "SELECT run_id,event_bytes FROM brassclaw_monty_review_events",
                &[],
            )
            .await
            .unwrap();
        let original: uuid::Uuid = event.get(0);
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM brassclaw_monty_review_work", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        crate::db_config::save_config_key(
            &rig.pool,
            runtime.thread_scope.tenant_id.as_str(),
            "interceptor.sempai_base_prompt",
            "Pinned review prefix: immutable candidate authoring knowledge",
            crate::db_config::ConfigWriteContext::Operator,
        )
        .await
        .unwrap();
        let candidate =
            json!({"format":"component-revision/1","uuid":uuid::Uuid::new_v4(),"class_code":22,
            "document":{"content":"def retain_value(inputs):\n    return inputs['value']",
                "input_contract":{"value":{"type":"string","required":true,"checks":[]}},
                "result_contract":{"type":"string"},
                "preload":{"format":"python-preload/2","exports":{"retain_value":{"symbol":"retain_value",
                    "parameters":["inputs"],"mapping":true}},"private_functions":{},"constants":[],
                    "imports":[],"dependencies":[],"default_export":"retain_value"}},
            "dependencies":[],"association":null})
            .to_string();
        let sempai=Arc::new(Sempai {calls:std::sync::atomic::AtomicUsize::new(0),response:json!({
            "format":"completed-turn-sempai-analysis/1","analysis":"Retain a draft for separate behavioral and Q1/Q2 review",
            "proposals":[{"candidate_bytes":candidate,"base":null,"dependencies":[]}]}).to_string()});
        use brassclaw_turns::run_profile::{RunProfileResolutionRequest, RunProfileResolver};
        let resolved = InMemoryRunProfileResolver::default()
            .resolve_run_profile(RunProfileResolutionRequest::interactive_default())
            .await
            .unwrap();
        let provider = Arc::new(brassclaw_llm::SwappableLlmProvider::new(sempai.clone()));
        let replacement = Arc::new(Sempai {
            calls: std::sync::atomic::AtomicUsize::new(0),
            response: "invalid replacement response".into(),
        });
        let review = crate::completed_turn_review::ReviewOwner::start(
            rig.pool.clone(),
            runtime.services.monty_kernel.clone().unwrap(),
            runtime.global_monty_owner.client(),
            runtime.global_monty_owner.ownership_check(),
            runtime.thread_service.clone(),
            runtime.thread_scope.clone(),
            crate::completed_turn_review::ReviewSource {
                provider: Some(provider.clone()),
                accountant: None,
                profile: resolved,
            },
        )
        .await
        .unwrap();
        let waiting=tokio::time::timeout(Duration::from_secs(180),async {
            let mut replaced = false;
            loop {
                let row=client.query_opt("SELECT phase,receipt_bytes,response_bytes FROM brassclaw_monty_review_work WHERE source_run_id=$1",&[&original]).await.unwrap();
                if let Some(row)=row {
                    let phase:String=row.get(0);
                    if !replaced {
                        assert_eq!(sempai.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
                        provider.swap(replacement.clone());
                        crate::db_config::save_config_key(&rig.pool, runtime.thread_scope.tenant_id.as_str(),
                            "interceptor.sempai_base_prompt", "Replacement prefix for later admissions only",
                            crate::db_config::ConfigWriteContext::Operator).await.unwrap();
                        replaced = true;
                    }
                    if phase=="acknowledged" {break;}
                    assert!(!matches!(phase.as_str(),"failed"|"incomplete"|"uncertain"),"review phase {phase}, receipt {:?}, provider response {:?}, provider calls {}, operations {:?}",
                        row.get::<_,Option<String>>(1),row.get::<_,Option<String>>(2),sempai.calls.load(std::sync::atomic::Ordering::SeqCst),client.query("SELECT operation,left(output_bytes,160) FROM brassclaw_monty_review_operations ORDER BY recorded_at",&[]).await.unwrap()
                            .iter().map(|r|(r.get::<_,String>(0),r.get::<_,Option<String>>(1))).collect::<Vec<_>>());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            loop {if client.query_one("SELECT count(*) FROM brassclaw_monty_review_settlements",&[]).await.unwrap().get::<_,i64>(0)==1 {break;}
                tokio::time::sleep(Duration::from_millis(25)).await;}
        }).await;
        if let Err(error) = waiting {
            panic!(
                "actual consumer settles: {error:?}, work {:?}, operations {:?}",
                client
                    .query("SELECT phase FROM brassclaw_monty_review_work", &[])
                    .await
                    .unwrap()
                    .iter()
                    .map(|r| r.get::<_, String>(0))
                    .collect::<Vec<_>>(),
                client
                    .query(
                        "SELECT operation,left(output_bytes,160) FROM brassclaw_monty_review_operations",
                        &[]
                    )
                    .await
                    .unwrap()
                    .iter()
                    .map(|r| (r.get::<_, String>(0), r.get::<_, Option<String>>(1)))
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(sempai.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            replacement.calls.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT model_dispatch_count FROM brassclaw_monty_review_work",
                    &[]
                )
                .await
                .unwrap()
                .get::<_, i32>(0),
            1
        );
        assert_eq!(client.query_one("SELECT count(*) FROM brassclaw_monty_review_operations WHERE output_bytes IS NOT NULL", &[]).await.unwrap().get::<_,i64>(0), 7);
        assert_eq!(kohai.call_count(), 1);
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            identity
        );
        assert_eq!(
            client
                .query_one("SELECT count(*) FROM brassclaw_monty_review_events", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        let receipt: String = client
            .query_one("SELECT receipt_bytes FROM brassclaw_monty_review_work", &[])
            .await
            .unwrap()
            .get(0);
        let receipt: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        assert_eq!(receipt["status"], "submitted_unreviewed");
        assert_eq!(
            receipt["submission_receipts"]["submissions"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(receipt["catalogue_activated"], false);
        let history = runtime
            .thread_service
            .list_thread_history(brassclaw_threads::ThreadHistoryRequest {
                scope: runtime.thread_scope.clone(),
                thread_id: conversation.0.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            history
                .messages
                .iter()
                .filter(|m| m.kind == brassclaw_threads::MessageKind::Assistant)
                .count(),
            1
        );
        review.request_shutdown();
        review.join().await.unwrap();
        drop(client);
        shutdown_shared_runtime(runtime).await.unwrap();
    }

    const RUNTIME_SEND_TIMEOUT: Duration = Duration::from_secs(10);

    #[cfg(feature = "postgres")]
    #[tokio::test]
    async fn native_startup_monty_settings_read_failure_never_disables_limits() {
        let database = super::test_pg::native_pg::NativePostgres::start().await;
        let booted =
            crate::booted_db::run_migrations_and_return_booted_db(Arc::clone(&database.pool))
                .await
                .unwrap();
        let pool = Arc::clone(booted.pool());
        let defaults = super::read_startup_monty_settings(&pool).await.unwrap();
        assert_eq!(defaults.max_duration_secs, 600);
        assert!(!defaults.token_budgets_enabled);
        // Only this test's private, real PostgreSQL instance is modified.
        let client = pool.get().await.unwrap();
        client
            .execute("DROP TABLE reborn_monty_vm_settings", &[])
            .await
            .unwrap();
        drop(client);
        let error = super::read_startup_monty_settings(&pool).await.unwrap_err();
        assert!(
            matches!(error, super::RebornRuntimeError::InvalidArgument { reason }
            if reason.contains("cannot load Monty VM settings at startup"))
        );
        pool.close();
        drop(pool);
        drop(booted);
        drop(database);
    }

    #[cfg(all(feature = "skills-db", feature = "test-support"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_webui_reconcile_interval_preserves_wake_and_durable_uptake() {
        use axum::{
            body::Body,
            http::{Method, Request, StatusCode},
        };
        use brassclaw_product_workflow::{MontyBudgetUptake, MontyVmSettingsStore};
        use tower::ServiceExt;
        struct Operator;
        #[async_trait]
        impl crate::WebuiAuthenticator for Operator {
            async fn authenticate(&self, token: &str) -> Option<UserId> {
                (token == "settings-owner").then(|| UserId::new(token).unwrap())
            }
            fn allows_operator_webui_config(&self) -> bool {
                true
            }
        }
        async fn save(app: &axum::Router, body: serde_json::Value) -> axum::response::Response {
            tokio::time::timeout(
                Duration::from_secs(10),
                app.clone().oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/api/settings/monty-vm")
                        .header("Authorization", "Bearer settings-owner")
                        .header("Content-Type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                ),
            )
            .await
            .expect("operator wake must not wait for the thirty-second idle timer")
            .unwrap()
        }
        let rig = super::test_pg::pg_rig().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let root = tempfile::tempdir().unwrap();
        let gateway = Arc::new(crate::test_support::BudgetTestGateway::new());
        let input = RebornRuntimeInput::from_services(
            rig.build_input("reconcile-interval-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_model_gateway_override(gateway.clone());
        let runtime = Arc::new(build_reborn_runtime(input).await.unwrap());
        let identity = runtime.global_monty_owner.client().root_identity();
        let settings = runtime.webui_monty_settings_store();
        let before = settings.get("default", "default").await.unwrap();
        assert_eq!(
            before.execution_limits.settings_reconcile_interval_millis,
            1000
        );
        let bundle = build_webui_services(runtime.clone(), None).await.unwrap();
        let app = crate::webui_v2_app(
            bundle,
            crate::WebuiServeConfig::new(
                runtime.thread_scope.tenant_id.clone(),
                Arc::new(Operator),
                Vec::new(),
            ),
        )
        .unwrap();
        let mut limits = before.execution_limits;
        limits.settings_reconcile_interval_millis = 100;
        limits.status_poll_interval_millis = 11;
        let mut invalid_interval = limits;
        invalid_interval.status_poll_interval_millis = i32::MAX as u32 + 1;
        assert_eq!(
            save(
                &app,
                serde_json::json!({"expected_revision":before.revision,
                "execution_limits":invalid_interval})
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            settings.get("default", "default").await.unwrap().revision,
            before.revision
        );
        assert_eq!(
            save(
                &app,
                serde_json::json!({"expected_revision":before.revision,
                "execution_limits":limits})
            )
            .await
            .status(),
            StatusCode::OK
        );
        let fast = settings.get("default", "default").await.unwrap();
        let status = settings.runtime_observation(&fast).unwrap();
        assert_eq!(status.execution_limits.unwrap().limits, limits);
        let desired = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            rig.pool.clone(),
            "default",
            "default",
        );
        let update = serde_json::from_value(serde_json::json!({
            "expected_revision":fast.revision,"max_duration_secs":733,
        }))
        .unwrap();
        // This is the real durable writer, with no Notify connection to the
        // controller. Only periodic reconciliation can discover the successor.
        let external = desired.upsert("default", "default", &update).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if runtime
                    .global_monty_owner
                    .client()
                    .live_task_settings()
                    .current()
                    .revision
                    == external.revision
                {
                    let status = settings.runtime_observation(&external).unwrap();
                    if status.execution_limits.unwrap().uptake == MontyBudgetUptake::Applied {
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("real periodic settings uptake");
        limits.settings_reconcile_interval_millis = 30_000;
        assert_eq!(
            save(
                &app,
                serde_json::json!({"expected_revision":external.revision,
                "execution_limits":limits})
            )
            .await
            .status(),
            StatusCode::OK
        );
        let slow = settings.get("default", "default").await.unwrap();
        assert_eq!(
            settings
                .runtime_observation(&slow)
                .unwrap()
                .execution_limits
                .unwrap()
                .limits,
            limits
        );
        assert_eq!(
            save(
                &app,
                serde_json::json!({"expected_revision":slow.revision,
                "max_duration_secs":755})
            )
            .await
            .status(),
            StatusCode::OK
        );
        let changed = settings.get("default", "default").await.unwrap();
        let status = settings.runtime_observation(&changed).unwrap();
        let execution = status.execution_limits.unwrap();
        assert_eq!(execution.effective_revision, changed.revision);
        assert_eq!(execution.uptake, MontyBudgetUptake::Applied);
        assert_eq!(execution.limits, limits);
        assert_eq!(status.task_budget.unwrap().max_duration_secs, 755);
        assert_eq!(status.memory_budget.unwrap().memory_sample_count, 0);
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            identity
        );
        assert_eq!(gateway.call_count(), 0);
        drop((app, settings));
        shutdown_shared_runtime(runtime).await.unwrap();
    }

    #[cfg(all(feature = "skills-db", feature = "test-support"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_webui_cancellation_deadline_keeps_accepted_revision_and_evidence() {
        use crate::global_monty_driver::GlobalTaskPortsFactory;
        use axum::{
            body::Body,
            http::{Method, Request, StatusCode},
        };
        use brassclaw_turns::run_profile::{AgentLoopDriverError, MontyTurnDriverPort};
        use tower::ServiceExt;
        struct Operator;
        #[async_trait]
        impl crate::WebuiAuthenticator for Operator {
            async fn authenticate(&self, token: &str) -> Option<UserId> {
                (token == "settings-owner").then(|| UserId::new(token).unwrap())
            }
            fn allows_operator_webui_config(&self) -> bool {
                true
            }
        }
        struct HeldProvider {
            run: StdMutex<Option<brassclaw_turns::TurnRunId>>,
            entered: tokio::sync::Notify,
            release: tokio::sync::Semaphore,
        }
        #[async_trait]
        impl HostManagedModelGateway for HeldProvider {
            async fn stream_model(
                &self,
                request: HostManagedModelRequest,
            ) -> Result<HostManagedModelResponse, HostManagedModelError> {
                *self.run.lock().unwrap() = Some(request.run_id);
                self.entered.notify_one();
                self.release.acquire().await.unwrap().forget();
                Ok(HostManagedModelResponse::assistant_reply(
                    "late cancelled provider answer",
                ))
            }
        }
        let rig = super::test_pg::pg_rig().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let root = tempfile::tempdir().unwrap();
        let provider = Arc::new(HeldProvider {
            run: StdMutex::new(None),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Semaphore::new(0),
        });
        let input = RebornRuntimeInput::from_services(
            rig.build_input("cancellation-budget-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_model_gateway_override(provider.clone());
        let runtime = Arc::new(build_reborn_runtime(input).await.unwrap());
        let identity = runtime.global_monty_owner.client().root_identity();
        let settings = runtime.webui_monty_settings_store();
        let before = settings.get("default", "default").await.unwrap();
        let driver = runtime.monty_test_controls.0.clone();
        let factory = runtime.monty_test_controls.1.clone();
        let acknowledged = factory.cancellation_acknowledgement().unwrap();
        assert_eq!(acknowledged.revision, before.revision);
        assert_eq!(acknowledged.timeout, Duration::from_secs(5));
        let bundle = build_webui_services(runtime.clone(), None).await.unwrap();
        let app = crate::webui_v2_app(
            bundle,
            crate::WebuiServeConfig::new(
                runtime.thread_scope.tenant_id.clone(),
                Arc::new(Operator),
                Vec::new(),
            ),
        )
        .unwrap();
        let conversation = runtime.new_conversation().await.unwrap();
        let sending = {
            let runtime = runtime.clone();
            tokio::spawn(async move {
                runtime
                    .send_user_message(
                        &conversation,
                        "cancellation deadline native acceptance canary",
                    )
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(10), provider.entered.notified())
            .await
            .unwrap();
        let run = provider.run.lock().unwrap().unwrap();
        let host = driver.retained_host_for_run(run).unwrap().unwrap();
        let attempt = host.attempt();
        let started = std::time::Instant::now();
        let mut stopping = Box::pin(driver.stop_attempt(attempt));
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
            result = &mut stopping => panic!("held provider must retain settlement: {result:?}"),
        }
        let mut successor = before.execution_limits;
        successor.cancellation_ack_timeout_millis = 250;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/settings/monty-vm")
                    .header("Authorization", "Bearer settings-owner")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "expected_revision":before.revision,"execution_limits":successor
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let policy = factory.cancellation_acknowledgement().unwrap();
        assert_eq!(policy.revision, before.revision + 1);
        assert_eq!(policy.timeout, Duration::from_millis(250));
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            identity
        );
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(500)) => {},
            result = &mut stopping => panic!("successor must not shorten accepted wait: {result:?}"),
        }
        assert!(matches!(
            stopping.await,
            Err(AgentLoopDriverError::Unavailable { .. })
        ));
        assert!(started.elapsed() >= acknowledged.timeout);
        assert!(driver.take_settlement(attempt).is_err());
        assert!(
            runtime
                .global_monty_owner
                .client()
                .retained_attempts()
                .retained
                > 0
        );
        let client = rig.pool.get().await.unwrap();
        let phase: String = client
            .query_one(
                "SELECT phase FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&run.as_uuid()],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(phase, "started");
        drop(client);
        provider.release.add_permits(1);
        let result = tokio::time::timeout(Duration::from_secs(10), sending)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_ne!(result.status, TurnStatus::Completed);
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match driver.stop_attempt(attempt).await {
                    Ok(()) => break,
                    Err(AgentLoopDriverError::Unavailable { .. }) => tokio::task::yield_now().await,
                    Err(error) => panic!("actual late settlement failed: {error:?}"),
                }
            }
        })
        .await
        .unwrap();
        let (_, receipt, _, driver_credit) = driver.take_settlement(attempt).unwrap().unwrap();
        let (_, admission, ports, factory_receipt, factory_credit) =
            factory.take_failed_settlement(attempt).unwrap().unwrap();
        assert!(Arc::ptr_eq(&receipt, &factory_receipt));
        assert!(Arc::ptr_eq(&driver_credit, &factory_credit));
        assert!(host.finalized_reply_ref().is_none());
        assert!(admission.check_and_start().await.is_err());
        drop((
            ports,
            admission,
            receipt,
            factory_receipt,
            driver_credit,
            factory_credit,
        ));
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            identity
        );
        drop((app, driver, factory));
        shutdown_shared_runtime(runtime).await.unwrap();
    }

    #[cfg(all(feature = "skills-db", feature = "test-support"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_webui_incomplete_execution_replacement_preserves_live_limits() {
        use axum::{
            body::Body,
            http::{Method, Request, StatusCode},
        };
        use brassclaw_product_workflow::MontyVmSettingsStore;
        use tower::ServiceExt;
        struct Operator;
        #[async_trait]
        impl crate::WebuiAuthenticator for Operator {
            async fn authenticate(&self, token: &str) -> Option<UserId> {
                (token == "settings-owner").then(|| UserId::new(token).unwrap())
            }
            fn allows_operator_webui_config(&self) -> bool {
                true
            }
        }
        let rig = super::test_pg::pg_rig().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let desired_store = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            rig.pool.clone(),
            "default",
            "default",
        );
        let before = desired_store.get("default", "default").await.unwrap();
        let mut custom = before.execution_limits;
        custom.max_pending_ownership_checks = 17;
        let update = serde_json::from_value(serde_json::json!({
            "expected_revision":before.revision,"execution_limits":custom
        }))
        .unwrap();
        let desired = desired_store
            .upsert("default", "default", &update)
            .await
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        let gateway = Arc::new(crate::test_support::BudgetTestGateway::new());
        let input = RebornRuntimeInput::from_services(
            rig.build_input("replacement-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_model_gateway_override(gateway.clone());
        let runtime = Arc::new(build_reborn_runtime(input).await.unwrap());
        let identity = runtime.global_monty_owner.client().root_identity();
        let bundle = build_webui_services(runtime.clone(), None).await.unwrap();
        let app = crate::webui_v2_app(
            bundle,
            crate::WebuiServeConfig::new(
                runtime.thread_scope.tenant_id.clone(),
                Arc::new(Operator),
                Vec::new(),
            ),
        )
        .unwrap();
        let complete = serde_json::to_value(custom).unwrap();
        let mut incomplete = complete.clone();
        incomplete
            .as_object_mut()
            .unwrap()
            .remove("max_pending_ownership_checks");
        let mut unknown = complete.clone();
        unknown["unsupported_limit"] = serde_json::json!(1);
        let mut null_field = complete.clone();
        null_field["ownership_check_timeout_millis"] = serde_json::Value::Null;
        let duplicate = format!(
            "{{\"expected_revision\":{},\"execution_limits\":{{\"max_pending_ownership_checks\":19,{}}}}}",
            desired.revision,
            serde_json::to_string(&complete)
                .unwrap()
                .trim_start_matches('{')
                .trim_end_matches('}')
        );
        for (case, body) in [
            serde_json::json!({"expected_revision":desired.revision,"execution_limits":incomplete})
                .to_string(),
            serde_json::json!({"expected_revision":desired.revision,"execution_limits":unknown})
                .to_string(),
            serde_json::json!({"expected_revision":desired.revision,"execution_limits":null_field})
                .to_string(),
            duplicate,
        ]
        .into_iter()
        .enumerate()
        {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/api/settings/monty-vm")
                        .header("Authorization", "Bearer settings-owner")
                        .header("Content-Type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "invalid replacement case {case}"
            );
            let stored = desired_store.get("default", "default").await.unwrap();
            assert_eq!(
                serde_json::to_value(stored).unwrap(),
                serde_json::to_value(&desired).unwrap()
            );
            let live = runtime
                .global_monty_owner
                .client()
                .live_task_settings()
                .current();
            assert_eq!(live.revision, desired.revision);
            assert_eq!(
                runtime
                    .global_monty_owner
                    .ownership_check()
                    .snapshot()
                    .unwrap()
                    .limits
                    .max_pending_checks,
                17
            );
            assert_eq!(
                runtime.global_monty_owner.client().root_identity(),
                identity
            );
        }
        // Omitting the entire replacement retains normal top-level patch
        // semantics. A valid duration edit still reaches the same global VM.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::PUT)
                    .uri("/api/settings/monty-vm")
                    .header("Authorization", "Bearer settings-owner")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "expected_revision":desired.revision,"max_duration_secs":777
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let updated = desired_store.get("default", "default").await.unwrap();
        assert_eq!(updated.execution_limits, custom);
        assert_eq!(updated.revision, desired.revision + 1);
        let live = runtime
            .global_monty_owner
            .client()
            .live_task_settings()
            .current();
        assert_eq!(live.revision, updated.revision);
        assert_eq!(live.limits.max_compute_time.as_secs(), 777);
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            identity
        );
        drop(app);
        shutdown_shared_runtime(runtime).await.unwrap();
        assert_eq!(gateway.call_count(), 0);
    }

    #[cfg(all(feature = "skills-db", feature = "test-support"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_global_owner_heartbeat_fences_real_connection_loss() {
        use crate::monty_instance_owner::{OwnershipError, PgMontyOwner};
        use brassclaw_product_workflow::MontyVmSettingsStore;
        let rig = crate::runtime::test_pg::pg_rig().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let settings = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            rig.pool.clone(),
            "default",
            "default",
        );
        let before = settings.get("default", "default").await.unwrap();
        let mut limits = before.execution_limits;
        limits.ownership_heartbeat_interval_millis = 200;
        let update = serde_json::from_value(
            serde_json::json!({"expected_revision":before.revision,"execution_limits":limits}),
        )
        .unwrap();
        settings
            .upsert("default", "default", &update)
            .await
            .unwrap();
        let home = tempfile::tempdir().unwrap();
        let gateway = Arc::new(crate::test_support::BudgetTestGateway::new());
        let input = crate::RebornRuntimeInput::from_services(
            rig.build_input("heartbeat-owner", home.path())
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_model_gateway_override(gateway.clone());
        let runtime = crate::build_reborn_runtime(input).await.unwrap();
        let owner = runtime.global_monty_owner.ownership_check();
        owner.check().await.unwrap();
        let client = rig.pool.get().await.unwrap();
        // Kill the actual detached PostgreSQL session holding the two-int
        // instance owner lock, not a fake channel or the worker process.
        let rows = client
            .query(
                "SELECT pid FROM pg_locks WHERE locktype='advisory'
            AND classid::bigint=$1 AND objid::bigint=$2 AND objsubid=2
            AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
            AND mode='ExclusiveLock' AND granted",
                &[&i64::from(0x4252_434c), &i64::from(0x4d4f_4e54)],
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let pid: i32 = rows[0].get(0);
        assert!(
            client
                .query_one("SELECT pg_terminate_backend($1)", &[&pid])
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        drop(client);
        tokio::time::timeout(Duration::from_secs(5), async {
            while !owner.is_closed() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("real heartbeat fences the lost owner");
        assert!(matches!(owner.check().await, Err(OwnershipError::Lost)));
        assert!(
            matches!(runtime.shutdown().await,
                Err(super::RebornRuntimeError::InvalidArgument { reason })
                if reason == "global Monty shutdown requires reconciliation"),
            "ownership loss must remain a reported shutdown failure"
        );
        let replacement = PgMontyOwner::acquire(&rig.pool).await.unwrap();
        replacement.check().await.unwrap();
        replacement.release().await.unwrap();
        assert_eq!(gateway.call_count(), 0);
    }

    async fn shutdown_shared_runtime(
        runtime: Arc<super::RebornRuntime>,
    ) -> Result<(), super::RebornRuntimeError> {
        let runtime = match Arc::try_unwrap(runtime) {
            Ok(runtime) => runtime,
            Err(_) => panic!("runtime still has outstanding owners during test shutdown"),
        };
        runtime.shutdown().await
    }

    fn local_dev_runtime_policy() -> EffectiveRuntimePolicy {
        EffectiveRuntimePolicy {
            deployment: DeploymentMode::LocalSingleUser,
            requested_profile: RuntimeProfile::LocalDev,
            resolved_profile: RuntimeProfile::LocalDev,
            filesystem_backend: FilesystemBackendKind::HostWorkspace,
            process_backend: ProcessBackendKind::LocalHost,
            network_mode: NetworkMode::DirectLogged,
            secret_mode: SecretMode::ScrubbedEnv,
            approval_policy: ApprovalPolicy::AskDestructive,
            audit_mode: AuditMode::LocalMinimal,
        }
    }

    #[derive(Debug)]
    struct RecordingGateway {
        reply: String,
        requests: Arc<StdMutex<Vec<HostManagedModelRequest>>>,
    }

    #[async_trait]
    impl HostManagedModelGateway for RecordingGateway {
        async fn stream_model(
            &self,
            request: HostManagedModelRequest,
        ) -> Result<HostManagedModelResponse, HostManagedModelError> {
            self.requests
                .lock()
                .expect("recording gateway requests lock poisoned")
                .push(request);
            Ok(HostManagedModelResponse::assistant_reply(
                self.reply.clone(),
            ))
        }
    }

    #[derive(Debug, Default)]
    struct ToolCallingGateway {
        calls: StdMutex<usize>,
        stream_model_calls: StdMutex<usize>,
        requests: StdMutex<Vec<HostManagedModelRequest>>,
    }

    #[async_trait]
    impl HostManagedModelGateway for ToolCallingGateway {
        async fn stream_model(
            &self,
            request: HostManagedModelRequest,
        ) -> Result<HostManagedModelResponse, HostManagedModelError> {
            *self
                .stream_model_calls
                .lock()
                .expect("tool gateway stream count lock poisoned") += 1;
            self.requests
                .lock()
                .expect("tool gateway requests lock poisoned")
                .push(request);
            Err(HostManagedModelError::safe(
                HostManagedModelErrorKind::InvalidRequest,
                "expected capability-aware model path",
            ))
        }

        async fn stream_model_with_capabilities(
            &self,
            request: HostManagedModelRequest,
            capabilities: Arc<dyn LoopCapabilityPort>,
        ) -> Result<HostManagedModelResponse, HostManagedModelError> {
            let call_index = {
                let mut calls = self.calls.lock().expect("tool gateway lock poisoned");
                let call_index = *calls;
                *calls += 1;
                call_index
            };
            self.requests
                .lock()
                .expect("tool gateway requests lock poisoned")
                .push(request.clone());
            if call_index > 0 {
                let tool_result = request
                    .messages
                    .iter()
                    .find(|message| message.role == HostManagedModelMessageRole::ToolResult)
                    .expect("second model call should include tool result");
                assert!(
                    tool_result.content.contains("hello from tool"),
                    "tool result should expose hydrated capability output, got {}",
                    tool_result.content
                );
                let provider_call = tool_result
                    .tool_result_provider_call
                    .as_ref()
                    .expect("provider replay metadata");
                assert_eq!(provider_call.provider_call_id, "call-1");
                assert_eq!(
                    provider_call.capability_id,
                    CapabilityId::new("builtin.echo").unwrap()
                );
                return Ok(HostManagedModelResponse::assistant_reply("tool ok"));
            }

            let surface = capabilities
                .visible_capabilities(VisibleCapabilityRequest {})
                .await
                .map_err(model_capability_error)?;
            let echo_id = CapabilityId::new("builtin.echo").expect("echo id");
            assert!(
                surface
                    .descriptors
                    .iter()
                    .any(|descriptor| descriptor.capability_id == echo_id),
                "builtin echo must be visible through local-dev runtime capability surface"
            );
            let echo_tool = capabilities
                .tool_definitions()
                .map_err(model_capability_error)?
                .into_iter()
                .find(|definition| definition.capability_id == echo_id)
                .expect("echo provider tool definition");
            let candidate = capabilities
                .register_provider_tool_call(ProviderToolCall {
                    provider_id: "test-provider".to_string(),
                    provider_model_id: "test-model".to_string(),
                    turn_id: Some("provider-turn-1".to_string()),
                    id: "call-1".to_string(),
                    name: echo_tool.name,
                    arguments: serde_json::json!({"message": "hello from tool"}),
                    response_reasoning: None,
                    reasoning: None,
                    signature: None,
                })
                .await
                .map_err(model_capability_error)?;
            Ok(HostManagedModelResponse::capability_calls(
                vec![candidate],
                "",
            ))
        }
    }

    #[derive(Debug, Default)]
    struct WorkspaceListingGateway {
        calls: StdMutex<usize>,
        requests: StdMutex<Vec<HostManagedModelRequest>>,
    }

    #[async_trait]
    impl HostManagedModelGateway for WorkspaceListingGateway {
        async fn stream_model(
            &self,
            request: HostManagedModelRequest,
        ) -> Result<HostManagedModelResponse, HostManagedModelError> {
            self.requests
                .lock()
                .expect("workspace gateway requests lock poisoned")
                .push(request);
            Err(HostManagedModelError::safe(
                HostManagedModelErrorKind::InvalidRequest,
                "expected capability-aware model path",
            ))
        }

        async fn stream_model_with_capabilities(
            &self,
            request: HostManagedModelRequest,
            capabilities: Arc<dyn LoopCapabilityPort>,
        ) -> Result<HostManagedModelResponse, HostManagedModelError> {
            let call_index = {
                let mut calls = self.calls.lock().expect("workspace gateway lock poisoned");
                let call_index = *calls;
                *calls += 1;
                call_index
            };
            self.requests
                .lock()
                .expect("workspace gateway requests lock poisoned")
                .push(request.clone());
            if call_index > 0 {
                let tool_result = request
                    .messages
                    .iter()
                    .find(|message| message.role == HostManagedModelMessageRole::ToolResult)
                    .expect("second model call should include tool result");
                assert!(
                    tool_result.content.contains("workspace-sentinel.txt"),
                    "workspace listing should expose configured workspace root, got {}",
                    tool_result.content
                );
                return Ok(HostManagedModelResponse::assistant_reply("workspace ok"));
            }

            let list_dir_id = CapabilityId::new("builtin.list_dir").expect("list_dir id");
            let list_dir_tool = capabilities
                .tool_definitions()
                .map_err(model_capability_error)?
                .into_iter()
                .find(|definition| definition.capability_id == list_dir_id)
                .expect("list_dir provider tool definition");
            let candidate = capabilities
                .register_provider_tool_call(ProviderToolCall {
                    provider_id: "test-provider".to_string(),
                    provider_model_id: "test-model".to_string(),
                    turn_id: Some("provider-turn-1".to_string()),
                    id: "call-1".to_string(),
                    name: list_dir_tool.name,
                    arguments: serde_json::json!({"path": "/workspace"}),
                    response_reasoning: None,
                    reasoning: None,
                    signature: None,
                })
                .await
                .map_err(model_capability_error)?;
            Ok(HostManagedModelResponse::capability_calls(
                vec![candidate],
                "",
            ))
        }
    }

    #[derive(Debug)]
    struct AllowingTriggerFireAccessChecker;

    #[async_trait]
    impl TriggerFireAccessChecker for AllowingTriggerFireAccessChecker {
        async fn check_trigger_fire_access(
            &self,
            _request: TriggerFireAccessCheck,
        ) -> Result<TriggerFireAccessDecision, TriggerFireAccessError> {
            Ok(TriggerFireAccessDecision::Allowed)
        }
    }

    fn model_capability_error(error: impl std::fmt::Display) -> HostManagedModelError {
        let safe_summary = error.to_string();
        HostManagedModelError::safe(HostManagedModelErrorKind::Unavailable, safe_summary)
    }

    #[cfg(feature = "root-llm-provider")]
    struct RuntimeEnvGuard {
        name: &'static str,
        previous: Option<String>,
    }

    #[cfg(feature = "root-llm-provider")]
    impl RuntimeEnvGuard {
        fn set(name: &'static str, value: &str) -> Self {
            let previous = brassclaw_common::env_helpers::env_or_override(name);
            brassclaw_common::env_helpers::set_runtime_env(name, value);
            Self { name, previous }
        }
    }

    #[cfg(feature = "root-llm-provider")]
    impl Drop for RuntimeEnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => brassclaw_common::env_helpers::set_runtime_env(self.name, value),
                None => brassclaw_common::env_helpers::remove_runtime_env(self.name),
            }
        }
    }

    #[cfg(feature = "root-llm-provider")]
    async fn start_nearai_auth_capture_server() -> (String, tokio::sync::oneshot::Receiver<String>)
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpSocket;

        let socket = TcpSocket::new_v4().expect("test server socket");
        socket
            .bind("127.0.0.1:0".parse().expect("test server address"))
            .expect("test server binds");
        let listener = socket.listen(1024).expect("test server listens");
        let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
        let (auth_tx, auth_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let mut auth_tx = Some(auth_tx);
            loop {
                let (mut stream, _) = listener.accept().await.expect("accept test request");
                let mut buffer = Vec::new();
                loop {
                    let mut chunk = [0u8; 1024];
                    let read = stream.read(&mut chunk).await.expect("read test request");
                    if read == 0 {
                        break;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                    if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }

                let request = String::from_utf8_lossy(&buffer);
                let request_line = request.lines().next().unwrap_or_default();
                let auth_header = request
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                    .map(|(_, value)| value.trim())
                    .unwrap_or_default()
                    .to_string();
                let is_chat_completion = request_line.contains("/v1/chat/completions");
                let body = if is_chat_completion {
                    r#"{"choices":[{"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#
                } else {
                    r#"{"data":[]}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write test response");

                if is_chat_completion {
                    if let Some(auth_tx) = auth_tx.take() {
                        let _ = auth_tx.send(auth_header);
                    }
                    break;
                }
            }
        });

        (base_url, auth_rx)
    }

    #[cfg(feature = "root-llm-provider")]
    fn nearai_gateway_test_request() -> HostManagedModelRequest {
        HostManagedModelRequest {
            model_profile_id: brassclaw_turns::run_profile::ModelProfileId::new(
                "interactive_model",
            )
            .expect("model profile id"),
            messages: vec![brassclaw_loop_support::HostManagedModelMessage {
                role: HostManagedModelMessageRole::User,
                content: "hello model".to_string(),
                content_ref: brassclaw_turns::LoopMessageRef::new(
                    "msg:22222222-2222-2222-2222-222222222222",
                )
                .expect("message ref"),
                tool_result_provider_call: None,
                tool_result_content: None,
            }],
            surface_version: None,
            resolved_model_route: None,
            run_id: TurnRunId::new(),
            turn_id: TurnId::new(),
        }
    }

    fn recorded_request_count(requests: &StdMutex<Vec<HostManagedModelRequest>>) -> usize {
        requests
            .lock()
            .expect("recording gateway requests lock poisoned")
            .len()
    }

    #[cfg(feature = "root-llm-provider")]
    #[tokio::test]
    async fn root_llm_gateway_bootstraps_nearai_session_token_from_env() {
        let _token_guard = RuntimeEnvGuard::set("NEARAI_SESSION_TOKEN", "sess_reborn_env_token");
        let session_dir = tempfile::tempdir().expect("session tempdir");
        let (base_url, auth_rx) = start_nearai_auth_capture_server().await;

        let config = brassclaw_llm::LlmConfig {
            backend: "nearai".to_string(),
            session: brassclaw_llm::SessionConfig {
                auth_base_url: base_url.clone(),
                session_path: session_dir.path().join("session.json"),
            },
            nearai: brassclaw_llm::NearAiConfig {
                model: "test-model".to_string(),
                cheap_model: None,
                base_url,
                api_key: None,
                fallback_model: None,
                max_retries: 0,
                circuit_breaker_threshold: None,
                circuit_breaker_recovery_secs: 30,
                response_cache_enabled: false,
                response_cache_ttl_secs: 3600,
                response_cache_max_entries: 1000,
                failover_cooldown_secs: 300,
                failover_cooldown_threshold: 3,
                smart_routing_cascade: false,
            },
            provider: None,
            bedrock: None,
            gemini_oauth: None,
            openai_codex: None,
            request_timeout_secs: 5,
            cheap_model: None,
            smart_routing_cascade: false,
            max_retries: 0,
            circuit_breaker_threshold: None,
            circuit_breaker_recovery_secs: 30,
            response_cache_enabled: false,
            response_cache_ttl_secs: 3600,
            response_cache_max_entries: 1000,
        };
        let llm = crate::runtime_input::ResolvedRebornLlm::from_llm_config(config);

        let bundle = super::build_llm_gateway(llm, None, None, None)
            .await
            .expect("gateway builds");
        let response = bundle
            .gateway
            .stream_model(nearai_gateway_test_request())
            .await
            .expect("gateway calls NEAR AI provider");

        assert_eq!(response.safe_text_deltas, vec!["ok".to_string()]);
        let auth_header = tokio::time::timeout(Duration::from_secs(2), auth_rx)
            .await
            .expect("chat request should be captured")
            .expect("auth header should be sent by capture server");
        assert_eq!(auth_header, "Bearer sess_reborn_env_token");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_yolo_records_trusted_laptop_access_audit_event() {
        let root = tempfile::tempdir().expect("tempdir");
        let host_home = root.path().join("host-home");
        std::fs::create_dir_all(&host_home).expect("host home");
        let mut policy = local_dev_runtime_policy();
        policy.requested_profile = RuntimeProfile::LocalYolo;
        policy.resolved_profile = RuntimeProfile::LocalYolo;
        policy.filesystem_backend = FilesystemBackendKind::HostWorkspaceAndHome;
        policy.network_mode = NetworkMode::Direct;
        policy.secret_mode = SecretMode::InheritedEnv;

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-yolo-audit-owner", root.path())
                .with_runtime_policy(policy)
                .with_local_dev_confirmed_host_home_root(host_home),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-yolo-audit-tenant".to_string(),
            agent_id: "runtime-yolo-audit-agent".to_string(),
            source_binding_id: "runtime-yolo-audit-source".to_string(),
            reply_target_binding_id: "runtime-yolo-audit-reply".to_string(),
        });

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let stream = EventStreamKey::new(
            runtime.thread_scope.tenant_id.clone(),
            runtime.actor_user_id.clone(),
            Some(runtime.thread_scope.agent_id.clone()),
        );
        let audit_log = brassclaw_reborn_event_store::PgDurableAuditLog::new(
            Arc::clone(&rig.pool),
            runtime.thread_scope.tenant_id.as_str(),
        );
        let replay = audit_log
            .read_after_cursor(&stream, &ReadScope::any(), None, 10)
            .await
            .expect("audit replay");

        let audit = replay
            .entries
            .iter()
            .map(|entry| &entry.record)
            .find(|record| record.action.kind == TRUSTED_LAPTOP_ACCESS_AUDIT_KIND)
            .expect("trusted laptop access audit event");
        assert_eq!(audit.stage, AuditStage::After);
        assert_eq!(
            audit.action.target.as_deref(),
            Some(TRUSTED_LAPTOP_ACCESS_AUDIT_TARGET)
        );
        assert_eq!(
            audit
                .result
                .as_ref()
                .and_then(|result| result.status.as_deref()),
            Some(TRUSTED_LAPTOP_ACCESS_AUDIT_STATUS)
        );
        assert_eq!(audit.decision.kind, "allowed");
        shutdown_shared_runtime(runtime).await.expect("shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_readiness_reports_trigger_poller_worker() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "trigger readiness".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-trigger-readiness-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-trigger-readiness-tenant".to_string(),
            agent_id: "runtime-trigger-readiness-agent".to_string(),
            source_binding_id: "runtime-trigger-readiness-source".to_string(),
            reply_target_binding_id: "runtime-trigger-readiness-reply".to_string(),
        })
        .with_trigger_poller_settings(
            TriggerPollerSettings::enabled_with_tenant_scoped_authorizer_for_test(),
        )
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));

        assert!(runtime.services().readiness.workers.turn_runner);
        assert!(runtime.services().readiness.workers.trigger_poller);

        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_rejects_trigger_poller_without_creator_authorization() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "trigger auth required".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-trigger-auth-required-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-trigger-auth-required-tenant".to_string(),
            agent_id: "runtime-trigger-auth-required-agent".to_string(),
            source_binding_id: "runtime-trigger-auth-required-source".to_string(),
            reply_target_binding_id: "runtime-trigger-auth-required-reply".to_string(),
        })
        .with_trigger_poller_settings(TriggerPollerSettings::enabled())
        .with_model_gateway_override(gateway);

        let err = match build_reborn_runtime(input).await {
            Ok(runtime) => {
                runtime
                    .shutdown()
                    .await
                    .expect("unexpected runtime shutdown");
                panic!(
                    "creator-access-required setting must not enable trigger poller without an access checker"
                );
            }
            Err(err) => err,
        };

        assert!(
            matches!(err, super::RebornRuntimeError::InvalidArgument { reason } if reason.contains("fire-time creator access checker"))
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_accepts_trigger_poller_with_creator_access_checker() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "trigger auth supplied".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-trigger-auth-supplied-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-trigger-auth-supplied-tenant".to_string(),
            agent_id: "runtime-trigger-auth-supplied-agent".to_string(),
            source_binding_id: "runtime-trigger-auth-supplied-source".to_string(),
            reply_target_binding_id: "runtime-trigger-auth-supplied-reply".to_string(),
        })
        .with_trigger_poller_settings(TriggerPollerSettings::enabled())
        .with_trigger_fire_access_checker(Arc::new(AllowingTriggerFireAccessChecker))
        .with_model_gateway_override(gateway);

        let runtime = build_reborn_runtime(input)
            .await
            .expect("runtime builds with creator access checker");

        assert!(runtime.services().readiness.workers.turn_runner);
        assert!(runtime.services().readiness.workers.trigger_poller);

        runtime.shutdown().await.expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_disables_trigger_poller_worker_by_default() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "trigger disabled".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-trigger-disabled-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-trigger-disabled-tenant".to_string(),
            agent_id: "runtime-trigger-disabled-agent".to_string(),
            source_binding_id: "runtime-trigger-disabled-source".to_string(),
            reply_target_binding_id: "runtime-trigger-disabled-reply".to_string(),
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));

        assert!(runtime.services().readiness.workers.turn_runner);
        assert!(!runtime.services().readiness.workers.trigger_poller);

        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_rejects_invalid_trigger_poller_worker_config() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "trigger invalid config".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let trigger_poller = TriggerPollerSettings {
            enabled: true,
            worker: brassclaw_triggers::TriggerPollerWorkerConfig {
                poll_interval: Duration::ZERO,
                ..Default::default()
            },
            ..Default::default()
        }
        .with_tenant_scoped_authorizer_for_test();

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-trigger-invalid-config-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-trigger-invalid-config-tenant".to_string(),
            agent_id: "runtime-trigger-invalid-config-agent".to_string(),
            source_binding_id: "runtime-trigger-invalid-config-source".to_string(),
            reply_target_binding_id: "runtime-trigger-invalid-config-reply".to_string(),
        })
        .with_trigger_poller_settings(trigger_poller)
        .with_model_gateway_override(gateway);

        let err = match build_reborn_runtime(input).await {
            Ok(runtime) => {
                runtime
                    .shutdown()
                    .await
                    .expect("unexpected runtime shutdown");
                panic!("invalid trigger poller config must fail runtime build");
            }
            Err(err) => err,
        };

        assert!(
            matches!(err, super::RebornRuntimeError::InvalidArgument { reason } if reason.contains("poll_interval must be non-zero"))
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_shutdown_cancels_trigger_poller_worker() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "trigger shutdown".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-trigger-shutdown-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-trigger-shutdown-tenant".to_string(),
            agent_id: "runtime-trigger-shutdown-agent".to_string(),
            source_binding_id: "runtime-trigger-shutdown-source".to_string(),
            reply_target_binding_id: "runtime-trigger-shutdown-reply".to_string(),
        })
        .with_trigger_poller_settings(
            TriggerPollerSettings::enabled_with_tenant_scoped_authorizer_for_test(),
        )
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        assert!(runtime.services().readiness.workers.trigger_poller);

        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            shutdown_shared_runtime(runtime),
        )
        .await
        .expect("shutdown returns before timeout")
        .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_yolo_message_flow_ignores_model_budget_gate() {
        let root = tempfile::tempdir().expect("tempdir");
        let host_home = root.path().join("host-home");
        std::fs::create_dir_all(&host_home).expect("host home");
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let gateway = Arc::new(RecordingGateway {
            reply: "yolo budget bypass reply".to_string(),
            requests: Arc::clone(&requests),
        });
        let cost_table = brassclaw_loop_support::StaticModelCostTable::new().with_entry(
            ModelProfileId::new("interactive_model").expect("model profile id"),
            ModelCost {
                input_per_token: dec!(1.00),
                output_per_token: dec!(1.00),
                max_output_tokens: 8_192,
                cache_write_multiplier_milli: 0,
                cache_read_multiplier_milli: 0,
            },
        );

        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-yolo-budget-owner", root.path())
                .with_runtime_policy(
                    crate::local_dev_yolo_runtime_policy(true).expect("local-yolo policy resolves"),
                )
                .with_local_dev_confirmed_host_home_root(host_home),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-yolo-budget-tenant".to_string(),
            agent_id: "runtime-yolo-budget-agent".to_string(),
            source_binding_id: "runtime-yolo-budget-source".to_string(),
            reply_target_binding_id: "runtime-yolo-budget-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway)
        .with_model_cost_table_override(Arc::new(cost_table));

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let conversation = runtime.new_conversation().await.expect("conversation");
        let reply = tokio::time::timeout(
            RUNTIME_SEND_TIMEOUT,
            runtime.send_user_message(&conversation, "ping"),
        )
        .await
        .expect("runtime send should finish")
        .expect("runtime send should succeed");

        assert_eq!(reply.status, TurnStatus::Completed);
        assert_eq!(reply.text.as_deref(), Some("yolo budget bypass reply"));
        assert_eq!(
            recorded_request_count(&requests),
            1,
            "local-dev-yolo must reach the model gateway even when a paid cost table is present"
        );

        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn send_user_message_returns_completed_assistant_text_with_recording_gateway() {
        let root = tempfile::tempdir().expect("tempdir");
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let gateway = Arc::new(RecordingGateway {
            reply: "recorded runtime reply".to_string(),
            requests: Arc::clone(&requests),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-success-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-success-tenant".to_string(),
            agent_id: "runtime-success-agent".to_string(),
            source_binding_id: "runtime-success-source".to_string(),
            reply_target_binding_id: "runtime-success-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: RUNTIME_SEND_TIMEOUT,
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        assert!(
            Arc::ptr_eq(
                &runtime.turn_coordinator,
                runtime
                    .services
                    .turn_coordinator
                    .as_ref()
                    .expect("RebornServices turn coordinator")
            ),
            "REPL runtime should drive turns through RebornServices"
        );
        let conversation = runtime.new_conversation().await.expect("conversation");
        let reply = tokio::time::timeout(
            RUNTIME_SEND_TIMEOUT,
            runtime.send_user_message(&conversation, "ping"),
        )
        .await
        .expect("runtime send should finish")
        .expect("runtime send should succeed");

        assert_eq!(reply.status, TurnStatus::Completed);
        assert_eq!(reply.text.as_deref(), Some("recorded runtime reply"));
        assert_eq!(recorded_request_count(&requests), 1);

        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[cfg(feature = "skills-db")]
    #[tokio::test]
    async fn kohai_gateway_preserves_admitted_run_and_turn_without_identity_fallback() {
        use brassclaw_engine::executor::kohai_port::{KohaiCallCtx, KohaiPort, KohaiPortError};

        let rig = super::test_pg::pg_rig().await;
        let tenant = "kohai-admitted-identity";
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let gateway = Arc::new(RecordingGateway {
            reply: "identity retained".into(),
            requests: Arc::clone(&requests),
        });
        let interceptor = Arc::new(brassclaw_interceptor::PgInterceptorStore::new(
            Arc::clone(&rig.pool),
            tenant,
        ));
        let prefix = Arc::new(crate::pg_basic_prompt_store::PgBasicPromptStore::new(
            Arc::clone(&rig.pool),
            tenant,
            "test-agent",
        ));
        let port = crate::pg_kohai_port::PgKohaiPort::new(interceptor, prefix, gateway)
            .expect("Kohai port");
        let run_id = brassclaw_turns::TurnRunId::new();
        let turn_id = brassclaw_turns::TurnId::new();
        let context = KohaiCallCtx {
            run_id: run_id.to_string(),
            turn_id: turn_id.to_string(),
            iteration: 0,
            tenant_id: tenant.into(),
            user_id: "test-user".into(),
            project_id: "test-project".into(),
        };
        let prompt = serde_json::json!({"chat_history": [], "user_query": "exact query"});
        for iteration in 0..2 {
            let mut call_context = context.clone();
            call_context.iteration = iteration;
            let answer = port
                .complete(prompt.clone(), call_context)
                .await
                .expect("model call");
            assert_eq!(answer.content, "identity retained");
        }
        {
            let recorded = requests.lock().unwrap();
            assert_eq!(recorded.len(), 2);
            for request in recorded.iter() {
                assert_eq!(request.run_id, run_id);
                assert_eq!(request.turn_id, turn_id);
            }
        }
        for invalid_turn in [false, true] {
            let mut invalid = context.clone();
            if invalid_turn {
                invalid.turn_id = "invalid".into();
            } else {
                invalid.run_id = "reborn-conv-opaque".into();
            }
            assert!(matches!(
                port.complete(prompt.clone(), invalid).await,
                Err(KohaiPortError::InvalidContext { .. })
            ));
        }
        assert_eq!(recorded_request_count(&requests), 2);
        let client = rig.pool.get().await.unwrap();
        let packets = client.query(
            "SELECT run_id, iteration, status FROM brassclaw_forensic_packets WHERE tenant_id = $1 ORDER BY iteration",
            &[&tenant],
        ).await.unwrap();
        assert_eq!(
            packets.len(),
            2,
            "invalid identities must not write forensic packets"
        );
        for (iteration, packet) in packets.iter().enumerate() {
            assert_eq!(packet.get::<_, String>(0), run_id.to_string());
            assert_eq!(packet.get::<_, i32>(1), iteration as i32);
            assert_eq!(packet.get::<_, String>(2), "complete");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancel_run_propagates_to_subagent_children() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "unused".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-cancel-child-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-cancel-child-tenant".to_string(),
            agent_id: "runtime-cancel-child-agent".to_string(),
            source_binding_id: "runtime-cancel-child-source".to_string(),
            reply_target_binding_id: "runtime-cancel-child-reply".to_string(),
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        super::test_pg::stop_worker_for_state_fixture(&runtime).await;
        let conversation = runtime.new_conversation().await.expect("conversation");
        let parent_scope = runtime.turn_scope_for(&conversation.0);
        let actor = TurnActor::new(runtime.actor_user_id.clone());
        let parent = runtime
            .turn_coordinator
            .submit_turn(SubmitTurnRequest {
                scope: parent_scope.clone(),
                actor: actor.clone(),
                accepted_message_ref: AcceptedMessageRef::new("msg:cancel-parent").unwrap(),
                source_binding_ref: SourceBindingRef::new("source:cancel-parent").unwrap(),
                reply_target_binding_ref: ReplyTargetBindingRef::new("reply:cancel-parent")
                    .unwrap(),
                requested_run_profile: None,
                idempotency_key: IdempotencyKey::new("cancel-parent").unwrap(),
                received_at: Utc::now(),
                requested_run_id: None,
                parent_run_id: None,
                subagent_depth: 0,
                spawn_tree_root_run_id: None,
            })
            .await
            .expect("parent submitted");
        let SubmitTurnResponse::Accepted {
            run_id: parent_run_id,
            ..
        } = parent;
        let child_scope = TurnScope::new(
            parent_scope.tenant_id.clone(),
            parent_scope.agent_id.clone(),
            parent_scope.project_id.clone(),
            ThreadId::new("runtime-cancel-child-thread").unwrap(),
        );
        let child = runtime
            .turn_tree_store
            .submit_child_turn(
                SubmitChildRunRequest {
                    parent_scope: parent_scope.clone(),
                    parent_run_id,
                    child_scope: child_scope.clone(),
                    actor,
                    accepted_message_ref: AcceptedMessageRef::new("msg:cancel-child").unwrap(),
                    source_binding_ref: SourceBindingRef::new("source:cancel-child").unwrap(),
                    reply_target_binding_ref: ReplyTargetBindingRef::new("reply:cancel-child")
                        .unwrap(),
                    requested_run_profile: None,
                    idempotency_key: IdempotencyKey::new("cancel-child").unwrap(),
                    received_at: Utc::now(),
                    requested_run_id: None,
                    spawn_tree_descendant_cap: 4,
                },
                &AllowAllTurnAdmissionPolicy,
                &InMemoryRunProfileResolver::default(),
            )
            .await
            .expect("child submitted");
        let SubmitTurnResponse::Accepted {
            run_id: child_run_id,
            ..
        } = child;
        let result_ref = LoopResultRef::new("result:runtime-cancel-child").unwrap();
        runtime
            .thread_service
            .append_tool_result_reference(AppendToolResultReferenceRequest {
                scope: runtime.thread_scope.clone(),
                thread_id: parent_scope.thread_id.clone(),
                turn_run_id: parent_run_id.to_string(),
                result_ref: result_ref.as_str().to_string(),
                safe_summary: ToolResultSafeSummary::new("subagent spawned").unwrap(),
                provider_call: None,
                model_observation: None,
            })
            .await
            .expect("parent result reference seeded");
        let child_thread_scope = ThreadScope {
            tenant_id: child_scope.tenant_id.clone(),
            agent_id: child_scope.agent_id.clone().unwrap(),
            project_id: child_scope.project_id.clone(),
            owner_user_id: Some(runtime.actor_user_id.clone()),
        };
        runtime
            .thread_service
            .ensure_thread(EnsureThreadRequest {
                scope: child_thread_scope,
                thread_id: Some(child_scope.thread_id.clone()),
                created_by_actor_id: "test".to_string(),
                title: Some("Subagent".to_string()),
                metadata_json: Some(
                    serde_json::to_string(&SubagentThreadMetadata {
                        kind: SubagentThreadKind::Subagent,
                        parent_run_id,
                        parent_thread_id: parent_scope.thread_id.clone(),
                        tree_root_run_id: parent_run_id,
                        child_run_id,
                        subagent_kind: SubagentKindId::new("general").unwrap(),
                        mode: SpawnSubagentMode::Blocking,
                        result_ref,
                        handoff: None,
                    })
                    .unwrap(),
                ),
            })
            .await
            .expect("child thread metadata seeded");

        runtime
            .cancel_run(
                &parent_scope,
                parent_run_id,
                SanitizedCancelReason::UserRequested,
                "test-parent-cancel",
            )
            .await
            .expect("parent cancellation succeeds");

        let child_state = runtime
            .turn_coordinator
            .get_run_state(GetRunStateRequest {
                scope: child_scope,
                run_id: child_run_id,
            })
            .await
            .expect("child state");
        // The child must be in a terminal non-success state. Ideally Cancelled
        // (cancel_descendant_runs marked it before the scheduler touched it),
        // but Failed is also acceptable — the scheduler may have picked it up
        // between cancel_run and cancel_descendant_runs and it then fails
        // because its parent context is being torn down.
        assert!(
            matches!(
                child_state.status,
                TurnStatus::Cancelled | TurnStatus::Failed
            ),
            "expected child to be Cancelled or Failed, got {:?}",
            child_state.status
        );

        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_global_runtime_retains_one_root_across_match_and_no_match() {
        native_mcp_runtime_acceptance(false).await;
    }

    #[cfg(all(
        feature = "postgres",
        feature = "skills-db",
        feature = "root-llm-provider"
    ))]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires the operator's LOCAL_TEST_ENV.md vLLM endpoint"]
    async fn native_mcp_provider_ordinary_chat_round_trip() {
        native_mcp_runtime_acceptance(true).await;
    }

    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    async fn native_mcp_runtime_acceptance(live_provider: bool) {
        use brassclaw_product_workflow::MontyVmSettingsStore;
        use sha2::Digest;

        let root = tempfile::tempdir().unwrap();
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let input = RebornRuntimeInput::from_services(
            rig.build_input("global-runtime-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_model_gateway_override(Arc::new(RecordingGateway {
            reply: "model response for an unmatched input".into(),
            requests: requests.clone(),
        }))
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: RUNTIME_SEND_TIMEOUT,
        });
        // This regression retains two complete startup futures across awaits.
        // Heap-allocate them rather than doubling the test-thread stack frame.
        let runtime = Arc::new(Box::pin(build_reborn_runtime(input)).await.unwrap());
        let discovery = runtime.mcp_recipe_discovery().snapshot().unwrap();
        assert!(!discovery.generation().is_nil());
        assert_eq!(discovery.tools_list(), serde_json::json!({"tools":[]}));
        let conversation = runtime.new_conversation().await.unwrap();
        assert!(
            uuid::Uuid::parse_str(conversation.0.as_str()).is_err(),
            "production conversation ID must stay opaque"
        );
        let literal = "quoted ' Unicode ü and {{not_source}}";
        let mut outcomes = Vec::new();
        for text in [
            "unmatched first input".to_string(),
            format!("reply {literal}"),
            "unmatched follow-up input".to_string(),
        ] {
            let reply = runtime
                .send_user_message(&conversation, &text)
                .await
                .unwrap();
            let state = runtime
                .turn_coordinator
                .get_run_state(GetRunStateRequest {
                    scope: runtime.turn_scope_for(&conversation.0),
                    run_id: reply.run_id,
                })
                .await
                .unwrap();
            assert_eq!(
                reply.status,
                TurnStatus::Completed,
                "failure category: {:?}",
                state.failure.as_ref().map(|failure| failure.category())
            );
            if text.starts_with("reply ") {
                assert_eq!(reply.text.as_deref(), Some(literal));
            }
            let client = rig.pool.get().await.unwrap();
            let row=client.query_one("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",&[&reply.run_id.as_uuid()]).await.unwrap();
            let retained_outcome: serde_json::Value = row.get(0);
            let review_event = client
                .query_opt(
                    "SELECT event_bytes FROM brassclaw_monty_review_events WHERE run_id=$1",
                    &[&reply.run_id.as_uuid()],
                )
                .await
                .unwrap();
            assert_eq!(review_event.is_some(), !text.starts_with("reply "));
            if let Some(event) = review_event {
                let event: serde_json::Value = serde_json::from_str(event.get(0)).unwrap();
                assert_eq!(event["outcome"], retained_outcome);
                assert_eq!(event["scope"]["thread_id"], conversation.0.as_str());
                assert_eq!(event["evidence_complete"], false);
            }
            outcomes.push(row.get::<_, serde_json::Value>(0));
        }
        let root_id = &outcomes[0]["execution"]["root"]["vm_id"];
        assert!(root_id.is_string());
        assert!(
            outcomes
                .iter()
                .all(|outcome| &outcome["execution"]["root"]["vm_id"] == root_id)
        );
        for outcome in &outcomes {
            assert_eq!(
                outcome["execution"]["recipes"].as_array().unwrap().len(),
                2,
                "both Match and No-Match must use retained reply and history workflows"
            );
        }
        for (index, outcome) in outcomes.iter().enumerate() {
            let execution = &outcome["execution"];
            assert_eq!(
                execution["intent_outcome"],
                if index == 1 { "match" } else { "no_match" }
            );
            let matched: Vec<_> = execution["recipes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|recipe| !recipe["normal_match"].is_null())
                .collect();
            if index != 1 {
                assert!(
                    matched.is_empty(),
                    "named reply/history lookup must not invent matching evidence"
                );
                continue;
            }
            assert_eq!(matched.len(), 1);
            let recipe = matched[0];
            let evidence = &recipe["normal_match"];
            assert_eq!(evidence["recipe_completed"], true);
            assert_eq!(evidence["task_completed"], true);
            let matching = &evidence["matching"];
            assert_eq!(matching["format"], "monty-normal-match/1");
            assert_eq!(
                matching["catalogue_generation"],
                discovery.generation().to_string()
            );
            assert_eq!(matching["recipe_uuid"], recipe["recipe_id"]);
            assert_eq!(matching["selection_checksum"], recipe["selection_checksum"]);
            assert_eq!(
                matching["command_checksum"],
                hex::encode(sha2::Sha256::digest(format!("reply {literal}").as_bytes()))
            );
            assert_eq!(
                matching["task_inputs_checksum"],
                hex::encode(sha2::Sha256::digest(
                    serde_json::json!({"answer":literal}).to_string().as_bytes()
                ))
            );
            assert_eq!(execution["semantic_approval"], false);
            assert_eq!(execution["catalogue_activation"], false);
        }
        let recorded = requests.lock().unwrap().clone();
        assert_eq!(
            recorded.len(),
            2,
            "deterministic matched Recipe must not call the model"
        );
        assert!(
            recorded[1]
                .messages
                .iter()
                .any(|message| message.content == literal),
            "eligible prior reply survives the Monty handoff"
        );
        // Activation uses the same ordinary chat matcher and real reply owner.
        let public = runtime
            .send_user_message(&conversation, "publish literal reply Ready.")
            .await
            .unwrap();
        assert_eq!(public.status, TurnStatus::Completed);
        assert_eq!(public.text.as_deref(), Some("Ready."));
        let client = rig.pool.get().await.unwrap();
        let selection: String = client.query_one(
            "SELECT selection_bytes FROM brassclaw_monty_recipe_selections WHERE run_id=$1 AND recipe_id=$2",
            &[&public.run_id.as_uuid(), &crate::installed_monty_catalogue::id("reply:recipe")],
        ).await.unwrap().get(0);
        let selection: serde_json::Value = serde_json::from_str(&selection).unwrap();
        let pin = &selection["association_approvals"]["0:2"];
        let approval_id: uuid::Uuid = serde_json::from_value(pin["approval_id"].clone()).unwrap();
        let row = client
            .query_one(
                "SELECT checksum FROM reborn_skill_association_approvals WHERE approval_id=$1",
                &[&approval_id],
            )
            .await
            .unwrap();
        assert_eq!(pin["checksum"], row.get::<_, String>(0));
        let invocation: String = client.query_one(
            "SELECT selection_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND recipe_id=$2",
            &[&public.run_id.as_uuid(), &crate::installed_monty_catalogue::id("reply:recipe")],
        ).await.unwrap().get(0);
        let invocation: serde_json::Value = serde_json::from_str(&invocation).unwrap();
        assert_eq!(invocation["association_approvals"]["0:2"], *pin);

        drop(client);
        assert_eq!(
            runtime
                .mcp_recipe_discovery()
                .snapshot()
                .unwrap()
                .tools_list(),
            serde_json::json!({"tools":[]})
        );
        assert!(
            runtime
                .mcp_recipe_discovery()
                .snapshot()
                .unwrap()
                .qualification_checksum()
                .is_none()
        );
        for literal in [
            "quoted ' Unicode ü and {{not_source}}",
            "first line\nsecond line with \\ and %",
        ] {
            let isolated = runtime.new_conversation().await.unwrap();
            let reply = runtime
                .send_user_message(&isolated, &format!("publish literal reply {literal}"))
                .await
                .unwrap();
            assert_eq!(reply.status, TurnStatus::Completed);
            assert_eq!(reply.text.as_deref(), Some(literal));
        }
        assert_eq!(
            runtime
                .mcp_recipe_discovery()
                .snapshot()
                .unwrap()
                .tools_list(),
            serde_json::json!({"tools":[]})
        );
        assert_eq!(requests.lock().unwrap().len(), 2);
        let settings = runtime.monty_settings_owner.store();
        let identity_before_capacity_edits = runtime.global_monty_owner.client().root_identity();
        for (capacity, text, expected_status) in [
            (
                1,
                "publish literal reply policy-blocked qualification case",
                TurnStatus::Failed,
            ),
            (16, "reply capacity raised", TurnStatus::Completed),
        ] {
            let current = settings.get("default", "default").await.unwrap();
            let update = serde_json::from_value(serde_json::json!({
                "expected_revision":current.revision,"max_recipes_per_task":capacity,
            }))
            .unwrap();
            let changed = settings
                .upsert("default", "default", &update)
                .await
                .unwrap();
            let observed = settings
                .runtime_observation(&changed)
                .unwrap()
                .recipe_budget
                .unwrap();
            assert_eq!(
                observed.uptake,
                brassclaw_product_workflow::MontyBudgetUptake::Applied
            );
            assert_eq!(observed.max_recipes_per_task, capacity);
            assert_eq!(observed.effective_revision, changed.revision);
            let result = runtime
                .send_user_message(&conversation, text)
                .await
                .unwrap();
            assert_eq!(result.status, expected_status);
            let client = rig.pool.get().await.unwrap();
            let row = client.query_one(
                "SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
                &[&result.run_id.as_uuid()],
            ).await.unwrap();
            let outcome: serde_json::Value = row.get(0);
            assert_eq!(outcome["execution"]["intent_outcome"], "match");
            let evidence = &outcome["execution"]["recipes"][0]["normal_match"];
            assert_eq!(evidence["matching"]["format"], "monty-normal-match/1");
            assert_eq!(
                evidence["task_completed"],
                expected_status == TurnStatus::Completed,
                "a matched Recipe followed by failure cannot qualify a completed command"
            );
            assert_eq!(
                outcome["execution"]["checked_recipe_capacity"]["max_recipes"],
                capacity
            );
            assert_eq!(
                outcome["execution"]["checked_recipe_capacity"]["revision"],
                changed.revision
            );
            assert_eq!(
                runtime.global_monty_owner.client().root_identity(),
                identity_before_capacity_edits
            );
        }
        let history = runtime
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: runtime.thread_scope.clone(),
                thread_id: conversation.0.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            history
                .messages
                .iter()
                .filter(|message| {
                    message.kind == MessageKind::Assistant
                        && message.content.as_deref() == Some("policy-blocked qualification case")
                })
                .count(),
            1,
            "the reply completed before capacity denial must never be replayed"
        );
        assert_eq!(
            requests.lock().unwrap().len(),
            2,
            "capacity failure must not enter Tier 2"
        );
        assert!(
            runtime
                .mcp_recipe_discovery()
                .snapshot()
                .unwrap()
                .qualification_checksum()
                .is_none(),
            "a capacity failure after a completed effect cannot qualify the required policy denial"
        );
        // A live block changes dispatch permission without replacing the pinned
        // Recipe or replaying its failure through the model path.
        let client = rig.pool.get().await.unwrap();
        let changed=client.execute("UPDATE brassclaw_instance_tool_settings SET enabled=false,revision=revision+1
            WHERE tool_id IN (SELECT id FROM reborn_tools WHERE tenant_id=$1 AND capability_id='host.post_reply')",
            &[&runtime.thread_scope.tenant_id.as_str()]).await.unwrap();
        assert_eq!(changed, 1);
        drop(client);
        let blocked = runtime
            .send_user_message(&conversation, "reply must not publish")
            .await
            .unwrap();
        assert_eq!(blocked.status, TurnStatus::Failed);
        let (driver, factory) = &runtime.monty_test_controls;
        let host = driver
            .retained_host_for_run(blocked.run_id)
            .unwrap()
            .unwrap();
        assert!(host.finalized_reply_ref().is_none());
        assert!(
            crate::pg_monty_admission::PgMontyAdmission::reserve(
                rig.pool.clone(),
                host.run_context(),
                host.attempt()
            )
            .await
            .is_err(),
            "a failed attempt cannot acquire a replacement admission"
        );
        let (original, receipt, control, _driver_retention) =
            driver.take_settlement(host.attempt()).unwrap().unwrap();
        let (owned, _, ports, owned_receipt, _factory_retention) = factory
            .take_failed_settlement(host.attempt())
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(&host, &original) && Arc::ptr_eq(&original, &owned));
        assert!(Arc::ptr_eq(&receipt, &owned_receipt));
        assert!(Arc::ptr_eq(&receipt, &control.receipt().unwrap().unwrap()));
        let report = ports.settlement_report(&receipt).await.unwrap();
        assert_eq!(report["root_completed"], false);
        let denied_chat = runtime.new_conversation().await.unwrap();
        let denied_command = runtime
            .send_user_message(
                &denied_chat,
                "publish literal reply policy-blocked qualification case",
            )
            .await
            .unwrap();
        assert_eq!(denied_command.status, TurnStatus::Failed);
        assert!(denied_command.text.is_none());
        let advertised = runtime.mcp_recipe_discovery().snapshot().unwrap();
        assert_eq!(advertised.generation(), discovery.generation());
        assert!(advertised.qualification_checksum().is_some());
        assert_eq!(
            advertised.tools_list()["tools"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            advertised.tools_list()["tools"][0]["name"],
            "publish_literal_reply"
        );
        runtime
            .mcp_recipe_discovery()
            .validate_advertised_command(
                &advertised,
                "publish_literal_reply",
                "publish literal reply Ready.",
            )
            .unwrap();
        assert!(
            runtime
                .mcp_recipe_discovery()
                .validate_advertised_command(
                    &discovery,
                    "publish_literal_reply",
                    "publish literal reply Ready."
                )
                .is_err(),
            "an earlier empty snapshot never advertised this command"
        );
        let qualified_program = ports.matched_program().await.unwrap();
        crate::public_recipe_population::assert_public_candidate_retention(
            rig.pool.clone(),
            &qualified_program,
            discovery.generation(),
            &brassclaw_engine::memory::intent_system::IntentScope {
                tenant_id: runtime.thread_scope.tenant_id.to_string(),
                user_id: runtime.actor_user_id.to_string(),
                agent_id: host
                    .run_context()
                    .scope
                    .agent_id
                    .as_ref()
                    .unwrap()
                    .to_string(),
                project_id: host
                    .run_context()
                    .scope
                    .project_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "default".into()),
            },
        )
        .await;
        crate::mcp_command_qualification::assert_recorded_command_cases(
            &rig.pool,
            &brassclaw_engine::memory::intent_system::IntentScope {
                tenant_id: runtime.thread_scope.tenant_id.to_string(),
                user_id: runtime.actor_user_id.to_string(),
                agent_id: host
                    .run_context()
                    .scope
                    .agent_id
                    .as_ref()
                    .unwrap()
                    .to_string(),
                project_id: host
                    .run_context()
                    .scope
                    .project_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "default".into()),
            },
            discovery.generation(),
            &qualified_program,
            &format!("reply {literal}"),
        )
        .await;
        assert_eq!(
            requests.lock().unwrap().len(),
            2,
            "failed matched workflow must not replay as Tier 2"
        );
        // The internal-turn facade's trusted marker also remains effective on
        // the global path, independently of whether a matching Recipe exists.
        let internal = runtime
            .new_internal_conversation(
                runtime.thread_scope.tenant_id.as_str(),
                runtime.actor_user_id.as_str(),
                "internal-project",
            )
            .await
            .unwrap();
        let denied = runtime
            .send_internal_user_message(
                &internal,
                "unmatched internal task",
                super::InternalTurnOptions {
                    hard_fail_on_recipe_miss: true,
                    allow_tier_two: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(denied.status, TurnStatus::Failed);
        assert_eq!(
            requests.lock().unwrap().len(),
            2,
            "internal No-Match must not call a model"
        );
        // Transport recovery must retain the original ordinary chat and run.
        let transport_id = uuid::Uuid::new_v4();
        let transport_chat = runtime
            .ensure_correlated_conversation(transport_id)
            .await
            .unwrap();
        assert_eq!(
            runtime
                .ensure_correlated_conversation(transport_id)
                .await
                .unwrap(),
            transport_chat
        );
        let command = "publish literal reply transport recovery";
        let first = runtime
            .send_correlated_user_message(
                &transport_chat,
                command,
                transport_id,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(first.run_id, TurnRunId::from_uuid(transport_id));
        assert_eq!(first.status, TurnStatus::Failed); // current global Tool block
        let before_retry: i64 = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get(0);
        let replay = runtime
            .send_correlated_user_message(
                &transport_chat,
                command,
                transport_id,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(first, replay);
        assert!(
            runtime
                .send_correlated_user_message(
                    &transport_chat,
                    "publish literal reply changed",
                    transport_id,
                    tokio_util::sync::CancellationToken::new(),
                )
                .await
                .is_err()
        );
        runtime
            .close_correlated_conversation(&transport_chat, transport_id)
            .await
            .unwrap();
        runtime
            .close_correlated_conversation(&transport_chat, transport_id)
            .await
            .unwrap();
        assert_eq!(
            runtime
                .recover_correlated_user_message(&transport_chat, transport_id)
                .await
                .unwrap(),
            Some(first)
        );
        assert!(
            runtime
                .send_user_message(&transport_chat, command)
                .await
                .is_err()
        );
        let preserved = runtime
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: runtime.thread_scope.clone(),
                thread_id: transport_chat.0.clone(),
            })
            .await
            .unwrap();
        assert_eq!(
            preserved
                .messages
                .iter()
                .filter(|message| message.kind == MessageKind::User)
                .count(),
            1
        );
        let after_retry: i64 = rig
            .pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            before_retry, after_retry,
            "replay/closure/recovery cannot repeat dispatch"
        );
        let client = rig.pool.get().await.unwrap();
        client.execute("UPDATE brassclaw_instance_tool_settings SET enabled=true,revision=revision+1 WHERE tool_id IN(SELECT id FROM reborn_tools WHERE tenant_id=$1 AND capability_id='host.post_reply')", &[&runtime.thread_scope.tenant_id.as_str()]).await.unwrap();
        drop(client);
        Box::pin(crate::mcp_chat_bridge::assert_native_chat_transport(
            runtime.clone(),
            blocked.run_id.as_uuid(),
        ))
        .await;
        let client = rig.pool.get().await.unwrap();
        client.execute("UPDATE brassclaw_instance_tool_settings SET enabled=false,revision=revision+1 WHERE tool_id IN(SELECT id FROM reborn_tools WHERE tenant_id=$1 AND capability_id='host.post_reply')", &[&runtime.thread_scope.tenant_id.as_str()]).await.unwrap();
        drop(client);
        shutdown_shared_runtime(runtime).await.unwrap();
        let client = rig.pool.get().await.unwrap();
        let before: i64 = client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get(0);
        drop(client);
        let restart_gateway: Arc<dyn HostManagedModelGateway> = if live_provider {
            mcp_provider_acceptance::gateway(&rig.pool).await
        } else {
            Arc::new(RecordingGateway {
                reply: "unused during bootstrap readback".into(),
                requests: requests.clone(),
            })
        };
        let restart = Arc::new(
            Box::pin(build_reborn_runtime(
                RebornRuntimeInput::from_services(
                    rig.build_input("global-runtime-owner", root.path())
                        .with_runtime_policy(local_dev_runtime_policy()),
                )
                .with_model_gateway_override(restart_gateway)
                .with_inbound_mcp_required(live_provider),
            ))
            .await
            .unwrap(),
        );
        let restarted_discovery = restart.mcp_recipe_discovery().snapshot().unwrap();
        assert_eq!(
            restart
                .mcp_chat_bridge()
                .unwrap()
                .prepare_startup()
                .await
                .unwrap(),
            1
        );
        assert_eq!(restarted_discovery.generation(), advertised.generation());
        assert_eq!(restarted_discovery.tools_list(), advertised.tools_list());
        assert_eq!(
            restarted_discovery.qualification_checksum(),
            advertised.qualification_checksum()
        );
        let client = rig.pool.get().await.unwrap();
        let after: i64 = client
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            before, after,
            "actual restart must reuse bootstrap approval without replaying effects, even while the Tool is blocked"
        );
        drop(client);
        if live_provider {
            mcp_provider_acceptance::round_trip(&restart, &rig.pool).await;
        }
        shutdown_shared_runtime(restart).await.unwrap();
    }

    #[cfg(feature = "skills-db")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_webui_monty_task_settings_reach_one_running_instance() {
        use axum::{
            Router,
            body::{Body, to_bytes},
            http::{Method, Request, StatusCode},
        };
        use brassclaw_product_workflow::{MontyBudgetUptake, MontyVmSettingsStore};
        use tower::ServiceExt;
        struct Operator;
        #[async_trait]
        impl crate::WebuiAuthenticator for Operator {
            async fn authenticate(&self, token: &str) -> Option<UserId> {
                match token {
                    "settings-a" | "settings-b" => Some(UserId::new(token).unwrap()),
                    _ => None,
                }
            }
            fn allows_operator_webui_config(&self) -> bool {
                true
            }
        }
        struct WaitingGateway {
            entered: Arc<tokio::sync::Notify>,
            release: Arc<tokio::sync::Semaphore>,
        }
        #[async_trait]
        impl HostManagedModelGateway for WaitingGateway {
            async fn stream_model(
                &self,
                _request: HostManagedModelRequest,
            ) -> Result<HostManagedModelResponse, HostManagedModelError> {
                self.entered.notify_one();
                self.release.acquire().await.unwrap().forget();
                Ok(
                    HostManagedModelResponse::assistant_reply("model result after the live edit")
                        .with_usage(brassclaw_turns::run_profile::LoopModelUsage {
                            input_tokens: 200,
                            output_tokens: 15,
                            ..Default::default()
                        }),
                )
            }
        }
        async fn request(
            app: &Router,
            method: Method,
            token: &str,
            path: &str,
            body: serde_json::Value,
        ) -> (StatusCode, serde_json::Value) {
            let bytes = if method == Method::GET {
                String::new()
            } else {
                body.to_string()
            };
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header("Authorization", format!("Bearer {token}"))
                        .header("Content-Type", "application/json")
                        .body(Body::from(bytes))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            let body = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                panic!("settings HTTP {status} returned a non-JSON body: {error}")
            });
            (status, body)
        }
        let root = tempfile::tempdir().unwrap();
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Startup)
            .await;
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let input = RebornRuntimeInput::from_services(
            rig.build_input("live-settings-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_model_gateway_override(Arc::new(WaitingGateway {
            entered: entered.clone(),
            release: release.clone(),
        }))
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: RUNTIME_SEND_TIMEOUT,
        });
        let runtime = Arc::new(build_reborn_runtime(input).await.unwrap());
        let bundle = build_webui_services(runtime.clone(), None).await.unwrap();
        let app = crate::webui_v2_app(
            bundle,
            crate::WebuiServeConfig::new(
                runtime.thread_scope.tenant_id.clone(),
                Arc::new(Operator),
                Vec::new(),
            ),
        )
        .unwrap();
        let (code, mut initial) = request(
            &app,
            Method::GET,
            "settings-a",
            "/api/settings/monty-vm",
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(initial["settings"]["memory_policy"]["mode"], "startup");
        assert_eq!(
            initial["runtime"]["memory_budget"]["measurement_status"],
            "startup_floor"
        );
        assert_eq!(
            initial["runtime"]["memory_budget"]["max_memory_bytes"],
            brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES
        );
        assert_eq!(initial["settings"]["max_recipes_per_task"], 8);
        assert_eq!(
            initial["settings"]["execution_limits"]["max_pending_settings"],
            8
        );
        assert_eq!(
            initial["settings"]["execution_limits"]["max_retained_attempts"],
            256
        );
        assert_eq!(
            initial["settings"]["execution_limits"]["max_recipe_contexts"],
            64
        );
        assert_eq!(
            initial["settings"]["execution_limits"]["max_queued_tasks"],
            64
        );
        assert_eq!(
            initial["settings"]["execution_limits"]["max_queued_bytes"],
            67108864
        );
        for capacity in [
            serde_json::json!(0),
            serde_json::json!(u32::MAX as u64 + 1),
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!(null),
            serde_json::json!("private-invalid-settings-value"),
        ] {
            let mut invalid = initial["settings"]["execution_limits"].clone();
            invalid["max_recipe_contexts"] = capacity;
            let (code, error) = request(&app, Method::PUT, "settings-a", "/api/settings/monty-vm",
                serde_json::json!({"expected_revision":initial["settings"]["revision"],"execution_limits":invalid})).await;
            assert_eq!(code, StatusCode::BAD_REQUEST);
            assert_eq!(error["error"], "invalid_request");
            assert!(!error.to_string().contains("private-invalid-settings-value"));
        }
        for field in [
            "max_queued_tasks",
            "max_queued_bytes",
            "max_pending_settings",
            "max_retained_attempts",
            "max_actor_requests",
            "max_actor_reserved_bytes",
            "max_actor_control_requests",
            "max_actor_control_reserved_bytes",
            "startup_timeout_millis",
            "response_timeout_millis",
            "settings_source_timeout_millis",
            "settings_uptake_timeout_millis",
            "ownership_check_timeout_millis",
            "ownership_heartbeat_interval_millis",
            "max_pending_ownership_checks",
            "cancellation_ack_timeout_millis",
            "settings_reconcile_interval_millis",
            "status_poll_interval_millis",
        ] {
            let mut invalid = initial["settings"]["execution_limits"].clone();
            invalid[field] = serde_json::json!(0);
            let (code, _) = request(&app, Method::PUT, "settings-a", "/api/settings/monty-vm",
                serde_json::json!({"expected_revision":initial["settings"]["revision"],"execution_limits":invalid})).await;
            assert_eq!(code, StatusCode::BAD_REQUEST);
        }
        for (content_type, body, expected) in [
            ("text/plain", "{}", StatusCode::UNSUPPORTED_MEDIA_TYPE),
            (
                "application/json",
                "{private-invalid-settings-value",
                StatusCode::BAD_REQUEST,
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/api/settings/monty-vm")
                        .header("Authorization", "Bearer settings-a")
                        .header("Content-Type", content_type)
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
            let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(error["error"], "invalid_request");
            assert!(!error.to_string().contains("private-invalid-settings-value"));
        }
        let (code, _) = request(&app, Method::PUT, "settings-a", "/api/settings/monty-vm",
            serde_json::json!({"expected_revision":initial["settings"]["revision"],"max_memory_bytes":brassclaw_product_workflow::DEFAULT_MONTY_HEAP_BYTES-1})).await;
        assert_eq!(code, StatusCode::BAD_REQUEST);
        for capacity in [0u64, i32::MAX as u64 + 1] {
            let (code, _) = request(&app, Method::PUT, "settings-a", "/api/settings/monty-vm",
                serde_json::json!({"expected_revision":initial["settings"]["revision"],"max_recipes_per_task":capacity})).await;
            assert_eq!(code, StatusCode::BAD_REQUEST);
        }
        let boot_samples = initial["runtime"]["memory_budget"]["memory_sample_count"]
            .as_u64()
            .unwrap();
        assert_eq!(boot_samples, 1);
        let root_before_memory_edit = runtime.global_monty_owner.client().root_identity();
        let initial_heap = runtime.global_monty_owner.client().heap_observation();
        let heap = initial_heap.status.effective.unwrap().max_vm_bytes as u64;
        let (code, manual) = request(&app, Method::PUT, "settings-a", "/api/settings/monty-vm",
            serde_json::json!({"expected_revision":initial["settings"]["revision"], "max_memory_bytes":heap + 64 * 1024 * 1024})).await;
        assert_eq!(code, StatusCode::OK, "{manual}");
        assert_eq!(manual["settings"]["memory_policy"]["mode"], "manual");
        assert_eq!(manual["runtime"]["memory_budget"]["uptake"], "applied");
        assert_eq!(
            manual["runtime"]["memory_budget"]["max_memory_bytes"],
            heap + 64 * 1024 * 1024
        );
        assert_eq!(
            manual["runtime"]["memory_budget"]["memory_sample_count"],
            boot_samples
        );
        let stored_revision = manual["settings"]["revision"].as_u64().unwrap();
        let (code, _) = request(
            &app,
            Method::PUT,
            "settings-a",
            "/api/settings/monty-vm",
            serde_json::json!({"expected_revision":stored_revision,"max_memory_bytes":1}),
        )
        .await;
        assert_eq!(code, StatusCode::BAD_REQUEST);
        let (code, unchanged) = request(
            &app,
            Method::GET,
            "settings-b",
            "/api/settings/monty-vm",
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(
            unchanged["settings"]["revision"], stored_revision,
            "unsafe manual reduction must not mutate the DB"
        );
        assert_eq!(
            unchanged["runtime"]["memory_budget"]["max_memory_bytes"],
            heap + 64 * 1024 * 1024
        );
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            root_before_memory_edit
        );
        initial = unchanged;
        let initial_revision = initial["settings"]["revision"].as_u64().unwrap();
        assert_eq!(
            initial["runtime"]["task_budget"]["effective_revision"],
            initial_revision
        );
        assert_eq!(initial["runtime"]["task_budget"]["uptake"], "applied");
        let governor = runtime.budget_resource_governor().unwrap();
        let budget_account = brassclaw_resources::ResourceAccount::user(
            runtime.thread_scope.tenant_id.clone(),
            runtime.actor_user_id.clone(),
        );
        governor
            .set_limit(
                budget_account.clone(),
                brassclaw_resources::ResourceLimits {
                    max_input_tokens: Some(10),
                    max_output_tokens: Some(10),
                    max_usd: Some(rust_decimal::Decimal::ONE),
                    period: brassclaw_resources::BudgetPeriod::Rolling24h,
                    ..Default::default()
                },
            )
            .unwrap();
        let budget_scope = brassclaw_host_api::ResourceScope {
            tenant_id: runtime.thread_scope.tenant_id.clone(),
            user_id: runtime.actor_user_id.clone(),
            agent_id: Some(runtime.thread_scope.agent_id.clone()),
            project_id: None,
            thread_id: None,
            invocation_id: brassclaw_host_api::InvocationId::new(),
        };
        let conversation = runtime.new_conversation().await.unwrap();
        let sending = {
            let runtime = runtime.clone();
            let conversation = conversation.clone();
            tokio::spawn(async move {
                runtime
                    .send_user_message(&conversation, "unmatched task waiting for its model")
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        let mut execution_limits = initial["settings"]["execution_limits"].clone();
        let root_identity = runtime.global_monty_owner.client().root_identity();
        // The task is in an external provider wait. Both edits must reach the
        // existing VM and its Rust watch without charging that wait or restarting.
        for (offset, duration, enabled) in [(1, 7200, true), (2, 600, false)] {
            execution_limits["max_feeds"] = serde_json::json!(128 + offset);
            execution_limits["max_recipe_contexts"] = serde_json::json!(128 + offset);
            execution_limits["max_queued_tasks"] = serde_json::json!(128 + offset);
            execution_limits["max_queued_bytes"] = serde_json::json!(67108864 + offset);
            execution_limits["max_pending_settings"] = serde_json::json!(16 + offset);
            execution_limits["max_retained_attempts"] = serde_json::json!(256 + offset);
            execution_limits["max_actor_requests"] = serde_json::json!(2048 + offset);
            execution_limits["max_actor_reserved_bytes"] = serde_json::json!(1073741824 + offset);
            execution_limits["max_actor_control_requests"] = serde_json::json!(2048 + offset);
            execution_limits["max_actor_control_reserved_bytes"] =
                serde_json::json!(1073741824 + offset);
            execution_limits["startup_timeout_millis"] = serde_json::json!(30000 + offset);
            execution_limits["response_timeout_millis"] = serde_json::json!(40000 + offset);
            execution_limits["settings_source_timeout_millis"] = serde_json::json!(3000 + offset);
            execution_limits["settings_uptake_timeout_millis"] = serde_json::json!(7000 + offset);
            execution_limits["ownership_check_timeout_millis"] = serde_json::json!(3000 + offset);
            execution_limits["ownership_heartbeat_interval_millis"] =
                serde_json::json!(2500 + offset);
            execution_limits["max_pending_ownership_checks"] = serde_json::json!(1024 + offset);
            execution_limits["cancellation_ack_timeout_millis"] = serde_json::json!(4000 + offset);
            execution_limits["settings_reconcile_interval_millis"] =
                serde_json::json!(1000 + offset);
            execution_limits["status_poll_interval_millis"] = serde_json::json!(3000 + offset);
            execution_limits["execution_slice_millis"] = serde_json::json!(5 + offset);
            let (code, result) = request(
                &app,
                Method::PUT,
                "settings-a",
                "/api/settings/monty-vm",
                serde_json::json!({"expected_revision":initial_revision + offset - 1,
                    "max_duration_secs":duration,"token_budgets_enabled":enabled,
                    "execution_limits": execution_limits,"max_recipes_per_task": 16 + offset}),
            )
            .await;
            assert_eq!(code, StatusCode::OK, "{result}");
            assert_eq!(
                result["runtime"]["memory_budget"]["memory_sample_count"],
                boot_samples
            );
            assert_eq!(
                result["runtime"]["memory_budget"]["measurement_status"],
                "disabled"
            );
            assert_eq!(result["runtime"]["task_budget"]["uptake"], "applied");
            assert_eq!(result["runtime"]["execution_limits"]["uptake"], "applied");
            assert_eq!(result["runtime"]["recipe_budget"]["uptake"], "applied");
            assert_eq!(
                result["runtime"]["recipe_budget"]["max_recipes_per_task"],
                16 + offset
            );
            assert_eq!(
                result["runtime"]["recipe_budget"]["effective_revision"],
                initial_revision + offset
            );
            assert_eq!(
                result["runtime"]["execution_limits"]["limits"],
                execution_limits
            );
            assert_eq!(
                runtime.global_monty_owner.client().root_identity(),
                root_identity
            );
            let ownership = runtime
                .global_monty_owner
                .ownership_check()
                .snapshot()
                .unwrap();
            assert_eq!(ownership.revision, initial_revision + offset);
            assert_eq!(
                ownership.limits.check_timeout.as_millis() as u64,
                3000 + offset
            );
            assert_eq!(
                ownership.limits.heartbeat_interval.as_millis() as u64,
                2500 + offset
            );
            assert_eq!(ownership.limits.max_pending_checks as u64, 1024 + offset);
            assert_eq!(
                result["runtime"]["execution_limits"]["ownership_effective_revision"],
                ownership.revision
            );
            let bounds = runtime.global_monty_owner.client().vm_bounds();
            assert_eq!(bounds.max_feeds as u64, 128 + offset);
            assert_eq!(
                runtime
                    .global_monty_owner
                    .client()
                    .recipe_context_capacity()
                    .limit as u64,
                128 + offset
            );
            let backlog = runtime.global_monty_owner.client().admission_observation();
            assert_eq!(backlog.limits.max_tasks as u64, 128 + offset);
            assert_eq!(backlog.limits.max_bytes as u64, 67108864 + offset);
            let publications = runtime.global_monty_owner.client().settings_capacity();
            assert_eq!(publications.limit as u64, 16 + offset);
            assert_eq!(
                result["runtime"]["execution_limits"]["limits"]["max_pending_settings"],
                publications.limit
            );
            assert_eq!(
                result["runtime"]["execution_limits"]["queued_tasks"],
                backlog.tasks
            );
            assert_eq!(
                result["runtime"]["execution_limits"]["queued_bytes"],
                backlog.bytes as u64
            );
            assert_eq!(bounds.execution_slice.as_millis() as u64, 5 + offset);
            assert_eq!(
                result["runtime"]["task_budget"]["effective_revision"],
                initial_revision + offset
            );
            assert_eq!(
                result["runtime"]["task_budget"]["max_duration_secs"],
                duration
            );
            let effective = runtime
                .global_monty_owner
                .client()
                .live_task_settings()
                .current();
            assert_eq!(effective.revision, initial_revision + offset);
            assert_eq!(effective.limits.token_budgets_enabled, enabled);
            assert_eq!(effective.limits.max_compute_time.as_secs(), duration);
            let retrieval = runtime
                .monty_settings_owner
                .store()
                .effective_view()
                .get("another-user", "another-project")
                .await
                .unwrap();
            assert_eq!(retrieval.revision, effective.revision);
            assert_eq!(retrieval.token_budgets_enabled, enabled);
            let quote = brassclaw_host_api::ResourceEstimate {
                input_tokens: Some(100),
                ..Default::default()
            };
            let reservation = governor.reserve(budget_scope.clone(), quote);
            if enabled {
                assert!(matches!(
                    reservation,
                    Err(brassclaw_resources::ResourceError::LimitExceeded { .. })
                ));
            } else {
                governor.release(reservation.unwrap().id).unwrap();
            }
            assert_eq!(
                governor
                    .account_snapshot(&budget_account)
                    .unwrap()
                    .unwrap()
                    .limits
                    .unwrap()
                    .max_input_tokens,
                Some(10)
            );
        }
        let (code, shared) = request(
            &app,
            Method::GET,
            "settings-b",
            "/api/settings/monty-vm",
            serde_json::Value::Null,
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(
            shared["settings"]["revision"],
            initial_revision + 2,
            "settings must be instance-wide"
        );
        let (code, _) = request(
            &app,
            Method::PUT,
            "settings-b",
            "/api/settings/monty-vm",
            serde_json::json!({"expected_revision":initial_revision,"max_duration_secs":300}),
        )
        .await;
        assert_eq!(code, StatusCode::CONFLICT);
        // Persist without any browser waiter/notification. The runtime owner
        // must still reconcile this durable edit through its bounded source lane.
        let durable = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
            rig.pool.clone(),
            "default",
            "default",
        );
        let update = serde_json::from_value(
            serde_json::json!({"expected_revision":initial_revision+2,"max_duration_secs":777}),
        )
        .unwrap();
        let desired = durable.upsert("default", "default", &update).await.unwrap();
        let mut changed = runtime
            .global_monty_owner
            .client()
            .live_task_settings()
            .subscribe();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let current = changed.borrow_and_update().revision;
                if current == desired.revision {
                    break;
                }
                changed.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        release.add_permits(1);
        let reply = sending.await.unwrap().unwrap();
        assert_eq!(reply.status, TurnStatus::Completed);
        let ledger = governor
            .account_snapshot(&budget_account)
            .unwrap()
            .unwrap()
            .ledger;
        assert_eq!(ledger.spent.input_tokens, 200);
        assert_eq!(ledger.spent.output_tokens, 15);
        assert!(ledger.spent.usd > rust_decimal::Decimal::ZERO);
        assert_eq!(ledger.reserved.input_tokens, 0);
        let client = rig.pool.get().await.unwrap();
        let row = client.query_one("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
            &[&reply.run_id.as_uuid()]).await.unwrap();
        let first: serde_json::Value = row.get(0);
        assert_eq!(
            first["execution"]["accounting"]["effective_revision"],
            desired.revision
        );
        drop(client);
        let matched = runtime
            .send_user_message(&conversation, "reply after the live edit")
            .await
            .unwrap();
        assert_eq!(matched.status, TurnStatus::Completed);
        let client = rig.pool.get().await.unwrap();
        let row = client.query_one("SELECT outcome FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND phase='settled'",
            &[&matched.run_id.as_uuid()]).await.unwrap();
        let second: serde_json::Value = row.get(0);
        assert_eq!(
            first["execution"]["root"]["vm_id"],
            second["execution"]["root"]["vm_id"]
        );
        drop(client);
        // Exercise the optional controller through the same live HTTP owner.
        // It samples once on activation, then no more than once per interval;
        // returning to startup mode takes one sizing and stops measurements.
        let mut policy = initial["settings"]["memory_policy"].clone();
        policy["mode"] = serde_json::json!("automatic");
        // A ten-minute durable-settings poll must not postpone the independent
        // real sixty-second Automatic sample. Operator saves still wake it.
        execution_limits["settings_reconcile_interval_millis"] = serde_json::json!(600_000);
        let (code, automatic) = request(
            &app,
            Method::PUT,
            "settings-a",
            "/api/settings/monty-vm",
            serde_json::json!({"expected_revision":desired.revision,"memory_policy":policy,
                "execution_limits":execution_limits}),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{automatic}");
        let samples = automatic["runtime"]["memory_budget"]["memory_sample_count"]
            .as_u64()
            .unwrap();
        assert_eq!(samples, boot_samples + 1);
        assert_eq!(automatic["runtime"]["memory_budget"]["mode"], "automatic");
        assert_eq!(
            automatic["runtime"]["execution_limits"]["limits"]["settings_reconcile_interval_millis"],
            600_000
        );
        let store = runtime.webui_monty_settings_store();
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(
            store
                .runtime_status()
                .await
                .unwrap()
                .unwrap()
                .memory_budget
                .unwrap()
                .memory_sample_count,
            samples
        );
        tokio::time::timeout(Duration::from_secs(65), async {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let budget = store
                    .runtime_status()
                    .await
                    .unwrap()
                    .unwrap()
                    .memory_budget
                    .unwrap();
                if budget.memory_sample_count != samples {
                    assert_eq!(budget.memory_sample_count, samples + 1);
                    break;
                }
            }
        })
        .await
        .unwrap();
        policy["mode"] = serde_json::json!("startup");
        let (code, startup) = request(
            &app,
            Method::PUT,
            "settings-a",
            "/api/settings/monty-vm",
            serde_json::json!({"expected_revision":desired.revision + 1,"memory_policy":policy}),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{startup}");
        assert_eq!(startup["runtime"]["memory_budget"]["mode"], "startup");
        assert_eq!(
            startup["runtime"]["memory_budget"]["memory_sample_count"],
            samples + 2
        );
        assert_eq!(
            runtime.global_monty_owner.client().root_identity(),
            root_identity
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(
            store
                .runtime_status()
                .await
                .unwrap()
                .unwrap()
                .memory_budget
                .unwrap()
                .memory_sample_count,
            samples + 2
        );
        let store = runtime.webui_monty_settings_store();
        let status = store.runtime_status().await.unwrap().unwrap();
        assert_eq!(
            status.task_budget.unwrap().uptake,
            MontyBudgetUptake::Applied
        );
        drop(app);
        Arc::try_unwrap(runtime)
            .ok()
            .expect("runtime references released")
            .shutdown()
            .await
            .unwrap();
        assert_eq!(
            store
                .runtime_observation(&desired)
                .unwrap()
                .task_budget
                .unwrap()
                .uptake,
            MontyBudgetUptake::Failed
        );
        let input = RebornRuntimeInput::from_services(
            rig.build_input("live-settings-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_model_gateway_override(Arc::new(RecordingGateway {
            reply: "restart did not request inference".into(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        }));
        let restarted = build_reborn_runtime(input).await.unwrap();
        let effective = restarted
            .global_monty_owner
            .client()
            .live_task_settings()
            .current();
        assert_eq!(effective.revision, desired.revision + 2);
        assert_eq!(effective.limits.max_compute_time.as_secs(), 777);
        restarted.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_exposes_host_runtime_capabilities_to_model_calls() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(ToolCallingGateway::default());
        let gateway_for_runtime: Arc<dyn HostManagedModelGateway> = gateway.clone();
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
            .await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-tools-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-tools-tenant".to_string(),
            agent_id: "runtime-tools-agent".to_string(),
            source_binding_id: "runtime-tools-source".to_string(),
            reply_target_binding_id: "runtime-tools-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway_for_runtime);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let conversation = runtime.new_conversation().await.expect("conversation");
        let reply = tokio::time::timeout(
            RUNTIME_SEND_TIMEOUT,
            runtime.send_user_message(&conversation, "use echo tool"),
        )
        .await
        .expect("runtime send should finish")
        .expect("runtime send should succeed");

        assert_eq!(reply.status, TurnStatus::Completed);
        assert_eq!(reply.text.as_deref(), Some("tool ok"));
        assert_eq!(
            *gateway
                .stream_model_calls
                .lock()
                .expect("tool gateway stream count lock poisoned"),
            0,
            "runtime should use capability-aware model path"
        );
        assert_eq!(
            gateway
                .requests
                .lock()
                .expect("tool gateway requests lock poisoned")
                .len(),
            2,
            "tool call should require initial request plus tool-result follow-up"
        );
        let history = runtime
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: runtime.thread_scope.clone(),
                thread_id: conversation.0.clone(),
            })
            .await
            .expect("thread history");
        let tool_result = history
            .messages
            .iter()
            .find(|message| message.kind == MessageKind::ToolResultReference)
            .expect("tool result reference should persist in thread history");
        assert!(
            tool_result
                .tool_result_ref
                .as_deref()
                .is_some_and(|result_ref| result_ref.starts_with("result:")),
            "tool result should persist a durable result ref"
        );
        assert!(
            tool_result.tool_result_provider_call.is_none(),
            "product thread history should scrub provider replay metadata"
        );
        let context = runtime
            .thread_service
            .load_context_messages(LoadContextMessagesRequest {
                scope: runtime.thread_scope.clone(),
                thread_id: conversation.0.clone(),
                message_ids: vec![tool_result.message_id],
            })
            .await
            .expect("tool result context");
        let provider_call = context.messages[0]
            .tool_result_provider_call
            .as_ref()
            .expect("model context should preserve provider replay metadata");
        assert_eq!(provider_call.provider_call_id, "call-1");
        assert_eq!(
            provider_call.capability_id,
            CapabilityId::new("builtin.echo").unwrap()
        );

        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_maps_workspace_to_configured_root() {
        let root = tempfile::tempdir().expect("tempdir");
        let workspace_root = tempfile::tempdir().expect("workspace tempdir");
        std::fs::write(
            workspace_root.path().join("workspace-sentinel.txt"),
            "visible through /workspace",
        )
        .expect("write sentinel");
        let gateway = Arc::new(WorkspaceListingGateway::default());
        let gateway_for_runtime: Arc<dyn HostManagedModelGateway> = gateway.clone();
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-workspace-owner", root.path())
                .with_local_dev_workspace_root(workspace_root.path().to_path_buf())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-workspace-tenant".to_string(),
            agent_id: "runtime-workspace-agent".to_string(),
            source_binding_id: "runtime-workspace-source".to_string(),
            reply_target_binding_id: "runtime-workspace-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: RUNTIME_SEND_TIMEOUT,
        })
        .with_model_gateway_override(gateway_for_runtime);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let conversation = runtime.new_conversation().await.expect("conversation");
        let reply = tokio::time::timeout(
            RUNTIME_SEND_TIMEOUT,
            runtime.send_user_message(&conversation, "list workspace"),
        )
        .await
        .expect("runtime send should finish")
        .expect("runtime send should succeed");

        assert_eq!(reply.status, TurnStatus::Completed);
        assert_eq!(reply.text.as_deref(), Some("workspace ok"));
        let request_count = {
            let requests = gateway
                .requests
                .lock()
                .expect("workspace gateway requests lock poisoned");
            requests.len()
        };
        assert_eq!(
            request_count, 2,
            "workspace listing should require initial request plus tool-result follow-up"
        );

        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_runtime_webui_bundle_reuses_thread_and_turn_facades() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "webui projection ok".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-webui-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-webui-tenant".to_string(),
            agent_id: "runtime-webui-agent".to_string(),
            source_binding_id: "runtime-webui-source".to_string(),
            reply_target_binding_id: "runtime-webui-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let runtime_turn_coordinator = runtime.webui_turn_coordinator();
        let bundle = build_webui_services(Arc::clone(&runtime), None)
            .await
            .expect("webui bundle");
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("runtime-webui-tenant").unwrap(),
            UserId::new("runtime-webui-owner").unwrap(),
            Some(AgentId::new("runtime-webui-agent").unwrap()),
            None,
        );
        let created = bundle
            .api
            .create_thread(
                caller.clone(),
                WebUiCreateThreadRequest {
                    client_action_id: Some("create-webui-stream-thread".to_string()),
                    requested_thread_id: None,
                },
            )
            .await
            .expect("create webui thread");
        let submitted = bundle
            .api
            .submit_turn(
                caller.clone(),
                WebUiSendMessageRequest {
                    client_action_id: Some("send-webui-stream-message".to_string()),
                    thread_id: Some(created.thread.thread_id.to_string()),
                    content: Some("hello webui stream".to_string()),
                },
            )
            .await
            .expect("submit webui turn");
        let RebornSubmitTurnResponse::Submitted { run_id, .. } = submitted else {
            panic!("webui submit should start a run");
        };
        let stream = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let stream = bundle
                    .api
                    .stream_events(
                        caller.clone(),
                        RebornStreamEventsRequest {
                            thread_id: created.thread.thread_id.to_string(),
                            after_cursor: None,
                        },
                    )
                    .await
                    .expect("webui event stream");
                if stream.events.iter().any(|event| {
                    matches!(
                        event.payload(),
                        ProductOutboundPayload::ProjectionSnapshot { state }
                            | ProductOutboundPayload::ProjectionUpdate { state }
                            if state.items.iter().any(|item| matches!(
                                item,
                                ProductProjectionItem::RunStatus {
                                    run_id: seen,
                                    status,
                                    ..
                                }
                                    if *seen == run_id && status == "completed"
                            ))
                    )
                }) {
                    break stream;
                }
                let state = runtime_turn_coordinator
                    .get_run_state(GetRunStateRequest {
                        scope: caller.turn_scope(created.thread.thread_id.clone()),
                        run_id,
                    })
                    .await
                    .expect("submitted webui run state");
                assert!(
                    !state.status.is_terminal() || state.status == TurnStatus::Completed,
                    "webui run ended before its completed projection: status={:?}, category={:?}",
                    state.status,
                    state.failure.as_ref().map(|failure| failure.category())
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("completed webui projection should appear");

        let _api = bundle.api.clone();
        assert!(Arc::ptr_eq(
            &runtime_turn_coordinator,
            &runtime.webui_turn_coordinator()
        ));
        assert!(
            stream.events.iter().all(|event| matches!(
                event.payload(),
                ProductOutboundPayload::CapabilityActivity(_)
                    | ProductOutboundPayload::CapabilityDisplayPreview(_)
                    | ProductOutboundPayload::ProjectionSnapshot { .. }
                    | ProductOutboundPayload::ProjectionUpdate { .. }
            )),
            "webui bundle should expose only projection stream events"
        );
        assert_eq!(bundle.readiness, runtime.services().readiness);
        assert_eq!(bundle.readiness.state, RebornReadinessState::DevOnly);

        drop(_api);
        drop(bundle);
        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_webui_bundle_uses_local_lifecycle_facade_for_setup_extension() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "webui lifecycle ok".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-webui-lifecycle-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-webui-lifecycle-tenant".to_string(),
            agent_id: "runtime-webui-lifecycle-agent".to_string(),
            source_binding_id: "runtime-webui-lifecycle-source".to_string(),
            reply_target_binding_id: "runtime-webui-lifecycle-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let bundle = build_webui_services(Arc::clone(&runtime), None)
            .await
            .expect("webui bundle");
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("runtime-webui-lifecycle-tenant").unwrap(),
            UserId::new("runtime-webui-lifecycle-owner").unwrap(),
            Some(AgentId::new("runtime-webui-lifecycle-agent").unwrap()),
            None,
        );

        // Verify the lifecycle facade is wired by testing setup_extension for a
        // first-party extension with no credentials (web-access).
        let web_access_setup = bundle
            .api
            .setup_extension(
                caller.clone(),
                LifecyclePackageRef::new(LifecyclePackageKind::Extension, "web-access")
                    .expect("valid package ref"),
                WebUiSetupExtensionRequest::default(),
            )
            .await
            .expect("setup extension lifecycle projection");

        assert_eq!(web_access_setup.package_ref.id.as_str(), "web-access");
        assert_eq!(web_access_setup.phase, LifecyclePhase::Discovered);
        assert!(web_access_setup.secrets.is_empty());
        assert!(
            matches!(
                web_access_setup.payload.as_ref(),
                Some(LifecycleProductPayload::ExtensionList { extensions, count })
                    if *count == 1
                        && extensions.len() == 1
                        && extensions[0].summary.package_ref.id.as_str() == "web-access"
            ),
            "local webui bundle should use the local lifecycle facade package projection"
        );
        assert!(
            !web_access_setup.blockers.iter().any(|blocker| matches!(
                blocker,
                LifecycleReadinessBlocker::Runtime { ref_id: Some(ref_id) }
                    if ref_id.as_str() == "reborn_lifecycle_facade_unwired"
            )),
            "local webui bundle must not fall back to the default unwired facade"
        );

        // Also verify the google-calendar OAuth credential setup shape.
        let google_setup = bundle
            .api
            .setup_extension(
                caller.clone(),
                LifecyclePackageRef::new(LifecyclePackageKind::Extension, "google-calendar")
                    .expect("valid package ref"),
                WebUiSetupExtensionRequest::default(),
            )
            .await
            .expect("google setup extension lifecycle projection");
        assert_eq!(google_setup.secrets.len(), 2);
        let google_oauth_setups = google_setup
            .secrets
            .iter()
            .map(|secret| {
                assert_eq!(secret.provider, "google");
                assert!(!secret.provided);
                match &secret.setup {
                    RebornExtensionCredentialSetup::OAuth {
                        account_label,
                        scopes,
                        ..
                    } => (account_label.clone(), scopes.clone()),
                    RebornExtensionCredentialSetup::ManualToken => {
                        panic!("Google setup secret should use OAuth")
                    }
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            google_oauth_setups
                .iter()
                .map(|(_, scopes)| scopes.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![GOOGLE_CALENDAR_READONLY_SCOPE.to_string()],
                vec![GOOGLE_CALENDAR_EVENTS_SCOPE.to_string()],
            ]
        );
        let google_setup_json =
            serde_json::to_value(&google_setup.secrets[0]).expect("serialize setup secret");
        assert_eq!(google_setup_json["setup"]["kind"], "oauth");

        drop(bundle);
        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn webui_route_rejects_list_automations_without_agent_binding() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use brassclaw_webui_v2::{WebUiV2State, webui_v2_router};
        use tower::ServiceExt;

        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "unused".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-webui-no-agent-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-webui-no-agent-tenant".to_string(),
            agent_id: "runtime-webui-no-agent-agent".to_string(),
            source_binding_id: "runtime-webui-no-agent-source".to_string(),
            reply_target_binding_id: "runtime-webui-no-agent-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway);

        let mut runtime = build_reborn_runtime(input).await.expect("runtime builds");
        runtime.services.host_runtime = None;
        let runtime = Arc::new(runtime);
        let bundle = build_webui_services(Arc::clone(&runtime), None)
            .await
            .expect("webui bundle");
        let caller_without_agent = WebUiAuthenticatedCaller::new(
            TenantId::new("runtime-webui-no-agent-tenant").unwrap(),
            UserId::new("runtime-webui-no-agent-owner").unwrap(),
            None,
            None,
        );
        let router = webui_v2_router(WebUiV2State::new(bundle.api))
            .layer(axum::Extension(caller_without_agent));

        let response = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/webchat/v2/automations")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("route response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let runtime = match Arc::try_unwrap(runtime) {
            Ok(runtime) => runtime,
            Err(_) => panic!("webui router retained the runtime after being dropped"),
        };
        runtime.shutdown().await.expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn build_webui_services_without_host_runtime_returns_503_on_list_automations() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "unused".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-webui-no-host-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-webui-no-host-tenant".to_string(),
            agent_id: "runtime-webui-no-host-agent".to_string(),
            source_binding_id: "runtime-webui-no-host-source".to_string(),
            reply_target_binding_id: "runtime-webui-no-host-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway);

        let mut runtime = build_reborn_runtime(input).await.expect("runtime builds");
        runtime.services.host_runtime = None;
        let runtime = Arc::new(runtime);
        let bundle = build_webui_services(Arc::clone(&runtime), None)
            .await
            .expect("webui bundle");
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("runtime-webui-no-host-tenant").unwrap(),
            UserId::new("runtime-webui-no-host-owner").unwrap(),
            Some(AgentId::new("runtime-webui-no-host-agent").unwrap()),
            None,
        );

        let error = bundle
            .api
            .list_automations(caller, WebUiListAutomationsRequest::default())
            .await
            .expect_err("missing host runtime should leave automation facade unavailable");

        assert_eq!(error.code, RebornServicesErrorCode::Unavailable);
        assert_eq!(error.kind, RebornServicesErrorKind::ServiceUnavailable);
        assert_eq!(error.status_code, 503);
        assert!(error.retryable);
        drop(bundle);
        let runtime = match Arc::try_unwrap(runtime) {
            Ok(runtime) => runtime,
            Err(_) => panic!("webui services retained the runtime after being dropped"),
        };
        runtime.shutdown().await.expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_webui_bundle_routes_approval_gates_into_interaction_service() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "unused".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-webui-approval-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-webui-approval-tenant".to_string(),
            agent_id: "runtime-webui-approval-agent".to_string(),
            source_binding_id: "runtime-webui-approval-source".to_string(),
            reply_target_binding_id: "runtime-webui-approval-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let bundle = build_webui_services(Arc::clone(&runtime), None)
            .await
            .expect("webui bundle");
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("runtime-webui-approval-tenant").unwrap(),
            UserId::new("runtime-webui-approval-owner").unwrap(),
            Some(AgentId::new("runtime-webui-approval-agent").unwrap()),
            None,
        );
        let created = bundle
            .api
            .create_thread(
                caller.clone(),
                WebUiCreateThreadRequest {
                    client_action_id: Some("create-webui-approval-thread".to_string()),
                    requested_thread_id: None,
                },
            )
            .await
            .expect("create thread");
        let gate_ref = approval_gate_ref(ApprovalRequestId::new()).expect("approval gate");

        let err = bundle
            .api
            .resolve_gate(
                caller,
                WebUiResolveGateRequest {
                    client_action_id: Some("resolve-webui-approval-gate".to_string()),
                    thread_id: Some(created.thread.thread_id.to_string()),
                    run_id: Some(TurnRunId::new().to_string()),
                    gate_ref: Some(gate_ref.as_str().to_string()),
                    resolution: Some("approved".to_string()),
                    always: None,
                    credential_ref: None,
                },
            )
            .await
            .expect_err("missing approval gate should reach approval interaction service");

        assert_eq!(err.code, RebornServicesErrorCode::NotFound);
        assert_eq!(err.kind, RebornServicesErrorKind::NotFound);
        assert_eq!(err.status_code, 404);
        drop(bundle);
        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_webui_bundle_routes_auth_gates_into_interaction_service() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "unused".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-webui-auth-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-webui-auth-tenant".to_string(),
            agent_id: "runtime-webui-auth-agent".to_string(),
            source_binding_id: "runtime-webui-auth-source".to_string(),
            reply_target_binding_id: "runtime-webui-auth-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        let bundle = build_webui_services(Arc::clone(&runtime), None)
            .await
            .expect("webui bundle");
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("runtime-webui-auth-tenant").unwrap(),
            UserId::new("runtime-webui-auth-owner").unwrap(),
            Some(AgentId::new("runtime-webui-auth-agent").unwrap()),
            None,
        );
        let created = bundle
            .api
            .create_thread(
                caller.clone(),
                WebUiCreateThreadRequest {
                    client_action_id: Some("create-webui-auth-thread".to_string()),
                    requested_thread_id: None,
                },
            )
            .await
            .expect("create thread");

        let err = bundle
            .api
            .resolve_gate(
                caller,
                WebUiResolveGateRequest {
                    client_action_id: Some("resolve-webui-auth-gate".to_string()),
                    thread_id: Some(created.thread.thread_id.to_string()),
                    run_id: Some(TurnRunId::new().to_string()),
                    gate_ref: Some("gate:hook-auth-missing".to_string()),
                    resolution: Some("denied".to_string()),
                    always: None,
                    credential_ref: None,
                },
            )
            .await
            .expect_err("missing auth gate should reach auth interaction service");

        assert_eq!(err.code, RebornServicesErrorCode::NotFound);
        assert_eq!(err.kind, RebornServicesErrorKind::BlockedAuthentication);
        assert_eq!(err.status_code, 404);
        drop(bundle);
        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_dev_webui_spawn_approval_emits_redacted_audit_and_grants_process() {
        let root = tempfile::tempdir().expect("tempdir");
        let gateway = Arc::new(RecordingGateway {
            reply: "unused".to_string(),
            requests: Arc::new(StdMutex::new(Vec::new())),
        });
        let rig = super::test_pg::pg_rig().await;
        let _db_guard = rig.lock_db().await;
        let input = RebornRuntimeInput::from_services(
            rig.build_input("runtime-webui-audit-owner", root.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "runtime-webui-audit-tenant".to_string(),
            agent_id: "runtime-webui-audit-agent".to_string(),
            source_binding_id: "runtime-webui-audit-source".to_string(),
            reply_target_binding_id: "runtime-webui-audit-reply".to_string(),
        })
        .with_poll_settings(PollSettings {
            interval: Duration::from_millis(10),
            max_total: Duration::from_secs(3),
        })
        .with_model_gateway_override(gateway);

        let runtime = Arc::new(build_reborn_runtime(input).await.expect("runtime builds"));
        super::test_pg::stop_worker_for_state_fixture(&runtime).await;
        let bundle = build_webui_services(Arc::clone(&runtime), None)
            .await
            .expect("webui bundle");
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("runtime-webui-audit-tenant").unwrap(),
            UserId::new("runtime-webui-audit-owner").unwrap(),
            Some(AgentId::new("runtime-webui-audit-agent").unwrap()),
            None,
        );
        let created = bundle
            .api
            .create_thread(
                caller.clone(),
                WebUiCreateThreadRequest {
                    client_action_id: Some("create-webui-audit-thread".to_string()),
                    requested_thread_id: None,
                },
            )
            .await
            .expect("create thread");
        let scope = caller.turn_scope(created.thread.thread_id.clone());
        let actor = caller.actor();
        let submitted = runtime
            .turn_coordinator
            .submit_turn(SubmitTurnRequest {
                scope: scope.clone(),
                actor: actor.clone(),
                accepted_message_ref: AcceptedMessageRef::new("msg:audit").unwrap(),
                source_binding_ref: SourceBindingRef::new("src:audit").unwrap(),
                reply_target_binding_ref: ReplyTargetBindingRef::new("reply:audit").unwrap(),
                requested_run_profile: None,
                idempotency_key: IdempotencyKey::new("submit-audit").unwrap(),
                received_at: chrono::Utc::now(),
                requested_run_id: None,
                parent_run_id: None,
                subagent_depth: 0,
                spawn_tree_root_run_id: None,
            })
            .await
            .expect("submit turn");
        let run_id = match submitted {
            SubmitTurnResponse::Accepted { run_id, .. } => run_id,
        };
        // The runtime selected durable stores. The legacy local substrate is
        // not the approval store consumed by the WebUI facade.
        let approvals = brassclaw_approvals::pg_store::PgApprovalRequestStore::new(
            Arc::clone(&rig.pool),
            runtime.thread_scope.tenant_id.as_str(),
        );
        let capability_leases = brassclaw_authorization::PgCapabilityLeaseStore::new(
            Arc::clone(&rig.pool),
            runtime.thread_scope.tenant_id.as_str(),
        );
        let runner_id = TurnRunnerId::new();
        let lease_token = TurnLeaseToken::new();
        let turn_state = brassclaw_turns::PgTurnStateStore::new(
            Arc::clone(&rig.pool),
            runtime.thread_scope.tenant_id.as_str(),
        );
        let claimed = turn_state
            .claim_next_run(ClaimRunRequest {
                runner_id,
                lease_token,
                scope_filter: Some(scope.clone()),
            })
            .await
            .expect("claim run")
            .expect("claimed run");
        assert_eq!(claimed.state.run_id, run_id);
        let request_id = ApprovalRequestId::new();
        let gate_ref = approval_gate_ref(request_id).expect("approval gate");
        turn_state
            .block_run(BlockRunRequest {
                run_id,
                runner_id,
                lease_token,
                checkpoint_id: TurnCheckpointId::new(),
                state_ref: LoopCheckpointStateRef::new("checkpoint:audit").unwrap(),
                reason: BlockedReason::Approval {
                    gate_ref: gate_ref.clone(),
                },
            })
            .await
            .expect("block approval");
        let resource_scope = ResourceScope {
            tenant_id: scope.tenant_id.clone(),
            user_id: actor.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
            thread_id: Some(scope.thread_id.clone()),
            invocation_id: InvocationId::new(),
        };
        let capability = CapabilityId::new("demo.echo").expect("capability");
        let mut approval = ApprovalRequest {
            id: request_id,
            correlation_id: CorrelationId::new(),
            requested_by: Principal::User(actor.user_id.clone()),
            action: Box::new(Action::SpawnCapability {
                capability: capability.clone(),
                estimated_resources: ResourceEstimate::default(),
            }),
            invocation_fingerprint: None,
            reason: "raw /Users/alice/private token sk-live".to_string(),
            reusable_scope: None,
        };
        approval.invocation_fingerprint = Some(
            InvocationFingerprint::for_spawn(
                &resource_scope,
                &capability,
                &ResourceEstimate::default(),
                &serde_json::json!({"secret": "hidden"}),
            )
            .expect("fingerprint"),
        );
        // Match the host invocation lifecycle: approvals reference an existing
        // capability invocation, independently of the conversation turn run.
        let invocations = brassclaw_run_state::pg_store::PgRunStateStore::new(
            Arc::clone(&rig.pool),
            runtime.thread_scope.tenant_id.as_str(),
        );
        brassclaw_run_state::RunStateStore::start(
            &invocations,
            brassclaw_run_state::RunStart {
                invocation_id: resource_scope.invocation_id,
                capability_id: capability.clone(),
                scope: resource_scope.clone(),
            },
        )
        .await
        .expect("start durable capability invocation");
        approvals
            .save_pending(resource_scope.clone(), approval)
            .await
            .expect("save approval");

        let pending = runtime
            .approval_interaction_service
            .list_pending(brassclaw_product_workflow::ListPendingApprovalsRequest {
                scope: scope.clone(),
                actor: actor.clone(),
            })
            .await
            .expect("durable pending approvals");
        assert_eq!(pending.approvals.len(), 1);
        assert_eq!(pending.approvals[0].run_id, run_id);
        assert_eq!(pending.approvals[0].gate_ref, gate_ref);
        let wrong_actor = runtime
            .approval_interaction_service
            .list_pending(brassclaw_product_workflow::ListPendingApprovalsRequest {
                scope: scope.clone(),
                actor: TurnActor::new(UserId::new("another-owner").unwrap()),
            })
            .await
            .expect_err("a different actor must not list an explicitly owned task's approvals");
        assert!(matches!(
            wrong_actor,
            brassclaw_product_workflow::ProductWorkflowError::ApprovalInteractionRejected {
                kind:
                    brassclaw_product_workflow::ApprovalInteractionRejectionKind::CrossScopeDenied,
            }
        ));

        bundle
            .api
            .resolve_gate(
                caller,
                WebUiResolveGateRequest {
                    client_action_id: Some("resolve-webui-audit-gate".to_string()),
                    thread_id: Some(scope.thread_id.to_string()),
                    run_id: Some(run_id.to_string()),
                    gate_ref: Some(gate_ref.as_str().to_string()),
                    resolution: Some("approved".to_string()),
                    always: None,
                    credential_ref: None,
                },
            )
            .await
            .expect("resolve approval gate");

        let records = runtime.webui_approval_audit_sink().records();
        assert_eq!(records.len(), 1);
        let serialized = serde_json::to_string(&records[0]).expect("serialize audit");
        for forbidden in ["/Users/alice/private", "sk-live", "hidden", "sha256:"] {
            assert!(
                !serialized.contains(forbidden),
                "approval audit leaked {forbidden}: {serialized}"
            );
        }
        let leases = capability_leases.leases_for_scope(&resource_scope).await;
        assert_eq!(leases.len(), 1);
        assert!(
            leases[0]
                .grant
                .constraints
                .allowed_effects
                .contains(&EffectKind::SpawnProcess)
        );
        drop(bundle);
        shutdown_shared_runtime(runtime)
            .await
            .expect("runtime shutdown");
    }
}
