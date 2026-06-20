# Phase 11B: Fix Remaining 41 Compilation Errors

## Error Categories

### 1. Agent.tools() Method Missing (10 errors)
- agent_loop.rs:1066, 1348, 1721
- commands.rs:789
- Need to add `tools()` method to Agent struct that returns Arc<ToolRegistry>

### 2. AgentDeps.builder Field Missing (1 error)
- agent_loop.rs:1064
- Need to uncomment builder field in AgentDeps struct

### 3. ExtensionManager Missing Methods (4 errors)
- agent_loop.rs:440 - notification_target_for_channel()
- Need to add 4 stub methods to ExtensionManager

### 4. Tool Trait vs Struct Confusion (1 error)
- channels/channel.rs:742 - expects trait, found struct
- Need to fix Tool stub to be a trait

### 5. Channel Result Type Mismatches (12 errors)
- channels/wasm.rs:58, 66, 70 (3 errors for type alias + 3 for trait methods)
- channels/web.rs:36, 44, 48 (3 errors for type alias + 3 for trait methods)
- Need to fix Result type to use single generic argument

### 6. ExtensionManager Type Issue (1 error)
- extensions/manager.rs:30 - expected type, found trait
- Need to check trait bound issue

## Implementation Plan

1. Fix Tool stub to be a trait (not struct)
2. Add Agent.tools() method stub
3. Uncomment AgentDeps.builder field
4. Add ExtensionManager stub methods
5. Fix Channel Result types in web.rs and wasm.rs
6. Verify compilation

## Files to Modify

1. src/tools.rs - Fix Tool to be trait
2. src/agent/agent_loop.rs - Add tools() method, uncomment builder
3. src/extensions/manager.rs - Add missing methods
4. src/channels/web.rs - Fix Result types
5. src/channels/wasm.rs - Fix Result types
6. src/channels/channel.rs - May need adjustment