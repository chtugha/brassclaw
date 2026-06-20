# Step 9.4: Routine Engine V2 Migration Strategy

## Current State Analysis

### How Routines Currently Work
1. **Lightweight routines** execute tools directly via `ToolRegistry`:
   - Line 1931-1932: `ctx.tools.tool_definitions_visible_under()` / `tool_definitions()`
   - Lines 2054-2080: `ctx.tools.get(&tc.name)` → `tool.execute()`
   - Direct V1 tool execution with timeout, validation, sanitization

2. **Full-job routines** delegate to `Scheduler`:
   - Scheduler also uses `ToolRegistry` for tool execution
   - Background job execution with approval flow

### The Problem
- Routines bypass the V2 capability system
- No V2 permission checks
- No V2 effect tracking
- Duplicates tool execution logic that exists in `EffectBridgeAdapter`

## Target State

### How Routines Should Work
1. **Lightweight routines** execute capabilities via `EffectExecutor`:
   - Replace `ToolRegistry` with `Arc<dyn EffectExecutor>`
   - Use `executor.list_actions()` to get available capabilities
   - Use `executor.execute_action()` to invoke capabilities
   - Leverage existing V2 permission checks, effect tracking, safety

2. **Full-job routines** continue delegating to `Scheduler`:
   - Scheduler will also be migrated to use `EffectExecutor`
   - Maintains approval flow through V2 system

## Implementation Plan

### Phase 1: Update RoutineEngine Structure

**File**: `src/agent/routine_engine.rs`

1. **Replace ToolRegistry with EffectExecutor**:
   ```rust
   // OLD (line 122):
   tools: Arc<ToolRegistry>,
   
   // NEW:
   executor: Arc<dyn EffectExecutor>,
   ```

2. **Update constructor** (line 154):
   ```rust
   // OLD:
   tools: Arc<ToolRegistry>,
   
   // NEW:
   executor: Arc<dyn EffectExecutor>,
   ```

3. **Update EngineContext** (line 1136):
   ```rust
   // OLD:
   tools: Arc<ToolRegistry>,
   
   // NEW:
   executor: Arc<dyn EffectExecutor>,
   ```

### Phase 2: Update Tool Definition Retrieval

**Location**: Lines 1930-1936 in `execute_lightweight_with_tools()`

**OLD**:
```rust
let tool_defs = match &ctx.runtime_policy {
    Some(policy) => ctx.tools.tool_definitions_visible_under(policy).await,
    None => ctx.tools.tool_definitions().await,
}
.into_iter()
.filter(|tool| allowed_tools.contains(&tool.name))
.collect();
```

**NEW**:
```rust
// Get all available actions from executor
let all_actions = ctx.executor.list_actions().await?;

// Filter to allowed autonomous tools
let tool_defs: Vec<_> = all_actions
    .into_iter()
    .filter(|action| allowed_tools.contains(&action.name))
    .map(|action| convert_action_to_tool_def(action))
    .collect();
```

**Helper function needed**:
```rust
fn convert_action_to_tool_def(action: ActionDef) -> brassclaw_llm::ToolDefinition {
    brassclaw_llm::ToolDefinition {
        name: action.name,
        description: action.description,
        parameters: action.parameters_schema,
    }
}
```

### Phase 3: Update Tool Execution

**Location**: Lines 2042-2124 in `execute_routine_tool()`

**OLD**:
```rust
// Check if tool exists
let tool = ctx.tools.get(&tc.name).await
    .ok_or_else(|| format!("Tool '{}' not found", tc.name))?;
let normalized_params = prepare_tool_params(tool.as_ref(), &tc.arguments);

// Validate tool parameters
let validation = ctx.safety.validator().validate_tool_params(&normalized_params);
if !validation.is_valid {
    // ... error handling
}

// Execute with per-tool timeout
let timeout = tool.execution_timeout();
let result = tokio::time::timeout(timeout, async {
    tool.execute(normalized_params.clone(), job_ctx).await
}).await;
```

