# Bridge Integration Changes Specification

## Overview
This document specifies the exact changes needed to migrate `effect_adapter.rs` from v1 ToolRegistry to v2 CapabilityHost.

## File: `/Volumes/SSDE/brassclaw/src/bridge/effect_adapter.rs`

### 1. Import Changes

**Remove these v1 imports:**
```rust
use crate::tools::ToolRegistry;
use crate::tools::permissions::PermissionState;
use crate::tools::{ApprovalRequirement, Tool};
use crate::bridge::tool_permissions::{ToolPermissionResolution, ToolPermissionSnapshot};
```

**Add these v2 imports:**
```rust
use brassclaw_capabilities::CapabilityHost;
use brassclaw_extensions::{ExtensionRegistry, SharedExtensionRegistry};
use crate::capabilities::dispatcher::BuiltinCapabilityDispatcher;
use crate::capabilities::resolver::PermissionResolver;
use brassclaw_host_api::{CapabilityDispatcher, CapabilityId, ExecutionContext};
use brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer;
```

### 2. Struct Field Changes (lines 47-92)

**Replace:**
```rust
pub struct EffectBridgeAdapter {
    tools: Arc<ToolRegistry>,
    // ... other fields
}
```

**With:**
```rust
pub struct EffectBridgeAdapter {
    // V2 capability system
    extension_registry: Arc<SharedExtensionRegistry>,
    dispatcher: Arc<BuiltinCapabilityDispatcher>,
    permission_resolver: Arc<PermissionResolver>,
    authorizer: Arc<dyn TrustAwareCapabilityDispatchAuthorizer + Send + Sync>,
    
    // Keep existing fields
    safety: Arc<SafetyLayer>,
    hooks: Arc<HookRegistry>,
    auto_approve_tools: bool,
    auto_approved: RwLock<HashSet<String>>,
    call_count: std::sync::atomic::AtomicU32,
    rate_limiter: RateLimiter,
    mission_manager: RwLock<Option<Arc<brassclaw_engine::MissionManager>>>,
    auth_manager: RwLock<Option<Arc<AuthManager>>>,
    http_interceptor: RwLock<Option<Arc<dyn brassclaw_llm::recording::HttpInterceptor>>>,
    engine_store: RwLock<Option<Arc<dyn Store>>>,
    skill_registry: RwLock<Option<Arc<std::sync::RwLock<SkillRegistry>>>>,
    workspace_mounts: RwLock<Option<Arc<WorkspaceMounts>>>,
    capability_registry: RwLock<Option<Arc<CapabilityRegistry>>>,
    external_tool_catalog: RwLock<Option<Arc<crate::bridge::ExternalToolCatalog>>>,
}
```

### 3. Constructor Changes (lines 115-137)

**Replace `new()` method with:**
```rust
pub fn new(
    extension_registry: Arc<SharedExtensionRegistry>,
    dispatcher: Arc<BuiltinCapabilityDispatcher>,
    permission_resolver: Arc<PermissionResolver>,
    authorizer: Arc<dyn TrustAwareCapabilityDispatchAuthorizer + Send + Sync>,
    safety: Arc<SafetyLayer>,
    hooks: Arc<HookRegistry>,
) -> Self {
    Self {
        extension_registry,
        dispatcher,
        permission_resolver,
        authorizer,
        safety,
        hooks,
        auto_approve_tools: false,
        auto_approved: RwLock::new(HashSet::new()),
        call_count: std::sync::atomic::AtomicU32::new(0),
        rate_limiter: RateLimiter::new(),
        mission_manager: RwLock::new(None),
        auth_manager: RwLock::new(None),
        http_interceptor: RwLock::new(None),
        engine_store: RwLock::new(None),
        skill_registry: RwLock::new(None),
        workspace_mounts: RwLock::new(None),
        capability_registry: RwLock::new(None),
        external_tool_catalog: RwLock::new(None),
    }
}
```

### 4. Remove `tools()` accessor (lines 231-234)

**Delete:**
```rust
pub fn tools(&self) -> &Arc<ToolRegistry> {
    &self.tools
}
```

