---
paths:
  - "crates/**/*.rs"
  - "tests/**"
---
# Testing Rules

Read [Development reasoning and validation policy](../../docs/development-policy.md).
Choose the smallest check that establishes the claim, reuse passing evidence and
stop when required checks pass. Source reasoning precedes compilation; runtime
Tier-0 rules do not constrain development reasoning.

## Test Tiers

| Tier | Command | External deps |
|------|---------|---------------|
| Focused behavior/caller | `cargo test -p <crate_name> <test_filter>` | Determined by the test |
| Database semantics | `cargo test -p <crate_name> --features integration <test_filter>` | Real PostgreSQL |

Add relevant feature flags. `cargo test` selects Cargo's default packages, not
necessarily the entire workspace. Full workspace acceptance is
`cargo test --workspace ...`; it is not an iteration default.

## Key Patterns

- Unit tests in `mod tests {}` at the bottom of each file
- Async tests with `#[tokio::test]`
- Use real implementations and verify real results; do not introduce mocks,
  stubs, fake commands or simulated success to avoid required checks
- Use `tempfile` crate for test directories, never hardcode `/tmp/`
- Meaningful regression coverage for behavioral fixes; reuse an existing test
  when it catches the bug and explain the evidence instead of duplicating it
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

Drive the production caller with real implementations at the smallest practical
layer that establishes the claim:

- A module test or `tests/<module>_integration.rs` may drive the actual
  handler/factory, asserting every relevant input and real effect.
- Use real PostgreSQL when SQL, transactions, constraints, locking, migrations
  or persistence semantics are the claim.
- Use `tests/e2e/` when only the full production path establishes the claim or a
  subsystem acceptance contract requires it. User visibility alone does not
  require duplicating an adequate caller regression in browser tests.

Helper-only tests do not satisfy this rule. A module test that drives the actual
caller does; test placement and external dependencies do not determine coverage.

### Verify the actual call

Trace every argument through wrappers into the real operation. Assert the result
and relevant side effect. If that verification is unavailable, report it as
unverified rather than substituting a simulated success.
