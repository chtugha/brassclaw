# V2 Integration Status Report

## Current State Summary

### Completed Work

1. **V2 Capability Modules (Steps 1-6)**: All 47 execute functions across 13 domain modules have been created:
   - `filesystem.rs` - 7 capabilities (read_file, write_file, list_dir, apply_patch, glob, grep, file_undo)
   - `shell.rs` - 1 capability (shell)
   - `network.rs` - 1 capability (http)
   - `memory.rs` - 4 capabilities (memory_read, memory_write, memory_search, memory_tree)
   - `messaging.rs` - 1 capability (message)
   - `jobs.rs` - 6 capabilities (create_job, cancel_job, list_jobs, job_status, job_events, job_prompt)
   - `routines.rs` - 7 capabilities (routine_create, routine_update, routine_delete, routine_list, routine_history, routine_fire, event_emit)
   - `skills.rs` - 4 capabilities (skill_install, skill_remove, skill_list, skill_search)
   - `extensions.rs` - 9 capabilities (tool_install, tool_remove, tool_list, tool_search, tool_upgrade, tool_auth, tool_info, extension_info, tool_permission_set)
   - `secrets.rs` - 2 capabilities (secret_list, secret_delete)
   - `images.rs` - 3 capabilities (image_generate, image_analyze, image_edit)
   - `system.rs` - 7 capabilities (echo, time, json, plan_update, restart, system_version, system_tools_list)
   - `pairing.rs` - 1 capability (pairing_approve)

2. **V2 Infrastructure (Steps 7-8)**:
   - `EffectBridgeAdapter` V2 implementation in `src/bridge/effect_adapter_v2.rs`
   - `BuiltinCapabilityDispatcher` in `src/capabilities/dispatcher.rs`
   - Permission storage system with database integration
   - All 13 context structs for capability execution

3. **Partial Agent Cleanup**:
   - `self_repair.rs` cleaned (ToolRegistry removed)
   - `agent_loop.rs` cleaned (AgentDeps.tools removed, tools added as Agent::new() parameter)
   - `RoutinesContext` circular dependency broken (engine field is now `Arc<RwLock<Option<Arc<RoutineEngine>>>>`)

### Critical Discovery: V2 System Never Instantiated

**The V2 infrastructure exists but is completely unused.** The system still runs on V1:

- `AppComponents.tools: Arc<ToolRegistry>` (line 46 of app.rs)
- `init_tools()` creates V1 `ToolRegistry` (line 525 of app.rs)
- `Agent::new()` takes `tools: Arc<ToolRegistry>` parameter
- `Scheduler` stores and uses `ToolRegistry`
- `RoutineEngine` stores and uses `ToolRegistry`
- All tool execution goes through V1 `ToolDispatcher`
- `bridge/router.rs` uses V1 adapter (line 1751)

The V2 `EffectBridgeAdapter` in `effect_adapter_v2.rs` properly wraps `CapabilityHost`, but it's never instantiated or used.

## Path Forward: Full V2 Integration

### Architecture Overview

The V2 system requires these components working together:

```
AppBuilder.build_all()
  ├─> Create 13 Context Objects (from AppComponents data)
  ├─> Create BuiltinCapabilityDispatcher (with all 13 contexts)
  ├─> Create CapabilityHost (with dispatcher + extension_registry)
  ├─> Create V2 EffectBridgeAdapter (wrapping CapabilityHost)
  └─> Replace AppComponents.tools with effect_executor
```

Then throughout the system:
- `Agent::new()` accepts `Arc<dyn EffectExecutor>` instead of `Arc<ToolRegistry>`
- `Scheduler` uses `EffectExecutor` instead of `ToolRegistry`
- `RoutineEngine` uses `EffectExecutor` instead of `ToolRegistry`
- `bridge/router.rs` uses V2 adapter instead of V1

### 13 Context Objects Required

Each context needs specific dependencies from AppComponents:

1. **FilesystemContext**
   - `base_dir: PathBuf`
   - `state: Arc<dyn FilesystemState>`

