# V2 Reborn Implementation Plan (CORRECTED)

## Critical Error in Previous Plan

**The previous `V2_IMPLEMENTATION_PLAN.md` is COMPLETELY WRONG.** It attempted to manually create 13 context objects, which is a V1 migration approach, not V2 Reborn design.

## V2 Reborn Architecture (Correct)

The V2 Reborn system uses:

1. **SharedExtensionRegistry** - Manages all capability registrations (built-in + extensions)
2. **BuiltinCapabilityDispatcher** - Routes capability IDs to execute functions
3. **DefaultHostRuntime** or **CapabilityHost** - Handles authorization, approval gates, run state, obligations
4. **EffectBridgeAdapter** - Thin translation layer between engine and Reborn

**Key Insight**: The V2 system does NOT manually create context objects. Instead:
- Contexts are created internally by each capability's execute function
- The dispatcher routes to the correct execute function
- Execute functions access app state through dependency injection

## Correct Implementation Approach

### Phase 1: Create SharedExtensionRegistry with Built-in Capabilities

**File**: `/Volumes/SSDE/brassclaw/src/app.rs`
**Location**: After line 1169 in `build_all()`

```rust
use brassclaw_extensions::{ExtensionRegistry, SharedExtensionRegistry, ExtensionPackage};
use brassclaw_host_api::{CapabilityDescriptor, ExtensionId, RuntimeKind};

// Create extension registry
let mut extension_registry = ExtensionRegistry::new();

// Register built-in capabilities package
let builtin_package = create_builtin_capabilities_package();
extension_registry.upsert(builtin_package)
    .map_err(|e| anyhow::anyhow!("Failed to register built-in capabilities: {}", e))?;

let shared_registry = Arc::new(SharedExtensionRegistry::new(extension_registry));
```

### Phase 2: Create BuiltinCapabilityDispatcher

The `BuiltinCapabilityDispatcher` already exists in `src/capabilities/dispatcher.rs`. However, it needs access to app state. There are two approaches:

**Approach A: Dependency Injection via Dispatcher Constructor**

The dispatcher needs access to:
- Database
- Secrets store
- Workspace resolver
- LLM provider
- Extension manager
- Skill registry
- etc.

These should be passed to the dispatcher constructor and stored as fields.

**Approach B: Global App State (Current V1 Approach)**

The V1 system uses global/static access to app components. This is NOT the Reborn way.

**Recommended**: Use Approach A with proper dependency injection.

### Phase 3: Create DefaultHostRuntime

```rust
use brassclaw_host_runtime::DefaultHostRuntime;
use brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer;
use brassclaw_trust::HostTrustPolicy;
use brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy;

// Create authorizer
let authorizer = Arc::new(/* Create TrustAwareCapabilityDispatchAuthorizer */);

// Create trust policy
let trust_policy = Arc::new(HostTrustPolicy::default());

// Create runtime policy
let runtime_policy = EffectiveRuntimePolicy::default();

// Create surface version
let surface_version = CapabilitySurfaceVersion::default();

// Create host runtime
let host_runtime = DefaultHostRuntime::from_shared_registry(
    Arc::clone(&shared_registry),
    Arc::clone(&builtin_dispatcher) as Arc<dyn CapabilityDispatcher>,
    authorizer,
    surface_version,
    runtime_policy,
)
.with_trust_policy(trust_policy);

// Optionally attach stores if available:
// .with_run_state(run_state_store)
// .with_approval_requests(approval_store)
// .with_capability_leases(lease_store)
// .with_process_manager(process_manager)
// .with_obligation_handler(obligation_handler)
```

### Phase 4: Create EffectBridgeAdapter

```rust
use crate::bridge::effect_adapter_v2::EffectBridgeAdapter;
use brassclaw_engine::EffectExecutor;

let effect_executor: Arc<dyn EffectExecutor> = Arc::new(
    EffectBridgeAdapter::new(
        Arc::new(host_runtime), // Or CapabilityHost if using that directly
        Arc::clone(&shared_registry),
        Arc::clone(&safety),
    )
);
```

