//! v3 Phase H.12.2 — production `EffectExecutor` adapter (engine Monty VM →
//! `HostRuntime`).
//!
//! This module hosts the composition-owned bridge that lets the engine Tier-0
//! Monty sandbox (`execute_tier_zero_channel` → `handle_execute_action` →
//! `effects.execute_action`) reach the **production** Rust capability layer
//! (`HostRuntime::invoke_capability`). Before H.12.2 only `#[cfg(test)]`
//! impls of `EffectExecutor` existed anywhere in the workspace; H.12.2
//! introduces the first production impl so Q-H12-3 (every PythonVM calls tools
//! via the Rust execution layer) holds for Tier 0.
//!
//! Crate-boundary discipline: only public / host-API types live here. The
//! per-run `TierZeroEffectExecutorBuilder` — which touches the `pub(super)`
//! local-dev extension surface and `pub(crate)` capability policy — lives in
//! `runtime/local_dev.rs` (where those items are visible) and returns an
//! `Arc<dyn EffectExecutor>` built from the types defined here.
//!
//! `dead_code` is allowed module-wide until the adapter is wired in H.12.2.5;
//! the `#![allow(dead_code)]` is removed once the builder constructs these
//! types in production.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use brassclaw_engine::types::capability::{
    ActionDef, CapabilityStatus, CapabilitySummary, CapabilitySummaryKind, ModelToolSurface,
};
use brassclaw_engine::{
    ActionResult, CapabilityLease, EffectExecutor, EngineError, ThreadExecutionContext,
};
use brassclaw_host_api::{
    AgentId, CapabilityId, CapabilitySet, EffectKind, ExecutionContext, ExtensionId, MountView,
    PermissionMode, ProjectId, ResourceEstimate, RuntimeKind, TenantId, ThreadId, TrustClass,
    UserId,
};
use brassclaw_host_runtime::{
    CapabilitySurfacePolicy, HostRuntime, HostRuntimeError, RuntimeCapabilityOutcome,
    RuntimeCapabilityRequest, RuntimeFailureKind, SurfaceKind, VisibleCapability,
    VisibleCapabilityAccess, VisibleCapabilityRequest, VisibleCapabilitySurface,
};
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use serde_json::{Value, json};

/// Composition-owned factory that builds a production host
/// [`ExecutionContext`] for a single Tier-0 `__execute_action__` call from an
/// engine [`ThreadExecutionContext`] + composition-held, per-run scope.
///
/// The engine context carries `user_id` / `project_id` / `thread_id` but
/// **lacks** `tenant_id` / `agent_id` / `extension_id` / `grants` / `mounts` /
/// `trust` / `resource_scope` (`traits/effect.rs:23`). Those are resolved per
/// `run_tier_zero` turn by the `TierZeroEffectExecutorBuilder`
/// (`runtime/local_dev.rs`): the extension id comes from
/// `loop_driver_execution_extension_id(run_context)`, tenant/agent from
/// `TurnScope`, and grants from `LocalDevCapabilityPolicy::builtin_grants`
/// plus the snapshotted extension surface. The resolved values are baked into
/// this factory, so [`TierZeroExecutionContextFactory::build`] is a pure,
/// **sync** engine→host-api projection — the extension surface is snapshotted
/// once per run (Q-H12-2-SNAP = A), not per action.
///
/// `context.mounts` is intentionally [`MountView::default`]: the production
/// `LoopCapabilityPort` rejects caller-supplied mounts as `Unauthorized`
/// (`capability_port.rs:796`); workspace/skill/memory mounts reach capabilities
/// via the grant constraints that `builtin_grants` already bakes, not via the
/// context mount view.
#[derive(Clone)]
pub(crate) struct TierZeroExecutionContextFactory {
    tenant_id: TenantId,
    agent_id: Option<AgentId>,
    extension_id: ExtensionId,
    grants: CapabilitySet,
}

impl TierZeroExecutionContextFactory {
    pub(crate) fn new(
        tenant_id: TenantId,
        agent_id: Option<AgentId>,
        extension_id: ExtensionId,
        grants: CapabilitySet,
    ) -> Self {
        Self {
            tenant_id,
            agent_id,
            extension_id,
            grants,
        }
    }

    /// Build a production host [`ExecutionContext`] for one Tier-0 action call.
    ///
    /// `user_id` / `project_id` / `thread_id` are projected from the engine
    /// context (engine uuid ids → host-api validated string ids). The engine
    /// `user_id` is converted **fail-closed**: the engine context always
    /// carries a `user_id` (it is a `String`, not an `Option`), so an invalid
    /// value is corruption rather than a missing user — mapping it to a
    /// fallback user would misattribute the action, so [`EngineError::Effect`]
    /// is returned instead and `execute_tier_zero_channel` degrades to Tier 2.
    /// `tenant_id` / `agent_id` / `extension_id` / `grants` come from the
    /// per-run config held at construction. Mirrors
    /// `local_dev_visible_capability_request` (`runtime/local_dev.rs:732`):
    /// `ExecutionContext::local_default` then override tenant/agent/project/
    /// thread + the matching `resource_scope.*` fields, then `validate()`.
    pub(crate) fn build(
        &self,
        engine_ctx: &ThreadExecutionContext,
    ) -> Result<ExecutionContext, EngineError> {
        let user_id =
            UserId::new(engine_ctx.user_id.as_str()).map_err(|e| EngineError::Effect {
                reason: format!("tier-zero user id is not a valid host scope id: {e}"),
            })?;
        let mut context = ExecutionContext::local_default(
            user_id,
            self.extension_id.clone(),
            RuntimeKind::FirstParty,
            TrustClass::UserTrusted,
            self.grants.clone(),
            MountView::default(),
        )
        .map_err(|e| EngineError::Effect {
            reason: format!("tier-zero execution context build failed: {e}"),
        })?;
        context.tenant_id = self.tenant_id.clone();
        context.agent_id = self.agent_id.clone();
        context.project_id = Some(ProjectId::new(engine_ctx.project_id.0.to_string()).map_err(
            |e| EngineError::Effect {
                reason: format!("tier-zero project id conversion failed: {e}"),
            },
        )?);
        context.thread_id = Some(ThreadId::new(engine_ctx.thread_id.0.to_string()).map_err(
            |e| EngineError::Effect {
                reason: format!("tier-zero thread id conversion failed: {e}"),
            },
        )?);
        context.resource_scope.tenant_id = context.tenant_id.clone();
        context.resource_scope.agent_id = context.agent_id.clone();
        context.resource_scope.project_id = context.project_id.clone();
        context.resource_scope.thread_id = context.thread_id.clone();
        context.validate().map_err(|e| EngineError::Effect {
            reason: format!("tier-zero execution context invalid: {e}"),
        })?;
        Ok(context)
    }
}

