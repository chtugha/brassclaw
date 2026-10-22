//! Host-owned child-task operation behind an already checked native Tool.
//!
//! The request selects a workflow, not a Rust execution strategy. Its owner
//! resolves the exact parent-pinned workflow and typed inputs, admits through
//! the durable coordinator and uses the existing global Monty service. Parent
//! claims, catalogue provenance, dispatch identity and cancellation stay behind
//! that owner; none can be supplied by model-authored request fields.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{CapabilityId, ResourceScope, ThreadId};
use brassclaw_turns::{LoopMessageRef, SanitizedFailure, TurnRunId};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    InvocationServices, InvocationServicesError, InvocationServicesResolutionRequest,
    InvocationServicesResolver,
};

/// Stable workflow identity within the parent's captured catalogue. It carries
/// no version override, Python source, approval or execution authority.
pub struct RuntimeChildWorkflowRef {
    recipe: Uuid,
    variant: String,
}
impl RuntimeChildWorkflowRef {
    pub fn new(recipe: Uuid, variant: String) -> Result<Self, RuntimeChildTaskError> {
        if recipe.is_nil() || variant.is_empty() {
            return Err(RuntimeChildTaskError::InvalidRequest);
        }
        Ok(Self { recipe, variant })
    }

    pub fn recipe(&self) -> Uuid {
        self.recipe
    }

    pub fn variant(&self) -> &str {
        &self.variant
    }
}

/// Complete typed inputs. No Debug/serde surface accidentally publishes goal
/// or handoff text. The trusted owner checks the selected workflow's recursive
/// contract and actual data limits before any admission effect.
pub struct RuntimeChildTaskRequest {
    workflow: RuntimeChildWorkflowRef,
    inputs: Value,
}
impl RuntimeChildTaskRequest {
    pub fn new(
        workflow: RuntimeChildWorkflowRef,
        inputs: Value,
    ) -> Result<Self, RuntimeChildTaskError> {
        if !inputs.is_object() {
            return Err(RuntimeChildTaskError::InvalidRequest);
        }
        Ok(Self { workflow, inputs })
    }

    pub fn workflow(&self) -> &RuntimeChildWorkflowRef {
        &self.workflow
    }

    pub fn inputs(&self) -> &Value {
        &self.inputs
    }

    pub fn into_parts(self) -> (RuntimeChildWorkflowRef, Value) {
        (self.workflow, self.inputs)
    }
}

/// Actual correlated final reply. These fields describe an observation, not
/// completion proof: the owner must read and verify the durable run/reply pair.
/// Preserve the full content; a safe summary or display preview is insufficient.
pub struct RuntimeChildTaskReply {
    pub child_run_id: TurnRunId,
    pub thread_id: ThreadId,
    pub reply_ref: LoopMessageRef,
    pub content: String,
}

/// Observed terminal child state, distinct from admission and parent success.
/// Recovery-required children retain their effects and cannot be replayed.
pub enum RuntimeChildTaskOutcome {
    Completed(RuntimeChildTaskReply),
    Failed {
        child_run_id: TurnRunId,
        thread_id: ThreadId,
        failure: Option<SanitizedFailure>,
    },
    Cancelled {
        child_run_id: TurnRunId,
        thread_id: ThreadId,
    },
    RecoveryRequired {
        child_run_id: TurnRunId,
        thread_id: ThreadId,
    },
}

/// Fixed public categories only; no backend error, input or claim is formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeChildTaskError {
    #[error("child task request is invalid")]
    InvalidRequest,
    #[error("child task service is unavailable")]
    Unavailable,
    #[error("child task parent attempt is fenced")]
    Fenced,
    #[error("child task operation was cancelled")]
    Cancelled,
    #[error("child task admission or completion requires reconciliation")]
    Uncertain,
}

/// One task-bound native operation. Monty/Recipes choose the workflow and own
/// sequencing; Rust owns admission, transport and the awaited native outcome.
///
/// Implementations retain the original private claim/dispatch key before I/O,
/// recheck durable freshness, preserve the parent's exact catalogue, and use
/// supported dependent admission so waiting parents cannot starve their child.
/// Lost acknowledgements never mint a new child or prove no effect. A missing
/// reply/unknown outcome is an error, never an empty completed result.
#[async_trait]
pub trait RuntimeChildTaskPort: Send + Sync {
    async fn delegate(
        &self,
        scope: &ResourceScope,
        request: RuntimeChildTaskRequest,
    ) -> Result<RuntimeChildTaskOutcome, RuntimeChildTaskError>;
}

