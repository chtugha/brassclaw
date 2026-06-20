# Step 9 Execution Plan: Remove V1 Agent Integration

## Overview
Remove all v1 tool system references from agent layer (59 references across 7+ files).

## Critical Discovery
`AgentDeps.tools: Arc<ToolRegistry>` is a **core dependency** used throughout the agent system. Removing it requires careful coordination across multiple modules.

## Execution Strategy

### Phase 1: Remove ToolRegistry from AgentDeps (CRITICAL)
**File**: `src/agent/agent_loop.rs`
- Remove `tools: Arc<ToolRegistry>` field from `AgentDeps` struct (line 495)
- Remove `tools()` accessor method (line 742)
- Update all `AgentDeps` construction sites

**Impact**: This will break compilation in ~50+ places. We'll fix them systematically.

### Phase 2: Fix Test Harness (FOUNDATION)
**File**: `src/testing/mod.rs`
- Remove `tools: Option<Arc<ToolRegistry>>` from `TestHarnessBuilder` (line 285)
- Remove `with_tools()` method (line 313)
- Remove `tools` field initialization in `build()` (lines 345-349, 380)
- Remove `use crate::tools::ToolRegistry;` import (line 43)

### Phase 3: Fix Agent Modules (SYSTEMATIC)

#### 3.1 thread_ops.rs (8 references)
- Remove `ToolRegistry` imports
- Remove `tools` parameter from `make_thread_ops_test_agent_with()` 
- Update test `AgentDeps` construction (remove `tools` field)
- Remove `complete_with_tools` test helper references

#### 3.2 self_repair.rs (4 references)
- Remove `tools: Option<Arc<ToolRegistry>>` from `SelfRepairContext`
- Remove `with_builder()` method's `tools` parameter
- Update test `AgentDeps` construction

#### 3.3 routine_engine.rs (4 references)
- Remove `tools: Arc<ToolRegistry>` from `EngineContext`
- Remove `tools` parameter from `new()` constructor
- Remove `execute_lightweight_with_tools()` function (replaced by v2 capabilities)
- Update autonomous tool name resolution

#### 3.4 scheduler.rs (8 references)
- Remove `tools: Arc<ToolRegistry>` from `SchedulerDeps`
- Remove `tools()` accessor method
- Remove `autonomous_allowed_tool_names()` calls (v2 uses capability permissions)
- Update test `AgentDeps` construction

#### 3.5 dispatcher.rs (30+ references - MOST COMPLEX)
- Remove `execute_chat_tool_standalone()` function (v1-only)
- Remove all `complete_with_tools()` test helper references
- Update test `AgentDeps` construction (10+ test functions)
- Remove `ToolRegistry` imports

#### 3.6 commands.rs (2 references)
- Remove `ToolRegistry` imports from tests
- Update test `AgentDeps` construction

### Phase 4: Verification
1. `cargo build` - must succeed
2. `cargo test` - all tests pass
3. `grep -r "ToolRegistry" src/agent/` - zero matches
4. `grep -r "with_tools\|register_builtin_tools" src/` - zero matches

## Risk Mitigation

### High-Risk Areas
1. **AgentDeps removal**: Core struct change affects 50+ call sites
2. **dispatcher.rs**: 30+ references, complex test infrastructure
3. **routine_engine.rs**: Lightweight tool execution needs v2 replacement

### Safety Measures
1. Make changes in dependency order (test harness → consumers)
2. Verify compilation after each major change
3. Keep v2 capability system intact (already working)
4. Document any temporary compilation errors

## Dependencies

### What We're Removing
- `ToolRegistry` - v1 tool registration system
- `ToolDispatcher` - v1 tool execution pipeline
- `complete_with_tools()` - LLM test helper for v1 tools
- `register_builtin_tools()` - v1 tool initialization
- `autonomous_allowed_tool_names()` - v1 autonomy filtering

### What We're Keeping
- `CapabilityHost` - v2 capability registry (already implemented)
- `EffectExecutor` - v2 execution interface (already implemented)
- `CapabilityLease` - v2 authorization (already implemented)
- All v2 capability modules in `src/capabilities/`

## Success Criteria
- [ ] Zero `ToolRegistry` references in `src/agent/`
- [ ] Zero `ToolDispatcher` references in `src/agent/`
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes
- [ ] No compilation errors
- [ ] No clippy warnings

## Next Steps After Step 9
Once Step 9 is complete:
1. Step 10: Delete `./src/tools/` directory entirely
2. Step 11: Delete `./src/channels/web/` directory
3. Steps 12-14: Build WebUI v2 tools management interface