/// Seam that translates an engine `action_name` (the string the Monty VM
/// passes to `EffectExecutor::execute_action`) into a validated host
/// [`CapabilityId`] for `HostRuntime::invoke_capability`.
///
/// Today the mapping is 1:1 — the engine `action_name` *is* the capability id
/// string (e.g. `shell.exec`, `github.issues.search`), so the default
/// [`TierZeroActionRegistry`] just validates and forwards. The trait exists so
/// a future non-1:1 resolver (alias maps, short-name expansion, capability
/// re-prefixing) can be swapped in behind the `ProductionEffectExecutor`
/// without touching the executor body — the executor holds an
/// `Arc<dyn TierZeroActionResolver>`.
pub(crate) trait TierZeroActionResolver: Send + Sync {
    /// Resolve an engine `action_name` to a host [`CapabilityId`].
    ///
    /// Returns [`EngineError::Effect`] (→ Tier-2 degrade) on any invalid
    /// name, mirroring the factory's fail-closed projection: an action name
    /// that is not a valid `<extension>.<capability>[.<sub>...]` id is
    /// malformed input, not a recoverable capability miss.
    fn resolve(&self, action_name: &str) -> Result<CapabilityId, EngineError>;
}

/// Default 1:1 action resolver: validates the `action_name` as a
/// [`CapabilityId`] and forwards it unchanged.
#[derive(Clone, Default)]
pub(crate) struct TierZeroActionRegistry;

impl TierZeroActionRegistry {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl TierZeroActionResolver for TierZeroActionRegistry {
    fn resolve(&self, action_name: &str) -> Result<CapabilityId, EngineError> {
        CapabilityId::new(action_name).map_err(|e| EngineError::Effect {
            reason: format!("tier-zero action name is not a valid capability id: {e}"),
        })
    }
}

/// Production [`EffectExecutor`] adapter: bridges the engine Tier-0 Monty
/// `__execute_action__` channel to `HostRuntime::invoke_capability`.
///
/// Holds a long-lived `Arc<dyn HostRuntime>` plus the per-run
/// [`TierZeroExecutionContextFactory`], [`TierZeroActionResolver`], and
/// visibility config (`surface_kind` / `provider_trust` / `visibility_policy`)
/// baked by `TierZeroEffectExecutorBuilder::build_for_run` (H.12.2.5).
/// [`dispatch_action`] is the real engine `execute_action` body; the
/// `impl EffectExecutor` block (H.12.2.4) delegates to it and adds the
/// `available_*` projections, which drive the visibility config through
/// `HostRuntime::visible_capabilities`.
///
/// The visibility config is held per-run (Q-H12-2-SNAP = A): the extension
/// surface — including `provider_trust` — is snapshotted once per
/// `run_tier_zero` by the H.12.2.5 builder, not per action, so every
/// `available_*` call within a run sees a coherent surface.
///
/// `dead_code` is allowed module-wide until the adapter is wired in H.12.2.5.
pub(crate) struct ProductionEffectExecutor {
    runtime: Arc<dyn HostRuntime>,
    context_factory: Arc<TierZeroExecutionContextFactory>,
    action_resolver: Arc<dyn TierZeroActionResolver>,
    surface_kind: SurfaceKind,
    provider_trust: BTreeMap<VisibleCapabilityProvider, TrustDecision>,
    visibility_policy: CapabilitySurfacePolicy,
}

/// Key under which a capability provider's trust decision is held in the
/// per-run visibility snapshot. This is a type alias for [`ExtensionId`]
/// (the host-runtime `VisibleCapabilityRequest::provider_trust` is keyed by
/// `ExtensionId`) kept to make the executor field self-documenting.
pub(crate) type VisibleCapabilityProvider = ExtensionId;

impl ProductionEffectExecutor {
    pub(crate) fn new(
        runtime: Arc<dyn HostRuntime>,
        context_factory: Arc<TierZeroExecutionContextFactory>,
        action_resolver: Arc<dyn TierZeroActionResolver>,
        surface_kind: SurfaceKind,
        provider_trust: BTreeMap<VisibleCapabilityProvider, TrustDecision>,
        visibility_policy: CapabilitySurfacePolicy,
    ) -> Self {
        Self {
            runtime,
            context_factory,
            action_resolver,
            surface_kind,
            provider_trust,
            visibility_policy,
        }
    }

    /// Real `EffectExecutor::execute_action` body.
    ///
    /// The engine has already atomically consumed one lease use via
    /// `LeaseManager::find_and_consume` (`orchestrator.rs:1485`) before
    /// calling this — the adapter MUST NOT consume again. It validates the
    /// lease (valid + covers the action + belongs to this thread) fail-closed,
    /// resolves the action name to a [`CapabilityId`], builds a production
    /// [`ExecutionContext`], dispatches via `HostRuntime::invoke_capability`,
    /// and maps the outcome per Q-H12-2-GATE = A (interim non-resumable:
    /// gate outcomes → `Err(EngineError::Effect)` → Tier-2 degrade).
    pub(crate) async fn dispatch_action(
        &self,
        action_name: &str,
        parameters: Value,
        lease: &CapabilityLease,
        engine_ctx: &ThreadExecutionContext,
    ) -> Result<ActionResult, EngineError> {
        validate_lease(lease, action_name, engine_ctx)?;

        let capability_id = self.action_resolver.resolve(action_name)?;
        let context = self.context_factory.build(engine_ctx)?;

        let request = RuntimeCapabilityRequest::new(
            context,
            capability_id,
            ResourceEstimate::default(),
            parameters,
            tier_zero_trust_decision(),
        );

        let started = Instant::now();
        let outcome = self
            .runtime
            .invoke_capability(request)
            .await
            .map_err(map_host_runtime_error)?;
        let duration = started.elapsed();

        map_capability_outcome(outcome, action_name, engine_ctx, duration)
    }
}

#[async_trait::async_trait]
impl EffectExecutor for ProductionEffectExecutor {
    async fn execute_action(
        &self,
        action_name: &str,
        parameters: Value,
        lease: &CapabilityLease,
        context: &ThreadExecutionContext,
    ) -> Result<ActionResult, EngineError> {
        // Delegate to the real dispatch body implemented in H.12.2.3.
        self.dispatch_action(action_name, parameters, lease, context)
            .await
    }

    async fn available_actions(
        &self,
        leases: &[CapabilityLease],
        context: &ThreadExecutionContext,
    ) -> Result<Vec<ActionDef>, EngineError> {
        let surface = self.visible_capability_surface(context).await?;
        // The callable inventory is the leased subset of the visible surface:
        // an action is callable now only if a valid lease covers its
        // capability id for this thread. `find_lease_for_action` re-checks at
        // execution time, so this filter only narrows what is advertised to
        // the model — it is not an authority boundary.
        Ok(surface
            .capabilities
            .into_iter()
            .filter(|visible| lease_covers_action(leases, visible.descriptor.id.as_str()))
            .map(project_action_def)
            .collect())
    }