/// Add only this task's child operation to the selected native registration.
/// Resolve every other service through its original planner/policy boundary.
pub(crate) struct ChildTaskBoundServices {
    inner: Arc<dyn InvocationServicesResolver>,
    capability: CapabilityId,
    child: Arc<dyn RuntimeChildTaskPort>,
}
impl ChildTaskBoundServices {
    pub(crate) fn new(
        inner: Arc<dyn InvocationServicesResolver>,
        capability: CapabilityId,
        child: Arc<dyn RuntimeChildTaskPort>,
    ) -> Self {
        Self {
            inner,
            capability,
            child,
        }
    }
}
impl InvocationServicesResolver for ChildTaskBoundServices {
    fn resolve(
        &self,
        request: InvocationServicesResolutionRequest<'_>,
    ) -> Result<InvocationServices, InvocationServicesError> {
        let selected = request.plan.capability == self.capability;
        let mut services = self.inner.resolve(request)?;
        if selected {
            services.child_tasks = Some(self.child.clone());
        }
        Ok(services)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExecutionPlan, LocalHostProcessPort, LocalInvocationServicesResolver, RuntimeProcessPort,
    };
    use brassclaw_filesystem::{InMemoryBackend, RootFilesystem};
    use brassclaw_host_api::runtime_policy::{
        DeploymentMode, FilesystemBackendKind, NetworkMode, ProcessBackendKind, RuntimeProfile,
        SecretMode,
    };

    // Binding inspection must never delegate work. This marker has no fake
    // admission or success path; actual child execution needs its own owner.
    struct NeverDelegated;
    #[async_trait]
    impl RuntimeChildTaskPort for NeverDelegated {
        async fn delegate(
            &self,
            _: &ResourceScope,
            _: RuntimeChildTaskRequest,
        ) -> Result<RuntimeChildTaskOutcome, RuntimeChildTaskError> {
            panic!("binding inspection must not execute child work")
        }
    }

    #[test]
    fn child_binding_preserves_original_resolution_and_never_runs_work() {
        let filesystem: Arc<dyn RootFilesystem> = Arc::new(InMemoryBackend::new());
        let process: Arc<dyn RuntimeProcessPort> = Arc::new(LocalHostProcessPort::new());
        let original = Arc::new(LocalInvocationServicesResolver::new(
            filesystem.clone(),
            None,
            process.clone(),
            None,
        ));
        let child: Arc<dyn RuntimeChildTaskPort> = Arc::new(NeverDelegated);
        let capability = CapabilityId::new(crate::SPAWN_SUBAGENT_CAPABILITY_ID).unwrap();
        let bound =
            ChildTaskBoundServices::new(original.clone(), capability.clone(), child.clone());
        let mut plan = ExecutionPlan {
            capability,
            deployment: DeploymentMode::LocalSingleUser,
            resolved_profile: RuntimeProfile::LocalDev,
            filesystem_backend: FilesystemBackendKind::HostWorkspace,
            process_backend: ProcessBackendKind::None,
            network_mode: NetworkMode::Deny,
            secret_mode: SecretMode::ScrubbedEnv,
            requires_filesystem: false,
            requires_process: false,
            requires_network: false,
            requires_secret: false,
        };
        let scope = ResourceScope::system();
        fn request<'a>(
            plan: &'a ExecutionPlan,
            scope: &'a ResourceScope,
        ) -> InvocationServicesResolutionRequest<'a> {
            InvocationServicesResolutionRequest {
                plan,
                scope,
                mounts: None,
            }
        }
        assert!(
            original
                .resolve(request(&plan, &scope))
                .unwrap()
                .child_tasks
                .is_none()
        );
        let services = bound.resolve(request(&plan, &scope)).unwrap();
        assert!(Arc::ptr_eq(services.child_tasks.as_ref().unwrap(), &child));
        assert!(Arc::ptr_eq(&services.filesystem, &filesystem));
        assert!(Arc::ptr_eq(&services.process, &process));
        assert!(services.runtime_http_egress.is_none());
        assert!(services.secret_store.is_none());
        plan.capability = CapabilityId::new(crate::ECHO_CAPABILITY_ID).unwrap();
        assert!(
            bound
                .resolve(request(&plan, &scope))
                .unwrap()
                .child_tasks
                .is_none()
        );
        // A bound child port cannot repair or bypass a rejected placement.
        plan.requires_filesystem = true;
        plan.filesystem_backend = FilesystemBackendKind::TenantWorkspace;
        assert_eq!(
            bound.resolve(request(&plan, &scope)).unwrap_err(),
            original.resolve(request(&plan, &scope)).unwrap_err()
        );
        plan.filesystem_backend = FilesystemBackendKind::HostWorkspace;
        plan.requires_process = true;
        plan.process_backend = ProcessBackendKind::TenantSandbox;
        assert_eq!(
            bound.resolve(request(&plan, &scope)).unwrap_err(),
            original.resolve(request(&plan, &scope)).unwrap_err()
        );
        plan.requires_process = false;
        plan.requires_network = true;
        assert_eq!(
            bound.resolve(request(&plan, &scope)).unwrap_err(),
            original.resolve(request(&plan, &scope)).unwrap_err()
        );
        plan.requires_network = false;
        plan.requires_secret = true;
        assert_eq!(
            bound.resolve(request(&plan, &scope)).unwrap_err(),
            original.resolve(request(&plan, &scope)).unwrap_err()
        );
    }
}
