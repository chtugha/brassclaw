# Steps 7-8 Implementation Progress

## Overview
Implementing the complete Reborn architecture integration for v2 capabilities, including built-in capability dispatcher, permission storage, and permission resolution.

## Completed Work

### 1. ✅ Built-in Capability Dispatcher
**File**: `/Volumes/SSDE/brassclaw/src/capabilities/dispatcher.rs` (502 lines)

**Implementation**:
- Implements `brassclaw_host_api::CapabilityDispatcher` trait
- Routes all 47 built-in capabilities across 13 domain modules
- Handles both async and sync execute functions
- Tracks resource usage (wall clock time, output bytes)
- Returns proper `CapabilityDispatchResult` with receipts
- Maps capability errors to `DispatchError::FirstParty`

**Capabilities Routed**:
- Filesystem (7): read_file, write_file, list_dir, apply_patch, glob, grep, file_undo
- Shell (1): shell
- Network (1): http
- Memory (4): memory_read, memory_write, memory_search, memory_tree
- Messaging (1): message
- Jobs (6): create_job, cancel_job, list_jobs, job_status, job_events, job_prompt
- Routines (7): routine_create, routine_update, routine_delete, routine_list, routine_history, routine_fire, event_emit
- Skills (4): skill_install, skill_remove, skill_list, skill_search
- Extensions (9): tool_install, tool_remove, tool_list, tool_search, tool_upgrade, tool_auth, tool_info, extension_info, tool_permission_set
- Secrets (2): secret_list, secret_delete
- Images (3): image_generate, image_analyze, image_edit
- System (7): echo, json, time, system_version, system_tools_list, plan_update, restart
- Pairing (1): pairing_approve

### 2. ✅ Permission Storage
**File**: `/Volumes/SSDE/brassclaw/src/capabilities/permissions.rs` (330 lines)

**Implementation**:
- `CapabilityPermissionStore` trait for permission override storage
- `InMemoryPermissionStore` implementation (for testing/development)
- `DbPermissionStore` implementation (stub for database backend)
- Full CRUD operations: get, set, delete, list, clear
- Tenant isolation (per-user permissions)
- Comprehensive test coverage

**API**:
```rust
pub trait CapabilityPermissionStore: Send + Sync {
    async fn get_permission(&self, tenant_id: &str, capability_id: &str) 
        -> Result<Option<PermissionMode>, DatabaseError>;
    async fn set_permission(&self, tenant_id: &str, capability_id: &str, mode: PermissionMode) 
        -> Result<(), DatabaseError>;
    async fn delete_permission(&self, tenant_id: &str, capability_id: &str) 
        -> Result<bool, DatabaseError>;
    async fn list_overrides(&self, tenant_id: &str) 
        -> Result<HashMap<String, PermissionMode>, DatabaseError>;
    async fn clear_overrides(&self, tenant_id: &str) 
        -> Result<usize, DatabaseError>;
}
```

### 3. ✅ Permission Resolution
**File**: `/Volumes/SSDE/brassclaw/src/capabilities/resolver.rs` (289 lines)

**Implementation**:
- `PermissionResolver` for hierarchical permission resolution
- Resolution order: Override → Descriptor Default → Deny (fail-closed)
- Dynamic capability registration/unregistration
- Provider-based capability management
- Comprehensive test coverage

**Resolution Hierarchy**:
1. **Override**: Tenant-specific permission from storage
2. **Default**: Capability descriptor's default_permission
3. **Deny**: Fail-closed if capability not found

**API**:
```rust
impl PermissionResolver {
    pub async fn resolve_permission(&self, tenant_id: &str, capability_id: &str) 
        -> PermissionMode;
    pub async fn get_descriptor(&self, capability_id: &str) 
        -> Option<CapabilityDescriptor>;
    pub async fn list_descriptors(&self) -> Vec<CapabilityDescriptor>;
    pub async fn register_descriptors(&self, descriptors: Vec<CapabilityDescriptor>);
    pub async fn unregister_provider(&self, provider_id: &str) -> usize;
    pub async fn is_registered(&self, capability_id: &str) -> bool;
    pub async fn list_provider_capabilities(&self, provider_id: &str) 
        -> Vec<CapabilityId>;
}
```

### 4. ✅ Module Exports
**File**: `/Volumes/SSDE/brassclaw/src/capabilities/mod.rs`

**Changes**:
- Added `pub mod permissions;`
- Added `pub mod resolver;`
- Exported `CapabilityPermissionStore`, `DbPermissionStore`, `InMemoryPermissionStore`
- Exported `PermissionResolver`

## Remaining Work

### 5. ⏳ Database Integration
**Tasks**:
- Add `CapabilityPermissionStore` trait to `Database` supertrait in `/Volumes/SSDE/brassclaw/src/db/mod.rs`
- Implement trait methods in `PgBackend` (postgres.rs)
- Implement trait methods in libSQL backend
- Add migration for `capability_permissions` table
- Complete `DbPermissionStore` implementation

**Migration SQL** (libSQL):
```sql
CREATE TABLE IF NOT EXISTS capability_permissions (
    tenant_id TEXT NOT NULL,
    capability_id TEXT NOT NULL,
    permission_mode TEXT NOT NULL CHECK (permission_mode IN ('allow', 'ask', 'deny')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (tenant_id, capability_id)
);

CREATE INDEX IF NOT EXISTS idx_capability_permissions_tenant 
    ON capability_permissions(tenant_id);
```

