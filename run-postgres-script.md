# run-postgres-script.md — Execution Script for integrate-postgres.md

> **Purpose:** Break the full PostgreSQL migration plan (`integrate-postgres.md`) into
> discrete, independently-completable subtask segments sized to avoid context-window
> exhaustion. Each segment is a self-contained unit of work with explicit entry criteria,
> exit criteria, and a clippy/test validation gate before the next segment begins.
>
> **Source of truth:** `integrate-postgres.md`. This file only describes execution order,
> segment sizing, and critical ordering constraints — it does not duplicate spec detail.
> Always open `integrate-postgres.md` as the primary reference when executing a segment.
>
> **Status:** S13 complete. S14 not started.

---

## ⚠️ Critical Phase-Ordering Constraints

Before starting, every executor must internalize these non-obvious orderings:

### Phase 7 is implemented BEFORE Phase 6 merges

The plan's Phase numbering is NOT the implementation order for Phases 6 and 7.

```
Implement order:  0 → 1 → 2 → 3 → 4 → 5 → [7-code] → 6-merge → 8 → 9 → 10 → 11
                                                  ↑               ↑
                                        Write Phase 7        Only merge/land
                                        migration code       Phase 6 (libSQL
                                        while libSQL         removal) AFTER
                                        still exists         Phase 7 is green
```

**Why:** Phase 7 writes the libSQL→Postgres data migration module, gated behind
`migrate-from-libsql`. Phase 6 removes the unconditional `libsql` dep. If Phase 6
is merged before Phase 7's migration code exists, the `migrate-from-libsql`
feature references a deleted crate and the upgrade migration fails to compile.

**The CI gate** is `seed_libsql_then_migrate_asserts_all_rows_in_pg` — it must be
green before Phase 6 is merged.

### Phase 8 depends on Phases 2 + 6

File-based config removal (Phase 8) can only happen after the DB-backed config
(Phase 2) is wired and libSQL is removed (Phase 6).

### Phase 10 gates the migration release

All Phase 10 integration tests must be green before cutting the upgrade release.
The hardened-unit test (`MemoryDenyWriteExecute=yes` + `jit=off`) is a hard gate.

---

## Segment Map

| Seg | Phases | What it covers | Gate before next |
|-----|--------|----------------|------------------|
| S0 | Phase 0 | `brassclaw_embedded_postgres` crate | `cargo test -p brassclaw_embedded_postgres` |
| S1 | Phase 1a | `brassclaw_pg` crate + SQL migrations V000–V020 | `cargo test -p brassclaw_pg` |
| S2 | Phase 1b | SQL migrations V021–V026 + pgvector wiring | `cargo test -p brassclaw_pg` |
| S3 | Phase 2a | `db_config` module + `save_config_key` | `cargo test -p brassclaw_reborn_composition` |
| S4 | Phase 2b | Config subcommands + `ProviderRepo` → DB | `cargo clippy -p brassclaw_reborn_cli` + tests |
| S5 | Phase 3 | Secrets: `PgSecretStore`, `rewrap`, ceremonies | `cargo test -p brassclaw_reborn_config` + integration |
| S6 | Phase 4a | Runtime stores: run-state, approvals, turns, checkpoints | `cargo clippy --all` + unit tests |
| S7 | Phase 4b | Runtime stores: sessions, leases, resources, processes, extensions, event log | `cargo clippy --all` + unit tests |
| S8 | Phase 4c | Runtime stores: token settings, safety config, memory docs, retention sweep | `cargo clippy --all` + unit tests |
| S9 | Phase 4d | Trigger + conversation + outbound + subagent-goal stores | `cargo clippy --all` + unit tests |
| S10 | Phase 4e | Interceptor store + chat-memory + chunk/embedding wiring | `cargo clippy --all` + integration test |
| S11 | Phase 5 | Hooks rename + auth + factory wiring ✅ | `cargo clippy --all` + parity tests |
| S12 | Phase 7-code | Migration module (`migrate-from-libsql`) — written BEFORE Phase 6 merges ✅ | migration integration test green |
| S13 | Phase 6 | libSQL removal ✅ | `cargo build --release` clean |
| S14 | Phase 8 | File-based config removal | `cargo clippy --all` |
| S15 | Phase 9 | Systemd unit + docs | review gate |
| S16 | Phase 10 | Integration + E2E tests | all gates green → cut release |
| S17 | Phase 11 | `BRASSCLAW_REBORN_PROFILE` → three-knob refactor (independent track) | `cargo clippy --all` |

