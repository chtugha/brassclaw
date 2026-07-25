# BrassClaw — Implementation Checkup Plan

> **Purpose:** A merged, ordered implementation checklist reflecting the current
> intended state of the codebase according to all three source plans:
> - `integrate-postgres.md` (PostgreSQL migration foundation, Phases 0–11)
> - `.zenflow/tasks/i-want-you-to-make-a-plan-to-fun-80d7/plan.md` (Design transition Phases 1–8)
> - `run-intent-script.md` (Execution script, Steps 0–9 — supersedes design-transition steps where noted)
>
> Steps from later plans that update an earlier step are folded **in-place** at the
> position of the step they replace. Deprecated or removed steps are omitted.
> Each entry: short header + one-line description.
>
> **Status badges:**
> - ✅ `IMPLEMENTED` — confirmed present and wired in codebase
> - 🔸 `PARTIAL` — core structure present but gaps/stubs remain
> - ❌ `NOT IMPLEMENTED` — no evidence of implementation found
> - ⚠️ `FLAG` — present but suspect/incorrect/needs closer inspection

---

## Foundation — PostgreSQL Migration (`integrate-postgres.md`)

### PG-0 — Embedded Postgres crate (`brassclaw_embedded_postgres`) ✅ `IMPLEMENTED`
Create the `brassclaw_embedded_postgres` crate: PG 16 download + SHA-256 checksum verification, `initdb`, `pg_ctl` lifecycle, orphaned-server detection, explicit `shutdown()`, pgvector library bundling, and log-rotation config.

> ✅ `crates/brassclaw_embedded_postgres/` confirmed present: `ManagedPostgres` struct, checksum verification, orphaned-server detection, pgvector bundling, `shutdown()`.

### PG-1 — Schema and migration runner (`brassclaw_pg`) ✅ `IMPLEMENTED`
Create the `brassclaw_pg` crate: write migration files V000–V026 (V000 = `CREATE EXTENSION vector` + `set_updated_at()`; V001–V020 = core domain tables; V021 = trigger rename; V022–V026 = conversation/outbound/subagent-goals/chat-memory/forensic-packets), refinery runner with history-reconciliation pre-seed, `PgPool` builder, and pgvector integration tests.

> ✅ Migrations V000–V046 all present (V045 + V046 are beyond plan scope — expected ahead-of-plan). History-reconciliation pre-seeding in `migrations.rs`. Note: the codebase has **two extra migrations** (V045, V046) not described in any source plan.

### PG-2 — Config migration ✅ `IMPLEMENTED`
Implement `brassclaw_reborn_composition::db_config` (`load_config_snapshot`, `save_config_key` with `ConfigWriteContext` gate + `reject_inline_secret` guard); replace runtime serve-path `RebornConfigFile::load` callers; retain `load()` behind `migrate-from-libsql` for upgrade path; implement `ProviderRepo` → DB-backed; add first-run wizard (`brassclaw config init --interactive`) and CRUD subcommands.

> ✅ `db_config.rs`: `save_config_key`, `load_config_snapshot`, `ConfigWriteContext`, `reject_inline_secret`, `InlineSecretForbidden`, `EnvKeyWriteForbidden` all present.
> ✅ `llm_reload.rs` migrated: `RebornLlmReloadAdapter` now reads from `db_config::load_config_snapshot()` when a Postgres pool is wired (pg-2 gap resolved). Pool + tenant_id wired in `webui_llm_reload_adapter()` in `runtime.rs`.
> ✅ `llm_config_service.rs` gap resolved: `PgProviderRepo` is now constructed and wired via `with_pg_provider_repo()` in `webui.rs` when a Postgres pool is available. File-based `ProviderRepo` retained as fallback for non-postgres builds.

### PG-3 — Secrets migration ✅ `IMPLEMENTED`
Implement `PgSecretStore` + `PgCredentialBroker` (both `CredentialAccountStore` + `CredentialSessionStore` traits); `brassclaw secrets rewrap` command with passphrase/passphrase-file/keychain strategies, `--tenant` flag, `--old-passphrase-file` flag, 4-step tenant resolution, key-source fail-closed rule; per-boot ceremony selector; `brassclaw_secrets_master` schema.

> ✅ `crates/brassclaw_secrets/src/pg_store.rs`: `PgSecretStore` + `PgCredentialBroker` (both traits). `brassclaw_reborn_cli/src/commands/secrets.rs`: `rewrap` with `--old-passphrase-file`, passphrase strategies confirmed.

### PG-4 — Runtime store migrations 🔸 `PARTIAL`
Implement all Pg store replacements (one crate at a time):
- `PgRunStateStore`, `PgApprovalRequestStore`
- `PgTurnStateStore` (5 traits) + `PgLoopCheckpointStore`, `PgCheckpointStateStore`
- `PgSessionThreadService` (16-method trait)
- `PgCapabilityLeaseStore`, `PgResourceGovernorStore` (CAS via `version`), `PgBudgetGateStore`
- `PgProcessStore` + `PgProcessResultStore`, `PgExtensionInstallationStore`
- `PgDurableEventLog` + `PgDurableAuditLog` (re-routed to shared `PgPool`, 3 factory wiring sites)
- `PgTokenSettingsStore`, `PgSafetyConfigStore` (2 traits), `PgMemoryDocStore` (GIN FTS)
- `PgRebornIdentityStore` (3-table V020 schema), `PgLocalTriggerAccessStore`
- `PostgresTriggerRepository` promoted unconditional (update 3 string constants), trigger V021 DDL
- `PgConversationStateStore` (CAS via `revision`), `PgOutboundStateStore` (4 tables)
- `PgSubagentGoalStore` (remove `filesystem-goal-store` gate)
- `PgInterceptorStore` + `PgChatMemoryRecordStore` + bidirectional `chat_record_id`/`forensic_packet_id` linking
- Background retention sweep task; `brassclaw maintenance prune-old-data` CLI
- `brassclaw_memory` trait extension: `index_content(scope, source_ref, content, chat_record_id)` on `ChunkingMemoryDocumentIndexer`
- `brassclaw_embeddings` crate refactor: remove `EmbeddingsConfig`/`create_provider()`/HTTP impls; retain trait/cache/url_check/dimension util
- `EmbeddingRoleAdapter` in `brassclaw_reborn_composition` (error-type mapping table)
- `build_backend()` re-wiring: resolve embedding role → wire adapter → `dispatch_search` uses `.with_vector(embedding_active)`
- Chunk-cascade in retention sweep (transactional: delete chunks before Path A row)
- `brassclaw maintenance backfill-embeddings` CLI
- Three-role provider preconfiguration (`kohai` / `sempai` / `embedding`)

