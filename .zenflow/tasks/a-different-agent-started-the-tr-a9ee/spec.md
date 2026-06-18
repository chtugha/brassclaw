# Technical Specification: V1-to-V2 Tool Architecture Migration

> **Source code location**: `/Volumes/SSDE/brassclaw`. All relative paths below (e.g. `./src/`, `./crates/`) are relative to this root.

## Technical Context

- **Language**: Rust (backend), JavaScript (frontend — vanilla JS, no framework)
- **Build system**: Cargo workspace with multiple crates
- **Key crates**:
  - `brassclaw_engine` — v2 engine: Thread/Step/Capability execution model
  - `brassclaw_product_workflow` — `RebornServicesApi` trait, orchestration logic
  - `brassclaw_webui_v2` — Axum-based HTTP handlers for v2 web UI
  - `brassclaw_webui_v2_static` — static JS/CSS assets served by the v2 web UI
  - `brassclaw_common` — shared types across crates
  - `brassclaw_host_api` — host-level API traits
  - `brassclaw_extensions` — extension/tool registry and metadata
- **V1 modules being removed**:
  - `./src/tools/` — Tool trait, ToolRegistry, ToolDispatcher, all built-in tool impls, WASM loader, MCP client
  - `./src/channels/web/` — legacy web channel (settings, chat, auth, SSE, WS, OAuth)
- **V2 capability model** (already exists in `brassclaw_engine`):
  - `CapabilityDescriptor` — declares a capability's identity, schema, effects, trust level
  - `CapabilityHost` — registry/host that manages capabilities
  - `EffectExecutor` trait — execution interface (`execute_action -> ActionResult`)
  - `CapabilityLease` — authorization token scoped to a single invocation
  - `PermissionMode` — enum: `Allow`, `Ask`, `Deny`
  - `EffectKind` — enum tagging side-effect categories (`ReadFilesystem`, `WriteFilesystem`, `ExecuteCode`, `SpawnProcess`, `Network`, `ExternalWrite`, `DispatchCapability`, `ModifyExtension`, `UseSecret`, `ModifyApproval`)

## Implementation Approach

### 1. V1 Tool Architecture Removal

**Strategy**: Bottom-up deletion — remove leaf consumers first, then intermediate layers, then the `./src/tools/` root.

#### 1.1 Decouple Cross-Cutting Dependencies

Before deleting `./src/tools/` or `./src/channels/web/`, extract shared code that other modules depend on:

- **`./src/channels/web/log_layer.rs`** (`init_tracing`): Move to `./src/logging.rs`. Update `./src/main.rs` import.
- **`./src/channels/web/types.rs`** (shared types like `ChannelOnboardingState`, `ChannelOnboardingInfo`): Move to `brassclaw_common` or `brassclaw_host_api`. Update `./src/extensions/mod.rs` imports.
- **`./src/channels/web/` message builders** (`build_turns_from_db_messages`): Move to a shared utility in `./src/agent/` or `brassclaw_common`.
- **SSE event streaming** (`SseManager`): Replace references in `./src/agent/scheduler.rs` and `./src/worker/job.rs` with the v2 event stream mechanism from `brassclaw_webui_v2`.

#### 1.2 Remove V1 Bridge Wrappers

- **`./src/bridge/effect_adapter.rs`**: Currently wraps `ToolRegistry` to implement `EffectExecutor`. Rewrite to dispatch directly to v2 capability implementations (see Section 2). Remove all `ToolRegistry` imports.
- **`./src/bridge/tool_permissions.rs`**: Currently a stub returning always-allow via `PermissionState`. Replace with delegation to the v2 `CapabilityHost` permission checks. Remove file entirely once `CapabilityHost` handles it.
- **`./src/bridge/router.rs`**: Remove `ToolRegistry` usage for action resolution. Route through `CapabilityHost::resolve` instead.
- **`./src/bridge/action_projector.rs`**: Remove tool permission references; use `CapabilityLease` checks.

#### 1.3 Remove V1 Agent Integration

- **`./src/agent/agentic_loop.rs`**: Remove `ToolDispatcher` calls. Tool invocations already go through the engine's step executor; ensure all paths use `EffectExecutor`.
- **`./src/agent/thread_ops.rs`**: Remove `ToolRegistry` references and any v1 message formatting helpers.
- **`./src/agent/routine.rs`**, `./src/agent/scheduler.rs`: Remove `ToolRegistry` / `ToolDispatcher` dependencies.

