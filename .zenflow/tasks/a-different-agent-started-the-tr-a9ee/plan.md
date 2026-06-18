# Full SDD workflow

## Configuration
- **Artifacts Path**: {@artifacts_path} → `.zenflow/tasks/{task_id}`

---

## Agent Instructions

**IMPORTANT — Source code location**: The actual codebase is at `/Volumes/SSDE/brassclaw`, NOT in the Zenflow project directory (`/Users/ollama/zenflow_projects/ironclaw`). All relative paths in the steps below (e.g. `./src/`, `./crates/`) are relative to `/Volumes/SSDE/brassclaw`. Implementation agents MUST `cd /Volumes/SSDE/brassclaw` before running any commands (`cargo build`, `cargo test`, `grep`, etc.) and use `/Volumes/SSDE/brassclaw` as the working directory for all file reads/edits.

---

## Workflow Steps

### [x] Step: Requirements
<!-- chat-id: d02bcf48-97c7-4ba2-868a-063f76a23ee6 -->

Create a Product Requirements Document (PRD) based on the feature description.

1. Review existing codebase to understand current architecture and patterns
2. Analyze the feature definition and identify unclear aspects
3. Ask the user for clarifications on aspects that significantly impact scope or user experience
4. Make reasonable decisions for minor details based on context and conventions
5. If user can't clarify, make a decision, state the assumption, and continue

Focus on **what** the feature should do and **why**, not **how** it should be built. Do not include technical implementation details, technology choices, or code-level decisions — those belong in the Technical Specification.

Save the PRD to `{@artifacts_path}/requirements.md`.

### [x] Step: Technical Specification
<!-- chat-id: 9045a673-1618-4ea7-8fdc-a95e67c5966f -->

Create a technical specification based on the PRD in `{@artifacts_path}/requirements.md`.

1. Review existing codebase architecture and identify reusable components
2. Define the implementation approach

Do not include implementation steps, phases, or task breakdowns — those belong in the Planning step.

Save to `{@artifacts_path}/spec.md` with:
- Technical context (language, dependencies)
- Implementation approach referencing existing code patterns
- Source code structure changes
- Data model / API / interface changes
- Verification approach using project lint/test commands

### [x] Step: Planning
<!-- chat-id: bc36befa-b9a7-45dc-a816-c35ac3ea6e33 -->

Create a detailed implementation plan based on `{@artifacts_path}/spec.md`.

### [x] Step: Extract cross-cutting dependencies from v1 modules
<!-- chat-id: 0ee6fa07-cb26-4843-a89c-f24c92c55640 -->

Before deleting `./src/tools/` or `./src/channels/web/`, relocate shared code that other modules depend on:

- Move `init_tracing` from `./src/channels/web/log_layer.rs` to new `./src/logging.rs`. Update `./src/main.rs` import.
- Move shared types (`ChannelOnboardingState`, `ChannelOnboardingInfo`) from `./src/channels/web/types.rs` to `brassclaw_common` or `brassclaw_host_api`. Update `./src/extensions/mod.rs` imports.
- Move `build_turns_from_db_messages` from `./src/channels/web/` to a shared utility in `./src/agent/` or `brassclaw_common`.
- Define an abstract event publisher trait (e.g. `EventPublisher`) in `brassclaw_common` or `brassclaw_host_api`. Replace `SseManager` references in `./src/agent/scheduler.rs` and `./src/worker/job.rs` with this trait. The `brassclaw_webui_v2` crate implements the consumer/subscriber side, preserving unidirectional dependency (WebUI → Core → Common). Do NOT import `brassclaw_webui_v2` from core modules.

**Verification**: `cargo build` succeeds. Relocated code still functions. No imports from `./src/channels/web/` remain outside that directory (except within the channel itself).

### [x] Step: Extract WASM runtime and MCP client transport
<!-- chat-id: e0827f38-ed24-48f3-90f3-5af470a1c8db -->

Extract tool-agnostic transport code before deleting `./src/tools/`:

- Extract WASM sandbox runtime, tool loader, and capability schema code from `./src/tools/wasm/` into `./src/wasm_runtime/` (or new `brassclaw_wasm` crate).
- Extract MCP transport (stdio, SSE, HTTP), session management, and auth from `./src/tools/mcp/` into `./src/mcp_client/` (or new `brassclaw_mcp` crate).
- If creating new workspace crates, add them to `./Cargo.toml` workspace members list and configure their `[dependencies]` sections. If the overhead is not justified, keep them as internal submodules (`./src/wasm_runtime/`, `./src/mcp_client/`) within the main crate instead.
- Update any internal imports to use the new module locations.