---

## Segment Specifications

### S0 — Embedded Postgres Crate

**integrate-postgres.md reference:** §2.2, Phase 0 checklist  
**Entry:** no codebase changes yet  
**Crates created:** `crates/brassclaw_embedded_postgres/`

Work items (from Phase 0 checklist):
- Create crate with `postgresql_embedded` dep, pinned PG 16
- `checksums.rs`: compiled-in SHA-256 list; verify after download; suppress env override
- `initdb`, `pg_ctl` lifecycle, `health.rs` retry loop
- Orphaned-server detection: `postmaster.pid` PID liveness check
- Explicit `shutdown()` method; `Drop` best-effort only
- Log rotation config in `postgresql.conf` (`jit=off` + log settings from §2.2)
- Unit tests (mock pg_ctl, verify `postgresql.conf` tuning)

**Exit gate:** `cargo test -p brassclaw_embedded_postgres` green; `cargo clippy -p brassclaw_embedded_postgres -- -D warnings` clean

---

### S1 — Migration Runner + SQL V000–V020

**integrate-postgres.md reference:** §3, §4.1–§4.20, Phase 1 checklist (first half)  
**Entry:** S0 complete  
**Crates created:** `crates/brassclaw_pg/`

Work items:
- Create `brassclaw_pg` crate: `PgPool` builder, `refinery` runner, history-reconciliation bootstrap (§3)
- Add `pgvector` crate dep to `brassclaw_pg/Cargo.toml`
- Write `V000__shared_triggers.sql` (shared `set_updated_at()` function + `CREATE EXTENSION IF NOT EXISTS vector`)
- Write `V001__settings.sql` through `V020__identities.sql` (original tables §4.1–§4.20)
  - V018 covers the 3 sibling `root_filesystem_*` tables
- Update `brassclaw_embedded_postgres/src/initdb.rs` to install pgvector shared library

**Exit gate:** `cargo test -p brassclaw_pg` green; fresh DB schema matches all §4.1–§4.20 DDL; re-run is idempotent

---

### S2 — SQL V021–V026 + pgvector VFS test

**integrate-postgres.md reference:** §4.21–§4.30, Phase 1 checklist (second half)  
**Entry:** S1 complete  
**Crates modified:** `crates/brassclaw_pg/`

Work items:
- Write `V021__triggers_local_access.sql` (§4.24 — renames `trigger_records` → `brassclaw_triggers`; history pre-seed guard)
- Write `V022__conversation_state.sql` (§4.25)
- Write `V023__outbound_state.sql` (§4.26, 4 tables)
- Write `V024__subagent_goals.sql` (§4.27)
- Write `V025__memory_chat_records.sql` (§4.29, including `run_id` index, `source_ref`, `forensic_packet_id`)
- Write `V026__forensic_packets.sql` (§4.28)
- Test: fresh DB gets all V000–V026; re-run idempotent
- Test: pre-existing hooks/settings tables don't trip refinery
- Test: `PostgresRootFilesystem::ensure_index` with `IndexKind::Vector { dim }` compiles; `Filter::VectorNearest` → pgvector `<=>` cosine query (§4.30.3 prerequisite)

**Exit gate:** `cargo test -p brassclaw_pg --features integration` green

---

### S3 — DB-Backed Config Core

**integrate-postgres.md reference:** §4.2, §5.4, §5.5, Phase 2 checklist  
**Entry:** S1 complete (does not need S2)  
**Crates modified:** `crates/brassclaw_reborn_composition/`

