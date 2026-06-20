# V2 Migration Status - Current Session

## Completed Work

### 1. RoutineEngine V2 Migration ✅
- **Updated struct**: Changed from `executor: Arc<dyn EffectExecutor>` to `dispatcher: Arc<BuiltinCapabilityDispatcher>`
- **Updated constructor**: Modified `RoutineEngine::new()` to accept dispatcher instead of executor
- **Updated EngineContext**: Changed field from `executor` to `dispatcher`
- **Fixed all EngineContext creation sites**: Updated lines 853, 941, 982 to use dispatcher
- **Created helper functions**:
  - `list_all_v2_capabilities()`: Returns all 47 V2 capability IDs with descriptions
  - `get_autonomous_allowed_capabilities()`: Replaces V1 `autonomous_allowed_tool_names`
  - `is_autonomous_capability_denylisted()`: Replaces V1 denylist function
- **Updated tool execution**: Modified lines 2183-2211 to use V2 CapabilityDispatchRequest
- **Removed V1 imports**: Cleaned up `ToolRegistry` import

### 2. App.rs V2 System Instantiation ✅
- **Created all 13 context objects**:
  - FilesystemContext with base_dir from `brassclaw_base_dir()`
  - ShellContext with proper V2 structure
  - NetworkContext, MemoryContext, MessagingContext
  - JobsContext, RoutinesContext, SkillsContext
  - ExtensionsContext, SecretsContext, ImagesContext
  - SystemContext, PairingContext
- **Instantiated BuiltinCapabilityDispatcher** with all contexts
- **Created EffectBridgeAdapterV2**

### 3. Bridge Module Updates ✅
- **Added re-exports** to bridge/mod.rs:
  - `pub use brassclaw_engine::{EffectExecutor, ThreadExecutionContext};`

## Remaining Compilation Errors

### High Priority (Blocking)

1. **agent_loop.rs:1329** - RoutineEngine::new() call needs dispatcher instead of tools
2. **routine_engine.rs:2212** - CapabilityDispatchResult has no `result` field (need to use `output`)
3. **app.rs multiple locations** - Several context initialization issues:
   - Missing `metadata` field in RoutinesContext
   - Type mismatches in context creation
   - Missing `base_dir()` method on Workspace
   - Missing `timezone` field on Config

### Medium Priority

4. **Scheduler** - Still uses V1 ToolRegistry, needs V2 migration
5. **Agent** - Still uses V1 ToolRegistry, needs V2 migration  
6. **bridge/router.rs** - Needs to use V2 adapter

### Low Priority (Cleanup)

7. **Test files** - 15+ test files need updates
8. **V1 code removal** - Delete ./src/tools/ and ./src/channels/web/
9. **WebUI updates** - Backend and frontend for V2 tools

## Key Technical Decisions

### Simplified Architecture
- **Decision**: RoutineEngine uses `Arc<BuiltinCapabilityDispatcher>` directly
- **Rationale**: No need for complex engine orchestrator wrapper
- **Impact**: Cleaner, more maintainable code

### V2 Capability Dispatch
- **Structure**: CapabilityDispatchRequest with ResourceScope
- **Method**: `dispatch_json()` from CapabilityDispatcher trait
- **Result**: CapabilityDispatchResult with `output` field (not `result`)

### Context Architecture
- 13 domain contexts feed into BuiltinCapabilityDispatcher
- Each context encapsulates domain-specific data sources
- Dispatcher routes capability IDs to execute functions

## Next Steps

1. Fix CapabilityDispatchResult field access (use `output` not `result`)
2. Fix agent_loop.rs RoutineEngine::new() call
3. Complete app.rs context initialization
4. Migrate Scheduler to V2
5. Migrate Agent to V2
6. Update bridge/router.rs
7. Fix test files
8. Remove V1 code
9. Update WebUI

## Files Modified This Session

- `/Volumes/SSDE/brassclaw/src/agent/routine_engine.rs` - V2 migration complete
- `/Volumes/SSDE/brassclaw/src/app.rs` - V2 system instantiation in progress
- `/Volumes/SSDE/brassclaw/src/bridge/mod.rs` - Added re-exports

## Compilation Progress

- **Starting errors**: ~50+
- **Current errors**: ~17
- **Progress**: ~66% reduction in errors