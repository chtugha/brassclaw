//! Minimal bridge adapter — converts engine types to Reborn capability types.
//!
//! This adapter is a thin translation layer between the engine's `EffectExecutor`
//! interface and the Reborn `CapabilityHost`. All security controls (authorization,
//! approval, run state, obligations) are handled by `CapabilityHost::invoke_json()`.

use std::sync::Arc;
use std::time::Instant;

use brassclaw_capabilities::{CapabilityHost, CapabilityInvocationRequest};
use brassclaw_engine::{
    ActionDef, ActionInventory, ActionResult, CapabilityLease, CapabilitySummary,
    EffectExecutor, EngineError, ThreadExecutionContext,
};
use brassclaw_extensions::SharedExtensionRegistry;
use brassclaw_host_api::{
    AgentId, CapabilityDispatcher, CapabilityId, CapabilitySet, CorrelationId, ExecutionContext,
    ExtensionId, InvocationId, MountView, ProjectId, ResourceEstimate, ResourceScope, RuntimeKind,
    TenantId, TrustClass, UserId,
};
use brassclaw_safety::SafetyLayer;
use brassclaw_trust::TrustDecision;
use tracing::{debug, warn};

/// Minimal bridge adapter that delegates to Reborn's CapabilityHost.
///
/// This adapter's sole responsibility is type conversion between the engine's
/// `EffectExecutor` interface and the Reborn capability system. All security
/// controls are handled by the `CapabilityHost`.
pub struct EffectBridgeAdapter<D>
where
    D: CapabilityDispatcher + ?Sized + 'static,
{
    /// Reborn capability host (handles authorization, approval, dispatch)
    capability_host: Arc<CapabilityHost<'static, D>>,
    /// Extension registry for listing available capabilities
    extension_registry: Arc<SharedExtensionRegistry>,
    /// Safety layer for output sanitization (LLM protection)
    safety: Arc<SafetyLayer>,
}

impl<D> EffectBridgeAdapter<D>
where
    D: CapabilityDispatcher + ?Sized,
{
    pub fn new(
        capability_host: Arc<CapabilityHost<'static, D>>,
        extension_registry: Arc<SharedExtensionRegistry>,
        safety: Arc<SafetyLayer>,
    ) -> Self {
        Self {
            capability_host,
            extension_registry,
            safety,
        }
    }

    /// Access the underlying safety layer.
    ///
    /// The bridge router uses this to redact verbose-only observability
    /// events through the leak detector before broadcasting them on SSE.
    pub fn safety(&self) -> &Arc<SafetyLayer> {
        &self.safety
    }

    /// Convert engine ThreadExecutionContext to capability ExecutionContext.
    fn to_execution_context(
        context: &ThreadExecutionContext,
    ) -> Result<ExecutionContext, EngineError> {
        let invocation_id = InvocationId::new();
        let user_id = UserId::new(&context.user_id)
            .map_err(|e| EngineError::Effect { reason: format!("Invalid user_id: {}", e) })?;
        let tenant_id = TenantId::new("default")
            .map_err(|e| EngineError::Effect { reason: format!("Invalid tenant_id: {}", e) })?;
        let agent_id = AgentId::new("default")
            .map_err(|e| EngineError::Effect { reason: format!("Invalid agent_id: {}", e) })?;
        let project_id = ProjectId::new("bootstrap")
            .map_err(|e| EngineError::Effect { reason: format!("Invalid project_id: {}", e) })?;
        
        let resource_scope = ResourceScope {
            tenant_id: tenant_id.clone(),
            user_id: user_id.clone(),
            agent_id: Some(agent_id),
            project_id: Some(project_id),
            mission_id: None,
            thread_id: None,
            invocation_id,
        };

        Ok(ExecutionContext {
            invocation_id,
            correlation_id: CorrelationId::new(),
            process_id: None,
            parent_process_id: None,
            tenant_id,
            user_id,
            agent_id: resource_scope.agent_id.clone(),
            project_id: resource_scope.project_id.clone(),
            mission_id: None,
            thread_id: None,
            extension_id: ExtensionId::new("brassclaw.builtin")
                .map_err(|e| EngineError::Effect { reason: format!("Invalid extension_id: {}", e) })?,
            runtime: RuntimeKind::FirstParty,
            trust: TrustClass::UserTrusted,
            grants: CapabilitySet { grants: Vec::new() },
            mounts: MountView { mounts: Vec::new() },
            resource_scope,
        })
    }

    /// Convert capability invocation result to engine ActionResult.
    fn to_action_result(
        action_name: &str,
        context: &ThreadExecutionContext,
        result: brassclaw_capabilities::CapabilityInvocationResult,
        duration: std::time::Duration,
        safety: &SafetyLayer,
    ) -> ActionResult {
        // Sanitize output through safety layer
        let output_str =
            serde_json::to_string(&result.dispatch.output).unwrap_or_else(|_| "{}".to_string());
        let sanitized = safety.sanitize_tool_output(action_name, &output_str);

        ActionResult {
            call_id: context
                .current_call_id
                .clone()
                .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple())),
            action_name: action_name.to_string(),
            output: serde_json::from_str(&sanitized.content)
                .unwrap_or(serde_json::json!({"content": sanitized.content})),
            is_error: false,
            duration,
        }
    }

    /// Convert capability invocation error to engine ActionResult.
    fn error_to_action_result(
        action_name: &str,
        context: &ThreadExecutionContext,
        error: brassclaw_capabilities::CapabilityInvocationError,
        duration: std::time::Duration,
        safety: &SafetyLayer,
    ) -> ActionResult {
        let error_msg = format!("Capability '{}' failed: {:?}", action_name, error);
        let sanitized = safety.sanitize_tool_output(action_name, &error_msg);

        ActionResult {
            call_id: context
                .current_call_id
                .clone()
                .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple())),
            action_name: action_name.to_string(),
            output: serde_json::json!({"error": sanitized.content}),
            is_error: true,
            duration,
        }
    }
}

