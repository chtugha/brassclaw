# Stub Fix Plan: Step 6.3 — `max_duration_secs` from `reborn_monty_vm_settings` → live turn timeout

## Status: ✅ IMPLEMENTED

## Problem

`MontyVmSettings.max_duration_secs` is stored in DB (V034, via `PgMontyVmSettingsStore`) and
returned via the WebUI settings API (`GET /api/settings/monty-vm`). But in the v2 Reborn execution
path (`DefaultPlannedRuntimeParts` / `TurnRunnerWorker`), the value is never applied as an actual
per-turn wall-clock timeout. The v1 engine path (`ExecutionLoop::with_max_duration_secs`, Step 9.3)
has the mechanism, but the v2 `DefaultPlannedRuntimeConfig` has no `max_duration` field.

## What already exists

- `MontyVmSettings.max_duration_secs: u64` in `brassclaw_product_workflow/src/settings.rs`
- `PgMontyVmSettingsStore` reads/writes `reborn_monty_vm_settings.max_duration_secs`
- `default_monty_vm_settings()` returns `max_duration_secs: 300` (5 min compiled-in default)
- `DefaultPlannedRuntimeConfig` in `crates/brassclaw_reborn/src/runtime.rs` — no `max_duration` field
- `TurnRunnerWorkerConfig` in `crates/brassclaw_reborn/src/driver/worker.rs` — check for timeout field
- `orchestrator_max_duration()` in `execute_orchestrator` reads from env var OnceLock (v1 path only)

## Steps

### Step 1 — Add `max_turn_duration: Option<Duration>` to `DefaultPlannedRuntimeConfig`

In `crates/brassclaw_reborn/src/runtime.rs`:
```rust
/// Optional wall-clock ceiling for a single turn. When `Some`, the
/// `TurnRunnerWorker` wraps the turn future in a `tokio::time::timeout`
/// and fails the turn with `TurnError::Timeout` if it exceeds the budget.
/// When `None`, turns run unconstrained (the compiled-in env-var fallback
/// `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` applies at the orchestrator
/// level, not the loop level).
pub max_turn_duration: Option<Duration>,
```

### Step 2 — Enforce the timeout in `TurnRunnerWorker`

In `crates/brassclaw_reborn/src/driver/worker.rs`, find where the turn future is awaited.
If `config.max_turn_duration` is `Some(d)`, wrap with `tokio::time::timeout(d, turn_future)`.
On `Err(Elapsed)` → map to an appropriate `TurnError::Timeout` / `TurnOutcome::Failed`.

### Step 3 — Wire `max_turn_duration` from `MontyVmSettings` in `build_reborn_runtime`

In `crates/brassclaw_reborn_composition/src/runtime.rs`, after the MontyVmSettings store is
available (loaded from `services.monty_vm_settings`), load the settings and populate:
```rust
if let Some(store) = &services.monty_vm_settings {
    if let Ok(settings) = store.get_settings().await {
        config.max_turn_duration = Some(Duration::from_secs(settings.max_duration_secs));
    }
}
// DB-less fallback: use BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS env var
if config.max_turn_duration.is_none() {
    config.max_turn_duration = Some(brassclaw_engine::executor::orchestrator::orchestrator_max_duration());
}
```

### Step 4 — Clippy + tests
```bash
cargo clippy -p brassclaw_reborn -p brassclaw_reborn_composition --all-targets -- -D warnings
cargo test -p brassclaw_reborn
cargo test -p brassclaw_reborn_composition
```

### Step 5 — Update Step 6.3 in checkup.md + commit + push

## Files to touch
- `crates/brassclaw_reborn/src/runtime.rs` — add `max_turn_duration` field to `DefaultPlannedRuntimeConfig`
- `crates/brassclaw_reborn/src/driver/worker.rs` — enforce timeout on turn execution
- `crates/brassclaw_reborn_composition/src/runtime.rs` — load from MontyVmSettings store + wire
- `checkup.md` — update Step 6.3 note
