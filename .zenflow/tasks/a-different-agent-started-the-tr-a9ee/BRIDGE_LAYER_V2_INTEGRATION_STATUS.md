# Bridge Layer V2 Integration - Current Status & Recommendation

## Discovery Summary

After thorough analysis, I've discovered that the V2 bridge layer integration is **significantly more complex** than initially understood. The issue is not just with the routine engine - it's a **system-wide architectural change**.

### Root Cause

The system currently uses V1 `ToolRegistry` throughout:
- `AppComponents.tools: Arc<ToolRegistry>` (line 46 of app.rs)
- `Agent::new()` takes `tools: Arc<ToolRegistry>` parameter
- `Scheduler` stores and uses `ToolRegistry`
- `RoutineEngine` stores and uses `ToolRegistry`
- All tool execution goes through V1 `ToolDispatcher`

The V2 `EffectBridgeAdapter` (in `effect_adapter_v2.rs`) exists but is **completely unused**. The system still uses the V1 adapter from `effect_adapter.rs`.

### What Was Actually Completed in Steps 7-8

✅ **Completed**:
- 47 V2 capability execute functions across 13 domains
- `BuiltinCapabilityDispatcher` implementation
- `PermissionResolver` + `CapabilityPermissionStore`
- Database integration (LibSQL + PostgreSQL)
- V2 `EffectBridgeAdapter` code (but not wired up)

❌ **NOT Completed** (despite status saying "Complete"):
- `CapabilityHost` instantiation and wiring
- Switching from V1 to V2 adapter
- Removing `ToolRegistry` from Agent/Scheduler/RoutineEngine
- Bridge layer integration

### Scope of Required Changes

This is not a simple "clean up routine engine" task. It requires:

1. **AppBuilder Changes** (`app.rs`):
   - Remove `tools: Arc<ToolRegistry>` from `AppComponents`
   - Add `effect_executor: Arc<dyn EffectExecutor>`
   - Create `CapabilityHost` during initialization
   - Create V2 `EffectBridgeAdapter` wrapping `CapabilityHost`
   - Register all 47 built-in capabilities

2. **Agent Changes** (`agent_loop.rs`):
   - Remove `tools: Arc<ToolRegistry>` parameter from `Agent::new()`
   - Add `effect_executor: Arc<dyn EffectExecutor>` parameter
   - Update all internal tool references

3. **Scheduler Changes** (`scheduler.rs`):
   - Replace `tools: Arc<ToolRegistry>` with `executor: Arc<dyn EffectExecutor>`
   - Rewrite all tool execution to use V2 interface
   - Update tool listing logic

4. **RoutineEngine Changes** (`routine_engine.rs`):
   - Replace `tools: Arc<ToolRegistry>` with `executor: Arc<dyn EffectExecutor>`
   - Create `ThreadExecutionContext` from `JobContext`
   - Rewrite tool execution logic

5. **Bridge Router Changes** (`bridge/router.rs`):
   - Switch from V1 to V2 `EffectBridgeAdapter`
   - Remove all `ToolRegistry` references

6. **Autonomous Tool Filtering** (`tools/autonomy.rs`):
   - Create V2 equivalent using `EffectExecutor` interface

7. **All Test Files**:
   - Update 15+ test files that call `Agent::new()`
   - Replace mock `ToolRegistry` with mock `EffectExecutor`

### Estimated Effort

- **Full V2 Integration**: 12-16 hours of focused development
- **Testing & Debugging**: 4-6 hours
- **Total**: 16-22 hours

This is essentially completing the unfinished work from Steps 7-8.

## Recommendation

Given the scope, I recommend one of these approaches:

### Option A: Complete Full V2 Integration Now (Recommended)
**Pros**:
- Properly completes Steps 7-8
- Enables all subsequent Step 9 work
- Clean architecture with no hybrid state

**Cons**:
- Large, complex change
- Requires 16-22 hours
- High risk of breaking changes

**Steps**:
1. Create `CapabilityHost` in `AppBuilder`
2. Replace `ToolRegistry` with `EffectExecutor` throughout
3. Update all 15+ call sites
4. Fix all tests
5. Then continue with Step 9

### Option B: Minimal Routine Engine Fix (Temporary)
**Pros**:
- Smaller, focused change
- Can complete Step 9.4 quickly
- Defer full integration

**Cons**:
- Leaves system in hybrid V1/V2 state
- Technical debt
- Will need full integration eventually anyway

**Steps**:
1. Keep `ToolRegistry` in routine engine for now
2. Add TODO comments
3. Continue with other Step 9 tasks
4. Schedule full V2 integration as separate work

### Option C: Pause and Reassess
**Pros**:
- Allows user to decide priority
- Can plan resources properly

**Cons**:
- Blocks Step 9 progress
- Delays overall migration

## My Recommendation: Option A

I recommend **Option A** - complete the full V2 integration now. Here's why:

1. **It's the right thing to do**: Steps 7-8 were marked "complete" but the critical integration work wasn't done
2. **Enables everything else**: Step 9 cannot be properly completed without this
3. **Cleaner result**: No hybrid state, no technical debt
4. **Better testing**: Can verify V2 system works end-to-end

The work is substantial but necessary. Trying to work around it (Option B) just delays the inevitable and makes the codebase harder to maintain.

## Next Steps if Proceeding with Option A

1. Start with `AppBuilder` - create `CapabilityHost` and V2 adapter
2. Update `Agent::new()` signature
3. Update `Scheduler` to use `EffectExecutor`
4. Update `RoutineEngine` to use `EffectExecutor`
5. Update `bridge/router.rs`
6. Fix all test files
7. Verify compilation
8. Run tests
9. Then continue with Step 9

## Files That Need Changes (Option A)

### Core Architecture (Critical Path):
1. `/Volumes/SSDE/brassclaw/src/app.rs` - AppBuilder & AppComponents
2. `/Volumes/SSDE/brassclaw/src/agent/agent_loop.rs` - Agent::new()
3. `/Volumes/SSDE/brassclaw/src/agent/scheduler.rs` - Scheduler
4. `/Volumes/SSDE/brassclaw/src/agent/routine_engine.rs` - RoutineEngine
5. `/Volumes/SSDE/brassclaw/src/bridge/router.rs` - Bridge router
6. `/Volumes/SSDE/brassclaw/src/main.rs` - Main initialization

### Supporting Files:
7. `/Volumes/SSDE/brassclaw/src/tools/autonomy.rs` - Autonomous filtering
8. `/Volumes/SSDE/brassclaw/src/agent/dispatcher.rs` - Tool dispatch
9. `/Volumes/SSDE/brassclaw/src/agent/commands.rs` - Command handlers

### Test Files (15+ files):
10. `/Volumes/SSDE/brassclaw/src/agent/thread_ops.rs` - Tests
11. `/Volumes/SSDE/brassclaw/src/agent/dispatcher.rs` - Tests
12. `/Volumes/SSDE/brassclaw/src/agent/commands.rs` - Tests
13. `/Volumes/SSDE/brassclaw/src/agent/agent_loop.rs` - Tests
14. `/Volumes/SSDE/brassclaw/src/bridge/router.rs` - Tests
15. ... (10+ more test files)

## Conclusion

The routine engine issue revealed that Steps 7-8 were not actually complete. The V2 infrastructure exists but isn't wired up. We need to complete the bridge layer integration before Step 9 can proceed properly.

**Awaiting user decision on which option to pursue.**