> ✅ `PgInterceptorStore`, `PgChatMemoryRecordStore`, `EmbeddingRoleAdapter`, `dispatch_search` with `.with_vector(embedding_active)`, `index_content` on `MemoryDocumentIndexer`, `brassclaw_embeddings` refactor (concrete HTTP impls removed), retention sweep (`retention_sweep.rs`), `backfill-embeddings` CLI all confirmed.
> ✅ `PgMemoryDocStore` confirmed as a partial adapter (module doc: "MemoryDoc-only adapter"); stubs are intentional design per plan — MemoryDoc-only surface.
> ✅ **RESOLVED:** `RebornEventStoreConfig::SharedPool` variant added; factory `build_postgres_production` now uses `SharedPool { pool, tenant_id }` — event stores reuse the shared pool instead of opening a second connection.
> ✅ **VERIFIED:** Forensic-packet deletion sweep IS fully implemented (lines 134–139 in `retention_sweep.rs` show actual DELETE statement). The debug-log at line 119 is only for malformed rows, not the main sweep path.
> ✅ **RESOLVED:** `DocType::Recipe` and `DocType::ToolSkill` routing gap resolved — `PgRecipeStoreFacade` now implements the full `RecipeStore` trait; wired in `webui.rs` as primary path (PG pool present) with `StoreBackedRecipeStore` as non-postgres fallback. `PgRecipeLibrary` wired in `runtime.rs` as primary agent-loop recipe lookup.
> ✅ `with_pg_run_state`, `with_pg_turn_state_store`, `with_pg_resource_governor` builder methods added to `crates/brassclaw_host_runtime/src/services/builder.rs`. `PgCapabilityLeaseStore` constructed in the pg-only path. These are wired in `build_pg_backend_production_with_tools` (the full-PG factory function).
> ⚠️ **IMPORTANT ARCHITECTURAL NOTE (confirmed in checkup session):** `build_pg_backend_production_with_tools` and `build_postgres_production` are `#[allow(dead_code)]` — the live `brassclaw serve` production path uses the **hybrid local-dev+PG path** in `build_reborn_services` (`LocalDev` profile + `RebornStorageInput::Postgres`) which calls `build_local_dev` then injects `pg_pool`. This means in production: `turn_state`, `checkpoint_state_store`, `loop_checkpoint_store`, `run_state`, `approval_requests`, `capability_leases` are still **in-memory** even on the postgres `serve` path. The full-PG factory path is not yet live. Tracked in `subplan_pg4_runtime_pg_path.md`.
> ✅ **RESOLVED (this session):** `PgSessionThreadService` now wired in `build_reborn_runtime` when `services.pg_pool` is available — thread history (conversations, messages) survives restarts. `PgSubagentGoalStore` now wired in `build_reborn_runtime` when `services.pg_pool` is available — subagent goals survive restarts.
> ℹ️ **REMAINING GAP:** `turn_state` (turn runs, blocking, approvals), `loop_checkpoint_store`, `checkpoint_state_store`, `run_state`, `capability_leases` in `build_reborn_runtime` still use in-memory stores even on the postgres `serve` path. The full PG wiring of these requires a significant refactor of `DefaultPlannedRuntimeParts` and `RebornLocalRuntimeServices` to use trait objects (tracked in `subplan_pg4_runtime_pg_path.md`). `ProcessServices` already uses `PgProcessStore`/`PgProcessResultStore` via `ProcessServices::postgres()` in `build_pg_backend_production_with_tools` (dead code). `PgConversationStateStore`, `PgOutboundStateStore`, `PgExtensionInstallationStore` fully implemented in their crates, not yet wired.

### PG-5 — Hooks and auth migration ✅ `IMPLEMENTED`
Rename `brassclaw_hooks_postgres` → `brassclaw_hooks_pg`; strip `#[cfg(feature = "postgres")]` module gates from `lib.rs`; make `deadpool-postgres`/`tokio-postgres` unconditional; port `parity_matrix.rs` + `multi_host_adversarial.rs` tests; delete `brassclaw_hooks_libsql` + `brassclaw_hooks_parity`; implement `PgAuthProductServices`; wire `PostgresPredicateStateBackend` in production factory.

> ✅ `crates/brassclaw_hooks_pg/` confirmed with `tests/parity_matrix.rs` + `tests/multi_host_adversarial.rs`. `brassclaw_hooks_libsql` + `brassclaw_hooks_parity` crates deleted. `lib.rs` cfg gates stripped, `deadpool-postgres`/`tokio-postgres` unconditional.

### PG-6 — libSQL removal 🔸 `PARTIAL`
Remove all `#[cfg(feature = "libsql")]` blocks; remove `libsql` from workspace `Cargo.toml` default array; remove `libsql` from `brassclaw_reborn_composition/Cargo.toml` and `brassclaw_reborn_cli/Cargo.toml` defaults; delete `brassclaw_hooks_libsql` crate dir; delete `RebornLibSqlIdempotencyLedger`; remove `RebornEventStoreConfig::Libsql/InMemory/Jsonl` variants; remove `filesystem-goal-store` gate; add WebUI v2 **"use for embedding"** button (third provider role button); update architecture boundary tests.

> ✅ Workspace `Cargo.toml` default array no longer includes `libsql`. `RebornLibSqlIdempotencyLedger` deleted. `RebornEventStoreConfig::Libsql/InMemory/Jsonl` variants removed.
> ⚠️ **GAP:** `brassclaw_reborn_composition/Cargo.toml` still has `libsql` as an **optional dep** (line ~97) and `migrate-from-libsql` feature is **default-ON** in `brassclaw_reborn_cli/Cargo.toml` — libSQL still compiled in by default (intentional for upgrade release but not yet stripped).
> ✅ **VERIFIED:** WebUI v2 **"use for embedding"** button IS fully implemented — `provider-card.js` lines 247–257 show the button, `useProviderManagementActions.js` wires `handleUseEmbedding`, `useLlmProviders.js` calls `setActiveLlm` with `role: "embedding"`, i18n strings present. The checkup flag was incorrect.

### PG-7 — libSQL → Postgres data migration at boot ✅ `IMPLEMENTED`
Implement `brassclaw_reborn_composition::migration` module; implement §8.1 steps 3–7 (profile-aware secrets, config.toml import, encrypted root-fs migration, `rewrap --tenant`, wizard/boot.initialized gate); `migrate-from-libsql` default-on for upgrade release; integration tests: seed libSQL → migrate → verify PG; upgrade-flow decryption test; non-default tenant upgrade test.

> ✅ `crates/brassclaw_reborn_composition/src/migration.rs` confirmed present with boot-path migration logic. `migrate-from-libsql` default-on confirmed.

### PG-8 — File-based config removal ⚠️ `FLAG`
Remove `config_file_path()`, `providers_file_path()`, `sempai_provider_file_path()` from `RebornHome`; remove `toml_edit`/`fs4` file-locking discipline; delete `DefaultLlmSlotUpdateSession` struct; update `brassclaw_reborn_cli config init` to use wizard.

> ⚠️ **NOT YET DONE:** This step is explicitly deferred — plan notes removal is behind `migrate-from-libsql`. `llm_reload.rs` and `llm_config_service.rs` still read `config.toml`/`providers.json` directly (see PG-2 flags). Expected state: incomplete until PG-6 libSQL strip is finalized.

### PG-9 — Systemd unit and documentation 🔸 `PARTIAL`
Write `brassclaw.service` systemd unit template with hardening directives; update `AGENTS.md` Database Rules (retire dual-backend mandate); purge stale v1 `src/` sections from `CLAUDE.md`; update `CLAUDE.md` env var table (two-tier model); write operator guide (fresh-install, upgrade, `master.key` ownership, DR backup, `rewrap` vs `rotate`, `brassclaw maintenance prune-old-data`); update `brassclaw_interceptor/src/store.rs` module doc (single-backend mandate); update per-crate AGENTS/CLAUDE docs.

> ✅ `deploy/brassclaw.service` confirmed present. `AGENTS.md` Database Rules updated (single-backend, no dual-backend mandate). `CLAUDE.md` two-tier env var table confirmed.
> ⚠️ **GAP:** Operator guide (fresh-install, upgrade, DR backup, rewrap vs rotate) not confirmed written. Per-crate AGENTS/CLAUDE doc updates not confirmed complete.

### PG-10 — Integration tests and E2E 🔸 `PARTIAL`
Full boot cycle from scratch (embedded PG → wizard → agent turn → graceful shutdown); restart from existing Postgres state; `BRASSCLAW_PG_URL` override; SIGKILL → orphaned-server detection; provider CRUD across restart; hardened-unit test (`jit=off` + `MemoryDenyWriteExecute`); `brassclaw config get` does not stop embedded PG.

> ⚠️ **GAP:** Full boot-cycle E2E test suite not confirmed present. Individual unit tests exist throughout crates, but the scenario-level integration tests (SIGKILL recovery, provider CRUD across restart, hardened-unit `jit=off`) were not located in the sweep.

### PG-11 — Remove `BRASSCLAW_REBORN_PROFILE` (three independent knobs) ✅ `IMPLEMENTED`
Phase 11a: add new per-dimension knobs (`sandboxing`, `permissions`, `trust`) keeping `BRASSCLAW_REBORN_PROFILE` functional; Phase 11b: deprecate old var with warning; Phase 11c: remove old boot-profile code and rename env var to `BRASSCLAW_RUNTIME_PROFILE`; update all §7 unit templates (currently use `BRASSCLAW_REBORN_PROFILE`).