#### 1.4 Remove V1 Settings/Config References

- **`./src/settings.rs`**: Remove the `tool_permissions: HashMap<String, PermissionState>` field. V2 permissions are stored via `CapabilityHost`.
- **`./src/app.rs`**: Remove `cleanup_ghost_seeded_tool_permissions`.
- **`./src/tenant.rs`**: Remove `AdminToolPolicy` / `AdminToolPolicyCache` management.
- **`./src/workspace/settings_schemas.rs`**: Remove v1 tool permission schema definitions.

#### 1.5 Remove V1 Testing Helpers

- **`./src/testing/mod.rs`**: Remove v1 tool test fixtures and mock registries. Replace with v2 capability test fixtures.

#### 1.6 Delete `./src/tools/`

After all dependents are updated, delete the entire directory:
- `tool.rs`, `registry.rs`, `dispatch.rs`, `execute.rs`, `permissions.rs`, `mod.rs`
- `builtin/` (all ~30 tool implementations)
- `wasm/` (extract reusable transport code first — see Section 1.7)
- `mcp/` (extract reusable transport code first — see Section 1.7)
- `coercion.rs`, `autonomy.rs`, `rate_limiter.rs`, `redaction.rs`, `runtime_filter.rs`, `schema_metrics.rs`, `schema_validator.rs`, `builder/`

#### 1.7 Extract Reusable Transport Code

Before deleting `./src/tools/wasm/` and `./src/tools/mcp/`:

- **WASM runtime**: The sandbox runtime, tool loader, and capability schema code is tool-agnostic transport. Extract to `./src/wasm_runtime/` (or a new `brassclaw_wasm` crate). The v2 `EffectExecutor` implementations for WASM-based tools will use this extracted module.
- **MCP client**: The MCP transport (stdio, SSE, HTTP), session management, and auth code is protocol infrastructure. Extract to `./src/mcp_client/` (or a new `brassclaw_mcp` crate). V2 capability implementations that talk to MCP servers will use this.

#### 1.8 Delete `./src/channels/web/`

After cross-cutting dependencies are relocated (Section 1.1), delete the entire directory:
- `features/`, `handlers/`, `platform/`, `oauth/`
- `types.rs`, `util.rs`, `openai_compat.rs`, `responses_api.rs`, `onboarding.rs`, `log_layer.rs`, `test_helpers.rs`, `mod.rs`

Update `./src/main.rs` to no longer mount v1 routes.

### 2. V2 Capability Implementations

**Location**: New module `./src/capabilities/` with sub-modules per domain.

**Pattern**: Each capability module defines:
1. A `const` or `fn` returning `CapabilityDescriptor` (id, provider, runtime, trust_ceiling, description, parameters_schema as JSON Schema, effects as `Vec<EffectKind>`, default_permission as `PermissionMode`)
2. An `execute` function matching the `EffectExecutor::execute_action` signature, returning `ActionResult`
3. Registration via a `register_all(host: &mut CapabilityHost)` function called at startup

**Module structure**:

```
./src/capabilities/
  mod.rs                  — register_all(), re-exports
  filesystem.rs           — read_file, write_file, list_dir, apply_patch, glob, grep, file_undo
  shell.rs                — shell
  network.rs              — http
  memory.rs               — memory_read, memory_write, memory_search, memory_tree
  messaging.rs            — message
  jobs.rs                 — create_job, cancel_job, list_jobs, job_status, job_events, job_prompt
  routines.rs             — routine_create, routine_update, routine_delete, routine_list, routine_history, routine_fire, event_emit
  skills.rs               — skill_install, skill_remove, skill_list, skill_search
  extensions.rs           — tool_install, tool_remove, tool_list, tool_search, tool_upgrade, tool_auth, tool_info, extension_info, tool_permission_set
  secrets.rs              — secret_list, secret_delete
  images.rs               — image_generate, image_analyze, image_edit
  system.rs               — echo, time, json, plan_update, restart, system_version, system_tools_list
  pairing.rs              — pairing_approve
```

**Capability descriptor example** (filesystem/read_file):

