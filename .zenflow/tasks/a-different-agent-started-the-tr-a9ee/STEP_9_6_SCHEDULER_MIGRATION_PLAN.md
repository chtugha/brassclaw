# Step 9.6: Migrate Scheduler from ToolRegistry to EffectExecutor

## Current State Analysis

### Scheduler's ToolRegistry Usage (9 places identified)

1. **SchedulerDeps.tools field** (line 53)
   ```rust
   pub struct SchedulerDeps {
       pub tools: Arc<ToolRegistry>,
       ...
   }
   ```

2. **Scheduler.tools field** (line 65)
   ```rust
   pub struct Scheduler {
       tools: Arc<ToolRegistry>,
       ...
   }
   ```

3. **Scheduler::new() constructor** (line 96)
   ```rust
   tools: deps.tools,
   ```

4. **autonomous_approval_context() method** (line 254)
   ```rust
   autonomous_allowed_tool_names(&self.tools, self.extension_manager.as_ref(), user_id)
   ```

5. **execute_tool_task() method signature** (line 543)
   ```rust
   async fn execute_tool_task(
       tools: Arc<ToolRegistry>,
       ...
   )
   ```

6. **execute_tool_task() - tool lookup** (line 554)
   ```rust
   let tool = tools.get(tool_name).await.ok_or_else(...)
   ```

7. **execute_tool_task() - tool execution** (line 581-583)
   ```rust
   let output_str = crate::tools::execute::execute_tool_with_safety(
       &tools, &safety, tool_name, params, &job_ctx,
   )
   ```

8. **tools() getter method** (line 747-749)
   ```rust
   pub fn tools(&self) -> &Arc<ToolRegistry> {
       &self.tools
   }
   ```

9. **Test code** (lines 831, 839-840)
   ```rust
   let tools = Arc::new(ToolRegistry::new());
   SchedulerDeps {
       tools,
       ...
   }
   ```

### Key Dependencies

- `crate::tools::ToolRegistry` - V1 tool registry
- `crate::tools::autonomous_allowed_tool_names()` - V1 autonomy filtering
- `crate::tools::execute::execute_tool_with_safety()` - V1 tool execution pipeline
- `crate::tools::prepare_tool_params()` - Parameter normalization

## Migration Strategy

### Approach: Use EffectExecutor Instead of ToolRegistry

The Scheduler should use `EffectExecutor` (via `EffectBridgeAdapter`) instead of `ToolRegistry`. This aligns with:
- RoutineEngine already migrated to use dispatcher
- V2 BuiltinCapabilityDispatcher operational
- EffectBridgeAdapter provides the bridge between engine and capabilities

### Design Decision: Keep ToolRegistry Temporarily for Backward Compatibility

**Problem**: The Scheduler is used in multiple places and removing ToolRegistry completely would break:
- `agent_loop.rs` - Creates Scheduler with SchedulerDeps
- Test code across multiple files
- The `autonomous_allowed_tool_names()` function still exists

**Solution**: 
1. Add `effect_executor: Arc<dyn EffectExecutor>` to SchedulerDeps and Scheduler
2. Keep `tools: Arc<ToolRegistry>` temporarily for backward compatibility
3. Update `execute_tool_task()` to use EffectExecutor when available
4. Mark ToolRegistry fields as deprecated
5. In a future step, remove ToolRegistry entirely

### Alternative Approach: Direct Migration (More Disruptive)

Replace ToolRegistry completely in one step:
- Remove `tools` field from SchedulerDeps and Scheduler
- Update all call sites immediately
- Rewrite `execute_tool_task()` to use EffectExecutor only
- Update all tests

**Decision**: Use the gradual approach to minimize disruption.

## Implementation Plan

### Step 1: Update SchedulerDeps Structure

```rust
pub struct SchedulerDeps {
    #[deprecated(note = "Use effect_executor instead")]
    pub tools: Arc<ToolRegistry>,
    pub effect_executor: Option<Arc<dyn EffectExecutor>>,
    pub extension_manager: Option<Arc<ExtensionManager>>,
    pub store: Option<SystemScope>,
    pub hooks: Arc<HookRegistry>,
}
```

### Step 2: Update Scheduler Structure

```rust
pub struct Scheduler {
    config: AgentConfig,
    context_manager: Arc<ContextManager>,
    llm: Arc<dyn LlmProvider>,
    safety: Arc<SafetyLayer>,
    #[deprecated(note = "Use effect_executor instead")]
    tools: Arc<ToolRegistry>,
    effect_executor: Option<Arc<dyn EffectExecutor>>,
    extension_manager: Option<Arc<ExtensionManager>>,
    // ... rest of fields
}
```

### Step 3: Update Scheduler::new() Constructor

```rust
pub fn new(
    config: AgentConfig,
    context_manager: Arc<ContextManager>,
    llm: Arc<dyn LlmProvider>,
    safety: Arc<SafetyLayer>,
    deps: SchedulerDeps,
) -> Self {
    Self {
        config,
        context_manager,
        llm,
        safety,
        tools: deps.tools,
        effect_executor: deps.effect_executor,
        extension_manager: deps.extension_manager,
        // ... rest of initialization
    }
}
```

