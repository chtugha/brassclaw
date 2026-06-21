# Unimplemented Features Analysis

## Executive Summary

**Total unimplemented calls found: 71**

**Key Finding: Zero production code issues.** All 71 occurrences are intentional, well-documented test stubs that follow Rust best practices for mock implementations.

## Summary Statistics

- Total unimplemented calls: 71
- Test stubs (intentional): 71
- V2 planned features: 0
- Deprecated V1 stubs: 0
- Production code requiring implementation: 0
- Test/mock code: 71

## Distribution by File

| File | Count | Type | Status |
|------|-------|------|--------|
| `src/hooks/session_summary.rs` | 18 | Test Mock | ✅ Intentional |
| `crates/brassclaw_conversations/tests/inbound_contract.rs` | 18 | Test Mock | ✅ Intentional |
| `crates/brassclaw_reborn/src/turn_runner/tests/mod.rs` | 12 | Test Mock | ✅ Intentional |
| `crates/brassclaw_reborn_composition/src/trigger_poller_trusted_submit.rs` | 16 | Test Mock | ✅ Intentional |
| `crates/brassclaw_conversations/src/inbound.rs` | 6 | Test Mock | ✅ Intentional |
| `crates/brassclaw_llm/src/reasoning.rs` | 1 | Test Mock | ✅ Intentional |

## Detailed Analysis

### Test Stubs (All 71 calls - No Action Required)

All unimplemented calls in the codebase are intentional test stubs that follow Rust best practices:

#### 1. src/hooks/session_summary.rs (18 calls)
**Context:** Mock `ConversationStore` trait implementation for testing session summary hooks  
**Pattern:** Minimal implementation - only implements methods actually used in tests  
**Status:** ✅ Intentional test stubs  
**Action:** None required  
**Rationale:** Using `unimplemented!()` catches accidental calls to unused methods, making tests safer

**Methods:**
- `get_conversation()` - 1 occurrence
- `create_conversation()` - 1 occurrence
- `update_conversation()` - 1 occurrence
- `delete_conversation()` - 1 occurrence
- `list_conversations()` - 1 occurrence
- `get_thread()` - 1 occurrence
- `create_thread()` - 1 occurrence
- `update_thread()` - 1 occurrence
- `delete_thread()` - 1 occurrence
- `list_threads()` - 1 occurrence
- `get_message()` - 1 occurrence
- `create_message()` - 1 occurrence
- `update_message()` - 1 occurrence
- `delete_message()` - 1 occurrence
- `list_messages()` - 1 occurrence
- `get_turn()` - 1 occurrence
- `create_turn()` - 1 occurrence
- `list_turns()` - 1 occurrence

#### 2. crates/brassclaw_conversations/tests/inbound_contract.rs (18 calls)
**Context:** Multiple mock coordinator implementations for testing inbound facade  
**Pattern:** Each mock explicitly documents "not used by inbound facade tests"  
**Status:** ✅ Intentional test stubs  
**Action:** None required  
**Rationale:** Clear documentation of test scope; safer than dummy implementations

**Mock Implementations:**
- `MockConversationCoordinator` - 3 methods
- `MockThreadCoordinator` - 3 methods
- `MockMessageCoordinator` - 3 methods
- `MockTurnCoordinator` - 3 methods
- `MockRunCoordinator` - 3 methods
- `MockToolCallCoordinator` - 3 methods

#### 3. crates/brassclaw_reborn/src/turn_runner/tests/mod.rs (12 calls)
**Context:** `StubHost` implementation for turn runner testing  
**Pattern:** All methods marked "never called by mock driver"  
**Status:** ✅ Intentional test stubs  
**Action:** None required  
**Rationale:** Documents which host methods the turn runner actually uses

**Methods:**
- `get_conversation()` - 1 occurrence
- `create_conversation()` - 1 occurrence
- `get_thread()` - 1 occurrence
- `create_thread()` - 1 occurrence
- `get_message()` - 1 occurrence
- `create_message()` - 1 occurrence
- `get_turn()` - 1 occurrence
- `create_turn()` - 1 occurrence
- `get_run()` - 1 occurrence
- `create_run()` - 1 occurrence
- `get_tool_call()` - 1 occurrence
- `create_tool_call()` - 1 occurrence

#### 4. crates/brassclaw_reborn_composition/src/trigger_poller_trusted_submit.rs (16 calls)
**Context:** Partial mock implementation for trigger poller testing  
**Pattern:** Each unimplemented method has descriptive comment  
**Status:** ✅ Intentional test stubs  
**Action:** None required  
**Rationale:** Clear documentation of what the test exercises vs. what it doesn't

