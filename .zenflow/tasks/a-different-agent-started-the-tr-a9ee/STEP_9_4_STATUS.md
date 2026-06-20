# Step 9.4 Status: Routine Engine Migration

## Current State

### Changes Made
1. ✅ Updated imports - removed `ToolRegistry`, `ToolError`, `prepare_tool_params`
2. ✅ Added V2 imports - `EffectExecutor`, `ThreadExecutionContext`
3. ✅ Updated `RoutineEngine` struct - replaced `tools: Arc<ToolRegistry>` with `executor: Arc<dyn EffectExecutor>`
4. ✅ Updated constructor parameter
5. ✅ Updated `EngineContext` struct
6. ✅ Updated 3 locations where `EngineContext` is created
7. ✅ Updated tool definition retrieval to use `executor.list_actions()`
8. ✅ Updated `execute_routine_tool()` to use `executor.execute_action()`

### Compilation Errors Remaining

#### Error 1: `agent_loop.rs` - ToolRegistry cast to EffectExecutor
```
error[E0277]: the trait bound `ToolRegistry: EffectExecutor` is not satisfied
    --> src/agent/agent_loop.rs:1329:25
```
**Issue**: `agent_loop.rs` is trying to pass `self.tools()` (which returns `Arc<ToolRegistry>`) to `RoutineEngine::new()` which now expects `Arc<dyn EffectExecutor>`.

**Solution**: Need to pass the `EffectBridgeAdapter` instead of `ToolRegistry` to `RoutineEngine::new()`.

#### Error 2: `autonomous_allowed_tool_names` needs ToolRegistry
```
error[E0609]: no field `tools` on type `&EngineContext`
    --> src/agent/routine_engine.rs:1891:44
```
**Issue**: `autonomous_allowed_tool_names(&ctx.tools, ...)` expects a `ToolRegistry`, but we removed it.

**Solution**: This function filters which tools can run autonomously. We need to either:
- Option A: Rewrite `autonomous_allowed_tool_names` to work with `EffectExecutor`
- Option B: Get the list from `executor.list_actions()` and filter locally
- Option C: Keep a reference to ToolRegistry just for this filtering (temporary)

#### Error 3: `EffectExecutor` doesn't have `list_actions` method
```
error[E0599]: no method named `list_actions` found for struct `Arc<(dyn EffectExecutor + 'static)>`
```
**Issue**: The `EffectExecutor` trait doesn't have a `list_actions()` method.

**Solution**: Need to check what methods `EffectExecutor` actually has and use the correct one.

#### Error 4: `ThreadExecutionContext` field mismatch
```
error[E0609]: no field `thread_id` on type `&JobContext`
error[E0560]: struct `ThreadExecutionContext` has no field named `workspace_id`
error[E0560]: struct `ThreadExecutionContext` has no field named `tenant_id`
error[E0560]: struct `ThreadExecutionContext` has no field named `session_id`
error[E0560]: struct `ThreadExecutionContext` has no field named `channel`
```
**Issue**: `ThreadExecutionContext` has different fields than expected:
- Has: `thread_id: ThreadId`, `thread_type`, `project_id`, `user_id`, `step_id`, `current_call_id`, `source_channel`, `user_timezone`, `thread_goal`, `available_actions_snapshot`, `available_action_inventory_snapshot`
- Missing: `workspace_id`, `tenant_id`, `session_id`, `channel`

**Solution**: Need to properly map `JobContext` fields to `ThreadExecutionContext` fields.

## Next Steps

### Immediate Fixes Needed

1. **Check `EffectExecutor` trait definition** to see what methods are available
2. **Fix `ThreadExecutionContext` construction** with correct field mapping
3. **Fix `autonomous_allowed_tool_names` call** - either rewrite or find alternative
4. **Update `agent_loop.rs`** to pass `EffectBridgeAdapter` instead of `ToolRegistry`

### Alternative Approach

Given the complexity, we could consider a **hybrid approach**:
- Keep `ToolRegistry` reference in `RoutineEngine` temporarily (alongside `executor`)
- Use `ToolRegistry` for autonomous filtering and tool definitions
- Use `executor` only for actual execution
- This would allow incremental migration

## Recommendation

**PAUSE** and reassess the approach. The routine engine migration is more complex than anticipated because:

1. `EffectExecutor` trait may not have all the methods we need
2. `ThreadExecutionContext` structure is different from `JobContext`
3. Autonomous tool filtering logic is tightly coupled to `ToolRegistry`
4. Multiple call sites need updates

**Better approach**: 
- Complete the simpler Step 9 tasks first (9.7, 9.8, 9.9)
- Come back to routine engine migration with a clearer understanding of the V2 system
- Or keep routine engine using V1 tools temporarily and migrate in a dedicated step

## Files Modified (Uncommitted)
- `src/agent/routine_engine.rs` - Partially migrated, has compilation errors