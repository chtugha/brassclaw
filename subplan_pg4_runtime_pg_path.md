# Subplan: PG-4 / factory-wiring — Extend `build_reborn_runtime` to the Postgres path

## Status: STEP 1-3 IMPLEMENTED (previous session); STEP 9.3 IMPLEMENTED (this session)

### Resolved (previous sessions)
- `PgSessionThreadService` wired in `build_reborn_runtime` when `services.pg_pool` is available
- `PgSubagentGoalStore` wired in `build_reborn_runtime` when `services.pg_pool` is available

### Resolved (this session)
- **Step 1**: `PgRuntimeStores` struct added to `factory.rs` — bundles all PG-backed stores needed
  by `build_reborn_runtime` on the pure-postgres path: `PgTurnStateStore`, `PgCheckpointStateStore`,
  `PgApprovalRequestStore`, `PgCapabilityLeaseStore`, `PersistentResourceGovernor<PgResourceGovernorStore>`,
  `InMemoryBudgetGateStore`, `BroadcastBudgetEventSink`, `PgDurableEventLog`, `PgDurableAuditLog`,
  `PostgresTriggerRepository`.
- **Step 1**: `build_pg_runtime_stores(pool, reborn_home)` constructor added — derives
  `local_dev_storage_root` + `default_system_prompt_path` from `reborn_home` the same way local-dev does.
- **Step 2 (trigger poller)**: `TriggerTurnSnapshotSource` impl added for
  `LocalTriggerTurnSnapshotSource<PgTurnStateStore>` in `trigger_poller.rs` — uses new
  `PgTurnStateStore::all_active_runs_snapshot()` to get all non-terminal runs in one query.
- **Step 3**: `PgTurnStateStore::all_active_runs_snapshot()` added — queries all snapshot rows
  and merges run arrays via JSONB `payload->'runs'`.
- **Gate removed**: LocalDev-only gate removed from `build_reborn_runtime`. Non-LocalDev profiles
  are no longer rejected at the gate; instead they fail with a clear actionable error message when
  `services.local_runtime` is None, referencing this subplan for the remaining work.
- Test updated: `runtime_rejects_disabled_profile_before_local_substrate_lookup` updated to check
  the "resolved runtime policy" error (disabled profile has no runtime_policy).

### Remaining
- `turn_state`, `loop_checkpoint_store`, `checkpoint_state_store` still InMemory (hybrid path)
- `run_state`, `approval_requests`, `capability_leases` in HostRuntimeServices still InMemory (hybrid path)
- `trigger_repository` still InMemory (Arc aliasing prevents simple replacement on hybrid path)
- Full pure-PG composition path (Steps 4–9 below) returns "not yet fully wired" error until done
- `build_pg_backend_production_with_tools` + `build_postgres_production` remain `#[allow(dead_code)]`

## Actual Architecture (discovered in checkup session)

The `brassclaw serve` command does NOT pass a production postgres profile to `build_reborn_runtime`.
Instead it uses the **hybrid local-dev+PG path**: `RebornBuildInput` with `LocalDev` profile +
`RebornStorageInput::Postgres`. This goes through `build_reborn_services` → hybrid branch →
`build_local_dev(local_input)` + injects `pg_pool`. The `local_runtime` IS present with all
InMemory stores. The profile guard at runtime.rs lines 1554-1563 is never hit.

`build_pg_backend_production_with_tools` and `build_postgres_production` are dead code
(`#[allow(dead_code)]` + "Phase-5 factory wiring") — the full-PG factory path is not yet live.

## Core Problem (revised)

The hybrid path builds InMemory stores everywhere. Even with `pg_pool` injected, the turn-runner
and turn-coordinator use `InMemoryTurnStateStore` from `local_runtime.turn_state`. Thread service
uses `InMemorySessionThreadService`. Loop checkpoints are InMemory.

The reason this is hard to fix: `DefaultPlannedRuntimeParts<T>` requires `T` to be a concrete
type satisfying `T: TurnSpawnTreeStateStore + TurnRunTransitionPort`. Rust's type system does not
support `Arc<dyn TraitA + TraitB>` unless one is a supertrait of the other (no upcasting for
non-auto traits).

## Dependency Inventory

`build_reborn_runtime` currently references `local_runtime` for 30 fields.
Each one needs a PG-backed substitute or a safe no-op on the postgres path:

| Field | Local-dev value | PG substitute |
|-------|-----------------|---------------|
| `turn_state` | `LocalDevTurnStateStore` | `PgTurnStateStore` — already wired in `build_pg_backend_production_with_tools` via `.with_pg_turn_state_store()` but not accessible outside `host_runtime`. Must be constructed separately here. |
| `checkpoint_state_store` | `FilesystemCheckpointStateStore` | `PgCheckpointStateStore::new(pool, "default")` — crate `brassclaw_turns` |
| `loop_checkpoint_store` | `FilesystemLoopCheckpointStore` | `PgLoopCheckpointStore::new(pool, "default")` — crate `brassclaw_turns` |
| `thread_service` | `FilesystemSessionThreadService` | `PgSessionThreadService::new(pool, "default")` — crate `brassclaw_threads` |
| `broadcast_budget_event_sink` | `BroadcastBudgetEventSink` | New `BroadcastBudgetEventSink::default()` — pure in-memory, same as local-dev. |
| `resource_governor` | `LocalDevResourceGovernor` | `PersistentResourceGovernor<PgResourceGovernorStore>` via host_runtime's wired governor — but we need the Arc. We create a fresh `PgResourceGovernorStore::new(pool, "default")` wrapped in `PersistentResourceGovernor` and pass it in. |
| `budget_gate_store` | `InMemoryBudgetGateStore` | Keep `InMemoryBudgetGateStore` on PG path (no persistent gate store currently). |
| `approval_requests` | `LocalDevApprovalRequestStore` | `PgApprovalRequestStore::new(pool, "default")` — crate `brassclaw_approvals`. Already wired into host_runtime, but we need a separate Arc for the runtime's `approval_interaction_service`. |
| `capability_leases` | `LocalDevCapabilityLeaseStore` | `PgCapabilityLeaseStore::new(pool, "default")` — crate `brassclaw_authorization`. Already wired into host_runtime. |
| `workspace_mounts`, `skill_mounts`, `memory_mounts` | Local VFS paths | Empty `MountView::empty()` — no local mount points on postgres path. |
| `local_dev_storage_root` | `~/.brassclaw/reborn/…` path | Default prompt path from env/compile-time constant. |
| `default_system_prompt_path` | path to `default.py` | Same: from `brassclaw_reborn_home` env-derived default path. |
| `event_log` | `FilesystemDurableEventLog` | Already wired: `services.host_runtime` contains the PG-backed `DurableEventLog`. Needs to be extracted. |
| `audit_log` | `FilesystemDurableAuditLog` | Same: already wired via `host_runtime`. |
| `extension_filesystem` | `LocalDevRootFilesystem` | No on-disk extension discovery on PG path — pass `None` to hook discovery. |
| `skill_filesystem` | `ScopedFilesystem<LocalDevRootFilesystem>` | Not needed on PG path (DB-backed skill source). |
| `workspace_filesystem` | `ScopedFilesystem<LocalDevRootFilesystem>` | Needed only for `LocalDevSkillExecutionAdapter`. On PG path: pass `None` for skill context source so `local_dev_filesystem_skill_context_source` is never called. |
| `content_cache_slot` | `CurrentCacheBridgeSlot` | New `CurrentCacheBridgeSlot::default()`. |
| `plan_state_slot` | `CurrentPlanStateSlot` | New `CurrentPlanStateSlot::default()`. |
| `trigger_repository` | `LocalDevFilesystemTriggerRepository` | `PostgresTriggerRepository::new(pool.into_inner())` — crate `brassclaw_triggers`. |
| `trigger_conversation_services` | lazy-init `RebornFilesystemConversationServices` | Not usable without filesystem. On PG path: trigger conversation init must use PG. |

## Steps

### Step 1 — Introduce `RebornRuntimeStores` enum

Create an enum in `factory.rs` (or a new file `runtime_stores.rs`) that carries
either the local-dev substrate or the PG substrate:

```rust
pub(crate) enum RebornRuntimeStores {
    LocalDev(Arc<RebornLocalRuntimeServices>),
    Postgres(PgRuntimeStores),
}
```