    async fn available_capabilities(
        &self,
        _leases: &[CapabilityLease],
        context: &ThreadExecutionContext,
    ) -> Result<Vec<CapabilitySummary>, EngineError> {
        let surface = self.visible_capability_surface(context).await?;
        // `CapabilitySummary` is the background/contextual surface
        // (`types/capability.rs:295`): ready callable actions live in the
        // action inventory; summaries cover visible capabilities regardless of
        // whether the thread currently holds a lease, so the model can see
        // askable/needs-setup capabilities it cannot yet call directly. No
        // lease filter is applied here.
        Ok(surface
            .capabilities
            .into_iter()
            .map(project_capability_summary)
            .collect())
    }
}

impl ProductionEffectExecutor {
    /// Build a `VisibleCapabilityRequest` from the per-run visibility config
    /// together with an engine [`ThreadExecutionContext`] and fetch the
    /// host-filtered [`VisibleCapabilitySurface`]. Shared by `available_actions`
    /// and `available_capabilities`. Errors map to [`EngineError::Effect`] so
    /// the Tier-0 channel degrades to Tier 2 rather than rendering a partial
    /// inventory.
    async fn visible_capability_surface(
        &self,
        context: &ThreadExecutionContext,
    ) -> Result<VisibleCapabilitySurface, EngineError> {
        let exec_ctx = self.context_factory.build(context)?;
        let request = VisibleCapabilityRequest::new(exec_ctx, self.surface_kind.clone())
            .with_policy(self.visibility_policy.clone())
            .with_provider_trust(self.provider_trust.clone());
        self.runtime
            .visible_capabilities(request)
            .await
            .map_err(map_host_runtime_error)
    }
}

/// True if any currently-active lease covers `action_name`. Mirrors the
/// `validate_lease` authority check (valid + covers) minus the thread-id
/// equality, because the leases passed to `available_actions` are already
/// scoped to the executing thread by `LeaseManager::active_for_thread`
/// (`orchestrator.rs:1287`).
fn lease_covers_action(leases: &[CapabilityLease], action_name: &str) -> bool {
    leases
        .iter()
        .any(|lease| lease.is_valid() && lease.covers_action(action_name))
}

/// Project a host [`VisibleCapability`] into an engine [`ActionDef`] for the
/// Tier-0 callable inventory.
///
/// `effects` is intentionally empty (Q-H12-2-EFFECTS = A): the host runtime is
/// the authoritative trust/grant/approval/policy layer for these
/// capabilities, so the engine `PolicyEngine` acts as a passthrough
/// (lease-validity + lease-coverage + `requires_approval` only). Emitting a
/// lossy `EffectKind`→`EffectType` mapping here would feed the engine
/// `PolicyEngine::evaluate` (`policy.rs:75`) and the gate tier classifier with
/// a coarse projection that could cause false denials or miss a deny; the host
/// re-authorizes on every `invoke_capability` regardless.
///
/// `requires_approval` is fail-safe: it is set when the host marks the
/// capability askable (`VisibleCapabilityAccess::RequiresApproval`) OR when the
/// descriptor's static default permission is `Ask`.
fn project_action_def(visible: VisibleCapability) -> ActionDef {
    let descriptor = visible.descriptor;
    ActionDef {
        name: descriptor.id.as_str().to_string(),
        description: descriptor.description,
        parameters_schema: descriptor.parameters_schema,
        effects: Vec::new(),
        requires_approval: matches!(visible.access, VisibleCapabilityAccess::RequiresApproval)
            || descriptor.default_permission == PermissionMode::Ask,
        model_tool_surface: ModelToolSurface::FullSchema,
        discovery: None,
    }
}

/// Project a host [`VisibleCapability`] into a background [`CapabilitySummary`].
///
/// `kind` follows the descriptor runtime: MCP-backed capabilities are
/// extension providers, first-party/system capabilities are runtime
/// backgrounds. `status` follows the host visibility access: `Available` →
/// `Ready`, `RequiresApproval` → `ReadyScoped` (usable only through the
/// approval route). `action_preview` surfaces the single callable capability
/// id; the engine `ActionInventory` carries the full callable schema.
fn project_capability_summary(visible: VisibleCapability) -> CapabilitySummary {
    let descriptor = visible.descriptor;
    let name = descriptor.id.as_str().to_string();
    CapabilitySummary {
        name: name.clone(),
        display_name: None,
        kind: capability_summary_kind(descriptor.runtime),
        status: capability_status(visible.access),
        description: Some(descriptor.description),
        action_preview: vec![name],
        routing_hint: None,
    }
}

fn capability_summary_kind(runtime: RuntimeKind) -> CapabilitySummaryKind {
    match runtime {
        RuntimeKind::Mcp => CapabilitySummaryKind::Provider,
        RuntimeKind::FirstParty | RuntimeKind::System => CapabilitySummaryKind::Runtime,
    }
}

fn capability_status(access: VisibleCapabilityAccess) -> CapabilityStatus {
    match access {
        VisibleCapabilityAccess::Available => CapabilityStatus::Ready,
        VisibleCapabilityAccess::RequiresApproval => CapabilityStatus::ReadyScoped,
    }
}

/// Validate the engine-supplied lease without consuming it (the engine already
/// consumed one use via `find_and_consume`). Any mismatch is fail-closed →
/// [`EngineError::Effect`] so the Tier-0 channel degrades to Tier 2 rather than
/// dispatching under a stale/wrong-scope lease.
fn validate_lease(
    lease: &CapabilityLease,
    action_name: &str,
    engine_ctx: &ThreadExecutionContext,
) -> Result<(), EngineError> {
    if !lease.is_valid() {
        return Err(EngineError::Effect {
            reason: "tier-zero lease is no longer valid".into(),
        });
    }
    if !lease.covers_action(action_name) {
        return Err(EngineError::Effect {
            reason: format!("tier-zero lease does not cover action '{action_name}'"),
        });
    }
    if lease.thread_id.0 != engine_ctx.thread_id.0 {
        return Err(EngineError::Effect {
            reason: "tier-zero lease thread id does not match the executing thread".into(),
        });
    }
    Ok(())
}

/// Transitional caller-supplied trust decision. `DefaultHostRuntime` ignores
/// this field and resolves provider trust itself (`lib.rs:337-345`); it is kept
/// on the request shape for compatibility. Mirrors `automation.rs::
/// trigger_trust_decision`: `UserTrusted`, `DispatchCapability`-only authority.
fn tier_zero_trust_decision() -> TrustDecision {
    TrustDecision {
        effective_trust: EffectiveTrustClass::user_trusted(),
        authority_ceiling: AuthorityCeiling {
            allowed_effects: vec![EffectKind::DispatchCapability],
            max_resource_ceiling: None,
        },
        provenance: TrustProvenance::Default,
        evaluated_at: chrono::Utc::now(),
    }
}

/// Map a host-runtime infrastructure error to a categorical safe summary.
/// Mirrors `automation.rs::map_host_runtime_error`: the raw `reason` is
/// discarded (it may echo host-internal detail) and a fixed safe string is
/// surfaced — the Tier-0 channel degrades to Tier 2 on this error.
fn map_host_runtime_error(error: HostRuntimeError) -> EngineError {
    match error {
        HostRuntimeError::InvalidRequest { .. } => EngineError::Effect {
            reason: "tier-zero host runtime rejected the request".into(),
        },
        HostRuntimeError::Unavailable { .. } => EngineError::Effect {
            reason: "tier-zero host runtime unavailable".into(),
        },
    }
}

/// Categorical safe summary for a capability failure. The raw
/// `RuntimeCapabilityFailure::message` is discarded (it may contain
/// capability-internal detail); only the sanitized kind is surfaced.
fn safe_failure_message(kind: RuntimeFailureKind) -> &'static str {
    match kind {
        RuntimeFailureKind::Authorization => "capability authorization denied",
        RuntimeFailureKind::PolicyDenied => "capability policy denied",
        RuntimeFailureKind::InvalidInput => "capability rejected invalid input",
        RuntimeFailureKind::InvalidOutput => "capability produced invalid output",
        RuntimeFailureKind::Cancelled => "capability execution cancelled",
        RuntimeFailureKind::Unavailable | RuntimeFailureKind::MissingRuntime => {
            "capability unavailable"
        }
        RuntimeFailureKind::Backend
        | RuntimeFailureKind::Network
        | RuntimeFailureKind::Transient => "capability backend unavailable",
        RuntimeFailureKind::Dispatcher | RuntimeFailureKind::Internal => {
            "capability dispatch failed"
        }
        RuntimeFailureKind::OperationFailed => "capability operation failed",
        RuntimeFailureKind::OutputTooLarge => "capability output too large",
        RuntimeFailureKind::Process => "capability process failed",
        RuntimeFailureKind::Resource => "capability resource limit reached",
        // `RuntimeFailureKind` is `#[non_exhaustive]`.
        _ => "capability failed",
    }
}