2. **ShellContext**
   - `working_dir: PathBuf`
   - `timeout: Duration`
   - `sandbox: SandboxConfig`
   - `allowed_commands: Vec<String>`

3. **NetworkContext**
   - `credential_registry: Arc<SharedCredentialRegistry>`
   - `secrets_store: Arc<dyn SecretsStore>`
   - `user_id: String`
   - `http_interceptor: Option<Arc<dyn HttpInterceptor>>`

4. **MemoryContext**
   - `resolver: Arc<dyn WorkspaceResolver>`
   - `user_id: String`
   - `timezone: String`
   - `llm: Option<Arc<dyn LlmProvider>>`

5. **MessagingContext**
   - `channel_manager: Arc<dyn ChannelManager>`
   - `extension_manager: Arc<ExtensionManager>`
   - `base_dir: PathBuf`
   - `user_id: String`

6. **JobsContext**
   - `context_manager: Arc<ContextManager>`
   - `scheduler_slot: Arc<RwLock<Option<Arc<Scheduler>>>>`
   - `job_manager: Arc<JobManager>`
   - `store: Arc<dyn JobStore>`
   - `user_id: String`
   - `llm: Arc<dyn LlmProvider>`

7. **RoutinesContext**
   - `store: Arc<dyn RoutineStore>`
   - `engine: Arc<RwLock<Option<Arc<RoutineEngine>>>>`  ← Starts as None!
   - `user_id: String`

8. **SkillsContext**
   - `registry: Arc<RwLock<SkillRegistry>>`
   - `catalog: Arc<SkillCatalog>`

9. **ExtensionsContext**
   - `manager: Arc<ExtensionManager>`
   - `user_id: String`

10. **SecretsContext**
    - `store: Arc<dyn SecretsStore>`
    - `user_id: String`

11. **ImagesContext**
    - `api_base_url: String`
    - `api_key: String`
    - `models: Vec<String>`
    - `client: Arc<dyn HttpClient>`
    - `base_dir: PathBuf`

12. **SystemContext**
    - `event_publisher: Arc<dyn EventPublisher>`
    - `tool_output_stash: Arc<ToolOutputStash>`
    - `timezone: String`
    - `version: String`

13. **PairingContext**
    - `store: Arc<dyn PairingStore>`
    - `user_id: String`

### Circular Dependency Solution

**Problem**: RoutinesContext needs RoutineEngine, but RoutineEngine needs EffectExecutor which comes from CapabilityHost which needs RoutinesContext.

**Solution** (Already Implemented):
- RoutinesContext.engine is `Arc<RwLock<Option<Arc<RoutineEngine>>>>`
- Starts as `None` during V2 system creation
- Gets filled in later when RoutineEngine is created
- All access points check for `Some(engine)` before using

This was implemented in the previous session by modifying:
- `src/capabilities/routines.rs` line 60: Changed engine field type
- Lines 858, 977, 1009: Updated engine access to handle Option

### Implementation Steps

#### Step 1: Create V2 System in AppBuilder.build_all()

Location: After line 1169 in `src/app.rs` (after `init_tools` returns)

```rust
// Create V2 capability system
let filesystem_ctx = Arc::new(FilesystemContext::new(/* ... */));
let shell_ctx = Arc::new(ShellContext::new(/* ... */));
// ... create all 13 contexts ...

let builtin_dispatcher = Arc::new(BuiltinCapabilityDispatcher::new(
    filesystem_ctx,
    shell_ctx,
    // ... all 13 contexts
));

let capability_host = CapabilityHost::new(
    &extension_registry,
    &*builtin_dispatcher,
    &authorizer,
)
.with_run_state(&*run_state_store)
.with_approval_requests(&*approval_store)
.with_capability_leases(&*lease_store)
.with_process_manager(&*process_manager)
.with_obligation_handler(&*obligation_handler);

let effect_executor: Arc<dyn EffectExecutor> = Arc::new(
    EffectBridgeAdapter::new(capability_host)
);
```