**Verification**: `cargo build` succeeds. WASM and MCP transport modules compile independently of `./src/tools/`. If new crates were created, they appear in workspace members and resolve correctly.

### [x] Step: Create v2 capabilities module — filesystem tools
<!-- chat-id: 911397a2-54ab-4392-be4b-96f03e65684a -->

Create `./src/capabilities/mod.rs` and `./src/capabilities/filesystem.rs`:

- Implement `CapabilityDescriptor` for: `read_file`, `write_file`, `list_dir`, `apply_patch`, `glob`, `grep`, `file_undo`.
- Each descriptor declares `effects: vec![EffectKind::ReadFilesystem]` (or `WriteFilesystem` for write ops).
- Implement `execute` functions matching `EffectExecutor::execute_action` signature, returning `ActionResult`.
- Port core logic from `./src/tools/builtin/` equivalents (read_file, write_file, etc.).
- Add `register_all(host: &mut CapabilityHost)` skeleton in `mod.rs` that registers filesystem capabilities.
- Write unit tests for each capability's descriptor correctness and execution happy path.

**Verification**: `cargo test` for the capabilities module passes. Descriptors have correct IDs, effects, and schemas.

### [x] Step: Create v2 capabilities — shell, network, memory, messaging
<!-- chat-id: 73a64107-b22d-4f44-88de-c37f444a6ed5 -->

Implement the following capability modules:

- `./src/capabilities/shell.rs` — `shell` capability. Effects: `ExecuteCode`, `SpawnProcess`. Port from `ShellTool`.
- `./src/capabilities/network.rs` — `http` capability. Effects: `Network`. Port from `HttpTool`.
- `./src/capabilities/memory.rs` — `memory_read`, `memory_write`, `memory_search`, `memory_tree`. Effects: `ReadFilesystem`, `WriteFilesystem`. Port from `MemoryReadTool`/`MemoryWriteTool`.
- `./src/capabilities/messaging.rs` — `message`. Effects: `ExternalWrite`. Port from `MessageTool`.

Each module: `CapabilityDescriptor` + `execute` function + unit tests. Register in `mod.rs::register_all`.

**Verification**: `cargo test` passes for all new capability modules.

### [ ] Step: Create v2 capabilities — jobs, routines, skills, extensions

Implement the following capability modules:

- `./src/capabilities/jobs.rs` — `create_job`, `cancel_job`, `list_jobs`, `job_status`, `job_events`, `job_prompt`. Effects: `DispatchCapability`.
- `./src/capabilities/routines.rs` — `routine_create`, `routine_update`, `routine_delete`, `routine_list`, `routine_history`, `routine_fire`, `event_emit`. Effects: `DispatchCapability`.
- `./src/capabilities/skills.rs` — `skill_install`, `skill_remove`, `skill_list`, `skill_search`. Effects: `ModifyExtension`.
- `./src/capabilities/extensions.rs` — `tool_install`, `tool_remove`, `tool_list`, `tool_search`, `tool_upgrade`, `tool_auth`, `tool_info`, `extension_info`, `tool_permission_set`. Effects: `ModifyExtension`.

Each module: `CapabilityDescriptor` + `execute` function + unit tests. Register in `mod.rs::register_all`.

**Verification**: `cargo test` passes for all new capability modules.

### [ ] Step: Create v2 capabilities — secrets, images, system, pairing

Implement the remaining capability modules:

- `./src/capabilities/secrets.rs` — `secret_list`, `secret_delete`. Effects: `UseSecret`.
- `./src/capabilities/images.rs` — `image_generate`, `image_analyze`, `image_edit`. Effects: `Network`, `ExternalWrite`.
- `./src/capabilities/system.rs` — `echo`, `time`, `json`, `plan_update`, `restart`, `system_version`, `system_tools_list`. Effects: varies (mostly none).
- `./src/capabilities/pairing.rs` — `pairing_approve`. Effects: `ModifyApproval`.

Each module: `CapabilityDescriptor` + `execute` function + unit tests. Register in `mod.rs::register_all`.

**Verification**: `cargo test` passes for all new capability modules.

### [ ] Step: Rewrite EffectBridgeAdapter and bridge layer

Rewrite the bridge to use v2 capabilities instead of v1 `ToolRegistry`:

