# V2 System Implementation Plan

## Overview

This document provides a step-by-step implementation plan for creating and integrating the V2 capability system into BrassClaw. The V2 infrastructure exists but is never instantiated - the system still runs entirely on V1.

## Prerequisites

All V2 capability modules are complete:
- 47 execute functions across 13 domain modules
- `BuiltinCapabilityDispatcher` in `src/capabilities/dispatcher.rs`
- `EffectBridgeAdapter` V2 in `src/bridge/effect_adapter_v2.rs`
- Permission storage with database integration
- Circular dependency in `RoutinesContext` resolved (engine field is `Arc<RwLock<Option<Arc<RoutineEngine>>>>`)

## Phase 1: Create V2 System in AppBuilder.build_all()

### Location
File: `/Volumes/SSDE/brassclaw/src/app.rs`
After line 1169 (after `init_tools` returns)

### Step 1.1: Create FilesystemContext

```rust
use crate::capabilities::filesystem::{FilesystemContext, FilesystemCapabilityState};

// After line 1169, before extension initialization
let filesystem_state = Arc::new(FilesystemCapabilityState::default());
let filesystem_ctx = Arc::new(FilesystemContext {
    base_dir: self.config.workspace.base_dir.clone(),
    state: filesystem_state,
});
```

**Data Sources:**
- `base_dir`: `self.config.workspace.base_dir`
- `state`: New `FilesystemCapabilityState::default()`

### Step 1.2: Create ShellContext

```rust
use crate::capabilities::shell::ShellContext;

let shell_ctx = Arc::new(ShellContext {
    working_dir: self.config.workspace.base_dir.clone(),
    timeout: Duration::from_secs(self.config.agent.shell_timeout_secs.unwrap_or(120)),
    sandbox_enabled: self.config.sandbox.enabled,
    sandbox_policy: if self.config.sandbox.enabled {
        Some(self.config.sandbox.policy.clone())
    } else {
        None
    },
    allowed_commands: self.config.agent.allowed_shell_commands.clone(),
});
```

**Data Sources:**
- `working_dir`: `self.config.workspace.base_dir`
- `timeout`: `self.config.agent.shell_timeout_secs` (default 120)
- `sandbox_enabled`: `self.config.sandbox.enabled`
- `sandbox_policy`: `self.config.sandbox.policy`
- `allowed_commands`: `self.config.agent.allowed_shell_commands`

### Step 1.3: Create NetworkContext

```rust
use crate::capabilities::network::NetworkContext;

let network_ctx = Arc::new(NetworkContext {
    credential_registry: Some(Arc::clone(&credential_registry)),
    secrets_store: self.secrets_store.clone(),
    role_lookup: self.db.clone().map(|db| db as Arc<dyn UserStore>),
    user_id: self.config.owner_id.clone(),
    http_interceptor: http_interceptor.clone(),
});
```

**Data Sources:**
- `credential_registry`: From `init_tools` return value
- `secrets_store`: `self.secrets_store`
- `role_lookup`: `self.db` (cast to `UserStore`)
- `user_id`: `self.config.owner_id`
- `http_interceptor`: From `init_tools` return value

### Step 1.4: Create MemoryContext

```rust
use crate::capabilities::memory::MemoryContext;

let memory_ctx = Arc::new(MemoryContext {
    resolver: workspace_resolver.clone().expect("workspace resolver required for memory tools"),
    user_id: self.config.owner_id.clone(),
    user_timezone: self.config.timezone.clone().unwrap_or_else(|| "UTC".to_string()),
    llm: cheap_llm.clone().or_else(|| Some(Arc::clone(&llm))),
    reasoning_enabled: self.config.search.reasoning_enabled,
});
```

**Data Sources:**
- `resolver`: From `init_tools` return value (`workspace_resolver`)
- `user_id`: `self.config.owner_id`
- `user_timezone`: `self.config.timezone` (default "UTC")
- `llm`: `cheap_llm` or `llm`
- `reasoning_enabled`: `self.config.search.reasoning_enabled`

### Step 1.5: Create MessagingContext

```rust
use crate::capabilities::messaging::MessagingContext;
use std::sync::RwLock;

let messaging_ctx = Arc::new(MessagingContext {
    channel_manager: Arc::new(ChannelManager::new()), // TODO: Get from app state
    extension_manager: extension_manager.clone(),
    default_channel: Arc::new(RwLock::new(None)),
    default_target: Arc::new(RwLock::new(None)),
    base_dir: self.config.workspace.base_dir.clone(),
    user_id: self.config.owner_id.clone(),
    metadata: serde_json::json!({}),
});
```