> ✅ `BRASSCLAW_RUNTIME_PROFILE` implemented; `BRASSCLAW_REBORN_PROFILE` deprecated with warning logged at boot. `brassclaw runtime-profile list` command confirmed. `AGENTS.md` env var table updated to two-tier model with `BRASSCLAW_RUNTIME_PROFILE`.

---

## Design Transition — DB-Stored Components + Intent + Actions (`plan.md` + `run-intent-script.md`)

> **Prerequisites:** PG-0 through PG-4 (schemas V000–V026 in place).
> Steps are numbered per `run-intent-script.md`; sub-steps from `plan.md` are inlined.
> Phase/Step designations from the execution script are canonical.

### Step 0 — Phase 0 sign-off (spec v5.5) ✅ `IMPLEMENTED`
Verify `spec.md` v5.5 completeness: all 31 open questions Q1–Q31 resolved in §7; glossary matches codebase; validation table matches `recipe_validator.rs`; trust-removal targets confirmed present; interceptor infrastructure confirmed present; 5 wiring gaps confirmed. No code changes.

> ✅ `spec.md` v5.5 present at `.zenflow/tasks/.../spec.md`. This is a sign-off/review step — no code changes required.

---

### Step 1 — Phase 1: DB-stored Skills + Intent system + Actions

#### Step 1.1 — V027 `reborn_skills` migration ✅ `IMPLEMENTED`
Add `V027__reborn_skills.sql`: scope-tuple table with explicit activation columns, `intent_examples JSONB`, `consumer_tags[] TEXT[]` (GIN index, CHECK `^[0-9]{2}(:[a-z0-9-]+)?$`), `class_code` (01/02/03), `prompt_uid`, reward/lineage columns. No `trust` column. Scope-isolation and CHECK constraint tests.

> ✅ `V027__reborn_skills.sql` through `V029__reborn_actions.sql` all confirmed present in `crates/brassclaw_pg/migrations/`.

#### Step 1.2 — Skill store + validator wiring ✅ `IMPLEMENTED`
New `crates/brassclaw_skills/src/db_store.rs`: CRUD over `reborn_skills` using `LoadedSkill` shape. Reuse `RecipeValidator` + content-safety validation; strict name pattern; set `class_code` from `compatibility`; seed `consumer_tags[]` from class defaults; add `05:validator` on every new/updated row. Validation-split semantics (content columns gated, reward columns immediate-write).

> ✅ `crates/brassclaw_skills/src/db_store.rs`: `DbSkillStore` with `fetch_for_consumer`, `05:validator` filtering, validation-split semantics confirmed.

#### Step 1.3 — SKILL.md importer 🔸 `PARTIAL`
One-shot `crates/brassclaw_reborn_composition/src/skill_import.rs`: walk `skills/*/SKILL.md`, split large skills into ≤1-tool rows, assign `class_code`/`consumer_tags[]`, extract `intent_examples` from keywords/patterns/description sentences. Idempotent via `content_hash`.

> ✅ `skill_import.rs` with `run_skill_import` confirmed present.
> ⚠️ **FLAG:** `bundled_skills.rs` + `build.rs` embedding `migrated_skills_catalog.json` are still live. Step 9.1 says to delete these — they have not been deleted yet (v1→v2 blob migration path still active alongside new DB importer).

#### Step 1.4 — Prompt assembler reads from `reborn_skills` ✅ `IMPLEMENTED`
Update `brassclaw_skills::selector` + `brassclaw_engine::executor::context` to load from DB (feature-gated: `skills-db`); deterministic selection pipeline (gating → scoring → budget); ordered injection by `(class_code asc, prompt_uid asc)`.

> ✅ `DbSkillStore` wired as the primary skill source. `selector.rs` and `context.rs` updated.
> ℹ️ `score_skill()` in `selector.rs` is the v1 Rust skill selection function — a Step 9.1 deletion target (along with the rest of the v1 skill shim), NOT a Step 2.2 target. `format_docs_as_context` in `context.rs` is the "resurrected" prior-knowledge formatter that is the basis for `__assemble_prior_knowledge__` (spec §2.3b explicitly keeps it). Both are intentionally present.

#### Step 1.5 — V028 `reborn_intent_inputs` + `__resolve_intent__` host function ✅ `IMPLEMENTED`
`V028__reborn_intent_inputs.sql`: normalized schema (one row per `(scope, input_text, input_class, component_id)`), B-tree exact-match index (PERF-01), GIN trigram (PERF-04), `pg_trgm` at install time. New `crates/brassclaw_engine/src/memory/intent_system.rs`: 4-class query classifier (PERF-02 single `CASE WHEN` query); match-order rules a–f; atomic score increment (PERF-03, SEC-05 cap 100 + rate-limit 50/scope/hr); disambiguation chat message type; "try it with AI" fallback (class-4, Rust-side); "AI before User" flip switch (per-user in `reborn_user_preferences`; silent keyword-fallback path, no new rows); no-match reformulate flow.

> ✅ `crates/brassclaw_engine/src/memory/intent_system.rs`: `resolve_intent`, 4-class query classifier, disambiguation, `record_disambiguation_choice` all confirmed.

#### Step 1.6 — V029 `reborn_actions` + default.py execution ✅ `IMPLEMENTED`
`V029__reborn_actions.sql`: class 16, 13 step types in `steps JSONB`, hard limits (256KB/500 steps/50 tools, PERF-18), recursion bounding (SEC-09 depth 5, cycle detection, step budget 1000), `prior_knowledge_content TEXT NULL`, `override_prompt_creation BOOLEAN DEFAULT true`. No separate Rust executor — default.py is the executor. `tool_call` steps go through `EffectExecutor` bridge with `allowed_tools[]` defense-in-depth (SEC-07). `spawn_subprocess` dispatches via host runtime script lane only (SEC-08). 8-step orchestrator dispatch flow. Token-budget exemption.

> ✅ `V029__reborn_actions.sql` confirmed. `default.py` acts as executor.

---

### Step 2 — Phase 1.5: Prompt-path dedup + self-modification boundary

#### Step 2.1 — Resurrect `build_step_context` (User-at-N-1 injection) ✅ `IMPLEMENTED`
Implement the Phase 1.5 design: resurrect `build_step_context` as a Rust-side User-message injection at position N-1 (KV-cache-friendly). Static DB objects stay in System prefix ordered by `(class_code asc, prompt_uid asc)`.

> ✅ `insert_as_user_message_at_n_minus_1` and `insert_volatile_context_at_n_minus_1` confirmed in `crates/brassclaw_engine/orchestrator/default.py`.

#### Step 2.2 — Delete 8 intent-detection functions + 3 Python formatters ✅ `IMPLEMENTED`
Delete from `default.py`: `signals_tool_intent`, `signals_execution_intent`, `score_skill`, `extract_explicit_skills`, `format_docs`, `format_skills`, `append_system_append`. Delete from `reasoning.rs`: `llm_signals_tool_intent`, `user_signals_execution_intent`. Note: `extract_keywords` (Rust, `retrieval.rs:80`) is NOT deleted here — relocated to `retrieval_dbless.rs` in Phase 5.

> ✅ All 7 Python functions deleted from `default.py` (confirmed: no `def score_skill`, `def signals_tool_intent`, etc. found). Both Rust reasoning.rs functions (`llm_signals_tool_intent`, `user_signals_execution_intent`) deleted. `format_docs_as_context` in `context.rs` is NOT a deletion target — spec §2.3b explicitly "Resurrects" it as the basis for `__assemble_prior_knowledge__` (content-is-king assembly). The Rust `score_skill()` in `selector.rs` is the **v1 skill selection** function (not the Python intent-detection twin) and is a Step 9.1 deletion target (delete v1 skill code), not Step 2.2.

#### Step 2.3 — Reroute `memory_write` through `__validate_component__` ✅ `IMPLEMENTED`
Intercept `memory_write` calls for code/component changes at the Rust bridge; route to `__validate_component__` instead of direct write; update-candidates enter Q1 with `05:validator` tag. 3-failure auto-rollback retained as safety net.

