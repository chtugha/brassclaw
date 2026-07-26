# Final Gaps Before v3 — Pre-v3 Wiring Plan

## Purpose

This plan collects all deferred "not yet wired" items identified during the
post-checkup readiness assessment. Everything here has its store/implementation
already present in the codebase — the gaps are pure wiring.

Steps are ordered: highest-impact / most isolated first, larger restructuring
last, gated items at the bottom.

---

## Steps

### Step 1 — Wire `PgBudgetGateStore` in `build_pg_runtime_stores()` ✅ DONE

**File:** `crates/brassclaw_reborn_composition/src/factory.rs`

`build_pg_runtime_stores()` constructs `InMemoryBudgetGateStore` with a
comment "No persistent budget gate store yet". `PgBudgetGateStore` is fully
implemented in `brassclaw_resources/src/pg_store.rs` and exported at crate
root. Budget-approval gates are advisory pauses that should survive process
restart on the pure-PG path.

**Action:**
- Replace `Arc::new(brassclaw_resources::InMemoryBudgetGateStore::new())` at
  factory.rs ~line 446 with
  `Arc::new(brassclaw_resources::PgBudgetGateStore::new(Arc::clone(&pool), "default"))`.
- Remove the "no persistent budget gate store yet" comment.
- Run clippy + tests, commit + push.

### Step 2 — Wire `PgExtensionInstallationStore` in the serve path ✅ DONE

**Files:** `crates/brassclaw_reborn_composition/src/factory.rs`,
`crates/brassclaw_reborn_composition/src/runtime.rs`

Extension install records are persisted via `FilesystemExtensionInstallationStore`
in `build_local_dev_extension_management()` (factory.rs ~line 862). This store
writes to the local-dev VFS. When a Postgres pool is available (hybrid serve
path or pure-PG path), extension installs should be backed by
`PgExtensionInstallationStore` so they survive process restart.

`PgExtensionInstallationStore` is fully implemented in
`crates/brassclaw_extensions/src/pg_store.rs`.

**Action:**
- In `build_local_dev_extension_management()`, when `pg_pool` is `Some`, swap
  `FilesystemExtensionInstallationStore` for `PgExtensionInstallationStore`.
- Ensure `ExtensionLifecycleService::new` and `restore_extension_lifecycle_state`
  both receive the PG-backed store.
- Run clippy + tests, commit + push.

### Step 3 — Wire `PgOutboundStateStore` in `build_reborn_projection_services()`

**File:** `crates/brassclaw_reborn_composition/src/projection.rs`

`build_reborn_projection_services()` passes `Arc::new(InMemoryOutboundStateStore::default())`
to `EventStreamManager::from_services()`. Notification targets (final-reply /
progress delivery preferences) are lost on restart. `PgOutboundStateStore` is
fully implemented in `crates/brassclaw_outbound/src/pg_store.rs`.

**Action:**
- Add an `Option<Arc<deadpool_postgres::Pool>>` parameter (or an
  `Arc<dyn OutboundStateStore>`) to `build_reborn_projection_services()`.
- When pool is `Some`, pass `Arc::new(PgOutboundStateStore::new(pool, "default"))`.
- Thread the pool through from `build_reborn_runtime` (where `pg_pool` is
  available).
- Run clippy + tests, commit + push.

### Step 4 — Wire `PgConversationStateStore` in the serve path

**Files:** `crates/brassclaw_conversations/src/pg_store.rs`,
`crates/brassclaw_reborn_composition/src/`

`PgConversationStateStore` is implemented but no reference to it exists anywhere
in `brassclaw_reborn_composition`. Conversation state (pairing, actor mapping) is
stored entirely in memory. This means conversation pairings are lost on restart.

Investigate where `ConversationStateRepository` is currently constructed and wired
in the composition layer, then replace with `PgConversationStateStore` when pool
available.

**Action:**
- Search for every `ConversationStateRepository` / `InMemoryConversationStateStore`
  construction in `brassclaw_reborn_composition`.