Work items:
- `brassclaw_reborn_composition::db_config` module: `load_config_snapshot`, `save_config_key`
- Confirm `db_config.rs` is NOT in `brassclaw_reborn_config` (boundary crate stays pure)
- `save_config_key` must call `reject_inline_secret(value)` before any DB write (§5.5 security note)
- `save_config_key` `ConfigWriteContext` enum + `EnvKeyWriteForbidden` guard for `*_env` suffix + `AgentSession` context
- Replace **runtime serve-path** `RebornConfigFile::load` callers with `db_config::load_config_snapshot`; retain `load()` behind `migrate-from-libsql` for §8.1 step 3 + §4.4 rewrap step 2
- Remove `toml_edit`, `fs4`, `tempfile` from `brassclaw_reborn_config/Cargo.toml`
- All Phase 2 unit tests (round-trip, write-gate, inline-secret guard, boolean/integer serialization)

**Exit gate:** `cargo test -p brassclaw_reborn_composition` green; `cargo clippy -p brassclaw_reborn_composition -- -D warnings` clean

---

### S4 — Config CLI + ProviderRepo

**integrate-postgres.md reference:** §6.2–§6.5, Phase 2 checklist (CLI items)  
**Entry:** S3 complete  
**Crates modified:** `crates/brassclaw_reborn_cli/`, `crates/brassclaw_reborn_composition/`

Work items:
- First-run wizard (`brassclaw config init --interactive`, §6.1)
- `brassclaw config` CRUD subcommands: `get`, `set`, `unset`, `list`, `show-all`, `export`, `import` (§6.3)
- `ProviderRepo` → DB-backed (`brassclaw_llm_providers` table §4.3)
- `sempai_provider.json` → `brassclaw_config` rows
- `ProviderRole::Embedding` variant added to `brassclaw_llm/src/role.rs` (§3)
- Three-role conflict check in `set_active()` (`llm_config_service.rs`) — Embedding may coexist with Kohai or Sempai; Kohai+Sempai still conflicts (§3)
- CLI lifecycle for DB-touching commands (§6.4): start embedded PG or detect/reuse live postmaster; run migrations; conditional shutdown

**Exit gate:** `cargo clippy -p brassclaw_reborn_cli -- -D warnings` clean; `cargo test -p brassclaw_reborn_cli` green

---

### S5 — Secrets: PgSecretStore + Rewrap

**integrate-postgres.md reference:** §4.4, §4.5 (secrets_master schema), Phase 3 checklist  
**Entry:** S3 complete  
**Crates modified:** `crates/brassclaw_reborn_config/`, `crates/brassclaw_reborn_composition/`

Work items:
- `PgSecretStore` and `PgCredentialBroker` (must implement both `CredentialAccountStore` and `CredentialSessionStore`)
- `brassclaw_secrets_master` table wiring (§4.4 schema: `tenant_id`, `version`, `wrapped_key`, `algorithm`)
- local-dev: 0600 raw key file at `$REBORN_HOME/.secrets-master-key`
- `brassclaw secrets rewrap` command with `--strategy passphrase|passphrase-file=<path>|keychain` and `--tenant <id>` flag
- `rewrap` 4-step tenant resolution (§4.4 R6-MH1)
- `rewrap --old-passphrase-file=<path>` for passphrase-change flow
- `rewrap` key-source rule (check `.reborn-local-dev-secrets-master-key` first, then `.secrets-master-key`; fail-closed)
- `rewrap` passphrase-change unwrap path
- Per-boot unwrap: ceremony-selector; `algorithm` consistency check; fail-closed guard
- Abstract secret reads: check `$CREDENTIALS_DIRECTORY` first, env second

**Exit gate:** `cargo test -p brassclaw_reborn_config` green; integration test: `rewrap` → `serve` → secret decrypts correctly

---

### S6 — Runtime Stores Batch A (run-state, approvals, turns, checkpoints)