### 5. Core Method Rewrites

#### 5a. `execute_action_internal` (lines 1194-1796)

This method needs significant rewriting. Key changes:

1. **Remove v1 tool resolution** (lines 1210-1214):
   - Delete `self.tools.resolve_name()` call
   - Use capability ID directly from action_name

2. **Replace tool execution** (lines 1612-1620):
   - Remove `execute_tool_with_safety()` call
   - Use `CapabilityHost::invoke_json()` instead

3. **Update permission checks** (lines 1364-1367, 1484-1498):
   - Remove `resolved_user_permission_for_tool()` 
   - Use `permission_resolver.resolve()` instead

4. **Remove v1-specific checks** (lines 1369-1386):
   - Remove rate limiting (handled by v2 system)
   - Remove tool-specific permission enforcement

#### 5b. `available_actions` (lines 1896-1905)

**Replace with:**
```rust
async fn available_actions(
    &self,
    leases: &[CapabilityLease],
    context: &ThreadExecutionContext,
) -> Result<Vec<ActionDef>, EngineError> {
    let registry_snapshot = self.extension_registry.snapshot();
    let mut actions = Vec::new();
    
    // Convert capabilities to ActionDef format
    for capability in registry_snapshot.capabilities() {
        // Check if user has permission
        let permission = self.permission_resolver
            .resolve(&capability.id, &context.user_id)
            .await;
        
        if permission.is_allowed() {
            actions.push(ActionDef {
                name: capability.id.to_string(),
                description: capability.description.clone(),
                parameters: capability.input_schema.clone(),
            });
        }
    }
    
    // Merge external tools (Responses API)
    if let Some(catalog) = self.external_tool_catalog().await {
        let mut external: Vec<ActionDef> = Vec::new();
        for key in Self::external_tool_catalog_keys(context) {
            let entries = catalog.list(key).await;
            if !entries.is_empty() {
                external = entries;
                break;
            }
        }
        if !external.is_empty() {
            let existing: std::collections::HashSet<&str> =
                actions.iter().map(|a| a.name.as_str()).collect();
            let extras: Vec<ActionDef> = external
                .into_iter()
                .filter(|a| !existing.contains(a.name.as_str()))
                .collect();
            actions.extend(extras);
        }
    }
    
    Ok(actions)
}
```

### 6. Helper Method Changes

#### Remove these v1-specific methods:
- `resolved_user_permission_for_tool()` (if exists)
- `ensure_tool_not_disabled()` (lines 1367)
- Any other tool-registry-specific helpers

#### Update these methods to use v2:
- `is_known_credential()` (lines 1809-1814) - needs alternative implementation
- Any methods that call `self.tools.*`

### 7. Files to Delete After Migration

Once bridge integration is complete:
- `/Volumes/SSDE/brassclaw/src/bridge/tool_permissions.rs`
- Update `/Volumes/SSDE/brassclaw/src/bridge/router.rs` to remove v1 references
- Update `/Volumes/SSDE/brassclaw/src/bridge/action_projector.rs` to use v2

## Implementation Strategy

1. **Phase 1**: Update struct fields and constructor
2. **Phase 2**: Rewrite `available_actions` method
3. **Phase 3**: Rewrite `execute_action_internal` method core logic
4. **Phase 4**: Update all helper methods
5. **Phase 5**: Remove v1 imports and dead code
6. **Phase 6**: Update dependent files (router.rs, action_projector.rs)
7. **Phase 7**: Delete tool_permissions.rs

## Risk Mitigation

- Each phase should be tested independently
- Keep safety layer integration intact
- Preserve all security controls (hooks, approval, sanitization)
- Maintain backward compatibility with external tool catalog
- Ensure mission_* calls continue to work

## Testing Requirements

After implementation:
1. Test basic capability invocation
2. Test permission resolution
3. Test approval workflows
4. Test external tool catalog integration
5. Test mission_* function calls
6. Test authentication gates
7. Test sandbox interception
8. Run full integration test suite