**Data Sources:**
- `channel_manager`: Need to create or get from app state
- `extension_manager`: From `init_extensions` return value
- `default_channel`: New `RwLock<Option<String>>`
- `default_target`: New `RwLock<Option<String>>`
- `base_dir`: `self.config.workspace.base_dir`
- `user_id`: `self.config.owner_id`
- `metadata`: Empty JSON object

**NOTE**: ChannelManager needs to be created or passed from somewhere. Check if it exists in app state.

### Step 1.6: Create JobsContext

```rust
use crate::capabilities::jobs::{JobsContext, SchedulerSlot, PromptQueue};

let jobs_ctx = Arc::new(JobsContext {
    context_manager: Arc::clone(&context_manager),
    scheduler_slot: None, // Will be filled later when Scheduler is created
    job_manager: None, // TODO: Get ContainerJobManager if available
    store: self.db.clone(),
    event_tx: None, // TODO: Get event broadcaster
    inject_tx: None, // TODO: Get message injector
    secrets_store: self.secrets_store.clone(),
    prompt_queue: None, // TODO: Create or get prompt queue
    user_id: self.config.owner_id.clone(),
    metadata: serde_json::json!({}),
});
```

**Data Sources:**
- `context_manager`: Created in `build_all` (line 1331)
- `scheduler_slot`: None initially (filled later)
- `job_manager`: Need to find ContainerJobManager
- `store`: `self.db`
- `event_tx`: Need to find event broadcaster
- `inject_tx`: Need to find message injector
- `secrets_store`: `self.secrets_store`
- `prompt_queue`: Need to create or find
- `user_id`: `self.config.owner_id`
- `metadata`: Empty JSON object

**NOTE**: Several optional fields need investigation to find proper sources.

### Step 1.7: Create RoutinesContext

```rust
use crate::capabilities::routines::RoutinesContext;

let routines_ctx = Arc::new(RoutinesContext {
    store: self.db.clone().expect("database required for routines"),
    engine: Arc::new(tokio::sync::RwLock::new(None)), // Filled later when RoutineEngine is created
    user_id: self.config.owner_id.clone(),
});
```

**Data Sources:**
- `store`: `self.db` (cast to `RoutineStore`)
- `engine`: New `RwLock<Option<Arc<RoutineEngine>>>` starting as None
- `user_id`: `self.config.owner_id`

**CRITICAL**: This is the circular dependency solution. Engine starts as None and gets filled in later.

### Step 1.8: Create SkillsContext

```rust
use crate::capabilities::skills::SkillsContext;

let skills_ctx = Arc::new(SkillsContext {
    registry: skill_registry.clone().expect("skill registry required"),
    catalog: skill_catalog.clone().expect("skill catalog required"),
});
```

**Data Sources:**
- `registry`: From `build_all` (line 1323)
- `catalog`: From `build_all` (line 1324)

### Step 1.9: Create ExtensionsContext

```rust
use crate::capabilities::extensions::ExtensionsContext;

let extensions_ctx = Arc::new(ExtensionsContext {
    manager: extension_manager.clone().expect("extension manager required"),
    user_id: self.config.owner_id.clone(),
});
```

**Data Sources:**
- `manager`: From `init_extensions` return value
- `user_id`: `self.config.owner_id`

### Step 1.10: Create SecretsContext

```rust
use crate::capabilities::secrets::SecretsContext;

let secrets_ctx = Arc::new(SecretsContext {
    store: self.secrets_store.clone().expect("secrets store required"),
    user_id: self.config.owner_id.clone(),
});
```

**Data Sources:**
- `store`: `self.secrets_store`
- `user_id`: `self.config.owner_id`

### Step 1.11: Create ImagesContext

```rust
use crate::capabilities::images::ImagesContext;
use secrecy::SecretString;

let (api_base, api_key_opt) = if let Some(ref provider) = self.config.llm.provider {
    (
        provider.base_url.clone(),
        provider.api_key.clone(),
    )
} else {
    (
        self.config.llm.nearai.base_url.clone(),
        self.config.llm.nearai.api_key.clone(),
    )
};

let images_ctx = if let Some(api_key) = api_key_opt {
    let model_name = self.config.llm.provider
        .as_ref()
        .map(|p| p.model.clone())
        .unwrap_or_else(|| self.config.llm.nearai.model.clone());
    let models = vec![model_name.clone()];
    let gen_model = brassclaw_llm::image_models::suggest_image_model(&models)
        .unwrap_or("black-forest-labs/FLUX.2-klein-4B")
        .to_string();
    let vision_model = brassclaw_llm::vision_models::suggest_vision_model(&models)
        .unwrap_or(&model_name)
        .to_string();
    
    Some(Arc::new(ImagesContext {
        api_base_url: api_base,
        api_key,
        gen_model,
        vision_model,
        client: reqwest::Client::new(),
        base_dir: workspace.as_ref().map(|ws| ws.base_dir().to_path_buf()),
    }))
} else {
    None
};
```

