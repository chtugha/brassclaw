# Bridge Layer V2 Integration Plan

## Current State Analysis

The V2 `EffectBridgeAdapter` exists in `effect_adapter_v2.rs` but is NOT being used. The system still uses the V1 adapter from `effect_adapter.rs` which wraps `ToolRegistry`.

### Key Components Status:
- ✅ V2 Capability modules (47 execute functions) - COMPLETE
- ✅ `BuiltinCapabilityDispatcher` - COMPLETE  
- ✅ `PermissionResolver` + `CapabilityPermissionStore` - COMPLETE
- ✅ Database integration (LibSQL + PostgreSQL) - COMPLETE
- ❌ `CapabilityHost` wiring - NOT DONE
- ❌ V2 `EffectBridgeAdapter` usage - NOT DONE
- ❌ Agent initialization with V2 components - NOT DONE

## Integration Strategy

### Phase 1: Create CapabilityHost Instance

**Location**: Where Agent is initialized (likely `main.rs` or `app.rs`)

**Steps**:
1. Create `BuiltinCapabilityDispatcher` with all context objects
2. Register built-in capabilities with `ExtensionRegistry`
3. Create `CapabilityHost` with dispatcher and extension registry
4. Create V2 `EffectBridgeAdapter` wrapping the `CapabilityHost`

**Code Pattern**:
```rust
// 1. Create dispatcher
let dispatcher = Arc::new(BuiltinCapabilityDispatcher::new(
    // ... all 13 domain contexts
));

// 2. Register built-in capabilities
let descriptors = brassclaw::capabilities::register_all();
for desc in &descriptors {
    extension_registry.register_capability(desc.clone())?;
}

// 3. Create CapabilityHost
let capability_host = Arc::new(CapabilityHost::new(
    extension_registry.clone(),
    dispatcher,
    safety.clone(),
));

// 4. Create V2 EffectBridgeAdapter
let effect_executor = Arc::new(EffectBridgeAdapter::new(
    capability_host,
    extension_registry.clone(),
    safety.clone(),
));
```

### Phase 2: Update Agent Initialization

**File**: `/Volumes/SSDE/brassclaw/src/agent/agent_loop.rs`

**Changes**:
1. Remove `tools: Arc<ToolRegistry>` parameter from `Agent::new()`
2. Add `effect_executor: Arc<dyn EffectExecutor>` parameter
3. Update `Scheduler::new()` to accept `EffectExecutor` instead of `ToolRegistry`
4. Store `effect_executor` in `AgentDeps` or directly in `Agent`

### Phase 3: Update Scheduler

**File**: `/Volumes/SSDE/brassclaw/src/agent/scheduler.rs` (or wherever Scheduler is defined)

**Changes**:
1. Replace `tools: Arc<ToolRegistry>` field with `executor: Arc<dyn EffectExecutor>`
2. Update all tool execution paths to use `executor.execute_action()`
3. Update tool listing to use `executor.available_actions()`

### Phase 4: Update RoutineEngine

**File**: `/Volumes/SSDE/brassclaw/src/agent/routine_engine.rs`

**Changes**:
1. Replace `tools: Arc<ToolRegistry>` with `executor: Arc<dyn EffectExecutor>`
2. Update constructor signature
3. Update all tool execution to use V2 interface
4. Create `ThreadExecutionContext` from `JobContext` where needed

### Phase 5: Update Bridge Router

**File**: `/Volumes/SSDE/brassclaw/src/bridge/router.rs`

**Changes**:
1. Switch from V1 `EffectBridgeAdapter` to V2 version
2. Update effect_adapter creation to use `CapabilityHost`
3. Remove `ToolRegistry` references

## Critical Challenges

### Challenge 1: JobContext vs ThreadExecutionContext

**Problem**: Routine engine uses V1 `JobContext`, but V2 `EffectExecutor` requires `ThreadExecutionContext`.

**Solution Options**:
A. Create a conversion function `JobContext -> ThreadExecutionContext`
B. Refactor routine engine to use V2 thread/step model
C. Create a compatibility adapter that bridges the two contexts

**Recommended**: Option A (conversion function) for now, with Option B as future work.

### Challenge 2: Autonomous Tool Filtering

**Problem**: `autonomous_allowed_tool_names()` function is tightly coupled to `ToolRegistry`.

**Solution**: 
- Create V2 equivalent using `EffectExecutor::available_actions()`
- Filter based on capability metadata instead of tool registry

### Challenge 3: Tool Permission Migration

**Problem**: Existing tool permissions stored in V1 format need to work with V2.

**Solution**:
- Migration script to convert V1 permissions to V2 format
- Or: Keep V1 permissions temporarily and add compatibility layer
- Or: Accept that permissions will be reset (document this)

## Implementation Order

1. **First**: Create CapabilityHost and V2 adapter instance in main.rs/app.rs
2. **Second**: Update Agent::new() signature to accept EffectExecutor
3. **Third**: Update Scheduler to use EffectExecutor
4. **Fourth**: Update RoutineEngine to use EffectExecutor  
5. **Fifth**: Update bridge/router.rs to use V2 adapter
6. **Sixth**: Remove V1 adapter (effect_adapter.rs)
7. **Seventh**: Verify compilation and fix any remaining issues

## Files to Modify

### Critical Path:
1. `/Volumes/SSDE/brassclaw/src/main.rs` or `/Volumes/SSDE/brassclaw/src/app.rs` - Create CapabilityHost
2. `/Volumes/SSDE/brassclaw/src/agent/agent_loop.rs` - Update Agent::new()
3. `/Volumes/SSDE/brassclaw/src/agent/scheduler.rs` - Replace ToolRegistry with EffectExecutor
4. `/Volumes/SSDE/brassclaw/src/agent/routine_engine.rs` - Replace ToolRegistry with EffectExecutor
5. `/Volumes/SSDE/brassclaw/src/bridge/router.rs` - Use V2 adapter

### Supporting Files:
6. `/Volumes/SSDE/brassclaw/src/tools/autonomy.rs` - Create V2 equivalent
7. `/Volumes/SSDE/brassclaw/src/agent/dispatcher.rs` - Update tool execution
8. `/Volumes/SSDE/brassclaw/src/agent/commands.rs` - Update tool references

## Success Criteria

- [ ] `CapabilityHost` instance created at startup
- [ ] V2 `EffectBridgeAdapter` used throughout system
- [ ] No references to V1 `EffectBridgeAdapter` (effect_adapter.rs)
- [ ] `Agent::new()` no longer takes `ToolRegistry`
- [ ] Scheduler uses `EffectExecutor` interface
- [ ] RoutineEngine uses `EffectExecutor` interface
- [ ] `cargo build` succeeds
- [ ] No `ToolRegistry` imports in agent/ or bridge/ modules

## Next Steps

Start with Phase 1: Find where Agent is initialized and create the CapabilityHost instance there.