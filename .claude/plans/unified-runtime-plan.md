# Plan: Unified Postgres-backed Runtime (Remove Profiles + DB Secrets)

**Date:** 2026-08-09  
**Scope:** Two phases — an immediate 2-hour bug-fix (Phase A) then a multi-sprint architectural consolidation (Phase B).

---

## Root Cause of the Immediate Bug

`build_local_dev()` in [`factory.rs:771`](crates/brassclaw_reborn_composition/src/factory.rs) always builds a `FilesystemSecretStore` whose `/secrets` virtual mount resolves to:

```
/var/lib/brassclaw/.brassclaw/reborn/db/tenants/__system__/users/__system__/secrets/
```

That directory is **never created**, so every `LlmKeyStore::put()` call fails and is mapped to `LlmConfigServiceError::Unavailable`. The provider's `api_key_required` flag stays `true` in the DB, and the live-reload path fails with *"requires API key env var … to be set"*.

`PgSecretStore` already exists in [`crates/brassclaw_secrets/src/pg_store.rs:194`](crates/brassclaw_secrets/src/pg_store.rs) and the `brassclaw_secrets` + `brassclaw_secrets_master` tables have existed since migration V003. Nothing currently wires it.

---

## Phase A — Wire PgSecretStore (immediate fix, ~2 hours)

**One file, one function:** [`crates/brassclaw_reborn_composition/src/factory.rs`](crates/brassclaw_reborn_composition/src/factory.rs)

### Change at `build_local_dev()` lines 770–773

Replace the unconditional `FilesystemSecretStore` construction with a branch on the `pg_pool` parameter (which is already present in the function signature at line 658 under `#[cfg(feature = "postgres")]`):

```rust
// BEFORE (lines 771–773):
let local_dev_secret_store =
    build_local_dev_secret_store(&root, Arc::clone(&local_dev_product_auth_filesystem))?;
let secret_store: Arc<dyn SecretStore> = local_dev_secret_store.clone();

// AFTER:
#[cfg(feature = "postgres")]
let secret_store: Arc<dyn SecretStore> = if let Some(pool) = pg_pool.as_ref() {
    let master_key = resolve_local_dev_secret_master_key(&root)?;
    Arc::new(brassclaw_secrets::PgSecretStore::new(
        (**pool).clone(),
        master_key,
        "default",
    )?)
} else {
    let local_dev_secret_store =
        build_local_dev_secret_store(&root, Arc::clone(&local_dev_product_auth_filesystem))?;
    local_dev_secret_store as Arc<dyn SecretStore>
};
#[cfg(not(feature = "postgres"))]
let secret_store: Arc<dyn SecretStore> = {
    let local_dev_secret_store =
        build_local_dev_secret_store(&root, Arc::clone(&local_dev_product_auth_filesystem))?;
    local_dev_secret_store as Arc<dyn SecretStore>
};
```

Key notes:
- `resolve_local_dev_secret_master_key(&root)` (already exists, line 1356) reads the `.reborn-local-dev-secrets-master-key` file and generates it if missing — **same key material, new storage backend**.
- `PgSecretStore::new()` returns `Result<Self, SecretError>` and `RebornBuildError` already has `Secret(#[from] SecretError)` (error.rs:32), so `?` propagation works.
- `PgSecretStore::new()` takes a `deadpool_postgres::Pool` (not `Arc<Pool>`); use `(**pool).clone()` to get the inner pool value as the existing `build_pg_runtime_stores()` does (factory.rs:457).
- `brassclaw_secrets::PgSecretStore` is already re-exported from `brassclaw_secrets::lib.rs:17` and importable without new Cargo.toml changes since `brassclaw_secrets` is already in scope.
- The `HostRuntimeServices::with_secret_store_dyn(Arc::clone(&secret_store))` call at line 785 consumes the same `secret_store` and gets the `PgSecretStore` automatically — no other wiring change needed.
- `RebornServices.secret_store` field at line 988 also captures it — `LlmKeyStore` in runtime.rs:675 gets it.

### No migration needed

`brassclaw_secrets` and `brassclaw_secrets_master` tables exist since V003. The `PgSecretStore` uses the same `SecretsCrypto` as the filesystem store with the same master key material — existing filesystem-stored secrets are not migrated, but since none exist on the test machine (proven by empty `brassclaw_secrets` table and no filesystem files), there is nothing to migrate.

### Credential broker (OAuth)

`FilesystemAuthProductServices` (line 821) still uses the filesystem-based `FilesystemCredentialBroker`. On the test machine no OAuth credentials are configured, so this is not blocking. Wiring `PgCredentialBroker` is deferred to Phase B-2. The immediate fix only replaces the `LlmKeyStore` path.

### Tests to add

Add a unit test in `crates/brassclaw_reborn_composition/src/factory.rs` (in the existing `#[cfg(test)]` block around line 2870):
- `build_local_dev_with_pg_pool_uses_pg_secret_store` — construct a `build_local_dev` call with a mock pool, verify `services.secret_store()` returns a store that can `put` and `read` without touching the filesystem.