**integrate-postgres.md reference:** §4.5–§4.7, §4.10 (approvals), Phase 4 checklist (first quarter)  
**Entry:** S2 + S3 complete  
**Crates modified:** `crates/brassclaw_run_state/`, `crates/brassclaw_approvals/`, `crates/brassclaw_turns/`, `crates/brassclaw_loop_support/`

Work items:
- `PgRunStateStore` (§4.5 — `RunStatus` has `#[serde(rename_all = "snake_case")]`)
- `PgApprovalRequestStore` (§4.10)
- `PgTurnStateStore` — **must implement all 5 traits**: `TurnStateStore`, `TurnSpawnTreeStateStore`, `TurnEventProjectionSource`, `LoopCheckpointStore`, `TurnRunTransitionPort`
- `PgLoopCheckpointStore` (in `brassclaw_loop_support`)
- `TurnStatus` adapter: use `heck::ToSnakeCase` (NOT `.to_lowercase()`) — `RecoveryRequired` → `"recovery_required"`, not `"recoveryrequired"` (§4.7 C2 note)
- FK invariant: write the same ULID into both `brassclaw_turns.id` and `brassclaw_turns.run_id`

**Exit gate:** `cargo test -p brassclaw_run_state -p brassclaw_approvals -p brassclaw_turns -p brassclaw_loop_support` green; `cargo clippy --all -- -D warnings` clean

---

### S7 — Runtime Stores Batch B (sessions, leases, resources, processes, extensions, event log)

**integrate-postgres.md reference:** §4.6, §4.8, §4.9, §4.11–§4.16, Phase 4 checklist (second quarter)  
**Entry:** S6 complete  
**Crates modified:** `crates/brassclaw_threads/`, `crates/brassclaw_authorization/`, `crates/brassclaw_resources/`, `crates/brassclaw_processes/`, `crates/brassclaw_extensions/`, `crates/brassclaw_reborn_event_store/`

Work items:
- `PgSessionThreadService` — `SessionThreadService` has **16 required methods** (§5.3 note)
- `PgCapabilityLeaseStore` — `CapabilityLeaseStatus` has no `rename_all`; app layer must lowercase variant names; partial index is `WHERE status = 'active'` only (§4.8 C1 note); expiry filter belongs in query WHERE clause
- `PgResourceGovernorStore` — CAS via `version` column conditional UPDATE; return `BudgetConflict` on 0-rows-affected
- `PgBudgetGateStore` (§4.22)
- `PgProcessStore` + `PgProcessResultStore`
- `PgExtensionInstallationStore`
- `PgDurableEventLog` + `PgDurableAuditLog` — **two-part rewrite**: direct SQL (not VFS fabric); shared `Arc<PgPool>`; update all 3 factory wiring sites (§4.14 M7 note)

**Exit gate:** `cargo test -p brassclaw_threads -p brassclaw_authorization -p brassclaw_resources -p brassclaw_processes -p brassclaw_extensions -p brassclaw_reborn_event_store` green; `cargo clippy --all -- -D warnings` clean

---

### S8 — Runtime Stores Batch C (token settings, safety config, memory docs, retention sweep)

**integrate-postgres.md reference:** §4.17, §4.18, §4.20, §4.21, Phase 4 checklist (third quarter)  
**Entry:** S7 complete  
**Crates modified:** `crates/brassclaw_reborn_composition/`, `crates/brassclaw_product_workflow/`

Work items:
- `PgTokenSettingsStore`
- `PgSafetyConfigStore` — one struct must implement **both** `SafetyConfigStore` AND `CapabilityPermissionStore`
- `PgMemoryDocStore` (§4.17) with generated-column GIN FTS
- Background retention sweep task in `brassclaw serve` only (§4.21): TTL-based pruning per table; forensic-packet pruning nulls-out `forensic_packet_id` on linked memory rows before deleting; chunk-cascade on memory-chat-records pruning (§4.30.2)
- `brassclaw maintenance prune-old-data` CLI command
- `verify brassclaw_root_filesystem` queries always scope to `tenant_id`; extend to support sibling tables (`_index_specs`, `_events`) (§4.19)