Where `PgRuntimeStores` bundles:
```rust
pub(crate) struct PgRuntimeStores {
    pub(crate) pool: Arc<deadpool_postgres::Pool>,
    pub(crate) turn_state: Arc<PgTurnStateStore>,
    pub(crate) checkpoint_state_store: Arc<dyn CheckpointStateStore>,
    pub(crate) loop_checkpoint_store: Arc<dyn LoopCheckpointStore>,
    pub(crate) thread_service: Arc<dyn SessionThreadService>,
    pub(crate) approval_requests: Arc<PgApprovalRequestStore>,
    pub(crate) capability_leases: Arc<PgCapabilityLeaseStore>,
    pub(crate) resource_governor: Arc<dyn ResourceGovernor>,
    pub(crate) budget_gate_store: Arc<dyn BudgetGateStore>,
    pub(crate) broadcast_budget_event_sink: Arc<BroadcastBudgetEventSink>,
    pub(crate) event_log: Arc<dyn DurableEventLog>,
    pub(crate) audit_log: Arc<dyn DurableAuditLog>,
    pub(crate) local_dev_storage_root: PathBuf,
    pub(crate) default_system_prompt_path: PathBuf,
    pub(crate) subagent_goal_store: Arc<dyn SubagentGoalStore>,
}
```

### Step 2 — Add `build_pg_runtime_stores` constructor

In `factory.rs`, add:

```rust
#[cfg(feature = "postgres")]
pub(crate) async fn build_pg_runtime_stores(
    pool: Arc<deadpool_postgres::Pool>,
    reborn_home: &RebornHome,
) -> Result<PgRuntimeStores, RebornBuildError>
```

This function constructs all the PG-backed stores listed in `PgRuntimeStores`.
For `event_log` / `audit_log`: use `PgDurableEventLog::new(pool, "default")` and
`PgDurableAuditLog::new(pool, "default")`.
For `default_system_prompt_path` / `local_dev_storage_root`: derive from `reborn_home`
the same way the local-dev path does.

### Step 3 — Thread `RebornRuntimeStores` through `build_reborn_services`

`build_reborn_services` builds `RebornServices`. Currently `RebornServices` has
`local_runtime: Option<Arc<RebornLocalRuntimeServices>>`. The PG path sets it to `None`.

**Option A (minimal):** Keep `local_runtime: Option<Arc<RebornLocalRuntimeServices>>` on
`RebornServices` as-is. Call `build_pg_runtime_stores(pool, home)` directly inside
`build_reborn_runtime` when `services.pg_pool.is_some() && services.local_runtime.is_none()`.
This avoids touching `RebornServices` struct at all.

**Use Option A.** It is the minimal safe change.

### Step 4 — Modify `build_reborn_runtime` gate + body

Remove the hard gate at lines 1554–1563 that rejects non-LocalDev profiles.
Replace with:

```rust
// Resolve runtime stores: local-dev has a full local substrate;
// postgres production builds PG-backed equivalents.
let (turn_state_store, checkpoint_state_store, loop_checkpoint_store,
     thread_service, approval_requests, capability_leases,
     resource_governor, budget_gate_store, broadcast_budget_event_sink,
     event_log, audit_log, local_dev_storage_root, default_system_prompt_path,
     is_local_dev) = if let Some(lr) = services.local_runtime.as_ref() {
    // local-dev path (unchanged)
    (...)
} else {
    // postgres path
    #[cfg(feature = "postgres")]
    {
        let pool = services.pg_pool.as_ref().ok_or(...)?.clone();
        let pg_stores = build_pg_runtime_stores(Arc::clone(&pool), ...).await?;
        (...)
    }
    #[cfg(not(feature = "postgres"))]
    return Err(RebornRuntimeError::InvalidArgument { reason: "..." });
};
```

### Step 5 — Resolve all `local_runtime.X` references in the body

Walk each of the 30 references:

1. **Skill context source** (lines 1661–1677):
   `local_dev_filesystem_skill_context_source(local_runtime, ...)` — On PG path,
   leave `configured_skill_context_source` as `None`; the caller (CLI `serve` command)
   already sets `skill_context_source: None` for the postgres path, so no change needed.
   Guard the call: `if let Some(local_runtime) = &services.local_runtime { ... }`.

2. **Budget accountant** (lines 1781–1828):
   `local_runtime.broadcast_budget_event_sink`, `local_runtime.resource_governor`,
   `local_runtime.budget_gate_store` → use the PG-path equivalents from `PgRuntimeStores`.

3. **Loop exit evidence** (line 1830): uses `thread_service` + `turn_state_store` —
   already replaced by PG equivalents above.

4. **Event log + audit log** (lines 1836–1837): from `PgRuntimeStores`.

5. **Content cache slot** (line 1897): `local_runtime.content_cache_slot` →
   use `CurrentCacheBridgeSlot::default()` on PG path.

6. **Hook discovery** (lines 1926–1927): `local_runtime.extension_filesystem` →
   pass `None` for `ThirdPartyDiscoveryInput.filesystem` on PG path.

