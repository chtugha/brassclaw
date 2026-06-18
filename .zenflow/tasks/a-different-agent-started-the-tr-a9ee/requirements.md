# PRD: Complete V1-to-V2 Tool Architecture Migration

> **Source code location**: `/Volumes/SSDE/brassclaw`. All relative paths below (e.g. `./src/`, `./crates/`) are relative to this root.

## Problem Statement

BrassClaw's tool system is split between two incompatible architectures:

- **V1 (legacy)**: Tools are registered via `ToolRegistry` in `./src/tools/`, implement the `Tool` trait, are dispatched through `ToolDispatcher`, and were governed by a per-user `PermissionState` system (now stubbed to always-allow). The v1 web channel (`./src/channels/web/`) exposes settings and tool policy handlers for this system.

- **V2 (Reborn)**: The engine (`brassclaw_engine`) uses `CapabilityDescriptor`, `CapabilityHost`, `EffectExecutor`, and `CapabilityLease` abstractions. Tools are capabilities with `PermissionMode` (Allow/Ask/Deny). The WebUI v2 frontend and `brassclaw_webui_v2` crate handle the user-facing surface.

A previous agent attempted a partial migration by:
1. Stubbing `./src/tools/permissions.rs` and `./src/bridge/tool_permissions.rs` to always-allow
2. Removing the v1 "Tools" settings tab from the WebUI v2 frontend
3. Removing v2 tool permission route stubs from `./crates/brassclaw_webui_v2`
4. Leaving all v1 tool implementations, registry, dispatch, and execution infrastructure intact

The result is a broken hybrid: the v1 permission system is bypassed (no authorization at all), the v2 frontend has no tool management UI, and the entire `./src/tools/` directory still contains ~60+ files of v1-only infrastructure.

## Goals

1. **Eliminate the v1 tool architecture entirely** -- remove `./src/tools/` and all code that depends on it
2. **Rewrite every built-in tool as a v2 engine-native capability** using `CapabilityDescriptor`, `EffectExecutor`, and the Reborn capability model
3. **Build a new "Tools" tab in the WebUI v2 settings page** that dynamically lists all registered capabilities and lets users toggle their `PermissionMode` (Allow/Ask/Deny)
4. **Remove the v1 web channel** (`./src/channels/web/`) since it is fully replaced by WebUI v2

## Non-Goals

- Changing the v2 engine execution model (Thread/Step/Capability primitives stay as-is)
- Modifying the WASM sandbox runtime or MCP protocol layer (these are tool-agnostic transport layers that may need minor interface changes but not architectural rewrites)
- Migrating user data from v1 permission settings to v2 (v1 permissions are already stubbed/bypassed)

## Scope

### 1. Remove V1 Tool Architecture

Remove the entire `./src/tools/` directory and all v1 remnants:

- **`./src/tools/tool.rs`** -- `Tool` trait, `ToolSchema`, `ToolOutput`, `ToolError`, `ApprovalRequirement`, `ApprovalContext`, `EngineCompatibility`, `EngineVersion`, `RiskLevel`, `ToolRateLimitConfig`, `ToolRuntimeAffordance`, `ToolDomain`, `ToolDiscoverySummary`
- **`./src/tools/registry.rs`** -- `ToolRegistry`, tool registration, alias resolution, protected-name enforcement
- **`./src/tools/dispatch.rs`** -- `ToolDispatcher`, v1 dispatch pipeline (safety, validation, redaction, timeout, audit)
- **`./src/tools/execute.rs`** -- `execute_tool_with_safety`, `process_tool_result`
- **`./src/tools/permissions.rs`** -- Stubbed `PermissionState`, `AdminToolPolicy`, `AdminToolPolicyCache`
- **`./src/tools/builtin/`** -- All 30+ built-in tool implementations (`ShellTool`, `ReadFileTool`, `WriteFileTool`, `HttpTool`, `GrepTool`, `GlobTool`, `MemoryReadTool`, `MemoryWriteTool`, `CreateJobTool`, `RoutineCreateTool`, `SkillInstallTool`, `MessageTool`, `TimeTool`, `EchoTool`, `JsonTool`, `ImageGenerateTool`, `ImageAnalyzeTool`, `ImageEditTool`, `PlanUpdateTool`, `RestartTool`, `ToolInstallTool`, `ToolListTool`, `ToolAuthTool`, `ToolRemoveTool`, `ToolSearchTool`, `ToolUpgradeTool`, `ToolInfoTool`, `ToolPermissionSetTool`, `SecretListTool`, `SecretDeleteTool`, `PairingApproveTool`, etc.)
- **`./src/tools/wasm/`** -- WASM tool loader, wrapper, runtime, storage, capabilities schema, credential injector, security checks
- **`./src/tools/mcp/`** -- MCP client, transport, session, auth, factory, config
- **`./src/tools/mod.rs`** -- Module root and re-exports
- **Supporting modules**: `coercion.rs`, `autonomy.rs`, `rate_limiter.rs`, `redaction.rs`, `runtime_filter.rs`, `schema_metrics.rs`, `schema_validator.rs`, `builder/`