> ✅ `__validate_component__` in `crates/brassclaw_engine/src/executor/orchestrator.rs` confirmed. `memory_write` interception path confirmed.

#### Step 2.4 — LLM code-audit gate for Orchestrator/Scaffold (Q1→Q2) ✅ `IMPLEMENTED`
For class 10 (Orchestrator) and class 50 (Scaffold): add kohai-provider LLM code-audit step at Q1→Q2 transition; disable WebUI "Validate" button until audit clean; route flagged components to Q3 with `review_feedback`.

> ✅ `llm_audit_status` + `llm_audit_findings` columns confirmed in `crates/brassclaw_product_workflow/src/recipes.rs` (4-queue schema).
> ✅ **IMPLEMENTED (this session):** WebUI "Validate" button guard added to `validation-queue-tab.js`. Q2 items now show Validate/Reject buttons. For class codes 10 (Orchestrator) and 50 (Scaffold), the Validate button is disabled when `llm_audit_status` is `"pending"` or `"flagged"`, with a descriptive tooltip. The backend `validate_component` handler already enforces the same guard (returns 403 if audit not clean). Both `validateComponent` and `rejectComponent` API functions added to `settings-api.js`; i18n keys added to `en.js`.

---

### Step 3 — Phase 2: DB-stored Tools (Rusty-only)

#### Step 3.1 — V030 `reborn_tools` migration ✅ `IMPLEMENTED`
`V030__reborn_tools.sql`: class 00, `param_schema JSONB`, `param_template JSONB`, `effect_type`, `preconditions`, `error_handling`, `consumer_tags[]` (default `{00:rusty}` + `05:validator`), GIN index, validation/lineage columns.

> ✅ `V030__reborn_tools.sql` confirmed present in `crates/brassclaw_pg/migrations/`.

#### Step 3.2 — DB-backed tool store + capability surface ✅ `IMPLEMENTED`
Implement tool store in `crates/brassclaw_capabilities` reading from `reborn_tools` into `ToolRegistry`. Strip Monty/LLM prompt text from tool rows. Capability surface `RecipeValidator` checks `tool_name` against DB-backed surface.

> ✅ **IMPLEMENTED (this session):**
> - `DbToolSource` (in `crates/brassclaw_engine/src/capability/db_tool_source.rs`) reads `reborn_tools`, returning validated Rusty tool names behind the `skills-db` feature. Confirmed present.
> - `ToolRegistry` + `ToolRegistryStore` trait confirmed at `crates/brassclaw_capabilities/src/tool_registry.rs`.
> - `auto_validate_pending` method added to the `RecipeStore` trait (`brassclaw_product_workflow::recipes`) — sweeps all `pending` rows in `q1_auto`, fetches `available_tools` from `reborn_tools` via `DbToolSource`, runs `ComponentValidator::validate_by_class`, and writes `auto_passed` or `auto_failed` back.
> - `PgRecipeStoreFacade::auto_validate_pending` implemented in `pg_recipe_store.rs` (behind `skills-db` feature gate).
> - `spawn_q1_validation_sweep` added to `retention_sweep.rs` — spawns a 30-second periodic task calling `auto_validate_pending`.
> - Sweep wired in `brassclaw_reborn_cli/src/commands/serve.rs` when both `postgres` and `skills-db` features are active.
> - `skills-db` feature propagated from `brassclaw_reborn_composition` → `brassclaw_engine/skills-db`.
> - `skills-db` feature added to `brassclaw_reborn_cli/Cargo.toml`.
> - Pre-existing bug fixed: `webui_tenant_id()` on `RebornRuntime` was gated `all(postgres, root-llm-provider)` but called from `webui.rs` inside a `postgres`-only gate; widened to `#[cfg(feature = "postgres")]`.
> - Note: `reborn_tools` has no prompt text columns (tools are Rusty-only, class 00) — §3.3 prompt-text stripping is N/A for the DB schema; only legacy in-memory tool descriptions need cleaning (Step 3.3).

#### Step 3.3 — Monty/LLM instruction via Skills only ❌ `NOT IMPLEMENTED`
Remove tool-definition prompt text from Monty/LLM prompt paths; confirm Monty callables and LLM guidance come only from class 01/02/03 Skill rows.

> ❌ No evidence this cleanup has been done — tool-definition prompt text removal from Monty/LLM paths not confirmed.

---

### Step 4 — Phase 3: Remove trust layer + 4-queue validation lifecycle

#### Step 4.1 — Delete the trust layer ❌ `NOT IMPLEMENTED`
Delete `SkillTrust` enum, `V2SkillMetadata.trust`, `default_trust()`, trust-by-source-directory logic in `registry.rs`, skill-trust attenuation phase. Delete `Installed`/`Trusted` tool-access distinction.

> ❌ `SkillTrustLevel` enum still fully present in `crates/brassclaw_turns/src/run_profile/skill_context.rs` with `Installed`/`Trusted` variants, `trust_rank()` function, and `trust` field on skill context structs. `crates/brassclaw_first_party_extension_ports/src/skills.rs` still filters by `entry.trust == SkillTrustLevel::Trusted`. The trust layer has NOT been removed.

#### Step 4.2 — `source` as pure provenance; confidence factor universal ❌ `NOT IMPLEMENTED`
Remove `if source == "extracted"` gate from `score_skill`/`selector.rs`; confidence factor is source-independent and used as fallback-routing signal only; skills with no usage data default to 1.0.

> ❌ Gated path not confirmed removed — dependent on Step 2.2 deletions + Step 4.1 trust removal, neither of which is done.

#### Step 4.3 — `Validated == trusted` + validator-tag invariant ✅ `IMPLEMENTED`
Audit all loop-facing fetch paths; replace trust filters with `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)`. `fetch_for_consumer` excludes `05:validator`-tagged rows.

> ✅ `fetch_for_consumer` in `db_store.rs`, `unified_store.rs`, `pg_recipe_store.rs` all confirmed to exclude `05:validator`-tagged rows and require `Validated` status.

#### Step 4.4 — Expand validator + validator independence + LLM code-audit ✅ `IMPLEMENTED`
Extend `RecipeValidator`→`ComponentValidator` to validate Orchestrator Python + Monty-class extension payloads. Validator runs as Rust-side infrastructure outside default.py (bootstrapping-paradox fix). Self-improvement `memory_write` rerouted (Step 2.3 applies). LLM code-audit for class 10/50 at Q1→Q2 (Step 2.4 applies here too).

> ✅ `ComponentValidator::validate_by_class` in `crates/brassclaw_engine/src/memory/component_validator.rs` confirmed.

#### Step 4.5 — Validator-tag greyed-out mechanism + 4-queue lifecycle ✅ `IMPLEMENTED`
Lifecycle rules: create/import/update → add `05:validator`; Step-2 validate → pop `05:validator`; update-candidate inherits active version's tags ∪ `{05:validator}`. Formalize Q1 (auto), Q2 (manual WebUI), Q3 (automated revision Extension, class 09), Q4 (rejection + wipe). `is_valid_transition` extended for all 4 queues + `Rejected → Pending` + `Garbage → deleted`. Q4 wipe reads `q4_retention_days` from `reborn_monty_vm_settings`. Old recipe/tool_skill routes kept as aliases.

> ✅ `is_valid_transition` extended for `Rejected→Pending`, `AutoFailed→Pending`, `Rejected→Garbage` in `crates/brassclaw_product_workflow/src/recipes.rs`. 4-queue schema with `queue_code` column confirmed. Old recipe/tool_skill route aliases retained.

#### Step 4.6 — V031 `reborn_validation_config` + per-class thresholds + `ComponentValidator` dispatch ✅ `IMPLEMENTED`
`V031__reborn_validation_config.sql`: one row per `(scope, class_code)` with name/description/token-budget/require-fields thresholds. `ComponentValidator::validate_by_class` dispatch: Skills (01-03) = full agentskills.io; Tools = tool_name + param_schema; Extensions = soft; Actions = no token budget; former doctypes = soft; Recipes = trigger validation; Orchestrator/Scaffold = LLM audit. Compiled-in safety floors prevent config from weakening critical gates.

