# Step 10: V1 Architecture Removal - Implementation Plan

## Executive Summary

**Scope:** Remove 97 V1 files (46 in `./src/tools/`, 51 in `./src/channels/web/`) and update 300+ references across 50+ files.

**Status:** V2 Reborn system is COMPLETE. This is a cleanup task to remove deprecated V1 code.

**Approach:** Incremental removal with verification at each step to maintain clean compilation.

## Phase 1: Extract Remaining Cross-Cutting Dependencies (MINIMAL)

Most extractions are already complete. Only need to move a few constants:

### 1.1: Move SSE Constants to Common Location
**Files to modify:** 4
- Extract `DEFAULT_BROADCAST_BUFFER` and `DEFAULT_MAX_CONNECTIONS` from `./src/channels/web/platform/sse.rs`
- Move to `./src/config/channels.rs` or create `./src/sse_config.rs`
- Update imports in:
  - `./src/tunnel/mod.rs` (test only)
  - `./src/config/channels.rs`
  - `./src/channels/web/` (will be deleted anyway)

### 1.2: Move GATEWAY_CHANNEL_NAME Constant
**Files to modify:** 2
- Extract `GATEWAY_CHANNEL_NAME` from `./src/channels/web/mod.rs`
- Move to `./src/channels/mod.rs` or `./src/config/channels.rs`
- Update import in `./src/bridge/router.rs`

**Verification:** `cargo check` passes

## Phase 2: WASM & MCP (ALREADY COMPLETE)

✅ `./src/wasm_runtime/` exists
✅ `./src/mcp_client/` exists
No work needed.

## Phase 3: Remove V1 References from Core Modules

### 3.1: Update Bridge Layer to Remove ToolRegistry Dependency
**Critical:** The `EffectBridgeAdapter` is V2 code that temporarily wraps V1 `ToolRegistry`.

**Strategy:** Replace `tools: Arc<ToolRegistry>` with V2-only dependencies:
- Use `capability_registry` for action lookup
- Use `auth_manager` for credential checks
- Remove V1 fallback paths

**Files to modify:**
- `./src/bridge/effect_adapter.rs` - Remove `tools` field, update 12 usages
- `./src/bridge/action_projector.rs` - Update to use V2 capabilities
- `./src/bridge/tool_permissions.rs` - Update or remove (stub module)
- `./src/bridge/gate_controller.rs` - Remove ToolRegistry parameter

**Verification:** `cargo check --lib` passes for bridge module

### 3.2: Remove V1 from Agent Layer
**Files to modify:**
- `./src/agent/scheduler.rs` - Remove deprecated `tools` field (line 55, 70)
- `./src/agent/agent_loop.rs` - Remove `tools()` method, update construction
- `./src/agent/dispatcher.rs` - Remove ToolRegistry references
- `./src/agent/thread_ops.rs` - Remove tool_permissions DB writes
- `./src/agent/routine.rs` - Remove tool_permissions from routine config

**Verification:** `cargo check --lib` passes for agent module

### 3.3: Remove V1 from Worker Layer
**Files to modify:**
- `./src/worker/job.rs` - Remove ToolRegistry from WorkerDeps
- `./src/worker/container.rs` - Remove ToolRegistry parameter

**Verification:** `cargo check --lib` passes for worker module

### 3.4: Remove V1 from Extensions & Auth
**Files to modify:**
- `./src/extensions/manager.rs` - Remove ToolRegistry parameter from constructor
- `./src/auth/extension.rs` - Remove ToolRegistry parameter

**Verification:** `cargo check --lib` passes

### 3.5: Remove V1 from Settings Layer
**Files to modify:**
- `./src/settings.rs` - Remove `tool_permissions` field
- `./src/app.rs` - Remove `cleanup_ghost_seeded_tool_permissions` function
- `./src/tenant.rs` - Remove `AdminToolPolicy` stubs
- `./src/workspace/settings_schemas.rs` - Remove `tool_permissions.*` schema

**Verification:** `cargo check --lib` passes

### 3.6: Remove V1 from Other Dependencies
**Files to modify:**
- `./src/webhooks/mod.rs` - Remove ToolRegistry from ToolWebhookState
- `./src/gate/approval.rs` - Remove ToolRegistry parameters
- `./src/wasm_runtime/loader.rs` - Remove ToolRegistry parameter
- `./src/lib.rs` - Remove ToolRegistry from public exports
- `./src/main.rs` - Remove ToolDispatcher creation

**Verification:** `cargo check` passes

## Phase 4: Delete V1 Directories

### 4.1: Remove `mod tools;` Declaration
**File:** `./src/lib.rs` or `./src/main.rs`
- Comment out or remove `mod tools;`
- Comment out or remove `pub use tools::...;` exports

### 4.2: Delete `./src/tools/` Directory
**Command:** `rm -rf /Volumes/SSDE/brassclaw/src/tools`

