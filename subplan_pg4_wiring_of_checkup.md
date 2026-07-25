# Subplan: PG-4 Wiring Gaps — Factory Postgres Store Wiring

## Status (as of this checkup session)

### ✅ Steps 1–5 — CONFIRMED DONE (in dead-code path)
`PgCapabilityLeaseStore`, `PgRunStateStore`, `PgApprovalRequestStore`, `PgTurnStateStore`,
`PgResourceGovernorStore` are all wired in `build_pg_backend_production_with_tools`.
The `#[allow(dead_code)]` functions exist and are correct. However, they are not yet reached
by the live `brassclaw serve` path (hybrid local-dev+PG). See `subplan_pg4_runtime_pg_path.md`.

### ✅ Step 6 — Clippy and tests PASS (zero warnings, 490+ tests)

### ✅ Step 7 — checkup.md updated, committed and pushed

---

## Problem

`build_backend_production_with_tools` (factory.rs) is the production build path called by
`build_postgres_production`. Even though all Pg stores exist (PgRunStateStore, PgTurnStateStore,
PgApprovalRequestStore, PgResourceGovernorStore, PgCapabilityLeaseStore), the factory still wires:

1. `InMemoryResourceGovernor` (line 2089) — NOT a persistent governor
2. `with_filesystem_run_state(stores_scoped_fs)` (line 2110) — filesystem-backed RunState + Approvals
3. `with_filesystem_turn_state_store(stores_scoped_fs)` (line 2111) — filesystem-backed TurnState
4. `FilesystemCapabilityLeaseStore` in `ProductionStoreBundle` (line 2004) — NOT PgCapabilityLeaseStore

## Steps

### Step 1 — Fix `ProductionStoreBundle` to use PgCapabilityLeaseStore

In `ProductionStoreBundle<F>`:
- Change `leases: Arc<FilesystemCapabilityLeaseStore<F>>` → `leases: Arc<dyn CapabilityLeaseStore>`
- In `new_postgres()`: replace `FilesystemCapabilityLeaseStore` with `PgCapabilityLeaseStore::new(Arc::new(pg_pool.clone()), "default")`
- Add missing imports: `brassclaw_authorization::PgCapabilityLeaseStore`, `brassclaw_authorization::CapabilityLeaseStore`

### Step 2 — Wire PgRunStateStore + PgApprovalRequestStore

In `build_backend_production_with_tools`:
- When `pg_pool.is_some()`, replace `.with_filesystem_run_state(Arc::clone(&stores_scoped_fs))`
  with `.with_run_state(pg_run_state).with_approval_requests(pg_approval_requests)`
  where `pg_run_state = Arc::new(PgRunStateStore::new(Arc::clone(&pool), "default"))`
  and `pg_approval_requests = Arc::new(PgApprovalRequestStore::new(Arc::clone(&pool), "default"))`
- Imports: `brassclaw_run_state::PgRunStateStore`, `brassclaw_approvals::PgApprovalRequestStore`

### Step 3 — Wire PgTurnStateStore

In `build_backend_production_with_tools`:
- When `pg_pool.is_some()`, replace `.with_filesystem_turn_state_store(Arc::clone(&stores_scoped_fs))`
  with `.with_turn_state_and_transition_port(Arc::new(PgTurnStateStore::new(Arc::clone(&pool), "default")))`
- Import: `brassclaw_turns::PgTurnStateStore`

### Step 4 — Wire PgResourceGovernorStore

In `build_backend_production_with_tools`:
- When `pg_pool.is_some()`, replace `Arc::new(InMemoryResourceGovernor::new())`
  in `HostRuntimeServices::new(...)` with:
  `Arc::new(PersistentResourceGovernor::new(PgResourceGovernorStore::new(Arc::clone(&pool), "default")))`
- Remove the `.with_filesystem_resource_governor(Arc::clone(&stores_scoped_fs))` call that follows
  (since the governor is already set)
- Imports: `brassclaw_resources::{PgResourceGovernorStore, PersistentResourceGovernor}`

### Step 5 — Handle Rust type system constraint

`HostRuntimeServices<F, G, S, R>` is generic over `G: ResourceGovernor`.
`HostRuntimeServices::new(...)` takes `G` as the governor type.
To avoid complex conditional branching on types, use the approach:
- `build_backend_production_with_tools` is already `#[cfg(feature = "postgres")]`
- Extract a helper `fn wire_pg_stores(services, pool)` that applies pg store overrides
  to an already-constructed `HostRuntimeServices<F, InMemoryResourceGovernor, S, R>`
  BUT — this won't work because `with_resource_governor` changes the type parameter.
  
**Correct approach**: Since `pg_pool` in the production postgres build path is always `Some`,
extract the pool at the top of the function and use it unconditionally in `HostRuntimeServices::new`.
The `pg_pool` parameter is `Option<Arc<...>>` only because the function signature is shared;
in practice `build_postgres_production` always passes `Some(shared_pool)`.

Therefore: add a new internal `build_pg_backend_production_with_tools` that takes `pool: Arc<Pool>`
(not Option) and uses Pg stores unconditionally. The existing `build_backend_production_with_tools`
can be renamed to the fallback path, or kept as-is for the non-Pg case.

**Simplest safe approach**: In `build_backend_production_with_tools`, when `pg_pool.is_some()`,
construct pg-backed stores before the `HostRuntimeServices::new` call, and swap them in.
For the resource governor: pass `PersistentResourceGovernor<PgResourceGovernorStore>` to `::new()`
instead of `InMemoryResourceGovernor`. Since the type changes, we need two `HostRuntimeServices::new`
calls in separate branches, each building the full builder chain. Extract a `wire_common_services`
closure that handles everything after the governor/run-state assignment.

Actually the cleanest solution: add a dedicated `#[cfg(feature = "postgres")] async fn build_pg_backend_services_wired(...)`
that is the postgres-only path and wires everything correctly. Then `build_backend_production_with_tools`
stays as the non-Pg path and `build_postgres_production` calls `build_pg_backend_services_wired` directly.

### Step 6 — Run clippy and tests

```bash
cargo clippy -p brassclaw_reborn_composition --all-targets --all-features -- -D warnings
cargo test -p brassclaw_reborn_composition
cargo test -p brassclaw_run_state
cargo test -p brassclaw_turns
cargo test -p brassclaw_resources
cargo test -p brassclaw_authorization
```

### Step 7 — Mark PG-4 IMPLEMENTED in checkup.md, commit and push