All call sites across the codebase that reference `./src/tools/` types must be updated or removed. Key dependents include:
- `./src/bridge/effect_adapter.rs` (implements `EffectExecutor` by wrapping `ToolRegistry`)
- `./src/bridge/router.rs` (uses `ToolRegistry` for action resolution)
- `./src/bridge/tool_permissions.rs` (stub referencing `PermissionState`)
- `./src/bridge/action_projector.rs` (references tool permissions)
- `./src/agent/` (dispatcher, thread_ops, agentic_loop, routine, scheduler)
- `./src/settings.rs` (`tool_permissions` HashMap field)
- `./src/app.rs` (`cleanup_ghost_seeded_tool_permissions`)
- `./src/tenant.rs` (admin policy management)
- `./src/workspace/settings_schemas.rs`
- `./src/channels/web/` (settings handlers, types, platform router)
- `./src/testing/mod.rs`

### 2. Rewrite Built-in Tools as V2 Capabilities

Each v1 built-in tool must be reimplemented as a v2 capability that:
- Declares a `CapabilityDescriptor` with `id`, `provider`, `runtime`, `trust_ceiling`, `description`, `parameters_schema`, `effects` (Vec<EffectKind>), and `default_permission` (PermissionMode)
- Executes through the `EffectExecutor` trait (`execute_action` method)
- Returns `ActionResult` instead of `ToolOutput`
- Uses `CapabilityLease` for authorization context instead of v1 `ApprovalRequirement`

The built-in tools to rewrite (grouped by domain):

**Filesystem**:
- `read_file`, `write_file`, `list_dir`, `apply_patch`, `glob`, `grep`
- Effects: `ReadFilesystem`, `WriteFilesystem`

**Shell & Code Execution**:
- `shell`
- Effects: `ExecuteCode`, `SpawnProcess`

**Network & HTTP**:
- `http`
- Effects: `Network`

**Memory & Knowledge**:
- `memory_read`, `memory_write`, `memory_search`, `memory_tree`
- Effects: `ReadFilesystem`, `WriteFilesystem`

**Messaging**:
- `message` (send messages to channels)
- Effects: `ExternalWrite`

**Jobs & Routines**:
- `create_job`, `cancel_job`, `list_jobs`, `job_status`, `job_events`, `job_prompt`
- `routine_create`, `routine_update`, `routine_delete`, `routine_list`, `routine_history`, `routine_fire`, `event_emit`
- Effects: `DispatchCapability`

**Skills**:
- `skill_install`, `skill_remove`, `skill_list`, `skill_search`
- Effects: `ModifyExtension`

**Extensions/Tools Management**:
- `tool_install`, `tool_remove`, `tool_list`, `tool_search`, `tool_upgrade`, `tool_auth`, `tool_info`, `extension_info`, `tool_permission_set`
- Effects: `ModifyExtension`

**Secrets**:
- `secret_list`, `secret_delete`
- Effects: `UseSecret`

**Images**:
- `image_generate`, `image_analyze`, `image_edit`
- Effects: `Network`, `ExternalWrite`

**System & Utility**:
- `echo`, `time`, `json`, `plan_update`, `restart`, `system_version`, `system_tools_list`
- Effects: varies (mostly none)

**Pairing**:
- `pairing_approve`
- Effects: `ModifyApproval`

**File History**:
- `file_undo`
- Effects: `WriteFilesystem`

### 3. New "Tools" Tab in WebUI V2 Settings

Build a new settings tab that dynamically displays and manages tool/capability permissions:

**Data Source**: The tab must fetch the list of registered capabilities from a new v2 API endpoint. Capabilities are dynamic -- extensions can add/remove them at runtime.

**Display Requirements**:
- Each capability shown with: name, description, provider/extension, current `PermissionMode`, list of `EffectKind` tags
- Group by provider/extension or category
- Search/filter by name

**User Actions**:
- Toggle `PermissionMode` per capability: `Allow` (no approval needed), `Ask` (approval gate fires on each invocation), `Deny` (capability blocked)
- Changes persist and take effect on the next invocation

**Boundary and Architectural Requirements**:
- **Architecture Boundary Adherence**: To satisfy the strict crate dependency boundaries enforced by tests, handlers in the `./crates/brassclaw_webui_v2` crate must **not** directly query the capability database or interact with low-level storage. All retrieval and modification operations must be routed through `./crates/brassclaw_product_workflow::RebornServicesApi`.
- **API Extension**: Add the following capability management methods to the `RebornServicesApi` trait to decouple the UI from engine/storage internals:
  - `list_capabilities(...) -> Result<Vec<RebornCapabilityInfo>, RebornServicesError>`
  - `update_capability_permission(...) -> Result<(), RebornServicesError>`

