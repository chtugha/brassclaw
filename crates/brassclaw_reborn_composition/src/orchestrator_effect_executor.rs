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

use brassclaw_engine::{EngineError, ThreadExecutionContext};
use brassclaw_host_api::{
    AgentId, CapabilitySet, ExecutionContext, ExtensionId, MountView, ProjectId, RuntimeKind,
    TenantId, ThreadId, TrustClass, UserId,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::gate::CancellingGateController;
    use brassclaw_engine::types::project::ProjectId as EngineProjectId;
    use brassclaw_engine::types::step::StepId;
    use brassclaw_engine::types::thread::{ThreadId as EngineThreadId, ThreadType};

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
}