**Exit gate:** `cargo test -p brassclaw_reborn_composition -p brassclaw_product_workflow` green; `cargo clippy --all -- -D warnings` clean

---

### S9 — Runtime Stores Batch D (triggers, conversation, outbound, subagent goals, identity)

**integrate-postgres.md reference:** §4.23–§4.27, Phase 4 checklist (fourth quarter)  
**Entry:** S8 complete  
**Crates modified:** `crates/brassclaw_triggers/`, `crates/brassclaw_conversations/`, `crates/brassclaw_outbound/`, `crates/brassclaw_reborn/`, `crates/brassclaw_reborn_identity/`

Work items:
- `PostgresTriggerRepository`: remove `#[cfg(feature = "postgres")]` gate; update all 3 constants (`TRIGGER_TABLE` → `"brassclaw_triggers"`, `TRIGGER_COLUMNS`, `POSTGRES_TRIGGER_SCHEMA` → references `brassclaw_triggers`); remove `LibSqlTriggerRepository`
- `PgLocalTriggerAccessStore` (local-dev only, replaces `RebornLibSqlLocalTriggerAccessStore`)
- `PgConversationStateStore` — CAS via `revision` column; return `InboundTurnError::DurableState { reason }` on exhausted retries (NOT `ConflictRetry` — that variant does not exist; §4.25 C1 note)
- `PgOutboundStateStore` — implements **both** `OutboundStateStore` AND `CommunicationPreferenceRepository`; `updated_by` must always be a real `UserId`, never the empty-string default
- `PgSubagentGoalStore` — remove `filesystem-goal-store` feature gate
- `PgRebornIdentityStore` — wired in `brassclaw_reborn_composition/src/factory.rs` (not in `brassclaw_reborn_identity/factory.rs` — that file does not exist; §4.23)
- `PgResourceGovernorStore` CAS integration test for concurrent increments

**Exit gate:** `cargo test -p brassclaw_triggers -p brassclaw_conversations -p brassclaw_outbound -p brassclaw_reborn -p brassclaw_reborn_identity` green; `cargo clippy --all -- -D warnings` clean

---

### S10 — Interceptor + Chat-Memory + Embedding Wiring

**integrate-postgres.md reference:** §3, §4.28–§4.30, §6.2a, §7.4, Phase 4 (embedding checklist items)  
**Entry:** S9 complete  
**Crates modified:** `crates/brassclaw_interceptor/`, `crates/brassclaw_memory/`, `crates/brassclaw_embeddings/`, `crates/brassclaw_reborn_composition/`, `crates/brassclaw_host_runtime/`

Work items (do in this sub-order — each depends on the previous):

