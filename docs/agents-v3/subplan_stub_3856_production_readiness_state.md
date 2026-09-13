# Subplan: Implement ProductionValidated Readiness State (#3856)

## Status: IN-PROGRESS

## Problem

`RebornReadinessState` currently has only two variants: `Disabled` and `DevOnly`.
Two integration tests in `crates/brassclaw_reborn_composition/tests/facade_factory.rs`
are marked `#[ignore]` with the comment:
  "TODO(#3856): restore when tenant sandbox process-port wiring exists"

Their bodies are empty stubs. The intent was to assert that when `build_reborn_services`
is called with a Postgres input + production trust policy + sandbox process port,
the returned `RebornServices::readiness.state` should be `ProductionValidated`
(not `DevOnly`). The `ProductionValidated` variant was never added to the enum.

## Root cause

The factory path for Postgres builds (`src/factory.rs`) calls `build_local_dev()`
internally and returns `readiness_for(RebornReadinessState::DevOnly, ...)` regardless
of whether a production trust policy was supplied. There is no differentiation between
a "production-quality" postgres build (has trust policy + process port) and a plain
local dev build.

## What must be done

### Step 1: Add `ProductionValidated` variant to `RebornReadinessState`

In `crates/brassclaw_reborn_composition/src/readiness.rs`:
- Add `ProductionValidated` variant to the enum.
- Update `serde(rename_all = "kebab-case")` so it serializes as `"production-validated"`.

### Step 2: Return `ProductionValidated` in the Postgres factory path

In `crates/brassclaw_reborn_composition/src/factory.rs`:
- When the Postgres build path succeeds AND `input.production_trust_policy.is_some()`,
  change the readiness state from `DevOnly` to `ProductionValidated`.
- The process-binding validation already happens before the build proceeds, so
  if we get past that gate and have a production trust policy, it is production-validated.

### Step 3: Restore the two ignored tests

In `crates/brassclaw_reborn_composition/tests/facade_factory.rs`:

#### `production_postgres_services_wire_first_party_runtime_http_egress`
- Build postgres services with: production_trust_policy + production_runtime_policy 
  + turn_run_wake_notifier + with_runtime_process_binding(test_sandbox_process_binding())
  + require_runtime_http_egress()
- Assert: services builds successfully (no error)
- Assert: services.host_runtime.is_some()
- Assert: services.readiness.state == RebornReadinessState::ProductionValidated
- Assert: services.readiness.facades.host_runtime == true

#### `migration_dry_run_validates_postgres_planned_turn_profile`
- Build postgres services with: production_trust_policy + production_runtime_policy
  + turn_run_wake_notifier + with_runtime_process_binding(test_sandbox_process_binding())
  (no require_runtime_http_egress — simulates migration dry run)
- Assert: services builds successfully (no error)
- Assert: services.host_runtime.is_some()
- Assert: services.readiness.state == RebornReadinessState::ProductionValidated

### Step 4: Verify no serialization breakage

The `readiness.state` field is serialized to JSON in the WebUI health endpoint.
Since it's `serde(rename_all = "kebab-case")`, the new variant serializes as
`"production-validated"`. The frontend must accept unknown readiness states gracefully
(checked — the WebUI shows readiness as a display-only status badge, no logic branches on it).

## Files to change

1. `crates/brassclaw_reborn_composition/src/readiness.rs` — add variant
2. `crates/brassclaw_reborn_composition/src/factory.rs` — update postgres path readiness
3. `crates/brassclaw_reborn_composition/tests/facade_factory.rs` — restore two tests
4. Any test that asserts `DevOnly` for postgres builds must be updated

## Validation

```bash
CARGO_TARGET_DIR=/Users/ollama/brassclaw-target \
  cargo test -p brassclaw_reborn_composition -- --nocapture 2>&1 | grep -E "FAILED|ok\.|ignored"

CARGO_TARGET_DIR=/Users/ollama/brassclaw-target \
  cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings
```