**API Requirements**:
New endpoints in `./crates/brassclaw_webui_v2`:
- `GET /api/webchat/v2/tools` -- list all registered capabilities with their current permission mode (delegates to `RebornServicesApi::list_capabilities`)
- `PUT /api/webchat/v2/tools/{capability_id}/permission` -- update a capability's permission mode (delegates to `RebornServicesApi::update_capability_permission`)

**Frontend Implementation**:
- New `./crates/brassclaw_webui_v2_static/static/js/pages/settings/components/tools-tab.js` component
- New `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/tools-manager.js` state manager (vanilla JS, no framework)
- Add "Tools" entry to `SETTINGS_TABS` in `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-schema.js`
- Add API functions to `./crates/brassclaw_webui_v2_static/static/js/pages/settings/lib/settings-api.js`
- Add i18n keys for the tools tab

### 4. Remove V1 Web Channel

Remove the legacy `./src/channels/web/` directory and all of its components since WebUI v2 fully replaces it.

**Cross-cutting Legacy Dependency Relocation**:
Before deleting `./src/channels/web/`, all core systems that reference elements within it must be decoupled to prevent compiler breakages:
1. **Shared Data Structures**: Shared types (such as `ChannelOnboardingState`, `ChannelOnboardingInfo`, and any onboarding-related configurations) currently imported by `./src/extensions/mod.rs` must be relocated to `./crates/brassclaw_common` or `./crates/brassclaw_host_api`.
2. **Global Tracing/Logging**: The tracing initialization function (`init_tracing`) located in `./src/channels/web/log_layer.rs` and consumed by `./src/main.rs` must be relocated to `./src/logging.rs` or directly into the main runtime.
3. **Database Turn Builders**: Message formatting helpers like `build_turns_from_db_messages` used in `./src/agent/thread_ops.rs` must be moved to a shared library utility or test helper module.
4. **SSE Manager Refactoring**: The legacy `SseManager` within `./src/channels/web` must be cleanly separated or deprecated from `./src/agent/scheduler.rs` and `./src/worker/job.rs`, routing events via the native v2 event stream mechanism instead.

**Deleted Legacy Elements**:
- `./src/channels/web/features/` -- settings, chat, debug, extensions, jobs, logs, oauth, pairing, routines, status handlers
- `./src/channels/web/handlers/` -- auth, engine, frontend, llm, memory, secrets, skills, system_prompt, tokens, traces, users, webhooks
- `./src/channels/web/platform/` -- router, auth, sse, ws, state, static_files, engine_dispatch, legacy_auth
- `./src/channels/web/oauth/` -- OAuth providers, state store
- Supporting files: `types.rs`, `util.rs`, `openai_compat.rs`, `responses_api.rs`, `onboarding.rs`, `log_layer.rs`, `test_helpers.rs`, `mod.rs`

All v1 API routes (`/api/settings/*`, `/api/chat/*`, `/api/tools/*`, `/api/skills/*`, `/api/users/*`, etc.) are removed in favor of the v2 routes (`/api/webchat/v2/*`).

## Assumptions

1. The WASM sandbox runtime (`./src/tools/wasm/`) and MCP protocol layer (`./src/tools/mcp/`) contain reusable transport logic that should be extracted into standalone crates or into the bridge layer rather than deleted outright -- the protocol/transport code is tool-agnostic.
2. The `registry/tools/*.json` files define extension metadata and will continue to work with the v2 extension system (`brassclaw_extensions`).
3. The v2 engine's `EffectExecutor` trait is the correct integration point for built-in tool implementations.
4. The bridge's `EffectBridgeAdapter` will be rewritten to call v2 capability implementations directly instead of wrapping `ToolRegistry`.
5. Test coverage in `tests/` that depends on v1 tool types will need to be rewritten against v2 capability interfaces.

## Success Criteria

1. The `./src/tools/` directory is completely removed -- no v1 `Tool` trait, `ToolRegistry`, `ToolDispatcher`, or `PermissionState` types exist in the codebase
2. The `./src/channels/web/` directory is completely removed
3. All built-in tools function correctly as v2 capabilities through the engine
4. The WebUI v2 settings page has a working "Tools" tab that dynamically shows all registered capabilities with toggleable `PermissionMode`
5. The project compiles cleanly (`cargo build`) and all tests pass (`cargo test`)
6. No "Made with Bob" markers or stub comments remain
7. No references to `EngineVersion::V1`, `EngineCompatibility::V1Only`, or `v1` tool patterns remain in production code
