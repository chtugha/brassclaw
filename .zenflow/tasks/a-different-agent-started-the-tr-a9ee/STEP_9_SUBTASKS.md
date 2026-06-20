# Step 9: Sub-Task Breakdown

## Overview
Breaking down the removal of v1 agent integration into manageable sub-tasks, ordered from least to most complex.

---

## Sub-Task 9.1: Update Test Fixtures (Low Risk)
**Files**: `src/testing/mod.rs`, test code in `src/agent/commands.rs`
**Complexity**: LOW
**Estimated Time**: 30 minutes

### Actions:
1. Read `src/testing/mod.rs` to understand current v1 test fixtures
2. Create v2 capability test fixtures (mock `EffectExecutor`, test capabilities)
3. Update test code in `src/agent/commands.rs` to use v2 fixtures
4. Verify tests still pass

### Success Criteria:
- `cargo test --test commands` passes
- No `ToolRegistry` references in test code

---

## Sub-Task 9.2: Update Thread Operations (Low-Medium Risk)
**Files**: `src/agent/thread_ops.rs`
**Complexity**: LOW-MEDIUM
**Estimated Time**: 45 minutes

### Actions:
1. Replace `PermissionState::AlwaysAllow` with `PermissionMode::Allow` (line 1830)
2. Update test fixtures to use v2 capabilities
3. Remove `ToolRegistry` imports and usage in tests
4. Verify thread operations still work

### Success Criteria:
- `cargo test --test thread_ops` passes
- No `ToolRegistry` or `PermissionState` references

---

## Sub-Task 9.3: Update Self-Repair Module (Medium Risk)
**Files**: `src/agent/self_repair.rs`
**Complexity**: MEDIUM
**Estimated Time**: 1 hour

### Actions:
1. Replace `ToolRegistry` with `EffectExecutor` in struct fields
2. Update `with_builder()` method signature
3. Update tool repair logic to use v2 capabilities
4. Update test fixtures
5. Verify self-repair functionality

### Success Criteria:
- `cargo test --test self_repair` passes
- No `ToolRegistry` references
- Self-repair can still detect and fix broken tools

---

## Sub-Task 9.4: Update Routine Engine (Medium Risk)
**Files**: `src/agent/routine_engine.rs`
**Complexity**: MEDIUM
**Estimated Time**: 1 hour

### Actions:
1. Replace `ToolRegistry` with `EffectExecutor` in struct fields
2. Update routine execution to use v2 capability invocation
3. Update autonomous tool execution logic
4. Update tests
5. Verify routine execution works

### Success Criteria:
- `cargo test --test routine_engine` passes
- No `ToolRegistry` references
- Routines can still execute tools autonomously

---

## Sub-Task 9.5: Update Agent Loop Core (High Risk)
**Files**: `src/agent/agent_loop.rs`
**Complexity**: HIGH
**Estimated Time**: 1.5 hours

### Actions:
1. Replace `ToolRegistry` in `AgentDeps` struct with `EffectExecutor`
2. Update `tools()` accessor method
3. Update all call sites throughout the agent loop
4. Update test fixtures
5. Verify agent loop functionality

### Success Criteria:
- `cargo test --test agent_loop` passes
- No `ToolRegistry` references
- Agent loop can still execute actions via v2 system

---

## Sub-Task 9.6: Update Scheduler (High Risk)
**Files**: `src/agent/scheduler.rs`
**Complexity**: HIGH
**Estimated Time**: 1.5 hours

### Actions:
1. Replace `ToolRegistry` in `SchedulerDeps` struct
2. Update `execute_tool_task()` to use `EffectExecutor`
3. Update autonomous tool execution logic
4. Update test fixtures
5. Verify scheduled task execution

### Success Criteria:
- `cargo test --test scheduler` passes
- No `ToolRegistry` references
- Scheduler can still execute autonomous tasks

---

## Sub-Task 9.7: Update Dispatcher (Very High Risk)
**Files**: `src/agent/dispatcher.rs`
**Complexity**: VERY HIGH
**Estimated Time**: 2-3 hours

### Actions:
1. Remove `AdminToolPolicyCache` from struct
2. Replace `PermissionState` with `PermissionMode` throughout
3. Update `effective_permission()` calls to use v2 permission resolution
4. Update tool definition filtering logic
5. Update `execute_chat_tool_standalone()` function
6. Update all test cases
7. Verify dispatcher functionality

### Success Criteria:
- `cargo test --test dispatcher` passes
- No `ToolRegistry`, `PermissionState`, or `AdminToolPolicy` references
- Tool execution and permission checks work via v2 system

---

## Sub-Task 9.8: Update Settings and App Layer (Medium Risk)
**Files**: `src/settings.rs`, `src/app.rs`, `src/tenant.rs`, `src/workspace/settings_schemas.rs`
**Complexity**: MEDIUM
**Estimated Time**: 1 hour

### Actions:
1. Remove `tool_permissions: HashMap<String, PermissionState>` from settings
2. Remove `cleanup_ghost_seeded_tool_permissions` from app.rs
3. Remove `AdminToolPolicy` / `AdminToolPolicyCache` from tenant.rs
4. Remove v1 permission schemas from settings_schemas.rs
5. Update any migration code
6. Verify settings load/save works

### Success Criteria:
- `cargo build` succeeds
- No `PermissionState` or `AdminToolPolicy` references
- Settings system works without v1 permission fields

---

## Sub-Task 9.9: Final Verification (Low Risk)
**Complexity**: LOW
**Estimated Time**: 30 minutes

### Actions:
1. Run full grep to verify all v1 references removed:
   ```bash
   grep -r "ToolRegistry\|ToolDispatcher\|PermissionState\|AdminToolPolicy" ./src/agent/ ./src/settings.rs ./src/app.rs ./src/tenant.rs
   ```
2. Run `cargo build --all-targets`
3. Run `cargo test`
4. Run `cargo clippy`
5. Manual smoke test of key workflows

### Success Criteria:
- Grep returns zero matches
- All builds and tests pass
- No clippy warnings related to changes

---

## Execution Order

1. **9.1** - Test Fixtures (safe, establishes foundation)
2. **9.2** - Thread Operations (low risk, small scope)
3. **9.3** - Self-Repair (medium risk, isolated module)
4. **9.4** - Routine Engine (medium risk, isolated module)
5. **9.5** - Agent Loop (high risk, core functionality)
6. **9.6** - Scheduler (high risk, core functionality)
7. **9.7** - Dispatcher (very high risk, most complex)
8. **9.8** - Settings/App Layer (medium risk, cleanup)
9. **9.9** - Final Verification (validation)

## Risk Mitigation Strategy

- **After each sub-task**: Run `cargo build` and relevant tests
- **Before moving to next sub-task**: Commit changes with descriptive message
- **If tests fail**: Revert and analyze before proceeding
- **Keep changes minimal**: Only change what's necessary for that sub-task

## Total Estimated Time
**8-11 hours** of focused work, spread across multiple sessions

## Current Status
- [ ] 9.1 - Test Fixtures
- [ ] 9.2 - Thread Operations  
- [ ] 9.3 - Self-Repair
- [ ] 9.4 - Routine Engine
- [ ] 9.5 - Agent Loop
- [ ] 9.6 - Scheduler
- [ ] 9.7 - Dispatcher
- [ ] 9.8 - Settings/App Layer
- [ ] 9.9 - Final Verification

## Next Action
Start with **Sub-Task 9.1: Update Test Fixtures**