1. **`brassclaw_embeddings` crate refactor** — remove `EmbeddingsConfig`, `create_provider()`, concrete HTTP impls, `resolve_embeddings_config`, `Workspace::with_embeddings_*`; retain `EmbeddingProvider` trait + `EmbeddingError`, `CachedEmbeddingProvider`, `url_check`, `default_dimension_for_model`, `MockEmbeddings`; update `AGENTS.md`
2. **`brassclaw_memory` trait extension** — add `index_content(scope: &MemoryDocumentScope, source_ref, content, chat_record_id)` to `MemoryDocumentIndexer` trait; implement on `ChunkingMemoryDocumentIndexer`; add `fs_keys::CHAT_RECORD_ID` constant
3. **`EmbeddingRoleAdapter`** — new file `crates/brassclaw_reborn_composition/src/embedding_role_adapter.rs`; implements `brassclaw_memory::EmbeddingProvider` by delegating to inner `brassclaw_embeddings::EmbeddingProvider`; apply error mapping table from §3 (6-variant → 3-variant mapping); `url_check::check_base_url`; wrap in `CachedEmbeddingProvider`
4. **`PgInterceptorStore`** — replaces `NoopInterceptorStore` in production; add `link_chat_record(run_id, iteration, chat_record_id)` helper; update `store.rs` module-level doc comment (remove dual-backend mandate)
5. **`PgChatMemoryRecordStore`** — unconditional Path A write on every `memory_write`; `source_ref = NULL` initially; call `link_chat_record` after write; call `write_path_b_posthook` to update `source_ref` after chunk write
6. **`build_backend()` re-wiring** — resolve `embedding.provider_id` at composition startup; wire `EmbeddingRoleAdapter` into indexer + backend when active; add `embedding_active: bool` + `embedding_provider: Option<Arc<dyn brassclaw_memory::EmbeddingProvider>>` to `MemoryServices`; update `dispatch_search()` to use `.with_vector(services.embedding_active)`
7. **Three-role preconfiguration** — resolve `embedding.provider_id` alongside `kohai`/`sempai` at startup in `factory.rs`
8. **`brassclaw maintenance backfill-embeddings` CLI command** — reconstructs `MemoryDocumentScope` from row's `(tenant_id, user_id, agent_id, project_id)`; calls `indexer.index_content(scope, source_ref, content, chat_record_id)` for each; idempotent; handles dimension change
9. **Update call sites** in `src/app.rs`, `src/cli/mod.rs`, `src/cli/doctor.rs::check_embeddings` to remove `brassclaw_embeddings::create_provider` + `Workspace::with_embeddings_*`
10. **WebUI v2 `use for embedding` button** — third provider-role action button; `PUT /providers/{id}/role/embedding` endpoint; active/inactive highlight; "restart required" hint

**Exit gate:** `cargo clippy --all -- -D warnings` clean; `cargo test -p brassclaw_interceptor -p brassclaw_memory -p brassclaw_reborn_composition`; end-to-end embedding integration test (§4.30 revision 17 test) green

---

### S11 — Hooks Rename + Auth + Factory Wiring

**integrate-postgres.md reference:** §4 (hooks DDL), Phase 5 checklist
**Entry:** S10 complete
**Crates modified:** `crates/brassclaw_hooks_pg/`, `crates/brassclaw_reborn_composition/`

> **Status: All Phase 5 work items complete.** (S11 complete)

Work items (completed — retained for record):
- ~~Rename `brassclaw_hooks_postgres` → `brassclaw_hooks_pg`; update workspace `members` + all dependent `[dependencies]` entries~~
- ~~Strip `#[cfg(feature = "postgres")]` module declarations from `lib.rs` (not just `Cargo.toml` — the `lib.rs` cfg attributes must also go)~~
- ~~Remove `postgres` optional feature from `brassclaw_hooks_pg/Cargo.toml`; make `deadpool-postgres` + `tokio-postgres` unconditional~~
- ~~**Port** `brassclaw_hooks_parity` tests into `brassclaw_hooks_pg/tests/` (`parity_matrix.rs` + `multi_host_adversarial.rs`) before deleting the crate~~
- ~~Delete `brassclaw_hooks_libsql` and `brassclaw_hooks_parity` after port is CI-green~~
- ~~`PgAuthProductServices`~~
- ~~Wire `brassclaw_reborn_composition::factory` to single Postgres path; wire `PostgresPredicateStateBackend` via `PredicateEvaluator::with_state_backend(...)`~~
- ~~Pool drop before `managed_pg.shutdown().await` — `serve.rs` now starts `ManagedPostgres` (or uses `BRASSCLAW_PG_URL`), upgrades the runtime input to the Postgres production profile, and calls `managed_pg.shutdown().await` only after `runtime.shutdown()` has consumed the runtime and its pool~~

**Exit gate:** `cargo clippy --all -- -D warnings` clean; `cargo test -p brassclaw_hooks_pg` (parity + adversarial tests) green

---

### S12 — Migration Module (Phase 7 code — written BEFORE Phase 6 merges)

> **⚠️ Do not merge Phase 6 (S13) until this segment's CI gate is green.**

**integrate-postgres.md reference:** §8.1, Phase 7 checklist  
**Entry:** S11 complete; libSQL crates still present (Phase 6 not yet merged)  
**Crates modified:** `crates/brassclaw_reborn_composition/` (new `migration` module behind `migrate-from-libsql`)