---

## Phase B — Remove Runtime Profiles Entirely

**Goal:** One code path, always Postgres, no profile enum in the composition/storage-selection layer.

> **Distinction preserved:** `RuntimeProfile` in [`crates/brassclaw_host_api/src/runtime_policy.rs`](crates/brassclaw_host_api/src/runtime_policy.rs) (12-variant enum used by the per-invocation capability policy resolver and audit log) is **NOT deleted**. Only `RebornCompositionProfile` (3-variant enum that selects which store backends to build) is removed.

### Sprint B-1: Upgrade remaining in-memory stores in the hybrid path (~3 days)

Stores still in-memory in the hybrid `local_dev + PG` path. All PG implementations already exist:

| Store | Current (hybrid) | Target | File |
|-------|-----------------|--------|------|
| Turn State | `InMemoryTurnStateStore` | `PgTurnStateStore` | `build_pg_runtime_stores()` line 412 |
| Checkpoint State | `InMemoryCheckpointStateStore` | `PgCheckpointStateStore` | line 417 |
| Approval Requests | `InMemoryApprovalRequestStore` | `PgApprovalRequestStore` | line 422 |
| Capability Leases | `InMemoryCapabilityLeaseStore` | `PgCapabilityLeaseStore` | line 426 |
| Resource Governor | `InMemoryResourceGovernor` | `PersistentResourceGovernor` | lines 427–432 |
| Budget Gate Store | in-memory | `PgBudgetGateStore` | line 436 |
| Event Log | `InMemoryDurableEventLog` | `PgDurableEventLog` | line 440 |
| Audit Log | `InMemoryDurableAuditLog` | `PgDurableAuditLog` | line 443 |
| Trigger Repository | `InMemoryTriggerRepository` | `PostgresTriggerRepository` | line 456 |

**Approach (implemented):** In `build_reborn_runtime()` in [`crates/brassclaw_reborn_composition/src/runtime.rs`](crates/brassclaw_reborn_composition/src/runtime.rs), the hybrid path block (lines ~1692–1720) was upgraded. When `services.pg_pool.is_some()`, `build_pg_runtime_stores()` is called and PG stores are used for the entire tuple (turn state, checkpoint, loop checkpoint, thread service, budget sink, resource governor, budget gate, event log, audit log). `pg_stores = Some(...)` is set so the downstream approval/lease/broadcast extractions also use PG (priority order flipped to prefer `pg_stores` when set). `local_dev_turn_state` is set to `None` when `pg_stores.is_some()` so `EmptyApprovalTurnRunLocator` and `EmptyTriggerTurnSnapshotSource` are used.

**Bugs found and fixed during B-1:**

1. **Missing `version` column in `brassclaw_session_threads`** — V008 migration never added the `version BIGINT` column that `PgSessionThreadService::write_snapshot` references. Added [`V049__session_threads_version.sql`](crates/brassclaw_pg/migrations/V049__session_threads_version.sql) with `ALTER TABLE brassclaw_session_threads ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0`.

2. **`find_thread_id_for_run` jsonb parameter mismatch** — [`pg_turn_state_store.rs`](crates/brassclaw_turns/src/pg_turn_state_store.rs) was binding a `String` for `$2::jsonb` in the `payload->'runs' @> $2::jsonb` query. `tokio_postgres` encodes `String` as `text` (OID 25) but the prepared statement resolves the parameter as `jsonb` (OID 3802), causing `error serializing parameter 1` at runtime. Fixed by using `serde_json::Value` via `serde_json::json!([{"run_id": run_id_str}])` and removing the explicit `::jsonb` cast from the SQL.

### Sprint B-2: Wire PgCredentialBroker (~2 days)

Replace `FilesystemAuthProductServices` with `PgAuthProductServices` (or equivalent) when a PG pool is present:

- **File:** `crates/brassclaw_reborn_composition/src/factory.rs` lines 821–843
- When `pg_pool.is_some()`, build `PgCredentialBroker::new(pool, master_key, "default")` and pass it to the product auth composition instead of `FilesystemAuthProductServices`
- `PgCredentialBroker` exists at [`crates/brassclaw_secrets/src/pg_store.rs:467`](crates/brassclaw_secrets/src/pg_store.rs)

### Sprint B-3: Collapse build paths + remove RebornCompositionProfile (~4 days)

**Step 1:** Make `build_reborn_services()` require a PG pool — remove the local-dev-only path.

New signature shape:
```rust
pub async fn build_reborn_services(
    pool: Arc<deadpool_postgres::Pool>,
    reborn_home: &Path,
    runtime_policy: Option<ResolvedRuntimePolicy>,
    // ... other inputs
) -> Result<RebornServices, RebornBuildError>
```

