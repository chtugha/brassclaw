# Plan: Promote or Remove Dead Factory Production Path

## Status: STUB — needs architectural decision

## Problem

`crates/brassclaw_reborn_composition/src/factory.rs` contains a chain of dead code
(all annotated `#[allow(dead_code)] // Phase-5 factory wiring`) that was intended
as the pure-PG non-LocalDev production path:

```
build_production_shaped(input: RebornBuildInput)
  → build_postgres_production(context, pool, url, key, home)
    → build_pg_backend_production_with_tools(pool, filesys, ...)
      → build_postgres_memory_tools(pool)
```

This path is never called. `build_reborn_services` only handles `LocalDev | LocalDevYolo`
(and `Disabled`) — non-LocalDev profiles return an error. The CLI always uses `LocalDev`
profile with `RebornStorageInput::Postgres` for production `brassclaw serve`.

## Why it exists

The `build_production_shaped` chain was written as a future entry point for when a
non-LocalDev profile (e.g. `HostedSafe`, `HostedYolo`) is used. In this mode:
- No local filesystem substrate at all
- Full host runtime with PG-backed run_state, turn_state, resource_governor, leases
- Uses `build_postgres_production_host_runtime_services` for the host runtime

The `build_reborn_runtime` pure-PG path added in this session uses `PgRuntimeStores`
which provides the same PG-backed stores, but the `host_runtime` itself is still built
with a LocalDev profile (via `build_local_dev` hybrid path).

## Options

### Option A: Wire `build_production_shaped` into `build_reborn_services`

Add a new branch in `build_reborn_services` for non-LocalDev profiles:
```rust
RebornCompositionProfile::HostedSafe | RebornCompositionProfile::HostedYolo => {
    build_production_shaped(input).await
}
```
This would activate the entire chain and remove all `#[allow(dead_code)]` guards.

**Risk:** The chain has never been exercised. Unknown whether it compiles and runs
correctly end-to-end. Requires a production host runtime binary to test.

### Option B: Remove the dead chain entirely

Delete `build_production_shaped`, `build_postgres_production`,
`build_pg_backend_production_with_tools`, `build_postgres_memory_tools` and all
helpers that are only used by them. The hybrid LocalDev+PG path in `build_reborn_services`
combined with `build_reborn_runtime`'s pure-PG path covers the real production use case.

**Risk:** Deletes infrastructure that may be needed for a future hosted deployment.
Irreversible without git.

### Option C: Keep as-is with documented deferral (current state)

Accept the dead code with a comment pointing to this plan. The `#[allow(dead_code)]`
guards suppress compiler warnings. The chain is preserved for future Option A work.

**Current choice: Option C** — defer until a hosted deployment profile is needed.

## Steps (when this plan is executed)

### Step 1 — Audit and document what `build_production_shaped` covers that the hybrid path does not

Compare the two paths:
- `build_production_shaped` → `build_postgres_production_host_runtime_services` builds
  a `HostRuntimeServices` with PG-backed stores wired inside the host runtime itself
  (via `.with_pg_run_state()`, `.with_pg_turn_state_store()`, etc.)
- The hybrid path builds `HostRuntimeServices` with InMemory stores inside the host
  runtime, but `build_reborn_runtime` overrides them with PG-backed equivalents for
  the composition layer

Determine whether the host-runtime-level PG wiring matters for any host runtime
capability or feature.

### Step 2 — Choose Option A or B

Based on Step 1:
- If host-runtime-level PG wiring matters → implement Option A
- If the composition-layer override is sufficient → implement Option B

### Step 3 — Execute chosen option

For Option A:
- Add the new branch in `build_reborn_services`
- Run clippy + tests
- Deploy to a staging environment and verify

For Option B:
- Delete the dead chain
- Run clippy + tests
- Update checkup.md

### Step 4 — Remove all `#[allow(dead_code)] // Phase-5 factory wiring` annotations

After Option A or B, all surviving functions should be live code.

## Files

- `crates/brassclaw_reborn_composition/src/factory.rs` — lines 2399, 2468, 2655
  and all helpers at lines 322, 328, 341, 644, 1100, 1791, 1849, 1880, 1889, 1900,
  1924, 1937, 2099, 2135, 2198, 2468, 2654
