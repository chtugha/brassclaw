# Step 9.3: Self-Repair Module Strategy

## Context
The `self_repair.rs` module has two responsibilities:
1. **Stuck Job Repair** - Detect and recover stuck agent jobs (V2-compatible)
2. **Tool Repair** - Detect and rebuild broken WASM/MCP tools (V1-specific)

## Problem
- Tool repair uses `ToolRegistry` (V1 system being removed)
- Tool repair uses `crate::tools::is_protected_tool_name()` (V1 API)
- Step 10 will delete entire `./src/tools/` directory

## Decision: Graceful Degradation

### Approach
Keep the self-repair module but disable tool repair functionality:

1. **Keep `tools` field as `Option<Arc<ToolRegistry>>`** (already optional)
2. **Remove ToolRegistry import** - not needed since it's optional
3. **Tool repair becomes no-op** when tools is None:
   - `detect_broken_tools()` returns empty vec
   - `repair_broken_tool()` returns success with skip message
4. **Stuck job repair continues working** (doesn't use ToolRegistry)

### Benefits
- ✅ Production code compiles
- ✅ Stuck job repair still works (critical functionality)
- ✅ Tool repair gracefully degrades (non-critical)
- ✅ No breaking changes to public API
- ✅ Clean removal in Step 10 when `./src/tools/` is deleted

### Implementation
- Remove `use crate::tools::ToolRegistry` import
- Keep `tools: Option<Arc<ToolRegistry>>` field (will be removed in Step 10)
- Update `with_builder()` to accept generic type instead of ToolRegistry
- Tool repair methods check if tools is Some, return early if None

### Future (Step 10)
When `./src/tools/` is deleted:
- Remove entire tool repair functionality
- Remove `builder` and `tools` fields
- Remove `with_builder()` method
- Keep only stuck job repair