**Step 2:** Delete `RebornCompositionProfile` enum from [`crates/brassclaw_reborn_composition/src/profile.rs`](crates/brassclaw_reborn_composition/src/profile.rs). Remove all references in:
- `crates/brassclaw_reborn_composition/src/factory.rs` — remove the `match input.profile` branch
- `crates/brassclaw_reborn_cli/src/runtime/mod.rs` — remove `BRASSCLAW_RUNTIME_PROFILE` env var parsing that maps to composition profile; keep `RuntimeProfile` parsing for the per-invocation policy
- `crates/brassclaw_reborn_cli/src/commands/runtime_profile.rs` — update or remove the `runtime-profile list` command
- Any `AGENTS.md` / deployment documentation referencing `local_dev`, `local_safe`, `local_yolo`

**Step 3:** Replace `BRASSCLAW_RUNTIME_PROFILE` env var (which currently controls composition) with a simpler flag:
- Remove `local_dev` / `local_yolo` as composition-level choices
- Keep `BRASSCLAW_RUNTIME_PROFILE` but scope it ONLY to the per-invocation `RuntimeProfile` used by the capability resolver (which `local_dev` remains a valid value for)
- Remove `composition_profile_from_legacy_env()` and `BRASSCLAW_REBORN_PROFILE`

**Step 4:** Remove `RebornStorageInput::LocalDev` variant. Make `RebornStorageInput` always `Postgres`.

**Step 5:** Remove `build_local_dev()` function. All composition flows through the direct PG path.

**Step 6:** Remove `LocalDevRootFilesystem` / `CompositeRootFilesystem` from the wiring path for storage. The workspace and skills filesystem (needed by the agent to read/write files) is **separate** from storage — it is wired through `HostRuntimeServices` mount views and is NOT affected. Only the storage-layer filesystem (secrets, credentials, subagent goals) is removed.

### Sprint B-4: Test cleanup (~2 days)

- Update ~60 test files that reference `RebornCompositionProfile`, `LocalDev`, `LocalDevYolo`, or `build_local_dev`
- Replace test fixtures that use in-memory stores with Postgres test stores (using `brassclaw_pg::test_pool()` or equivalent)
- Tests marked `#[cfg(feature = "integration")]` that already use PG are unaffected

---

## What is NOT Changed

| Item | Why it stays |
|------|-------------|
| `RuntimeProfile` (12-variant enum) | Per-invocation capability policy; used by resolver, audit log, approval gates — not a storage selector |
| `BRASSCLAW_RUNTIME_PROFILE` env var | Still valid for controlling invocation policy; only its mapping to composition profile is removed |
| Workspace filesystem (`LocalFilesystem`) | Agent file access; different from the storage-layer filesystem; stays |
| `BRASSCLAW_REBORN_HOME` | Still needed as root for workspace, skills, prompt files, the master key file |
| `local_dev_capability_policy()` | Safety invariant; still used for per-invocation trust policy regardless of profile removal |
| Embedded Postgres (`ManagedPostgres`) | Still used when `BRASSCLAW_PG_URL` is not set |

---

## Deployment Impact

**systemd unit** — no changes to `ReadWritePaths` or `ProtectSystem`. The master key file at `$BRASSCLAW_REBORN_HOME/.reborn-local-dev-secrets-master-key` still needs to be writable. After Phase B, rename this to `$BRASSCLAW_REBORN_HOME/.secrets-master-key` (remove `local-dev` from the name).

**AGENTS.md** — remove the `BRASSCLAW_RUNTIME_PROFILE` bootstrap tier entry (or update it to document only the invocation-policy use).

**`brassclaw runtime-profile list`** — the command becomes informational only (lists valid per-invocation `RuntimeProfile` values, with no effect on storage backend selection).

**`BRASSCLAW_REBORN_PROFILE` (legacy)** — remove entirely after B-3; emit a hard error at startup if set (rather than deprecation warning) once B-3 lands.

---

## Execution Order

```
Phase A  ← do immediately (fixes the test machine LLM key bug)
B-1      ← next sprint (makes all state durable across restarts)
B-2      ← after B-1 (makes OAuth credentials durable)
B-3      ← after B-1 + B-2 (removes the profile machinery)
B-4      ← after B-3 (test suite cleanup)
```

---

## Verification

**Phase A:**
```bash
# Build
cargo build -p brassclaw_reborn_composition --features "root-llm-provider postgres"
cargo clippy -p brassclaw_reborn_composition --all-targets -- -D warnings

# Deploy to test machine, configure openai_compatible via WebUI → verify secret appears in DB:
PSQL=… psql -c "SELECT name FROM brassclaw_secrets;"
# Should show: llm_provider_openai_compatible_api_key (or similar)

# Send a chat message → verify no "requires API key env var" warning in journalctl
```

**Phase B-1:**
```bash
cargo test -p brassclaw_reborn_composition --features integration
# Verify turn/approval/trigger state survives process restart on test machine
```

**Phase B-3:**
```bash
cargo build --release --bin brassclaw
cargo clippy --all -- -D warnings
# Verify BRASSCLAW_RUNTIME_PROFILE=local_dev still accepted (per-invocation policy)
# Verify BRASSCLAW_REBORN_PROFILE=local-dev triggers hard error
```
