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

use std::sync::Arc;
use std::time::{Duration, Instant};

use brassclaw_engine::{ActionResult, CapabilityLease, EngineError, ThreadExecutionContext};
use brassclaw_host_api::{
    AgentId, CapabilityId, CapabilitySet, EffectKind, ExecutionContext, ExtensionId, MountView,
    ProjectId, ResourceEstimate, RuntimeKind, TenantId, ThreadId, TrustClass, UserId,
};
use brassclaw_host_runtime::{
    HostRuntime, HostRuntimeError, RuntimeCapabilityOutcome, RuntimeCapabilityRequest,
    RuntimeFailureKind,
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
/// [`TierZeroExecutionContextFactory`] and [`TierZeroActionResolver`] baked by
/// `TierZeroEffectExecutorBuilder::build_for_run` (H.12.2.5). [`dispatch_action`]
/// is the real engine `execute_action` body; the `impl EffectExecutor` block
/// (H.12.2.4) delegates to it and adds the `available_*` projections.
///
/// `dead_code` is allowed module-wide until the adapter is wired in H.12.2.5.
pub(crate) struct ProductionEffectExecutor {
    runtime: Arc<dyn HostRuntime>,
    context_factory: Arc<TierZeroExecutionContextFactory>,
    action_resolver: Arc<dyn TierZeroActionResolver>,
}

impl ProductionEffectExecutor {
    pub(crate) fn new(
        runtime: Arc<dyn HostRuntime>,
        context_factory: Arc<TierZeroExecutionContextFactory>,
        action_resolver: Arc<dyn TierZeroActionResolver>,
    ) -> Self {
        Self {
            runtime,
            context_factory,
            action_resolver,
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
    use brassclaw_host_api::{ApprovalRequestId, ProcessId, ResourceUsage};
    use brassclaw_host_runtime::{
        RuntimeApprovalGate, RuntimeAuthGate, RuntimeBlockedReason, RuntimeCapabilityCompleted,
        RuntimeCapabilityFailure, RuntimeCapabilityUnknown, RuntimeGateId, RuntimeProcessHandle,
        RuntimeResourceGate,
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
}