Work items:
- `brassclaw_reborn_composition::migration` module, fully gated behind `#[cfg(feature = "migrate-from-libsql")]`
- Implement §8.1 steps 3–7: migrate `config.toml`, `providers.json`, `sempai_provider.json`, secrets master key (ceremony-aware), libSQL DB rows
- `migrate-from-libsql` default-on in workspace `Cargo.toml` for upgrade release (§9.1)
- `brassclaw migrate --dry-run` support (steps 3–10 in read-only simulation)
- Integration test: seed a libSQL DB, run migration, verify all rows land in PG (`seed_libsql_then_migrate_asserts_all_rows_in_pg`)
- Integration test: upgrade-flow decryption (seed libSQL + encrypted secret, `rewrap`, `serve`, assert decrypt succeeds)
- Integration test: non-default tenant upgrade (`--tenant mycorp`)
- Tests: `boot.initialized` fresh-install vs found-artifact behaviour

**Exit gate:** CI gate `seed_libsql_then_migrate_asserts_all_rows_in_pg` green; upgrade-flow decryption test green; non-default-tenant test green

---

### S13 — libSQL Removal (Phase 6 — merges ONLY after S12 is green)

> **⚠️ Only begin this segment after S12's CI gate is confirmed green.**

**integrate-postgres.md reference:** Phase 6 checklist, §9.1–§9.2  
**Entry:** S12 CI gate confirmed green  
**Crates deleted/modified:** many

Work items:
- Rebase `replay` feature onto embedded Postgres test rig (§9.2)
- Deprecate/remove `import` feature; file follow-up issue for OpenClaw port
- Delete all `#[cfg(feature = "libsql")]` and `#[cfg(not(feature = "libsql"))]` blocks
- Remove `libsql` from all `Cargo.toml` files (keep only `libsql = ["migrate-from-libsql"]` alias in workspace)
- Remove `libsql` from workspace `Cargo.toml` `default` array (leave `postgres`, `html-to-markdown`, `tui`)
- Update `brassclaw_reborn_composition/Cargo.toml` default features (remove `libsql`)
- Update `brassclaw_reborn_cli/Cargo.toml` default features (remove `libsql` from `["root-llm-provider", "libsql"]`)
- Delete `brassclaw_hooks_libsql` crate directory
- Delete `RebornLibSqlIdempotencyLedger` from `brassclaw_product_workflow_storage`
- Remove `RebornEventStoreConfig::Libsql`, `::InMemory`, `::Jsonl` variants; remove enum if empty
- Remove `#[cfg(feature = "filesystem-goal-store")]` from `subagent/goal_store.rs`
- Update `brassclaw_architecture` boundary tests

**Exit gate:** `cargo build --release --bin brassclaw` clean (no libSQL dep in default build); `cargo clippy --all -- -D warnings` clean

---

### S14 — File-Based Config Removal (Phase 8)

**integrate-postgres.md reference:** Phase 8 checklist  
**Entry:** S13 complete  
**Crates modified:** `crates/brassclaw_reborn_config/`, `crates/brassclaw_reborn_cli/`

Work items:
- Remove `config_file_path()`, `providers_file_path()`, `sempai_provider_file_path()` from `RebornHome`
- Remove `config.toml.lock`, `providers.json.lock` discipline from `ProviderRepo` and `DefaultLlmSlotUpdateSession`
- Remove or hollow out `DefaultLlmSlotUpdateSession` struct entirely (grep `DefaultLlmSlotUpdateSession` to find all call sites before deleting)
- Update `brassclaw_reborn_cli` `config init` → wizard

**Exit gate:** `cargo clippy --all -- -D warnings` clean; `cargo test` green

---

### S15 — Systemd Unit + Documentation (Phase 9)

**integrate-postgres.md reference:** §7, Phase 9 checklist  
**Entry:** S14 complete  
**Files modified:** `AGENTS.md`, `CLAUDE.md`, `CHANGELOG.md`, per-crate `AGENTS.md`/`CLAUDE.md`, new operator guide

