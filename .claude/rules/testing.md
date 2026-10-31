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

## Cargo ownership and evidence

Follow the policy's [central build ownership](../../docs/development-policy.md#shared-source-and-central-build-ownership-2026-10-30).
Only the designated leading chat runs Cargo, including fmt, metadata and clean.
Other chats submit bounded validation requests to that owner. Preserve its source
freeze, configured canonical target, serial execution and disk-space rules.

Choose checks from the [evidence table](../../docs/development-policy.md#choose-evidence-by-the-claim)
and applicable subsystem acceptance. Review the connected diff first; reuse
evidence only for unchanged applicable source/artifact/configuration inputs.
Ordinary prose needs link/consistency review and `git diff --check`, not Cargo.
Embedded seed/runtime content still needs its actual packaging/integrity checks.
Do not routinely stack check/build/test/all-feature lint for the same targets.
Fix causes before repeating failed or invalidated checks; stop when required
checks pass. Keep broad CI/release and component-approval gates intact.

## Test Tiers

| Tier | Command | External deps |
|------|---------|---------------|
| Focused behavior/caller | `cargo test -p <crate_name> <test_filter>` | Determined by the test |
| Database semantics | Owning crate's actual integration/caller target and feature configuration | Real selected backend; PostgreSQL for SQL claims |

Add relevant feature flags. `cargo test` selects Cargo's default packages, not
necessarily the entire workspace. Full workspace acceptance is
`cargo test --workspace ...`; it is not an iteration default.
Read the selected crate's `Cargo.toml` and test gates before choosing features;
there is no universal `integration` feature. Rust behavior needs caller-level
regression evidence and relevant affected-package linting. Database, migration,
security and lifecycle claims require their real production/backend paths and
applicable failure cases. Read `LOCAL_TEST_ENV.md` when present before remote
tests or provider setup.

## Key Patterns

- Unit tests in `mod tests {}` at the bottom of each file
- Async tests with `#[tokio::test]`
- Use real implementations and verify real results; do not introduce mocks,
  stubs, fake commands or simulated success to avoid required checks
- Use `tempfile` crate for test directories, never hardcode `/tmp/`
- Meaningful regression coverage for behavioral fixes; reuse an existing test
  when it catches the bug and explain the evidence instead of duplicating it
- Report unavailable dependencies, skipped/ignored cases and zero selected tests
  as unverified, never passed. Missing PostgreSQL does not waive required SQL
  acceptance; record the blocker and route the required check to its owner.
- Fix introduced warnings, including affected consumers; report unrelated baseline
  warnings separately without suppression or unrelated cleanup.

## Test Through the Caller, Not Just the Helper

**When a helper gates a side-effecting flow, the test must go through the caller — not just the helper in isolation.**

A whole class of bugs in this repo has the same shape: a wrapper function silently loses one of its inputs, and the unit test for the helper passes because it never crosses the layer where the input gets dropped.

### When the rule applies

You need caller-level regression evidence when **all** of the following are true:

1. The helper is a **predicate, classifier, or transform** whose return value gates a side effect (HTTP call, DB write, tool execution, sandbox launch, approval gate, etc.).
2. There is **at least one wrapper or call site** between the helper and the side effect.
3. The helper has **more than one input** *or* its caller computes any of the inputs from the surrounding context.

If all three are true, a helper-only test is **not sufficient regression coverage**.
Reuse an existing applicable test that drives the handler/factory/manager and
catches the regression; otherwise add focused coverage through that caller.
Do not duplicate adequate coverage or refactor production code solely to avoid
the required verification.

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
