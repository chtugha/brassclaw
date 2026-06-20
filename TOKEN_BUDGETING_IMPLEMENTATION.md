# Token Budgeting Implementation

## Overview

This document describes the comprehensive token budgeting system implemented to prevent token overflow and improve prompt composition in BrassClaw Reborn.

## Problem Statement

In longer conversations, the LLM was getting spammed with excessive instructions, causing:
- Token limit overflows
- Degraded response quality
- Increased costs
- Context window exhaustion

## Solution Architecture

### 1. Token Budget Module (`crates/brassclaw_agent_loop/src/token_budget.rs`)

**Core Components:**

- `estimate_tokens(content: &str) -> usize`
  - Uses industry-standard ~4 characters per token heuristic
  - Fast estimation without requiring tokenizer
  
- `TokenBudgetTracker`
  - Tracks token consumption against a budget
  - Methods: `new()`, `unlimited()`, `would_exceed()`, `consume()`, `remaining()`, `used()`, `is_exceeded()`, `utilization()`
  - Comprehensive unit test coverage (13 tests)

### 2. Context Strategy Updates (`crates/brassclaw_agent_loop/src/strategies/context.rs`)

**Token-Aware Planning:**

- Added `max_context_tokens: Option<usize>` field
- Default budget: 8,000 tokens (`DEFAULT_MAX_CONTEXT_TOKENS`)
- Constructor methods: `new()`, `with_token_budget()`

**Budget Allocation Strategy:**

```rust
// Reserve 50% for conversation history, 50% for instructions
let conversation_budget = max_tokens / 2;
let instructions_budget = max_tokens / 2;
```

**Dynamic Message Reduction:**

- Estimates tokens from inline control messages
- Reduces `max_messages` when estimated tokens exceed budget
- Uses ~200 tokens per message as average estimate
- Prevents token overflow before prompt building

### 3. Prompt Building Updates (`crates/brassclaw_agent_loop/src/executor/prompt.rs`)

**Token Monitoring:**

- Added `estimate_prompt_bundle_tokens()` function
- Debug logging tracks:
  - Iteration number
  - Message count
  - Estimated tokens
  - Identity messages count
  - Instruction snippets count

**Visibility:**

```rust
debug!(
    iteration = %iteration,
    message_count = %messages.len(),
    estimated_tokens = %estimated_tokens,
    identity_messages = %identity_messages.len(),
    instruction_snippets = %instruction_snippets.len(),
    "Building prompt bundle"
);
```

### 4. Instruction Bundle Updates (`crates/brassclaw_turns/src/run_profile/instruction_bundle.rs`)

**Token Estimation Helpers:**

- `estimate_snippet_tokens()` - Estimates tokens for individual snippets
- `estimate_section_tokens()` - Estimates tokens for entire sections
- Marked as `pub(crate)` for internal use
- `#[allow(dead_code)]` for future integration

## Configuration

### Default Values

```rust
const DEFAULT_MAX_CONTEXT_TOKENS: usize = 8000;
const CHARS_PER_TOKEN: usize = 4;
const AVG_TOKENS_PER_MESSAGE: usize = 200;
```

### Usage Examples

```rust
// Create strategy with default budget (8000 tokens)
let strategy = DefaultContextStrategy::new();

// Create strategy with custom budget
let strategy = DefaultContextStrategy::new()
    .with_token_budget(Some(16000));

// Create strategy with unlimited budget
let strategy = DefaultContextStrategy::new()
    .with_token_budget(None);
```

## Testing

### Unit Tests (`crates/brassclaw_agent_loop/src/token_budget.rs`)

1. `test_estimate_tokens_empty`
2. `test_estimate_tokens_simple`
3. `test_estimate_tokens_long`
4. `test_tracker_new`
5. `test_tracker_unlimited`
6. `test_tracker_would_exceed`
7. `test_tracker_consume_success`
8. `test_tracker_consume_failure`
9. `test_tracker_remaining`
10. `test_tracker_used`
11. `test_tracker_is_exceeded`
12. `test_tracker_utilization`
13. `test_tracker_unlimited_utilization`

### Running Tests

```bash
cargo test -p brassclaw_agent_loop token_budget
```

## Integration Points

### Module Exports

```rust
// crates/brassclaw_agent_loop/src/lib.rs
pub mod token_budget;
```

### Import Patterns

```rust
// Use fully qualified paths for internal crate usage
let tokens = crate::token_budget::estimate_tokens(content);

// Or import the module
use crate::token_budget::{estimate_tokens, TokenBudgetTracker};
```

## Performance Characteristics

- **Token Estimation**: O(n) where n is string length
- **Budget Tracking**: O(1) for all operations
- **Memory Overhead**: Minimal (single usize counter)
- **No External Dependencies**: Pure Rust implementation

## Future Enhancements

1. **Dynamic Budget Adjustment**
   - Adjust budget based on model context window
   - Per-model budget configuration

2. **Advanced Estimation**
   - Optional integration with actual tokenizers
   - Model-specific token counting

3. **Budget Reporting**
   - Expose budget metrics via API
   - Dashboard visualization

4. **Adaptive Strategies**
   - Learn optimal budget allocation from usage patterns
   - Automatic message prioritization

## Related Changes

This implementation builds on the prompt reordering work documented in `PROMPT_REORDERING_CHANGES.md`:

1. **Priority 1**: Reordered message assembly (COMPLETED)
   - Moved conversation history to priority 2
   - Reduced instruction spam

2. **Priority 2**: Token-aware budgeting (THIS DOCUMENT)
   - Proactive token management
   - Dynamic message reduction

## Files Modified

1. `crates/brassclaw_agent_loop/src/token_budget.rs` (NEW)
2. `crates/brassclaw_agent_loop/src/lib.rs`
3. `crates/brassclaw_agent_loop/src/strategies/context.rs`
4. `crates/brassclaw_agent_loop/src/executor/prompt.rs`
5. `crates/brassclaw_turns/src/run_profile/instruction_bundle.rs`
6. `crates/brassclaw_agent_loop/Cargo.toml`

## Compilation Requirements

- Added `macros` feature to tokio dependency for `tokio::select!` support
- All code follows clippy guidelines with zero warnings
- Comprehensive test coverage

## References

- Industry standard: ~4 characters per token
- OpenAI tokenizer documentation
- Anthropic context window best practices