Work items:
- Write `brassclaw.service` systemd unit template (§7.1/§7.2/§7.3, all hardening directives)
- Update `AGENTS.md` Database Rules section — retire dual-backend mandate (§0a)
- Update `crates/brassclaw_interceptor/src/store.rs` module-level doc comment (remove dual-backend statement)
- Purge stale v1 `src/` sections from `CLAUDE.md`
- Update `CLAUDE.md` env var table (two-tier model; remove retired vars)
- Write operator guide: prerequisites (§7.0), fresh-install (§7.1), upgrade (§7.2), `master.key` ownership, DR backup mandate, CLI-only users, `rewrap` vs `rotate`
- Update all per-crate `CLAUDE.md`/`AGENTS.md` spec files
- `CHANGELOG.md` entry
- Add architecture test: no `std::fs::read_to_string` / `File::open` in any non-migration production path

**Exit gate:** review gate (no automated test); ensure all checklist items ticked before proceeding to S16

---

### S16 — Integration Tests + E2E (Phase 10)

> **⚠️ All tests in this segment must be green before cutting the upgrade release.**

**integrate-postgres.md reference:** Phase 10 checklist  
**Entry:** S15 complete  

Work items:
- Integration test: full boot cycle from scratch (embedded PG starts, wizard runs, turn served, graceful shutdown)
- Integration test: restart resumes state from Postgres
- Integration test: `BRASSCLAW_PG_URL` override (no embedded PG spawned)
- Integration test: SIGKILL → restart → orphaned-server detection and reuse
- E2E: provider add/edit/delete via WebUI persists across restart
- **Hardened-unit integration test (hard gate):** embedded PG starts under `MemoryDenyWriteExecute=yes`, `SystemCallFilter=@system-service`, `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX`; validates `jit=off` prevents MDWE JIT crash
- Integration test: `brassclaw config get <key>` against running `brassclaw serve` does not stop embedded PG (§6.4 step 4)

**Exit gate:** ALL tests green, including hardened-unit test → cut upgrade release

---

### S17 — BRASSCLAW_REBORN_PROFILE Three-Knob Refactor (Phase 11)

> **Independent track — can be started after S5 (Phase 3) is complete. Does not block
> the upgrade release; ships in a subsequent release.**

**integrate-postgres.md reference:** Phase 11, §§11a–11d checklist  
**Entry:** S5 complete; may proceed in parallel with S6–S16  

Work items (in sub-phase order):
1. **Phase 11a** — Add new knobs (`BRASSCLAW_SAFETY_MODE`, `BRASSCLAW_EMBEDDED_PG`, etc.), keep old `BRASSCLAW_REBORN_PROFILE` working unchanged
2. **Phase 11b** — Deprecate old env var with a `warn!` on startup when it is set
3. **Phase 11c** — Remove old boot profile code and `BRASSCLAW_REBORN_PROFILE` support
4. **Phase 11d** — Update all files listed in Phase 11 checklist

**Exit gate:** `cargo clippy --all -- -D warnings` clean; `cargo test` green

---

## Dependency Graph (visual)

```
S0 ──► S1 ──► S2
       │
       ▼
       S3 ──► S4
       │
       ▼
       S5 ──────────────────────────────────────────► S17 (parallel)
       │
       ▼
       S6 ──► S7 ──► S8 ──► S9 ──► S10 ──► S11
                                           │
                                           ▼
                                           S12  ← implement Phase 7 migration code here
                                           │
                                           ▼ (only after S12 CI gate green)
                                           S13 (Phase 6 — libSQL removal)
                                           │
                                           ▼
                                           S14 ──► S15 ──► S16 → CUT RELEASE
```

> S2 can run in parallel with S3/S4/S5 (it only needs S1).  
> S6–S10 each need the previous; they cannot be parallelised.  
> S17 (Phase 11) is fully independent after S5 and ships in its own release.
