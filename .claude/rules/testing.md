---
paths:
  - "crates/**/*.rs"
  - "tests/**"
---
# Testing Rules

## Test Tiers

| Tier | Command | External deps |
|------|---------|---------------|
| Unit | `cargo test` | None |
| Integration | `cargo test --features integration` | Running PostgreSQL |

Run `cargo test -p <crate_name>` for a single crate, `cargo test` for all crates.

## Key Patterns

- Unit tests in `mod tests {}` at the bottom of each file
- Async tests with `#[tokio::test]`
- No mocks, prefer real implementations or stubs
- Use `tempfile` crate for test directories, never hardcode `/tmp/`
- Regression test with every bug fix
- Integration tests (`--features integration`) require PostgreSQL; skipped if DB is unreachable

## Test Through the Caller, Not Just the Helper

**When a helper gates a side-effecting flow, the test must go through the caller — not just the helper in isolation.**

A whole class of bugs in this repo has the same shape: a wrapper function silently loses one of its inputs, and the unit test for the helper passes because it never crosses the layer where the input gets dropped.

### When the rule applies

You must add a caller-level test (not just a helper-level unit test) when **all** of the following are true:

1. The helper is a **predicate, classifier, or transform** whose return value gates a side effect (HTTP call, DB write, tool execution, sandbox launch, approval gate, etc.).
2. There is **at least one wrapper or call site** between the helper and the side effect.
3. The helper has **more than one input** *or* its caller computes any of the inputs from the surrounding context.

If all three are true, a unit test on the helper alone is **not sufficient regression coverage**. You must additionally either:

- Add a test that drives the call site (the handler, factory, or manager function), **or**
- Inline the helper into its single caller so there is no wrapper to silently drop an input.

### Where the test belongs

Most of these gaps are above unit-test scope and below e2e scope. Default to the **integration tier** (`cargo test --features integration`):

- `tests/<module>_integration.rs` for Rust integration tests against the public handler/factory surface
- `tests/e2e/` for end-to-end scenarios when the lost axis is user-visible

Unit tests in `mod tests {}` are still fine for the helper itself, but they do not satisfy this rule.

### Mock hygiene corollary

When you mock a runtime API in a test, the mock's signature must match the production call site's signature, and assertions should cover **every argument** the production code passes.