#[async_trait::async_trait]
impl<D> EffectExecutor for EffectBridgeAdapter<D>
where
    D: CapabilityDispatcher + Send + Sync + ?Sized,
{
    async fn execute_action(
        &self,
        action_name: &str,
        parameters: serde_json::Value,
        _lease: &CapabilityLease,
        context: &ThreadExecutionContext,
    ) -> Result<ActionResult, EngineError> {
        let start = Instant::now();

        debug!(
            action_name = %action_name,
            user_id = %context.user_id,
            thread_id = %context.thread_id,
            "Bridge: executing action via CapabilityHost"
        );

        // Convert engine types to capability types
        let capability_id = CapabilityId::new(action_name)
            .map_err(|e| EngineError::Effect { reason: format!("Invalid capability_id: {}", e) })?;
        let exec_context = Self::to_execution_context(context)?;

        let request = CapabilityInvocationRequest {
            context: exec_context,
            capability_id: capability_id.clone(),
            estimate: ResourceEstimate::default(),
            input: parameters.clone(),
            trust_decision: TrustDecision {
                effective_trust: brassclaw_trust::EffectiveTrustClass::user_trusted(),
                authority_ceiling: brassclaw_trust::AuthorityCeiling::empty(),
                provenance: brassclaw_trust::TrustProvenance::Default,
                evaluated_at: chrono::Utc::now(),
            },
        };

        // Invoke via CapabilityHost (handles all security: authorization,
        // approval, run state, obligations, dispatch)
        match self.capability_host.invoke_json(request).await {
            Ok(result) => {
                debug!(
                    action_name = %action_name,
                    duration_ms = ?start.elapsed().as_millis(),
                    "Bridge: action completed successfully"
                );
                Ok(Self::to_action_result(
                    action_name,
                    context,
                    result,
                    start.elapsed(),
                    &self.safety,
                ))
            }
            Err(e) => {
                warn!(
                    action_name = %action_name,
                    error = ?e,
                    duration_ms = ?start.elapsed().as_millis(),
                    "Bridge: action failed"
                );
                Ok(Self::error_to_action_result(
                    action_name,
                    context,
                    e,
                    start.elapsed(),
                    &self.safety,
                ))
            }
        }
    }

    async fn available_actions(
        &self,
        _leases: &[CapabilityLease],
        _context: &ThreadExecutionContext,
    ) -> Result<Vec<ActionDef>, EngineError> {
        let registry_snapshot = self.extension_registry.snapshot();
        let mut actions = Vec::new();

        // Convert capabilities to ActionDef format
        for capability in registry_snapshot.capabilities() {
            actions.push(ActionDef {
                name: capability.id.to_string(),
                description: capability.description.clone(),
                parameters_schema: capability.parameters_schema.clone(),
                effects: Vec::new(),
                requires_approval: capability.default_permission != brassclaw_host_api::PermissionMode::Allow,
                model_tool_surface: brassclaw_engine::ModelToolSurface::FullSchema,
                discovery: None,
            });
        }

        debug!(
            action_count = actions.len(),
            "Bridge: listed available actions"
        );

        Ok(actions)
    }

    async fn available_action_inventory(
        &self,
        leases: &[CapabilityLease],
        context: &ThreadExecutionContext,
    ) -> Result<ActionInventory, EngineError> {
        let inline = self.available_actions(leases, context).await?;
        Ok(ActionInventory {
            inline,
            discoverable: Vec::new(),
        })
    }

    async fn available_capabilities(
        &self,
        _leases: &[CapabilityLease],
        _context: &ThreadExecutionContext,
    ) -> Result<Vec<CapabilitySummary>, EngineError> {
        let registry_snapshot = self.extension_registry.snapshot();
        let summaries: Vec<CapabilitySummary> = registry_snapshot
            .capabilities()
            .map(|cap| CapabilitySummary {
                name: cap.id.to_string(),
                display_name: Some(cap.id.to_string()),
                kind: brassclaw_engine::CapabilitySummaryKind::Provider,
                status: brassclaw_engine::CapabilityStatus::Ready,
                description: Some(cap.description.clone()),
                action_preview: Vec::new(),
                routing_hint: None,
            })
            .collect();

        debug!(
            capability_count = summaries.len(),
            "Bridge: listed available capabilities"
        );

        Ok(summaries)
    }
}

#[cfg(test)]
mod tests {
    // TODO: Add tests for type conversion
    // TODO: Add tests for error handling
    // TODO: Add integration tests with mock CapabilityHost
}

