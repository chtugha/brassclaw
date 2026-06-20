# Phase 11 Comprehensive Status: Post-V1 Deletion Error Fixing

## Current State (Commit 247ce42bb)

**Branch**: `step-10-phase-11-fix-errors`

**What's Been Done**:
- Phase 10 complete: Deleted V1 directories (tools/, channels/web/, worker/, mcp_client/, wasm_runtime/, channels/wasm/)
- Phase 10: Stubbed app.rs init_tools() and init_extensions() methods
- Phase 11 (partial): Fixed import-only errors in 15 files

**Current Compilation Status**: ~100+ compilation errors across ~30+ files

## Error Analysis

### Error Categories

1. **Module Not Found Errors** (E0432/E0433):
   - `crate::tools` - 50+ references
   - `crate::channels::web` - 10+ references  
   - `crate::wasm_runtime` - 15+ references
   - `crate::mcp_client` - 5+ references
   - `crate::channels::wasm` - 5+ references

2. **Type Not Found Errors** (E0425):
   - `ToolRegistry` - 15+ references
   - `ApprovalRequirement` - 10+ references
   - `ApprovalContext` - 5+ references
   - `PermissionState` - 5+ references
   - `SoftwareBuilder` - 2 references
   - `SharedCredentialRegistry` - 3 references
   - `WorkerDeps` - 1 reference
   - Many others

3. **Value Not Found Errors** (E0425):
   - `mcp_session_manager`, `mcp_process_manager`, `wasm_tool_runtime`, `dev_loaded_tool_names`, `credential_registry`, `tool`

## Files Requiring Fixes (Grouped by Complexity)

### Critical Agent Files (High Complexity)
These files have deep integration with V1 tool system and require careful refactoring:

1. **src/agent/thread_ops.rs** - 6 errors
   - Complex approval flow logic (lines 2030-2070)
   - ApprovalRequirement usage
   - Tool execution and result processing
   - **Strategy**: Stub approval logic temporarily, mark for V2 reimplementation

2. **src/agent/dispatcher.rs** - 10 errors
   - AdminToolPolicyCache, tool filtering
   - Tool execution pipeline
   - Result processing
   - **Strategy**: Remove admin policy logic, stub tool execution paths

3. **src/agent/scheduler.rs** - 11 errors
   - ToolRegistry dependencies
   - ApprovalContext usage
   - WorkerDeps construction
   - **Strategy**: Remove ToolRegistry fields, stub approval contexts

4. **src/agent/agent_loop.rs** - 2 errors
   - ToolRegistry field
   - SoftwareBuilder field (already commented out)
   - **Strategy**: Remove ToolRegistry field entirely

5. **src/agent/commands.rs** - 1 error
   - Tool execution reference
   - **Strategy**: Comment out or stub

6. **src/agent/self_repair.rs** - 2 errors
   - SoftwareBuilder trait references
   - **Strategy**: Comment out self-repair functionality temporarily

### App Initialization (Medium Complexity)

7. **src/app.rs** - 5 errors
   - Missing V1 initialization values
   - Credential registry references
   - **Strategy**: Remove V1 field references from AppComponents initialization

8. **src/auth/extension.rs** - 5 errors
   - ToolRegistry and SharedCredentialRegistry
   - **Strategy**: Remove tool registry integration, stub credential resolution

### CLI Commands (Medium Complexity)

9. **src/cli/acp.rs** - Already has TODO comment
10. **src/cli/mcp.rs** - Already has TODO comment  
11. **src/cli/tool.rs** - 10+ errors referencing wasm_runtime types
    - **Strategy**: Stub entire CLI command implementations

### Configuration (Low-Medium Complexity)

12. **src/config/builder.rs** - 2 errors
    - BuilderConfig type
    - **Strategy**: Stub or remove builder config conversion

13. **src/config/wasm.rs** - Already stubbed

### Other Core Files (Medium Complexity)

14. **src/gate/approval.rs** - 3 errors
    - Tool parameter extraction and redaction
    - Rate limiting
    - **Strategy**: Stub approval gate logic

15. **src/settings.rs** - 1 error
    - tool_permissions HashMap field
    - **Strategy**: Remove field entirely

16. **src/tenant.rs** - 2 errors
    - AdminToolPolicy types
    - **Strategy**: Remove admin policy methods

17. **src/webhooks/mod.rs** - 4 errors
    - Tool trait reference
    - Signature verification from channels::wasm
    - **Strategy**: Stub webhook tool integration

18. **src/setup/channels.rs** - 1 error
    - channels::wasm::SetupSchema
    - **Strategy**: Stub channel setup

19. **src/skills/mod.rs** - 1 error (beyond import)
    - SharedCredentialRegistry usage
    - **Strategy**: Stub credential registration

### Test Files (Deferred)
- 15+ test files need fixing
- **Strategy**: Fix after main code compiles

## Recommended Approach

Given the scale (100+ errors across 30+ files), the most pragmatic approach is:

### Phase 11A: Aggressive Stubbing Strategy
1. **Goal**: Get the code to compile, even if functionality is temporarily broken
2. **Method**: 
   - Comment out entire functions/methods that reference V1 types
   - Add `unimplemented!()` or `todo!()` macros with clear V2 migration notes
   - Remove V1 fields from structs entirely
   - Stub return values where needed

3. **Priority Order**:
   - App initialization (src/app.rs) - blocks everything
   - Agent core (agent_loop.rs, scheduler.rs) - critical path
   - Agent dispatcher/thread_ops - complex but can be stubbed
   - Settings/config files - straightforward removals
   - CLI commands - can be fully stubbed
   - Other files - case by case

### Phase 11B: Selective Restoration
After compilation succeeds:
1. Identify which stubbed functionality is actually needed for basic operation
2. Implement V2 equivalents for critical paths only
3. Leave non-critical features stubbed with TODO markers

### Phase 11C: Test Fixes
Fix test files to work with V2 architecture

## Estimated Effort

- **Phase 11A (Stubbing)**: 3-4 hours of focused work
- **Phase 11B (Restoration)**: 5-10 hours depending on scope
- **Phase 11C (Tests)**: 2-3 hours

**Total**: 10-17 hours of implementation work

## Alternative: Incremental Restoration

Instead of deleting V1 first, could have:
1. Built complete V2 system alongside V1
2. Migrated call sites incrementally
3. Deleted V1 only after V2 was fully wired

However, we're past that point - V1 is deleted, so forward is the only option.

## Next Immediate Steps

1. Fix src/app.rs to remove V1 field references
2. Fix src/agent/agent_loop.rs to remove ToolRegistry
3. Fix src/agent/scheduler.rs to remove ToolRegistry and ApprovalContext
4. Continue systematically through remaining files
5. Commit progress frequently

## Success Criteria for Phase 11

- `cargo build` succeeds (even with stubbed functionality)
- No compilation errors
- Clear TODO markers for all stubbed V2 work
- Tests can be addressed separately in Phase 11C