**Data Sources:**
- `api_base_url`: `self.config.llm.provider.base_url` or `self.config.llm.nearai.base_url`
- `api_key`: `self.config.llm.provider.api_key` or `self.config.llm.nearai.api_key`
- `gen_model`: Suggested from model list
- `vision_model`: Suggested from model list
- `client`: New `reqwest::Client`
- `base_dir`: From workspace if available

**NOTE**: ImagesContext is optional - only create if API key is available.

### Step 1.12: Create SystemContext

```rust
use crate::capabilities::system::SystemContext;

let system_ctx = Arc::new(SystemContext {
    event_publisher: None, // TODO: Get event publisher
    tool_output_stash: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
    user_timezone: self.config.timezone.clone().unwrap_or_else(|| "UTC".to_string()),
    conversation_id: None, // Set per-conversation
    registered_capability_names: Vec::new(), // Filled after registration
});
```

**Data Sources:**
- `event_publisher`: Need to find DynEventPublisher
- `tool_output_stash`: New HashMap
- `user_timezone`: `self.config.timezone` (default "UTC")
- `conversation_id`: None (set per-conversation)
- `registered_capability_names`: Empty initially

### Step 1.13: Create PairingContext

```rust
use crate::capabilities::pairing::PairingContext;

let pairing_ctx = Arc::new(PairingContext {
    store: Arc::new(PairingStore::new(self.db.clone().expect("database required for pairing"))),
    user_id: self.config.owner_id.clone(),
});
```

**Data Sources:**
- `store`: New `PairingStore` with database
- `user_id`: `self.config.owner_id`

### Step 1.14: Create BuiltinCapabilityDispatcher

```rust
use crate::capabilities::dispatcher::BuiltinCapabilityDispatcher;

let builtin_dispatcher = Arc::new(BuiltinCapabilityDispatcher::new(
    filesystem_ctx,
    shell_ctx,
    network_ctx,
    memory_ctx,
    messaging_ctx,
    jobs_ctx,
    routines_ctx,
    skills_ctx,
    extensions_ctx,
    secrets_ctx,
    images_ctx.expect("images context required"), // Or handle Option
    system_ctx,
    pairing_ctx,
));
```

### Step 1.15: Create CapabilityHost

```rust
use brassclaw_capabilities::CapabilityHost;
use brassclaw_extensions::ExtensionRegistry;
use brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer;

// Need to get or create these components:
let extension_registry = Arc::new(ExtensionRegistry::new()); // TODO: Get from app state
let authorizer = Arc::new(/* TODO: Create authorizer */);

let capability_host = CapabilityHost::new(
    &*extension_registry,
    &*builtin_dispatcher,
    &*authorizer,
);

// Optionally attach stores if available:
// .with_run_state(&*run_state_store)
// .with_approval_requests(&*approval_store)
// .with_capability_leases(&*lease_store)
// .with_process_manager(&*process_manager)
// .with_obligation_handler(&*obligation_handler)
```

**NOTE**: Need to investigate where ExtensionRegistry and authorizer come from.

### Step 1.16: Create V2 EffectBridgeAdapter

```rust
use crate::bridge::effect_adapter_v2::EffectBridgeAdapter;
use brassclaw_host_api::EffectExecutor;

let effect_executor: Arc<dyn EffectExecutor> = Arc::new(
    EffectBridgeAdapter::new(capability_host)
);
```

### Step 1.17: Update AppComponents Return

Change the return statement in `build_all()` to include `effect_executor` instead of `tools`:

```rust
Ok(AppComponents {
    config: self.config,
    db: self.db,
    secrets_store: self.secrets_store,
    llm,
    cheap_llm,
    llm_reload,
    safety,
    effect_executor,  // NEW: Replace tools
    embeddings,
    workspace,
    settings_store,
    settings_cache,
    extension_manager,
    mcp_session_manager,
    mcp_process_manager,
    wasm_tool_runtime,
    log_broadcaster: self.log_broadcaster,
    context_manager,
    hooks,
    agent_session_manager,
    skill_registry,
    skill_catalog,
    cost_guard,
    recording_handle,
    http_interceptor,
    session: self.session,
    catalog_entries,
    dev_loaded_tool_names,
    builder,
    ownership_cache,
})
```