```rust
pub fn read_file_descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: "read_file".into(),
        provider: "builtin".into(),
        runtime: CapabilityRuntime::Native,
        trust_ceiling: TrustLevel::Standard,
        description: "Read the contents of a file at a given path".into(),
        parameters_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the file" },
                "offset": { "type": "integer", "description": "Line offset to start reading from" },
                "limit": { "type": "integer", "description": "Maximum number of lines to read" }
            },
            "required": ["path"]
        }),
        effects: vec![EffectKind::ReadFilesystem],
        default_permission: PermissionMode::Allow,
    }
}
```

**Execution flow**:
1. Engine Step requests capability execution via `CapabilityHost`
2. `CapabilityHost` checks `PermissionMode` via `CapabilityLease`
3. If allowed, `CapabilityHost` calls `EffectExecutor::execute_action`
4. The `EffectBridgeAdapter` (rewritten) dispatches to the appropriate capability module's `execute` function based on the capability ID
5. Capability executes and returns `ActionResult`

**Rewritten `EffectBridgeAdapter`**:

```rust
impl EffectExecutor for EffectBridgeAdapter {
    async fn execute_action(
        &self,
        capability_id: &str,
        params: serde_json::Value,
        lease: &CapabilityLease,
    ) -> Result<ActionResult, EffectError> {
        match capability_id {
            "read_file" => capabilities::filesystem::execute_read_file(params, lease, &self.ctx).await,
            "write_file" => capabilities::filesystem::execute_write_file(params, lease, &self.ctx).await,
            "shell" => capabilities::shell::execute_shell(params, lease, &self.ctx).await,
            // ... all capabilities matched by ID
            _ => self.execute_extension_action(capability_id, params, lease).await,
        }
    }
}
```

The fallthrough `execute_extension_action` handles dynamically-loaded WASM and MCP capabilities using the extracted transport modules.

#### 2.1 Dynamic Capability Registration (WASM & MCP Extensions)

Built-in capabilities are registered at startup via `capabilities::register_all()`. Dynamically-loaded capabilities (WASM extensions, MCP servers) require a separate registration path:

**WASM extensions**:
- On startup, scan the `registry/tools/*.json` metadata files (already used by `brassclaw_extensions`)
- For each installed WASM extension, load its manifest to extract capability metadata (id, description, parameters schema, effects)
- Construct a `CapabilityDescriptor` with `runtime: CapabilityRuntime::Wasm` and register it with `CapabilityHost`
- At runtime, when new extensions are installed via the `tool_install` capability, dynamically register the new capability with `CapabilityHost` so it appears in `list_capabilities` immediately

**MCP server capabilities**:
- On startup, iterate configured MCP server connections from the MCP config
- For each connected MCP server, query its `tools/list` endpoint to discover available tools
- Map each MCP tool to a `CapabilityDescriptor` with `runtime: CapabilityRuntime::Mcp` and register with `CapabilityHost`
- On MCP server reconnection or tool list change, re-sync registrations with `CapabilityHost`

**`CapabilityHost` dynamic registration API**:
```rust
impl CapabilityHost {
    pub fn register(&mut self, descriptor: CapabilityDescriptor, executor: Arc<dyn EffectExecutor>);
    pub fn unregister(&mut self, capability_id: &str);
    pub fn list_registered(&self) -> Vec<&CapabilityDescriptor>;
}
```

This ensures `RebornServicesApi::list_capabilities` returns the full set of built-in + WASM + MCP capabilities at any point in time.

### 3. V2 Permission Storage

**Approach**: Extend the existing v2 engine's `CapabilityHost` to persist per-user permission overrides.

**Data model**:

```rust
/// Stored in the user's settings database (same DB as other v2 settings)
struct CapabilityPermissionOverride {
    tenant_id: String,
    capability_id: String,
    permission_mode: PermissionMode,  // Allow | Ask | Deny
    updated_at: chrono::DateTime<chrono::Utc>,
}
```

**Storage**: Use the existing settings/config database that `brassclaw_engine` already uses. Add a `capability_permissions` table/collection:

| Column | Type | Description |
|--------|------|-------------|
| `tenant_id` | TEXT | Tenant/user identifier |
| `capability_id` | TEXT | Capability identifier |
| `permission_mode` | TEXT | "allow", "ask", or "deny" |
| `updated_at` | TIMESTAMP | Last modification time |

