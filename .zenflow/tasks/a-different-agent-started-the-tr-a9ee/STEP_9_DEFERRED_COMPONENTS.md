# Step 9: Deferred Components

## Overview
During Step 9 execution, we identified two critical production components that require **functional migration** to V2, not just reference removal. These are being deferred to a dedicated migration step.

---

## Deferred Component 1: Routine Engine (`src/agent/routine_engine.rs`)

### Current State
- Uses `ToolRegistry` for lightweight routine tool execution
- Lines 122, 154, 1136: `tools: Arc<ToolRegistry>` fields
- Lines 1931-1932: `ctx.tools.tool_definitions_visible_under()` / `tool_definitions()`
- Lines 2054-2080: `ctx.tools.get()` and tool execution

### Why Deferred
- **Critical Feature**: Routines execute tools autonomously (cron triggers, event triggers)
- **Functional Change Required**: Need to migrate from V1 tool execution to V2 capability invocation
- **Complex Migration**: Requires updating:
  - Tool definition retrieval logic
  - Tool execution flow
  - Autonomous tool filtering
  - Error handling
  - Test coverage

### Migration Strategy (Future Step)
1. Replace `Arc<ToolRegistry>` with `Arc<CapabilityHost>`
2. Update `execute_routine_tool()` to use V2 effect execution
3. Migrate tool definition retrieval to capability metadata
4. Update autonomous tool filtering for V2
5. Comprehensive testing of routine execution

### Current Workaround
- Routine engine continues using V1 tools
- No breaking changes to routine functionality
- Will be migrated in dedicated step after Step 9 complete

---

## Deferred Component 2: Scheduler (`src/agent/scheduler.rs`)

### Current State
- Uses `ToolRegistry` for background tool task execution
- Line 53: `SchedulerDeps` has `tools: Arc<ToolRegistry>`
- Line 65: `Scheduler` struct has `tools: Arc<ToolRegistry>`
- Line 543: `execute_tool_task()` function uses ToolRegistry
- Line 747: Public `tools()` accessor method

### Why Deferred
- **Critical Feature**: Scheduler executes tool tasks in background slots
- **Functional Change Required**: Need to migrate from V1 tool execution to V2 capability invocation
- **Complex Migration**: Requires updating:
  - Tool task execution flow
  - Approval context handling
  - Parameter normalization
  - Error handling
  - Test coverage

### Migration Strategy (Future Step)
1. Replace `Arc<ToolRegistry>` in `SchedulerDeps` and `Scheduler` with `Arc<CapabilityHost>`
2. Update `execute_tool_task()` to use V2 effect execution
3. Migrate approval context to V2 permission system
4. Update parameter preparation for V2
5. Comprehensive testing of scheduled task execution

### Current Workaround
- Scheduler continues using V1 tools
- No breaking changes to background task execution
- Will be migrated in dedicated step after Step 9 complete

---

## Impact Analysis

### What Still Works
✅ Routines can execute (using V1 tools)
✅ Scheduled tasks can execute (using V1 tools)
✅ Agent loop uses V2 capabilities for chat turns
✅ Self-repair stuck job detection works
✅ Production build succeeds

### What's Deferred
⏸️ Routine tool execution migration to V2
⏸️ Scheduler tool execution migration to V2
⏸️ Complete removal of `./src/tools/` directory (Step 10)

### Migration Path
```
Current State (Step 9 Partial):
├── Chat turns → V2 capabilities ✅
├── Routines → V1 tools ⏸️
└── Scheduler → V1 tools ⏸️

Target State (After Routine/Scheduler Migration):
├── Chat turns → V2 capabilities ✅
├── Routines → V2 capabilities ✅
└── Scheduler → V2 capabilities ✅

Final State (Step 10):
└── ./src/tools/ deleted ✅
```

---

## Revised Step 9 Scope

### Completed Sub-Tasks
- ✅ 9.3: Clean self_repair.rs (disabled tool repair)
- ✅ 9.5: Clean agent_loop.rs (removed tools from AgentDeps)

### Remaining Sub-Tasks (Simplified)
- 🔄 9.7: Clean dispatcher.rs (remove test ToolRegistry usage)
- 🔄 9.8: Clean commands.rs (remove test ToolRegistry usage)
- 🔄 9.9: Clean settings modules (remove v1 permission fields)
- 🔄 9.10: Final verification (grep checks, build verification)

### Deferred to Future Step
- ⏸️ 9.4: Routine engine V2 migration
- ⏸️ 9.6: Scheduler V2 migration

---

## Recommendation

**Proceed with simplified Step 9 scope**:
1. Complete remaining test cleanup tasks (9.7, 9.8)
2. Remove v1 permission fields from settings (9.9)
3. Verify production build still works (9.10)
4. Create dedicated step for routine/scheduler V2 migration
5. Then proceed with Step 10 (delete `./src/tools/`)

This approach:
- ✅ Maintains system stability
- ✅ Allows incremental progress
- ✅ Preserves critical functionality
- ✅ Enables proper testing at each stage