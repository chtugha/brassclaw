# Step 10 Phase 11 Status: Fixing Compilation Errors

## Overview
Phase 11 focuses on fixing compilation errors after deleting V1 infrastructure (tools/, channels/web/, worker/, mcp_client/, wasm_runtime/, channels/wasm/).

## Progress Summary

### ✅ Completed: Import-Only Fixes (Commit 247ce42bb)
Fixed files that only had import statements referencing deleted modules:

**Auth Files:**
- `src/auth/extension.rs` - Removed SharedCredentialRegistry import
- `src/auth/oauth.rs` - Removed SSRF protection functions import
- `src/auth/mod.rs` - Removed OAuthRefreshConfig and SSRF functions

**Capabilities:**
- `src/capabilities/network.rs` - Removed credential injection imports

**CLI Files:**
- `src/cli/acp.rs` - Removed worker::acp_bridge imports
- `src/cli/mcp.rs` - Removed mcp_client imports
- `src/cli/tool.rs` - Removed wasm_runtime imports

**Config:**
- `src/config/wasm.rs` - Stubbed to_runtime_config() method

**Hooks:**
- `src/hooks/bootstrap.rs` - Removed channels::wasm and wasm_runtime imports

**Orchestrator:**
- `src/orchestrator/api.rs` - Removed worker API imports

**Pairing:**
- `src/pairing/approval.rs` - Removed channels::wasm imports

**Setup:**
- `src/setup/wizard.rs` - Removed channels::wasm imports

**Skills:**
- `src/skills/mod.rs` - Removed OAuthRefreshConfig import

**Agent Files (Partial):**
- `src/agent/dispatcher.rs` - Commented out ApprovalRequirement import
- `src/agent/agent_loop.rs` - Commented out SoftwareBuilder field
- `src/agent/scheduler.rs` - Removed Worker imports (Phase 10)

### ⏳ In Progress: Code Usage Fixes

The following files have **code** (not just imports) that references deleted V1 types and need deeper refactoring:

#### Agent Files (High Priority)
1. **src/agent/agent_loop.rs** - 1 error
   - Line 748: `crate::tools::SoftwareBuilder` usage in code

2. **src/agent/dispatcher.rs** - 7 errors
   - Lines 447, 537, 542, 952, 1348, 1461, 1467, 1681: Various `crate::tools` type usages
   - Approval logic, tool execution, permission checks

3. **src/agent/scheduler.rs** - 1 error
   - Line 612: `crate::tools` usage

4. **src/agent/thread_ops.rs** - 5 errors (COMPLEX - Save for Last)
   - Line 2043: `use crate::tools::ApprovalRequirement` import
   - Lines 1811, 1829, 1965, 2032: Various `crate::tools` usages
   - Complex approval flow logic (lines 2030-2070)

#### CLI Files
5. **src/cli/acp.rs** - 2 errors
   - Module `acp_bridge` usage in code
   - Type `JobEventPayload` usage

6. **src/cli/mcp.rs** - Multiple errors
   - Types: `McpServerConfig`, `EffectiveTransport`, `McpSessionManager`, `McpClient`, `McpProcessManager`, `OAuthConfig`
   - Module `config` usage

7. **src/cli/tool.rs** - 6 errors
   - Type `CapabilitiesFile` usage (6 locations)

#### Other Files
8. **src/orchestrator/api.rs** - Additional errors beyond imports
   - Type `ToolDecisionDto` from deleted channels::web

9. **src/setup/wizard.rs** - Additional errors beyond imports
   - Type `ChannelCapabilitiesFile` usage

10. **src/capabilities/network.rs** - Additional errors beyond imports
    - Type `InjectedCredentials` usage

11. **src/skills/mod.rs** - Additional errors beyond imports
    - Type `SkillInstallPayload` usage

#### Test Files (15+ files)
Multiple test files reference deleted V1 infrastructure and need fixing.

## Error Categories

### Type Errors (Most Common)
- `ApprovalRequirement` - Tool approval logic
- `PermissionState` - Permission checking
- `ToolError`, `ToolOutput` - Tool execution
- `ToolPermissionSnapshot` - Permission snapshots
- `ApprovalContext` - Approval context
- `Worker` - Worker infrastructure
- `RateLimiter` - Rate limiting
- `InjectedCredentials` - Credential injection
- `McpServerConfig`, `McpClient`, etc. - MCP infrastructure
- `CapabilitiesFile`, `ChannelCapabilitiesFile` - WASM capabilities
- `ToolDecisionDto` - Web channel DTOs

### Module Errors
- `crate::tools` - Deleted tools module
- `crate::channels::web` - Deleted web channel
- `crate::wasm_runtime` - Deleted WASM runtime
- `crate::mcp_client` - Deleted MCP client
- `crate::worker` - Deleted worker
- `acp_bridge` - ACP bridge module
- `config` (in mcp context) - MCP config module

## Strategy for Remaining Work

### Phase 11A: Simple Code Fixes (Next)
Fix files with straightforward code changes:
1. Comment out or stub simple type usages
2. Add TODO comments for V2 reimplementation
3. Focus on getting compilation to succeed

### Phase 11B: Complex Logic Fixes
Handle files with complex business logic:
1. `src/agent/thread_ops.rs` - Approval flow (most complex)
2. `src/agent/dispatcher.rs` - Tool execution logic
3. CLI command implementations

### Phase 11C: Test Files
Fix all test files after main code compiles.

## Current Branch
`step-10-phase-11-fix-errors` (commit 247ce42bb)

## Next Steps
1. Continue fixing code usage errors in agent files
2. Fix CLI files
3. Fix remaining src files
4. Fix test files
5. Verify compilation succeeds
6. Merge to main

## Notes
- Import-only fixes are complete and committed
- Code usage fixes require more careful analysis
- Some logic may need to be stubbed temporarily
- V2 reimplementation will happen in future steps