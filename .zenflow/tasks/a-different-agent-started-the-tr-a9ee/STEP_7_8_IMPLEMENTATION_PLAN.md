# Steps 7-8 Implementation Plan: CapabilityDispatcher + Permission Storage

## Overview
Implement the complete Reborn architecture integration for v2 capabilities, including:
1. Built-in capability dispatcher
2. Permission storage system
3. Bridge layer rewrite
4. Dynamic capability registration

## Part A: Built-in Capability Dispatcher

### File: `./src/capabilities/dispatcher.rs`

**Purpose**: Implement `brassclaw_host_api::CapabilityDispatcher` trait to route capability IDs to v2 `execute_*` functions.

**Implementation**:
```rust
pub struct BuiltinCapabilityDispatcher {
    // Context objects for each capability domain
    filesystem_ctx: Arc<FilesystemContext>,
    shell_ctx: Arc<ShellContext>,
    network_ctx: Arc<NetworkContext>,
    memory_ctx: Arc<MemoryContext>,
    messaging_ctx: Arc<MessagingContext>,
    jobs_ctx: Arc<JobsContext>,
    routines_ctx: Arc<RoutinesContext>,
    skills_ctx: Arc<SkillsContext>,
    extensions_ctx: Arc<ExtensionsContext>,
    secrets_ctx: Arc<SecretsContext>,
    images_ctx: Arc<ImagesContext>,
    system_ctx: Arc<SystemContext>,
    pairing_ctx: Arc<PairingContext>,
}

#[async_trait]
impl CapabilityDispatcher for BuiltinCapabilityDispatcher {
    async fn dispatch_json(
        &self,
        request: CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchResult, DispatchError> {
        // Route to appropriate execute_* function based on capability_id
        // Track resource usage
        // Return CapabilityDispatchResult
    }
}
```

**Key Points**:
- No hardcoded match block - use dynamic lookup
- Each context is Arc-wrapped for cheap cloning
- Proper error mapping from capability errors to DispatchError
- Resource usage tracking

## Part B: Permission Storage

### 1. Database Migration

**File**: `./migrations/YYYYMMDD_capability_permissions.sql` (or in-code migration)

```sql
CREATE TABLE IF NOT EXISTS capability_permissions (
    tenant_id TEXT NOT NULL,
    capability_id TEXT NOT NULL,
    permission_mode TEXT NOT NULL CHECK (permission_mode IN ('allow', 'ask', 'deny')),
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, capability_id)
);

CREATE INDEX idx_capability_permissions_tenant ON capability_permissions(tenant_id);
```

### 2. Permission Storage Implementation

**File**: `./src/capabilities/permissions.rs` (new)

```rust
pub struct CapabilityPermissionStore {
    db: Arc<dyn Database>,
}

impl CapabilityPermissionStore {
    pub async fn get_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<PermissionMode>, Error>;
    
    pub async fn set_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: PermissionMode,
    ) -> Result<(), Error>;
    
    pub async fn list_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<HashMap<String, PermissionMode>, Error>;
}
```

### 3. Capability Registry Extension

**Extend**: `brassclaw_extensions::ExtensionRegistry`

Add methods:
- `register_builtin_capabilities(descriptors: Vec<CapabilityDescriptor>)`
- `get_capability(id: &CapabilityId) -> Option<&CapabilityDescriptor>`
- `list_all_capabilities() -> Vec<&CapabilityDescriptor>`

### 4. Permission Resolution

**File**: `./src/capabilities/resolver.rs` (new)

```rust
pub struct PermissionResolver {
    registry: Arc<ExtensionRegistry>,
    store: Arc<CapabilityPermissionStore>,
}

impl PermissionResolver {
    pub async fn resolve_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> PermissionMode {
        // 1. Check store for override
        if let Some(override_mode) = self.store.get_permission(tenant_id, capability_id).await {
            return override_mode;
        }
        
        // 2. Fall back to descriptor default
        if let Some(descriptor) = self.registry.get_capability(capability_id) {
            return descriptor.default_permission;
        }
        
        // 3. Fail-closed default
        PermissionMode::Deny
    }
}
```

## Part C: RebornServicesApi Extensions