Primary key: composite `(tenant_id, capability_id)`. This ensures permission overrides are scoped per-tenant/per-user and do not collide across tenants.

**Resolution order**:
1. Check the current tenant's `CapabilityPermissionOverride` for the capability ID
2. Fall back to the capability's `default_permission` from its `CapabilityDescriptor`

### 4. RebornServicesApi Extensions

Add capability management methods to the `RebornServicesApi` trait in `brassclaw_product_workflow`:

```rust
/// In brassclaw_product_workflow/src/lib.rs (or services.rs)
#[async_trait]
pub trait RebornServicesApi {
    // ... existing methods ...

    async fn list_capabilities(&self) -> Result<Vec<RebornCapabilityInfo>, RebornServicesError>;

    async fn update_capability_permission(
        &self,
        capability_id: &str,
        permission_mode: PermissionMode,
    ) -> Result<(), RebornServicesError>;
}
```

**`RebornCapabilityInfo`** (new DTO in `brassclaw_product_workflow`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebornCapabilityInfo {
    pub id: String,
    pub description: String,
    pub provider: String,
    pub effects: Vec<String>,
    pub permission_mode: PermissionMode,
    pub default_permission: PermissionMode,
}
```

The implementation delegates to `CapabilityHost` to enumerate registered capabilities and merges with stored permission overrides.

### 5. WebUI V2 API Endpoints

Add two new endpoints in `brassclaw_webui_v2`:

#### `GET /api/webchat/v2/tools`

**Handler**: `./crates/brassclaw_webui_v2/src/handlers/tools.rs`

```rust
pub async fn list_tools(
    State(state): State<AppState>,
) -> Result<Json<Vec<ToolInfo>>, ApiError> {
    let capabilities = state.services.list_capabilities().await?;
    Ok(Json(capabilities.into_iter().map(ToolInfo::from).collect()))
}
```

**Response**:
```json
[
  {
    "id": "read_file",
    "description": "Read the contents of a file at a given path",
    "provider": "builtin",
    "effects": ["ReadFilesystem"],
    "permission_mode": "allow",
    "default_permission": "allow"
  }
]
```

#### `PUT /api/webchat/v2/tools/:capability_id/permission`

**Handler**: `./crates/brassclaw_webui_v2/src/handlers/tools.rs`

**Request body**:
```json
{
  "permission_mode": "ask"
}
```

**Response**: `200 OK` with empty body on success.

**Route registration**: Add to the existing v2 router in `brassclaw_webui_v2`.

### 6. Frontend: Tools Settings Tab

**New files**:
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tools-tab.js`
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/tools-manager.js`

**Modified files**:
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-schema.js` — add `tools` entry to `SETTINGS_TABS`
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js` — add `fetchTools()` and `updateToolPermission()` API functions

**`tools-tab.js` component structure**:

```
ToolsTab
  ├── Search/filter input
  ├── ToolsList (grouped by provider)
  │   ├── ProviderGroup (collapsible)
  │   │   ├── ToolRow
  │   │   │   ├── Name + Description
  │   │   │   ├── Effect tags (badges)
  │   │   │   └── PermissionMode selector (Allow/Ask/Deny dropdown or toggle)
  │   │   └── ...
  │   └── ...
  └── Empty state (no tools registered)
```

**`tools-manager.js`** (vanilla JS state manager — no React/framework dependency):
- ES module class `ToolsManager` that manages tool state and API communication
- `async init()` — fetches tool list from `GET /api/webchat/v2/tools`, stores in internal state
- `getTools()` — returns the current tools array
- `filterTools(query)` — returns tools filtered by name/description
- `groupByProvider()` — returns tools grouped by `provider` field
- `async updatePermission(id, mode)` — calls `PUT /api/webchat/v2/tools/:capability_id/permission`, updates local state optimistically
- `onChange(callback)` — registers a listener notified on state changes (tools loaded, permission updated)
- Uses standard DOM `CustomEvent` / callback pattern for reactivity, not React hooks

**UI behavior**:
- Tools are dynamically fetched — no hardcoded list
- Grouped by `provider` field (e.g., "builtin", extension names)
- Each tool shows effect kind badges (color-coded by category)
- Permission selector is a three-state toggle or dropdown: Allow (green), Ask (yellow), Deny (red)
- Search filters by name and description
- Changes persist immediately via PUT endpoint

**i18n**: Add keys to the existing i18n system:
- `settings.tools.title` = "Tools"
- `settings.tools.search_placeholder` = "Search tools..."
- `settings.tools.permission.allow` = "Allow"
- `settings.tools.permission.ask` = "Ask"
- `settings.tools.permission.deny` = "Deny"
- `settings.tools.empty` = "No tools registered"
- `settings.tools.effects` = "Effects"

### 7. Source Code Structure Changes Summary

**New files/modules**:
- `./src/capabilities/mod.rs` — capability registration root
- `./src/capabilities/filesystem.rs`
- `./src/capabilities/shell.rs`
- `./src/capabilities/network.rs`
- `./src/capabilities/memory.rs`
- `./src/capabilities/messaging.rs`
- `./src/capabilities/jobs.rs`
- `./src/capabilities/routines.rs`
- `./src/capabilities/skills.rs`
- `./src/capabilities/extensions.rs`
- `./src/capabilities/secrets.rs`
- `./src/capabilities/images.rs`
- `./src/capabilities/system.rs`
- `./src/capabilities/pairing.rs`
- `./src/logging.rs` (relocated from `./src/channels/web/log_layer.rs`)
- `./src/wasm_runtime/` or `./src/wasm_runtime.rs` (extracted from `./src/tools/wasm/`)
- `./src/mcp_client/` or `./src/mcp_client.rs` (extracted from `./src/tools/mcp/`)
- `./crates/brassclaw_webui_v2/src/handlers/tools.rs`
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tools-tab.js`
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/tools-manager.js`

**Deleted directories**:
- `./src/tools/` (entire directory)
- `./src/channels/web/` (entire directory)

**Modified files** (key changes):
- `./src/bridge/effect_adapter.rs` — rewrite to dispatch to `./src/capabilities/`
- `./src/bridge/router.rs` — remove `ToolRegistry`, use `CapabilityHost`
- `./src/bridge/tool_permissions.rs` — delete (functionality moves to `CapabilityHost`)
- `./src/bridge/action_projector.rs` — remove v1 permission references
- `./src/agent/agentic_loop.rs` — remove `ToolDispatcher` usage
- `./src/agent/thread_ops.rs` — remove v1 message formatting
- `./src/agent/routine.rs` — remove `ToolRegistry` dependency
- `./src/agent/scheduler.rs` — remove `SseManager`, remove `ToolRegistry`
- `./src/settings.rs` — remove `tool_permissions` field
- `./src/app.rs` — remove `cleanup_ghost_seeded_tool_permissions`
- `./src/tenant.rs` — remove `AdminToolPolicy` management
- `./src/workspace/settings_schemas.rs` — remove v1 tool schemas
- `./src/main.rs` — remove v1 channel mount, update tracing init import
- `./src/testing/mod.rs` — replace v1 test fixtures with v2
- `./crates/brassclaw_product_workflow/src/lib.rs` — add `list_capabilities`, `update_capability_permission` to `RebornServicesApi`
- `./crates/brassclaw_webui_v2/src/router.rs` — add tool endpoints
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-schema.js` — add tools tab
- `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js` — add tool API functions

## Verification Approach

1. **Compilation**: `cargo build` must succeed with zero errors. No references to `ToolRegistry`, `ToolDispatcher`, `Tool` trait, `PermissionState`, `AdminToolPolicy`, or any `./src/tools/` types.
2. **Tests**: `cargo test` must pass. Rewrite any tests that depended on v1 types to use v2 capability interfaces.
3. **Lint**: `cargo clippy -- -D warnings` must pass.
4. **Grep verification**: `grep -r "ToolRegistry\|ToolDispatcher\|PermissionState\|EngineVersion::V1\|V1Only" ./src/ ./crates/` returns zero matches.
5. **Frontend**: Tools tab loads, displays capabilities fetched from API, permission toggles persist correctly.
6. **No stub remnants**: `grep -r "stub\|TODO\|FIXME\|always.allow\|Made with Bob" ./src/ ./crates/` returns zero matches related to the migration.
