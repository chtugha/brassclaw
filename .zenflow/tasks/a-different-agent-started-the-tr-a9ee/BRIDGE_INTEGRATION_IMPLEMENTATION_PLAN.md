# Bridge Layer Integration - Detailed Implementation Plan

## Overview

Replace `ToolRegistry` with `CapabilityHost` in the bridge layer to complete the v1-to-v2 migration.

## Architecture

```
EffectBridgeAdapter → CapabilityHost → BuiltinCapabilityDispatcher → execute_*() functions
```

## Key Components

### 1. CapabilityHost (exists in `brassclaw_capabilities` crate)
- **Location**: `/Volumes/SSDE/brassclaw/crates/brassclaw_capabilities/src/host.rs`
- **Purpose**: Manages capability invocation, authorization, approval, run state
- **Key methods**:
  - `invoke_json()` - Execute a capability
  - `resume_json()` - Resume after approval
  - `spawn_json()` - Spawn long-running process

### 2. BuiltinCapabilityDispatcher (we created this)
- **Location**: `/Volumes/SSDE/brassclaw/src/capabilities/dispatcher.rs`
- **Purpose**: Routes capability IDs to execute functions
- **Implements**: `brassclaw_host_api::CapabilityDispatcher` trait

### 3. EffectBridgeAdapter (needs modification)
- **Location**: `/Volumes/SSDE/brassclaw/src/bridge/effect_adapter.rs`
- **Size**: 8,629 lines
- **Current**: Wraps `ToolRegistry` to implement `EffectExecutor`
- **Target**: Use `CapabilityHost` instead

## Implementation Steps

### Step 1: Update Struct Fields

**File**: `/Volumes/SSDE/brassclaw/src/bridge/effect_adapter.rs`
**Lines**: ~47-92

**Current**:
```rust
pub struct EffectBridgeAdapter {
    tools: Arc<ToolRegistry>,
    safety: Arc<SafetyLayer>,
    hooks: Arc<HookRegistry>,
    // ... other fields
}
```

**New**:
```rust
pub struct EffectBridgeAdapter {
    // Remove: tools: Arc<ToolRegistry>,
    capability_host: Arc<CapabilityHost<'static, BuiltinCapabilityDispatcher>>,
    extension_registry: Arc<SharedExtensionRegistry>,
    permission_resolver: Arc<PermissionResolver>,
    safety: Arc<SafetyLayer>,
    hooks: Arc<HookRegistry>,
    // ... other fields stay the same
}
```

### Step 2: Update Constructor

**Method**: `EffectBridgeAdapter::new()`

**Changes**:
- Remove `tools: Arc<ToolRegistry>` parameter
- Add `capability_host: Arc<CapabilityHost<...>>` parameter
- Add `extension_registry: Arc<SharedExtensionRegistry>` parameter
- Add `permission_resolver: Arc<PermissionResolver>` parameter

### Step 3: Find and Update `execute_action` Method

**Search for**: `fn execute_action` or `async fn execute_action`

**Current logic** (approximate):
```rust
async fn execute_action(&self, ...) -> ActionResult {
    // 1. Look up tool from registry
    let tool = self.tools.get(action_name)?;
    
    // 2. Check permissions
    // Uses v1 PermissionState
    
    // 3. Execute tool
    let result = tool.execute(...)?;
    
    // 4. Apply safety/hooks
    // ...
}
```

**New logic**:
```rust
async fn execute_action(&self, ...) -> ActionResult {
    // 1. Look up capability from registry
    let descriptor = self.extension_registry
        .snapshot()
        .get_capability(&capability_id)?;
    
    // 2. Check permissions via resolver
    let permission = self.permission_resolver
        .resolve_permission(&tenant_id, &capability_id)
        .await?;
    
    if permission == PermissionMode::Deny {
        return Err(EngineError::PermissionDenied);
    }
    
    // 3. Create invocation request
    let request = CapabilityInvocationRequest {
        capability_id,
        context: ExecutionContext { ... },
        input: parameters,
        estimate: ResourceEstimate::default(),
    };
    
    // 4. Invoke through CapabilityHost
    let result = self.capability_host.invoke_json(request).await?;
    
    // 5. Apply safety/hooks (keep existing logic)
    // ...
    
    // 6. Convert CapabilityInvocationResult to ActionResult
    ActionResult {
        output: result.output,
        // ... map fields
    }
}
```

### Step 4: Find and Update `available_actions` Method

**Search for**: `fn available_actions` or `async fn available_actions`

