# Tool Permissions V2 Implementation

## Summary

This document describes the implementation of tool permissions configuration in the BrassClaw Reborn WebUI v2.

## Changes Made

### 1. Backend - WebUI V2 Handlers (`crates/brassclaw_webui_v2/src/handlers.rs`)

Added two new handler functions:

- **`get_tools`** - `GET /api/webchat/v2/tools`
  - Lists all tools with current permission state
  - Currently returns empty list (stub implementation)
  - TODO: Requires bridging to v1 tool registry or adding methods to RebornServicesApi

- **`update_tool_permission`** - `PUT /api/webchat/v2/tools/{tool_name}`
  - Updates permission state for a single tool
  - Currently returns 501 Not Implemented
  - TODO: Requires bridging to v1 settings store or adding methods to RebornServicesApi

Added response/request types:
- `ToolPermissionsResponse` - Response for GET endpoint
- `ToolPermissionEntry` - Single tool entry with name, description, state, etc.
- `UpdateToolPermissionRequest` - Request body for PUT endpoint

### 2. Backend - Route Descriptors (`crates/brassclaw_webui_v2/src/descriptors.rs`)

Added route constants:
- `WEBUI_V2_ROUTE_GET_TOOLS`
- `WEBUI_V2_ROUTE_UPDATE_TOOL_PERMISSION`
- `WEBUI_V2_PATTERN_GET_TOOLS`
- `WEBUI_V2_PATTERN_UPDATE_TOOL_PERMISSION`

Added descriptor functions:
- `get_tools_descriptor()` - Read policy with projection-only effect path
- `update_tool_permission_descriptor()` - Mutation policy with product workflow effect path

### 3. Backend - Router (`crates/brassclaw_webui_v2/src/router.rs`)

Wired the new handlers into the router:
- Added route for `GET /api/webchat/v2/tools`
- Added route for `PUT /api/webchat/v2/tools/{tool_name}`

### 4. Frontend - API Client (`crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js`)

Updated tool permission API functions to use v1 endpoints:
- `fetchTools()` - Now calls `/api/settings/tools` (v1 endpoint)
- `updateToolPermission(name, state)` - Now calls `/api/settings/tools/:name` (v1 endpoint)

### 5. Frontend - React Hook (`crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/useTools.js`)

Updated mutation handler to properly handle v1 API response:
- Uses `updatedTool.current_state` from response
- Updates cache with correct field name

### 6. Frontend - UI Component (`crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tools-tab.js`)

Updated to support both field name conventions:
- Added `toolState` variable that checks for both `current_state` and `state`
- Ensures compatibility with v1 API response format

## Architecture Notes

### Current State

The implementation follows a hybrid approach:

1. **V2 Route Surface**: New routes are defined in the v2 crate with proper descriptors and policies
2. **V1 Backend Delegation**: The frontend currently calls v1 endpoints (`/api/settings/tools`) because:
   - V2 handlers are stubs that don't have full implementation
   - Tool registry access requires bridging to v1 infrastructure
   - Settings store access requires bridging to v1 infrastructure

### Future Work

To complete the v2 native implementation:

1. **Add Tool Permission Methods to RebornServicesApi**:
   ```rust
   async fn get_tool_permissions(&self, caller: WebUiAuthenticatedCaller) 
       -> Result<ToolPermissionsResponse, RebornServicesError>;
   
   async fn update_tool_permission(&self, caller: WebUiAuthenticatedCaller, 
       tool_name: String, request: UpdateToolPermissionRequest) 
       -> Result<ToolPermissionEntry, RebornServicesError>;
   ```

2. **Implement Facade Methods**: Bridge to tool registry and settings store
3. **Update Frontend**: Switch from v1 endpoints to v2 endpoints once implemented
4. **Remove V1 Endpoints**: Deprecate `/api/settings/tools` routes

## Testing

The implementation maintains backward compatibility:
- Frontend continues to work with existing v1 endpoints
- V2 routes are available but return stubs
- No breaking changes to existing functionality

## Security

- Both routes require bearer token authentication
- GET uses read policy with projection-only effect path
- PUT uses mutation policy with product workflow effect path
- Rate limiting applied per descriptor policies
- CORS policy: same-origin only

## Files Modified

1. `crates/brassclaw_webui_v2/src/handlers.rs`
2. `crates/brassclaw_webui_v2/src/descriptors.rs`
3. `crates/brassclaw_webui_v2/src/router.rs`
4. `crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js`
5. `crates/brassclaw_webui_v2_static/static/js/pages/settings/hooks/useTools.js`
6. `crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tools-tab.js`

## Compliance

This implementation follows the architecture rules defined in `AGENTS.md`:
- New work belongs in `crates/` (Reborn)
- Handlers dispatch through RebornServicesApi facade
- No direct access to dispatcher, runtime, or DB stores
- Proper error mapping through WebUiV2HttpError
- Route descriptors define the contract
- Zero clippy warnings enforced