### Phase 5: Update AppComponents

Change `AppComponents` struct:

```rust
// OLD:
pub tools: Arc<ToolRegistry>,

// NEW:
pub effect_executor: Arc<dyn EffectExecutor>,
pub extension_registry: Arc<SharedExtensionRegistry>,
```

## Critical Design Questions

### Q1: How do capability execute functions access app state?

**Current Problem**: The `BuiltinCapabilityDispatcher` routes to execute functions like:

```rust
super::filesystem::execute_read_file(params, &self.filesystem_ctx)
```

But where does `filesystem_ctx` come from?

**Answer**: The dispatcher must be constructed with all necessary dependencies:

```rust
impl BuiltinCapabilityDispatcher {
    pub fn new(
        db: Option<Arc<dyn Database>>,
        secrets_store: Option<Arc<dyn SecretsStore>>,
        workspace_resolver: Arc<dyn WorkspaceResolver>,
        llm: Arc<dyn LlmProvider>,
        // ... all other dependencies
    ) -> Self {
        // Create contexts from dependencies
        let filesystem_ctx = Arc::new(FilesystemContext {
            base_dir: /* from config */,
            state: Arc::new(FilesystemCapabilityState::default()),
        });
        
        Self {
            filesystem_ctx,
            // ... other contexts
        }
    }
}
```

### Q2: Where do we create the 13 context objects?

**Answer**: Inside the `BuiltinCapabilityDispatcher::new()` constructor, using data passed as parameters.

### Q3: What about the circular dependency with RoutineEngine?

**Answer**: The solution is already in place - `RoutinesContext.engine` is `Arc<RwLock<Option<Arc<RoutineEngine>>>>`. It starts as None and gets filled in later.

## Revised Implementation Steps

### Step 1: Update BuiltinCapabilityDispatcher Constructor

**File**: `/Volumes/SSDE/brassclaw/src/capabilities/dispatcher.rs`

Add a proper constructor that takes all dependencies and creates the 13 context objects internally:

```rust
impl BuiltinCapabilityDispatcher {
    pub fn new(
        // All dependencies needed by contexts
        base_dir: PathBuf,
        db: Option<Arc<dyn Database>>,
        secrets_store: Option<Arc<dyn SecretsStore>>,
        workspace_resolver: Arc<dyn WorkspaceResolver>,
        llm: Arc<dyn LlmProvider>,
        cheap_llm: Option<Arc<dyn LlmProvider>>,
        credential_registry: Arc<SharedCredentialRegistry>,
        http_interceptor: Option<Arc<dyn HttpInterceptor>>,
        extension_manager: Arc<ExtensionManager>,
        skill_registry: Arc<std::sync::RwLock<SkillRegistry>>,
        skill_catalog: Arc<SkillCatalog>,
        context_manager: Arc<ContextManager>,
        owner_id: String,
        timezone: String,
        // ... other dependencies
    ) -> Self {
        // Create all 13 contexts from dependencies
        let filesystem_ctx = Arc::new(FilesystemContext {
            base_dir: base_dir.clone(),
            state: Arc::new(FilesystemCapabilityState::default()),
        });
        
        let shell_ctx = Arc::new(ShellContext {
            working_dir: base_dir.clone(),
            timeout: Duration::from_secs(120),
            sandbox_enabled: false, // from config
            sandbox_policy: None,
            allowed_commands: Vec::new(),
        });
        
        // ... create all other contexts
        
        Self {
            filesystem_ctx,
            shell_ctx,
            // ... all contexts
        }
    }
}
```

### Step 2: Create Built-in Capabilities Package

**File**: `/Volumes/SSDE/brassclaw/src/capabilities/mod.rs`

Add a function to create the built-in capabilities package:

