# Step 10: V1 Architecture Removal - Comprehensive Analysis

## Current State Assessment

### Phase 1: Cross-Cutting Dependencies (MOSTLY COMPLETE)
✅ **Already Extracted:**
- `init_tracing` - Already in `./src/logging.rs` (line 128)
- `wasm_runtime/` - Already exists as separate module
- `mcp_client/` - Already exists as separate module
- `ToolDecisionDto` - Already re-exported from `brassclaw_common`
- `OnboardingStateDto` - Already re-exported from `brassclaw_common`
- `AppEvent` - Already re-exported from `brassclaw_common`

⚠️ **Still Need to Extract:**
- `DEFAULT_BROADCAST_BUFFER` (used in 4 files outside web/)
- `DEFAULT_MAX_CONNECTIONS` (used in 2 files outside web/)
- `GATEWAY_CHANNEL_NAME` (used in 1 file outside web/)
- Message builders like `build_turns_from_db_messages`

### Phase 2: WASM & MCP (COMPLETE)
✅ Both already extracted to separate modules

### Phase 3: V1 Tool System Removal (MAJOR WORK)

#### Files with ToolRegistry References (300+ occurrences):
1. **Core Agent Files:**
   - `./src/agent/agent_loop.rs` - 8 references
   - `./src/agent/scheduler.rs` - 20+ references (deprecated but still used)
   - `./src/agent/thread_ops.rs` - 2 references
   - `./src/agent/routine.rs` - 1 reference
   - `./src/agent/dispatcher.rs` - 50+ references

2. **Bridge Layer:**
   - `./src/bridge/router.rs` - 100+ references
   - `./src/bridge/effect_adapter.rs` - 200+ references (wraps ToolRegistry)
   - `./src/bridge/action_projector.rs` - 30+ references
   - `./src/bridge/gate_controller.rs` - 10+ references
   - `./src/bridge/tool_permissions.rs` - Stub module

3. **Settings & Permissions:**
   - `./src/settings.rs` - `tool_permissions` field
   - `./src/app.rs` - `cleanup_ghost_seeded_tool_permissions`
   - `./src/tenant.rs` - `AdminToolPolicy` stubs
   - `./src/workspace/settings_schemas.rs` - v1 schemas

4. **Extensions & Auth:**
   - `./src/extensions/manager.rs` - 50+ references
   - `./src/auth/extension.rs` - 20+ references

5. **Worker & Orchestrator:**
   - `./src/worker/job.rs` - 30+ references
   - `./src/worker/container.rs` - 10+ references
   - `./src/orchestrator/api.rs` - 1 reference

6. **Tools Module (TO DELETE):**
   - `./src/tools/` - Entire directory (30+ files)

7. **Channels Web (TO DELETE):**
   - `./src/channels/web/` - Entire directory (100+ files)

8. **Other Dependencies:**
   - `./src/webhooks/mod.rs` - 10+ references
   - `./src/gate/approval.rs` - 10+ references
   - `./src/wasm_runtime/loader.rs` - 5+ references
   - `./src/main.rs` - ToolDispatcher creation
   - `./src/lib.rs` - Public exports

### Phase 4: Directory Deletion

**Directories to Delete:**
1. `./src/tools/` - Contains:
   - `mod.rs`, `tool.rs`, `registry.rs`, `dispatch.rs`, `execute.rs`
   - `permissions.rs`, `autonomy.rs`, `rate_limiter.rs`, `redaction.rs`
   - `runtime_filter.rs`, `schema_metrics.rs`, `schema_validator.rs`
   - `coercion.rs`, `builder/`
   - `builtin/` (30+ tool implementations)
   - `wasm/` (already extracted)
   - `mcp/` (already extracted)

2. `./src/channels/web/` - Contains:
   - `mod.rs`, `log_layer.rs`, `types.rs`, `util.rs`
   - `onboarding.rs`, `responses_api.rs`, `openai_compat.rs`
   - `features/`, `handlers/`, `platform/`, `oauth/`, `tests/`

## Critical Challenges

### 1. Bridge Layer Dependency
The `EffectBridgeAdapter` wraps `ToolRegistry` to provide V2 capability execution. This is the PRIMARY adapter between V1 and V2 systems. Removing it requires:
- Full V2 capability system implementation
- Migration of all tool execution paths
- Removal of V1 fallback paths

### 2. Scheduler Deprecation
`Scheduler` has deprecated `tools` field but still uses it as fallback. Need to:
- Remove V1 fallback path
- Ensure all execution goes through `effect_executor`
- Update all test fixtures

### 3. Settings Migration
`tool_permissions` field in Settings is deeply integrated:
- Database schema includes `tool_permissions.*` keys
- Migration logic in `cleanup_ghost_seeded_tool_permissions`
- UI depends on these settings
- Need data migration strategy

### 4. Test Infrastructure
Hundreds of tests create `ToolRegistry` instances:
- Need V2 capability fixtures
- Update test helpers
- Maintain test coverage

## Recommended Approach

### Option A: Incremental Removal (SAFER)
1. Mark all V1 code as deprecated
2. Add compile-time warnings
3. Remove one subsystem at a time
4. Verify compilation after each step

### Option B: Big Bang Removal (RISKY)
1. Delete directories first
2. Fix all compilation errors
3. Update all imports
4. High risk of missing dependencies

### Option C: Hybrid Approach (RECOMMENDED)
1. **Phase 1:** Extract remaining cross-cutting concerns (constants, types)
2. **Phase 2:** Remove V1 references from agent/scheduler/worker
3. **Phase 3:** Remove settings/permissions infrastructure
4. **Phase 4:** Delete directories and fix remaining errors
5. **Phase 5:** Verify and test

## Estimated Complexity

- **Files to Modify:** 50+
- **Lines to Change:** 5000+
- **Test Files to Update:** 100+
- **Compilation Errors Expected:** 500+

## Risk Assessment

**HIGH RISK:**
- Bridge layer removal could break V2 execution
- Settings migration could lose user data
- Test infrastructure could become unusable

**MITIGATION:**
- Incremental approach with verification
- Comprehensive testing at each step
- Backup/rollback strategy

## Next Steps

1. ✅ Complete Phase 1 extractions (constants, types)
2. Create V2 test fixtures
3. Remove V1 from agent layer (scheduler, dispatcher)
4. Remove V1 from settings layer
5. Delete directories
6. Fix compilation errors
7. Verify tests pass

## Status: ANALYSIS COMPLETE - READY FOR EXECUTION