> ✅ `V031__reborn_validation_config.sql` confirmed present. `ComponentValidator::validate_by_class` dispatch confirmed.

---

### Step 5 — Phase 4: Unified Extensions + DocPlans dissection + Recipes class 21

#### Step 5.1 — V032 `reborn_extensions_unified` + V033 `reborn_recipes` migrations ✅ `IMPLEMENTED`
`V032__reborn_extensions_unified.sql`: class enum (mcp_server/mcp_client/rusty/monty/llm/misc), `prior_knowledge_content TEXT NULL`, `override_prompt_creation BOOLEAN DEFAULT false`, `consumer_tags[]`, validation/lineage columns, GIN index. `V033__reborn_recipes.sql`: class 21, solution-class schema (trigger JSONB, steps JSONB, `prior_knowledge_content`, `override_prompt_creation DEFAULT false`, `intent_examples`, `consumer_tags[]`, reward/lineage).

> ✅ `V032__reborn_extensions_unified.sql` through `V043__reborn_notes.sql` all present in `crates/brassclaw_pg/migrations/`.

#### Step 5.2 — Unified extension store + class adapters ✅ `IMPLEMENTED`
New `crates/brassclaw_extensions/src/unified_store.rs`: CRUD over `reborn_extensions_unified`. Adapters: mcp→`ExtensionManifestV2`, rusty→tool surface, monty→recipe/plan, llm→prompt template. Manifest v2 validation fail-closed.

> ✅ `crates/brassclaw_extensions/src/unified_store.rs`: `PgUnifiedExtensionStore`, `fetch_for_consumer` confirmed.

#### Step 5.3 — Dissect DocPlans + migrate Recipes/ToolSkills ✅ `IMPLEMENTED`
Decompose DocPlans into constituent rows in `reborn_skills`/`reborn_tools`/`reborn_extensions_unified`/`reborn_recipes`. Migrate `DocType::Recipe` → `reborn_recipes` (class 21); `DocType::ToolSkill` → `reborn_tool_skills` (class 13, Phase 5 Step 5.3). Retire `recipe_store.rs`/`recipe_library.rs` in favor of unified store + `reborn_recipes`; preserve `RecipeLookup` trait boundary.

> ✅ `crates/brassclaw_reborn_composition/src/docplan_dissector.rs` confirmed present. `crates/brassclaw_reborn_composition/src/pg_recipe_store.rs`: `PgRecipeStore` + `PgRecipeLibrary` (`RecipeLookup`) confirmed.
> ✅ `PgRecipeStoreFacade` implements full `RecipeStore` trait (13 methods) — wired as primary path in `webui.rs` when Postgres pool is present. `StoreBackedRecipeStore` retained as non-postgres fallback.
> ✅ `PgRecipeLibrary` wired as primary `RecipeLookup` in `runtime.rs` when Postgres pool is present. `RecipeLibrary` (MemoryDoc-backed) retained as fallback. ToolSkill methods return empty/no-op pending V037 migration (Phase 5).
> ℹ️ `recipe_store.rs` + `recipe_library.rs` retained as non-postgres fallback modules (PG-8 cleanup will remove them once the migrate-from-libsql gate is lifted).

#### Step 5.4 — Migrate existing Extensions ✅ `IMPLEMENTED`
Import installed extensions from `brassclaw_extensions::pg_store` into `reborn_extensions_unified` deriving `class` from `runtime`. Extension contract tests pass.

> ✅ `PgUnifiedExtensionStore` wired; extension contract confirmed via `unified_store.rs` adapters.

---

### Step 6 — Phase 5: PlanA-Memory universal connector + intent-driven retrieval + de-chunk + DB-less fallback + Rust formatting + former-doctype tables

#### Step 6.1 — `RetrievalSource` trait + two backends + `reborn_component_catalog` ❌ `NOT IMPLEMENTED`
New `RetrievalSource` trait with `PostgresSource` (reads all component tables + catalog) and `RamSource` (compiled-in defaults + fallback-content file). `reborn_component_catalog` read model (PERF-05 single-query fetch). `fetch_for_consumer(consumer_tag)` enforces `validation_status = 'Validated' AND '05:validator' != ANY(consumer_tags)` on both backends.

> ❌ No `RetrievalSource` trait found. No `PostgresSource` or `RamSource` backends found. `reborn_component_catalog` read model not implemented — `__assemble_prior_knowledge__` in `orchestrator.rs` explicitly says "Phase 5 stub — delegates to `retrieve_context`" with `STUB_MAX_DOCS = 20`. The `fetch_for_turn` function is not implemented; `retrieve_context` is still the actual retrieval path.

#### Step 6.2 — DB-less fallback-content file ❌ `NOT IMPLEMENTED`
Static file created at installation time (~256KB, ~50K tokens, Tools→Scaffold→Orchestrator→Skills→Extensions→Recipes→Specs/Lessons priority; Issues/Notes/Summaries excluded). `RamSource` loads it at startup. DB-less path uses keyword-retrieval (pre-v4 `extract_keywords`/`keyword_match_score`/`doc_type_weight`) via `retrieval_dbless.rs`. "Try it with AI" and "AI before User" unavailable in DB-less mode.

> ❌ `retrieval_dbless.rs` module file exists but no `RamSource`, no fallback-content file generation, no keyword-retrieval wiring inside it. Module appears to be a placeholder.

#### Step 6.3 — V034 `reborn_monty_vm_settings` + Monty VM lifecycle manager 🔸 `PARTIAL`
`V034__reborn_monty_vm_settings.sql`: single row per scope (upsert), `max_duration_secs`, `max_allocations`, `max_memory_bytes`, `failure_rollback_threshold`, `active_orchestrator_id` (gated FK to validated orchestrator), `prior_knowledge_token_budget DEFAULT 2000`, `q4_retention_days DEFAULT 30`, `forensic_packet_retention_days DEFAULT 90`. Kernel-owned `monty_lifecycle.rs`: drain + admission control (PERF-16), `force=true` abort, status polling (`running`/`draining`/`restarting`/`stopped`/`error`). Replaces compiled-in `orchestrator_limits()` constants; DB-less fallback to env override + compiled-in defaults.

> ✅ `V034__reborn_monty_vm_settings.sql` confirmed present. `MontyVmSettings` struct, `MontyVmRestartRequest/Response`, `restart_monty_vm` trait method defined in product_workflow layer.
> ❌ **NOT IMPLEMENTED:** Kernel-owned `monty_lifecycle.rs` drain + admission-control logic — `restart_monty_vm` is an async trait method stub returning unimplemented; no actual lifecycle manager exists in composition.
> ❌ **NOT IMPLEMENTED:** `PgMontyVmSettingsStore` (or equivalent DB read/write path) — settings struct exists but no DB wiring found.

#### Step 6.4 — V035 `reborn_user_preferences` migration 🔸 `PARTIAL`
`V035__reborn_user_preferences.sql`: `(user_id, preference_key, preference_value)` key-value. Current key: `ai_before_user` (default `false`). Hidden/disabled in DB-less mode. Persisted via `PUT /api/chat/preferences/{key}`.

> ✅ `V035__reborn_user_preferences.sql` confirmed present. Handler doc confirms `ai_before_user` key and `PUT /api/chat/preferences/{key}` endpoint.
> ❌ **NOT IMPLEMENTED:** No `PgUserPreferenceStore` or actual DB read/write code found in composition — the endpoint likely exists in the router but the backing store is not wired.

#### Step 6.5 — Former-doctype tables (V036–V043) 🔸 `PARTIAL`
Add migrations for 8 former-doctype tables (classes 12–20 except 16 which exists already):
`V036__reborn_specs.sql` (12), `V037__reborn_tool_skills.sql` (13), `V038__reborn_plans.sql` (14), `V039__reborn_summaries.sql` (15), `V040__reborn_docus.sql` (17), `V041__reborn_lessons.sql` (18), `V042__reborn_issues.sql` (19), `V043__reborn_notes.sql` (20). Each: scope tuple, `class_code`, `prompt_uid`, `title`, `content`, `intent_examples JSONB`, `consumer_tags[]`, validation/lineage. Document splitting importer (`component_import.rs`): migrate all `MemoryDoc` retired `DocType` rows; split large docs into ≤5000-token rows; extract intent_examples.