- `./src/bridge/effect_adapter.rs` — rewrite `EffectExecutor` impl to use `CapabilityHost` for dynamic dispatch. Each native capability registers its own `EffectExecutor` impl with `CapabilityHost` at startup (via `capabilities::register_all`), so `EffectBridgeAdapter` simply looks up the executor from `CapabilityHost` by capability ID and delegates. No hardcoded `match` block over 40+ capability IDs — both built-in and extension (WASM/MCP) capabilities use the same dynamic lookup path.
- `./src/bridge/router.rs` — remove `ToolRegistry` usage for action resolution, route through `CapabilityHost::resolve`.
- `./src/bridge/action_projector.rs` — remove v1 tool permission references, use `CapabilityLease` checks.
- `./src/bridge/tool_permissions.rs` — delete this file entirely; permissions handled by `CapabilityHost`.

**Verification**: `cargo build` succeeds. No imports from `./src/tools/` remain in `./src/bridge/`.

### [ ] Step: V2 permission storage and CapabilityHost extensions

Implement per-user permission persistence:

- Create a database migration (e.g. `./migrations/YYYYMMDD_capability_permissions.sql` or integrate into the settings DB startup/initialization routine) to add the `capability_permissions` table/collection (composite key: `tenant_id`, `capability_id`; columns: `permission_mode`, `updated_at`). Ensure the migration runs automatically on application startup.
- Extend `CapabilityHost` with dynamic registration API: `register()`, `unregister()`, `list_registered()`.
- Implement permission resolution: check `CapabilityPermissionOverride` for tenant → fall back to `CapabilityDescriptor::default_permission`.
- Add `RebornCapabilityInfo` DTO to `brassclaw_product_workflow`.
- Add `list_capabilities()` and `update_capability_permission()` methods to `RebornServicesApi` trait and implement them.
- Write tests for permission resolution order and CRUD operations.

**Verification**: `cargo test` passes. Permission overrides persist and resolve correctly.

### [ ] Step: Remove v1 agent integration and settings references

Remove all v1 tool references from the agent and settings layers:

- `./src/agent/agentic_loop.rs` — remove `ToolDispatcher` calls; ensure all paths use `EffectExecutor`.
- `./src/agent/thread_ops.rs` — remove `ToolRegistry` references and v1 message formatting helpers.
- `./src/agent/routine.rs` — remove `ToolRegistry` dependency.
- `./src/agent/scheduler.rs` — remove `SseManager` and `ToolRegistry` references.
- `./src/settings.rs` — remove `tool_permissions: HashMap<String, PermissionState>` field.
- `./src/app.rs` — remove `cleanup_ghost_seeded_tool_permissions`.
- `./src/tenant.rs` — remove `AdminToolPolicy` / `AdminToolPolicyCache` management.
- `./src/workspace/settings_schemas.rs` — remove v1 tool permission schema definitions.
- `./src/testing/mod.rs` — remove v1 test fixtures, replace with v2 capability test fixtures.

**Verification**: `cargo build` succeeds. `grep -r "ToolRegistry\|ToolDispatcher\|PermissionState\|AdminToolPolicy" ./src/agent/ ./src/settings.rs ./src/app.rs ./src/tenant.rs` returns zero matches.

### [ ] Step: Delete ./src/tools/ and ./src/channels/web/

Final removal of v1 directories:

- Delete `./src/tools/` entirely (all files: `tool.rs`, `registry.rs`, `dispatch.rs`, `execute.rs`, `permissions.rs`, `mod.rs`, `builtin/`, `coercion.rs`, `autonomy.rs`, `rate_limiter.rs`, `redaction.rs`, `runtime_filter.rs`, `schema_metrics.rs`, `schema_validator.rs`, `builder/`). WASM and MCP already extracted in Step 2.
- Delete `./src/channels/web/` entirely (`features/`, `handlers/`, `platform/`, `oauth/`, all supporting files).
- Update `./src/main.rs` — remove v1 channel mount, update module declarations.
- Remove `mod tools;` and `mod channels;` (or `mod web;`) from parent module files.

**Verification**: `cargo build` succeeds. `grep -r "ToolRegistry\|ToolDispatcher\|PermissionState\|EngineVersion::V1\|V1Only" ./src/ ./crates/` returns zero matches. The directories no longer exist.

### [ ] Step: WebUI v2 backend — tools API endpoints

Add capability management API to the v2 web UI:

