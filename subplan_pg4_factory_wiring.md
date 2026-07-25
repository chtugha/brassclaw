# Sub-plan: PG-4 Phase-5 Factory Wiring
## Wire build_reborn_runtime for Postgres production path

## Problem

`build_reborn_runtime` (called by `brassclaw serve`) enforces `LocalDev | LocalDevYolo` only
(lines 1554-1563 of `runtime.rs`). Any other profile returns an error:
  "profile={profile} is not yet wired end-to-end by build_reborn_runtime"

The postgres production path (`build_postgres_production`) builds a `RebornServices` with:
- `local_runtime: None` — no LocalDev substrate
- `pg_pool: Some(pool)` — shared Postgres pool
- All PG stores wired in `build_pg_backend_production_with_tools`

But `build_reborn_runtime` immediately derefs `services.local_runtime` after calling
`build_reborn_services`, crashing if it's None.

Additional gaps in `build_pg_backend_production_with_tools`:
1. Uses `ProcessServices::filesystem(...)` instead of `ProcessServices::new(PgProcessStore, PgProcessResultStore)`.
2. Uses `InMemoryResourceGovernor` (line 2372) before the `.with_pg_resource_governor()` call replaces it.
3. No `ProcessServices::postgres()` convenience constructor exists.

## What is NOT needed (already done)
- PgRunStateStore, PgApprovalRequestStore wired via `.with_pg_run_state()` ✅
- PgTurnStateStore wired via `.with_pg_turn_state_store()` ✅
- PgResourceGovernorStore wired via `.with_pg_resource_governor()` ✅
- PgCapabilityLeaseStore wired directly ✅
- PgSessionThreadService — implemented in `crates/brassclaw_threads` ✅ (not wired yet)
- PgConversationStateStore, PgOutboundStateStore — implemented ✅ (not wired)
- PgExtensionInstallationStore — implemented ✅ (not wired)

## Plan

### Sub-step 1 — Add `ProcessServices::postgres()` to `brassclaw_processes`
Add a convenience constructor on `ProcessServices<PgProcessStore, PgProcessResultStore>`:
```rust
impl ProcessServices<PgProcessStore, PgProcessResultStore> {
    pub fn postgres(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        let t = tenant_id.into();
        Self::new(
            Arc::new(PgProcessStore::new(Arc::clone(&pool), &t)),
            Arc::new(PgProcessResultStore::new(Arc::clone(&pool), &t)),
        )
    }
}
```

### Sub-step 2 — Fix `build_pg_backend_production_with_tools` to use PgProcessStore
Replace the `ProcessServices::filesystem(...)` call with `ProcessServices::postgres(...)`.
The `InMemoryResourceGovernor` at line 2372 is fine since `.with_pg_resource_governor()`
replaces it immediately; that path is already correct.

### Sub-step 3 — Add a `LocalRuntimeSurface` enum to `RebornServices`
`RebornServices` has `local_runtime: Option<LocalRuntime>` which is always `None` on the
postgres path. `build_reborn_runtime` needs access to:
- `thread_service: Arc<dyn SessionThreadService>` 
- `turn_state: Arc<dyn ...>` (for checkpoint/approval stores)
- `loop_checkpoint_store` / `checkpoint_state_store`

For the postgres path these come from:
- `PgSessionThreadService::new(pool, tenant_id)` for thread service
- `PgTurnStateStore` (already wired via `.with_pg_turn_state_store()` into host_runtime)

The cleanest approach: allow `build_reborn_runtime` to accept the postgres path by extracting
the needed stores from `services.pg_pool` when `local_runtime` is None.

### Sub-step 4 — Extend `build_reborn_runtime` to support postgres profiles
Remove the `LocalDev | LocalDevYolo` enforcement gate (lines 1554-1563). Instead:
- After `build_reborn_services(services_input)`, check for `pg_pool` when `local_runtime` is None
- Build PG-backed thread service, turn state store, checkpoint stores from pool
- Route approval locator to PG-backed turn state
- Continue with the rest of the runtime construction

The `local_runtime` references in the function body are for:
- `turn_state` — already in host_runtime for PG path
- `checkpoint_state_store` / `loop_checkpoint_store` — need PG equivalents
- `thread_service` — need `PgSessionThreadService`
- `extension_filesystem`, `skill_filesystem`, `workspace_filesystem` — postgres path uses VFS

### Sub-step 5 — Wire `PgSubagentGoalStore` on the postgres path
Currently uses `InMemoryBoundedSubagentGoalStore` always. When `pg_pool` is available, use
`PgSubagentGoalStore::new(pool, tenant_id)`.

### Sub-step 6 — Remove `#[allow(dead_code)]` from `build_pg_backend_production_with_tools`
Once wired through `build_reborn_runtime`, the dead_code allow can be removed.

### Sub-step 7 — Clippy clean + tests + update checkup.md

## Files to touch
- `crates/brassclaw_processes/src/services.rs` — add `ProcessServices::postgres()`
- `crates/brassclaw_reborn_composition/src/factory.rs` — fix ProcessServices in PG path
- `crates/brassclaw_reborn_composition/src/runtime.rs` — remove LocalDev-only gate, handle PG path
- `checkup.md` — update PG-4 note to reflect full wiring