- For each: when `pg_pool` is `Some`, substitute `PgConversationStateStore`.
- Run clippy + tests, commit + push.

### Step 5 — Execute Option B: Delete `build_production_shaped` dead chain

**File:** `crates/brassclaw_reborn_composition/src/factory.rs`

`plan_stub_factory_production_path.md` documents three options. The pure-PG path
in `build_reborn_runtime` (via `PgRuntimeStores`) now covers everything the
`build_production_shaped` chain was designed for. Option B (delete) is safe.

**Dead chain to delete (~300 lines, 18 `#[allow(dead_code)]` items):**
- `build_production_shaped()`
- `build_postgres_production()`
- `build_pg_backend_production_with_tools()`
- `build_backend_production_with_tools()`  (non-PG fallback, also dead)
- `build_postgres_memory_tools()`
- `resolve_pg_embedding_provider()`  (only used by the above)
- `ProductionStoreBundle<F>` struct + impl
- `ProductionCredentialBundle` struct
- `RebornProductionWiring` struct
- `RebornProductionBuildContext` struct
- `production_wiring()` fn
- `validate_production_process_binding()` fn
- `planned_run_profile_resolver()` fn
- `production_config()` fn
- `FilesystemProductionHostRuntimeServices<F>` type alias
- `build_filesystem_production_host_runtime_services()` fn
- `FilesystemSecretCredentialStores<F>` struct + impl
- `build_filesystem_secret_credential_stores()` fn
- `ScopedFilesystemTriggerCreatorPairingHook<F>` struct + TriggerCreateHook impl
- `resolve_explicit_or_keychain_master_key()` fn  (check if used elsewhere first)
- `attach_hosted_mcp_runtime()` fn  (check if used elsewhere first)
- `require_product_auth_runtime_ports()` fn  (check if used elsewhere first)

After deletion: all 18 `#[allow(dead_code)] // Phase-5 factory wiring` annotations
should be gone. Update `plan_stub_factory_production_path.md` to mark Option B
executed.

**Action:**
- Delete the chain top-down, verify no external callers remain.
- For helpers that might be used by live code: check references first.
- Run clippy + tests, commit + push.

---

## Gated Steps (prerequisites not yet met)

These steps are documented here for completeness but must NOT be executed yet.

### Step 6 — PG-6: libSQL strip (gate: upgrade cycle complete)

Strip `migrate-from-libsql` feature, all `#[cfg(feature = "libsql")]` blocks,
delete `memory_doc_libsql_store.rs` stubs, remove `RebornConfigFile` file-path
readers.

**Blocked:** Must wait for all installs to complete the libSQL → PG migration.
Then execute PG-8 cleanup (file-based config removal) in the same pass.

### Step 7 — Step 6.10: Full `DocType` retirement (gate: PG-6/PG-8)

Delete all `DocType` variants, retire `MemoryDoc`/`Store` trait surface, drop
~100 test fixtures using `DocType`. Blocked on PG-6 completing.

### Step 8 — Budget event projection (gate: design decision)

Wire `broadcast_budget_event_sink` drain into an SSE / gateway event stream.
Requires a design decision on the projection owner. The sink is already
populated; only the consumer is missing.

---

## Status

| Step | Description | Status |
|------|-------------|--------|
| 1 | `PgBudgetGateStore` in `build_pg_runtime_stores` | ❌ NOT DONE |
| 2 | `PgExtensionInstallationStore` in serve path | ✅ DONE |
| 3 | `PgOutboundStateStore` in projection services | ❌ NOT DONE |
| 4 | `PgConversationStateStore` in serve path | ❌ NOT DONE |
| 5 | Delete `build_production_shaped` dead chain (Option B) | ❌ NOT DONE |
| 6 | PG-6 libSQL strip | ⛔ GATED |
| 7 | Step 6.10 DocType retirement | ⛔ GATED |
| 8 | Budget event projection | ⛔ NEEDS DESIGN |