- Create `./crates/brassclaw_webui_v2/src/handlers/tools.rs`:
  - `GET /api/webchat/v2/tools` — list all registered capabilities with current permission mode. Delegates to `RebornServicesApi::list_capabilities`.
  - `PUT /api/webchat/v2/tools/:capability_id/permission` — update a capability's permission mode. Delegates to `RebornServicesApi::update_capability_permission`.
- Register routes in `./crates/brassclaw_webui_v2/src/router.rs`.
- Write integration tests for both endpoints (list returns correct data, update persists).

**Verification**: `cargo test` passes. Endpoints respond correctly. Architecture boundary tests pass (no direct DB access from webui crate).

### [ ] Step: WebUI v2 frontend — tools settings tab

Build the frontend tools management UI (vanilla JS, no framework):

- Create `./crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tools-tab.js`:
  - Search/filter input
  - Tools list grouped by provider (collapsible provider groups)
  - Each tool row: name, description, effect kind badges (color-coded), permission mode selector (Allow/Ask/Deny)
  - Empty state when no tools registered
- Create `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/tools-manager.js`:
  - `ToolsManager` class: `init()`, `getTools()`, `filterTools(query)`, `groupByProvider()`, `updatePermission(id, mode)`
- Update `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-schema.js` — add `tools` entry to `SETTINGS_TABS`.
- Update `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js` — add `fetchTools()` and `updateToolPermission()` API functions.
- Add i18n keys: `settings.tools.title`, `settings.tools.search_placeholder`, `settings.tools.permission.allow/ask/deny`, `settings.tools.empty`, `settings.tools.effects`.

**Verification**: Tools tab appears in settings. Capabilities load dynamically from API. Permission toggles update via PUT and persist across page reloads.

### [ ] Step: WebUI v2 frontend — safety settings panel

Add a safety configuration panel to the tools settings tab that lets users view and customize:

- **Sensitive path blocking**: Display the current list of sensitive path patterns (e.g. `.env`, SSH keys, credential files). Allow users to add custom patterns or disable specific default patterns. Persist overrides via a new `PUT /api/webchat/v2/safety/sensitive-paths` endpoint backed by the settings DB.
- **Workspace file rules**: Display the list of workspace-protected files (e.g. `MEMORY.md`, `HEARTBEAT.md`, `IDENTITY.md`) that are redirected to `memory_write`. Allow users to add/remove entries. Persist via `PUT /api/webchat/v2/safety/workspace-rules`.
- **Device/process path blocking**: Display blocked device paths (e.g. `/dev/zero`, `/dev/urandom`, `/proc/kcore`). Allow users to add custom blocked paths or remove defaults. Persist via `PUT /api/webchat/v2/safety/blocked-paths`.

Implementation:

- Create `./crates/brassclaw_webui_v2/src/handlers/safety.rs` with GET/PUT endpoints for each category. Register routes in the v2 router.
- Add `SafetyConfig` storage (sensitive paths, workspace rules, blocked paths) to the settings DB with per-tenant scoping.
- Update `./src/capabilities/filesystem.rs` to load safety overrides from the DB at execution time instead of using only hardcoded lists.
- Create `./crates/brassclaw_webui_v2_static/static/js/pages/settings/components/safety-panel.js`:
  - Three collapsible sections (sensitive paths, workspace rules, blocked paths)
  - Each section: list of current entries with toggle/remove controls, input to add new entries
  - Visual distinction between default (system) entries and user-added entries
- Update `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js` — add safety config fetch/update API functions.
- Add i18n keys: `settings.safety.title`, `settings.safety.sensitive_paths.*`, `settings.safety.workspace_rules.*`, `settings.safety.blocked_paths.*`.

**Verification**: Safety panel appears in the tools settings tab. Default entries display correctly. User can add/remove/toggle entries. Changes persist across page reloads. Filesystem capabilities respect the overrides at runtime.

### [ ] Step: Final verification and cleanup

End-to-end verification of the complete migration:

- `cargo build` — zero errors, clean compilation.
- `cargo test` — all tests pass.
- `cargo clippy -- -D warnings` — no warnings.
- `grep -r "ToolRegistry\|ToolDispatcher\|PermissionState\|EngineVersion::V1\|V1Only" ./src/ ./crates/` — zero matches.
- `grep -r "stub\|TODO\|FIXME\|always.allow\|Made with Bob" ./src/ ./crates/` — zero migration-related matches.
- Verify `./src/tools/` and `./src/channels/web/` directories do not exist.
- Verify frontend tools tab renders, lists capabilities, and permission changes persist.