**Files deleted:** 46 files including:
- `mod.rs`, `tool.rs`, `registry.rs`, `dispatch.rs`, `execute.rs`
- `permissions.rs`, `autonomy.rs`, `rate_limiter.rs`
- `builtin/` (30+ tool implementations)
- All supporting modules

### 4.3: Remove `mod channels;` Declaration for Web
**File:** `./src/channels/mod.rs`
- Remove `pub mod web;` or comment it out

### 4.4: Delete `./src/channels/web/` Directory
**Command:** `rm -rf /Volumes/SSDE/brassclaw/src/channels/web`

**Files deleted:** 51 files including:
- `mod.rs`, `types.rs`, `util.rs`, `log_layer.rs`
- `features/`, `handlers/`, `platform/`, `oauth/`, `tests/`

**Verification:** Directories no longer exist

## Phase 5: Fix Remaining Compilation Errors

### 5.1: Run Initial Build
**Command:** `cargo build 2>&1 | tee build_errors.log`

Expected errors: 100-500 compilation errors from:
- Test files creating ToolRegistry instances
- Imports from deleted modules
- Type mismatches from removed fields

### 5.2: Fix Test Infrastructure
**Strategy:** Update test helpers to use V2 fixtures
- Replace `ToolRegistry::new()` with V2 capability setup
- Update test imports
- Remove V1-specific test cases

### 5.3: Fix Remaining Imports
**Strategy:** Search and replace
- `use crate::tools::` → Remove or replace with V2
- `use crate::channels::web::` → Remove or replace with V2

### 5.4: Iterative Compilation
Repeat until `cargo build` succeeds:
1. Run `cargo build`
2. Fix first 10-20 errors
3. Commit changes
4. Repeat

**Verification:** `cargo build` succeeds with zero errors

## Phase 6: Final Verification

### 6.1: Grep Verification
**Commands:**
```bash
grep -r "ToolRegistry" ./src/ ./crates/
grep -r "ToolDispatcher" ./src/ ./crates/
grep -r "tool_permissions" ./src/ ./crates/
grep -r "AdminToolPolicy" ./src/ ./crates/
grep -r "use crate::tools::" ./src/
grep -r "use crate::channels::web::" ./src/
```

**Expected:** Zero matches (except in comments/docs)

### 6.2: Directory Verification
**Commands:**
```bash
ls ./src/tools 2>&1
ls ./src/channels/web 2>&1
```

**Expected:** "No such file or directory"

### 6.3: Module Verification
**Check:** `./src/wasm_runtime/` and `./src/mcp_client/` compile independently

### 6.4: Test Suite
**Command:** `cargo test`

**Expected:** All tests pass (or only expected failures)

## Risk Mitigation

### Backup Strategy
Before starting:
```bash
cd /Volumes/SSDE/brassclaw
git checkout -b step-10-v1-removal
git commit -am "Checkpoint before V1 removal"
```

### Rollback Plan
If compilation cannot be fixed:
```bash
git reset --hard HEAD
git checkout main
```

### Incremental Commits
Commit after each phase:
- Phase 1: "Extract remaining constants"
- Phase 3.1: "Remove ToolRegistry from bridge"
- Phase 3.2: "Remove ToolRegistry from agent"
- etc.

## Estimated Effort

- **Phase 1:** 30 minutes (minimal work)
- **Phase 2:** 0 minutes (already done)
- **Phase 3:** 4-6 hours (50+ files to modify)
- **Phase 4:** 5 minutes (directory deletion)
- **Phase 5:** 4-8 hours (fixing 100-500 errors)
- **Phase 6:** 30 minutes (verification)

**Total:** 10-15 hours of focused work

## Success Criteria

1. ✅ `./src/tools/` directory deleted
2. ✅ `./src/channels/web/` directory deleted
3. ✅ `cargo build` succeeds with zero errors
4. ✅ No imports from deleted directories
5. ✅ Grep verification shows zero V1 references
6. ✅ WASM and MCP modules compile independently
7. ✅ Test suite passes

## Recommendation

Given the scope, I recommend:

**Option A (Recommended):** Execute phases incrementally with verification
- Start with Phase 1 (constants)
- Then Phase 3 (remove references)
- Then Phase 4 (delete directories)
- Then Phase 5 (fix errors)

**Option B:** Create sub-tasks for each phase
- Allows multiple sessions
- Easier to track progress
- Lower risk of incomplete work

**Option C:** Pair with another developer
- One person removes references
- Other person fixes compilation errors
- Faster completion

## Next Steps

1. Get approval for approach
2. Create git branch for work
3. Start with Phase 1 (minimal extraction)
4. Proceed incrementally through phases
5. Verify at each step
6. Complete with full test suite run

## Status: PLAN COMPLETE - AWAITING APPROVAL TO PROCEED