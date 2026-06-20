# Step 9: Test Compilation Fixes Needed

## Summary
After removing `tools` field from `AgentDeps` and adding it as a parameter to `Agent::new()`, multiple test files need updates.

## Files Requiring Fixes

### 1. tests/support/gateway_workflow_harness.rs
- ✅ Line 274: Remove `tools: components.tools,` from AgentDeps
- ✅ Line 293: Add `components.tools,` as 3rd parameter

### 2. tests/support/test_rig.rs
- Line 1503: Remove `tools: components.tools,` from AgentDeps
- Line 1565: Remove `deps.tools` access
- Line 1583: Add tools parameter to Agent::new()

### 3. src/agent/thread_ops.rs (test code)
- Line 3263: Add tools parameter to Agent::new()
- Line 3292: Fix parameter order (ContextManager in wrong position)
- Line 3754: Add tools parameter to Agent::new()
- Line 3783: Fix parameter order
- Line 3837: Add tools parameter to Agent::new()
- Line 3866: Fix parameter order

### 4. src/agent/commands.rs (test code)
- Line 1254: Add tools parameter to Agent::new()
- Line 1283: Fix parameter order

### 5. src/agent/dispatcher.rs (test code)
- Line 2156: Remove `tools:` from AgentDeps (line 2466)
- Line 2176: Add tools parameter to Agent::new() (line 2486)
- Line 2511: Fix parameter - should be tools, not ChannelManager
- Line 2515: Fix parameter order
- Multiple similar issues at lines 3586, 3606, 3736, 3756, 3868, 3892

### 6. src/agent/agent_loop.rs (test code)
- Line 2634: Add tools parameter to Agent::new()
- Line 2663: Fix parameter order

### 7. src/bridge/router.rs (test code)
- Line 8580: Add tools parameter to Agent::new()
- Line 10353: Add tools parameter to Agent::new()
- Line 11690: Remove `agent.deps.tools =` assignment

## Pattern for Fixes

### Before:
```rust
Agent::new(
    config,
    AgentDeps {
        // ...
        tools: some_tools,
        // ...
    },
    channels,
    heartbeat_config,
    hygiene_config,
    routine_config,
    context_manager,
    session_manager,
)
```

### After:
```rust
Agent::new(
    config,
    AgentDeps {
        // ...
        // tools field removed
        // ...
    },
    some_tools,  // <-- Added as 3rd parameter
    channels,
    heartbeat_config,
    hygiene_config,
    routine_config,
    context_manager,
    session_manager,
)
```

## Status
- Gateway workflow harness: ✅ Fixed
- Remaining files: 🔄 In progress