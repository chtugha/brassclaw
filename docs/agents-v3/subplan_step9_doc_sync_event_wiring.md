# Subplan: Phase P Step 9 — doc-sync Event Wiring

**Status:** COMPLETE (all steps implemented)
**Parent plan:** subplan_phase_p_steps_2_to_11.md Step 9
**Created:** Phase P implementation session

---

## Problem Statement

The `doc-sync` Action (seeded in Phase P Step 7) needs to fire automatically when:

1. **(a) File-change trigger**: a `docs/agents-v3/*.md` file changes on disk
2. **(b) DB-change trigger**: a `reborn_docus` row is updated (edited via WebUI or Sempai re-compression)

Currently neither pathway exists. There is no file-watching crate in the workspace and no Postgres LISTEN/NOTIFY channel for `reborn_docus`. The `brassclaw_triggers` crate is cron/schedule-based, not event-based.

**Design constraint (§5 of DOC_CONVERSION_MECHANISM_DESIGN.md):**
> No auto-refresh, no idle-time loop, no scheduled cadence, no boot trigger.

---

## Infrastructure survey

| Component | Status |
|-----------|--------|
| File-watching crate (`notify`, `hotwatch`, etc.) | ❌ None in workspace |
| Postgres LISTEN/NOTIFY from Rust | ❌ No existing path |
| `brassclaw_triggers` file-watch support | ❌ Schedule/cron only |
| `doc-sync` Action DB row | ✅ Seeded in Pass 16 |
| `TriggerRepository::upsert_trigger` | ✅ Exists in `brassclaw_triggers` |
| `RebornRuntime` background task slot | ✅ Pattern exists (trigger_poller) |
| `TriggerCompletionPolicy::CompleteAfterFirstFire` | ✅ Exists — enables one-shot triggers |

---

## Design Decision: Synthetic one-shot triggers

The simplest viable approach avoids building a new Action executor or ActionRunner. Instead:

### Dispatch mechanism
**Write a one-shot `TriggerRecord`** into the `TriggerRepository` on each change event, with:
- `schedule = TriggerSchedule::Cron("* * * * * *")` — fires every second (picked up on next tick)
- `completion_policy = TriggerCompletionPolicy::CompleteAfterFirstFire` — fires once, then `Completed`
- `prompt = "run doc-sync"` — the trigger poller sends this as user input; the intent system routes it to the `doc-sync` Action (via its intent examples)
- `creator_user_id = SYSTEM_RESERVED_ID` (the system user)
- Unique name per event type: `doc-sync::file-change` / `doc-sync::docus-change`

On `upsert_trigger`, if the same name already has a `Scheduled` row, the upsert is a no-op (dedup). So multiple rapid file changes collapse into one trigger fire.

### (a) File-watcher service
- Use the `notify` crate (cross-platform: `FSEvents` on macOS, `inotify` on Linux)
- Watch `docs/agents-v3/*.md` with debounce (500ms)
- On change: call `TriggerRepository::upsert_trigger` with `doc-sync::file-change`

### (b) Postgres NOTIFY listener
- Add a DB trigger on `reborn_docus` that calls `pg_notify('reborn_docus_changed', row.name)`
- New Postgres-listening task in composition layer using a dedicated `tokio_postgres` connection
- On NOTIFY: call `TriggerRepository::upsert_trigger` with `doc-sync::docus-change`

---

## Implementation Steps

### Step A — Add `notify` crate dependency
**Files:** `crates/brassclaw_reborn_composition/Cargo.toml`

Add:
```toml
notify = { version = "6", optional = true }
```

**Investigate first:** Check if `notify` v6 is already in the workspace transitive deps.

### Step B — Add DB trigger migration
**Files:** `crates/brassclaw_pg/migrations/V081__reborn_docus_notify.sql`

```sql
-- V081__reborn_docus_notify.sql
-- Add a NOTIFY trigger to reborn_docus so Postgres pushes a change signal
-- to listening composition processes when a doc row is inserted or updated.
-- Channel: 'reborn_docus_changed', payload: row name (slug).

CREATE OR REPLACE FUNCTION reborn_docus_notify_fn()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('reborn_docus_changed', NEW.name);
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS reborn_docus_notify_tg ON reborn_docus;
CREATE TRIGGER reborn_docus_notify_tg
    AFTER INSERT OR UPDATE ON reborn_docus
    FOR EACH ROW EXECUTE FUNCTION reborn_docus_notify_fn();
```

**Note:** V080 is already taken by `V080__allow_system_source_on_actions.sql`. Use **V081**.

### Step C — DocSyncWatcher struct
**Files:** `crates/brassclaw_reborn_composition/src/doc_sync_watcher.rs` (new)

```
DocSyncWatcher {
    trigger_repo: Arc<dyn TriggerRepository>,
    tenant_id: TenantId,
    system_user_id: UserId,
    agent_id: Option<AgentId>,
    project_id: Option<ProjectId>,
}

impl DocSyncWatcher {
    async fn fire_doc_sync(&self, trigger_name: &str, prompt: &str)
    // Creates a one-shot TriggerRecord + upserts it.
    // Dedup: if a Scheduled row with the same name exists, upsert is no-op.
}
```

### Step D — File-watcher task
**Files:** `crates/brassclaw_reborn_composition/src/doc_sync_watcher.rs` (same file)

```rust
pub fn spawn_doc_sync_file_watcher(
    docs_dir: PathBuf,
    watcher: Arc<DocSyncWatcher>,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()>
```

Uses `notify::RecommendedWatcher` with debounce (500ms). Shuts down cleanly on `shutdown`.

### Step E — Postgres LISTEN task
**Files:** `crates/brassclaw_reborn_composition/src/doc_sync_watcher.rs` (same file)

```rust
pub fn spawn_doc_sync_pg_listener(
    pg_url: String,           // dedicated non-pooled connection
    watcher: Arc<DocSyncWatcher>,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()>
```

Uses a non-pooled `tokio_postgres::connect()` for LISTEN (cannot share a pool connection).
Reconnects with exponential backoff on disconnection.

### Step F — Wire into factory / RebornRuntime
**Files:** `crates/brassclaw_reborn_composition/src/factory.rs`

After `seed_builtin_components` and trigger-poller setup:
1. If PG pool available + trigger_repository available:
   - Create `Arc<DocSyncWatcher>`
   - Spawn file-watcher task (if `docs/agents-v3/` dir exists)
   - Spawn PG listener task (if PG URL available)
   - Add both JoinHandles to the RebornRuntime shutdown sequence

### Step G — Tests
Add unit tests for:
- `DocSyncWatcher::fire_doc_sync()` inserts the expected `TriggerRecord`
- Upsert dedup: second call with same name while first is `Scheduled` → no duplicate
- Integration test (PG): trigger poller picks up the one-shot trigger and fires prompt

---

## Commit messages (per step)

- **Step A+B:** `feat(pg): V081 — reborn_docus NOTIFY trigger; add notify dep`
- **Step C+D+E:** `feat(composition): Phase P Step 9a — DocSyncWatcher + file-watcher + PG listener`
- **Step F:** `feat(composition): Phase P Step 9b — wire doc-sync event tasks at boot`
- **Step G:** `test: Phase P Step 9 — DocSyncWatcher contract tests`
