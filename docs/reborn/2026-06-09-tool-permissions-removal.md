# Tool Permissions Removal - Migration to V2 Capability-Based Authorization

**Date:** 2026-06-09  
**Status:** In Progress  
**Related:** `docs/reborn/contracts/approvals.md`, `docs/reborn/contracts/host-api.md`

## Summary

Removed the v1 tool permissions system entirely from BrassClaw. Tool authorization in v2 Reborn uses capability grants and approval gates instead of per-tool permission states.

## What Was Removed

### Backend (v1)
- **`src/tools/permissions.rs`** - Complete permissions module including:
  - `PermissionState` enum (AlwaysAllow/AskEachTime/Disabled)
  - `AdminToolPolicy` for multi-tenant restrictions
  - `effective_permission()` function
  - `filter_admin_disabled_tools()` function
  - `ADMIN_SETTINGS_USER_ID` constant
  - Seeded default permissions
  - Permission validation and caching

- **API Handlers:**
  - `settings_tools_list_handler()` in `src/channels/web/features/settings/mod.rs`
  - `settings_tools_set_handler()` in `src/channels/web/features/settings/mod.rs`
  - Helper functions: `permission_state_to_str()`, `str_to_permission_state()`

- **API Routes:**
  - `GET /api/settings/tools`
  - `PUT /api/settings/tools/:name`

- **Types:**
  - `ToolPermissionEntry` in `src/channels/web/types.rs`
  - `ToolPermissionsResponse` in `src/channels/web/types.rs`
  - `UpdateToolPermissionRequest` in `src/channels/web/types.rs`

### Backend (v2)
- **Stub Handlers** in `crates/brassclaw_webui_v2/src/handlers.rs`:
  - `get_tools()`
  - `update_tool_permission()`
  - Associated types

- **Routes** in `crates/brassclaw_webui_v2/src/router.rs`:
  - Removed tool permission route registrations

- **Descriptors** in `crates/brassclaw_webui_v2/src/descriptors.rs`:
  - `WEBUI_V2_ROUTE_GET_TOOLS`
  - `WEBUI_V2_ROUTE_UPDATE_TOOL_PERMISSION`
  - `WEBUI_V2_PATTERN_GET_TOOLS`
  - `WEBUI_V2_PATTERN_UPDATE_TOOL_PERMISSION`
  - `get_tools_descriptor()`
  - `update_tool_permission_descriptor()`

### Frontend
- **Components:**
  - `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tools-tab.js`
  - `crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/useTools.js`

- **API Functions** in `settings-api.js`:
  - `fetchTools()`
  - `updateToolPermission()`

- **UI Integration:**
  - Removed "Tools" tab from settings page
  - Removed tools tab from `SETTINGS_TABS` in `settings-schema.js`
  - Removed ToolsTab import and usage from `settings-page.js`

## V2 Reborn Architecture

In the Reborn architecture, tool authorization is handled through:

### 1. Capability Grants
Tools are exposed as capabilities with specific grants that define:
- Which operations are allowed
- Resource constraints (mounts, network, secrets)
- Expiry and invocation limits
- Principal/grantee identity

See `docs/reborn/contracts/host-api.md` for capability grant structure.

### 2. Approval Gates
Tools that require user confirmation use the approval system:
- `ApprovalRequirement::Always` - Always requires approval
- `ApprovalRequirement::Never` - Never requires approval  
- `ApprovalRequirement::Conditional` - Requires approval based on parameters

Approval requests are stored durably and resolved into scoped capability leases.

See `docs/reborn/contracts/approvals.md` for the approval workflow.

### 3. Authorization Flow
```
Tool Invocation
  → CapabilityHost checks grants
  → If RequireApproval: save ApprovalRecord, mark run BlockedApproval
  → User approves/denies
  → ApprovalResolver issues CapabilityLease
  → LeaseBackedAuthorizer validates lease
  → CapabilityHost dispatches with claimed lease
```

## Breaking Changes

### For V1 Runtime
The v1 runtime still has 67+ references to the permissions module that will need to be refactored:
- Agent dispatcher tool filtering
- Worker job admin policy caching
- Bridge adapter permission checks
- Settings system tool_permissions HashMap
- Tenant admin policy management
- Extension tools permission validation
- Config admin scope handling

**Next Steps:** These will be addressed in follow-up work to either:
1. Remove v1 tool authorization entirely
2. Implement a simpler authorization model for v1
3. Bridge v1 to use v2's capability system

### For V2 Users
- No UI for configuring per-tool permissions
- Tool authorization is controlled through:
  - Capability grants (configured by host/admin)
  - Approval gates (per-invocation user confirmation)
  - Admin policies (if implemented in v2)

## Migration Path

Users migrating from v1 to v2:
1. **Existing tool permissions are ignored** - v2 does not read v1 permission settings
2. **Use approval gates** - Tools that previously required `AskEachTime` will use approval gates
3. **Use capability grants** - Admin restrictions should be implemented via capability grant constraints

## Future Work

1. **Fix v1 compilation errors** - Refactor or remove the 67+ call sites that reference the permissions module
2. **Implement v2 admin policies** - If multi-tenant tool restrictions are needed, implement them using capability grants
3. **Document capability configuration** - Provide examples of how to configure tool authorization in v2
4. **Migration tooling** - Consider providing a tool to help users understand how their v1 permissions map to v2 capabilities

## References

- `docs/reborn/contracts/approvals.md` - Approval resolution contract
- `docs/reborn/contracts/host-api.md` - Host API and capability grants
- `docs/reborn/contracts/kernel-boundary.md` - Authority boundaries
- `AGENTS.md` - Architecture overview (Products/Loops/Kernel)