> ✅ `V036__reborn_specs.sql` through `V043__reborn_notes.sql` all confirmed present in migrations.
> ❌ **NOT IMPLEMENTED:** `component_import.rs` document-splitting importer — no file found to migrate existing `MemoryDoc` rows into the new class-specific tables.

#### Step 6.6 — Repair chunk/embedding machinery (revised: keep + fix) ✅ `IMPLEMENTED`
**Decision: DO NOT remove `brassclaw_embeddings`. Repair it.** Fix regressions from `cbc5d437`: restore `skills/portfolio/` from archive; add `InMemoryBackend` at `/tenants` in `build_local_dev_root_filesystem`; repair any further `brassclaw_embeddings`/`brassclaw_memory` path regressions. All tests pass with zero failures.

> ✅ `brassclaw_embeddings` crate retained and refactored (concrete HTTP impls removed, trait/cache/url_check retained). `ProviderRole::Embedding` variant, `EmbeddingRoleAdapter`, `dispatch_search` with `.with_vector(embedding_active)` all confirmed.

#### Step 6.7 — Intent-driven retrieval + `__assemble_prior_knowledge__` + token-budget 🔸 `PARTIAL`
Replace `retrieve_context` "load all docs" path with `fetch_for_turn(query, sender_class_code, token_budget)`: resolve intent → fetch by ID from `reborn_component_catalog` (PERF-05, SEC-01 validation gate filter) → assemble. Actions (class 16) exempt from `prior_knowledge_token_budget` truncation. "Try it with AI" fallback (class-4 keyword path). "AI before User" silent path (no new `reborn_intent_inputs` rows). DB-less fallback path. Token budget replaces hardcoded 5-doc limit.

> ✅ `__assemble_prior_knowledge__` function exists in `orchestrator.rs`. `prior_knowledge_token_budget` wired in `default.py`.
> ❌ **STUB:** `__assemble_prior_knowledge__` explicitly documented as "Phase 5 stub — delegates to `retrieve_context`" with `STUB_MAX_DOCS = 20`. `fetch_for_turn` not implemented. Intent-driven catalog lookup (`reborn_component_catalog`) not wired. Token budget does not replace hardcoded limit yet.

#### Step 6.8 — Prior knowledge formatting: "content is king" + Solution Override ✅ `IMPLEMENTED`
Delete `format_docs`, `format_skills`, `append_system_append` from Python (already deleted in Step 2.2; verified here). Implement `__assemble_prior_knowledge__(goal, token_budget, sender_class_code)` Rust host function: two paths — **Solution Override** (single solution-class component with `override_prompt_creation: true` → return PKC/content verbatim, no headers, `override_prompt_creation: true`) and **Normal Assembly** (multiple components → concatenate in `(class_code asc, prompt_uid asc)` order with `## Prior Knowledge` + per-item `### [{class_code}:{CLASS-LABEL}] {name}` headers). Static class-code→label lookup table (00→TOOL … 50→SCAFFOLD). Returns `PriorKnowledgeResult { content, override_prompt_creation, matched_component_ids }`. Add `prior_knowledge_content TEXT NULL` + `override_prompt_creation BOOLEAN` columns to solution-class tables (Extensions, Plans, Recipes, Actions).

> ✅ `__assemble_prior_knowledge__` + `format_prior_knowledge_for_llm` in `orchestrator.rs` confirmed. `PriorKnowledgeResult` struct with `content`, `override_prompt_creation`, `matched_component_ids` confirmed. Solution Override path + Normal Assembly path both present.

#### Step 6.9 — Volatile memories injected as User message at N-1 ✅ `IMPLEMENTED`
Wire resurrected `build_step_context` path: volatile prior-knowledge injected as User message at N-1. Static DB objects in System prefix ordered by `(class_code asc, prompt_uid asc)`. System message byte-identical across turns with same static components.

> ✅ `insert_volatile_context_at_n_minus_1` called on both Override and Normal paths in `default.py`. Wiring confirmed.

#### Step 6.10 — Retire ALL DocType variants ❌ `NOT IMPLEMENTED`
Delete all `DocType::` enum variants (`Skill`, `Recipe`, `ToolSkill`, `Plan`, `Summary`, `Lesson`, `Issue`, `Spec`, `Note`); delete `DocType` enum. Update `brassclaw_engine::memory` modules to read from new class-specific tables. Delete `doc_type_weight`/`keyword_match_score`/`extract_keywords` from DB-mode `retrieval.rs` (relocate to `retrieval_dbless.rs` for DB-less fallback path). Update `context.rs` to call `fetch_for_turn` + `__assemble_prior_knowledge__`.

> ❌ `DocType` enum still fully present and active in `crates/brassclaw_engine/src/types/memory.rs`. 119+ references to `DocType::` in production code. `recipe_store.rs` still uses `DocType::Recipe` and `DocType::ToolSkill` extensively. `doc_type_weight`/`keyword_match_score` still in `retrieval.rs`. This entire step is NOT done — it depends on Step 6.1 (`fetch_for_turn` + catalog) being completed first.

---

### Step 7 — Phase 5.5: Interceptor activation (Sempai–Kohai wiring)

#### Step 7.0 — V044 `brassclaw_forensic_packets` ALTER migration ✅ `IMPLEMENTED`
`V044__brassclaw_forensic_packets_alter.sql`: ALTER existing V026 table — add `component_refs JSONB` (array of `{class_code, prompt_uid, component_id, schema_version}`) and `volatile_tail TEXT`. Keep existing `prompt JSONB` for backward compatibility. Do NOT fix `pg_store.rs` V026 module doc (reference is correct).

> ✅ `V044__brassclaw_forensic_packets_alter.sql` confirmed present in migrations.

#### Step 7.1 — `InterceptorResult` trait change ✅ `IMPLEMENTED`
Add `InterceptorResult { packet_id, adjusted_messages: Option<Vec<(String, String)>> }`; change `on_prompt_assembled` return from `Option<String>` to `Option<InterceptorResult>`. Update `SempaiReviewOutcome`: replace `adjusted_messages` with `adjusted_volatile_messages` + `bridge_messages` + `composition_summary` + `proposed_recipe_updates` + `proposed_intent_examples` + `settings_adjustments`. Update `InterceptorPromptOutput`, `ModelInput.resolved_messages`. Update 6 test stub files. Wire `adjusted_messages` → `ModelInput` → skip `resolve_model_messages` when pre-resolved.

> ✅ `InterceptorResult` struct + `on_prompt_assembled` return type change confirmed. `SempaiReviewOutcome` updated fields (`adjusted_volatile_messages`, `bridge_messages`, `composition_summary`, `proposed_recipe_updates`, `proposed_intent_examples`, `settings_adjustments`) all confirmed.

#### Step 7.2 — Wire `PgInterceptorStore` + `sempai_swappable` + `SharedInterceptorMode` ✅ `IMPLEMENTED`
In `brassclaw_reborn_composition/src/runtime.rs`: replace `interceptor_store: None` with `PgInterceptorStore::new(pool, tenant_id)` (cfg `postgres`); allocate `sempai_swappable` via `SwappableLlmProvider::new(PlaceholderLlmProvider)`; create `SharedInterceptorMode`; wire `interceptor_mode` through `DefaultPlannedRuntimeParts` → factory → host. Feature-gated: store = `postgres`; mode + live-swap = `postgres` + `root-llm-provider`.

> ✅ `PgInterceptorStore` wired in `runtime.rs` (cfg `postgres`). `sempai_swappable` allocated as `SwappableLlmProvider::new(PlaceholderLlmProvider)`. `SharedInterceptorMode` created and threaded through confirmed.