**Methods:** 16 coordinator methods with explicit comments:
- `replay_canonical_inbound_message()` - "trigger prompt recorder tests do not replay canonical inbound messages"
- `mark_message_submitted()` - "trigger prompt recorder tests do not mark messages submitted"
- `defer_message()` - "trigger prompt recorder tests do not defer messages"
- `append_assistant_draft()` - "trigger prompt recorder tests do not append assistant drafts"
- `append_tool_result()` - "trigger prompt recorder tests do not append tool results"
- `append_display_preview()` - "trigger prompt recorder tests do not append display previews"
- `update_tool_result()` - "trigger prompt recorder tests do not update tool results"
- `update_assistant_draft()` - "trigger prompt recorder tests do not update assistant drafts"
- `finalize_assistant_message()` - "trigger prompt recorder tests do not finalize assistant messages"
- `redact_message()` - "trigger prompt recorder tests do not redact messages"
- `load_context_window()` - "trigger prompt recorder tests do not load context windows"
- `load_context_messages()` - "trigger prompt recorder tests do not load context messages"
- `list_message_range()` - "trigger prompt recorder tests do not list message ranges"
- `read_latest_message()` - "trigger prompt recorder tests do not read latest messages"
- `create_summary()` - "trigger prompt recorder tests do not create summaries"
- `update_thread_goal()` - "trigger prompt recorder tests do not update thread goals"

#### 5. crates/brassclaw_conversations/src/inbound.rs (6 calls)
**Context:** Test mocks matching the contract test patterns  
**Pattern:** Consistent with inbound_contract.rs approach  
**Status:** ✅ Intentional test stubs  
**Action:** None required  
**Rationale:** Maintains consistency across test suite

**Methods:**
- `resolve_conversation_binding()` - "not used by inbound facade tests"
- `resolve_linked_conversation()` - "not used by inbound facade tests"
- `resolve_reply_target()` - "not used by inbound facade tests"
- `resume_turn()` - "not used by inbound facade tests"
- `cancel_run()` - "not used by inbound facade tests"
- `get_run_state()` - "not used by inbound facade tests"

#### 6. crates/brassclaw_llm/src/reasoning.rs (1 call)
**Context:** Mock LLM provider for tool selection tests  
**Pattern:** Only implements `complete()` method needed for tests  
**Status:** ✅ Intentional test stub  
**Action:** None required  
**Rationale:** Minimal mock implementation for focused testing

**Details:**
- Location: Inside `TruncatingLlm` struct (lines 4262-4301)
- Purpose: Mock LLM provider for testing tool selection behavior with different finish reasons
- Used in: `test_select_tools_returns_empty_on_truncation()` test

### V2 Planned Features (0 items)

No V2 planned features requiring implementation were found.

### Deprecated V1 Stubs (0 items)

No deprecated V1 stubs requiring removal were found.

## Implementation Roadmap

### Immediate (Next Sprint)
**No action items.** All unimplemented calls are intentional test stubs.

### Short-term (1-2 months)
**No action items.**

### Long-term (3-6 months)
**No action items.**

## Best Practices Observed

The brassclaw codebase demonstrates excellent test engineering practices:

1. **Minimal Mock Implementations**: Only implement trait methods actually used in tests
2. **Explicit Documentation**: Each `unimplemented!()` includes context about why it's not needed
3. **Safety First**: Using `unimplemented!()` catches accidental calls to unused methods
4. **Living Documentation**: Test mocks document which parts of traits are actually exercised
5. **Consistency**: Similar patterns across the entire test suite

## Recommendations

### ✅ Current Approach is Optimal

The current use of `unimplemented!()` in test mocks is **recommended practice** and should be maintained:

1. **Safer than dummy implementations**: Catches bugs if test scope changes
2. **Self-documenting**: Makes test boundaries explicit
3. **Maintenance friendly**: Clear what each test exercises
4. **Performance**: No overhead from unused mock implementations

### 📋 Optional Enhancements

If desired, consider these non-critical improvements:

1. **Standardize comments**: Use consistent format like `// Not used by [test name]`
2. **Add test coverage metrics**: Track which trait methods are tested vs. mocked
3. **Document mock patterns**: Add section to CLAUDE.md about mock implementation strategy

### ⚠️ Do Not Change

**Do not replace `unimplemented!()` with dummy implementations.** The current approach is superior because:
- Dummy implementations hide bugs
- `unimplemented!()` provides clear failure messages
- Current approach is self-documenting

## Conclusion

**Phase 1 Analysis Complete: Zero issues found.**

All 71 `unimplemented!()` calls in the brassclaw codebase are intentional, well-documented test stubs that represent best practices in Rust testing. No implementation work is required.

The codebase demonstrates mature test engineering with:
- Clear separation between production and test code
- Minimal, focused mock implementations
- Excellent documentation of test scope
- Consistent patterns across the test suite

**Recommendation: Proceed to Phase 2 with confidence that the codebase has no hidden technical debt from unimplemented features.**