## Phase 2: Update AppComponents Structure

### File: `/Volumes/SSDE/brassclaw/src/app.rs`

Change line 46:
```rust
// OLD:
pub tools: Arc<ToolRegistry>,

// NEW:
pub effect_executor: Arc<dyn EffectExecutor>,
```

Add import:
```rust
use brassclaw_host_api::EffectExecutor;
```

## Phase 3: Update Agent::new() Signature

### File: `/Volumes/SSDE/brassclaw/src/agent/agent_loop.rs`

Find `Agent::new()` method (around line 569) and change signature:

```rust
// OLD:
pub fn new(
    tools: Arc<ToolRegistry>,
    // ... other params
) -> Self

// NEW:
pub fn new(
    effect_executor: Arc<dyn EffectExecutor>,
    // ... other params
) -> Self
```

Update the struct field:
```rust
// OLD:
tools: Arc<ToolRegistry>,

// NEW:
effect_executor: Arc<dyn EffectExecutor>,
```

## Phase 4: Update Scheduler

### File: `/Volumes/SSDE/brassclaw/src/agent/scheduler.rs`

Change the struct field:
```rust
// OLD:
tools: Arc<ToolRegistry>,

// NEW:
effect_executor: Arc<dyn EffectExecutor>,
```

Update constructor and all usages.

## Phase 5: Update RoutineEngine

### File: `/Volumes/SSDE/brassclaw/src/agent/routine_engine.rs`

Change the struct field:
```rust
// OLD:
tools: Arc<ToolRegistry>,

// NEW:
effect_executor: Arc<dyn EffectExecutor>,
```

Update constructor and all usages.

## Phase 6: Update bridge/router.rs

### File: `/Volumes/SSDE/brassclaw/src/bridge/router.rs`

Find line 1751 and change:
```rust
// OLD:
let adapter = EffectBridgeAdapter::new(tools);

// NEW:
let adapter = effect_executor; // Already V2
```

## Phase 7: Update main.rs

### File: `/Volumes/SSDE/brassclaw/src/main.rs`

Find Agent::new() call (around line 1372) and update:
```rust
// OLD:
let agent = Agent::new(
    components.tools,
    // ... other params
);

// NEW:
let agent = Agent::new(
    components.effect_executor,
    // ... other params
);
```

## Phase 8: Fill RoutineEngine Slot

After RoutineEngine is created, fill in the RoutinesContext slot:

```rust
// After RoutineEngine creation:
if let Some(engine) = routine_engine {
    *routines_ctx.engine.write().await = Some(Arc::clone(&engine));
}
```

## Phase 9: Fix Test Files

Update all test files that use mock `ToolRegistry` to use mock `EffectExecutor` instead.

Files to update (15+):
- Search for `ToolRegistry` in test files
- Replace with mock `EffectExecutor`
- Update test assertions

## Phase 10: Verification

1. Run `cargo build` - should compile without errors
2. Run `cargo test` - all tests should pass
3. Run `cargo clippy -- -D warnings` - no warnings
4. Verify no V1 references remain:
   ```bash
   cd /Volumes/SSDE/brassclaw
   grep -r "ToolRegistry\|ToolDispatcher\|PermissionState" ./src/ ./crates/
   ```

## Critical Notes

1. **Circular Dependency**: RoutinesContext.engine starts as None and gets filled later
2. **Optional Contexts**: ImagesContext is optional (only if API key available)
3. **Missing Components**: Several components need investigation:
   - ChannelManager source
   - ContainerJobManager source
   - Event broadcaster
   - Message injector
   - Prompt queue
   - ExtensionRegistry
   - Authorizer
   - Various stores (run_state, approval, lease, process_manager, obligation_handler)

4. **Order Matters**: Contexts must be created in order due to dependencies

## Estimated Effort

- Phase 1 (Create V2 System): 6-8 hours
- Phase 2-8 (Update System): 4-6 hours
- Phase 9 (Fix Tests): 4-6 hours
- Phase 10 (Verification): 2-3 hours

**Total**: 16-23 hours of focused work

## Next Steps

1. Start with Phase 1, Step 1.1
2. Work through each context creation sequentially
3. Investigate missing component sources as you encounter them
4. Test compilation after each major phase
5. Fix errors incrementally rather than all at once