#### Step 7.3 — `set_active(Sempai)` live-swap + mode flip ✅ `IMPLEMENTED`
In `llm_config_service.rs`: add `sempai_swappable` + `interceptor_mode` fields; extend `ProviderRole::Sempai` arm with live-swap after DB write; implement `build_sempai_provider` reusing `brassclaw_llm::build_static_provider_chain`; flip `interceptor_mode` to Rerouting on set / Routing on clear.

> ✅ `sempai_swappable` + `interceptor_mode` fields in `llm_config_service.rs` confirmed. `ProviderRole::Sempai` arm with live-swap confirmed.

#### Step 7.4 — Sempai gateway + rerouting branch + 3-part prompt + KV-cache pre-warm ✅ `IMPLEMENTED`
Wrap `sempai_swappable` in its own `LlmProviderModelGateway`; thread through factory → host. Rerouting branch in `on_prompt_assembled`: Routing path saves ForensicPacket (`component_refs` + `volatile_tail`, NOT full `prompt JSONB`), returns `adjusted_messages: None`; Rerouting path resolves message refs → builds 3-part Sempai prompt (Part A = `reassemble_base_prompt()` via direct SQL to individual component tables Q20; Part B = `sempai_persona` config key, default from `prompts/sempai_audit.md`; Part C = per-turn volatile tail + component manifest from `matched_component_ids`) → calls Sempai → parses `SempaiReviewOutcome` → routes `proposed_recipe_updates`/`proposed_intent_examples` to Q1 validation queue → returns `adjusted_messages: Some`. Actions bypass (no `__llm_complete__` → interceptor not reached, no ForensicPacket). Pre-warm endpoint `POST /api/interceptor/prewarm` (rate-limit 1/min/caller).

> ✅ Sempai gateway + rerouting branch + ForensicPacket save path all confirmed present.
> ✅ **RESOLVED (this session):** `SempaiProposalSink` trait added to `brassclaw_interceptor` (`crates/brassclaw_interceptor/src/proposal_sink.rs`). `PgSempaiProposalSink` implementation added to `brassclaw_reborn_composition` (`src/sempai_proposal_sink.rs`) — inserts `proposed_recipe_updates` as class-21 `reborn_recipes` rows with `validation_status = 'pending'`, `queue_code = 'q1_auto'`, `consumer_tags = ['05:validator']`, `source = 'sempai_proposal'`; inserts `proposed_intent_examples` as class-21 rows with `source = 'sempai_intent_proposal'` and the raw blobs in `intent_examples` JSONB. `proposal_sink` field added to `RebornLoopDriverHostFactory` + `RebornLoopDriverHost`; wired in `DefaultPlannedRuntimeParts`; constructed and wired in `build_reborn_runtime` when `pg_pool` available; called (non-fatally) in `run_sempai_review` after parsing `SempaiReviewOutcome`.

#### Step 7.5 — Interceptor config service + `reassemble_base_prompt()` + ForensicPacket cleanup ✅ `IMPLEMENTED`
New `InterceptorConfigService` trait + `RebornInterceptorConfigService` impl; `InterceptorConfigStore` backed by `brassclaw_config` table (4 keys). `reassemble_base_prompt()`: direct SQL to individual component tables (NOT `reborn_component_catalog`), `information_schema.tables` guard for missing tables, merge + sort `(class_code asc, prompt_uid asc)`, write Part A to `brassclaw_config`. HTTP endpoints: `GET/POST /api/interceptor/config`, `POST /api/interceptor/reassemble`, `POST /api/interceptor/prewarm`. `prompts/sempai_audit.md` (default Part B). ForensicPacket cleanup task: daily, reads `forensic_packet_retention_days` from `reborn_monty_vm_settings`, `forensic_packet_retention_days = 0` = no-op.

> ✅ `InterceptorConfigService` trait + `RebornInterceptorConfigService` impl confirmed. `reassemble_base_prompt()` via direct SQL (Q20) with 4 config keys confirmed.
> ⚠️ **FLAG:** ForensicPacket cleanup task — `retention_sweep.rs` line ~119 references forensic packets but appears to log a debug warning only; actual deletion/daily sweep logic not confirmed implemented.

---

### Step 8 — Phase 6: Settings UI (10-tab editor)

#### Step 8.1 — WebUI v2 backend routes 🔸 `PARTIAL`
REST routes for `/api/settings/{skills,tools,extensions,actions,orchestrators,scaffolds,monty-vm}` (GET/PUT/POST/DELETE); `POST /api/settings/monty-vm/restart` (kernel lifecycle manager, optional `force=true`); `GET /api/settings/monty-vm/status`. Extend validation-queue routes: add `queue_code` param (q1_auto/q2_manual/q3_revision/q4_rejection); extend `PUT .../validate` to pop `05:validator`; add `PUT .../re-submit`, `DELETE .../wipe`; LLM code-audit guard for class 10/50 (403 if audit pending). Intent-inputs routes: `GET/PUT/DELETE /api/settings/intent-inputs`. `PUT /api/chat/preferences/{key}` (`ai_before_user` persistence). All existing recipe/tool_skill route aliases retained.

> ✅ `ai_before_user` endpoint handler documented in `crates/brassclaw_webui_v2/src/handlers.rs:1525`. Validation-queue routes with `queue_code` param confirmed. `POST /api/settings/monty-vm/restart` route exists in router.
> ❌ **NOT IMPLEMENTED:** Backend for `POST /api/settings/monty-vm/restart` calls `restart_monty_vm` which is an unimplemented stub — route exists but no actual restart logic behind it.

#### Step 8.2 — React SPA Settings section (10 tabs) ❌ `NOT IMPLEMENTED`
10 tabs: Skills / Tools / Extensions / Actions / Orchestrator / Scaffold / Monty VM / Validation Queue / Reliability / Interceptor Config.
- **All tabs:** list view (name, class_code, prompt_uid, validation status, consumer_tags[]); editor (frontmatter + body, immediate reward writes); **intent examples editor** ({input, class} list); **consumer-tag chip editor** (greyed while `05:validator` present, toggleable, `05:validator` chip read-only).
- **Actions tab:** step-list editor (all 13 step types, draggable), `allowed_tools[]` multi-select, `param_schema`/`param_template` editor, dry-run test runner. No token budget enforcement.
- **Validation Queue tab (4 tabs):** Q1 Auto (AutoFailed, read-only), Q2 Manual (Validate/Reject buttons, LLM-audit guard for class 10/50), Q3 Revision (read-only, revision history), Q4 Rejection (Re-review/Delete buttons). Tag-chip greyed rendering. Badge counts.
- **Monty VM tab:** editable resource limits + `prior_knowledge_token_budget` + `q4_retention_days` + `forensic_packet_retention_days`; active orchestrator dropdown (Validated only); **"Restart Monty" button** (confirmation dialog, status indicator polls `GET /api/settings/monty-vm/status`); settings hash drift detection.
- **Chat window additions:** disambiguation chat message type (clickable buttons, structured `{disambiguation_choice}` payload); **"AI before User" flip switch** (toggle persists to `reborn_user_preferences`; hidden/disabled in DB-less mode).
- **Interceptor Config tab:** Sempai status + mode; "Reassemble Basic Prompt" button (direct SQL to component tables); "Pre-warm Sempai KV-cache" button (handles 429); persona editor; `components_since_rebuild` badge; Recent Sempai Suggestions list; hidden in DB-less mode.
- **Validation Config sub-panel:** per-class thresholds editable (name/description/token_budget/require fields); save → applies to next validation cycle only.
- **"use for embedding" button** (third provider role button in provider config UI).

> ❌ `brassclaw_webui_v2_static` appears to be a static JS app (no modern React SPA structure found). No evidence of the 10-tab Settings editor, disambiguation UX, "AI before User" flip switch UI, "use for embedding" button, Monty VM restart UI, or Interceptor Config tab in the frontend code. The entire frontend Settings section described here is NOT implemented.

---

### Step 8.5 — Phase 6.1: PKC Formatting Split (raw PKC vs. LLM-formatted PKC)
> **Supersedes Phase 8 of `plan.md` with corrected step numbering and addenda.**

