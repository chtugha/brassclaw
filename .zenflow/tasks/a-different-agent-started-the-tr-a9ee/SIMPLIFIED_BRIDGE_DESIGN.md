# Simplified Bridge Design

## Core Insight

The Reborn `CapabilityHost` already provides ALL security controls:
- Authorization via `TrustAwareCapabilityDispatchAuthorizer`
- Approval workflows via `Decision::RequireApproval`
- Run state tracking
- Obligation handling
- Dispatch to capabilities

**The bridge should be a thin adapter that just converts types!**

## Minimal Bridge Implementation

### New Structure (~200 lines instead of 8,629)

```rust
//! Minimal bridge adapter — converts engine types to capability types

use std::sync::Arc;
use brassclaw_engine::{
    ActionDef, ActionResult, CapabilityLease, EffectExecutor, EngineError,
    ThreadExecutionContext,
};
use brassclaw_capabilities::{CapabilityHost, CapabilityInvocationRequest};
use brassclaw_extensions::SharedExtensionRegistry;
use brassclaw_host_api::{CapabilityId, ExecutionContext, ResourceEstimate};
use brassclaw_safety::SafetyLayer;

pub struct EffectBridgeAdapter {
    capability_host: Arc<CapabilityHost<'static, dyn CapabilityDispatcher>>,
    extension_registry: Arc<SharedExtensionRegistry>,
    safety: Arc<SafetyLayer>,
}

impl EffectBridgeAdapter {
    pub fn new(
        capability_host: Arc<CapabilityHost<'static, dyn CapabilityDispatcher>>,
        extension_registry: Arc<SharedExtensionRegistry>,
        safety: Arc<SafetyLayer>,
    ) -> Self {
        Self {
            capability_host,
            extension_registry,
            safety,
        }
    }
    
    pub fn safety(&self) -> &Arc<SafetyLayer> {
        &self.safety
    }
}

#[async_trait::async_trait]
impl EffectExecutor for EffectBridgeAdapter {
    async fn execute_action(
        &self,
        action_name: &str,
        parameters: serde_json::Value,
        lease: &CapabilityLease,
        context: &ThreadExecutionContext,
    ) -> Result<ActionResult, EngineError> {
        // Convert engine types to capability types
        let capability_id = CapabilityId::from(action_name.to_string());
        
        let exec_context = ExecutionContext {
            invocation_id: uuid::Uuid::new_v4(), // Generate invocation ID
            resource_scope: context.user_id.clone().into(),
            trust_context: Default::default(),
        };
        
        let request = CapabilityInvocationRequest {
            capability_id: capability_id.clone(),
            context: exec_context,
            input: parameters.clone(),
            estimate: ResourceEstimate::default(),
            trust_decision: None,
        };
        
        // Invoke via CapabilityHost (handles all security)
        match self.capability_host.invoke_json(request).await {
            Ok(result) => {
                // Sanitize output through safety layer
                let output_str = serde_json::to_string(&result.output)
                    .unwrap_or_else(|_| "{}".to_string());
                let sanitized = self.safety.sanitize_tool_output(action_name, &output_str);
                
                Ok(ActionResult {
                    call_id: context.current_call_id.clone()
                        .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple())),
                    action_name: action_name.to_string(),
                    output: serde_json::from_str(&sanitized.content)
                        .unwrap_or(serde_json::json!({"content": sanitized.content})),
                    is_error: false,
                    duration: std::time::Duration::from_secs(0), // TODO: track actual duration
                })
            }
            Err(e) => {
                // Convert capability error to engine error
                let error_msg = format!("Capability '{}' failed: {:?}", action_name, e);
                let sanitized = self.safety.sanitize_tool_output(action_name, &error_msg);
                
                Ok(ActionResult {
                    call_id: context.current_call_id.clone()
                        .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple())),
                    action_name: action_name.to_string(),
                    output: serde_json::json!({"error": sanitized.content}),
                    is_error: true,
                    duration: std::time::Duration::from_secs(0),
                })
            }
        }
    }
    
    async fn available_actions(
        &self,
        leases: &[CapabilityLease],
        context: &ThreadExecutionContext,
    ) -> Result<Vec<ActionDef>, EngineError> {
        let registry_snapshot = self.extension_registry.snapshot();
        let mut actions = Vec::new();
        
        // Convert capabilities to ActionDef format
        for capability in registry_snapshot.capabilities() {
            actions.push(ActionDef {
                name: capability.id.to_string(),
                description: capability.description.clone(),
                parameters: capability.input_schema.clone(),
            });
        }
        
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
            background: Vec::new(),
        })
    }
    
    async fn available_capabilities(
        &self,
        leases: &[CapabilityLease],
        context: &ThreadExecutionContext,
    ) -> Result<Vec<CapabilitySummary>, EngineError> {
        // Delegate to extension registry
        let registry_snapshot = self.extension_registry.snapshot();
        let summaries = registry_snapshot
            .capabilities()
            .map(|cap| CapabilitySummary {
                id: cap.id.to_string(),
                name: cap.id.to_string(),
                description: cap.description.clone(),
            })
            .collect();
        Ok(summaries)
    }
}
```

## What We Removed

All of this complexity is now handled by `CapabilityHost`:

- ❌ Tool registry lookups
- ❌ Manual permission checks
- ❌ Manual approval workflows
- ❌ Rate limiting (should be in authorizer)
- ❌ Hook interception (should be in authorizer)
- ❌ Auth gate handling (handled by CapabilityHost)
- ❌ Mission call routing (missions are capabilities now)
- ❌ Sandbox interception (should be in dispatcher)
- ❌ Tool-specific logic (all in capability execute functions)

## What We Kept

- ✅ Safety layer output sanitization (still needed for LLM protection)
- ✅ Type conversion (engine ↔ capability types)
- ✅ Extension registry access (for available_actions)

## Benefits

1. **Simplicity**: 200 lines vs 8,629 lines
2. **Correctness**: Uses Reborn's security model directly
3. **Maintainability**: Single responsibility (type conversion)
4. **Testability**: Much easier to test
5. **Performance**: No redundant checks

## Implementation Steps

1. Create new minimal `effect_adapter.rs` (~200 lines)
2. Update `router.rs` to use new adapter
3. Delete old 8,629-line file
4. Test with existing integration tests
5. Fix any type mismatches

## Estimated Effort

- **New bridge**: 2-3 hours
- **Integration**: 1-2 hours
- **Testing**: 1-2 hours
- **Total**: 4-7 hours (vs 12-17 hours for full rewrite)

## Next Steps

1. Implement minimal bridge
2. Wire into startup (main.rs/app.rs)
3. Test basic capability invocation
4. Iterate on type conversions as needed