7. **Identity context source** (lines 2049–2054): `local_runtime.local_dev_storage_root`
   + `local_runtime.default_system_prompt_path` → from `PgRuntimeStores`.

8. **Approval interaction** (lines 2107–2133):
   `local_runtime.approval_requests`, `local_runtime.capability_leases`,
   `local_runtime.workspace_mounts`, `local_runtime.skill_mounts`, `local_runtime.memory_mounts`
   → Use PG-path stores. `workspace_mounts`/`skill_mounts`/`memory_mounts` →
   `MountView::empty()`.

9. **Trigger poller** (lines 2175–2197): `local_runtime.trigger_repository` →
   `PgRuntimeStores.trigger_repository` (to be added to `PgRuntimeStores`).
   `build_trigger_poller_services(local_runtime, ...)` needs a refactor to accept
   the thread_service + approval_requests separately instead of an `&RebornLocalRuntimeServices`.

10. **Budget event projection** (lines 2228–2236): `local_runtime.broadcast_budget_event_sink`
    → PG equivalent.

11. **Plan library** (lines 2240–2254): `local_runtime.extension_filesystem` /
    `local_runtime.plan_state_slot` → `None`/`None` on PG path (plan library
    disabled without local filesystem).

12. **`build_trigger_poller_services` signature** (line 2175 call): currently takes
    `local_runtime: &RebornLocalRuntimeServices`. Must be refactored to take
    individual store arguments (thread_service, approval_requests + other deps).

13. **Display previews** (line 2143): `local_dev_capabilities.display_previews` —
    only built from `local_dev::capability_wiring(...)` call. On PG path, need
    a different capability wiring path. This is the largest change.

### Step 6 — Refactor `local_dev::capability_wiring` to be profile-agnostic

`local_dev::capability_wiring` is the linchpin — it returns a bundle that contains
the capability factory, input resolver, result writer, model gateway, AND display
previews. On the PG path we need the same bundle but without local-dev display
previews.

**Approach:** Extract a new `production_capability_wiring(...)` function (or add
a flag to the existing one) that omits display previews and uses PG-backed stores.
The model gateway + capability factory are already profile-agnostic; display_previews
is the only local-dev-specific output.

Alternatively, pass `display_previews: Option<...>` through the output struct and
leave `None` on the PG path. `with_display_previews` already accepts an Arc —
check if it can handle an empty/no-op value.

### Step 7 — Refactor `build_trigger_poller_services` signature

Change signature from:
```rust
async fn build_trigger_poller_services(
    local_runtime: &RebornLocalRuntimeServices,
    turn_coordinator: ...,
    thread_service: ...,
    ...
) -> ...
```
to:
```rust
async fn build_trigger_poller_services(
    approval_requests: Arc<dyn ApprovalRequestStore>,
    event_log: Arc<dyn DurableEventLog>,
    audit_log: Arc<dyn DurableAuditLog>,
    turn_coordinator: ...,
    thread_service: ...,
    ...
) -> ...
```
Keep backward compat by extracting values from `local_runtime` at the call site.

### Step 8 — Wire `PgSubagentGoalStore` on the postgres path

Sub-step 5 from factory_wiring plan: on the PG path, use
`PgSubagentGoalStore::new(Arc::clone(&pool), "default")` instead of
`InMemoryBoundedSubagentGoalStore::new()`.

### Step 9 — Remove `#[allow(dead_code)]` guards

After all the wiring is live:
- Remove `#[allow(dead_code)]` from `build_pg_backend_production_with_tools`
- Remove `#[allow(dead_code)]` from `build_postgres_production`
- Remove `#[allow(dead_code)]` from `build_postgres_memory_tools`

### Step 10 — Clippy + tests

```bash
cargo clippy -p brassclaw_reborn_composition --all-targets --all-features -- -D warnings
cargo test -p brassclaw_reborn_composition
cargo test -p brassclaw_turns
cargo test -p brassclaw_threads
cargo test -p brassclaw_processes
cargo test -p brassclaw_resources
cargo test -p brassclaw_authorization
```

## Files to touch

- `crates/brassclaw_reborn_composition/src/factory.rs` — add `PgRuntimeStores` struct + `build_pg_runtime_stores`
- `crates/brassclaw_reborn_composition/src/runtime.rs` — remove LocalDev gate, refactor all 30 `local_runtime` refs
- `crates/brassclaw_reborn_composition/src/runtime.rs` — refactor `build_trigger_poller_services` signature
- `crates/brassclaw_reborn_composition/src/local_dev.rs` — check if `capability_wiring` needs PG path variant
