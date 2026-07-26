# Subplan: Step 6.3 / Step 8.1 — PgMontyVmSettingsStore + Monty VM lifecycle

## Status: ✅ IMPLEMENTED

`PgMontyVmSettingsStore` implemented and wired in `webui.rs`. All 4 API
methods (`get_monty_vm_settings`, `update_monty_vm_settings`, `restart_monty_vm`,
`get_monty_vm_status`) are functional (no longer 501). `max_turn_duration` from
DB is live-plumbed via `plan_stub_step63_max_duration_wiring.md` (Step 9.3).
All steps 1–6 complete.

## Problem (historical)

`get_monty_vm_settings`, `update_monty_vm_settings`, `restart_monty_vm`, and
`get_monty_vm_status` all return HTTP 501 (`Err(RebornServicesError::from_status(501))`).
No `PgMontyVmSettingsStore` exists. Settings are backed by `reborn_monty_vm_settings`
(V034) but nothing reads or writes that table.

The orchestrator uses `orchestrator_limits()` which is cached in a `OnceLock` at
first call from the env var `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS`.

## Scope boundary

The "kernel-owned lifecycle manager" described in the spec (drain + admission control,
`running`/`draining`/`restarting` states) is aspirational. The Monty VM is NOT a
persistent process — it runs per-turn inside the agent loop. There is no actual
`MontyVmState::Draining` to implement. A "restart" means:
- Read the current settings from DB
- Return state = Restarting (caller polls GET /status which immediately returns Running)

The practical implementation focuses on:
1. DB read/write for `reborn_monty_vm_settings`
2. Passing max_duration + limits per-run from DB settings instead of OnceLock cache
3. Status endpoint always returns `Running` (no persistent VM to drain)
4. `restart_monty_vm` flushes any local cache + returns `Restarting` state

## Steps

### Step 1 — Add `PgMontyVmSettingsStore` to `brassclaw_reborn_composition`

Create `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs`:

```rust
pub(crate) struct PgMontyVmSettingsStore {
    pool: Arc<PgPool>,
    tenant_id: String,
    agent_id: String,
}
```

Methods:
- `async fn get(user_id, project_id) -> Result<MontyVmSettings, PgError>`
  — SELECT from `reborn_monty_vm_settings` with scope filter; return compiled-in
  defaults if no row (first-run case).
- `async fn upsert(user_id, project_id, update: UpdateMontyVmSettingsRequest) -> Result<MontyVmSettings, PgError>`
  — INSERT … ON CONFLICT DO UPDATE for the editable fields.

Default values (when no row exists):
- `max_duration_secs`: 180 (3 min, env-var fallback)
- `max_allocations`: None
- `max_memory_bytes`: None
- `failure_rollback_threshold`: 3
- `prior_knowledge_token_budget`: 100_000
- `q4_retention_days`: 30
- `forensic_packet_retention_days`: 90
- `active_orchestrator_id`: None

### Step 2 — Wire into `RebornServicesApi`

In `brassclaw_reborn_composition/src/webui.rs` (or the appropriate composition
file that wires `RebornServicesApi`):
- When `pg_pool` is available: construct `PgMontyVmSettingsStore` and add it
  as `Arc<PgMontyVmSettingsStore>` to the service struct.

In `brassclaw_reborn_composition/src/runtime.rs` or wherever
`RebornServicesImpl` is constructed, add `monty_vm_settings_store` field.

### Step 3 — Implement `get_monty_vm_settings`, `update_monty_vm_settings`

In `RebornServicesImpl`:

```rust
async fn get_monty_vm_settings(&self, caller) -> Result<MontyVmSettingsResponse> {
    #[cfg(feature = "postgres")]
    if let Some(store) = &self.monty_vm_settings_store {
        let settings = store.get(&caller.user_id, &caller.project_id).await?;
        return Ok(MontyVmSettingsResponse { settings });
    }
    // DB-less: return compiled-in defaults
    Ok(MontyVmSettingsResponse { settings: default_monty_vm_settings() })
}
```

For `update_monty_vm_settings`: validate `active_orchestrator_id` if provided
(must be `Validated` status in `reborn_recipes`); then upsert.

### Step 4 — Implement `restart_monty_vm` + `get_monty_vm_status`

`restart_monty_vm`: Since there is no persistent VM to drain, this:
1. (If `force=true`) — no-op for now (no in-flight turns to abort at this layer)
2. Returns `MontyVmRestartResponse { state: MontyVmState::Restarting }`

`get_monty_vm_status`: Always returns `Running` (the VM runs per-turn).
`settings_hash`: SHA-256 of the serialized `MontyVmSettings` JSON.

### Step 5 — Pass DB settings to `orchestrator_limits()` per-run

The `orchestrator_limits()` OnceLock cache is wrong for DB-backed settings.
However, fully wiring DB settings per-run requires plumbing them through
the orchestrator execution stack (agent loop → script executor → Monty VM).

**Minimal correct approach**: Add an `AtomicU64` to the engine/orchestrator
for the max-duration that is updated by `update_monty_vm_settings`. The
`orchestrator_max_duration()` reads from this atomic instead of OnceLock.

Actually the minimal safe approach: keep OnceLock for the env-var default but
add a `SharedMontyVmSettings` (Arc<RwLock<Option<MontyVmSettings>>>) that is
updated by `update_monty_vm_settings`. The orchestrator reads from it.
Wire it in `build_reborn_runtime`.

But this is a larger architectural change. For Step 8.1 compliance the priority
is that the **API routes work** (not 501) and **settings persist to DB**. The
actual effect of changed `max_duration_secs` on running orchestrators is a
separate concern (the env var still works as override).

**Decision**: implement Steps 1-4 (DB CRUD + working API). Defer Step 5
(live-plumbing to orchestrator limits) to Step 9.3 (demote BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS).
Add a note to checkup.md that max_duration_secs is persisted but not yet live.

### Step 6 — Clippy, tests, mark checkup.md

- `cargo clippy -p brassclaw_reborn_composition --all-targets --all-features`
- `cargo test -p brassclaw_reborn_composition`
- Mark Step 6.3 as 🔸 PARTIAL with DB CRUD implemented (lifecycle live-plumbing deferred)
- Mark Step 8.1 as ✅ IMPLEMENTED (all 501 stubs replaced with working logic)
- Commit and push

## Files to touch

- `crates/brassclaw_reborn_composition/src/pg_monty_vm_settings.rs` (new)
- `crates/brassclaw_reborn_composition/src/lib.rs` (register module)
- `crates/brassclaw_reborn_composition/src/webui.rs` (wire store into RebornServicesImpl)
- `checkup.md` (update Step 6.3 and Step 8.1)