**Current logic**:
```rust
fn available_actions(&self, ...) -> ActionInventory {
    // Query self.tools.list_all()
    // Convert Tool definitions to ActionDef
}
```

**New logic**:
```rust
fn available_actions(&self, ...) -> ActionInventory {
    let registry = self.extension_registry.snapshot();
    
    // Iterate over all capabilities
    let actions: Vec<ActionDef> = registry
        .capabilities()
        .map(|descriptor| {
            ActionDef {
                name: descriptor.id.to_string(),
                description: descriptor.description.clone(),
                parameters_schema: descriptor.parameters_schema.clone(),
                // ... map other fields
            }
        })
        .collect();
    
    ActionInventory { actions }
}
```

### Step 5: Remove V1 Imports

**Remove these imports**:
```rust
use crate::tools::ToolRegistry;
use crate::tools::permissions::PermissionState;
use crate::tools::{ApprovalRequirement, Tool};
use crate::bridge::tool_permissions::{ToolPermissionResolution, ToolPermissionSnapshot};
```

**Add these imports**:
```rust
use brassclaw_capabilities::CapabilityHost;
use brassclaw_extensions::SharedExtensionRegistry;
use crate::capabilities::{BuiltinCapabilityDispatcher, PermissionResolver};
use brassclaw_host_api::{CapabilityInvocationRequest, ExecutionContext, ResourceEstimate, PermissionMode};
```

### Step 6: Update All Call Sites

**Search for**: `self.tools.` throughout the file

**Replace with**: Appropriate `CapabilityHost` or `ExtensionRegistry` calls

Common patterns:
- `self.tools.get(name)` → `self.extension_registry.snapshot().get_capability(id)`
- `self.tools.list_all()` → `self.extension_registry.snapshot().capabilities()`

### Step 7: Delete `tool_permissions.rs`

**File**: `/Volumes/SSDE/brassclaw/src/bridge/tool_permissions.rs`

**Action**: Delete entire file

**Reason**: Permissions now handled by `PermissionResolver`

### Step 8: Update Other Bridge Files

**Files to check**:
- `/Volumes/SSDE/brassclaw/src/bridge/router.rs`
- `/Volumes/SSDE/brassclaw/src/bridge/action_projector.rs`

**Changes**: Remove `ToolRegistry` references, use `ExtensionRegistry` instead

## Testing Strategy

### 1. Compilation Test
```bash
cd /Volumes/SSDE/brassclaw
cargo build
```

### 2. Unit Tests
```bash
cargo test --package brassclaw --lib bridge::effect_adapter
```

### 3. Integration Tests
- Test each of the 47 capabilities
- Verify permission resolution works
- Verify approval workflow works

### 4. Verification Commands
```bash
# Ensure no v1 imports remain
grep -r "use crate::tools::" ./src/bridge/

# Should return zero matches
grep -r "ToolRegistry\|PermissionState" ./src/bridge/
```

## Potential Issues and Solutions

### Issue 1: Lifetime Complications
`CapabilityHost` has a lifetime parameter `'a`. The struct field needs `'static`.

**Solution**: Use `Arc` and ensure all dependencies outlive the adapter.

### Issue 2: Type Mismatches
`ActionResult` vs `CapabilityInvocationResult` have different structures.

**Solution**: Create conversion functions:
```rust
fn to_action_result(result: CapabilityInvocationResult) -> ActionResult {
    ActionResult {
        output: result.output,
        // ... map fields
    }
}
```

### Issue 3: Missing Context
`ExecutionContext` requires fields that may not be available.

**Solution**: Extract from existing `ThreadExecutionContext` or create defaults.

## Estimated Effort

- **Reading/Analysis**: 2-3 hours
- **Implementation**: 3-4 hours
- **Testing/Debugging**: 2-3 hours
- **Total**: 7-10 hours

## Success Criteria

- [ ] `cargo build` succeeds
- [ ] No imports from `./src/tools/` in `./src/bridge/`
- [ ] All 47 capabilities execute correctly
- [ ] Permission resolution works (Allow/Ask/Deny)
- [ ] Approval workflow functions
- [ ] All existing tests pass
- [ ] `tool_permissions.rs` deleted

## Next Steps After Completion

1. Remove v1 agent integration (Step 8)
2. Delete `./src/tools/` directory (Step 9)
3. Add comprehensive tests
4. Update documentation

## Notes

- Keep all security controls (safety, hooks, rate limiting) intact
- The 8,629 lines include much more than just dispatch logic
- Most of the file can stay unchanged
- Focus on the dispatch mechanism only (~200-300 lines of actual changes)