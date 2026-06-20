# Corrected Bridge Layer Approach

## Understanding the Architecture

After re-reading the spec and requirements, I now understand the correct approach:

### What We've Built (Steps 1-6 ✅)
1. **V2 Capability Modules** (`./src/capabilities/`)
   - 47 capabilities across 13 domain modules
   - Each has an `execute_*()` function
   - Each has a `CapabilityDescriptor`

2. **BuiltinCapabilityDispatcher** (`./src/capabilities/dispatcher.rs`)
   - Routes capability IDs to the correct `execute_*()` function
   - Implements the dispatch logic
   - Returns `ActionResult`

3. **Permission Infrastructure**
   - `CapabilityPermissionStore` - stores permission overrides
   - `PermissionResolver` - resolves permissions (override → default → deny)
   - Database integration for both LibSQL and PostgreSQL

4. **RebornServicesApi Extensions**
   - API endpoints for listing/updating capability permissions
   - DTOs for capability management

### What Needs to Happen (Step 7 - Bridge Rewrite)

The `EffectBridgeAdapter` currently:
```rust
pub struct EffectBridgeAdapter {
    tools: Arc<ToolRegistry>,  // ← V1 system
    // ... other fields
}
```

It should become:
```rust
pub struct EffectBridgeAdapter {
    dispatcher: Arc<BuiltinCapabilityDispatcher>,  // ← V2 system
    permission_resolver: Arc<PermissionResolver>,   // ← V2 permissions
    extension_registry: Arc<SharedExtensionRegistry>,  // ← For extension capabilities
    // ... other fields (safety, hooks, etc. stay the same)
}
```

### The Rewrite Strategy

**NOT** a complete rewrite of 8629 lines. Instead:

1. **Replace the dispatch mechanism**:
   - Remove `tools: Arc<ToolRegistry>`
   - Add `dispatcher: Arc<BuiltinCapabilityDispatcher>`
   - Add `permission_resolver: Arc<PermissionResolver>`
   - Add `extension_registry: Arc<SharedExtensionRegistry>`

2. **Update `execute_action` method**:
   - Currently calls `self.tools.get(action_name)` → v1 tool
   - Should call `self.dispatcher.dispatch(capability_id, params)` → v2 capability
   - Check permissions via `self.permission_resolver.resolve_permission()`

3. **Update `available_actions` method**:
   - Currently queries `self.tools.list_all()`
   - Should query `self.extension_registry.capabilities()`

4. **Keep all security controls**:
   - Safety layer ✅
   - Hooks ✅
   - Rate limiting ✅
   - Approval workflow ✅
   - Sandbox/workspace mounts ✅

### Why This Is Simpler Than Expected

The 8629 lines include:
- Security controls (keep as-is)
- Hook system (keep as-is)
- Rate limiting (keep as-is)
- Approval workflow (keep as-is)
- Mission manager (keep as-is)
- External tool catalog (keep as-is)
- Workspace mounts (keep as-is)

**Only the tool dispatch logic needs to change** - probably 200-300 lines of actual changes.

### Concrete Steps

1. **Modify struct fields** (lines ~47-92):
   ```rust
   - tools: Arc<ToolRegistry>,
   + dispatcher: Arc<BuiltinCapabilityDispatcher>,
   + permission_resolver: Arc<PermissionResolver>,
   + extension_registry: Arc<SharedExtensionRegistry>,
   ```

2. **Update constructor** (`new()` method):
   - Accept new parameters
   - Remove ToolRegistry parameter

3. **Rewrite `execute_action` method**:
   - Find where it calls `self.tools.get()`
   - Replace with `self.dispatcher.dispatch()`
   - Add permission check via `self.permission_resolver`

4. **Rewrite `available_actions` method**:
   - Find where it calls `self.tools.list_all()`
   - Replace with `self.extension_registry.capabilities()`

5. **Remove v1 imports**:
   - Remove `use crate::tools::ToolRegistry`
   - Remove `use crate::tools::permissions::PermissionState`
   - Remove `use crate::bridge::tool_permissions`

6. **Delete `tool_permissions.rs`**:
   - No longer needed - permissions handled by PermissionResolver

### Estimated Effort

- **Actual code changes**: 200-300 lines
- **Testing**: Verify all 47 capabilities still work
- **Time**: 2-3 hours (not 8-12 hours)

### Next Steps

1. Read the `execute_action` method to understand current dispatch logic
2. Read the `available_actions` method to understand current listing logic
3. Make targeted changes to replace ToolRegistry with our dispatcher
4. Test thoroughly

This is a **surgical replacement**, not a complete rewrite.