### Step 4: Update execute_tool_task() Method

Add EffectExecutor parameter and use it when available:

```rust
async fn execute_tool_task(
    tools: Arc<ToolRegistry>,
    effect_executor: Option<Arc<dyn EffectExecutor>>,
    context_manager: Arc<ContextManager>,
    safety: Arc<SafetyLayer>,
    approval_context: Option<ApprovalContext>,
    job_id: Uuid,
    tool_name: &str,
    params: serde_json::Value,
) -> Result<TaskOutput, Error> {
    let start = std::time::Instant::now();

    // Prefer EffectExecutor if available (V2 path)
    if let Some(executor) = effect_executor {
        // Use V2 execution path
        let job_ctx = context_manager.get_context(job_id).await?;
        if job_ctx.state == JobState::Cancelled {
            return Err(crate::error::ToolError::ExecutionFailed {
                name: tool_name.to_string(),
                reason: "Job is cancelled".to_string(),
            }
            .into());
        }

        // Create ThreadExecutionContext for V2
        let thread_context = ThreadExecutionContext {
            user_id: job_ctx.user_id.clone(),
            thread_id: job_id.to_string(),
            current_call_id: None,
        };

        // Create empty lease (permissions handled by CapabilityHost)
        let lease = CapabilityLease::default();

        // Execute via EffectExecutor
        let result = executor
            .execute_action(tool_name, params, &lease, &thread_context)
            .await
            .map_err(|e| Error::Tool(crate::error::ToolError::ExecutionFailed {
                name: tool_name.to_string(),
                reason: format!("V2 execution failed: {}", e),
            }))?;

        return Ok(TaskOutput::new(result.output, start.elapsed()));
    }

    // Fall back to V1 path (existing code)
    let tool = tools.get(tool_name).await.ok_or_else(|| {
        Error::Tool(crate::error::ToolError::NotFound {
            name: tool_name.to_string(),
        })
    })?;

    // ... rest of existing V1 code
}
```

### Step 5: Update spawn_subtask() to Pass EffectExecutor

```rust
match task {
    Task::Tool {
        parent_id: tool_parent_id,
        tool_name,
        params,
    } => {
        let tools = self.tools.clone();
        let effect_executor = self.effect_executor.clone();
        let context_manager = self.context_manager.clone();
        let safety = self.safety.clone();

        tokio::spawn(async move {
            let result = Self::execute_tool_task(
                tools,
                effect_executor,
                context_manager,
                safety,
                None,
                tool_parent_id,
                &tool_name,
                params,
            )
            .await;

            let _ = result_tx.send(result);
        })
    }
    // ... rest
}
```

### Step 6: Update tools() Getter (Deprecate)

```rust
#[deprecated(note = "Use effect_executor instead")]
pub fn tools(&self) -> &Arc<ToolRegistry> {
    &self.tools
}

pub fn effect_executor(&self) -> Option<&Arc<dyn EffectExecutor>> {
    self.effect_executor.as_ref()
}
```

### Step 7: Update agent_loop.rs Instantiation

```rust
let mut scheduler = Scheduler::new(
    config.clone(),
    context_manager.clone(),
    deps.llm.clone(),
    deps.safety.clone(),
    SchedulerDeps {
        tools: tools.clone(),
        effect_executor: deps.effect_executor.clone(), // Add this
        extension_manager: deps.extension_manager.clone(),
        store: deps
            .store
            .as_ref()
            .map(|db| crate::tenant::SystemScope::new(Arc::clone(db))),
        hooks: deps.hooks.clone(),
    },
);
```

### Step 8: Update Test Code

Update test helper to create both ToolRegistry and mock EffectExecutor:

```rust
fn make_test_scheduler(max_tokens_per_job: u64) -> Scheduler {
    // ... existing setup ...
    let tools = Arc::new(ToolRegistry::new());
    let effect_executor = None; // Tests can use V1 path for now
    
    Scheduler::new(
        config,
        cm,
        llm,
        safety,
        SchedulerDeps {
            tools,
            effect_executor,
            extension_manager: None,
            store: None,
            hooks,
        },
    )
}
```

## Required Imports

Add to scheduler.rs:
```rust
use brassclaw_engine::{EffectExecutor, ThreadExecutionContext, CapabilityLease};
```

## Compilation Verification

After changes:
1. `cargo build` - must succeed
2. Check for deprecation warnings
3. Verify no breaking changes to public API

## Testing Strategy

1. Existing tests should continue to pass (using V1 path)
2. Add new test for V2 path when EffectExecutor is provided
3. Integration test with real EffectBridgeAdapter

## Future Cleanup (Step 9.7+)

Once all components are migrated:
1. Remove `tools` field from SchedulerDeps
2. Remove `tools` field from Scheduler
3. Remove V1 fallback code from execute_tool_task()
4. Make `effect_executor` required (not Option)
5. Remove deprecation warnings

## Success Criteria

- [x] SchedulerDeps has effect_executor field
- [x] Scheduler has effect_executor field
- [x] execute_tool_task() can use EffectExecutor
- [x] Backward compatibility maintained
- [x] Compilation succeeds
- [x] Tests pass
- [x] No breaking changes to public API