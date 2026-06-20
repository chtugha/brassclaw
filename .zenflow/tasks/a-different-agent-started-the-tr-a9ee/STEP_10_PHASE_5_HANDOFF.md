# Step 10 Phase 5: Compilation Error Fixing - Handoff Document

## Current Status

**Date**: 2026-06-19
**Branch**: `step-10-v1-removal`
**Commit**: `951d35726` - "Step 10 Phase 4 complete: Delete V1 directories"
**Compilation Errors**: 156 errors across ~30 files

## What Was Accomplished

### Phase 4: V1 Directory Deletion ✅ COMPLETE
- Deleted `./src/tools/` (46 files, ~40,000 lines)
- Deleted `./src/channels/web/` (51 files, ~36,000 lines)
- **Total V1 code removed**: 76,935 lines
- Updated module declarations in `lib.rs` and `channels/mod.rs`
- All changes committed to git

## Phase 5: Compilation Error Fixing - TODO

### Error Summary
- **Total Errors**: 156
- **Error Types**:
  - 117 "cannot find" errors (E0433)
  - 38 "unresolved import" errors (E0432)

### Files Requiring Fixes (Priority Order)

#### High Priority - Agent Module (9 files)
1. `src/agent/agent_loop.rs` - ToolRegistry import
2. `src/agent/dispatcher.rs` - permissions, redact_params
3. `src/agent/commands.rs` - Tool trait
4. `src/agent/routine_engine.rs` - tools imports
5. `src/agent/scheduler.rs` - tools imports
6. `src/agent/self_repair.rs` - SoftwareBuilder
7. `src/agent/thread_ops.rs` - redact_params, ApprovalRequirement

#### High Priority - Bridge Module (6 files)
8. `src/bridge/action_discovery.rs` - require_str, ToolError, ToolOutput
9. `src/bridge/action_projector.rs` - ToolRegistry, PermissionState
10. `src/bridge/effect_adapter.rs` - multiple tools imports
11. `src/bridge/gate_controller.rs` - tools imports
12. `src/bridge/tool_permissions.rs` - entire file can be deleted
13. `src/bridge/router.rs` - tools imports

#### Medium Priority - App/Auth (2 files)
14. `src/app.rs` - ToolRegistry
15. `src/auth/extension.rs` - ToolRegistry, builtin tools

#### Medium Priority - Capabilities (5 files)
16. `src/capabilities/filesystem.rs` - tools imports
17. `src/capabilities/images.rs` - tools imports
18. `src/capabilities/memory.rs` - tools imports
19. `src/capabilities/network.rs` - tools imports
20. `src/capabilities/skills.rs` - tools imports

#### Lower Priority - Other Modules
21. `src/config/channels.rs` - tools imports
22. `src/context/state.rs` - tools imports
23. Additional files from full error list

### Fixing Strategy

#### Step 1: Remove Import Statements
For each file, remove all `use crate::tools::*` import statements:
```rust
// DELETE these lines:
use crate::tools::ToolRegistry;
use crate::tools::permissions::PermissionState;
use crate::tools::redact_params;
use crate::tools::ApprovalRequirement;
// etc.
```

#### Step 2: Remove V1-Dependent Code
Delete or comment out code blocks that use V1 types:
- Functions that take `ToolRegistry` parameters
- Code using `PermissionState`
- Test code using V1 infrastructure

#### Step 3: Update Deprecated Fields
In `scheduler.rs`, remove the deprecated `tools` field that was marked for removal in Step 9.7+.

#### Step 4: Delete Obsolete Files
- `src/bridge/tool_permissions.rs` - entire file can be deleted (V1 permission stub)

#### Step 5: Verify Incrementally
After fixing each major module (agent, bridge, etc.), run:
```bash
cargo build 2>&1 | grep "^error" | wc -l
```
to track progress.

### Expected Outcome

After Phase 5 completion:
- ✅ Zero compilation errors
- ✅ All V1 imports removed
- ✅ V1-dependent code removed or stubbed
- ✅ Project compiles successfully
- ⚠️ Tests may still fail (Phase 6 work)

### Time Estimate

- **Estimated time**: 4-8 hours
- **Complexity**: Medium (mostly mechanical deletions)
- **Risk**: Low (V2 system is complete and operational)

## Next Steps After Phase 5

### Phase 6: Final Verification
1. Run `cargo test` and fix test failures
2. Run `cargo clippy -- -D warnings`
3. Verify no V1 remnants: `grep -r "ToolRegistry\|ToolDispatcher" ./src/`
4. Update documentation
5. Merge to main branch

## Key Context for Next Developer

### V2 System is Complete
- All 47 V2 capability execute functions are implemented
- EffectBridgeAdapter is fully wired
- WebUI v2 tools management is complete
- The V2 system is operational and tested

### V1 Code Can Be Safely Deleted
- Most V1 code should simply be removed, not replaced
- The deprecated fields in scheduler.rs can now be fully removed
- Test code may need V2 equivalents, but production code is V2-ready

### Architecture Notes
- V2 uses `EffectExecutor` trait instead of `ToolRegistry`
- V2 uses `CapabilityLease` instead of `ApprovalRequirement`
- V2 permissions are in `CapabilityHost`, not `PermissionState`
- Bridge layer (`effect_adapter.rs`) is V2 code, not V1

## Files for Reference

- `STEP_10_ANALYSIS.md` - Comprehensive dependency analysis
- `STEP_10_IMPLEMENTATION_PLAN.md` - Detailed 6-phase plan
- `STEP_10_PHASE_5_STATUS.md` - Current status tracking

## Quick Start Commands

```bash
# Switch to the branch
cd /Volumes/SSDE/brassclaw
git checkout step-10-v1-removal

# Check current error count
cargo build 2>&1 | grep "^error" | wc -l

# See which files have errors
cargo build 2>&1 | grep "\.rs:" | head -50

# Start fixing files (example)
# Edit src/agent/agent_loop.rs and remove V1 imports
# Then check progress
cargo build 2>&1 | grep "^error" | wc -l
```

## Success Criteria

Phase 5 is complete when:
- [ ] `cargo build` succeeds with zero errors
- [ ] All `use crate::tools::` imports removed
- [ ] All V1-dependent code removed
- [ ] Commit message: "Step 10 Phase 5 complete: Fix all compilation errors"