### 6. ⏳ ExtensionRegistry Extensions
**File**: `/Volumes/SSDE/brassclaw/crates/brassclaw_extensions/src/lib.rs` (or similar)

**Tasks**:
- Add `register_builtin_capabilities(descriptors: Vec<CapabilityDescriptor>)` method
- Add `get_capability(id: &CapabilityId) -> Option<&CapabilityDescriptor>` method
- Add `list_all_capabilities() -> Vec<&CapabilityDescriptor>` method

### 7. ⏳ RebornServicesApi Extensions
**File**: `/Volumes/SSDE/brassclaw/crates/brassclaw_product_workflow/src/reborn_services.rs`

**Tasks**:
- Add `list_capabilities()` method
- Add `update_capability_permission()` method
- Add `RebornCapabilityInfo` DTO

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

### 8. ⏳ Bridge Layer Rewrite
**Files**:
- `/Volumes/SSDE/brassclaw/src/bridge/effect_adapter.rs`
- `/Volumes/SSDE/brassclaw/src/bridge/router.rs`
- `/Volumes/SSDE/brassclaw/src/bridge/action_projector.rs`
- `/Volumes/SSDE/brassclaw/src/bridge/tool_permissions.rs` (DELETE)

**Tasks**:
- Remove `tools: Arc<ToolRegistry>` field from `EffectBridgeAdapter`
- Add `capability_host: Arc<CapabilityHost<BuiltinCapabilityDispatcher>>`
- Add `permission_resolver: Arc<PermissionResolver>`
- Rewrite `execute_action_internal` to use `CapabilityHost`
- Update `available_actions()` to use `ExtensionRegistry`
- Remove all `ToolRegistry` lookups
- Replace `PermissionState` checks with `PermissionMode` checks
- Update `router.rs` to use capability descriptors
- Update `action_projector.rs` to use `CapabilityLease`
- Delete `tool_permissions.rs`

### 9. ⏳ Startup Registration
**File**: `/Volumes/SSDE/brassclaw/src/main.rs` or `/Volumes/SSDE/brassclaw/src/app.rs`

**Tasks**:
```rust
// Register all built-in capabilities at startup
let descriptors = brassclaw::capabilities::register_all();
extension_registry.register_builtin_capabilities(descriptors.clone())?;

// Create permission store and resolver
let permission_store = Arc::new(InMemoryPermissionStore::new()); // or DbPermissionStore
let permission_resolver = Arc::new(PermissionResolver::new(permission_store.clone(), descriptors));

// Create dispatcher with all context objects
let dispatcher = BuiltinCapabilityDispatcher::new(
    filesystem_ctx,
    shell_ctx,
    network_ctx,
    memory_ctx,
    messaging_ctx,
    jobs_ctx,
    routines_ctx,
    skills_ctx,
    extensions_ctx,
    secrets_ctx,
    images_ctx,
    system_ctx,
    pairing_ctx,
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

## Testing Strategy

### Unit Tests
- ✅ Permission storage (InMemoryPermissionStore)
- ✅ Permission resolution hierarchy
- ✅ Tenant isolation
- ✅ Dynamic capability registration/unregistration

### Integration Tests (TODO)
- [ ] Dispatcher routing for all 47 capabilities
- [ ] Resource usage tracking accuracy
- [ ] Error handling and mapping
- [ ] Database-backed permission storage
- [ ] CapabilityHost integration
- [ ] Bridge layer with CapabilityHost

### End-to-End Tests (TODO)
- [ ] Full request flow: LLM → Bridge → CapabilityHost → Dispatcher → Capability
- [ ] Permission override persistence
- [ ] Extension capability registration/unregistration
- [ ] Multi-tenant permission isolation

## Files Created/Modified

### Created:
- `/Volumes/SSDE/brassclaw/src/capabilities/dispatcher.rs` (502 lines)
- `/Volumes/SSDE/brassclaw/src/capabilities/permissions.rs` (330 lines)
- `/Volumes/SSDE/brassclaw/src/capabilities/resolver.rs` (289 lines)
- `STEP_7_8_IMPLEMENTATION_PLAN.md` (283 lines)
- `DISPATCHER_IMPLEMENTATION_SUMMARY.md` (165 lines)
- `STEPS_7_8_PROGRESS.md` (this file)

### Modified:
- `/Volumes/SSDE/brassclaw/src/capabilities/mod.rs` (added dispatcher, permissions, resolver modules)

### Deleted:
- `MIGRATION_STATUS_ANALYSIS.md` (outdated)

## Next Immediate Steps

1. **Database Integration**: Add `CapabilityPermissionStore` to `Database` trait and implement in backends
2. **ExtensionRegistry**: Add capability registration methods
3. **RebornServicesApi**: Add capability management endpoints
4. **Bridge Rewrite**: Replace `ToolRegistry` with `CapabilityHost`
5. **Startup Wiring**: Register capabilities and create dispatcher at boot

## Status Summary

**Completed**: 3/9 major components (33%)
- ✅ Built-in Capability Dispatcher
- ✅ Permission Storage (trait + in-memory impl)
- ✅ Permission Resolution

**In Progress**: Steps 7-8 (Rewrite EffectBridgeAdapter + V2 permission storage)

**Remaining**: 6/9 major components
- Database integration
- ExtensionRegistry extensions
- RebornServicesApi extensions
- Bridge layer rewrite
- Startup registration
- Testing