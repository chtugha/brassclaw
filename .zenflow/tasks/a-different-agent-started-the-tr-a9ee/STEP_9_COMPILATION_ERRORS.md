# Step 9 Compilation Errors Analysis

## Current Status
- ✅ Removed `tools: Arc<ToolRegistry>` from `AgentDeps`
- ✅ Removed `tools()` accessor method from `Agent`
- ✅ Fixed test harness (`testing/mod.rs`)
- ✅ Fixed `commands.rs` tests
- ✅ Fixed `agent_loop.rs` tests  
- ✅ Fixed `thread_ops.rs` tests

## Remaining Compilation Errors

### 1. agent_loop.rs (4 errors)
- Line 1054: `self.tools()` → needs replacement for self-repair
- Line 1319: `self.tools()` → needs replacement for routine engine
- Line 1333: `.tools` field access → needs replacement
- Line 1707: `self.tools()` → needs replacement for tool definitions

### 2. commands.rs (3 errors)
- Line 810: `self.tools().list()` → needs replacement for `/tools` command

### 3. dispatcher.rs (multiple errors)
- Line 306: `self.tools().tool_definitions_visible_under()` → needs v2 capability listing
- Line 307+: More `self.tools()` calls

## Root Cause Analysis

The `Agent` struct no longer has a `tools` field, but several subsystems still need access to tool/capability information:

1. **Self-Repair** (line 1054): Needs tool registry for rebuilding tools
2. **Routine Engine** (line 1319): Needs tool registry for lightweight execution
3. **Scheduler** (line 1333): Needs tool registry for autonomous execution
4. **Dispatcher** (line 306): Needs tool definitions for LLM context
5. **Commands** (line 810): Needs tool list for `/tools` command

## Solution Strategy

### Option A: Keep ToolRegistry in Scheduler (TEMPORARY)
The scheduler still has `tools: Arc<ToolRegistry>` field. We could:
1. Access tools through `self.scheduler.tools()` (as compiler suggests)
2. This is a temporary bridge until we fully migrate these subsystems

### Option B: Migrate Each Subsystem to V2 (PROPER)
1. **Self-Repair**: Remove `tools` parameter, use v2 capabilities
2. **Routine Engine**: Remove `tools` parameter, use v2 capabilities  
3. **Scheduler**: Remove `tools` field, use v2 capabilities
4. **Dispatcher**: Use `CapabilityHost` instead of `ToolRegistry`
5. **Commands**: Use `CapabilityHost.list_registered()` for `/tools`

## Decision

We'll use **Option A** as an intermediate step:
1. Fix compilation errors by using `self.scheduler.tools()`
2. This allows us to proceed with removing other v1 references
3. In a follow-up, we'll remove `tools` from scheduler and complete the migration

## Next Steps

1. Fix agent_loop.rs calls to use `self.scheduler.tools()`
2. Fix commands.rs to use `self.scheduler.tools()`
3. Fix dispatcher.rs to use `self.scheduler.tools()`
4. Continue with self_repair.rs, routine_engine.rs, scheduler.rs cleanup
5. Final step: Remove `tools` from scheduler and migrate to v2