#### Step 2: Replace AppComponents.tools

Change `AppComponents` struct (line 46):
```rust
// OLD:
pub tools: Arc<ToolRegistry>,

// NEW:
pub effect_executor: Arc<dyn EffectExecutor>,
```

#### Step 3: Update Agent::new() Signature

Change `Agent::new()` in `src/agent/agent_loop.rs`:
```rust
// OLD:
pub fn new(
    tools: Arc<ToolRegistry>,
    // ...
) -> Self

// NEW:
pub fn new(
    effect_executor: Arc<dyn EffectExecutor>,
    // ...
) -> Self
```

#### Step 4: Update Scheduler

Change `Scheduler` in `src/agent/scheduler.rs`:
```rust
// OLD:
tools: Arc<ToolRegistry>,

// NEW:
effect_executor: Arc<dyn EffectExecutor>,
```

#### Step 5: Update RoutineEngine

Change `RoutineEngine` in `src/agent/routine_engine.rs`:
```rust
// OLD:
tools: Arc<ToolRegistry>,

// NEW:
effect_executor: Arc<dyn EffectExecutor>,
```

#### Step 6: Update bridge/router.rs

Switch from V1 to V2 adapter (line 1751):
```rust
// OLD:
let adapter = EffectBridgeAdapter::new(tools);

// NEW:
let adapter = effect_executor; // Already V2
```

#### Step 7: Fill RoutineEngine Slot

After RoutineEngine is created, fill in the RoutinesContext slot:
```rust
if let Some(engine) = routine_engine {
    *routines_ctx.engine.write().await = Some(Arc::clone(&engine));
}
```

#### Step 8: Fix Test Files

Update 15+ test files to use mock `EffectExecutor` instead of mock `ToolRegistry`.

### Files Requiring Changes

**Core System**:
- `src/app.rs` - Create V2 system, update AppComponents
- `src/agent/agent_loop.rs` - Update Agent::new() signature
- `src/agent/scheduler.rs` - Replace ToolRegistry with EffectExecutor
- `src/agent/routine_engine.rs` - Replace ToolRegistry with EffectExecutor
- `src/bridge/router.rs` - Use V2 adapter
- `src/main.rs` - Update Agent::new() call

**Step 9 Cleanup** (Remove V1 References):
- `src/agent/dispatcher.rs` - Remove execute_chat_tool_standalone
- `src/agent/commands.rs` - Remove test ToolRegistry usage
- `src/settings.rs` - Remove tool_permissions field
- `src/app.rs` - Remove cleanup_ghost_seeded_tool_permissions
- `src/tenant.rs` - Remove AdminToolPolicy management
- `src/workspace/settings_schemas.rs` - Remove v1 tool schemas

**Test Files** (15+ files):
- All files with mock ToolRegistry need mock EffectExecutor

### Estimated Effort

- **V2 System Creation**: 4-6 hours (creating 13 contexts, wiring dependencies)
- **Agent/Scheduler/RoutineEngine Updates**: 3-4 hours
- **Test File Fixes**: 4-6 hours (15+ files)
- **Step 9 Cleanup**: 2-3 hours
- **Verification & Debugging**: 3-5 hours

**Total**: 16-24 hours of focused work

### Success Criteria

1. `cargo build` succeeds with zero errors
2. `cargo test` passes all tests
3. No references to `ToolRegistry`, `ToolDispatcher`, or `PermissionState` in production code
4. V2 `EffectBridgeAdapter` is instantiated and used throughout the system
5. All 47 capabilities execute through V2 infrastructure

## Next Immediate Action

Start implementing Step 1: Create the V2 system in `AppBuilder.build_all()` by:
1. Reading the full `init_tools` method to understand all available data
2. Creating the 13 context objects using data from AppComponents
3. Instantiating BuiltinCapabilityDispatcher
4. Creating CapabilityHost
5. Creating V2 EffectBridgeAdapter
6. Replacing the return value to include effect_executor instead of tools

This is the foundation that enables all subsequent changes.