```rust
use brassclaw_extensions::ExtensionPackage;
use brassclaw_host_api::{ExtensionId, RuntimeKind};

pub fn create_builtin_capabilities_package() -> ExtensionPackage {
    let mut capabilities = Vec::new();
    
    // Add all 47 capability descriptors
    capabilities.push(filesystem::read_file_descriptor());
    capabilities.push(filesystem::write_file_descriptor());
    // ... all other descriptors
    
    ExtensionPackage {
        id: ExtensionId::new("brassclaw.builtin").expect("valid extension id"),
        runtime: RuntimeKind::FirstParty,
        capabilities,
        // ... other package fields
    }
}
```

### Step 3: Wire Everything in AppBuilder.build_all()

**File**: `/Volumes/SSDE/brassclaw/src/app.rs`

After line 1169:

```rust
// 1. Create extension registry with built-in capabilities
let mut extension_registry = ExtensionRegistry::new();
let builtin_package = crate::capabilities::create_builtin_capabilities_package();
extension_registry.upsert(builtin_package)?;
let shared_registry = Arc::new(SharedExtensionRegistry::new(extension_registry));

// 2. Create dispatcher with all dependencies
let builtin_dispatcher = Arc::new(BuiltinCapabilityDispatcher::new(
    self.config.workspace.base_dir.clone(),
    self.db.clone(),
    self.secrets_store.clone(),
    workspace_resolver.clone().expect("workspace resolver required"),
    Arc::clone(&llm),
    cheap_llm.clone(),
    Arc::clone(&credential_registry),
    http_interceptor.clone(),
    extension_manager.clone().expect("extension manager required"),
    skill_registry.clone().expect("skill registry required"),
    skill_catalog.clone().expect("skill catalog required"),
    Arc::clone(&context_manager),
    self.config.owner_id.clone(),
    self.config.timezone.clone().unwrap_or_else(|| "UTC".to_string()),
    // ... other dependencies
));

// 3. Create authorizer and trust policy
let authorizer = Arc::new(/* Create authorizer */);
let trust_policy = Arc::new(HostTrustPolicy::default());

// 4. Create host runtime
let host_runtime = DefaultHostRuntime::from_shared_registry(
    Arc::clone(&shared_registry),
    builtin_dispatcher as Arc<dyn CapabilityDispatcher>,
    authorizer,
    CapabilitySurfaceVersion::default(),
    EffectiveRuntimePolicy::default(),
)
.with_trust_policy(trust_policy);

// 5. Create effect executor
let effect_executor: Arc<dyn EffectExecutor> = Arc::new(
    EffectBridgeAdapter::new(
        Arc::new(host_runtime),
        Arc::clone(&shared_registry),
        Arc::clone(&safety),
    )
);
```

### Step 4: Update AppComponents

Replace `tools` field with `effect_executor` and `extension_registry`.

### Step 5: Update Agent, Scheduler, RoutineEngine

Replace all `Arc<ToolRegistry>` with `Arc<dyn EffectExecutor>`.

### Step 6: Fill RoutineEngine Slot

After RoutineEngine is created, fill in the slot in RoutinesContext.

## Key Differences from Previous Plan

1. **No manual context creation in build_all()** - Contexts are created inside `BuiltinCapabilityDispatcher::new()`
2. **Uses SharedExtensionRegistry** - Not individual context objects
3. **Uses DefaultHostRuntime** - Not raw CapabilityHost
4. **Proper dependency injection** - Dispatcher constructor takes all dependencies
5. **Follows Reborn architecture** - Not V1 migration approach

## Next Steps

1. Update `BuiltinCapabilityDispatcher::new()` to accept all dependencies
2. Create `create_builtin_capabilities_package()` function
3. Wire everything in `AppBuilder.build_all()`
4. Update `AppComponents` structure
5. Update Agent, Scheduler, RoutineEngine
6. Test compilation

## Estimated Effort

- Phase 1-3 (Reborn wiring): 8-12 hours
- Phase 4-5 (Update system): 4-6 hours
- Phase 6 (Testing): 4-6 hours

**Total**: 16-24 hours (same as before, but correct approach)