**NEW**:
```rust
// Build execution context from job_ctx
let thread_ctx = ThreadExecutionContext {
    thread_id: job_ctx.thread_id.clone(),
    user_id: job_ctx.user_id.clone(),
    workspace_id: job_ctx.workspace_id.clone(),
    // ... other fields from job_ctx
};

// Execute action through V2 system
let result = ctx.executor.execute_action(
    &tc.name,
    tc.arguments.clone(),
    thread_ctx,
).await;

// Convert ActionResult to tool output format
let result_str = match result {
    Ok(action_result) => {
        serde_json::to_string(&action_result.result)
            .unwrap_or_else(|_| "<serialize error>".to_string())
    }
    Err(e) => return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
};
```

### Phase 4: Remove V1 Tool Dependencies

1. **Remove imports** (lines 36-39):
   ```rust
   // REMOVE:
   use crate::tools::{
       ToolError, ToolRegistry, autonomous_allowed_tool_names, 
       autonomous_unavailable_message, prepare_tool_params,
   };
   ```

2. **Add V2 imports**:
   ```rust
   use brassclaw_engine::{
       ActionDef, EffectExecutor, ThreadExecutionContext,
   };
   ```

3. **Keep autonomous helpers**: `autonomous_allowed_tool_names()` and `autonomous_unavailable_message()` are still needed - they filter which capabilities can run autonomously

### Phase 5: Update Call Sites

**File**: `src/main.rs` or wherever `RoutineEngine::new()` is called

**OLD**:
```rust
RoutineEngine::new(
    config,
    store,
    llm,
    workspace,
    notify_tx,
    scheduler,
    extension_manager,
    tools,  // Arc<ToolRegistry>
    safety,
    sandbox_readiness,
    http_interceptor,
)
```

**NEW**:
```rust
RoutineEngine::new(
    config,
    store,
    llm,
    workspace,
    notify_tx,
    scheduler,
    extension_manager,
    executor,  // Arc<dyn EffectExecutor>
    safety,
    sandbox_readiness,
    http_interceptor,
)
```

## Benefits

1. **Unified Execution Path**: Routines use same V2 system as chat turns
2. **Proper Permission Checks**: V2 PermissionMode enforced
3. **Effect Tracking**: All routine actions tracked through V2 system
4. **Code Deduplication**: Remove duplicate tool execution logic
5. **Consistent Safety**: Same safety layer for all tool invocations
6. **Easier Testing**: Mock `EffectExecutor` instead of `ToolRegistry`

## Risks & Mitigation

### Risk 1: Breaking Routine Functionality
**Mitigation**: 
- Comprehensive testing of lightweight routines
- Test both cron and event triggers
- Verify tool execution still works

### Risk 2: Performance Impact
**Mitigation**:
- `EffectExecutor` is already optimized for chat turns
- No additional overhead expected
- Monitor routine execution times

### Risk 3: Autonomous Tool Filtering
**Mitigation**:
- Keep `autonomous_allowed_tool_names()` logic
- Ensure V2 capabilities have correct metadata
- Test autonomous execution restrictions

## Testing Strategy

1. **Unit Tests**: Update routine_engine tests to use mock `EffectExecutor`
2. **Integration Tests**: Test lightweight routine tool execution end-to-end
3. **Manual Testing**: 
   - Create test routine with tool calls
   - Verify execution works
   - Check permission enforcement
   - Verify error handling

## Rollout Plan

1. ✅ Complete Phase 1-4 changes
2. ✅ Update call sites
3. ✅ Fix compilation errors
4. ✅ Update tests
5. ✅ Manual testing
6. ✅ Commit and push
7. ⏭️ Move to Sub-Task 9.6 (Scheduler migration)

## Notes

- This is a **functional migration**, not just reference removal
- Requires careful testing to ensure routines still work
- Scheduler (Sub-Task 9.6) will follow same pattern
- After both complete, V1 tools only used by test code