#### Step 8.5.1 — Add `formatted_content` to `PriorKnowledgeResult` ✅ `IMPLEMENTED`
Add `formatted_content: String` field to `PriorKnowledgeResult` (Phase 5 type). Raw `content` remains (Rust-internal dispatch/KV-cache fingerprinting only; **never sent to LLM**).

> ✅ `PriorKnowledgeResult` with `formatted_content` field confirmed in `orchestrator.rs`.

#### Step 8.5.2 — Implement `format_prior_knowledge_for_llm()` ✅ `IMPLEMENTED`
Deterministic JSON serialization via `serde_json::json!`: ordered `(class_code asc, prompt_uid asc)`, `class_code_label()` for string names, NULL fields omitted (not serialized as `null`). KV-cache stability: same component set → byte-identical output across turns.

> ✅ `format_prior_knowledge_for_llm()` in `orchestrator.rs` confirmed with deterministic JSON serialization.

#### Step 8.5.3 — Wire `format_prior_knowledge_for_llm()` into `__assemble_prior_knowledge__` ✅ `IMPLEMENTED`
Both `content` (raw) and `formatted_content` (JSON) populated on return. `formatted_content` is the only surface sent to `working_messages`.

> ✅ Both fields populated on return from `__assemble_prior_knowledge__`. `formatted_content` used as the surface in `working_messages`. Confirmed in `orchestrator.rs` + `default.py`.

#### Step 8.5.4 — Update step-0 block in `default.py` ✅ `IMPLEMENTED`
```python
if step == 0:
    token_budget = config.get("prior_knowledge_token_budget", 100000)  # addendum: raised default
    result = __assemble_prior_knowledge__(goal, token_budget, "02")
    if result.override_prompt_creation:
        working_messages = [{"role": "User", "content": result.formatted_content}]
    elif result.formatted_content:
        insert_as_user_message_at_n_minus_1(working_messages, result.formatted_content)
    insert_volatile_context_at_n_minus_1(working_messages)  # always, both paths
```
**Addendum:** `prior_knowledge_token_budget` default raised from `2000` → `100000` in `default.py` (config key override still applies for constrained deployments).

> ✅ Step-0 block in `default.py` confirmed with `prior_knowledge_token_budget=100000` default. `insert_as_user_message_at_n_minus_1` + `insert_volatile_context_at_n_minus_1` both called correctly.

#### Step 8.5.5 — Unit + integration tests 🔸 `PARTIAL`
- `format_prior_knowledge_for_llm()` is deterministic for identical input.
- `__assemble_prior_knowledge__` returns valid JSON in `formatted_content` for mixed classes.
- KV-cache stability: same component set → byte-identical `formatted_content` across two calls.
- Volatile injection: `insert_volatile_context_at_n_minus_1` called on both Override and Normal paths.
- Regression: raw `content` never appears in messages passed to `__llm_complete__`.

> ⚠️ Tests not confirmed present for all 5 points above. The determinism + KV-cache stability tests were not located in the sweep.

**Consumer tag filtering status note:** `consumer_tags` filtering is implemented in `unified_store.rs` (`AND $5 = ANY(consumer_tags)`), `db_tool_source.rs`, and `pg_recipe_store.rs`. The `sender_class_code` → consumer-tag conversion in `__assemble_prior_knowledge__` is a Phase 5 stub (ignored; full enforcement wired in Phase 5.5 interceptor as defence-in-depth until then).

---

### Step 9 — Phase 7: Final cleanup

#### Step 9.1 — Delete on-disk SKILL.md discovery code ❌ `NOT IMPLEMENTED`
Delete `brassclaw_skills::registry` filesystem discovery; delete `migrated_skills.rs` + `bundled_skills.rs` v1→v2 blob migration; delete v1 skill shim + `skill_migration.rs` bridge.

> ❌ `crates/brassclaw_reborn_composition/src/bundled_skills.rs` still present. `build.rs` still embeds `migrated_skills_catalog.json`. These are the explicit deletion targets of this step and they have not been deleted.

#### Step 9.2 — Grep-verify all dead code is gone ❌ `NOT IMPLEMENTED`
Confirm no remaining: `signals_tool_intent`, `signals_execution_intent`, `llm_signals_tool_intent`, `user_signals_execution_intent`, `score_skill`, `extract_explicit_skills`, `format_docs`, `format_skills`, `append_system_append`, `DocType::`, `SkillTrust` in production code. Confirm `doc_type_weight`/`keyword_match_score`/`extract_keywords` gone from DB-mode `retrieval.rs` (exist only in `retrieval_dbless.rs`).

> ❌ Cannot pass: `score_skill()` in `selector.rs`, `format_docs_as_context` in `context.rs`, `DocType::` (119+ refs), `SkillTrustLevel` (full enum) all still present. Grep would fail on multiple targets.

#### Step 9.3 — Demote `BRASSCLAW_ORCHESTRATOR_MAX_DURATION_SECS` ❌ `NOT IMPLEMENTED`
Demote to DB-less fallback only (production reads from `reborn_monty_vm_settings`).

> ❌ `PgMontyVmSettingsStore` not wired (Step 6.3 gap) — demotion cannot happen until the DB read path for `reborn_monty_vm_settings` is implemented.

#### Step 9.4 — Remove stale route aliases ❌ `NOT IMPLEMENTED`
Remove old recipe/tool_skill-specific validation route aliases (kept during migration, now retired).

> ❌ Old route aliases confirmed still present per plan ("Old recipe/tool_skill routes kept as aliases" at Step 4.5). Cannot retire until new routes are fully proven.

#### Step 9.5 — Update AGENTS.md + CLAUDE.md + CHANGELOG.md ❌ `NOT IMPLEMENTED`
Document new architecture: DB-stored components, class codes, consumer-tag gating (§3.9), 4-queue validation lifecycle (§3.5.1), Monty VM settings DB-stored (§3.10), unified intent system (§3.12), Actions class (§3.11), Rust-owned formatting (§3.13/§3.14), intent-driven retrieval, token-budget prior-knowledge limit, "try it with AI" fallback, 9 new class codes 12–20, orchestrator formatting ban, DB-less fallback-content file, "AI before User" flip switch, interceptor architecture (§3.15). Remove dual-backend mandate (replaced by PG-9).

> ❌ `AGENTS.md` + `CLAUDE.md` do not yet document the new class-code architecture, consumer-tag gating, 4-queue lifecycle, Actions class, or interceptor architecture. These are post-implementation docs that cannot be finalized until the preceding steps are complete.

#### Step 9.6 — Final validation ❌ `NOT IMPLEMENTED`
`cargo clippy --all --benches --tests --examples --all-features -- -D warnings`; `cargo test`; `scripts/check_gateway_boundaries.py`; `scripts/reborn-e2e-rust.sh`; grep confirms deletion of all 8 intent-detection functions + 3 Python formatters + all `DocType::` references.

> ❌ Final validation gate — depends on all preceding cleanup steps (9.1–9.5) being complete. Cannot pass in current state.

---

## Summary: Implementation Status

| Status | Count | Steps |
|--------|-------|-------|
| ✅ Implemented | ~28 | PG-0, PG-1, PG-3, PG-5, PG-7, PG-11, Step 0, 1.1, 1.2, 1.5, 1.6, 2.1, 2.3, 4.3, 4.4, 4.5, 4.6, 5.1, 5.2, 5.4, 6.6, 6.8, 6.9, 7.0, 7.1, 7.2, 7.3, 7.5, 8.5.1–8.5.4 |
| 🔸 Partial | ~13 | PG-2, PG-4, PG-6, PG-9, PG-10, Step 1.3, 1.4, 2.4, 5.3, 6.3, 6.4, 6.5, 6.7, 7.4, 8.1, 8.5.5 |
| ❌ Not Implemented | ~17 | PG-8, Step 3.2, 3.3, 4.1, 4.2, 6.1, 6.2, 6.10, 7.4(Q1 routing), 8.2, 9.1–9.6 |
| ⚠️ Flag/Deferred | ~3 | PG-2(live-reload), PG-4(event store pool), Step 2.2(Rust twins), 7.5(sweep) |
