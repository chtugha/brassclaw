# Bridge V2 Integration - Detailed Implementation Steps

## Overview

Complete the missing bridge layer integration by creating `CapabilityHost` and V2 `EffectBridgeAdapter` instances, then replacing `ToolRegistry` with `EffectExecutor` throughout the system.

## Step 1: Create Context Objects and Dispatcher in AppBuilder

**File**: `/Volumes/SSDE/brassclaw/src/app.rs`

**Actions**:
1. Add imports for all context types and dispatcher
2. In `build_all()` method, after creating all components, create 13 context objects:
   - `FilesystemContext` - needs base_dir (from workspace)
   - `ShellContext` - needs working_dir
   - `NetworkContext` - needs credential_registry
   - `MemoryContext` - needs memory store
   - `MessagingContext` - needs channels
   - `JobsContext` - needs scheduler/context_manager
   - `RoutinesContext` - needs routine store
   - `SkillsContext` - needs skill registry
   - `ExtensionsContext` - needs extension manager
   - `SecretsContext` - needs secrets store
   - `ImagesContext` - needs image generator
   - `SystemContext` - needs system info
   - `PairingContext` - needs pairing store
3. Create `BuiltinCapabilityDispatcher` with all contexts
4. Create `CapabilityHost` with dispatcher, extension_registry, and authorizer
5. Create V2 `EffectBridgeAdapter` wrapping the `CapabilityHost`
6. Store in `AppComponents` as `effect_executor: Arc<dyn EffectExecutor>`

## Step 2: Update AppComponents Struct

**File**: `/Volumes/SSDE/brassclaw/src/app.rs`

**Actions**:
1. Replace `pub tools: Arc<ToolRegistry>` with `pub effect_executor: Arc<dyn EffectExecutor>`
2. Update all references in the file

## Step 3: Update Agent::new() Signature

**File**: `/Volumes/SSDE/brassclaw/src/agent/agent_loop.rs`

**Actions**:
1. Remove `tools: Arc<ToolRegistry>` parameter
2. Add `effect_executor: Arc<dyn EffectExecutor>` parameter
3. Update Scheduler::new() call to pass executor instead of tools
4. Update any other tool references

## Step 4: Update Scheduler

**File**: `/Volumes/SSDE/brassclaw/src/agent/scheduler.rs` (or wherever Scheduler is defined)

**Actions**:
1. Find Scheduler struct definition
2. Replace `tools: Arc<ToolRegistry>` field with `executor: Arc<dyn EffectExecutor>`
3. Update constructor
4. Update all tool execution to use `executor.execute_action()`
5. Update tool listing to use `executor.available_actions()`

## Step 5: Update RoutineEngine

**File**: `/Volumes/SSDE/brassclaw/src/agent/routine_engine.rs`

**Actions**:
1. Replace `tools: Arc<ToolRegistry>` with `executor: Arc<dyn EffectExecutor>`
2. Update constructor signature
3. Update `EngineContext` struct
4. Update all context creation sites
5. Rewrite tool execution logic to use V2 interface
6. Create conversion function `JobContext -> ThreadExecutionContext`

## Step 6: Update Bridge Router

**File**: `/Volumes/SSDE/brassclaw/src/bridge/router.rs`

**Actions**:
1. Find where `EffectBridgeAdapter` is created (around line 1751)
2. Switch from V1 adapter (effect_adapter.rs) to V2 adapter (effect_adapter_v2.rs)
3. Update imports
4. Pass `CapabilityHost` instead of `ToolRegistry`

## Step 7: Update Main Initialization

**File**: `/Volumes/SSDE/brassclaw/src/main.rs`

**Actions**:
1. Update `Agent::new()` call to pass `effect_executor` instead of `tools`
2. Remove `components.tools` reference

## Step 8: Fix Test Files

**Files**: 15+ test files that call `Agent::new()`

**Actions** (for each file):
1. Replace mock `ToolRegistry` with mock `EffectExecutor`
2. Update `Agent::new()` calls
3. Update any tool-related test assertions

**Test files to update**:
- `/Volumes/SSDE/brassclaw/src/agent/thread_ops.rs`
- `/Volumes/SSDE/brassclaw/src/agent/dispatcher.rs`
- `/Volumes/SSDE/brassclaw/src/agent/commands.rs`
- `/Volumes/SSDE/brassclaw/src/agent/agent_loop.rs`
- `/Volumes/SSDE/brassclaw/src/bridge/router.rs`
- ... (10+ more)

## Step 9: Remove V1 Adapter

**File**: `/Volumes/SSDE/brassclaw/src/bridge/effect_adapter.rs`

**Actions**:
1. Delete the entire file (after confirming no references remain)
2. Update `mod.rs` to remove the module

## Step 10: Verification

**Actions**:
1. Run `cargo build` - must succeed with zero errors
2. Run `cargo clippy` - must pass
3. Run `cargo test` - must pass
4. Grep for remaining `ToolRegistry` references:
   ```bash
   grep -r "ToolRegistry" ./src/agent/ ./src/bridge/
   ```
5. Should only find references in:
   - Old tool modules (to be deleted in Step 10)
   - Test fixtures (acceptable temporarily)

## Context Object Creation Details

### FilesystemContext
```rust
let filesystem_ctx = Arc::new(FilesystemContext {
    base_dir: workspace.as_ref().map(|w| w.root_dir().to_path_buf()).unwrap_or_else(|| PathBuf::from(".")),
    undo_snapshots: Arc::new(RwLock::new(VecDeque::new())),
});
```

### ShellContext
```rust
let shell_ctx = Arc::new(ShellContext {
    working_dir: workspace.as_ref().map(|w| w.root_dir().to_path_buf()),
    sandbox_readiness: components.sandbox_readiness,
});
```

### NetworkContext
```rust
let network_ctx = Arc::new(NetworkContext {
    credential_registry: components.wasm_tool_runtime.as_ref().map(|r| r.credential_registry().clone()),
});
```

... (similar for other 10 contexts)

## Estimated Time

- Step 1 (Context creation): 2-3 hours
- Step 2 (AppComponents): 30 minutes
- Step 3 (Agent::new): 1 hour
- Step 4 (Scheduler): 2-3 hours
- Step 5 (RoutineEngine): 2-3 hours
- Step 6 (Bridge router): 1 hour
- Step 7 (Main): 30 minutes
- Step 8 (Tests): 3-4 hours
- Step 9 (Cleanup): 30 minutes
- Step 10 (Verification): 1-2 hours

**Total**: 14-18 hours

## Success Criteria

- [ ] `CapabilityHost` instance created in AppBuilder
- [ ] V2 `EffectBridgeAdapter` created and stored in AppComponents
- [ ] `Agent::new()` takes `EffectExecutor` instead of `ToolRegistry`
- [ ] Scheduler uses `EffectExecutor`
- [ ] RoutineEngine uses `EffectExecutor`
- [ ] Bridge router uses V2 adapter
- [ ] All tests pass
- [ ] `cargo build` succeeds
- [ ] No `ToolRegistry` references in agent/ or bridge/ (except old tool modules)