# Phase 11B: Systematic Fix Plan for 41 Remaining Errors

## Error Analysis

### Category 1: Channel Trait Issues (14 errors)
**Files**: channels/wasm.rs, channels/web.rs, channels/channel.rs

**Errors**:
- 6 errors: Result type alias takes 1 generic argument but 2 supplied
- 6 errors: Method has incompatible type (expected ChannelError, found error::Error)
- 1 error: Tool expected trait, found struct
- 1 error: ExtensionManager expected type, found trait

**Root Cause**: The Channel trait methods return `Result<T, ChannelError>` but stubs use `Result<T, error::Error>`

**Fix Strategy**: Change stub return types from `crate::error::Error` to `crate::error::ChannelError`

### Category 2: Agent.tools() Method Missing (10 errors)
**Files**: agent_loop.rs, commands.rs, dispatcher.rs, thread_ops.rs

**Errors**: All E0599 - no method named `tools` found

**Root Cause**: Agent struct doesn't have a tools() method

**Fix Strategy**: Add simple stub method that returns empty ToolRegistry

### Category 3: ExtensionManager Methods Missing (4 errors)
**Files**: agent_loop.rs, routine_engine.rs, thread_ops.rs

**Errors**:
- notification_target_for_channel() - 1 error
- owner_id() - 1 error  
- active_tool_names() - 1 error
- pending_oauth_flows() - 1 error

**Root Cause**: ExtensionManager stub missing these methods

**Fix Strategy**: Add stub methods to ExtensionManager

### Category 4: Type Sizing Issues (10 errors)
**Files**: commands.rs, dispatcher.rs

**Errors**: All E0277 - size for values of type `[T]` cannot be known

**Root Cause**: Functions returning slices `&[T]` but callers expect `Vec<T>`

**Fix Strategy**: Change return types from `&[T]` to `Vec<T>` in stub functions

### Category 5: Miscellaneous (3 errors)
**Files**: agent_loop.rs, dispatcher.rs, scheduler.rs, routine_engine.rs

**Errors**:
- AgentDeps.builder field missing - 1 error
- AdminToolPolicyCache type mismatch - 1 error
- ApprovalContext type collision - 1 error
- Function argument count mismatch - 1 error (routine_engine.rs:2191)

**Fix Strategy**: Individual fixes for each

## Implementation Order

### Phase 1: Channel Fixes (14 errors → 0)
1. Fix web.rs and wasm.rs Result types
2. Verify channel.rs Tool trait issue

### Phase 2: ExtensionManager Methods (4 errors → 0)
1. Add notification_target_for_channel()
2. Add owner_id()
3. Add active_tool_names()
4. Add pending_oauth_flows()

### Phase 3: Agent.tools() Method (10 errors → 0)
1. Add tools() method to Agent impl
2. Verify all call sites work

### Phase 4: Type Sizing (10 errors → 0)
1. Fix commands.rs slice returns
2. Fix dispatcher.rs slice returns

### Phase 5: Miscellaneous (3 errors → 0)
1. Add builder field to AgentDeps
2. Fix AdminToolPolicyCache type
3. Fix ApprovalContext naming collision
4. Fix routine_engine function call

## Success Criteria
- Compilation errors reduced from 41 to 0
- No new errors introduced
- All changes committed incrementally
- Each phase verified before proceeding to next