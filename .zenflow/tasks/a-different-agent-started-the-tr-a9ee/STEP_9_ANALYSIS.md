# Step 9 Analysis: Remove v1 Agent Integration

## Overview
Remove all v1 tool system references (`ToolRegistry`, `ToolDispatcher`, `PermissionState`, `AdminToolPolicy`) from the agent and settings layers.

## Search Results Summary
Found 59 matches across the following files:

### Agent Layer Files
1. **src/agent/commands.rs** - 3 matches
2. **src/agent/scheduler.rs** - 11 matches  
3. **src/agent/dispatcher.rs** - 24 matches
4. **src/agent/agent_loop.rs** - 7 matches
5. **src/agent/self_repair.rs** - 6 matches
6. **src/agent/routine_engine.rs** - 4 matches
7. **src/agent/thread_ops.rs** - 4 matches

### Key Dependencies to Remove
- `ToolRegistry` - Central registry for v1 tools
- `ToolDispatcher` - Dispatches tool calls
- `PermissionState` - Enum for tool permissions (Disabled, AlwaysAllow, AskEachTime)
- `AdminToolPolicy` / `AdminToolPolicyCache` - Admin-level tool policies

## Impact Analysis

### Critical Files Requiring Major Changes

#### 1. src/agent/dispatcher.rs (24 matches)
**Current State**: Heavy use of v1 permission system
- Uses `PermissionState` for filtering tool definitions
- Uses `AdminToolPolicyCache` for caching
- Uses `effective_permission()` for permission checks
- Test code uses `ToolRegistry`

**Required Changes**:
- Replace `PermissionState` checks with v2 `PermissionMode` from `CapabilityHost`
- Remove `AdminToolPolicyCache` field
- Update permission filtering logic to use v2 system
- Update tests to use v2 capability fixtures

#### 2. src/agent/scheduler.rs (11 matches)
**Current State**: Uses `ToolRegistry` for autonomous tool execution
- `SchedulerDeps` struct contains `Arc<ToolRegistry>`
- `execute_tool_task()` uses `ToolRegistry`
- Tests create `ToolRegistry` instances

**Required Changes**:
- Replace `ToolRegistry` with `EffectExecutor` or `CapabilityHost`
- Update `SchedulerDeps` struct
- Refactor autonomous tool execution to use v2 capabilities
- Update test fixtures

#### 3. src/agent/agent_loop.rs (7 matches)
**Current State**: Core agent loop uses `ToolRegistry`
- `AgentDeps` struct contains `Arc<ToolRegistry>`
- `tools()` accessor method
- Tests use `ToolRegistry`

**Required Changes**:
- Replace `ToolRegistry` with `EffectExecutor` in `AgentDeps`
- Update accessor methods
- Update all call sites
- Update test fixtures

### Moderate Impact Files

#### 4. src/agent/routine_engine.rs (4 matches)
**Current State**: Routines use `ToolRegistry` for lightweight execution
**Required Changes**: Replace with `EffectExecutor` or capability-based execution

#### 5. src/agent/self_repair.rs (6 matches)
**Current State**: Self-repair uses `ToolRegistry` for tool repair
**Required Changes**: Update to use v2 capability system

#### 6. src/agent/thread_ops.rs (4 matches)
**Current State**: Thread operations reference `PermissionState::AlwaysAllow`
**Required Changes**: Replace with v2 `PermissionMode::Allow`

#### 7. src/agent/commands.rs (3 matches)
**Current State**: Test code uses `ToolRegistry`
**Required Changes**: Update test fixtures only

### Additional Files to Check

According to the plan, also need to check:
- `./src/settings.rs` - Remove `tool_permissions` field
- `./src/app.rs` - Remove `cleanup_ghost_seeded_tool_permissions`
- `./src/tenant.rs` - Remove `AdminToolPolicy` / `AdminToolPolicyCache`
- `./src/workspace/settings_schemas.rs` - Remove v1 permission schemas
- `./src/testing/mod.rs` - Replace v1 test fixtures

## Implementation Strategy

### Phase 1: Understand Current Architecture
1. Read key files to understand how v1 system works
2. Identify all integration points
3. Map v1 concepts to v2 equivalents

### Phase 2: Create v2 Equivalents
1. Ensure `EffectExecutor` can replace `ToolRegistry` functionality
2. Create v2 test fixtures in `testing/mod.rs`
3. Add any missing v2 APIs needed for migration

### Phase 3: Incremental Migration
1. Start with test files (commands.rs tests)
2. Move to peripheral files (self_repair, routine_engine)
3. Update core files (agent_loop, dispatcher, scheduler)
4. Update settings and app layers
5. Remove v1 imports and types

### Phase 4: Verification
1. Run `cargo build` after each major change
2. Run `cargo test` to catch regressions
3. Verify grep shows zero matches for v1 types
4. Manual testing of key workflows

## Risks and Mitigation

### Risk 1: Breaking Core Agent Functionality
**Mitigation**: Make changes incrementally, test after each change

### Risk 2: Missing v2 APIs
**Mitigation**: Identify gaps early, implement missing APIs before migration

### Risk 3: Test Coverage Gaps
**Mitigation**: Ensure v2 test fixtures are comprehensive before removing v1

### Risk 4: Permission Semantics Changes
**Mitigation**: Document v1→v2 permission mapping clearly

## V1 to V2 Mapping

### Permission States
- `PermissionState::Disabled` → `PermissionMode::Deny`
- `PermissionState::AlwaysAllow` → `PermissionMode::Allow`
- `PermissionState::AskEachTime` → `PermissionMode::Ask`

### Core Types
- `ToolRegistry` → `EffectExecutor` (via `EffectBridgeAdapter`)
- `ToolDispatcher` → `CapabilityHost::invoke()`
- `AdminToolPolicy` → Removed (handled by `CapabilityHost`)
- `tool_permissions: HashMap<String, PermissionState>` → Removed (in database)

### Execution Flow
**V1**: `ToolRegistry` → `Tool::execute()` → Result
**V2**: `EffectExecutor` → `CapabilityHost` → `CapabilityDispatcher` → Result

## Next Steps

1. Read the current implementation of key files
2. Verify `EffectExecutor` interface is sufficient
3. Create v2 test fixtures
4. Begin incremental migration starting with tests
5. Update core agent files
6. Remove v1 types and imports
7. Verify with grep and cargo build/test

## Estimated Complexity
**High** - This is a major refactoring touching core agent functionality across 7+ files with 59 references to remove.

## Recommendation
Given the scope, this should be done carefully with frequent testing. Consider breaking into sub-tasks if needed.