/// Map a `RuntimeCapabilityOutcome` to an [`ActionResult`] / [`EngineError`]
/// per Q-H12-2-GATE = A (interim non-resumable).
fn map_capability_outcome(
    outcome: RuntimeCapabilityOutcome,
    action_name: &str,
    engine_ctx: &ThreadExecutionContext,
    duration: Duration,
) -> Result<ActionResult, EngineError> {
    let call_id = engine_ctx.current_call_id.clone().unwrap_or_default();
    match outcome {
        RuntimeCapabilityOutcome::Completed(completed) => Ok(ActionResult {
            call_id,
            action_name: action_name.to_string(),
            output: completed.output,
            is_error: false,
            duration,
        }),
        RuntimeCapabilityOutcome::SpawnedProcess(handle) => Ok(ActionResult {
            call_id,
            action_name: action_name.to_string(),
            output: json!({ "process": handle.process_id.to_string() }),
            is_error: false,
            duration,
        }),
        RuntimeCapabilityOutcome::Failed(failure) => Ok(ActionResult {
            call_id,
            action_name: action_name.to_string(),
            output: json!({ "error": safe_failure_message(failure.kind) }),
            is_error: true,
            duration,
        }),
        // Q-H12-2-GATE = A: interim non-resumable. Gate outcomes degrade the
        // Tier-0 channel to Tier 2 (the LLM path owns full gate handling)
        // rather than emitting `EngineError::GatePaused`, which the inline
        // retry wrapper would try to resume — not supported for Tier 0 yet.
        RuntimeCapabilityOutcome::ApprovalRequired(_) => Err(EngineError::Effect {
            reason: "tier-zero action requires approval; degrading to tier-2".into(),
        }),
        RuntimeCapabilityOutcome::AuthRequired(_) => Err(EngineError::Effect {
            reason: "tier-zero action requires authentication; degrading to tier-2".into(),
        }),
        RuntimeCapabilityOutcome::ResourceBlocked(_) => Err(EngineError::Effect {
            reason: "tier-zero action blocked by resource limits; degrading to tier-2".into(),
        }),
        RuntimeCapabilityOutcome::Unknown(_) => Err(EngineError::Effect {
            reason: "tier-zero capability unknown to the host runtime".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use brassclaw_engine::gate::CancellingGateController;
    use brassclaw_engine::types::capability::{GrantedActions, LeaseId};
    use brassclaw_engine::types::project::ProjectId as EngineProjectId;
    use brassclaw_engine::types::step::StepId;
    use brassclaw_engine::types::thread::{ThreadId as EngineThreadId, ThreadType};
    use brassclaw_host_api::{ApprovalRequestId, CapabilityDescriptor, ProcessId, ResourceUsage};
    use brassclaw_host_runtime::{
        CapabilitySurfaceVersion, RuntimeApprovalGate, RuntimeAuthGate, RuntimeBlockedReason,
        RuntimeCapabilityCompleted, RuntimeCapabilityFailure, RuntimeCapabilityUnknown,
        RuntimeGateId, RuntimeProcessHandle, RuntimeResourceGate,
    };
    use chrono::Utc;
    use std::sync::Mutex;

    fn engine_ctx(user_id: &str) -> ThreadExecutionContext {
        ThreadExecutionContext {
            thread_id: EngineThreadId::new(),
            thread_type: ThreadType::Foreground,
            project_id: EngineProjectId::new(),
            user_id: user_id.to_string(),
            step_id: StepId::new(),
            current_call_id: None,
            source_channel: None,
            user_timezone: None,
            thread_goal: None,
            available_actions_snapshot: None,
            available_action_inventory_snapshot: None,
            conversation_scope: None,
            gate_controller: CancellingGateController::arc(),
            call_approval_granted: false,
            conversation_id: None,
        }
    }

    fn sample_factory() -> TierZeroExecutionContextFactory {
        TierZeroExecutionContextFactory::new(
            TenantId::new("default-tenant").unwrap(),
            Some(AgentId::new("default-agent").unwrap()),
            ExtensionId::new("loop-driver-test").unwrap(),
            CapabilitySet::default(),
        )
    }

    #[test]
    fn build_projects_engine_scope_into_host_execution_context() {
        let factory = sample_factory();
        let ctx = factory.build(&engine_ctx("alice")).unwrap();

        assert_eq!(ctx.tenant_id, TenantId::new("default-tenant").unwrap());
        assert_eq!(
            ctx.agent_id.as_ref(),
            Some(&AgentId::new("default-agent").unwrap())
        );
        assert_eq!(
            ctx.extension_id,
            ExtensionId::new("loop-driver-test").unwrap()
        );
        assert_eq!(ctx.user_id, UserId::new("alice").unwrap());
        assert!(ctx.project_id.is_some());
        assert!(ctx.thread_id.is_some());
        assert_eq!(ctx.resource_scope.tenant_id, ctx.tenant_id);
        assert_eq!(ctx.resource_scope.agent_id, ctx.agent_id);
        assert_eq!(ctx.resource_scope.project_id, ctx.project_id);
        assert_eq!(ctx.resource_scope.thread_id, ctx.thread_id);
        assert_eq!(ctx.mounts, MountView::default());
        assert_eq!(ctx.runtime, RuntimeKind::FirstParty);
        assert_eq!(ctx.trust, TrustClass::UserTrusted);
        // The context must pass host-api validation.
        assert!(ctx.validate().is_ok());
    }

    #[test]
    fn build_fails_closed_when_engine_user_id_contains_a_path_separator() {
        let factory = sample_factory();
        // validate_scope_id forbids path separators. The engine context
        // always carries a user_id (String, not Option), so an invalid value
        // is corruption — the factory must NOT misattribute the action to a
        // fallback user; it returns EngineError::Effect so the Tier-0 channel
        // degrades to Tier 2.
        let err = factory.build(&engine_ctx("alice/bob")).unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[test]
    fn build_fails_closed_when_engine_user_id_is_empty() {
        let factory = sample_factory();
        let err = factory.build(&engine_ctx("")).unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[test]
    fn build_converts_engine_uuid_project_and_thread_ids_to_host_api_ids() {
        let factory = sample_factory();
        let engine = engine_ctx("alice");
        let expected_project = ProjectId::new(engine.project_id.0.to_string()).unwrap();
        let expected_thread = ThreadId::new(engine.thread_id.0.to_string()).unwrap();
        let ctx = factory.build(&engine).unwrap();
        assert_eq!(ctx.project_id.as_ref(), Some(&expected_project));
        assert_eq!(ctx.thread_id.as_ref(), Some(&expected_thread));
    }

    fn registry() -> TierZeroActionRegistry {
        TierZeroActionRegistry::new()
    }

    #[test]
    fn registry_passes_through_a_valid_two_segment_capability_id() {
        let resolved = registry().resolve("shell.exec").unwrap();
        assert_eq!(resolved.as_str(), "shell.exec");
    }

    #[test]
    fn registry_passes_through_a_valid_namespaced_capability_id() {
        let resolved = registry().resolve("github.issues.search").unwrap();
        assert_eq!(resolved.as_str(), "github.issues.search");
    }

    #[test]
    fn registry_fails_closed_when_action_name_has_no_dot() {
        let err = registry().resolve("exec").unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[test]
    fn registry_fails_closed_when_action_name_has_an_empty_segment() {
        let err = registry().resolve("shell.").unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[test]
    fn registry_fails_closed_when_action_name_is_empty() {
        let err = registry().resolve("").unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[test]
    fn registry_fails_closed_when_action_name_has_an_uppercase_segment() {
        // validate_name_segment requires each segment to start with a
        // lowercase ASCII letter or digit.
        let err = registry().resolve("Shell.exec").unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    // ── ProductionEffectExecutor::dispatch_action ───────────────────────

    #[derive(Clone, Debug)]
    struct CapturedInvoke {
        tenant_id: TenantId,
        user_id: UserId,
        project_id: Option<ProjectId>,
        thread_id: Option<ThreadId>,
        capability_id: CapabilityId,
        input: Value,
        trust_decision: TrustDecision,
    }

    struct RecordingHostRuntime {
        captured: Mutex<Option<CapturedInvoke>>,
        invoke_outcome: Mutex<Option<Result<RuntimeCapabilityOutcome, HostRuntimeError>>>,
    }

    impl RecordingHostRuntime {
        fn with_outcome(outcome: RuntimeCapabilityOutcome) -> Self {
            Self {
                captured: Mutex::new(None),
                invoke_outcome: Mutex::new(Some(Ok(outcome))),
            }
        }

        fn with_error(error: HostRuntimeError) -> Self {
            Self {
                captured: Mutex::new(None),
                invoke_outcome: Mutex::new(Some(Err(error))),
            }
        }

        fn was_invoked(&self) -> bool {
            self.captured.lock().unwrap().is_some()
        }

        fn captured(&self) -> CapturedInvoke {
            self.captured
                .lock()
                .unwrap()
                .clone()
                .expect("invoke_capability was not called")
        }
    }

    #[async_trait]
    impl HostRuntime for RecordingHostRuntime {
        async fn invoke_capability(
            &self,
            request: RuntimeCapabilityRequest,
        ) -> Result<RuntimeCapabilityOutcome, HostRuntimeError> {
            let captured = CapturedInvoke {
                tenant_id: request.context.tenant_id.clone(),
                user_id: request.context.user_id.clone(),
                project_id: request.context.project_id.clone(),
                thread_id: request.context.thread_id.clone(),
                capability_id: request.capability_id.clone(),
                input: request.input.clone(),
                trust_decision: request.trust_decision.clone(),
            };
            *self.captured.lock().unwrap() = Some(captured);
            self.invoke_outcome
                .lock()
                .unwrap()
                .take()
                .expect("an invoke outcome must be configured")
        }

        async fn resume_capability(
            &self,
            _request: brassclaw_host_runtime::RuntimeCapabilityResumeRequest,
        ) -> Result<RuntimeCapabilityOutcome, HostRuntimeError> {
            unreachable!("resume_capability is not used in tier-zero adapter tests")
        }

        async fn visible_capabilities(
            &self,
            _request: brassclaw_host_runtime::VisibleCapabilityRequest,
        ) -> Result<brassclaw_host_runtime::VisibleCapabilitySurface, HostRuntimeError> {
            unreachable!("visible_capabilities is not used in tier-zero execute_action tests")
        }

        async fn cancel_work(
            &self,
            _request: brassclaw_host_runtime::CancelRuntimeWorkRequest,
        ) -> Result<brassclaw_host_runtime::CancelRuntimeWorkOutcome, HostRuntimeError> {
            unreachable!("cancel_work is not used in tier-zero adapter tests")
        }

        async fn runtime_status(
            &self,
            _request: brassclaw_host_runtime::RuntimeStatusRequest,
        ) -> Result<brassclaw_host_runtime::HostRuntimeStatus, HostRuntimeError> {
            unreachable!("runtime_status is not used in tier-zero adapter tests")
        }

        async fn health(
            &self,
        ) -> Result<brassclaw_host_runtime::HostRuntimeHealth, HostRuntimeError> {
            unreachable!("health is not used in tier-zero adapter tests")
        }
    }

    fn executor(runtime: Arc<RecordingHostRuntime>) -> ProductionEffectExecutor {
        ProductionEffectExecutor::new(
            runtime,
            Arc::new(sample_factory()),
            Arc::new(TierZeroActionRegistry::new()),
            SurfaceKind::new("agent_loop").expect("valid surface kind"),
            BTreeMap::new(),
            CapabilitySurfacePolicy::allow_all(),
        )
    }

    fn valid_lease(thread_id: EngineThreadId) -> CapabilityLease {
        CapabilityLease {
            id: LeaseId::new(),
            thread_id,
            capability_name: "memory.write".into(),
            granted_actions: GrantedActions::All,
            granted_at: Utc::now(),
            expires_at: None,
            max_uses: None,
            uses_remaining: None,
            revoked: false,
            revoked_reason: None,
        }
    }

    fn completed_outcome(output: Value) -> RuntimeCapabilityOutcome {
        RuntimeCapabilityOutcome::Completed(Box::new(RuntimeCapabilityCompleted {
            capability_id: CapabilityId::new("memory.write").unwrap(),
            output,
            display_preview: None,
            usage: ResourceUsage::default(),
        }))
    }

    fn failure_outcome(kind: RuntimeFailureKind) -> RuntimeCapabilityOutcome {
        RuntimeCapabilityOutcome::Failed(RuntimeCapabilityFailure::new(
            CapabilityId::new("memory.write").unwrap(),
            kind,
            None,
        ))
    }

    fn spawned_outcome() -> RuntimeCapabilityOutcome {
        RuntimeCapabilityOutcome::SpawnedProcess(RuntimeProcessHandle {
            process_id: ProcessId::new(),
            capability_id: CapabilityId::new("shell.spawn").unwrap(),
        })
    }

    fn approval_outcome() -> RuntimeCapabilityOutcome {
        RuntimeCapabilityOutcome::ApprovalRequired(RuntimeApprovalGate {
            approval_request_id: ApprovalRequestId::new(),
            capability_id: CapabilityId::new("memory.write").unwrap(),
            reason: RuntimeBlockedReason::ApprovalRequired,
        })
    }

    fn auth_outcome() -> RuntimeCapabilityOutcome {
        RuntimeCapabilityOutcome::AuthRequired(RuntimeAuthGate {
            gate_id: RuntimeGateId::new(),
            capability_id: CapabilityId::new("memory.write").unwrap(),
            reason: RuntimeBlockedReason::AuthRequired,
            required_secrets: Vec::new(),
            credential_requirements: Vec::new(),
        })
    }

    fn resource_outcome() -> RuntimeCapabilityOutcome {
        RuntimeCapabilityOutcome::ResourceBlocked(RuntimeResourceGate {
            gate_id: RuntimeGateId::new(),
            capability_id: CapabilityId::new("memory.write").unwrap(),
            reason: RuntimeBlockedReason::ResourceLimit,
            estimate: ResourceEstimate::default(),
        })
    }

    fn unknown_outcome() -> RuntimeCapabilityOutcome {
        RuntimeCapabilityOutcome::Unknown(RuntimeCapabilityUnknown {
            capability_id: CapabilityId::new("memory.write").unwrap(),
            kind: "test".into(),
            message: None,
        })
    }

    #[tokio::test]
    async fn dispatch_completed_outcome_returns_non_error_action_result() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(completed_outcome(
            json!({"ok": true}),
        )));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let result = exec
            .dispatch_action(
                "memory.write",
                json!({"k": "v"}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(result.action_name, "memory.write");
        assert_eq!(result.output, json!({"ok": true}));
        assert_eq!(result.call_id, "");
    }

    #[tokio::test]
    async fn dispatch_failed_outcome_returns_error_result_with_safe_message() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(failure_outcome(
            RuntimeFailureKind::Authorization,
        )));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let result = exec
            .dispatch_action(
                "memory.write",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap();
        assert!(result.is_error);
        assert_eq!(
            result.output,
            json!({"error": "capability authorization denied"})
        );
    }

    #[tokio::test]
    async fn dispatch_spawned_process_outcome_returns_a_process_handle() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(spawned_outcome()));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let result = exec
            .dispatch_action(
                "shell.spawn",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.output.get("process").is_some());
        assert!(result.output["process"].is_string());
    }

    #[tokio::test]
    async fn dispatch_approval_required_degrades_to_engine_error() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(approval_outcome()));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let err = exec
            .dispatch_action(
                "memory.write",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[tokio::test]
    async fn dispatch_auth_required_degrades_to_engine_error() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(auth_outcome()));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let err = exec
            .dispatch_action(
                "memory.write",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[tokio::test]
    async fn dispatch_resource_blocked_degrades_to_engine_error() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(resource_outcome()));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let err = exec
            .dispatch_action(
                "memory.write",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[tokio::test]
    async fn dispatch_unknown_capability_degrades_to_engine_error() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(unknown_outcome()));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let err = exec
            .dispatch_action(
                "memory.write",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[tokio::test]
    async fn dispatch_maps_host_runtime_unavailable_error_to_engine_error() {
        let runtime = Arc::new(RecordingHostRuntime::with_error(
            HostRuntimeError::unavailable("boom"),
        ));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let err = exec
            .dispatch_action(
                "memory.write",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[tokio::test]
    async fn dispatch_maps_host_runtime_invalid_request_error_to_engine_error() {
        let runtime = Arc::new(RecordingHostRuntime::with_error(
            HostRuntimeError::invalid_request("bad"),
        ));
        let exec = executor(runtime);
        let engine = engine_ctx("alice");
        let err = exec
            .dispatch_action(
                "memory.write",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[tokio::test]
    async fn dispatch_rejects_a_revoked_lease_without_invoking_the_host_runtime() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(completed_outcome(
            json!({}),
        )));
        let exec = executor(runtime.clone());
        let engine = engine_ctx("alice");
        let mut lease = valid_lease(engine.thread_id);
        lease.revoked = true;
        let err = exec
            .dispatch_action("memory.write", json!({}), &lease, &engine)
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
        assert!(!runtime.was_invoked());
    }

    #[tokio::test]
    async fn dispatch_rejects_a_lease_for_a_different_thread_without_invoking() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(completed_outcome(
            json!({}),
        )));
        let exec = executor(runtime.clone());
        let engine = engine_ctx("alice");
        let lease = valid_lease(EngineThreadId::new());
        let err = exec
            .dispatch_action("memory.write", json!({}), &lease, &engine)
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
        assert!(!runtime.was_invoked());
    }

    #[tokio::test]
    async fn dispatch_rejects_a_lease_that_does_not_cover_the_action_without_invoking() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(completed_outcome(
            json!({}),
        )));
        let exec = executor(runtime.clone());
        let engine = engine_ctx("alice");
        let mut lease = valid_lease(engine.thread_id);
        lease.granted_actions = GrantedActions::Specific(vec!["other.action".into()]);
        let err = exec
            .dispatch_action("memory.write", json!({}), &lease, &engine)
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
        assert!(!runtime.was_invoked());
    }

    #[tokio::test]
    async fn dispatch_rejects_an_invalid_action_name_without_invoking_the_host_runtime() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(completed_outcome(
            json!({}),
        )));
        let exec = executor(runtime.clone());
        let engine = engine_ctx("alice");
        // No dot -> not a valid `<extension>.<capability>` CapabilityId.
        let err = exec
            .dispatch_action(
                "memorywrite",
                json!({}),
                &valid_lease(engine.thread_id),
                &engine,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
        assert!(!runtime.was_invoked());
    }

    #[tokio::test]
    async fn dispatch_projects_engine_scope_and_input_into_the_invoke_request() {
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(completed_outcome(
            json!({}),
        )));
        let exec = executor(runtime.clone());
        let engine = engine_ctx("alice");
        let input = json!({"key": "value", "n": 3});
        exec.dispatch_action(
            "memory.write",
            input.clone(),
            &valid_lease(engine.thread_id),
            &engine,
        )
        .await
        .unwrap();
        let captured = runtime.captured();
        assert_eq!(captured.tenant_id, TenantId::new("default-tenant").unwrap());
        assert_eq!(captured.user_id, UserId::new("alice").unwrap());
        assert_eq!(
            captured.capability_id,
            CapabilityId::new("memory.write").unwrap()
        );
        assert_eq!(captured.input, input);
        assert!(captured.project_id.is_some());
        assert!(captured.thread_id.is_some());
        assert!(matches!(
            captured.trust_decision.provenance,
            TrustProvenance::Default
        ));
    }

    // ── H.12.2.4: available_actions / available_capabilities projections ──

    /// Host-runtime mock that serves a configured `visible_capabilities`
    /// result and captures the request, so the per-run visibility config
    /// projection can be asserted. All other trait methods are unreachable —
    /// the visibility tests never invoke capabilities.
    struct VisibilityHostRuntime {
        surface: Mutex<Option<Result<VisibleCapabilitySurface, HostRuntimeError>>>,
        captured_request: Mutex<Option<VisibleCapabilityRequest>>,
    }

    impl VisibilityHostRuntime {
        fn with_surface(surface: VisibleCapabilitySurface) -> Self {
            Self {
                surface: Mutex::new(Some(Ok(surface))),
                captured_request: Mutex::new(None),
            }
        }

        fn with_error(error: HostRuntimeError) -> Self {
            Self {
                surface: Mutex::new(Some(Err(error))),
                captured_request: Mutex::new(None),
            }
        }

        fn captured_request(&self) -> VisibleCapabilityRequest {
            self.captured_request
                .lock()
                .unwrap()
                .clone()
                .expect("visible_capabilities was not called")
        }
    }

    #[async_trait]
    impl HostRuntime for VisibilityHostRuntime {
        async fn invoke_capability(
            &self,
            _request: RuntimeCapabilityRequest,
        ) -> Result<RuntimeCapabilityOutcome, HostRuntimeError> {
            unreachable!("invoke_capability is not used in tier-zero visibility tests")
        }

        async fn resume_capability(
            &self,
            _request: brassclaw_host_runtime::RuntimeCapabilityResumeRequest,
        ) -> Result<RuntimeCapabilityOutcome, HostRuntimeError> {
            unreachable!("resume_capability is not used in tier-zero visibility tests")
        }

        async fn visible_capabilities(
            &self,
            request: VisibleCapabilityRequest,
        ) -> Result<VisibleCapabilitySurface, HostRuntimeError> {
            *self.captured_request.lock().unwrap() = Some(request);
            self.surface
                .lock()
                .unwrap()
                .as_ref()
                .expect("a visibility surface must be configured")
                .clone()
        }

        async fn cancel_work(
            &self,
            _request: brassclaw_host_runtime::CancelRuntimeWorkRequest,
        ) -> Result<brassclaw_host_runtime::CancelRuntimeWorkOutcome, HostRuntimeError> {
            unreachable!("cancel_work is not used in tier-zero visibility tests")
        }

        async fn runtime_status(
            &self,
            _request: brassclaw_host_runtime::RuntimeStatusRequest,
        ) -> Result<brassclaw_host_runtime::HostRuntimeStatus, HostRuntimeError> {
            unreachable!("runtime_status is not used in tier-zero visibility tests")
        }

        async fn health(
            &self,
        ) -> Result<brassclaw_host_runtime::HostRuntimeHealth, HostRuntimeError> {
            unreachable!("health is not used in tier-zero visibility tests")
        }
    }

    fn visibility_executor(runtime: Arc<VisibilityHostRuntime>) -> ProductionEffectExecutor {
        ProductionEffectExecutor::new(
            runtime,
            Arc::new(sample_factory()),
            Arc::new(TierZeroActionRegistry::new()),
            SurfaceKind::new("agent_loop").expect("valid surface kind"),
            BTreeMap::new(),
            CapabilitySurfacePolicy::allow_all(),
        )
    }

    fn descriptor(
        id: &str,
        runtime: RuntimeKind,
        permission: PermissionMode,
    ) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new(id).expect("valid capability id"),
            provider: ExtensionId::new("test").expect("valid extension id"),
            runtime,
            trust_ceiling: TrustClass::UserTrusted,
            description: format!("description for {id}"),
            parameters_schema: json!({"type": "object"}),
            effects: Vec::new(),
            default_permission: permission,
            runtime_credentials: Vec::new(),
            resource_profile: None,
        }
    }

    fn visible_capability(
        id: &str,
        runtime: RuntimeKind,
        permission: PermissionMode,
        access: VisibleCapabilityAccess,
    ) -> VisibleCapability {
        VisibleCapability {
            descriptor: descriptor(id, runtime, permission),
            access,
            estimated_resources: ResourceEstimate::default(),
        }
    }

    fn surface(capabilities: Vec<VisibleCapability>) -> VisibleCapabilitySurface {
        VisibleCapabilitySurface {
            version: CapabilitySurfaceVersion::new("test-v1").expect("valid version"),
            capabilities,
        }
    }

    fn lease_for(thread_id: EngineThreadId, granted: GrantedActions) -> CapabilityLease {
        CapabilityLease {
            id: LeaseId::new(),
            thread_id,
            capability_name: "memory".into(),
            granted_actions: granted,
            granted_at: Utc::now(),
            expires_at: None,
            max_uses: None,
            uses_remaining: None,
            revoked: false,
            revoked_reason: None,
        }
    }

    #[tokio::test]
    async fn available_actions_projects_visible_descriptors_into_action_defs() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            visible_capability(
                "memory.write",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
            visible_capability(
                "github.issues.create",
                RuntimeKind::Mcp,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
        ])));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        // Wildcard lease so both capabilities are callable.
        let leases = vec![lease_for(engine.thread_id, GrantedActions::All)];
        let actions = exec.available_actions(&leases, &engine).await.unwrap();
        assert_eq!(actions.len(), 2);
        let write = actions
            .iter()
            .find(|a| a.name == "memory.write")
            .expect("memory.write action present");
        assert_eq!(write.description, "description for memory.write");
        assert_eq!(write.parameters_schema, json!({"type": "object"}));
        // Q-H12-2-EFFECTS = A: empty effects.
        assert!(write.effects.is_empty());
        assert!(!write.requires_approval);
        assert_eq!(write.model_tool_surface, ModelToolSurface::FullSchema);
        assert!(write.discovery.is_none());
        assert!(actions.iter().any(|a| a.name == "github.issues.create"));
    }

    #[tokio::test]
    async fn available_actions_filters_to_leased_capabilities() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            visible_capability(
                "memory.write",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
            visible_capability(
                "memory.read",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
        ])));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        // Lease covers only memory.write; memory.read must be omitted.
        let leases = vec![lease_for(
            engine.thread_id,
            GrantedActions::Specific(vec!["memory.write".into()]),
        )];
        let actions = exec.available_actions(&leases, &engine).await.unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].name, "memory.write");
    }

    #[tokio::test]
    async fn available_actions_omits_capabilities_with_no_lease() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            visible_capability(
                "memory.write",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
        ])));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        // No leases at all -> empty callable inventory.
        let actions = exec.available_actions(&[], &engine).await.unwrap();
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn available_actions_marks_askable_or_ask_permission_as_requires_approval() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            // Host marks askable.
            visible_capability(
                "shell.exec",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::RequiresApproval,
            ),
            // Static default permission is Ask.
            visible_capability(
                "github.issues.create",
                RuntimeKind::Mcp,
                PermissionMode::Ask,
                VisibleCapabilityAccess::Available,
            ),
            // Plain allow + available -> no approval.
            visible_capability(
                "memory.read",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
        ])));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        let leases = vec![lease_for(engine.thread_id, GrantedActions::All)];
        let actions = exec.available_actions(&leases, &engine).await.unwrap();
        let by_name = |n: &str| {
            actions
                .iter()
                .find(|a| a.name == n)
                .expect("action present")
        };
        assert!(by_name("shell.exec").requires_approval);
        assert!(by_name("github.issues.create").requires_approval);
        assert!(!by_name("memory.read").requires_approval);
    }

    #[tokio::test]
    async fn available_actions_maps_host_runtime_error_to_engine_error() {
        let runtime = Arc::new(VisibilityHostRuntime::with_error(
            HostRuntimeError::unavailable("surface down"),
        ));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        let leases = vec![lease_for(engine.thread_id, GrantedActions::All)];
        let err = exec.available_actions(&leases, &engine).await.unwrap_err();
        assert!(matches!(err, EngineError::Effect { .. }));
    }

    #[tokio::test]
    async fn available_actions_passes_per_run_visibility_config_into_the_request() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            visible_capability(
                "memory.write",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
        ])));
        let exec = visibility_executor(runtime.clone());
        let engine = engine_ctx("alice");
        let leases = vec![lease_for(engine.thread_id, GrantedActions::All)];
        exec.available_actions(&leases, &engine).await.unwrap();
        let request = runtime.captured_request();
        assert_eq!(request.surface_kind.as_str(), "agent_loop");
        assert_eq!(request.policy, CapabilitySurfacePolicy::allow_all());
        assert!(request.provider_trust.is_empty());
        assert_eq!(
            request.context.tenant_id,
            TenantId::new("default-tenant").unwrap()
        );
        assert_eq!(request.context.user_id, UserId::new("alice").unwrap());
    }

    #[tokio::test]
    async fn available_capabilities_projects_visible_descriptors_into_summaries() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            visible_capability(
                "github.issues.create",
                RuntimeKind::Mcp,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
            visible_capability(
                "memory.write",
                RuntimeKind::FirstParty,
                PermissionMode::Ask,
                VisibleCapabilityAccess::RequiresApproval,
            ),
        ])));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        let summaries = exec.available_capabilities(&[], &engine).await.unwrap();
        assert_eq!(summaries.len(), 2);
        let gh = summaries
            .iter()
            .find(|s| s.name == "github.issues.create")
            .expect("github summary present");
        assert_eq!(gh.kind, CapabilitySummaryKind::Provider);
        assert_eq!(gh.status, CapabilityStatus::Ready);
        assert_eq!(
            gh.description.as_deref(),
            Some("description for github.issues.create")
        );
        assert_eq!(gh.action_preview, vec!["github.issues.create"]);
        let mem = summaries
            .iter()
            .find(|s| s.name == "memory.write")
            .expect("memory summary present");
        assert_eq!(mem.kind, CapabilitySummaryKind::Runtime);
        assert_eq!(mem.status, CapabilityStatus::ReadyScoped);
    }

    #[tokio::test]
    async fn available_capabilities_does_not_filter_by_lease() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            visible_capability(
                "memory.write",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
            visible_capability(
                "github.issues.create",
                RuntimeKind::Mcp,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
        ])));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        // No leases, yet summaries still surface (background view).
        let summaries = exec.available_capabilities(&[], &engine).await.unwrap();
        assert_eq!(summaries.len(), 2);
    }

    #[tokio::test]
    async fn effect_executor_execute_action_delegates_to_dispatch_action() {
        // RecordingHostRuntime serves invoke_capability; the trait
        // execute_action must delegate to dispatch_action and reach it.
        let runtime = Arc::new(RecordingHostRuntime::with_outcome(completed_outcome(
            json!({"ok": true}),
        )));
        let exec = executor(runtime.clone());
        let engine = engine_ctx("alice");
        let result = EffectExecutor::execute_action(
            &exec,
            "memory.write",
            json!({}),
            &valid_lease(engine.thread_id),
            &engine,
        )
        .await
        .unwrap();
        assert!(runtime.was_invoked());
        assert!(!result.is_error);
        assert_eq!(result.action_name, "memory.write");
        assert_eq!(result.output, json!({"ok": true}));
    }

    #[tokio::test]
    async fn available_action_inventory_wraps_available_actions_inline() {
        let runtime = Arc::new(VisibilityHostRuntime::with_surface(surface(vec![
            visible_capability(
                "memory.write",
                RuntimeKind::FirstParty,
                PermissionMode::Allow,
                VisibleCapabilityAccess::Available,
            ),
        ])));
        let exec = visibility_executor(runtime);
        let engine = engine_ctx("alice");
        let leases = vec![lease_for(engine.thread_id, GrantedActions::All)];
        let inventory = exec
            .available_action_inventory(&leases, &engine)
            .await
            .unwrap();
        assert_eq!(inventory.inline.len(), 1);
        assert_eq!(inventory.inline[0].name, "memory.write");
        assert!(inventory.discoverable.is_empty());
    }
}