### File: `./crates/brassclaw_product_workflow/src/reborn_services.rs`

**Add to trait**:
```rust
#[async_trait]
pub trait RebornServicesApi {
    // ... existing methods ...
    
    async fn list_capabilities(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<Vec<RebornCapabilityInfo>, RebornServicesError>;
    
    async fn update_capability_permission(
        &self,
        caller: WebUiAuthenticatedCaller,
        capability_id: &str,
        permission_mode: PermissionMode,
    ) -> Result<(), RebornServicesError>;
}
```

**New DTO**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornCapabilityInfo {
    pub id: String,
    pub description: String,
    pub provider: String,
    pub effects: Vec<String>,
    pub permission_mode: PermissionMode,
    pub default_permission: PermissionMode,
}
```

## Part D: Bridge Layer Rewrite

### 1. Update EffectBridgeAdapter

**File**: `./src/bridge/effect_adapter.rs`

**Changes**:
- Remove `tools: Arc<ToolRegistry>` field
- Add `capability_host: Arc<CapabilityHost<BuiltinCapabilityDispatcher>>`
- Add `permission_resolver: Arc<PermissionResolver>`
- Rewrite `execute_action_internal` to use CapabilityHost
- Remove all ToolRegistry lookups
- Replace PermissionState checks with PermissionMode checks

### 2. Update Router

**File**: `./src/bridge/router.rs`

**Changes**:
- Remove ToolRegistry usage for action discovery
- Use ExtensionRegistry for capability listing
- Update available_actions to use capability descriptors

### 3. Update Action Projector

**File**: `./src/bridge/action_projector.rs`

**Changes**:
- Remove v1 tool permission references
- Use CapabilityLease for authorization checks

### 4. Delete Tool Permissions Stub

**File**: `./src/bridge/tool_permissions.rs`

**Action**: Delete entirely

## Part E: Startup Registration

### File: `./src/main.rs` or `./src/app.rs`

**Add**:
```rust
// Register all built-in capabilities at startup
let descriptors = brassclaw::capabilities::register_all();
extension_registry.register_builtin_capabilities(descriptors)?;

// Create dispatcher
let dispatcher = BuiltinCapabilityDispatcher::new(
    filesystem_ctx,
    shell_ctx,
    // ... all contexts
);

// Create CapabilityHost
let capability_host = CapabilityHost::new(
    &extension_registry,
    &dispatcher,
    &authorizer,
)
.with_run_state_approval_store(&combined_store)
.with_capability_leases(&lease_store)
.with_process_manager(&process_manager);

// Pass to EffectBridgeAdapter
let bridge = EffectBridgeAdapter::new(
    capability_host,
    permission_resolver,
    safety,
    hooks,
);
```

## Implementation Order

1. ✅ **Create dispatcher.rs** - Implement CapabilityDispatcher trait
2. ✅ **Create permissions.rs** - Implement permission storage
3. ✅ **Create resolver.rs** - Implement permission resolution
4. ✅ **Extend ExtensionRegistry** - Add capability registration methods
5. ✅ **Add RebornServicesApi methods** - Capability management API
6. ✅ **Rewrite EffectBridgeAdapter** - Use CapabilityHost instead of ToolRegistry
7. ✅ **Update router.rs** - Remove ToolRegistry usage
8. ✅ **Update action_projector.rs** - Use CapabilityLease
9. ✅ **Delete tool_permissions.rs** - Remove stub
10. ✅ **Update startup code** - Register capabilities at boot
11. ✅ **Write tests** - Permission resolution, dispatcher routing
12. ✅ **Verify** - cargo build && cargo test

## Success Criteria

- [ ] `cargo build` succeeds with zero errors
- [ ] `cargo test` passes all tests
- [ ] No imports from `./src/tools/` in `./src/bridge/`
- [ ] Permission overrides persist correctly
- [ ] All 47 v2 capabilities route through dispatcher
- [ ] Dynamic capability registration works
- [ ] WASM/MCP capabilities can be added without code changes

## Notes

- This is a large change that touches many files
- Consider implementing in feature branch
- May need multiple PRs for review
- Keep v1 code temporarily for comparison during development