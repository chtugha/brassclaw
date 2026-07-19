# integrate-postgres.md — Full PostgreSQL Migration Plan

> **Scope:** Embed a self-managed PostgreSQL process (Option A — `postgresql_embedded`),
> abandon all file-based configuration, make environment variables serviceable only
> from the systemd unit file, and migrate every persistent store (file-based and
> libSQL-based) to PostgreSQL with a better schema design.
>
> **Status:** Plan only — no code changed.
>
> **Review revision 21:** Three fixes from investigation of the secrets-guard invariant, trigger
> schema constant scope, and config serialization for nested BTreeMap fields.
> (C1 §5.5 / Phase 2 — `save_config_key` missing `reject_inline_secret` call (SECURITY). The
> existing config write path in `RebornConfigFile::parse_text()` already calls
> `reject_inline_secret()` from `secrets_guard.rs` to prevent literal API keys from being stored
> in config (confirmed: secrets_guard.rs:93; also called from llm_catalog.rs:315,409,425,438).
> The new `db_config::save_config_key` omits this guard entirely — an operator or agent that
> accidentally writes `save_config_key("llm.default.api_key_env", "sk-abc123")` would store a
> literal secret in `brassclaw_config`, violating the env-only invariant and potentially
> persisting it to disk in DB backups. `save_config_key` MUST call
> `brassclaw_reborn_config::secrets_guard::reject_inline_secret(value)` before any DB write and
> return `ConfigError::InlineSecretForbidden` on failure. This is an unconditional guard —
> applies regardless of `ConfigWriteContext`. Added to §5.5 API spec and Phase 2 test checklist;
> C2 §4.24 / Phase 4 — `POSTGRES_TRIGGER_SCHEMA` DDL constant needs updating after V021 rename.
> `PostgresTriggerRepository::run_migrations()` executes `POSTGRES_TRIGGER_SCHEMA` via
> `batch_execute` (confirmed at postgres.rs:46). `POSTGRES_TRIGGER_SCHEMA` contains
> `CREATE TABLE IF NOT EXISTS trigger_records` (line 968). After V021 renames the table to
> `brassclaw_triggers`, a new deployment that calls `run_migrations()` after applying V021 would
> create a NEW `trigger_records` table alongside `brassclaw_triggers` — defeating the rename.
> `POSTGRES_TRIGGER_SCHEMA` must be updated to reference `brassclaw_triggers` alongside
> `TRIGGER_TABLE`/`TRIGGER_COLUMNS`. The Phase 4 checklist item for promoting
> `PostgresTriggerRepository` to unconditional now explicitly lists all three constants to update;
> L1 §4.2 / Phase 2 — `RebornConfigFile.llm` is `Option<BTreeMap<String, LlmSlotSelection>>`
> where each `LlmSlotSelection` has `provider_id`, `model`, `api_key_env`, `base_url`. Confirmed
> at config_file.rs:440-452. The serialization contract in §4.2 did not describe how nested
> BTreeMap entries serialize into dot-separated keys. Added a note: `llm.default.provider_id`,
> `llm.default.model`, `llm.default.api_key_env`, `llm.default.base_url` (using the slot key as
> the second path component); `load_config_snapshot` must reconstruct the BTreeMap by grouping
> rows with the `llm.` prefix. The slot key is open-ended (operators may name slots other than
> "default" or "kohai")).
> **Review revision 20:** Three correctness fixes from cross-reference of error types, event-store
> wiring paths, and conversation-state CAS semantics.
> (C1 §4.25 CAS conflict error variant wrong — plan said `InboundTurnError::ConflictRetry` but that
> variant does not exist in `InboundTurnError` (confirmed in `error.rs`: 9 variants, no
> `ConflictRetry`). The real variant is `InboundTurnError::DurableState { reason: String }` —
> the same variant the `FilesystemConversationStateStore` uses when CAS retries are exhausted
> (filesystem_store.rs:263-274). Corrected in §4.25; note added that `PgConversationStateStore`
> must return `InboundTurnError::DurableState { reason: "..." }` (not a Conflict-specific variant);
> C2 Phase 4 `PgDurableEventLog` / `PgDurableAuditLog` M7 note was incomplete — `build_local_dev()`
> does NOT use `RebornEventStoreConfig` at all; it calls `FilesystemDurableEventLog::new()` /
> `InMemoryDurableEventLog::new()` directly (confirmed in factory.rs:1785-1787). Only
> `build_postgres_production()` uses `RebornEventStoreConfig::Postgres`. After Phase 4, both build
> paths must use `PgDurableEventLog` / `PgDurableAuditLog` wired with the shared `PgPool` — the
> `build_local_dev()` path must also be updated (not just the production path); Phase 4 checklist
> item updated accordingly;
> L1 §5.3 `PgSessionThreadService` — the plan correctly names the trait `SessionThreadService` but
> the §5.3 note says "implements `SessionThreadService`" without enumerating that the trait has 16
> required methods (confirmed at service.rs:21); this is implementation-complexity context that
> implementers need; added a note to the §5.3 table row for `PgSessionThreadService` flagging the
> trait's method count and pointing to service.rs).
> **Review revision 19:** Four fixes from deep codebase investigation of the embedding adapter,
> event-store pool management, `information_schema` robustness, and `RebornEventStoreConfig` cleanup.
> (C1 §3/§4.30 `EmbeddingRoleAdapter` described the two `EmbeddingProvider` traits as "the same
> shape; the adapter is a thin pass-through" — this is wrong. Confirmed in source:
> `brassclaw_memory::EmbeddingProvider` (`embedding.rs`) has 4 methods (`dimension`, `model_name`,
> `embed`, `embed_batch`) while `brassclaw_embeddings::EmbeddingProvider` (`provider.rs`) has 5 (the
> same 4 plus `max_input_length()`). The adapter is NOT a pass-through struct; it must implement
> `brassclaw_memory::EmbeddingProvider` by delegating to the inner
> `brassclaw_embeddings::EmbeddingProvider`, mapping the 4 overlapping methods directly and
> discarding `max_input_length()` (not present on the memory seam). Corrected in §3 ADDED bullet;
> C2 §5.3 / Phase 4 / Phase 6 — `RebornEventStoreConfig::Postgres` currently opens its OWN
> `deadpool_postgres::Pool` from the config-specified URL (confirmed in
> `brassclaw_reborn_event_store/src/lib.rs` lines 481-519 and factory.rs line 2671). After this
> plan, `PgDurableEventLog` / `PgDurableAuditLog` must use the **shared `PgPool`** from composition
> — not a separately-created pool. The Phase 4 re-route note updated to call this out explicitly:
> the `RebornEventStoreConfig::Postgres { url }` variant is retired, and the stores are wired with
> the shared pool reference passed from composition. The `Libsql` and `InMemory` and `Jsonl`
> variants of `RebornEventStoreConfig` are all removed in Phase 6 — these are the complete set of
> variants (confirmed); Phase 6 checklist updated to add `Jsonl` / `InMemory` variant removal from
> the enum + the `brassclaw_reborn_event_store` crate's `Cargo.toml` libsql dep removal;
> M1 V021 `information_schema.tables` / `information_schema.columns` checks — three places use
> `WHERE table_name = '...'` without `table_schema`; in PG deployments where the search_path
> includes non-public schemas (or the table was created in a different schema) these could match
> the wrong table; added `AND table_schema = current_schema()` to all three occurrences (two
> `information_schema.tables` checks in the rename DO block, one `information_schema.columns`
> check in the trigger-creation DO block)).
> **Review revision 18:** Four targeted fixes from systematic cross-reference of DDL column names, SQL
> statement correctness, pre-seed table list completeness, and implementation-note consistency.
> (C1 §4.28 DDL `kohai_cache_creation_tokens` column name wrong — the real `KohaiUsage` field is
> `cache_creation_input_tokens` (verified in `crates/brassclaw_interceptor/src/packet.rs` line 173);
> the column has been renamed to `kohai_cache_creation_input_tokens` to match exactly; the Phase 4
> checklist and §4.28 implementation notes updated accordingly;
> C2 V021 DDL contained a stray no-op `SELECT 1 WHERE NOT EXISTS (...)` statement between the index
> definitions and the DO block that conditionally creates the `brassclaw_triggers_updated_at` trigger
> — this statement does nothing (it executes a SELECT with a WHERE clause but discards the result and
> has no side effects) and was apparently a copy-paste residue from a prototype; removed entirely;
> the DO block that immediately follows it is the correct and complete guard;
> H1 §3 migration-history reconciliation pre-seed list was incomplete — `root_filesystem_entries`
> appeared alone but `PostgresRootFilesystem` (and its libSQL equivalent) actually creates three
> sibling tables: `root_filesystem_entries`, `root_filesystem_index_specs`, and
> `root_filesystem_events`; all three are created via `CREATE TABLE IF NOT EXISTS` outside of
> refinery and all three must be in the pre-seed list to prevent V018 from attempting to re-create
> them on existing deployments; pre-seed list updated from `root_filesystem_entries` to all three
> table names;
> L1 §4.26 `brassclaw_outbound_preferences.updated_by` had `DEFAULT ''` which was under-documented —
> the `CommunicationPreferenceRecord` struct (confirmed in `communication_preferences.rs` line 35)
> has `updated_by: UserId` as a required field; the store must always supply a real UserId value from
> the record, never rely on the empty-string default at the app layer; the comment on the column is
> updated to clarify this, and to note that `DEFAULT ''` exists only as a schema-level fallback and
> must never be the actual written value).
> **Review revision 23:** Six correctness and completeness fixes from systematic second-pass
> codebase cross-reference.
> (C1 §3 `EmbeddingRoleAdapter` error mapping unspecified — the two `EmbeddingProvider` traits have
> entirely different error types: `brassclaw_embeddings::EmbeddingError` (6 variants: `HttpError`,
> `InvalidResponse`, `RateLimited`, `AuthFailed`, `TextTooLong`, `InvalidUrl`) vs
> `brassclaw_memory::EmbeddingError` (3 variants: `ProviderUnavailable`, `InvalidVector`,
> `TextTooLong`). The plan described the adapter but said nothing about how the error types map.
> Added explicit mapping table to §3: `HttpError(s)` → `ProviderUnavailable { reason: s }`;
> `InvalidResponse(s)` → `ProviderUnavailable { reason: s }`; `RateLimited { .. }` →
> `ProviderUnavailable { reason: "rate limited".into() }`; `AuthFailed` → `ProviderUnavailable {
> reason: "authentication failed".into() }`; `TextTooLong { length, max }` → `TextTooLong { length,
> max }` (pass-through — same field names); `InvalidUrl { url, reason }` → `ProviderUnavailable {
> reason: format!("invalid URL {url}: {reason}") }`. `InvalidVector` is never produced by the
> embeddings crate — it is only produced by the memory layer's own validation; the adapter has no
> path that produces it;
> C2 §4.30.1 `index_content` scope gap — the method signature
> `async fn index_content(&self, source_ref, content, chat_record_id)` has no scope parameter, but
> step 1 of the implementation notes says "the scope is supplied by the caller". These two are
> contradictory: constructing `MemoryDocumentPath` from `source_ref` requires a
> `MemoryDocumentScope` (tenant_id, user_id, agent_id, project_id), and `ChunkingMemoryDocumentIndexer`
> does not hold one at construction time (confirmed in `indexer.rs:68-73`). The scope must be an
> explicit method parameter. Fixed: `index_content` signature updated to
> `async fn index_content(&self, scope: &MemoryDocumentScope, source_ref: &str, content: &str,
> chat_record_id: Option<&str>)`. Step 1 implementation note, §4.30.1 trait listing, the §7.4 call
> site in the sequence diagram, and the Phase 4 checklist item all updated to match;
> C3 §4.29 `brassclaw_memory_chat_records` missing `run_id` index — the plan defines a `run_id`
> column and the retroactive `link_chat_record(run_id, iteration, chat_record_id)` UPDATE needs to
> look up packets by `(tenant_id, run_id, iteration)`, the retention sweep joins to
> `brassclaw_forensic_packets` by `run_id`, and `memory_search` may filter by `run_id` for
> turn-scoped retrieval; no index existed for this access pattern. Added:
> `CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_run_idx ON brassclaw_memory_chat_records
> (tenant_id, run_id, created_at DESC) WHERE run_id IS NOT NULL` to the §4.29 DDL block and updated
> the V025 migration file description in the Phase 4 checklist;
> M1 §4.28 `ForensicPacket` creation constructor unnamed — §4.28 stated "a forensic packet is
> created before Kohai is called" without naming the constructor. The real constructor is
> `ForensicPacket::new(run_id, iteration, prompt)` (confirmed in `packet.rs:179`). Added the
> constructor call signature to the §4.28 lifecycle description so implementers do not need to
> inspect the source;
> M2 §4 / Phase 9 `InterceptorStore` stale module-level comment — `store.rs`'s module comment
> currently reads "Implementations must support both PostgreSQL and libSQL". This contradicts the
> plan's Phase 0a retirement of the dual-backend mandate. Added a Phase 4 / Phase 9 task to update
> that comment to reflect the single-backend (PostgreSQL) mandate when `PgInterceptorStore` is
> wired. The comment is not a build error but leaves the next implementer with misleading guidance;
> M3 `EmbeddingRoleAdapter` `embed` vs `embed_batch` return-type note added — both the memory and
> embeddings traits use `Vec<f32>` for a single embedding. `embed_batch` returns
> `Vec<Vec<f32>>`. The adapter delegates `embed_batch` to the inner provider's `embed_batch`;
> there is no per-item fallback. This was already implied but now stated explicitly in §3).
> **Review revision 22:** Seven correctness, security, and accuracy fixes identified by systematic
> codebase cross-reference.
> (C1 §4.8 capability-leases partial index predicate had `expires_at > now()` — this is a static
> filter evaluated at index-creation time, not per-query; PG partial indexes are immutable snapshots.
> A dynamic expiry cutoff silently excludes rows that were unexpired at creation and becomes stale.
> Fixed: partial index predicate reduced to `WHERE status = 'active'` only; expiry filtering moved to
> the application-layer WHERE clause in `PgCapabilityLeaseStore` queries; corrective note added to DDL;
> C2 §4.7 adapter implementation note said "lowercase the variant name" — `RecoveryRequired.to_lowercase()`
> yields `"recoveryrequired"` (no underscore), not `"recovery_required"`. The DB CHECK values use snake_case
> (multi-word variants have underscores). The note was misleading: the adapter must use a proper
> PascalCase→snake_case converter (e.g. `heck::ToSnakeCase`), not a plain `.to_lowercase()` call.
> Corrected in §4.7 DDL comment;
> C3 §7.3 / §0 / §1c — `BRASSCLAW_RUNTIME_PROFILE` is introduced by Phase 11 and does not exist in
> the current codebase (current name is `BRASSCLAW_REBORN_PROFILE`). The §1c bootstrap tier table,
> §0 guiding-principles text, and the §7.3 systemd unit templates all referenced the post-Phase-11
> name as if already present. A service operator following §7.1/§7.2/§7.3 before Phase 11 ships
> would have a silently-ignored env var (typo-equivalent) and the process would default to `local-dev`
> instead of the intended policy. Fixed: all §7.3 unit templates now use `BRASSCLAW_REBORN_PROFILE`
> with inline notes explaining the Phase 11 rename; §1c table and §0 text amended with phase-11 qualifier;
> M1 §4.7 `brassclaw_turns` FK correctness — the plan noted "Confirm at implementation time that
> these IDs match" as a tentative caveat. Replaced with an explicit implementation invariant: the
> implementer must write the same ULID into both `id` and `run_id`, and verify the FK exists before
> inserting; the FK must be dropped and replaced with a soft reference if TurnRunId and InvocationId
> are ever decoupled;
> M2 §4.30.5 backfill contract referenced `docs/reborn/contracts/memory.md` — a stub reference to
> a document that is not verified to exist and contained no implementable content. Replaced with
> the actual three-operation contract specification inline;
> M3 §5.5 `llm_catalog.rs` call-site reference used line numbers (315, 409, 425, 438) that drift
> with each edit. Replaced with a search-instruction reference (grep `reject_inline_secret` in that
> file) paired with a function-role description;
> M4 Phase 6 checklist and §12 table both missed `brassclaw_reborn_cli/Cargo.toml` — this crate
> currently has `default = ["root-llm-provider", "libsql"]` (confirmed in source) and must have
> `libsql` removed from its default alongside `brassclaw_reborn_composition/Cargo.toml`. Added to
> Phase 6 checklist and §12 file-summary table).
> **Review revision 1:** All findings from the first cross-agent review addressed (see §0a).
> **Review revision 2:** All findings from the second cross-agent review addressed
> (C1 two-tier env model, C2 production headless passphrase, C3 systemd wizard guard,
> H1 tenant_id synthesis, H2 EnvironmentFile secrets, M1 rewrap strategy names,
> M2 pg_cron §4.13, M3 serve-only retention, M4 event_id-is-TEXT rationale, M5 process_results integrity,
> L1 full default feature line, L2 Phase 2 db_config.rs wording, L3 hardening directives added to §7).
> **Review revision 3:** All findings from the third cross-agent review addressed
> (C1 jit=off + hardened-unit test, C2 sudo -u brassclaw + file ownership, C3 fresh vs upgrade split,
> H1 brassclaw_secrets table purpose clarified, H2 algorithm default raw-key-on-disk,
> M1 wrapping threat-model candor + DR backup gap, M2 passphrase vs passphrase-file clarified,
> M3 LoadCredential note + abstraction path, M4 SystemCallFilter PG compat note,
> L1 §0 #2 bootstrap list updated, L2 operator-trusted env tier + Owner ID/WEBUI_USER_ID decoupled,
> L3 pg_cron removed from §4.18 occurred_at comment).
> **Review revision 4:** All findings from the fourth cross-agent review addressed
> (MH CLI PG lifecycle §6.4 + rewrap schema-first, M /etc/brassclaw ReadWritePaths removed,
> M §8.1 step 6 local-dev UPSERT both wrapped_key+algorithm, L1 §7.0 prerequisites block,
> L2 §6.5 --yes flag mapping, L3 rewrap vs rotate clarified in §4.4).
> **Review revision 5:** All findings from the fifth cross-agent review addressed
> (MH rewrap key-source invariant + fail-closed + passphrase-change unwrap, M §6.4 conditional
> PG shutdown, M passphrase-change old-passphrase required, L1 rotate old-version retirement,
> L2 §6.5 --no-llm + per-provider api-key-env defaults, L3 §7.0 RHEL nologin path).
> **Review revision 6:** All findings from the sixth cross-agent self-review addressed
> (C1 brassclaw_resource_accounts optimistic locking — version column + CAS UPDATE,
> C2 brassclaw_root_filesystem missing tenant_id for multi-tenant isolation,
> H1 brassclaw_approvals run_id FK missing ON DELETE clause,
> H2 BRASSCLAW_PG_URL SSL mode note for external production PG,
> H3 brassclaw_config *_env keys agent-write-gate security note,
> M1 §0 body "Secret tier" → "Operator-trusted env tier" terminology fix,
> M2 brassclaw_config.value non-string type serialization note,
> M3 brassclaw_extensions installed_at vs created_at design-philosophy consistency,
> M4 embedded PG connection URL pg_hba.conf trust-auth note,
> M5 brassclaw_turns missing tenant_id index,
> L1 Phase 7 depends on Phase 6 ordering constraint explicit note).
> **Review revision 7:** All findings from the seventh external review addressed
> (MH1 rewrap tenant-resolution unspecified — 4-step resolution + --tenant flag + §7.1/§7.2 explicit
> --tenant + Phase 7 non-default-tenant integration test + risks table row,
> M1 save_config_key missing ConfigWriteContext parameter — full signature + enum + §5.5 + §12 table update,
> M2 resource_accounts CAS first-write path — INSERT ON CONFLICT upsert pattern added to §4.12,
> L1 rewrap passphrase-change shell invocation — --old-passphrase-file flag + passphrase-read fallback chain,
> L2 ON DELETE RESTRICT comment broadened to approvals/turns/checkpoints + explicit FK clauses on turns + checkpoints,
> L3 raw-key file boot_tenant association documented in §4.4).
> **Review revision 8:** All findings from the eighth external review addressed
> (M1 RebornConfigFile::load() removal contradicts §4.4 rewrap step 2 + §8.1 step 3 — load()
> retained behind migrate-from-libsql feature; removal deferred to next release; §5.4, Phase 2
> checklist, Phase 7 checklist, §12 file table all updated,
> M2 §4.12 first-write DO UPDATE lost-update bug — replaced with INSERT DO NOTHING + read-back
> + CAS UPDATE two-step pattern that preserves CasSnapshotStore retry semantics,
> L1 §7.2 grep instruction wrong for TOML section syntax — corrected to grep tenant).
> **Review revision 9:** All findings from the ninth external review addressed — plan
> is now implementation-ready.
> (L1 §8.1 steps 3-5 not gated behind migrate-from-libsql — added module-level
> #[cfg(feature = "migrate-from-libsql")] note at top of §8.1; steps 1-2 clarified
> as unconditional,
> L2 Phase 2 missing AgentSession-succeeds-for-non-env-keys test — added third
> ConfigWriteContext test asserting gate is suffix-scoped only).
> **Review revision 10:** All findings from codebase cross-reference review addressed.
> (C1 RunRecord has no top-level thread_id — sourced from scope.thread_id; §4.5 + §5.3 updated,
> C2 FilesystemDurableEventLog/AuditLog home crate is brassclaw_reborn_event_store not brassclaw_events — §5.3 + Phase 4 + §12 corrected,
> H1 BudgetGateStore/FilesystemBudgetGateStore missing — brassclaw_budget_gates DDL added §4.22, §1a, §5.3, Phase 4, §12,
> H2 FilesystemRebornIdentityStore missing — brassclaw_identities DDL added §4.23, §1a, §5.3, Phase 4, §12,
> H3 brassclaw_hooks_postgres cfg guards must be stripped alongside feature flag removal — §5.2 + Phase 5 updated,
> H4 brassclaw_reborn_composition Cargo.toml defaults libsql not postgres — §5.5 + Phase 6 updated,
> M1 PgSafetyConfigStore implements both SafetyConfigStore + CapabilityPermissionStore in one struct — Phase 4 clarified,
> M2 recipe_store/reduction_rules_store libSQL audit note added to §5.3,
> M3 libsql feature alias needed in upgrade release Cargo.toml — §9.1 updated,
> M4 DefaultLlmSlotUpdateSession removal added to Phase 8,
> M5 FilesystemAuthProductServices source file product_auth_durable.rs noted in §5.3,
> M6 PostgresRootFilesystem already exists — §5.1 corrected,
> M7 RebornEventStoreConfig::Postgres already exists — Phase 4 event item scoped correctly,
> L3 FilesystemTurnStateStore implements 5 traits — Phase 4 updated,
> L4 RebornLibSqlIdempotencyLedger missing — §1a + Phase 6 added,
> L5 hooks_postgres rename needs workspace members + dep updates — Phase 5,
> L6 hooks factory uses in-memory backend not libSQL — Phase 5 PostgresPredicateStateBackend wiring added).
> **Review revision 11:** All findings from codebase enum/FK cross-reference review addressed.
> (C1 Phase 1 migration file count V000-V018 → V000-V020 (V019 budget_gates + V020 identities
> were added in rev 10 but Phase 1 count not updated),
> C2 brassclaw_identities.kind values wrong — "actor_binding"/"email_link" do not exist in
> SurfaceKind enum; corrected to "oauth"/"channel_actor" + CHECK constraint added,
> C3 brassclaw_budget_gates.status CHECK had "denied" — BudgetGateStatus has Cancelled not
> Denied; corrected to ('pending','approved','cancelled','expired'),
> H1 brassclaw_budget_gates.gate_kind not grounded in code — clarified as ResourceDimension
> extracted for indexing (open-ended, no CHECK), full gate in payload JSONB,
> H2 brassclaw_llm_providers soft-delete + natural key PK conflict — upsert pattern specified
> (ON CONFLICT DO UPDATE SET deleted_at = NULL for un-soft-delete),
> M1 brassclaw_processes.run_id FK lacked explicit ON DELETE RESTRICT — added,
> M2 brassclaw_process_results.process_id FK lacked explicit ON DELETE RESTRICT — added,
> M3 brassclaw_budget_gates.requested_amount NUMERIC → NUMERIC(18,6) for consistency,
> M4 brassclaw_root_filesystem.version column lacked explanation — comment added,
> M5 brassclaw_events.run_id lacked explanation for no FK — comment added).
> **Review revision 12:** All findings from post-revision-11 codebase review addressed.
> (M1 Phase 8 DefaultLlmSlotUpdateSession line reference wrong — struct at line ~476 not ~830;
> corrected to "struct at line ~476, impl body through ~974",
> L1 Phase 4 M7 note understated event store work — RebornEventStoreConfig::Postgres routes
> through PostgresRootFilesystem VFS fabric not direct SQL; corrected to "re-route" framing,
> L2 §5.3 M2 note resolved from speculation to confirmed finding — recipe_store.rs and
> reduction_rules_store.rs confirmed to have no direct libSQL; both delegate to Arc<dyn Store>,
> L3 Phase 6 checklist did not call out removing libsql from workspace default feature array
> separately from feature definition; explicit item added (current default:
> ["postgres","libsql","html-to-markdown","tui"] — libsql must be removed from the array),
> L4 §4.23 brassclaw_identities DDL was incomplete — missing provider_instance_id,
> external_subject_id, email, email_verified columns; UNIQUE index covered wrong columns;
> brassclaw_identity_users and brassclaw_identity_email_index tables missing; DDL rewritten
> from FilesystemRebornIdentityStore source; §1a and Phase 4 items updated accordingly).
> **Review revision 13:** All findings from enum/trait/codebase structure review addressed.
> (C1 §4.5 brassclaw_runs.status CHECK values wrong — plan had fictional
> 'pending'/'in_progress'/'stuck'; real RunStatus (rename_all=snake_case) is
> 'running'/'blocked_approval'/'blocked_auth'/'completed'/'failed'; corrected + source comment added,
> C2 §4.7 brassclaw_turns.status CHECK catastrophically wrong — plan had 4 values; real TurnStatus
> has 11 variants: queued/running/blocked_approval/blocked_auth/blocked_resource/
> blocked_dependent_run/cancel_requested/cancelled/completed/failed/recovery_required; corrected,
> C3 §4.11 brassclaw_extensions.status column had wrong values 'installed'/'active'/'removed' —
> real ExtensionActivationState is Installed/Disabled/Enabled (snake_case: 'installed'/'disabled'/
> 'enabled'); column renamed activation_state; §4.1 soft-delete bullet corrected; removed_at
> soft-delete design preserved with added comment explaining hard-delete→soft-delete upgrade,
> H1 §4.6 brassclaw_approvals had a 'kind' column with CHECK — ApprovalRecord has no kind field;
> the action is a typed Box<Action> (brassclaw_host_api::Action enum, 12+ variants); kind column
> removed; design note added explaining why no CHECK-constrained kind column is appropriate,
> H2 §5.3/§12 FilesystemRebornIdentityStore wired in 'factory.rs' — no factory.rs exists in
> brassclaw_reborn_identity; wiring is in brassclaw_reborn_composition/src/factory.rs;
> corrected in §5.3 table, §5.3 notes, Phase 4, §12,
> H3 §5.2/Phase 5 hooks_parity deletion — crate contains production adversarial test suite
> (parity_matrix + multi_host_adversarial); simple deletion removes the only concurrent-write
> regression gate; §5.2 and Phase 5 updated to require port-before-delete,
> M1 §5.3/Phase 3/§12 FilesystemCredentialBroker implements two traits (CredentialAccountStore
> + CredentialSessionStore) — PgCredentialBroker must implement both; note added to §5.3,
> Phase 3 checklist, and §12 table,
> M2 §5.2/Phase 5 H3 cfg-guard description wrong — guards are module-level in lib.rs (not
> file-internal); description corrected to 'strip the #[cfg] module declarations from lib.rs').
> **Review revision 14:** All findings from full store inventory + line-reference audit addressed.
> (C1 §1a/§5.3/Phase 4/§12 five missed persistent stores — `LibSqlTriggerRepository`/
> `RebornLibSqlLocalTriggerAccessStore` (`brassclaw_triggers`, `brassclaw_reborn`) +
> `FilesystemConversationStateStore` (`brassclaw_conversations`) +
> `FilesystemOutboundStateStore` (`brassclaw_outbound`) +
> `FilesystemSubagentGoalStore` (`brassclaw_reborn`) — were absent from every section;
> DDL added §4.24–§4.27 (V021–V024), store table updated, Phase 4 checklist updated,
> §12 updated; trigger_records/local_reborn_access synthesis added to §8.1 step 7 table,
> C2 §4.19 brassclaw_root_filesystem V018 covered only one table — libSQL root filesystem
> has three interdependent tables (entries, index_specs, events); V018 DDL extended to
> create all three with proper tenant_id scoping; Phase 4 verify item updated;
> H1 §4.28 InterceptorStore prompt storage — `brassclaw_interceptor` has `InterceptorStore`
> trait for `ForensicPacket` records (full prompt, response, sempai review) but the only
> current implementation is `NoopInterceptorStore` (no durable store); section §4.28 added
> documenting the current state, the stored fields, and the future DB path (V025),
> M1 Phase 1 migration file count updated V000–V020 → V000–V024 with per-file breakdown,
> M2 Phase 6 `filesystem-goal-store` feature gate removal added to checklist,
> L1 all verified line numbers confirmed correct against source (RunState:555/787,
> TurnState:80, LoopSupport:44, Resources:68/118, Processes:54/415, Auth:417,
> Threads:150, Identity:55 — all match; RebornEventStoreConfig::Postgres at line 78
> matches plan's M7 note; DefaultLlmSlotUpdateSession at line ~476 confirmed).
> **Review revision 14 (continued):** All findings from post-revision-13 enum/CHECK cross-reference review
> addressed.
> (C1 §4.10 brassclaw_processes.kind column had wrong name AND wrong CHECK — column renamed
> 'runtime' (matching ProcessRecord.runtime: RuntimeKind); CHECK corrected from fictional
> 'shell'/'docker'/'wasm'/'mcp'/'custom' to real RuntimeKind variants 'mcp'/'first_party'/'system'
> (rename_all=snake_case; Wasm and Script lanes removed in v1-removal Phase 4, now dispatch via
> Mcp; FirstParty and System have skip_deserializing but CAN be serialized to DB from host-trusted
> source); source comment added,
> C2 §4.10 brassclaw_processes.status CHECK had fictional 'pending'/'cancelled' — real
> ProcessStatus (rename_all=snake_case) is Running/Completed/Failed/Killed →
> 'running'/'completed'/'failed'/'killed'; corrected + source comment added,
> C3 §4.16 brassclaw_capability_permissions.permission_mode CHECK had fictional 'org_policy' —
> real PermissionMode (rename_all=snake_case) is Allow/Ask/Deny → 'allow'/'ask'/'deny' (3 values,
> not 4); the existing libSQL schema at safety_config_store.rs:102 already had the correct
> 3-value CHECK; 'org_policy' is from ApprovalPolicy (a different enum in runtime policy), not
> PermissionMode; corrected + source comment added,
> C4 §4.12 brassclaw_resource_accounts.scope_kind CHECK missing 3 of 6 ResourceAccount variants —
> real ResourceAccount enum has Tenant/User/Project/Agent/Mission/Thread; plan had only
> 'user'/'project'/'agent'; corrected to 'tenant'/'user'/'project'/'agent'/'mission'/'thread';
> source comment added noting ResourceAccount has no rename_all, app layer must lowercase variant
> names for the DB column,
> M1 §4.22 gate_kind example values corrected from 'spend_usd'/'token_count' to real
> ResourceDimension values 'usd'/'input_tokens' (rename_all=snake_case),
> M2 §4.10 brassclaw_processes.spec column documented as serialised ProcessRecord payload
> (grants, mounts, estimated_resources — no direct 'spec' field on ProcessRecord),
> C5 §4.10 brassclaw_process_results columns completely wrong — plan had fabricated
> exit_code/stdout/stderr/artifacts that do not exist on ProcessResultRecord; real fields are
> status (ProcessStatus), output (Option<Value> JSONB), output_ref (Option<VirtualPath> TEXT),
> error_kind (Option<String> TEXT); corrected to match ProcessResultRecord (types.rs:82-89) +
> ProcessResultStore trait (complete/fail); source comments added,
> C6 §4.9 brassclaw_session_threads schema didn't match SessionThreadRecord/ThreadScope —
> agent_id was nullable (ThreadScope.agent_id is non-optional); missing project_id, mission_id,
> created_by_actor_id, title columns; user_id NOT NULL needed SYSTEM_RESERVED_ID fallback doc;
> corrected to match source types (contract.rs:12-21, 99-107) + 2 new indexes added,
> M3 §4.16 brassclaw_safety_config.category CHECK added — SafetyCategory has 3 closed variants
> (sensitive_paths/workspace_rules/blocked_paths via as_str()); consistent with §4.1 design
> philosophy 'CHECK constraints on all enumerated columns').
> C7 §4.8 brassclaw_capability_leases was missing the status column and
> invocation_fingerprint column — CapabilityLease has a status: CapabilityLeaseStatus
> field (Active/Claimed/Consumed/Revoked, 4 variants, NO rename_all so app layer
> lowercases) that is the primary lifecycle indicator; the old partial index used
> 'revoked_at IS NULL' as a proxy for active which is incorrect (does not distinguish
> claimed/consumed from active, and misses leases that were never revoked but are no
> longer active). Added status column + CHECK + changed partial index predicate to
> 'status = active'; added invocation_fingerprint TEXT column for replay-attack
> prevention queries; documented that grant JSONB holds the full CapabilityGrant
> (id/capability/grantee/issued_by/constraints); added source comment.
> C8 §4.7 brassclaw_turns.status comment incorrectly claimed TurnStatus has
> #[serde(rename_all = "snake_case")] — the enum (status.rs:14-26) has NO
> rename_all; the serde representation is PascalCase ("Queued", "Running", etc.).
> The DB column stores lowercased variant names ('queued', 'running', etc.);
> comment corrected to document that the PgTurnStateStore adapter must
> lowercase/uppercase the variant name at the DB boundary.
> C9 §4.7 brassclaw_turns was missing the turn_id column — TurnRunRecord has
> turn_id: TurnId as a separate field from run_id: TurnRunId (store.rs:166-167);
> a turn can have multiple runs (retries/subagent spawns share the same TurnId);
> added turn_id TEXT NOT NULL column + (tenant_id, turn_id) index for "all runs
> for turn X" queries without payload JSONB scan.
> C10 §4.13 brassclaw_checkpoints table completely rewritten — the old table had
> a fabricated schema (id/sequence/payload BYTEA) that did not match
> CheckpointStateRecord (checkpoint_state.rs:55-65). Real fields: state_ref
> (LoopCheckpointStateRef), turn_id (TurnId), run_id (TurnRunId), schema_id
> (CheckpointSchemaId), schema_version (RunProfileVersion u64), kind
> (LoopCheckpointKind: before_model/before_side_effect/before_block/final via
> as_str()), payload (RedactedCheckpointPayload — opaque bytes, max 64KiB),
> created_at. Old table had wrong 'sequence' column (doesn't exist on
> CheckpointStateRecord), wrong run_id FK to brassclaw_runs(id) (TurnRunId ≠
> InvocationId — different ID spaces; changed to soft reference with comment),
> missing turn_id/state_ref/schema_id/schema_version/kind columns, missing kind
> CHECK constraint. Added natural-key unique index + run-kind index + tenant-age
> index; added schema note distinguishing CheckpointStateRecord (this table) from
> LoopCheckpointRecord (stored in brassclaw_turns.payload JSONB).
> **Review revision 15:** Three tightly coupled upgrades integrated as a single
> coherent unit — persistent chat-memory storage, `embedding` provider role, and
> dual-path memory persistence.
> (A1 §1a: three named upgrades added as explicit requirements + two new §1a table
> rows for brassclaw_memory_chat_records (V025, unconditional) and
> brassclaw_memory_embeddings (V026, conditional),
> B1 §3: pgvector dependency added to brassclaw_pg; three-role model (kohai/sempai/
> embedding) documented; dual-path data-flow overview added,
> B2 §4.17: cross-reference note added distinguishing memory_docs from
> memory_chat_records,
> C1 §4.20 V000: CREATE EXTENSION IF NOT EXISTS vector added as first statement
> before set_updated_at(); pgvector must precede all vector-column migrations,
> C2 §4.28: V025/V026 reserved — forensic_packets bumped to V027+,
> C3 §4.29 new section: brassclaw_memory_chat_records DDL (V025) with generated
> tsvector, GIN FTS + tags indexes, success_score/reinforcement columns,
> C4 §4.30 new section: brassclaw_memory_embeddings DDL (V026) with vector(1536),
> HNSW m=16/ef_construction=64 index, ON DELETE CASCADE FK to Path A row,
> D1 §5.3: PgChatMemoryRecordStore + PgMemoryEmbeddingStore added to store table,
> D2 §6.2a new section: three-role provider-config UI; "use for embedding" button
> layout, config keys, activation logic, no separate settings toggle,
> E1 §7.4 new section: dual-path memory write sequence diagram; embedding
> preconfiguration note; on-activation backfill description,
> E2 §8.1 steps 9-10: unconditional chat-memory activation (step 9) + embedding
> role bootstrapping and optional backfill-embeddings command (step 10);
> migration-dry-run updated to steps 3-10,
> F1 Phase 1: migration count V000-V026; pgvector Cargo dep; initdb pgvector
> install; FK cascade test,
> F2 Phase 4: PgChatMemoryRecordStore, PgMemoryEmbeddingStore, three-role
> preconfiguration, backfill-embeddings CLI command tasks added,
> F3 Phase 6: "use for embedding" WebUI v2 button task added,
> G1 §12: ten new file-summary rows for pgvector dep, V000 update, V025, V026,
> initdb.rs, memory module/crate, factory.rs wiring, WebUI button, ingress
> endpoint, maintenance CLI subcommand).
> **Review revision 16:** Codebase cross-reference of revision 15's chat-memory +
> embedding upgrades against the actual `memory_write` tool, `ProviderRole` enum,
> `EmbeddingsConfig`, and `brassclaw_llm_providers` DDL.
> (C1 §3/§7.4 — memory_write tool mismatch: the existing `memory_write` tool
> schema (`schemas/builtin/memory_write.input.v1.json`) has `content`, `target`,
> `append`, `metadata`, `old_string`, `new_string`, `replace_all`, `timezone` —
> NOT the structured fields (`kind`, `tags`, `importance`, `context`, `summary`)
> the plan assumed. Fixed: §3 and §7.4 now specify the tool schema will be
> extended with new optional parameters; the existing filesystem-based
> `MemoryBackend` write path is explicitly documented as NOT replaced (both
> paths coexist); `PgChatMemoryRecordStore` is a NEW parallel write path),
> C2 §3 — `brassclaw_llm_providers` has NO `role` column (§4.3 DDL has only
> `tenant_id`, `id`, `definition`, `is_builtin`, timestamps, `deleted_at`);
> role assignments are stored in `brassclaw_config` (`llm.kohai.*`,
> `llm.sempai.*`, `embedding.*`); §3 text corrected to reference `brassclaw_config`
> not `brassclaw_llm_providers`; existing `ProviderRole` enum (`role.rs:11`) has
> only `Kohai`/`Sempai` — `Embedding` variant must be added; conflict check
> spec added (Kohai+Embedding and Sempai+Embedding allowed; Kohai+Sempai
> conflict remains),
> C3 §4.21 retention table — `brassclaw_memory_chat_records` and
> `brassclaw_memory_embeddings` were missing from the retention table; added
> with default-no-expiry for chat records (memory is the workspace system per
> AGENTS.md) and CASCADE for embeddings; importance >= 0.8 exempt from TTL,
> C4 §4.29 — `importance`, `success_score`, `reinforcement` columns declared as
> `NUMERIC(5,4)` (allows 0.0000–9.9999) but documented as 0.0–1.0; CHECK
> constraints added to enforce the documented range,
> C5 §4.29 — `brassclaw_memory_chat_records` was missing `project_id` and
> `agent_id` columns that `brassclaw_memory_docs` (§4.17) and
> `brassclaw_session_threads` (§4.9) have; columns + project-scoped index added,
> C6 §3 — pgvector `CREATE EXTENSION` requires the shared library
> (`vector.so`/`vector.dll`) to be bundled; the plan only said "install the
> pgvector extension" without specifying how; full bundling procedure added
> (extract from pgvector release archive, copy to PG lib/extension dirs after
> initdb, build.rs assertion to verify presence),
> C7 §3 — Embedding role conflict check spec added (a provider MAY hold
> Kohai+Embedding or Sempai+Embedding since embedding is non-LLM-inference;
> Kohai+Sempai conflict remains),
> C8 §7.4 — existing filesystem-based `memory_write` path (MEMORY.md, daily
> logs, HEARTBEAT.md via `MemoryBackend`/`FilesystemMemoryDocumentRepository`)
> was not mentioned; explicitly documented as coexisting with the new Path A
> relational write,
> C9 §7.4 — Path B embedding API errors were "silently skipped"; changed to
> `warn!` log with chat_record_id + error reason; operator can retry via
> `brassclaw maintenance backfill-embeddings`; "feature off" path (no
> embedding.provider_id) remains truly silent).

**Review revision 17 — Path B redesigned to reuse the existing chunk embedding
system instead of creating a separate `brassclaw_memory_embeddings` table.**
Revision 16 introduced V026 (`brassclaw_memory_embeddings`) + a new
`PgMemoryEmbeddingStore` as a parallel, standalone embedding store. Investigation
of the existing `brassclaw_memory` crate revealed that this was a misinterpretation:
the codebase already contains a complete chunk-based embedding system
(`ChunkingMemoryDocumentIndexer`, `EmbeddingProvider` trait,
`MemorySearchRequest` hybrid RRF/weighted fusion, `FilesystemMemoryDocumentRepository`
chunk rows with an `embedding` indexed key) — but it is currently **not wired** in
the `memory_write` / `memory_search` tool dispatch path (`build_backend()` creates
the indexer without an embedding provider; `dispatch_search()` forces
`.with_vector(false)`). Revision 17 removes the duplicate V026 system and instead
activates + extends the existing chunk system so chat-memory records are chunked,
embedded, and retrieved through the same path as workspace documents.
> (R17-A §0: Upgrade C "Dual-path memory persistence" rewritten — Path B no longer
> creates a new `brassclaw_memory_embeddings` table; instead it reuses the existing
> `brassclaw_memory` chunk system (chunk rows under `/memory/*` VFS paths with an
> `embedding` indexed key). Path A (`brassclaw_memory_chat_records` V025) is
> unchanged and remains the authoritative relational store,
> R17-B §3: pgvector dependency RETAINED — the chunk system's
> `Filter::VectorNearest` translates to pgvector queries in the Postgres VFS
> backend (`PostgresRootFilesystem`), so pgvector IS the vector database for the
> chunk system. The standalone `vector(1536)` column on V026 is removed, but the
> `vector` extension + HNSW index now back the chunk system's embedding column,
> R17-C §3: `brassclaw_embeddings` crate REFACTORED — the existing
> `EmbeddingsConfig` + `create_provider()` factory + concrete HTTP provider impls
> (OpenAI / NEAR AI / Ollama / Bedrock) are REMOVED and replaced by the unified
> provider system (the `embedding` provider role from `brassclaw_config` resolves
> to a provider definition in `brassclaw_llm_providers`, and a new adapter
> implements `brassclaw_memory::EmbeddingProvider` by calling the resolved
> provider's embedding endpoint). The crate RETAINS `CachedEmbeddingProvider`
> (LRU cache decorator), `url_check::check_base_url` (SSRF defense floor),
> `default_dimension_for_model`, and `EmbeddingProvider` trait + `EmbeddingError`
> (workspace-level seam). The `brassclaw_memory::EmbeddingProvider` trait is the
> memory-owned seam the chunk indexer calls; the workspace-level trait stays as
> the public surface for downstream non-memory callers. A local embedding model
> (e.g. Ollama `nomic-embed-text`) is the preferred default,
> R17-D §4.29: V025 `brassclaw_memory_chat_records` gains a `source_ref` indexed
> text column (nullable) that stores the canonical VFS path of the chunk set
> derived from this record (e.g. `/memory/chat/<chat_record_id>`). This lets the
> chunk system join back to the Path A row for scoring/reinforcement without
> embedding the structured metadata into the chunk payload,
> R17-E §4.30: V026 (`brassclaw_memory_embeddings` DDL) REMOVED entirely. The
> section is replaced with the **file-less chunk creation** specification — a new
> `index_content(source_ref, content)` method on `MemoryDocumentIndexer` that
> chunks + embeds content directly from an in-memory string, without requiring a
> parent document file to exist on the VFS. The existing
> `replace_document_chunks_if_current()` path is extended so that when a
> `source_ref` is supplied without a parent document, the chunks are written
> directly under a synthetic `/memory/chat/<chat_record_id>/*.chunks/` subtree
> and a synthetic `doc_relative_path` is derived from the `source_ref`. Documents
> ingested via the new `memory_write --kind=document` path (transient documents)
> are chunked + embedded from memory and never persisted to the filesystem,
> R17-F §4.21: `brassclaw_memory_embeddings` row REMOVED from the retention
> table; replaced with a `memory_chunks (VFS)` row that cascades with
> `brassclaw_memory_chat_records` via the `source_ref` link (the chunk subtree is
> deleted when the Path A row is pruned),
> R17-G §5.3: `PgMemoryEmbeddingStore` row REMOVED from the store table;
> replaced with `ChunkingMemoryDocumentIndexer` (existing, re-wired) +
> `EmbeddingProvider` adapter (new, lives in `brassclaw_reborn_composition`),
> R17-H §6.2a: embedding role UI activation logic rewritten — pressing "use for
> embedding" no longer preconfigures a `PgMemoryEmbeddingStore`; instead it
> causes `build_backend()` at composition startup to wire the resolved embedding
> provider into `RepositoryMemoryBackend::with_embedding_provider(...)` and
> `ChunkingMemoryDocumentIndexer::with_embedding_provider(...)`, and
> `dispatch_search()` to stop forcing `.with_vector(false)`. The "feature off"
> path is the absence of an `embedding`-role provider — `build_backend()` then
> creates the indexer without an embedding provider (the current behaviour),
> R17-I §7.4: memory-write data-flow rewritten — Path B now (1) generates
> `chat_record_id` (shared with Path A), (2) calls
> `indexer.index_content(source_ref=/memory/chat/<id>, content=<memory text>)`
> which chunks + embeds + writes chunk rows under the synthetic subtree, (3) on
> embedding API error logs `warn!` with `chat_record_id` + reason and persists
> text-only chunks (embedding=NULL) so FTS stays current while vector search
> degrades — the existing indexer degrade-to-text-only behaviour is preserved,
> R17-J §8.1: onboarding step 10 rewritten — `backfill-embeddings` now reads
> `brassclaw_memory_chat_records` rows whose `source_ref` is NULL or whose chunk
> subtree has no `embedding` indexed key, calls `indexer.index_content(...)` for
> each, and is idempotent. No V026 table is involved,
> R17-K Phase 1/4/6: V026 migration file + `PgMemoryEmbeddingStore` tasks
> REMOVED; replaced with chunk-system wiring tasks (extend
> `MemoryDocumentIndexer` trait with `index_content`, wire embedding provider
> into `build_backend()`, flip `dispatch_search()` `.with_vector(true)` when
> embedding role is active, add `source_ref` column to V025 migration,
> `brassclaw_embeddings` crate refactor tasks,
> R17-L §11/§12: V026 migration file row REMOVED from §12 file summary;
> `PgMemoryEmbeddingStore` row REMOVED; `brassclaw_embeddings` crate change row
> UPDATED to reflect the refactor (remove `EmbeddingsConfig` + `create_provider`
> + concrete HTTP impls; keep `CachedEmbeddingProvider` + `url_check` +
> `default_dimension_for_model` + workspace-level `EmbeddingProvider` trait);
> new row added for `brassclaw_memory` trait extension (`index_content` method),
> R17-M §11 risks: new risk row added — "chunk embedding system wiring
> activation surfaces latent bugs in the existing (currently-unwired) indexer
> path; mitigated by Phase 4 integration test that drives
> `memory_write` → `memory_search` with vector enabled end-to-end",
> R17-N §4.29 ordering invariant: the `source_ref` column is added in V025 (not
> a separate migration) so the chunk-system wiring in Phase 4 can rely on it
> without a V026 dependency. The V025 migration file is updated in-place; no
> V026 file is ever created).

> **Review revision 17:** All findings from systematic cross-reference review addressed.
> (C1 §4.7 brassclaw_turns had a fabricated `sequence INT NOT NULL` column with a matching
> UNIQUE INDEX `(run_id, sequence)` — `TurnRunRecord` (store.rs:164-194) has NO sequence field;
> both the column and the spurious unique index are removed; a clarifying comment added explaining
> that `id` stores the TurnRunId ULID and is the natural primary key for the row;
> H1 §1a store inventory table still listed FilesystemSubagentGoalStore path as `/turns/goal/*` —
> the correct path from goal_path() is `/turns/subagent-goals/*`; corrected in §1a table;
> H2 §4.30 "No V026 file is ever created" was stale — V026 is now brassclaw_forensic_packets
> (§4.28, added in Revision 16); corrected to "V026 is brassclaw_forensic_packets, not an
> embedding table; Path B adds no new SQL migration files";
> H3 §1a last embedding-vector row still said "No separate vector table (V026 removed in
> revision 17)" which was doubly misleading; updated to remove the V026 reference and added a
> new §1a row for NoopInterceptorStore → brassclaw_forensic_packets V026 (§4.28);
> M1 §4.10 brassclaw_processes was missing a tenant+status index — "all running processes for
> tenant X" required a full table scan without it; added brassclaw_processes_tenant_status_idx).
> **Review revision 16:** All findings from full-codebase review pass addressed.
> (C1 §8.1 step 7 migration table: stale TIMESTAMPTZ cast instructions for trigger_records and
> local_reborn_access — both tables now use TEXT columns in PG (§4.24); no cast needed; replaced
> with "no type cast needed" notes;
> C2 §4.28 InterceptorStore was a deferred stub with an incomplete DDL — promoted to V026 with a
> full proper DDL derived from the real ForensicPacket struct (packet.rs): id TEXT (UUID), status
> CHECK ('awaiting_kohai'/'complete'/'sempai_reviewed'), run_id, iteration INTEGER, captured_at,
> completed_at, prompt JSONB, kohai_response TEXT, kohai_usage as four typed INTEGER columns
> (not opaque JSONB) for analytics, sempai_review JSONB; PgInterceptorStore implementation notes
> added (upsert pattern, list_recent tenant scoping requirement, link_chat_record UPDATE helper);
> C3 User requirement: link interceptor stored prompts to chat-memory records — bidirectional
> soft-reference link added: brassclaw_forensic_packets.chat_record_id (populated retroactively
> by the memory-write path) + brassclaw_memory_chat_records.forensic_packet_id (set when writing
> the memory record); both are nullable soft references (no FK) with indexed lookups; linking
> method link_chat_record(run_id, iteration, chat_record_id) is a best-effort UPDATE (no
> tenant_id parameter — store is constructed with tenant_id at wire-up time; see Revision 18);
> H1 §4.21 retention table was missing brassclaw_forensic_packets — added with 90-day default +
> pre-deletion NULL-out of forensic_packet_id on linked memory records;
> M1 Phase 1 migration file count updated V000-V025 → V000-V026; test item updated;
> M2 §5.3 store table was missing NoopInterceptorStore → PgInterceptorStore row — added;
> M3 Phase 4 checklist missing PgInterceptorStore and chat-memory↔packet linking tasks — added).
> **Review revision 15:** All findings from post-revision-14 codebase cross-reference review addressed.
> (C1 §4.24 brassclaw_triggers DDL had wrong column types (TIMESTAMPTZ) and wrong index definitions —
> real postgres.rs schema uses TEXT for all date columns (next_run_at, last_run_at, last_fired_slot,
> active_fire_slot, created_at); all four index definitions updated to match POSTGRES_TRIGGER_SCHEMA
> verbatim (correct column sets + orders); partial index WHERE corrected from 'active' → 'active_fire_slot
> IS NOT NULL' and 'scheduled' per actual query usage; added note that the table rename
> trigger_records → brassclaw_triggers requires all query constants (TRIGGER_TABLE, TRIGGER_COLUMNS)
> in postgres.rs to be updated,
> C2 §3 history-reconciliation bootstrap list was missing trigger_records and local_reborn_access —
> both tables already exist in production Postgres deployments (created by PostgresTriggerRepository
> and RebornLibSqlLocalTriggerAccessStore migrations); added to the pre-seed list,
> H1 §4.25 brassclaw_conversation_state CAS contract underspecified — added explicit
> UPDATE WHERE revision = $expected_revision pattern with RETURNING, concurrent-writer behaviour,
> and the requirement that PgConversationStateStore check rows-affected to detect conflicts,
> H2 §4.26 brassclaw_outbound_preferences used address_hash as the PK — the real
> CommunicationPreferenceKey is (tenant_id, user_id) per communication_preferences.rs; address_hash
> is only a filesystem path-encoding convenience; table rewritten with tenant_id + user_id columns +
> five typed preference columns (final_reply_target, progress_target, approval_prompt_target,
> auth_prompt_target, default_modality) as JSONB; address_hash dropped,
> H3 §4.26 brassclaw_outbound_policies stored thread_scope_key as an opaque TEXT — the real
> ThreadScopeKey has four structural fields (tenant_id, agent_id?, project_id?, thread_id); replaced
> with normalized columns so queries can target specific agents/projects/threads efficiently,
> H4 §4.27 brassclaw_subagent_goals used a JSONB payload column — SubagentGoal has only two scalar
> fields (task: String, handoff: Option<String>); normalized to TEXT columns; design note rewritten
> to reflect that the store key is (tenant_id, run_id) with a per-run UNIQUE constraint; design
> comment corrected to reference actual path /turns/subagent-goals/* not /turns/goal/*,
> M1 §4.3 brassclaw_llm_providers.is_builtin column — plan text says "built-in providers are never
> stored" yet the DDL carried is_builtin BOOLEAN NOT NULL DEFAULT false; the column is dead weight
> (never set to true by any code path, not referenced by any query); removed from DDL and upsert
> example; L1 revision header ordering — revision 13 appeared after revision 14 in the header block;
> order corrected).

---

## 0a. Exceptions to Current AGENTS.md Rules (sign-off required)

This plan intentionally supersedes two standing rules in `AGENTS.md`/`CLAUDE.md`.
Both must be explicitly rewritten as part of Phase 9 and need sign-off before
implementation begins.

**Rule being retired:**
> "New persistence behavior must support both PostgreSQL and libSQL.
>  Add new DB operations to the shared DB trait first, then implement both backends."

**Rationale for retirement:** The embedded-PG model (§2) makes libSQL redundant as
a runtime substrate. Maintaining dual backends requires every new store to be written
twice, tested in a parity suite, and kept in sync — a constraint that was justified
when libSQL was the simpler local-dev option. With an auto-managed embedded Postgres,
the cost of dual backends is no longer offset by a simpler local path.

**Specific AGENTS.md/CLAUDE.md lines to rewrite in Phase 9:**
- `AGENTS.md` "Database Rules" section: remove the dual-backend mandate; replace with
  "All persistence uses Postgres. In-memory backends are acceptable for unit tests only."
- `CLAUDE.md` "Database" section: same.
- `CLAUDE.md` "Key Traits" table: remove `Database` row (v1 trait no longer exists
  in any non-test path).
- `CLAUDE.md` `src/` structure section: purge all `src/db/`, `src/channels/`,
  `src/agent/`, `src/workspace/`, `src/sandbox/`, `src/registry/`, `src/tunnel/` docs —
  these describe v1 code removed in Phase 6 that is still documented as if live.

---

## 0. Guiding Principles

1. **Single source of truth.** All mutable state lives in Postgres. The filesystem is
   only used for the `BRASSCLAW_REBORN_HOME` pointer and binary artifacts (the embedded
   PG data directory, compiled skills bundles).
2. **Two-tier env var model.** The runtime reads env vars in two distinct tiers:
   - **Bootstrap tier (fixed set, read before DB starts):** `BRASSCLAW_REBORN_HOME`,
     `BRASSCLAW_RUNTIME_PROFILE` (introduced by Phase 11 — current name is `BRASSCLAW_REBORN_PROFILE`; see §Phase 11 for the rename), `BRASSCLAW_PG_URL`, `BRASSCLAW_EMBEDDED_PG_PORT`,
     `BRASSCLAW_REBORN_LOG`, `BRASSCLAW_SECRETS_PASSPHRASE_FILE`. These are the
     only vars that affect startup before Postgres is available (the last is
     ceremony-dependent: set when master key is passphrase-wrapped (see §4.4);
     absent for raw-key-on-disk ceremony).
   - **Operator-trusted env tier (data-driven, read by configured name after DB is up):** WebUI
     token, WebUI user-id, provider API keys, OAuth client secrets, trigger auth
     tokens, traces bearer token. The *names* of these env vars are stored in
     `brassclaw_config`; the *values* are read from the environment at runtime and
     never persisted. Because the names are operator-configurable, the set of secret
     env vars is unbounded — the runtime cannot enforce a closed allowlist here.
     The security boundary is: **config controls which names are read; values never
     touch the DB or any log**.
   All other configuration lives in the `brassclaw_config` Postgres table.
3. **Embedded Postgres is the default.** No external Postgres required. On first run,
   `postgresql_embedded` downloads the platform binary, runs `initdb`, and starts the
   server inside `$BRASSCLAW_REBORN_HOME/postgres/`. An external Postgres URL
   (`BRASSCLAW_PG_URL`) overrides this for production deployments.
4. **Dual-backend invariant is dissolved.** The libSQL / file-system dual path is
   eliminated. Every store implements the shared trait against Postgres only.
   In-memory backends are kept for unit tests only (behind `#[cfg(test)]`).
   *This directly supersedes the AGENTS.md dual-backend rule — see §0a.*
5. **No breaking change to agent-facing contracts.** Trait boundaries
   (`TurnCoordinator`, `HostRuntime`, `SecretStore`, etc.) stay identical. Only
   concrete implementations are swapped.
6. **Migration is non-destructive.** The first boot reads any existing
   file-based state and writes it to Postgres before removing the files.
   Down-migrations are not required.

---

## 1. Inventory of Everything Being Removed or Migrated

### 1a. File-based stores (all become Postgres tables)

> **Three co-delivered upgrades (added alongside the base PostgreSQL migration):**
>
> **Upgrade A — Persistent chat-memory storage by default.** Chat-memory operations
> (`memory_write`, `memory_read`, and related calls) must write to and read from
> PostgreSQL unconditionally. This is not optional and is not gated behind any
> feature flag or settings toggle. Every `memory_write` call must persist a
> human-readable, structured record in `brassclaw_memory_chat_records` (the
> "Path A" relational store described in §4.29 and §7). This table is the
> authoritative source for scoring, reinforcement, success control, recipe
> creation, skill creation, and all non-retrieval use cases.
>
> **Upgrade B — `embedding` provider role class.** A third provider role is added
> alongside `kohai` and `sempai`, named `embedding`. A third action button —
> **"use for embedding"** — is added to the provider-config UI next to the
> existing "use as sempai" and "use as kohai" buttons. Memory embedding (Path B,
> §7) can be enabled in settings if and only if a provider has been assigned the
> `embedding` role via this button and is active. The `embedding` role **replaces**
> the existing `brassclaw_embeddings` crate's standalone HTTP provider connections
> (OpenAI / NEAR AI / Ollama / Bedrock impls behind `create_provider()`); those
> are removed and the unified provider system routes embedding calls through the
> provider definition stored in `brassclaw_llm_providers` + the role config in
> `brassclaw_config` (see §3). A local embedding model (e.g. Ollama
> `nomic-embed-text`) is the preferred default. See §4.2 (config keys),
> §4.29–§4.30 (DDL + chunk-system spec), §6 (provider-config UI), and §7
> (data-flow).
>
> **Upgrade C — Dual-path memory persistence (revision 17: chunk-system reuse).**
> Every `memory_write` follows two parallel paths:
> - **Path A — Readable storage (PostgreSQL relational, always active):** The
>   memory is written to `brassclaw_memory_chat_records` in a human-readable,
>   structured format. This is the default and the authoritative non-retrieval
>   source. It executes unconditionally, even when Path B is disabled. The row
>   carries a `source_ref` column (nullable) that stores the canonical VFS path
>   of the chunk set derived from this record (e.g.
>   `/memory/chat/<chat_record_id>`), so the chunk system can join back to the
>   Path A row for scoring/reinforcement.
> - **Path B — Chunk + vector storage (pgvector via the existing chunk system,
>   optional):** When a provider has been assigned the `embedding` role and is
>   active, the memory content is chunked (using the existing
>   `ChunkingMemoryDocumentIndexer` word-overlap chunker), each chunk is sent to
>   the embedding model, and the resulting chunk rows + vectors are stored in
>   the **existing `brassclaw_memory` chunk subsystem** — chunk rows live as VFS
>   entries under `/memory/chat/<chat_record_id>/*.chunks/` with an `embedding`
>   indexed key, and the `Filter::VectorNearest` filter translates to pgvector
>   queries in `PostgresRootFilesystem`. There is **no separate vector table**;
>   the chunk system IS the vector store, and pgvector IS the vector database
>   backing it. The `pgvector` extension must be present from the very first DB
>   initialisation (see V000 in §4.20 and §4.30). The new
>   `index_content(source_ref, content)` method on `MemoryDocumentIndexer`
>   (§4.30) enables file-less chunk creation: chunks are produced directly from
>   an in-memory string, without persisting a parent document to the filesystem.
>   If no provider holds the `embedding` role, Path B is silently skipped
>   (`build_backend()` creates the indexer without an embedding provider — the
>   current behaviour); Path A is unaffected.

| Current file / path | Data it holds | New location |
|---|---|---|
| `$REBORN_HOME/config.toml` | Boot profile, LLM slot selections, WebUI settings, budget defaults, trigger poller settings, skill/token flags | `brassclaw_config` table |
| `$REBORN_HOME/providers.json` | Custom LLM provider definitions | `brassclaw_llm_providers` table |
| `$REBORN_HOME/sempai_provider.json` | Sempai role selection (`provider_id`, `model`) | `brassclaw_config` table (`sempai.*` keys) |
| `$REBORN_HOME/.reborn-local-dev-secrets-master-key` | AES-256 master key for the secret store | `brassclaw_secrets_master` table (key encrypted by hardware keyring or derived from PBKDF2 of a short passphrase at first-run) |
| Virtual path `/runs/*` | Run state records | `brassclaw_runs` table |
| Virtual path `/approvals/*` | Approval requests | `brassclaw_approvals` table |
| Virtual path `/turns/*` | Turn state | `brassclaw_turns` table |
| Virtual path `/capabilities/*` | Capability leases | `brassclaw_capability_leases` table |
| Virtual path `/processes/*` | Process records | `brassclaw_processes` table |
| Virtual path `/process-results/*` | Process results | `brassclaw_process_results` table |
| Virtual path `/extensions/*` | Extension installation state | `brassclaw_extensions` table |
| Virtual path `/resources/*` | Resource governor accounts | `brassclaw_resource_accounts` table |
| Virtual path `/resources/budget-gates.json` | Budget approval gate state (`BudgetGateStore`) | `brassclaw_budget_gates` table |
| Virtual path `/checkpoint-state/*` | Agent loop checkpoint state payloads (`FilesystemCheckpointStateStore`) | `brassclaw_checkpoints` table |
| Virtual path `/sessions/*` | Session thread service state | `brassclaw_session_threads` table |
| Virtual path `/events/*` | Durable event log | `brassclaw_events` table |
| Virtual path `/audits/*` | Durable audit log | `brassclaw_audit_log` table |
| Virtual path `/system/extensions/*` | Extension manifests (TOML) | `brassclaw_extension_manifests` table |
| Root filesystem generic entries (libSQL `root_filesystem_entries`) | All VFS blobs | Merged into domain tables above; `root_filesystem_entries` fallback kept for unrecognised paths |
| libSQL `settings` table (token settings) | Per-provider token budget settings | `brassclaw_token_settings` table |
| libSQL `safety_config` table | Safety rules & capability permissions | `brassclaw_safety_config` + `brassclaw_capability_permissions` tables |
| libSQL `memory_docs` table | Reduction rules, skill MemoryDocs | `brassclaw_memory_docs` table |
| `hooks_predicate_invocations` / `hooks_predicate_values` (both libSQL and Postgres) | Hook predicate state | Same tables, but now the canonical Postgres backend; libSQL path removed |
| VFS identity records (in `brassclaw_reborn_identity` via `FilesystemRebornIdentityStore`) | OAuth identities (external identity records), canonical user profiles, verified-email cross-provider link index | `brassclaw_identities` + `brassclaw_identity_users` + `brassclaw_identity_email_index` tables (all in V020) |
| `RebornLibSqlIdempotencyLedger` (in `brassclaw_product_workflow_storage`, `#[cfg(feature = "libsql")]`) | Idempotency ledger for product workflow actions | `RebornPostgresIdempotencyLedger` already exists in same crate; libSQL variant deleted in Phase 6 |
| libSQL `trigger_records` table (`LibSqlTriggerRepository` in `brassclaw_triggers`) | Scheduled trigger definitions, schedule state, last-run metadata, active fire slot | `brassclaw_triggers` table (V021) — Postgres backend already exists as `PostgresTriggerRepository`; this migration routes all users through the canonical PG table |
| libSQL `local_reborn_access` table (`RebornLibSqlLocalTriggerAccessStore` in `brassclaw_reborn`) | Local-dev bootstrap access grants (role + status for trigger-fire authorization) | `brassclaw_local_access` table (V021) — local-dev only; no multi-tenant migration needed; synthesised from `boot_tenant` + `boot_user` |
| Virtual path `/conversations/state.json` (`FilesystemConversationStateStore` in `brassclaw_conversations`) | Conversation actor pairings, thread records, accepted messages, reply targets, message idempotency keys, external event routes | `brassclaw_conversation_state` table (V022) |
| Virtual path `/outbound/*` (`FilesystemOutboundStateStore` in `brassclaw_outbound`) | Outbound notification policies, projection subscription cursors, delivery attempt records, communication preferences | `brassclaw_outbound_policies`, `brassclaw_outbound_subscriptions`, `brassclaw_outbound_deliveries`, `brassclaw_outbound_preferences` tables (V023) |
| Virtual path `/turns/subagent-goals/*` (`FilesystemSubagentGoalStore` in `brassclaw_reborn`) | Subagent goal records | `brassclaw_subagent_goals` table (V024) |
| In-memory / transient chat-memory records (`memory_write` / `memory_read` tool calls) | Human-readable, structured memory entries written by the agent during conversation | `brassclaw_memory_chat_records` table (V025) — **default-on, unconditional**; carries `source_ref` linking to the chunk subtree |
| In-memory / transient embedding vectors derived from chat-memory records | pgvector embeddings of chat-memory chunks (when `embedding` provider role is active) | **Existing `brassclaw_memory` chunk subsystem** — chunk rows under `/memory/chat/<chat_record_id>/*.chunks/` VFS paths with an `embedding` indexed key; `Filter::VectorNearest` translates to pgvector queries in `PostgresRootFilesystem`. No separate vector table — Path B reuses the VFS backing table (revision 17). Optional Path B; skipped when no `embedding`-role provider is assigned |
| `NoopInterceptorStore` / in-process `ForensicPacket` capture (`brassclaw_interceptor`) | One forensic packet per agent-loop turn/iteration: prompt, response, sempai review, token usage | `brassclaw_forensic_packets` table (V026) — links to `brassclaw_memory_chat_records` via `chat_record_id` (§4.28) |

### 1b. Environment variables removed from runtime config

All of the following stop being read at runtime. Their non-secret metadata moves to the
`brassclaw_config` table (set via the CLI wizard or `config set`):

- `LLM_BACKEND`, `LLM_MODEL`, `LLM_BASE_URL` (→ `brassclaw_config` LLM slot)
- `BRASSCLAW_REBORN_GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_ID` (→ `brassclaw_config` oauth.google)
- `DATABASE_BACKEND`, `LIBSQL_PATH`, `LIBSQL_URL` (eliminated — only Postgres now)

**API keys** (`LLM_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.) and
the **WebUI bearer token** (`BRASSCLAW_REBORN_WEBUI_TOKEN`) keep their env-var
model — see §1c and the model change note below.

> **Secret-store split:** Operator-sourced secrets (API keys, WebUI token/user-id,
> trigger/traces tokens) remain **env-only**: the env-var name is stored in
> `brassclaw_config`; the value is read from the environment at serve time and
> never written to `brassclaw_secrets`. The encrypted `brassclaw_secrets` table
> holds only **runtime-obtained credentials** — OAuth refresh/access tokens and
> credential-broker secrets acquired during auth flows (e.g. `FilesystemCredentialBroker`
> in `crates/brassclaw_secrets`). This is what justifies the master-key ceremony:
> a DB-level breach cannot expose OAuth tokens without the passphrase file on the
> app host. See §4.4 for the schema and §H1 rationale.

### 1c. Environment variables (two tiers)

#### Bootstrap tier — fixed set, read before DB starts

| Variable | Purpose | Default |
|---|---|---|
| `BRASSCLAW_REBORN_HOME` | Reborn state root | `~/.brassclaw/reborn` |
| `BRASSCLAW_RUNTIME_PROFILE` | Security policy (Phase 11 rename of `BRASSCLAW_REBORN_PROFILE`; current codebase still uses the old name — see §Phase 11) | `local_dev` |
| `BRASSCLAW_PG_URL` | External Postgres URL (overrides embedded PG) | unset → use embedded |
| `BRASSCLAW_EMBEDDED_PG_PORT` | Override embedded PG port (default 5434) | unset |
| `BRASSCLAW_REBORN_LOG` | Log filter | unset |
| `BRASSCLAW_SECRETS_PASSPHRASE_FILE` | Ceremony selector: present → passphrase-wrapped master key, absent → raw-key-on-disk (see §4.4) | unset |

These six vars are the only ones read before the DB is available (or during
unwrap before secrets are accessible). They are set in the systemd unit's
`EnvironmentFile` (or shell profile for local-dev). All others are ignored at
the bootstrap stage.

#### Operator-trusted env tier — data-driven, read by configured name after DB is up

> **Renamed from "secret tier":** This tier carries both secret values (API keys,
> tokens) and non-secret identity values (WebUI user-id) that must not be
> DB-influenceable. The security property is that an attacker who can write
> `brassclaw_config` rows cannot redirect which identity or token the serve
> process trusts — those values come from the operator's own environment.

The *name* of each env var is stored in `brassclaw_config`; the *value*
is read from the process environment at serve time and never persisted. Because
names are operator-configurable, the set is open-ended — there is no closed
allowlist. The security guarantee is: **values never touch the DB, config file,
or any structured log output.**

> **LoadCredential alternative (M3):** systemd 250+ supports
> `LoadCredential=secrets-passphrase:/etc/brassclaw/master.key`, which presents
> the file at `$CREDENTIALS_DIRECTORY/secrets-passphrase` readable by the service
> user (systemd handles the ownership) — resolving the C2 ownership issue without
> `sudo -u brassclaw rewrap`. Phase 3 implementation should read
> `$CREDENTIALS_DIRECTORY/secrets-passphrase` first (if set), falling back to
> `BRASSCLAW_SECRETS_PASSPHRASE_FILE`. The §7 unit can adopt `LoadCredential=`
> once Phase 3 implements the abstraction; the current `EnvironmentFile=` path
> remains valid for older systemd.

| Config key | Default env var name | What it holds |
|---|---|---|
| `webui.token_env` | `BRASSCLAW_REBORN_WEBUI_TOKEN` | WebUI bearer token (required; secret) |
| `webui.user_id_env` | `BRASSCLAW_REBORN_WEBUI_USER_ID` | WebUI owner user-id (required; non-secret identity value, but must not be DB-influenceable — see note) |
| `llm.<slot>.api_key_env` | e.g. `OPENAI_API_KEY` | LLM provider API key |
| `oauth.<provider>.client_secret_env` | operator-named | OAuth client secret |
| `trigger_poller.<name>.auth_token_env` | operator-named | Trigger auth token |
| `tracing.bearer_token_env` | operator-named | Traces/gateway bearer token |

> **WebUI user-id vs Owner ID:** `webui.user_id_env` (env tier) and
> `identity.default_owner` (in `brassclaw_config`) are different values in
> different stores. `identity.default_owner` is the config-layer default owner for
> new sessions; `BRASSCLAW_REBORN_WEBUI_USER_ID` (env) is the identity the serve
> process asserts for bearer-token WebUI auth. They should match in a standard
> single-user deployment, but operators must keep them consistent manually — the
> wizard prompts for both (Step 2 and Step 3) and warns if they diverge.

> **Security: agent must not be able to write `*_env` config keys.** The
> `webui.token_env`, `webui.user_id_env`, `llm.<slot>.api_key_env`,
> `oauth.<provider>.client_secret_env`, `trigger_poller.<name>.auth_token_env`,
> and `tracing.bearer_token_env` config keys are security-critical: they control
> *which* environment variable the serve process trusts for auth, identity, and
> secret resolution. An agent capability that can write arbitrary `brassclaw_config`
> rows could rename these keys to arbitrary variable names — effectively rerouting
> which env value is read for auth. **The agent loop must never be granted a
> capability to write `brassclaw_config` rows whose keys match the `*_env` pattern.**
> Changes to these keys require explicit operator intent (CLI `config set` or the
> first-run wizard). This invariant is enforced at the `db_config::save_config_key`
> layer: the function rejects writes to any key ending in `_env` unless the caller
> holds an operator-tier auth context (not an agent/session context).
> Phase 2 must add a test that asserts agent-sourced `save_config_key` calls are
> rejected for `*_env` keys.

Non-secret WebUI config that was previously env-based is moved to
`brassclaw_config` so no env var is required at serve time:

| Moved to config key | Previously | Notes |
|---|---|---|
| `webui.base_url` | `BRASSCLAW_REBORN_WEBUI_BASE_URL` | SSO callback base URL — not a secret |
| `webui.allowed_email_domains` | `BRASSCLAW_REBORN_WEBUI_ALLOWED_EMAIL_DOMAINS` | SSO admission list — not a secret |

> **Production passphrase:** `BRASSCLAW_SECRETS_PASSPHRASE_FILE` is required
> for unattended production boot. Without it, the only working strategy is
> `keychain`, which requires a desktop session unavailable under headless
> systemd. See §4.4 for the full strategy table and one-time setup procedure.

---

## 2. New Crate: `brassclaw_embedded_postgres`

### 2.1 Location

```
crates/brassclaw_embedded_postgres/
├── AGENTS.md
├── Cargo.toml
└── src/
    ├── lib.rs           # pub struct ManagedPostgres; pub fn start()
    ├── config.rs        # EmbeddedPostgresConfig (port, data_dir, bin_cache_dir)
    ├── download.rs      # uses `postgresql_embedded` crate to fetch+cache PG binary
    ├── initdb.rs        # runs initdb, writes postgresql.conf tuning
    ├── pgctl.rs         # pg_ctl start/stop/status wrappers
    ├── health.rs        # retry TCP connect until ready
    └── error.rs         # EmbeddedPostgresError (thiserror)
```

### 2.2 Key decisions

- **Postgres version pinned to 16.x.** Written as a `const` in `download.rs`;
  updating it is a deliberate, reviewable commit.

- **Binary cached in `$REBORN_HOME/postgres/bin/`.** Not bundled in the Rust
  binary. Downloaded exactly once from the zonky PostgreSQL distribution via
  `postgresql_embedded`. **Checksum verification is implemented by this crate,
  not by `postgresql_embedded` itself** (the upstream crate does not verify
  checksums): after download, `download.rs` computes SHA-256 of the archive and
  compares it against a `const` compiled into the binary. If the digest
  mismatches, the archive is deleted and the process aborts. The pinned digest
  list lives in `crates/brassclaw_embedded_postgres/src/checksums.rs` and must
  be updated for every version bump via a deliberate, reviewed commit. The
  `POSTGRESQL_VERSION` and `GITHUB_TOKEN` env vars that `postgresql_embedded`
  normally reads are suppressed so an attacker who can set env cannot change the
  downloaded version. Trust root: GitHub TLS + zonky publisher + compiled-in
  SHA-256 — protects against CDN compromise but not a compromised zonky build
  pipeline; Sigstore/cosign can be added later if upstream adopts it.
  **Production recommendation:** use `BRASSCLAW_PG_URL` to point at an
  operator-managed Postgres where supply-chain trust is a hard requirement.

- **Data directory `$REBORN_HOME/postgres/data/`.** Created by `initdb` on
  first start. If a `postmaster.pid` already exists in the data dir, startup
  checks whether the recorded PID is still alive (kill -0): if yes, the server
  is already running — reuse it; if no, remove the stale PID file and restart.
  This handles the SIGKILL / crash-orphan case correctly. `initdb` is skipped
  whenever the data dir already exists and is non-empty.

- **Port `5434` by default.** Avoids collision with system Postgres on 5432.
  Configurable via `BRASSCLAW_EMBEDDED_PG_PORT` env var (§1c) or
  `EmbeddedPostgresConfig::port`. On startup, if the port is already in use
  (TCP connect succeeds), the process aborts with:
  `"embedded PG port 5434 in use — set BRASSCLAW_PG_URL or BRASSCLAW_EMBEDDED_PG_PORT"`.

- **`postgresql.conf` tuning for single-user agent workload** (conservative —
  suitable for a laptop or a modest server):
  ```
  max_connections = 20
  shared_buffers = 32MB
  work_mem = 4MB
  max_wal_size = 1GB
  autovacuum = on
  # JIT disabled: pays off for OLAP scans, not the small OLTP-ish queries
  # this server runs. Also required for MemoryDenyWriteExecute=yes in the
  # systemd unit (§7) — PG JIT compiles into executable memory at runtime,
  # which MDWE forbids. Do not enable JIT without removing MDWE.
  jit = off
  log_destination = 'stderr'
  logging_collector = on
  log_directory = 'log'
  log_filename = 'postgresql-%Y-%m-%d.log'
  log_rotation_age = 1d
  log_rotation_size = 50MB
  log_truncate_on_rotation = on
  log_min_duration_statement = 1000
  ```
  `log/` is inside the data directory; the 50 MB rotation cap prevents
  disk fill. Operators can edit `postgresql.conf` directly for tuning.
  **If you remove `MemoryDenyWriteExecute=yes` from the §7 unit, also
  remove `jit = off` here — the two settings must be changed in tandem.**

- **Connection URL format:** `postgresql://brassclaw@127.0.0.1:5434/brassclaw`
  The database and role are created by an init SQL script run after `initdb`.
  **`pg_hba.conf` trust auth:** the init script also writes a `pg_hba.conf` entry:
  ```
  host  brassclaw  brassclaw  127.0.0.1/32  trust
  ```
  This allows passwordless TCP connection from localhost only — safe for an
  embedded loopback server owned by the service user. No password is required in
  the URL. This entry is written unconditionally by `initdb.rs` and must not be
  modified to accept non-loopback connections.

- **`BRASSCLAW_PG_URL` SSL requirement for external Postgres:** when
  `BRASSCLAW_PG_URL` points at an external or remote Postgres (not `127.0.0.1`
  or `::1`), operators **must** append `?sslmode=require` (or `sslmode=verify-full`
  for mTLS) to the URL:
  ```
  BRASSCLAW_PG_URL=postgresql://brassclaw@db.example.com:5432/brassclaw?sslmode=require
  ```
  AGENTS.md: "Review any change touching listeners, auth, secrets, or outbound HTTP
  with a security mindset." Connections to a remote PG without TLS expose
  `brassclaw_secrets` ciphertext, config, and session state in transit.
  The `brassclaw_pg::pool::build_pool` function must log a `warn!`-level message
  if the URL host is not a loopback address and the URL does not contain `sslmode=`:
  ```
  [warn] BRASSCLAW_PG_URL points to non-loopback host without sslmode — TLS is strongly recommended
  ```
  (The pool still connects, to avoid breaking environments with TLS enforced
  server-side via `pg_hba.conf`; but the warning is non-suppressible.)

- **Explicit shutdown before pool drop.** `ManagedPostgres` exposes a
  `shutdown()` async method that the composition root calls *after* closing
  the connection pool. `Drop` retains a best-effort `pg_ctl stop -m fast` as
  a last-resort fallback only — it must never be the primary shutdown path
  because a blocking `pg_ctl` inside `Drop` while open pool connections exist
  can deadlock.

- **`BRASSCLAW_PG_URL` override:** If this env var is set, the embedded
  server is never started. The URL is used directly for the pool.

---

## 3. New Crate: `brassclaw_pg`

All shared Postgres pool management, migration runner, and the canonical
`deadpool_postgres::Pool` constructor live here. This replaces the scattered
`#[cfg(feature = "postgres")]` blocks in individual crates.

```
crates/brassclaw_pg/
├── AGENTS.md
├── Cargo.toml            # deadpool-postgres, tokio-postgres, refinery, thiserror, pgvector
└── src/
    ├── lib.rs            # pub struct PgPool(deadpool_postgres::Pool); re-exports
    ├── pool.rs           # build_pool(url: &str) -> Result<PgPool, PgError>
    ├── migrations.rs     # run_migrations(&pool) -> Result<(), PgError>
    └── error.rs
```

**pgvector dependency.** `brassclaw_pg` adds `pgvector` as a mandatory
dependency. The `CREATE EXTENSION IF NOT EXISTS vector;` statement is the
**first** SQL executed in `V000__shared_triggers.sql` (before the
`set_updated_at()` function), guaranteeing the `vector` type is available to all
subsequent migrations. This means `pgvector` must be installed on the Postgres
server before migrations run.

**Bundling pgvector for embedded PG:** `CREATE EXTENSION vector` requires the
pgvector shared library (`vector.so` on Linux, `vector.dll` on Windows) to be
present in PG's `lib` directory and the control file (`vector.control`) in PG's
`extension` directory. For the embedded-Postgres path (§2), the
`brassclaw_embedded_postgres` crate's `initdb.rs` must:
1. Bundle the pre-compiled pgvector shared library + control file for the
   target platform (Linux x86_64 is the primary target; macOS aarch64 is the
   dev target). The files are extracted from the `pgvector` release archive
   matching the embedded PG version (PG 16).
2. Copy them into the embedded PG data directory's `lib/` and `extension/`
   subdirectories after `initdb` completes but before the first `pg_ctl start`.
3. The V000 migration's `CREATE EXTENSION IF NOT EXISTS vector;` then loads the
   shared library and registers the `vector` type.

If the shared library is missing, `CREATE EXTENSION` fails with a clear error
message and the service refuses to boot. This is a hard dependency — pgvector
cannot be optional because the existing `brassclaw_memory` chunk system's
`Filter::VectorNearest` translates to pgvector queries in
`PostgresRootFilesystem` (the chunk `embedding` indexed key is stored as a
`vector`-typed column in the VFS backing table; see §4.30). Revision 17 removed
the standalone V026 `vector(1536)` column, but the chunk system's vector
search path still requires the `vector` type to be registered. The
`brassclaw_embedded_postgres` build script should verify the library is present
at build time (a `build.rs` assertion) to catch missing bundles early.

For external-PG operators (`BRASSCLAW_PG_URL`), the operator must ensure pgvector
is installed (`CREATE EXTENSION IF NOT EXISTS vector;` can also be issued
manually before starting the service — it is idempotent). If the extension is not
available, V000 fails and the service refuses to boot with a diagnostic
explaining how to install pgvector.

**`brassclaw_embeddings` crate refactor (revision 17).** The existing
`brassclaw_embeddings` crate currently owns a parallel embedding-provider stack:
`EmbeddingsConfig` (a separate config shape read from `Settings`), a
`create_provider(config, deps)` factory, concrete HTTP provider impls
(`OpenAiEmbeddings`, `NearAiEmbeddings`, `OllamaEmbeddings`, `BedrockEmbeddings`,
all crate-private behind the factory), the `EmbeddingProvider` trait +
`EmbeddingError` (the workspace-level seam), `CachedEmbeddingProvider` (LRU
cache decorator), `url_check::check_base_url` (SSRF defense floor), and
`default_dimension_for_model`. Revision 17 replaces the standalone
provider-config + factory path with the **unified provider system**:

- **REMOVED:** `EmbeddingsConfig`, `create_provider()`, the concrete HTTP
  provider impls (`OpenAiEmbeddings` / `NearAiEmbeddings` / `OllamaEmbeddings` /
  `BedrockEmbeddings`), the binary-side resolver
  `src/config/embeddings.rs::resolve_embeddings_config`, and the
  `Workspace::with_embeddings_cached` / `with_embeddings_uncached` wiring.
  Embedding configuration no longer has its own settings shape — it is driven
  entirely by the `embedding` provider role in `brassclaw_config`
  (`embedding.provider_id` + `embedding.model`) plus the provider definition in
  `brassclaw_llm_providers` (which already carries endpoint URL, auth, and
  model metadata).
- **RETAINED:** `EmbeddingProvider` trait + `EmbeddingError` (workspace-level
  seam — downstream non-memory callers still hold `Arc<dyn EmbeddingProvider>`),
  `CachedEmbeddingProvider` + `EmbeddingCacheConfig` (LRU cache decorator),
  `url_check::check_base_url` (SSRF defense floor — the new adapter MUST call
  it before issuing HTTP requests, matching the existing safety rule in
  `crates/brassclaw_embeddings/AGENTS.md`), `default_dimension_for_model`
  (used by the new adapter to pick a dimension when the provider definition
  does not specify one), and `MockEmbeddings` (test double, behind the
  `testing` feature).
- **ADDED:** A new `EmbeddingRoleAdapter` (in
  `brassclaw_reborn_composition`, wiring layer) that resolves the
  `embedding`-role provider from `brassclaw_config` + `brassclaw_llm_providers`
  at composition startup, constructs an HTTP client that calls the provider's
  embedding endpoint, runs `url_check::check_base_url` on the resolved base
  URL, wraps the result in `CachedEmbeddingProvider`, and exposes the
  resulting `Arc<dyn brassclaw_memory::EmbeddingProvider>` for wiring into
  `RepositoryMemoryBackend::with_embedding_provider(...)` and
  `ChunkingMemoryDocumentIndexer::with_embedding_provider(...)`. The adapter
  bridges the workspace-level `brassclaw_embeddings::EmbeddingProvider` trait
  (5 methods: `dimension`, `model_name`, `max_input_length`, `embed`,
  `embed_batch`) to the memory-owned `brassclaw_memory::EmbeddingProvider`
  trait (4 methods: `dimension`, `model_name`, `embed`, `embed_batch` —
  confirmed in `crates/brassclaw_memory/src/embedding.rs`). The two traits are
  **not** the same shape: `brassclaw_memory::EmbeddingProvider` has no
  `max_input_length()` method. `EmbeddingRoleAdapter` is a concrete struct (not
  a pass-through alias) that implements `brassclaw_memory::EmbeddingProvider` by
  delegating to the inner `Arc<dyn brassclaw_embeddings::EmbeddingProvider>`,
  mapping the four overlapping methods directly. `embed` and `embed_batch` return
  `Vec<f32>` and `Vec<Vec<f32>>` respectively on both traits — the adapter
  delegates these calls straight through; `embed_batch` delegates to the inner
  provider's `embed_batch` and does not fall back to per-item `embed` calls.

  **Error type mapping (`embed` / `embed_batch`).** The two crates use
  completely different error types. The adapter's `embed` and `embed_batch`
  implementations receive `brassclaw_embeddings::EmbeddingError` from the inner
  provider and must convert it to `brassclaw_memory::EmbeddingError` before
  returning. The required mapping (confirmed by reading both error enums in
  source):

  | `brassclaw_embeddings::EmbeddingError` (inner) | `brassclaw_memory::EmbeddingError` (outer) |
  |---|---|
  | `HttpError(s)` | `ProviderUnavailable { reason: s }` |
  | `InvalidResponse(s)` | `ProviderUnavailable { reason: s }` |
  | `RateLimited { retry_after }` | `ProviderUnavailable { reason: "rate limited".into() }` |
  | `AuthFailed` | `ProviderUnavailable { reason: "authentication failed".into() }` |
  | `TextTooLong { length, max }` | `TextTooLong { length, max }` (pass-through — same field names) |
  | `InvalidUrl { url, reason }` | `ProviderUnavailable { reason: format!("invalid URL {url}: {reason}") }` |

  `brassclaw_memory::EmbeddingError::InvalidVector` is produced only by
  `validate_embedding_dimension` inside the memory layer itself; the adapter
  has no code path that produces it.
- **PREFERRED DEFAULT:** A local embedding model (e.g. Ollama
  `nomic-embed-text`, `mxbai-embed-large`, or `all-minilm`) is the preferred
  default for the `embedding` role — this keeps the embedding call on-host,
  avoids sending memory content to an external API, and aligns with the
  AGENTS.md rule that external services are untrusted. The operator may
  configure a remote provider (OpenAI, Bedrock, etc.) via the provider
  definition if they accept that trade-off.

**Provider role model (three roles).** The provider-role assignment is stored
in `brassclaw_config` (not in `brassclaw_llm_providers` — that table stores only
provider definitions, not role assignments; see §4.3 DDL). The three role config
keys are:
- `llm.kohai.provider_id` / `llm.kohai.model` — the primary agent-execution LLM
  provider ("use as kohai" button). Existing behaviour, unchanged.
- `llm.sempai.provider_id` / `llm.sempai.model` — the review/safety LLM provider
  ("use as sempai" button). Existing behaviour, unchanged.
- `embedding.provider_id` / `embedding.model` — the embedding provider for
  memory chunk vector search ("use for embedding" button, Upgrade B — see §1a,
  §6.2a, §4.30). **New.** Resolves to a provider definition in
  `brassclaw_llm_providers` (which carries the endpoint URL, auth, and model
  metadata); the `EmbeddingRoleAdapter` (above) turns this into a concrete
  `Arc<dyn brassclaw_memory::EmbeddingProvider>` at composition startup.

Only one provider can hold each role at a time. Assigning a role to a provider
unassigns it from any other provider that currently holds it. The existing
`ProviderRole` enum (`crates/brassclaw_llm/src/role.rs:11`) has two variants
(`Kohai`, `Sempai`); a third variant `Embedding` must be added. The existing
conflict check in `set_active()` (`llm_config_service.rs:607-631`) prevents the
same provider from holding both `Kohai` and `Sempai` — a similar check must be
added for `Embedding` (a provider MAY hold `Kohai` + `Embedding` or `Sempai` +
`Embedding` simultaneously, since embedding is a non-LLM-inference role; but
`Kohai` + `Sempai` conflict remains). If no provider holds the `embedding` role,
Path B of `memory_write` (chunk embedding) is silently skipped —
`build_backend()` creates the indexer without an embedding provider (the
current behaviour); Path A (relational store) always executes regardless.

**Dual-path memory write data-flow (overview).** Every `memory_write` tool call
triggers two parallel write paths (see §7.4 for the full sequence diagram):
1. **Path A — Relational write (unconditional):** Inserts a row into
   `brassclaw_memory_chat_records` with the memory content and structured
   metadata. The existing `memory_write` tool schema
   (`schemas/builtin/memory_write.input.v1.json`) will be extended with new
   optional parameters (`kind`, `tags`, `importance`, `context`, `summary`) to
   populate the structured columns; the existing `content` parameter maps to
   the `content` column. The row also carries a `source_ref` column (nullable)
   that stores the canonical VFS path of the chunk set derived from this record
   (e.g. `/memory/chat/<chat_record_id>`), so the chunk system can join back to
   the Path A row for scoring/reinforcement. The existing filesystem-based
   `MemoryBackend` write path (MEMORY.md, daily logs, HEARTBEAT.md — stored in
   `brassclaw_memory_docs` via §4.17) is **not replaced**; both paths coexist.
   `PgChatMemoryRecordStore` is a new parallel write path alongside the existing
   `MemoryBackend`.
2. **Path B — Chunk + vector write (conditional on `embedding` role):** Calls
   `indexer.index_content(scope, source_ref=/memory/chat/<chat_record_id>,
   content=<memory text>, chat_record_id)` on the existing
   `ChunkingMemoryDocumentIndexer`. The indexer chunks the content using the
   existing word-overlap chunker (`chunk_document`, 800-word default),
   embeds each chunk via the wired `EmbeddingProvider`, and writes chunk rows
   under the synthetic `/memory/chat/<chat_record_id>/*.chunks/` VFS subtree
   with an `embedding` indexed key. `Filter::VectorNearest` then translates to
   pgvector queries in `PostgresRootFilesystem` for retrieval. No separate
   vector table is involved. Silently skipped if no `embedding`-role provider
   is active (`build_backend()` creates the indexer without an embedding
   provider — the current behaviour).

`migrations.rs` uses `refinery` to run all SQL migration files embedded via
`include_str!`. Migration files live in `crates/brassclaw_pg/migrations/`
numbered `V000__` … `Vnnn__`. Each crate that currently has its own migration
folder (`brassclaw_hooks_postgres/migrations/`) has its SQL moved here.

**Migration-history reconciliation for existing deployments.** The existing hooks
and inline-DDL tables (`hooks_predicate_invocations`, `hooks_predicate_values`,
`root_filesystem_entries`, `root_filesystem_index_specs`, `root_filesystem_events`,
`memory_docs`, `settings`, `safety_config`,
`capability_permissions`, `trigger_records`, `local_reborn_access`) were applied
via idempotent `CREATE TABLE IF NOT EXISTS` batches, not via refinery. Refinery
tracks applied migrations in `refinery_schema_history` and would try to re-run
their consolidated SQL on deployments where these tables already exist. To
prevent that:

> **`trigger_records` and `local_reborn_access`:** `PostgresTriggerRepository`
> (in `brassclaw_triggers/src/postgres.rs`) and `RebornLibSqlLocalTriggerAccessStore`
> (in `brassclaw_reborn/src/local_trigger_access.rs`) both call their own
> `run_migrations()` at startup, creating these tables directly via
> `batch_execute` / `execute_batch` outside of refinery. Any Postgres deployment
> that has started a trigger-enabled service will have `trigger_records` in the DB.
> These must be in the pre-seed list so refinery doesn't try to create V021's
> `brassclaw_triggers` on top of an existing `trigger_records`. V021 **renames**
> `trigger_records` → `brassclaw_triggers` via `ALTER TABLE trigger_records RENAME
> TO brassclaw_triggers;` (idempotent: wrapped in a DO block that checks
> `information_schema.tables`). The `TRIGGER_TABLE` constant in
> `brassclaw_triggers/src/postgres.rs` (line 16) and the `TRIGGER_COLUMNS` constant
> (line 17) must be updated to reference `brassclaw_triggers` after V021 runs.
> Similarly, the libSQL `local_reborn_access` table is created by
> `RebornLibSqlLocalTriggerAccessStore` — on Postgres deployments a Postgres-backed
> equivalent (`PgLocalTriggerAccessStore`) creates `brassclaw_local_access` directly;
> its history row must be pre-seeded to prevent V021 from conflicting.

1. On first run, `brassclaw_pg::migrations::run_migrations` checks whether
   `refinery_schema_history` is empty **and** whether any of the
   already-existing-in-the-wild tables are present.
2. If so, it inserts pre-seeded history rows marking those migrations as
   already applied (using their compiled-in checksums).
3. Only then does it run the normal `refinery::embed_migrations!` pass.

All refinery migration SQL also uses `CREATE TABLE IF NOT EXISTS` and
`CREATE INDEX IF NOT EXISTS` so they remain safe to re-run in edge cases.

---

## 4. Schema Design

### 4.1 Design philosophy

- **JSONB for flexible document fields, typed columns for everything queried
  or indexed.** Avoids the "fat blob" anti-pattern while keeping schema
  evolution cheap.
- **`ulid` as primary key everywhere** (`TEXT` NOT NULL, 26-char ULID string).
  ULIDs are monotonic-sortable, URL-safe, and more debuggable than raw UUID
  bytes. They replace the current UUID-v4 random IDs and the libSQL integer
  rowids.
- **`tenant_id` on every domain table.** Multi-tenant separation at the data
  layer. `user_id` and `agent_id` are added where semantically meaningful.
  Exception: `brassclaw_process_results` is linked to `brassclaw_processes`
  via foreign key and inherits tenant context from the parent row.
- **`created_at` and `updated_at` on every mutable table.**
  `updated_at` is maintained by a `BEFORE UPDATE` trigger (`set_updated_at()`).
  Each table's migration `CREATE TRIGGER` statement is shown in §4.20.
  `brassclaw_process_results` is insert-only (results are never modified) —
  it has only `created_at`.
- **Soft-deletes on stateful lifecycle tables.** `brassclaw_runs` and
  `brassclaw_session_threads` use `deleted_at TIMESTAMPTZ`. `brassclaw_extensions`
  uses `removed_at TIMESTAMPTZ` (domain terminology that matches
  `ExtensionInstallationStore::delete_installation` semantics — the column name
  differs deliberately). Append-only tables (`brassclaw_events`,
  `brassclaw_audit_log`, `brassclaw_checkpoints`) have no soft-delete column —
  they are pruned by TTL (see §4.21 on retention).
- **CHECK constraints on all enumerated columns.** Enumerated TEXT columns
  (status, kind, scope_kind, etc.) carry a `CHECK (col IN (...))` constraint
  to enforce DB-level integrity. See each table definition.
- **Partial indexes for common filters** (e.g., active runs, pending approvals).

### 4.2 Config table

```sql
-- V001__config.sql
CREATE TABLE IF NOT EXISTS brassclaw_config (
    tenant_id   TEXT        NOT NULL,
    key         TEXT        NOT NULL,   -- dot-separated, e.g. "llm.default.provider_id"
    value       TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, key)
    -- Note: no secondary index on tenant_id alone; the (tenant_id, key) PK
    -- already serves prefix scans for "get all config for a tenant".
);
CREATE TRIGGER brassclaw_config_updated_at
    BEFORE UPDATE ON brassclaw_config
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `config.toml` (`boot.*`, `identity.*`, `policy.*`, `drivers.*`,
`harness.*`, `runner.*`, `skills.*`, `tokens.*`, `webui.*`, `budget.*`,
`trigger_poller.*`, `llm.*`), `sempai_provider.json`.

**Non-string value serialization.** `value` is plain `TEXT`. Non-string config
values in `RebornConfigFile` (booleans, integers, decimals) are serialized as
their natural string representations:
- Booleans: `"true"` / `"false"` (lowercase, matches TOML).
- Integers: decimal string (e.g. `"20"`).
- Floating-point / monetary: decimal string (e.g. `"5.00"`).
- Optional fields absent from config: the row is simply absent (no row with
  value `"null"` or `""`). The `db_config::load_config_snapshot` function uses
  `Option` and a fallback default for every field, matching the existing
  `RebornConfigFile::default()` behaviour.
`db_config::save_config_key` and `load_config_snapshot` are the only places
this serialization contract is enforced. They must be the only callers that
read/write `brassclaw_config` rows — never raw SQL from other modules.

**Nested BTreeMap serialization (the `llm` field).** `RebornConfigFile.llm` is
`Option<BTreeMap<String, LlmSlotSelection>>` where the map key is the slot name
(e.g. `"default"`, `"kohai"`, `"sempai"`). Each `LlmSlotSelection` has four
`Option<String>` fields: `provider_id`, `model`, `api_key_env`, `base_url`.
These serialize into dot-separated keys: `llm.<slot>.provider_id`,
`llm.<slot>.model`, `llm.<slot>.api_key_env`, `llm.<slot>.base_url`. Example:
`llm.default.provider_id = "openai"`. `load_config_snapshot` must reconstruct
the BTreeMap by grouping all rows whose key starts with `llm.`, splitting on
`.`, taking the second component as the slot name, and the third component as the
field name. Absent optional fields have no row (not `"null"`).

Bootstrap sequence: on first boot, if the table is empty for the tenant, the
CLI first-run wizard writes sensible defaults here (see §6).

**Config live-reload:** `load_config_snapshot` is called once at startup. Live
reload (without restart) is not supported in v1 of this plan. This preserves
the current behaviour where config.toml is also read only at boot.

### 4.3 LLM providers table

```sql
-- V002__llm_providers.sql
CREATE TABLE IF NOT EXISTS brassclaw_llm_providers (
    tenant_id       TEXT        NOT NULL,
    id              TEXT        NOT NULL,   -- provider id, e.g. "openai-custom"
    definition      JSONB       NOT NULL,   -- ProviderDefinition JSON (no api_key values)
    -- NOTE: no is_builtin column — built-in providers are never stored (compiled in);
    -- only user-overlay providers land in this table.
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at      TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, id)
);
CREATE TRIGGER brassclaw_llm_providers_updated_at
    BEFORE UPDATE ON brassclaw_llm_providers
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `providers.json` and `ProviderRepo`. Built-in providers are never
stored (they are compiled in); only user-overlay providers land here.

**Upsert pattern (soft-delete + natural key PK):** The current `ProviderRepo`
uses hard replace and hard delete (no `deleted_at`). This table introduces
soft-delete as an improvement, but `PRIMARY KEY (tenant_id, id)` + `deleted_at`
means a soft-deleted provider's ID cannot be re-inserted without conflicting.
The `ProviderRepo` rewrite must use this upsert pattern:
```sql
INSERT INTO brassclaw_llm_providers (tenant_id, id, definition, deleted_at)
VALUES ($tenant, $id, $definition, NULL)
ON CONFLICT (tenant_id, id) DO UPDATE
SET definition = excluded.definition, deleted_at = NULL, updated_at = now();
```
This un-soft-deletes and updates in one operation. The `delete` path sets
`deleted_at = now()` (soft-delete), and a subsequent `upsert` with the same ID
clears `deleted_at` (un-soft-delete). This preserves the natural-key PK while
supporting soft-delete.

### 4.4 Secrets master key

**Headless master-key strategy.** The key-wrapping model depends on the
deployment profile:

- **`local-dev` / `local-dev-yolo` (embedded PG, single user):** The AES-256
  master key is stored *unwrapped* as a 0600-permission file at
  `$REBORN_HOME/.secrets-master-key` — equivalent trust to the current
  `.reborn-local-dev-secrets-master-key` file. No passphrase required.
  A loud warning is printed at startup reminding operators not to use this
  mode for multi-user or internet-facing deployments.

- **`production` (systemd service, `BRASSCLAW_PG_URL` or embedded PG):**
  Production headless boot requires a working unwrap path at **every start**,
  not only at the one-time `rewrap` run. The supported strategies are:

  | `--strategy` value | Wrap-time key source | Per-boot unwrap mechanism | Suitable for headless systemd? |
  |---|---|---|---|
  | `passphrase` | Interactive terminal prompt | **Reads `BRASSCLAW_SECRETS_PASSPHRASE_FILE` at every boot** — operator must save the interactive passphrase to this file | Yes, if the operator also sets `BRASSCLAW_SECRETS_PASSPHRASE_FILE` |
  | `passphrase-file=<path>` | Reads passphrase from specified file at wrap time | Same file re-read at every boot — fully unattended | **Yes — recommended for production** |
  | `keychain` | OS keyring (macOS Keychain / GNOME Keyring) | Requires unlocked keyring at boot | **No** — no D-Bus session under `User=brassclaw` headless systemd |

  > **M2 — `passphrase` vs `passphrase-file` clarification:** `--strategy passphrase`
  > prompts interactively at wrap time. For unattended reboots, the operator must
  > also save that same passphrase into the `BRASSCLAW_SECRETS_PASSPHRASE_FILE`
  > file manually. `--strategy passphrase-file=<path>` is cleaner for production:
  > it reads and saves the passphrase from a file in one step. In practice,
  > **always use `passphrase-file` for production**; `passphrase` is for
  > ad-hoc interactive decryption sessions or local dev.

  **`BRASSCLAW_SECRETS_PASSPHRASE_FILE`** (bootstrap tier, ceremony-dependent)
  is the required input for passphrase-wrapped unattended boot. It is a path to
  a file containing the Argon2id passphrase — **must be readable by the service
  user** (`brassclaw`), not by root only; see §7 for ownership requirements.
  This var is **not deferred to a follow-up**: it is required in §1c (bootstrap
  tier, ceremony-dependent) and in the §7 unit when using passphrase-wrapped
  ceremony.

  **Ceremony selector:** The ceremony in effect is determined by the
  `BRASSCLAW_SECRETS_PASSPHRASE_FILE` env var (present and non-empty = passphrase-
  wrapped; absent or empty string = raw-key-on-disk). An empty string is treated
  as absent — an empty path is not a valid file path and is ignored. The
  `brassclaw_secrets_master.algorithm` column is the **source of truth** at boot
  time; the passphrase-file env var is the **required input** for the passphrase
  ceremony. The boot path checks algorithm-vs-env consistency:

  - **`algorithm = 'aes256gcm-argon2id'`** → require `BRASSCLAW_SECRETS_PASSPHRASE_FILE`
    (present and non-empty). If absent: fail with `"Master key is passphrase-wrapped
    but BRASSCLAW_SECRETS_PASSPHRASE_FILE is not set. Set the env var or run
    'brassclaw secrets rewrap --strategy raw-key' to revert."` If set: read passphrase,
    unwrap key, proceed.

  - **`algorithm = 'raw-key-on-disk'`** → read key from
    `$REBORN_HOME/.secrets-master-key`. If `BRASSCLAW_SECRETS_PASSPHRASE_FILE` is
    also set (present and non-empty): warn `"BRASSCLAW_SECRETS_PASSPHRASE_FILE is set
    but master key is not wrapped. The file will be ignored. Run 'brassclaw secrets
    rewrap --strategy passphrase-file=<path>' to switch to passphrase ceremony."` and
    proceed with raw-key. (Warn, not fail — the DB row is the source of truth; a stale
    env var should not block boot.)

  **Ordering invariant:** This ceremony-consistency check runs after the schema runner
  (Phase 1) has created `brassclaw_secrets_master` AND after either the boot migration
  (§8.1 step 6) or the first-run wizard (§6.1) has populated the row. On a completely
  fresh install where neither has run yet (`boot.initialized` absent), the ceremony
  check is **skipped** — the first-run wizard will set up the ceremony. The check only
  fires once a `brassclaw_secrets_master` row exists for the boot tenant.

  The one-time setup procedure is documented in §7 (fresh install and upgrade
  sequences), which specifies the correct ownership for each file. In brief:
  `rewrap` must run as the service user (`sudo -u brassclaw`) so that
  `master.key` ends up owned `brassclaw:brassclaw 0600` and is readable at
  per-boot unwrap time.

  The service **refuses to boot** if `brassclaw_secrets_master` has no row for
  the tenant **and** no raw key file exists. The raw key file is zeroed and
  deleted after a successful `rewrap`.

```sql
-- V003__secrets.sql
CREATE TABLE IF NOT EXISTS brassclaw_secrets_master (
    tenant_id       TEXT        NOT NULL,
    version         INT         NOT NULL DEFAULT 1,  -- bumped on rotation
    -- AES-256-GCM key wrapped per the strategy above.
    -- raw-key-on-disk ceremony (passphrase-file absent): wrapped_key = '' AND algorithm = 'raw-key-on-disk'
    --   (key lives at $REBORN_HOME/.secrets-master-key, never in the DB).
    --   The unwrap branch MUST check algorithm = 'raw-key-on-disk' first and
    --   read the key file, NOT attempt to decrypt an empty ciphertext.
    -- passphrase-wrapped ceremony (passphrase-file present): wrapped_key = base64(nonce || ciphertext), algorithm = 'aes256gcm-argon2id'
    wrapped_key     TEXT        NOT NULL DEFAULT '',
    algorithm       TEXT        NOT NULL DEFAULT 'raw-key-on-disk',
    -- Note: DEFAULT is 'raw-key-on-disk'. The production rewrap command writes
    -- 'aes256gcm-argon2id' explicitly. This prevents a newly-inserted local-dev
    -- row from accidentally using the production algorithm sentinel.
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, version)
);
CREATE TRIGGER brassclaw_secrets_master_updated_at
    BEFORE UPDATE ON brassclaw_secrets_master
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_secrets (
    tenant_id   TEXT        NOT NULL,
    scope       TEXT        NOT NULL,   -- e.g. "user:alice" or "operator"
    -- name: identifies the credential — e.g. "google_oauth_refresh_token", NOT "OPENAI_API_KEY".
    -- Operator-sourced secrets (API keys, WebUI token) are env-only and never written here.
    -- This table holds only runtime-obtained credentials: OAuth refresh/access tokens
    -- and credential-broker secrets acquired during auth flows (see FilesystemCredentialBroker
    -- in crates/brassclaw_secrets). A breach of this table alone, without master.key /
    -- the passphrase file, cannot expose these credentials.
    name        TEXT        NOT NULL,
    ciphertext  TEXT        NOT NULL,   -- base64(nonce || encrypted value)
    key_version INT         NOT NULL DEFAULT 1,  -- matches brassclaw_secrets_master.version
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, scope, name)
);
CREATE TRIGGER brassclaw_secrets_updated_at
    BEFORE UPDATE ON brassclaw_secrets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

**`rewrap` vs `rotate` — two distinct operations:**

- **`brassclaw secrets rewrap [--strategy ...] [--tenant <id>]`** — wraps the
  *existing* master key with a new passphrase or strategy. Updates `wrapped_key`
  and `algorithm` on the same `version = 1` row; does **not** generate a new key
  or re-encrypt any `brassclaw_secrets` rows. This is all that §7.1 and §7.2 require.

  **Tenant resolution (data-loss invariant — R6-MH1):** `brassclaw_secrets_master`
  is per-tenant (PK `(tenant_id, version)`). The tenant under which `rewrap`
  writes its row **must** match the `boot_tenant` that §8.1 step 6 will check,
  otherwise step 6 finds no row and aborts — and since `rewrap` already zeroed
  the raw key file, the master key is permanently lost. `rewrap` resolves
  `tenant_id` in this priority order:

  1. **`--tenant <id>` CLI flag** (explicit override — highest priority; use
     this in upgrade runbooks to avoid any ambiguity).
  2. **`identity.tenant` from `$REBORN_HOME/config.toml`** — read via
     `RebornConfigFile::load` (the same loader §8.1 step 3 uses). This is the
     upgrade-path case: `brassclaw_config` is empty when `rewrap` runs manually
     before first serve; config.toml is still present and authoritative.
  3. **`brassclaw_config.identity.tenant`** — read from the DB (the post-migration
     case: config.toml has already been renamed to `.migrated` by a prior run).
  4. **`"default"`** — fallback when neither source yields a value.

  The §7.1 and §7.2 runbook commands pass `--tenant` explicitly to make the
  tenant unambiguous regardless of which files exist on disk. See §7.1 and §7.2
  updated commands below.

  **Key-source rule (data-loss invariant):** `rewrap` MUST read an existing raw
  key file if one is present — it must NOT generate a new key when an existing
  key file exists. Generating a new key when persisted secrets exist orphans
  every ciphertext in `brassclaw_secrets` and `brassclaw_root_filesystem`
  (encrypted under the old key) — those rows become permanently unrecoverable.

  - **Filename search order:** check `$REBORN_HOME/.reborn-local-dev-secrets-master-key`
    first (the pre-migration filename), then `$REBORN_HOME/.secrets-master-key`
    (the post-migration filename). Use whichever is found.
  - **Fresh install (neither file exists and DB is empty):** generate a new key.
  - **Fail-closed:** if neither raw key file is found but `brassclaw_secrets` or
    `brassclaw_root_filesystem` rows are present (would-be orphaned ciphertext),
    `rewrap` must abort with:
    `"raw key file not found but encrypted rows exist — cannot generate new key; restore the original key file first"`.

  **Re-wrapping an already-wrapped key (passphrase change):** when no raw key
  file is present but `brassclaw_secrets_master` already has a row with
  `algorithm = 'aes256gcm-argon2id'` (i.e. the key has already been wrapped
  from a previous `rewrap`), `rewrap` must:
  1. Read `--old-passphrase-file=<path>` if supplied (shell invocation — see note
     below), else `BRASSCLAW_SECRETS_PASSPHRASE_FILE`, else
     `$CREDENTIALS_DIRECTORY/secrets-passphrase` to obtain the *current* (old) passphrase.
  2. Unwrap the stored `wrapped_key` with the old passphrase to recover the
     plaintext AES-256 master key.
  3. Re-wrap with the new `--strategy` and update the `brassclaw_secrets_master`
     row.
  The operator must keep the old passphrase file accessible until `rewrap`
  completes; `rewrap` fails closed if no old passphrase source is available.

  **`--old-passphrase-file=<path>` flag (R6-L1 — shell passphrase-change):**
  `BRASSCLAW_SECRETS_PASSPHRASE_FILE` and `$CREDENTIALS_DIRECTORY` are
  systemd-injected and absent in an interactive shell. When a passphrase change
  is performed interactively (e.g. `sudo -u brassclaw brassclaw secrets rewrap
  --strategy passphrase-file=<new-path>`), the operator must supply the *current*
  passphrase via `--old-passphrase-file=<path>`:
  ```bash
  sudo -u brassclaw brassclaw secrets rewrap \
      --tenant default \
      --strategy passphrase-file=/var/lib/brassclaw/master-new.key \
      --old-passphrase-file=/var/lib/brassclaw/master.key
  ```
  The env-var fallback remains valid for unattended systemd use. Document the
  `--old-passphrase-file` flag and its fallback chain in the §7 passphrase-rotation
  runbook (Phase 9 operator guide).

- **`brassclaw secrets rotate`** (or `rewrap --rotate`) — generates a *new*
  AES-256 master key, inserts a new `version` row into `brassclaw_secrets_master`,
  and re-encrypts all existing `brassclaw_secrets` rows in batches
  (`WHERE key_version < current_version`). `key_version` is what makes this
  incremental and crash-safe. This operation is separate from `rewrap` and
  is not required during upgrade or installation.

  **Old version retirement:** the old `brassclaw_secrets_master` version row
  is deleted **only** after a verification pass confirms no `brassclaw_secrets`
  row has `key_version < new_version`. Deleting it early would make not-yet-
  re-encrypted rows unreadable mid-rotation.

The `§7` sequences only need `rewrap`. Key rotation is a periodic operational
task with its own runbook.

**Legacy raw-key file ↔ `boot_tenant` association (R6-L3):** The legacy raw-key
files (`.reborn-local-dev-secrets-master-key` / `.secrets-master-key`) are single
files with no per-tenant structure — they implicitly belong to `boot_tenant` (the
tenant under which §8.1 step 7 migrates the single-tenant libSQL data). This is
why `rewrap` and §8.1 step 6 must agree on `boot_tenant` (see tenant resolution
note above). Non-`boot_tenant` tenants — created post-migration in a multi-tenant
deployment — have no legacy raw-key file. Their master key is generated fresh on
their first `rewrap` (fresh-install path: neither file exists and DB is empty for
that tenant).

Replaces: `.reborn-local-dev-secrets-master-key` file.

### 4.5 Run state

```sql
-- V004__runs.sql
CREATE TABLE IF NOT EXISTS brassclaw_runs (
    id          TEXT        NOT NULL PRIMARY KEY,   -- ULID
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT,
    project_id  TEXT,
    -- thread_id is NOT a direct field on RunRecord; it is sourced from
    -- record.scope.thread_id (ResourceScope) at write time.
    -- PgRunStateStore::create must extract: record.scope.thread_id.map(|t| t.to_string())
    thread_id   TEXT,
    -- status values are the snake_case serde representations of RunStatus
    -- (brassclaw_run_state::RunStatus, #[serde(rename_all = "snake_case")]):
    --   Running→'running', BlockedApproval→'blocked_approval',
    --   BlockedAuth→'blocked_auth', Completed→'completed', Failed→'failed'.
    status      TEXT        NOT NULL
        CHECK (status IN ('running','blocked_approval','blocked_auth','completed','failed')),
    payload     JSONB       NOT NULL DEFAULT '{}',
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_runs_tenant_status_idx
    ON brassclaw_runs (tenant_id, status) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS brassclaw_runs_thread_idx
    ON brassclaw_runs (tenant_id, thread_id) WHERE thread_id IS NOT NULL;
CREATE TRIGGER brassclaw_runs_updated_at
    BEFORE UPDATE ON brassclaw_runs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemRunStateStore` / `/runs/*` virtual path.

### 4.6 Approvals

> **Crate note:** `FilesystemApprovalRequestStore` lives in `brassclaw_run_state`;
> a dedicated `brassclaw_approvals` crate also exists. The Pg implementation
> (`PgApprovalRequestStore`) will live in `brassclaw_approvals` (its natural home),
> with `brassclaw_run_state` delegating to it.

```sql
-- V005__approvals.sql
CREATE TABLE IF NOT EXISTS brassclaw_approvals (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- ON DELETE RESTRICT: a run must not be hard-deleted while child records
    -- exist. This constraint applies to brassclaw_approvals and brassclaw_turns
    -- (both carry run_id FKs to brassclaw_runs(id) ON DELETE RESTRICT).
    -- brassclaw_checkpoints uses a soft run_id reference (no FK) because
    -- checkpoints may outlive turn rows during retention sweeps (§4.13, §4.21).
    -- Soft-delete (deleted_at) the run instead. If hard-delete is ever
    -- added to PgRunStateStore, it must first DELETE or settle all approval and
    -- turn rows for that run_id — not just approval rows.
    run_id      TEXT        NOT NULL REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    -- NOTE: there is no 'kind' column. ApprovalRecord (brassclaw_run_state) carries
    -- request: ApprovalRequest, where the action is a typed Box<Action> enum
    -- (brassclaw_host_api::Action — ReadFile, WriteFile, Network, Dispatch, etc.).
    -- The action kind is not a fixed-arity closed set suited for a CHECK constraint;
    -- it is stored as part of the JSONB 'request' payload below and can be extracted
    -- by the app layer as needed. Adding a derived 'kind' column would require a
    -- parallel Action→string mapping maintained separately from the enum and would
    -- break whenever a new Action variant is added. Do not add a CHECK-constrained
    -- 'kind' column here.
    --
    -- status values are the snake_case serde representations of ApprovalStatus
    -- (brassclaw_run_state::ApprovalStatus, #[serde(rename_all = "snake_case")]):
    --   Pending→'pending', Approved→'approved', Denied→'denied', Expired→'expired'.
    status      TEXT        NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','denied','expired')),
    request     JSONB       NOT NULL,   -- serialised ApprovalRecord (scope + ApprovalRequest)
    response    JSONB,
    expires_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_approvals_run_idx
    ON brassclaw_approvals (run_id);
CREATE INDEX IF NOT EXISTS brassclaw_approvals_pending_idx
    ON brassclaw_approvals (tenant_id, status) WHERE status = 'pending';
CREATE TRIGGER brassclaw_approvals_updated_at
    BEFORE UPDATE ON brassclaw_approvals
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemApprovalRequestStore` / `/approvals/*`.

### 4.7 Turns

```sql
-- V006__turns.sql
--
-- Key naming: `id` stores the TurnRunId (the run's unique ULID), NOT a
-- surrogate auto-increment. TurnRunRecord.run_id is the natural primary key
-- for this table; the column is named `id` for consistency with all other
-- plan tables. `run_id` is therefore a synonym for `id` at the app layer —
-- PgTurnStateStore must write the run_id value into the `id` column.
-- The FK `run_id TEXT REFERENCES brassclaw_runs(id)` references the
-- capability-invocation run in brassclaw_runs; these are the SAME ULID
-- namespace in this codebase (TurnRunId ≅ InvocationId for the purposes
-- of the FK). **Implementation invariant:** `PgTurnStateStore::create` must
-- write the same ULID value into both `id` (= TurnRunId) and `run_id`
-- (= the corresponding brassclaw_runs(id) row created in the same atomic
-- write). If TurnRunId and InvocationId are ever decoupled (separate ID
-- spaces), this FK must be dropped and replaced with a soft reference. Verify
-- at implementation time that `TurnRunRecord.run_id` resolves to an existing
-- `brassclaw_runs.id` row before inserting.
--
-- NOTE: TurnRunRecord (store.rs:164-194) has NO `sequence` field. The
-- earlier plan drafts fabricated a `sequence INT` column that does not exist
-- in the struct. That column is removed here. The unique constraint on
-- (run_id, sequence) is also removed — `id` (= run_id) is already the PK.
CREATE TABLE IF NOT EXISTS brassclaw_turns (
    -- id: the TurnRunId ULID. This IS the run_id for this table.
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- ON DELETE RESTRICT: see §4.6 comment; hard-delete of a run must settle
    -- all turn rows first.
    run_id      TEXT        NOT NULL REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    -- turn_id: TurnRunRecord.turn_id (TurnId). A turn can have multiple runs
    -- (retries/subagent spawns share the same TurnId); this column lets the
    -- app layer query "all runs for turn X" without scanning the payload JSONB.
    turn_id     TEXT        NOT NULL,
    -- status values are the snake_case conversions of TurnStatus variant names
    -- (brassclaw_turns::TurnStatus, status.rs:14-26). NOTE: TurnStatus has
    -- NO #[serde(rename_all = "snake_case")] — the serde representation is
    -- PascalCase ("Queued", "Running", etc.). The DB column stores snake_case
    -- values derived by converting PascalCase to snake_case (e.g.
    -- BlockedApproval→'blocked_approval', RecoveryRequired→'recovery_required').
    -- IMPORTANT: the adapter must use a proper PascalCase→snake_case converter,
    -- NOT a plain .to_lowercase() call. to_lowercase() on "RecoveryRequired" yields
    -- "recoveryrequired" (no underscore), not "recovery_required". Use a crate
    -- such as `heck::ToSnakeCase` or an equivalent. The round-trip is:
    -- write: snake_case(variant_name); read: parse snake_case back to enum.
    -- Terminal values (is_terminal() == true): cancelled, completed, failed,
    -- recovery_required. All others are non-terminal / in-flight.
    status      TEXT        NOT NULL
        CHECK (status IN (
            'queued',
            'running',
            'blocked_approval',
            'blocked_auth',
            'blocked_resource',
            'blocked_dependent_run',
            'cancel_requested',
            'cancelled',
            'completed',
            'failed',
            'recovery_required'
        )),
    payload     JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- turn_id index: needed for "all runs for turn X" queries (a turn can have
-- multiple runs via retries or subagent spawns sharing the same TurnId).
CREATE INDEX IF NOT EXISTS brassclaw_turns_turn_idx
    ON brassclaw_turns (tenant_id, turn_id);
-- tenant_id index: needed for retention sweeps and any query that lists all
-- turns for a tenant (e.g. "delete all turns for a tenant on offboarding").
-- The MountAlias tenant isolation from FilesystemTurnStateStore was structural
-- (filesystem scoping); the PG store must enforce this via the index and WHERE
-- clauses on tenant_id.
CREATE INDEX IF NOT EXISTS brassclaw_turns_tenant_idx
    ON brassclaw_turns (tenant_id, run_id);
CREATE TRIGGER brassclaw_turns_updated_at
    BEFORE UPDATE ON brassclaw_turns
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemTurnStateStore` / `/turns/*`.

### 4.8 Capability leases

```sql
-- V007__capability_leases.sql
CREATE TABLE IF NOT EXISTS brassclaw_capability_leases (
    id              TEXT        NOT NULL PRIMARY KEY,   -- CapabilityGrantId
    tenant_id       TEXT        NOT NULL,               -- scope.tenant_id
    user_id         TEXT        NOT NULL,               -- scope.user_id
    capability_id   TEXT        NOT NULL,               -- grant.capability
    -- status maps to CapabilityLeaseStatus
    -- (brassclaw_authorization::CapabilityLeaseStatus, lib.rs:188-193).
    -- The enum has NO #[serde(rename_all)] — the app layer must lowercase the
    -- variant name for this column: Active→'active', Claimed→'claimed',
    -- Consumed→'consumed', Revoked→'revoked'. This column is the primary
    -- lifecycle indicator; the partial index below uses it to find active leases.
    status          TEXT        NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','claimed','consumed','revoked')),
    -- grant: full serialised CapabilityGrant (id, capability, grantee,
    -- issued_by, constraints). The typed columns above are extracted from
    -- scope + grant for indexing; the full grant is preserved here.
    grant           JSONB       NOT NULL,
    -- invocation_fingerprint: Option<InvocationFingerprint> from CapabilityLease.
    -- Stored as TEXT (the fingerprint's string representation) for indexability;
    -- NULL when absent. Used for replay-attack prevention on lease claims.
    invocation_fingerprint TEXT,
    -- expires_at / revoked_at: lifecycle timestamps. expires_at is derived
    -- from GrantConstraints at issue time (when a time-bound constraint is
    -- present). revoked_at is set when status transitions to 'revoked' via
    -- CapabilityLeaseStore::revoke(). Both are informational; the status
    -- column is the authoritative lifecycle indicator.
    expires_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- NOTE: The partial index predicate uses only `status = 'active'`. A previously
-- drafted predicate also included `(expires_at IS NULL OR expires_at > now())`
-- but that is WRONG for a partial index: the expression `now()` is evaluated
-- once at index creation time, not at query time — PG partial indexes are static
-- filters. Including `expires_at > now()` would create an index that permanently
-- excludes rows that were not-yet-expired at index-creation time, silently
-- missing actually-expired-at-creation-time rows in subsequent scans. The
-- expiry check must be performed at the application layer (WHERE clause in
-- queries issued by PgCapabilityLeaseStore), not in the partial index predicate.
CREATE INDEX IF NOT EXISTS brassclaw_capability_leases_user_cap_idx
    ON brassclaw_capability_leases (tenant_id, user_id, capability_id)
    WHERE status = 'active';
CREATE TRIGGER brassclaw_capability_leases_updated_at
    BEFORE UPDATE ON brassclaw_capability_leases
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemCapabilityLeaseStore` / `/capabilities/*`.

### 4.9 Session threads

```sql
-- V008__session_threads.sql
CREATE TABLE IF NOT EXISTS brassclaw_session_threads (
    id          TEXT        NOT NULL PRIMARY KEY,   -- ThreadId
    tenant_id   TEXT        NOT NULL,               -- ThreadScope.tenant_id
    -- user_id: ThreadScope.owner_user_id (Option<UserId>). The app layer
    -- resolves None to SYSTEM_RESERVED_ID before writing (per
    -- ThreadScope::to_resource_scope fallback), so the column is NOT NULL.
    -- System-scoped threads store SYSTEM_RESERVED_ID here.
    user_id     TEXT        NOT NULL,
    -- agent_id: ThreadScope.agent_id (non-optional AgentId). NOT NULL —
    -- ThreadScope.agent_id is NOT Option<AgentId>; every thread has an agent.
    agent_id    TEXT        NOT NULL,
    -- project_id / mission_id: optional ThreadScope fields extracted to typed
    -- columns for tenant-scoped queries (e.g. "list all threads for project X").
    project_id  TEXT,
    mission_id  TEXT,
    -- created_by_actor_id: SessionThreadRecord.created_by_actor_id (String).
    -- Identifies the actor (user or system) that created the thread.
    created_by_actor_id TEXT NOT NULL,
    -- title: SessionThreadRecord.title (Option<String>). Extracted to a typed
    -- column for WHERE title LIKE '%...%' queries and UI list views.
    title       TEXT,
    -- metadata: SessionThreadRecord.metadata_json (Option<String>) parsed into
    -- JSONB. Also holds SessionThreadRecord.goal (Option<ThreadGoal>) — the
    -- goal is a structured object not queried by SQL, so it stays in the JSONB
    -- payload rather than getting its own column.
    metadata    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_session_threads_user_idx
    ON brassclaw_session_threads (tenant_id, user_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS brassclaw_session_threads_agent_idx
    ON brassclaw_session_threads (tenant_id, agent_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS brassclaw_session_threads_project_idx
    ON brassclaw_session_threads (tenant_id, project_id)
    WHERE deleted_at IS NULL AND project_id IS NOT NULL;
CREATE TRIGGER brassclaw_session_threads_updated_at
    BEFORE UPDATE ON brassclaw_session_threads
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemSessionThreadService` / `/sessions/*`.

### 4.10 Processes and results

```sql
-- V009__processes.sql
CREATE TABLE IF NOT EXISTS brassclaw_processes (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    -- runtime maps to RuntimeKind (brassclaw_host_api::runtime::RuntimeKind,
    -- #[serde(rename_all = "snake_case")]): Mcp→'mcp', FirstParty→'first_party',
    -- System→'system'. NOTE: Wasm and Script lanes were removed in Phase 4 of the
    -- v1-removal plan — capabilities that historically declared 'wasm' or 'script'
    -- now dispatch via the Mcp lane. FirstParty and System have
    -- #[serde(skip_deserializing)] (untrusted manifests cannot self-assert them)
    -- but CAN be serialized to the DB from a host-trusted source, so all three
    -- values are valid in this column. The column is named 'runtime' to match
    -- ProcessRecord.runtime: RuntimeKind (types.rs:44), NOT 'kind'.
    runtime     TEXT        NOT NULL
        CHECK (runtime IN ('mcp','first_party','system')),
    -- status maps to ProcessStatus (brassclaw_processes::types::ProcessStatus,
    -- #[serde(rename_all = "snake_case")]): Running→'running', Completed→'completed',
    -- Failed→'failed', Killed→'killed'. NOTE: there is no 'pending' (processes start
    -- in Running state) and no 'cancelled' (the terminal state for externally
    -- terminated processes is 'killed', not 'cancelled').
    status      TEXT        NOT NULL
        CHECK (status IN ('running','completed','failed','killed')),
    -- spec: serialised ProcessRecord payload (grants: CapabilitySet, mounts: MountView,
    -- estimated_resources: ResourceEstimate, resource_reservation_id, error_kind).
    -- ProcessRecord has no direct 'spec' field; this JSONB holds the fields not
    -- extracted to typed columns (id, tenant_id, run_id, runtime, status are columns;
    -- everything else goes here). ProcessStart.input is also stored here on insert.
    spec        JSONB       NOT NULL,
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Tenant-scoped status lookup: "all running processes for tenant X".
-- Without this index, a status-filtered query over a busy tenant requires a full table scan.
CREATE INDEX IF NOT EXISTS brassclaw_processes_tenant_status_idx
    ON brassclaw_processes (tenant_id, status);
CREATE TRIGGER brassclaw_processes_updated_at
    BEFORE UPDATE ON brassclaw_processes
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- insert-only; results are never modified after write
CREATE TABLE IF NOT EXISTS brassclaw_process_results (
    process_id  TEXT        NOT NULL PRIMARY KEY REFERENCES brassclaw_processes(id) ON DELETE RESTRICT,
    tenant_id   TEXT        NOT NULL,   -- denormalised from ResourceScope for tenant-scoped queries
    -- status maps to ProcessStatus (same enum as brassclaw_processes.status,
    -- #[serde(rename_all = "snake_case")]): 'running'/'completed'/'failed'/'killed'.
    -- Set by complete()→Completed or fail()→Failed.
    status      TEXT        NOT NULL
        CHECK (status IN ('running','completed','failed','killed')),
    -- output: successful process output (Option<Value> → JSONB). Set by complete();
    -- NULL when fail() stores a failure record.
    output      JSONB,
    -- output_ref: optional virtual path reference to large output stored in the VFS
    -- (Option<VirtualPath> → TEXT). Allows redirecting large outputs to a file path
    -- instead of inline JSONB.
    output_ref  TEXT,
    -- error_kind: classified failure category (Option<String> → TEXT). Set by fail();
    -- NULL when complete() stores a success record. This is a classified error
    -- kind, NOT raw stderr — the store deliberately does not persist raw backend
    -- detail strings (per ProcessResultStore::fail doc comment).
    error_kind  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
    -- no updated_at: this table is insert-only
);
```

> **M5 — `tenant_id` integrity:** `brassclaw_process_results.tenant_id` is
> denormalised for performance. No FK or CHECK can enforce it matches the parent
> `brassclaw_processes.tenant_id` without a trigger or deferred constraint.
> **App-layer invariant:** `PgProcessResultStore::insert` must always copy
> `tenant_id` directly from the parent `brassclaw_processes` row (read in the
> same transaction), never from a caller-supplied value. This invariant is
> enforced by the store's write path and covered by an integration test that
> inserts a process + result and asserts both rows carry identical `tenant_id`.

Replaces: `FilesystemProcessStore` / `FilesystemProcessResultStore`.

### 4.11 Extensions

> **Crate note:** `FilesystemExtensionInstallationStore` is a `pub(crate)` type
> in `brassclaw_reborn_composition` (not in `brassclaw_extensions`). The Pg
> implementation (`PgExtensionInstallationStore`) will be added to
> `brassclaw_extensions` (where the trait and `InMemoryExtensionInstallationStore`
> already live), making it a proper crate-public implementation.

```sql
-- V010__extensions.sql
CREATE TABLE IF NOT EXISTS brassclaw_extension_manifests (
    tenant_id   TEXT        NOT NULL,
    name        TEXT        NOT NULL,
    version     TEXT        NOT NULL,
    manifest    JSONB       NOT NULL,   -- parsed from TOML at registration time
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, name, version)
);
CREATE TRIGGER brassclaw_extension_manifests_updated_at
    BEFORE UPDATE ON brassclaw_extension_manifests
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS brassclaw_extensions (
    id           TEXT        NOT NULL PRIMARY KEY,
    tenant_id    TEXT        NOT NULL,
    user_id      TEXT        NOT NULL,
    name         TEXT        NOT NULL,
    version      TEXT        NOT NULL,
    -- activation_state maps to ExtensionActivationState enum variants
    -- (brassclaw_extensions::ExtensionActivationState, #[serde(rename_all = "snake_case")]):
    -- Installed→'installed', Disabled→'disabled', Enabled→'enabled'.
    -- Note: there is no 'removed' or 'active' variant in ExtensionActivationState.
    -- Soft-deletion is modelled by removed_at below (not by a 'removed' status value).
    -- The activation_state column retains its last non-removed value even when the
    -- row is soft-deleted; queries for active extensions must additionally filter
    -- WHERE removed_at IS NULL.
    activation_state TEXT     NOT NULL DEFAULT 'installed'
        CHECK (activation_state IN ('installed','disabled','enabled')),
    config       JSONB       NOT NULL DEFAULT '{}',
    -- created_at replaces "installed_at" from the draft. §4.1 design philosophy:
    -- "created_at and updated_at on every mutable table." The installation
    -- timestamp IS the created_at for this table. Using a domain-specific name
    -- would violate the consistency rule and make the updated_at trigger naming
    -- inconsistent. Queries that need "installed_at" semantics select created_at.
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- removed_at: soft-delete marker. The current ExtensionInstallationStore has a
    -- delete_installation() hard-delete path; PgExtensionInstallationStore upgrades
    -- this to soft-delete for audit retention. PgExtensionInstallationStore must
    -- set removed_at = now() instead of issuing DELETE.
    removed_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS brassclaw_extensions_user_idx
    ON brassclaw_extensions (tenant_id, user_id) WHERE removed_at IS NULL;
CREATE TRIGGER brassclaw_extensions_updated_at
    BEFORE UPDATE ON brassclaw_extensions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemExtensionInstallationStore` + extension manifest TOML files.

### 4.12 Resource accounts (budget / governor)

> **Optimistic locking (CAS):** The existing `FilesystemResourceGovernorStore`
> uses compare-and-swap (`CasSnapshotStore`) — it does NOT use `SELECT FOR UPDATE`.
> The Postgres implementation (`PgResourceGovernorStore`) must replicate this
> behaviour to avoid blocking concurrent budget checks. The `version` column
> enables this: on every budget update, the store issues a conditional UPDATE:
> ```sql
> UPDATE brassclaw_resource_accounts
>    SET reserved = $new_reserved, consumed = $new_consumed,
>        version = version + 1, updated_at = now()
>  WHERE id = $id AND version = $expected_version;
> ```
> If 0 rows are affected (version mismatch → concurrent update), the store
> returns a `BudgetConflict` error and the caller retries (identical to the
> CAS retry loop in `CasSnapshotStore`). The `version` column starts at 0 and
> is incremented on every UPDATE. The `updated_at` trigger still fires (it
> is compatible with this pattern). The `SELECT FOR UPDATE` pattern is
> intentionally **not** used here — it would serialize concurrent budget checks
> and degrade throughput under concurrent agent runs.
>
> **First-write path (R7-M2 fix — no existing row for this period):** The first
> reservation for a `(tenant_id, scope_kind, scope_id, period_key)` tuple has no
> row to read-then-CAS-update. **`INSERT … ON CONFLICT DO UPDATE` must NOT be
> used here** — `excluded.reserved` / `excluded.consumed` are absolute values
> computed by the writer assuming the row was absent. On a conflict, `DO UPDATE SET
> reserved = excluded.reserved` would overwrite the concurrent first-writer's
> reservation with the second writer's stale pre-computed absolute, silently
> losing the first writer's value. This is last-writer-wins, not CAS, and it
> deviates from `CasSnapshotStore` semantics (where the second writer's CAS fails
> and retries with a re-read).
>
> The correct two-step pattern:
> 1. **Ensure the row exists** with a no-op insert:
>    ```sql
>    INSERT INTO brassclaw_resource_accounts
>        (id, tenant_id, scope_kind, scope_id, period_key, reserved, consumed, version)
>    VALUES
>        ($id, $tenant_id, $scope_kind, $scope_id, $period_key, 0, 0, 0)
>    ON CONFLICT (tenant_id, scope_kind, scope_id, period_key) DO NOTHING;
>    ```
>    Whether this INSERT lands or conflicts (concurrent first-writer won), a row
>    now exists with some `version`.
> 2. **Read back** the row: `SELECT reserved, consumed, version … FOR UPDATE` is
>    unnecessary — just read and proceed with the CAS UPDATE:
>    ```sql
>    UPDATE brassclaw_resource_accounts
>       SET reserved = $new_reserved,   -- current_reserved + delta
>           consumed = $new_consumed,
>           version  = version + 1, updated_at = now()
>     WHERE (tenant_id, scope_kind, scope_id, period_key) =
>           ($tenant_id, $scope_kind, $scope_id, $period_key)
>       AND version = $expected_version;
>    ```
>    If 0 rows affected (concurrent writer changed `version` between the read and
>    this UPDATE), return `BudgetConflict` and retry from step 2. This preserves
>    full CAS semantics: both first-writers start from `reserved = 0`, both compute
>    their delta correctly, the second writer's CAS sees `version = 1` (set by the
>    first), retries, reads `reserved = first_delta`, computes
>    `new_reserved = first_delta + second_delta`, and succeeds on the next attempt.

```sql
-- V011__resources.sql
CREATE TABLE IF NOT EXISTS brassclaw_resource_accounts (
    id          TEXT        NOT NULL PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- scope_kind maps to the variant of ResourceAccount
    -- (brassclaw_resources::ResourceAccount, lib.rs:120-152). The enum has 6
    -- variants: Tenant, User, Project, Agent, Mission, Thread. ResourceAccount
    -- has NO #[serde(rename_all)] — the app layer must lowercase the variant
    -- name for this column (Tenant→'tenant', User→'user', etc.). The scope_id
    -- column holds the variant-specific identifier (user_id, project_id,
    -- agent_id, mission_id, or thread_id; for Tenant scope, scope_id = tenant_id).
    scope_kind  TEXT        NOT NULL
        CHECK (scope_kind IN ('tenant','user','project','agent','mission','thread')),
    scope_id    TEXT        NOT NULL,
    period_key  TEXT        NOT NULL,  -- e.g. "2025-01-15" (daily) or "2025-01-W3"
    reserved    NUMERIC(18,6) NOT NULL DEFAULT 0,
    consumed    NUMERIC(18,6) NOT NULL DEFAULT 0,
    limit_usd   NUMERIC(18,6),
    -- version: optimistic locking counter for CAS updates (mirrors CasSnapshotStore
    -- behaviour in FilesystemResourceGovernorStore). Starts at 0; incremented by
    -- every conditional UPDATE. Never reset. See the note above for the UPDATE pattern.
    version     BIGINT      NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, scope_kind, scope_id, period_key)
);
CREATE TRIGGER brassclaw_resource_accounts_updated_at
    BEFORE UPDATE ON brassclaw_resource_accounts
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemResourceGovernorStore` / `/resources/*`.

### 4.13 Checkpoints

> **Crate note:** `FilesystemCheckpointStateStore` lives in `brassclaw_loop_support`
> (not `brassclaw_turns`). Both `InMemoryCheckpointStateStore` and
> `InMemoryLoopCheckpointStore` exist in `brassclaw_turns::checkpoint_state`; the
> latter is not fictitious. The Pg implementations belong in `brassclaw_loop_support`.

> **Schema note:** This table stores `CheckpointStateRecord`
> (`brassclaw_turns::checkpoint_state`, checkpoint_state.rs:55-65), which is the
> full checkpoint state including the redacted payload bytes. The
> `FilesystemCheckpointStateStore` serialises a `StoredCheckpointStateRecord`
> (with `payload_hex: String`) as JSON; the Pg table stores the payload as
> `BYTEA` directly (avoiding the hex-encode/decode round-trip) and extracts the
> metadata fields to typed columns for indexing. The loop checkpoint *metadata*
> (`LoopCheckpointRecord` in checkpoint_state.rs:164-179) is stored separately
> in `brassclaw_turns.payload` JSONB as part of the `TurnPersistenceSnapshot` —
> it references this table via `state_ref`.

```sql
-- V012__checkpoints.sql
-- Retention: keep the last 10 checkpoints per run (enforced by app-layer
-- cleanup after run completion) plus a 30-day TTL background sweep. See §4.21.
-- Note: pg_cron is NOT used — retention runs inside the serve process only (§4.21).
CREATE TABLE IF NOT EXISTS brassclaw_checkpoints (
    -- Synthetic primary key. CheckpointStateRecord has no explicit id field;
    -- the natural key is (tenant_id, run_id, state_ref, schema_id, schema_version).
    -- A BIGSERIAL surrogate keeps the PK narrow and the secondary index small.
    id          BIGSERIAL    PRIMARY KEY,
    tenant_id   TEXT         NOT NULL,               -- scope.tenant_id
    -- turn_id: CheckpointStateRecord.turn_id (TurnId). Extracted for scoped
    -- queries ("all checkpoints for turn X").
    turn_id     TEXT         NOT NULL,
    -- run_id: CheckpointStateRecord.run_id (TurnRunId). Soft reference to
    -- brassclaw_turns(id) — NOT a FK to brassclaw_runs(id). TurnRunId is a
    -- turn-run identifier, not a capability-invocation identifier (InvocationId);
    -- the two ID spaces are distinct. A FK to brassclaw_turns(id) is not added
    -- because checkpoints may outlive turn rows during retention sweeps (see
    -- §4.21 — checkpoints have a 30-day TTL while turns are soft-deleted with
    -- the run). The app layer resolves run_id to a turn row if one exists.
    run_id      TEXT         NOT NULL,
    -- state_ref: CheckpointStateRecord.state_ref (LoopCheckpointStateRef — a
    -- validated String wrapper, run_profile/host.rs:248). This is the primary
    -- retrieval key: GetCheckpointStateRequest looks up by state_ref.
    state_ref   TEXT         NOT NULL,
    -- schema_id / schema_version: CheckpointStateRecord.schema_id
    -- (CheckpointSchemaId — a validated String wrapper) and schema_version
    -- (RunProfileVersion — a u64 wrapper, ids.rs:319). Together with state_ref
    -- these form the checkpoint identity tuple.
    schema_id   TEXT         NOT NULL,
    schema_version BIGINT    NOT NULL,
    -- kind: CheckpointStateRecord.kind (LoopCheckpointKind,
    -- run_profile/host.rs:1838-1843). The enum has NO #[serde(rename_all)];
    -- the DB column stores the as_str() values (before_model, before_side_effect,
    -- before_block, final) per the as_str() impl at host.rs:1846-1853.
    kind        TEXT         NOT NULL
        CHECK (kind IN ('before_model','before_side_effect','before_block','final')),
    -- payload: CheckpointStateRecord.payload (RedactedCheckpointPayload — opaque
    -- bytes, max 64 KiB per MAX_CHECKPOINT_STATE_PAYLOAD_BYTES at
    -- checkpoint_state.rs:13). BYTEA is correct: the payload is raw bytes, not
    -- JSON. The filesystem store hex-encodes this into payload_hex; the Pg
    -- store writes the raw bytes directly.
    payload     BYTEA        NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now()
    -- no updated_at: checkpoints are immutable after write
);
-- Natural-key lookup index: GetCheckpointStateRequest retrieves by
-- (scope, turn_id, run_id, state_ref, schema_id, schema_version, kind).
-- The tenant_id prefix ensures tenant isolation in multi-tenant deployments.
CREATE UNIQUE INDEX IF NOT EXISTS brassclaw_checkpoints_natural_key_idx
    ON brassclaw_checkpoints (tenant_id, run_id, state_ref, schema_id, schema_version, kind);
-- Run-scoped lookup: "all checkpoints for run X" (used by retention sweep and
-- loop recovery). kind is included to filter by checkpoint phase without
-- touching the payload.
CREATE INDEX IF NOT EXISTS brassclaw_checkpoints_run_kind_idx
    ON brassclaw_checkpoints (tenant_id, run_id, kind);
-- Retention sweep: tenant-wide by age (30-day TTL, §4.21).
CREATE INDEX IF NOT EXISTS brassclaw_checkpoints_tenant_age_idx
    ON brassclaw_checkpoints (tenant_id, created_at);
```

Replaces: `FilesystemCheckpointStateStore` / `/checkpoint-state/*`.

### 4.14 Events and audit log

> **Retention (H7):** These tables are append-only and grow indefinitely without
> pruning. See §4.21 for the retention/TTL policy. These are operational state,
> not LLM output — CLAUDE.md's "LLM data is never deleted" rule applies to
> conversation history, reasoning, and tool outputs stored in `brassclaw_turns`
> and `brassclaw_checkpoints` payloads, not to the event/audit log rows.

```sql
-- V013__events.sql
-- Retention: 90-day rolling window pruned by a background task (§4.21).
CREATE TABLE IF NOT EXISTS brassclaw_events (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    -- run_id is a soft reference (no FK): events are append-only and may
    -- outlive the run record (runs use soft-delete via deleted_at, not
    -- hard-delete). A FK with ON DELETE RESTRICT would prevent any future
    -- hard-delete of runs with events; a FK with ON DELETE SET NULL would
    -- silently sever the link. The soft reference is intentional — the
    -- app layer resolves run_id to a run record if one exists.
    run_id      TEXT,
    kind        TEXT        NOT NULL,
    payload     JSONB       NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
    -- append-only; no updated_at, no deleted_at
);
CREATE INDEX IF NOT EXISTS brassclaw_events_run_idx
    ON brassclaw_events (run_id) WHERE run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_events_tenant_idx
    ON brassclaw_events (tenant_id, occurred_at DESC);

-- Retention: 1-year rolling window (compliance-grade; operator-configurable).
CREATE TABLE IF NOT EXISTS brassclaw_audit_log (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    actor_id    TEXT,
    action      TEXT        NOT NULL,
    resource    TEXT,
    payload     JSONB,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
    -- append-only; no updated_at, no deleted_at
);
CREATE INDEX IF NOT EXISTS brassclaw_audit_log_tenant_idx
    ON brassclaw_audit_log (tenant_id, occurred_at DESC);
```

Replaces: `DurableEventLog` + `DurableAuditLog` (both VFS-backed).

### 4.15 Token settings

```sql
-- V014__token_settings.sql
CREATE TABLE IF NOT EXISTS brassclaw_token_settings (
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    provider_id TEXT        NOT NULL,
    settings    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id, provider_id)
);
CREATE TRIGGER brassclaw_token_settings_updated_at
    BEFORE UPDATE ON brassclaw_token_settings
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: libSQL `settings` table (`DbTokenSettingsStore`).

### 4.16 Safety config and capability permissions

```sql
-- V015__safety.sql
CREATE TABLE IF NOT EXISTS brassclaw_safety_config (
    id              TEXT        NOT NULL PRIMARY KEY,  -- ULID surrogate key
    tenant_id       TEXT        NOT NULL,
    user_id         TEXT        NOT NULL,
    -- category maps to SafetyCategory (brassclaw_product_workflow::safety_config_store::SafetyCategory)
    -- via as_str(): SensitivePaths→'sensitive_paths', WorkspaceRules→'workspace_rules',
    -- BlockedPaths→'blocked_paths'. Closed 3-variant enum; CHECK enforces DB-level
    -- integrity per §4.1 design philosophy. The existing libSQL schema had no CHECK
    -- on this column; the Pg schema adds one as an improvement.
    category        TEXT        NOT NULL
        CHECK (category IN ('sensitive_paths','workspace_rules','blocked_paths')),
    pattern         TEXT        NOT NULL,
    is_enabled      BOOLEAN     NOT NULL DEFAULT true,
    is_default      BOOLEAN     NOT NULL DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Natural-key uniqueness: mirrors the existing INSERT OR IGNORE semantics
    -- (safety_config_store.rs uses (user_id, category, pattern) as the dedup key).
    UNIQUE (tenant_id, user_id, category, pattern)
);
CREATE TRIGGER brassclaw_safety_config_updated_at
    BEFORE UPDATE ON brassclaw_safety_config
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
-- Upsert pattern in PgSafetyConfigStore:
--   INSERT ... ON CONFLICT (tenant_id, user_id, category, pattern) DO NOTHING

CREATE TABLE IF NOT EXISTS brassclaw_capability_permissions (
    tenant_id       TEXT        NOT NULL,
    capability_id   TEXT        NOT NULL,
    -- permission_mode maps to PermissionMode (brassclaw_host_api::capability::PermissionMode,
    -- #[serde(rename_all = "snake_case")]): Allow→'allow', Ask→'ask', Deny→'deny'.
    -- NOTE: there is NO 'org_policy' variant in PermissionMode — that value belongs
    -- to ApprovalPolicy (a different enum in runtime_policy.rs). The existing libSQL
    -- schema at safety_config_store.rs:102 already had the correct 3-value CHECK.
    permission_mode TEXT        NOT NULL
        CHECK (permission_mode IN ('allow','ask','deny')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, capability_id)
);
CREATE TRIGGER brassclaw_capability_permissions_updated_at
    BEFORE UPDATE ON brassclaw_capability_permissions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: libSQL `safety_config` and `capability_permissions` tables.

### 4.17 Memory docs

> **Scope of this table.** `brassclaw_memory_docs` migrates **only** the libSQL
> `memory_docs` table maintained by `MemoryDocLibSqlStore` (in
> `crates/brassclaw_reborn_composition/src/memory_doc_libsql_store.rs`). This table
> is the durable backing store for the memory **reduction-rule pipeline** (skill
> extraction, recipe creation, structured MemoryDoc records).
>
> It does **not** replace `FilesystemMemoryDocumentRepository` (in
> `crates/brassclaw_memory/src/repo/filesystem.rs`), which stores workspace
> **document chunks** (vectorised/FTS chunks of workspace files and chat-memory
> entries) via the VFS fabric (`ScopedFilesystem<F>`). That repository routes through
> `brassclaw_root_filesystem` (§4.19) on the Postgres path — it is covered by the
> VFS fallback table automatically and requires no separate migration table. The two
> stores serve orthogonal purposes and must both be present at runtime.

```sql
-- V016__memory_docs.sql
CREATE TABLE IF NOT EXISTS brassclaw_memory_docs (
    id               TEXT        NOT NULL,
    tenant_id        TEXT        NOT NULL,
    user_id          TEXT        NOT NULL,
    project_id       TEXT        NOT NULL,
    doc_type         TEXT        NOT NULL,
    title            TEXT        NOT NULL,
    content          TEXT        NOT NULL,
    source_thread_id TEXT,
    tags             TEXT[]      NOT NULL DEFAULT '{}',
    metadata         JSONB       NOT NULL DEFAULT '{}',
    -- Stored generated tsvector column so FTS index stays current on every
    -- INSERT/UPDATE without a separate trigger or manual expression restatement.
    -- PG 12+ (this plan targets PG 16). coalesce guards future nullable columns.
    tsv              tsvector    GENERATED ALWAYS AS (
                         to_tsvector('english',
                             coalesce(title, '') || ' ' || coalesce(content, ''))
                     ) STORED,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id, project_id, id)
);
-- FTS index over the generated column — auto-maintained on every write.
CREATE INDEX IF NOT EXISTS brassclaw_memory_docs_fts_idx
    ON brassclaw_memory_docs USING GIN (tsv);
-- Fast tag lookup
CREATE INDEX IF NOT EXISTS brassclaw_memory_docs_tags_idx
    ON brassclaw_memory_docs USING GIN (tags);
CREATE TRIGGER brassclaw_memory_docs_updated_at
    BEFORE UPDATE ON brassclaw_memory_docs
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: libSQL `memory_docs` table (`MemoryDocLibSqlStore`).
**Improvements over original:** (1) `TEXT[]` tags replaces the JSON-encoded
`tags_json` string; (2) `GENERATED ALWAYS AS ... STORED` tsvector means the GIN
index is auto-maintained on every INSERT/UPDATE — a plain expression GIN index
would have required every FTS query to repeat the identical expression verbatim.

> **Relationship to chat-memory storage (Upgrades A/C):** `brassclaw_memory_docs`
> stores reduction-rule / skill `MemoryDoc` records that are produced by the
> memory reduction pipeline (existing behaviour, unchanged). The new
> `brassclaw_memory_chat_records` table (§4.29, V025) stores raw per-turn
> chat-memory entries written by `memory_write` tool calls and is a separate,
> parallel concern. Do not conflate the two tables — they serve different purposes
> and both must exist.

### 4.18 Hook predicate state (verbatim from source)

> **DDL copied verbatim from `crates/brassclaw_hooks_postgres/migrations/V1__predicate_state.sql`.**
> All 8 indexes are preserved. Column order matches the source (`scope_hash` first,
> which the source comment explains is deliberate: "scope_hash is the trust boundary").
> `IF NOT EXISTS` guards added for refinery idempotency (see §3).

```sql
-- V017__hooks.sql
--
-- Design notes (preserved from V1__predicate_state.sql source):
--
-- scope_hash: BYTEA, not TEXT — raw blake3 digest; TEXT would require an
--   extra encoding/decoding step on every read/write and wastes ~35% space.
--   scope_hash is the trust boundary: all queries must scope to it first to
--   prevent cross-tenant predicate leakage.
--
-- key_hash: same BYTEA rationale as scope_hash.
--
-- event_id: TEXT, NOT uuid — event IDs are blake3 64-char hex digests.
--   A UUID column (128-bit) cannot store a 256-bit hex string; attempting to
--   cast would silently truncate and cause phantom dedup failures. Do NOT
--   change this column to UUID — any future migration that does so will reject
--   all existing 64-char hex event IDs. The column stays TEXT.
--
-- occurred_at: window-clock basis for per-key COUNT and LRU eviction queries.
--   Using TIMESTAMPTZ (not BIGINT epoch) allows age-based sweep queries
--   (WHERE occurred_at < now() - interval 'N days') directly without
--   epoch arithmetic. Retention is enforced by the brassclaw serve sweep
--   and brassclaw maintenance prune-old-data (§4.21) — not pg_cron.
--
CREATE TABLE IF NOT EXISTS hooks_predicate_invocations (
    scope_hash   BYTEA       NOT NULL,
    key_hash     BYTEA       NOT NULL,
    event_id     TEXT        NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (key_hash, event_id)
);
-- Per-key window-trim + COUNT scan
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_key_ts_idx
    ON hooks_predicate_invocations (key_hash, occurred_at);
-- Per-scope (tenant) distinct-key LRU eviction scans
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_scope_idx
    ON hooks_predicate_invocations (scope_hash);
-- Per-tenant LRU quota composite
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_scope_key_idx
    ON hooks_predicate_invocations (scope_hash, key_hash);
-- Operator reaper (evict_older_than) by age
CREATE INDEX IF NOT EXISTS hooks_predicate_invocations_ts_idx
    ON hooks_predicate_invocations (occurred_at);

CREATE TABLE IF NOT EXISTS hooks_predicate_values (
    scope_hash   BYTEA       NOT NULL,
    key_hash     BYTEA       NOT NULL,
    event_id     TEXT        NOT NULL,
    occurred_at  TIMESTAMPTZ NOT NULL,
    value        NUMERIC     NOT NULL,
    PRIMARY KEY (key_hash, event_id)
);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_key_ts_idx
    ON hooks_predicate_values (key_hash, occurred_at);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_scope_idx
    ON hooks_predicate_values (scope_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_scope_key_idx
    ON hooks_predicate_values (scope_hash, key_hash);
CREATE INDEX IF NOT EXISTS hooks_predicate_values_ts_idx
    ON hooks_predicate_values (occurred_at);
```

Note: `hooks_*` tables intentionally keep no `brassclaw_` prefix for backward
compatibility with existing deployments that already have these tables.
These tables are append-only (pruned by `evict_older_than`); no `updated_at`
trigger is needed.

No semantic schema change vs. the source. Migration consolidates it here so
`brassclaw_pg` is the single migration authority.

### 4.19 Root filesystem fallback

> **Multi-tenant isolation gap fixed (C2):** The earlier draft had no `tenant_id`
> on `brassclaw_root_filesystem`. This is dangerous: §4.4 (`rewrap`) queries this
> table for encrypted rows (`WHERE contents IS NOT NULL AND kind = 'encrypted'`) to
> determine whether a new key can be safely generated. Without `tenant_id`, this
> check is global — it would find encrypted rows from other tenants and block a
> per-tenant `rewrap`. Worse, a `PostgresRootFilesystem` implementation scoped to
> one tenant could accidentally surface another tenant's encrypted blobs if the
> query is not properly scoped. **`tenant_id` is required.** The PK changes from
> `(path)` to `(tenant_id, path)` to enforce per-tenant path isolation.

> **Three sibling tables.** The libSQL root filesystem has three interdependent
> tables (`root_filesystem_entries`, `root_filesystem_index_specs`,
> `root_filesystem_events`). All three must be created by V018 because
> `PostgresRootFilesystem` will need equivalent query-index and event-log
> support. Index specs store metadata for custom query indices registered
> by the VFS layer; events store a per-path audit/notification trail.
> Both are scoped to `tenant_id` for multi-tenant isolation.

```sql
-- V018__root_filesystem.sql  (kept for unrecognised VFS paths)

-- Primary VFS blob store.
CREATE TABLE IF NOT EXISTS brassclaw_root_filesystem (
    -- tenant_id is required for multi-tenant isolation. The rewrap encrypted-row
    -- check (§4.4 fail-closed rule) scopes to tenant_id to avoid cross-tenant
    -- false positives. PostgresRootFilesystem must always scope queries to tenant_id.
    tenant_id    TEXT        NOT NULL,
    path         TEXT        NOT NULL,
    contents     BYTEA,
    is_dir       BOOLEAN     NOT NULL DEFAULT false,
    content_type TEXT,
    kind         TEXT,
    indexed      JSONB,
    -- version: optimistic locking counter for VFS writes (mirrors the CAS
    -- pattern in ScopedFilesystem). Starts at 0; incremented on every
    -- conditional UPDATE. See brassclaw_resource_accounts.version (§4.12)
    -- for the same pattern used for budget CAS.
    version      BIGINT      NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, path)
);
CREATE INDEX IF NOT EXISTS brassclaw_root_filesystem_tenant_encrypted_idx
    ON brassclaw_root_filesystem (tenant_id)
    WHERE contents IS NOT NULL AND kind = 'encrypted';
CREATE TRIGGER brassclaw_root_filesystem_updated_at
    BEFORE UPDATE ON brassclaw_root_filesystem
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- VFS query-index specifications: stores custom index registrations keyed by
-- (tenant_id, prefix, name). Mirrors libSQL root_filesystem_index_specs.
-- 'keys' is a JSON-serialised list of indexed field paths; 'kind' is the index
-- type string (e.g. 'json_path').
CREATE TABLE IF NOT EXISTS brassclaw_root_filesystem_index_specs (
    tenant_id   TEXT        NOT NULL,
    prefix      TEXT        NOT NULL,
    name        TEXT        NOT NULL,
    keys        TEXT        NOT NULL,   -- JSON array of field paths
    kind        TEXT        NOT NULL,   -- index type (open-ended, no CHECK)
    PRIMARY KEY (tenant_id, prefix, name)
);

-- VFS per-path event log: append-only audit/notification trail keyed per path.
-- Mirrors libSQL root_filesystem_events. Used by the VFS event projection
-- (FilesystemEventProjectionSource). No updated_at — append-only.
-- Retention: swept by the §4.21 background maintenance loop if the volume
-- is a concern (no default TTL set; operator-configurable via retention.vfs_events).
CREATE TABLE IF NOT EXISTS brassclaw_root_filesystem_events (
    seq         BIGSERIAL   PRIMARY KEY,
    tenant_id   TEXT        NOT NULL,
    path        TEXT        NOT NULL,
    payload     BYTEA       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_root_filesystem_events_path_seq_idx
    ON brassclaw_root_filesystem_events (tenant_id, path, seq);
```

This is a slim fallback for any VFS path not covered by a domain table.
Long-term goal: eliminate it entirely by migrating each remaining path to a
typed table.

### 4.20 Shared trigger: updated_at

`V000__shared_triggers.sql` runs first, installs the `pgvector` extension, and
defines the `set_updated_at()` function. The extension is installed **before**
any other migration statement because the chunk system's `embedding` indexed key
is materialised as a `vector(N)` column in the VFS backing table
(`brassclaw_root_filesystem`, §4.19) by `PostgresRootFilesystem::ensure_index`
with `IndexKind::Vector { dim }` (§4.30.4) — that column depends on the
extension being registered. Each table's own migration includes its individual
`CREATE TRIGGER` statement (shown in §4.2–§4.19 and §4.29 below). This keeps
each migration self-contained and makes it clear which tables have the trigger
without requiring implementers to cross-reference V000.

```sql
-- V000__shared_triggers.sql  (runs first — installs pgvector, then defines the function)

-- pgvector must be installed before any migration that defines a 'vector' column.
-- For embedded-Postgres, brassclaw_embedded_postgres/src/initdb.rs must bundle
-- the pgvector shared library and run this statement after initdb completes.
-- For external-Postgres operators, this statement is idempotent and safe to re-run.
CREATE EXTENSION IF NOT EXISTS vector;

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
```

### 4.21 Retention / TTL policy

Append-only tables grow indefinitely without operator action. The following
default retention windows apply; all are configurable via `brassclaw_config`
keys (`retention.*`):

| Table | Default retention | Enforcement |
|---|---|---|
| `brassclaw_checkpoints` | Last 10 per run + 30 days | App-layer cleanup after run completion; background sweep daily |
| `brassclaw_events` | 90 days | Background task pruning `WHERE occurred_at < now() - interval '90 days'` |
| `brassclaw_audit_log` | 1 year | Background task (operator may extend for compliance) |
| `brassclaw_runs` (soft-deleted) | 90 days after `deleted_at` | Background sweep |
| `brassclaw_extensions` (removed) | 90 days after `removed_at` | Background sweep |
| `brassclaw_memory_chat_records` | No expiry (operator-configurable via `retention.memory_chat_records_days`) | Background sweep; default keeps all records (memory is the workspace system per AGENTS.md — persistent memory is not transient log data). Operator may set a TTL to prune old low-importance records. The sweep should respect `importance`: records with `importance >= 0.8` are exempt from TTL pruning. When a Path A record is pruned, the retention sweep MUST also delete the corresponding chunk subtree under `/memory/chat/<chat_record_id>/*.chunks/` (see §4.30 chunk-cascade invariant). |
| `memory_chunks` (VFS, under `/memory/chat/<id>/*.chunks/`) | Cascaded with `brassclaw_memory_chat_records` via `source_ref` | No separate sweep — the chunk subtree is deleted by the `brassclaw_memory_chat_records` sweep when the owning Path A row is pruned (see §4.30 chunk-cascade invariant). Document chunks under `/memory/docs/*` follow the existing `brassclaw_memory_docs` retention (§4.17) and are NOT cascaded here. |
| `brassclaw_forensic_packets` | 90 days (operator-configurable via `retention.forensic_packets_days`) | Background sweep: `WHERE captured_at < now() - interval 'N days'`. Forensic packets are diagnostic/audit data — operators with compliance requirements should extend the TTL. Pruning a packet row sets `forensic_packet_id = NULL` on any linked `brassclaw_memory_chat_records` rows (UPDATE … WHERE forensic_packet_id = $id) before deleting the packet, preserving the memory record even when the originating packet is pruned. |

Pruning tasks run as part of a background maintenance loop inside the
**`brassclaw serve` process only** (not via `pg_cron`, to avoid an external
dependency). `brassclaw run` (one-shot CLI) does not start the maintenance
loop and performs no retention sweep on exit. Consequence: operators who use
`brassclaw run` exclusively (no long-running serve) will see unbounded growth
in `brassclaw_checkpoints` and `brassclaw_events` until they run
`brassclaw serve` at least once (or manually run
`brassclaw maintenance prune-old-data`). This is documented in the operator
guide produced in Phase 9.

CLAUDE.md's "LLM data is never deleted" rule is not violated: LLM output
(reasoning, tool calls, messages) lives in `brassclaw_turns.payload` and
`brassclaw_checkpoints.payload`, which are retained until the run itself is
soft-deleted past its TTL.

### 4.22 Budget gates

> **H1:** `BudgetGateStore` trait + `FilesystemBudgetGateStore<F>` live in
> `brassclaw_resources` (path `/resources/budget-gates.json`). This store was
> absent from the original plan. `PgBudgetGateStore` must be added to
> `brassclaw_resources` alongside `PgResourceGovernorStore`.

```sql
-- V019__budget_gates.sql
CREATE TABLE IF NOT EXISTS brassclaw_budget_gates (
    id          TEXT        NOT NULL PRIMARY KEY,   -- ULID
    tenant_id   TEXT        NOT NULL,
    run_id      TEXT        REFERENCES brassclaw_runs(id) ON DELETE RESTRICT,
    -- gate_kind: the ResourceDimension that triggered the gate (e.g.
    -- "usd", "input_tokens", "output_tokens", "wall_clock_ms", "output_bytes",
    -- "network_egress_bytes", "process_count", "concurrency_slots" — per
    -- ResourceDimension #[serde(rename_all = "snake_case")] at lib.rs:394).
    -- Extracted from BudgetApprovalGate.needed.dimension for indexing; the full
    -- BudgetApprovalGate (including needed.account, needed.limit,
    -- needed.current_usage, etc.) is in payload JSONB.
    -- Open-ended string (no CHECK) because ResourceDimension may gain new
    -- variants without a schema migration.
    gate_kind   TEXT        NOT NULL,
    -- status maps to BudgetGateStatus enum variants (serde tag = "kind",
    -- rename_all = "snake_case"): Pending, Approved{...}, Cancelled{...},
    -- Expired{...}. Note: "cancelled" NOT "denied" — the enum has
    -- Cancelled, not Denied. The Approve/Cancel/Expire sub-fields (by, at,
    -- increased_limit) are stored in payload JSONB.
    status      TEXT        NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','cancelled','expired')),
    requested_amount NUMERIC(18,6) NOT NULL,
    payload     JSONB       NOT NULL DEFAULT '{}',
    expires_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_budget_gates_tenant_status_idx
    ON brassclaw_budget_gates (tenant_id, status) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS brassclaw_budget_gates_run_idx
    ON brassclaw_budget_gates (run_id) WHERE run_id IS NOT NULL;
CREATE TRIGGER brassclaw_budget_gates_updated_at
    BEFORE UPDATE ON brassclaw_budget_gates
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemBudgetGateStore` / `/resources/budget-gates.json`.

### 4.23 Identities

> **H2:** `FilesystemRebornIdentityStore` lives in `brassclaw_reborn_identity`
> and is wired in `factory.rs`. It was absent from the original plan.
> `PgRebornIdentityStore` must be added to `brassclaw_reborn_identity`.

> **L4 — Schema revised to match the full 5-part identity key:** The filesystem
> store keys identity records by all five components:
> `(tenant_id, surface_kind, provider_kind, provider_instance_id, external_subject_id)`.
> The previous draft had only `provider` and `kind`, omitting `provider_instance_id`
> (adapter installation id) and `external_subject_id` (OAuth sub / channel actor id) —
> both essential for uniqueness. Without `external_subject_id`, one row per
> `(tenant, surface, provider, instance)` is the maximum granularity, which would
> collapse all users of the same provider into one row. The table below adds all
> missing columns. There are also two ancillary record types modelled in separate
> tables: `brassclaw_identity_users` (canonical user profile) and
> `brassclaw_identity_email_index` (verified-email cross-provider link index).
> All three tables are created by `V020__identities.sql`.
>
> Note: `provider_instance_id` is `Option<ProviderInstanceId>` in the store — absent
> for browser OAuth (no adapter installation), present for channel actors (e.g. a
> specific Telegram bot token id). The PG column stores `''` (empty string) when
> absent, matching the filesystem path convention — NOT NULL is safe because the
> store always supplies a value (empty string for `None`).

```sql
-- V020__identities.sql

-- External identity records: one per (tenant, surface_kind, provider_kind,
-- provider_instance_id, external_subject_id) tuple. Maps the external identity
-- to a canonical Reborn user_id.
CREATE TABLE IF NOT EXISTS brassclaw_identities (
    id                   TEXT        NOT NULL PRIMARY KEY,  -- ULID
    tenant_id            TEXT        NOT NULL,
    -- surface_kind maps to SurfaceKind::as_str():
    -- "oauth" (browser SSO login) or "channel_actor" (Telegram/Slack/trigger/…).
    -- CHECK enforces the closed set; adding a new surface requires a migration.
    surface_kind         TEXT        NOT NULL
        CHECK (surface_kind IN ('oauth','channel_actor')),
    -- provider_kind: stable wire string, e.g. "google", "github", "telegram".
    -- No CHECK — new providers may be added without a schema migration.
    provider_kind        TEXT        NOT NULL,
    -- provider_instance_id: adapter installation id where relevant (channel
    -- actors); stored as '' (empty string) when absent (browser OAuth).
    -- NOT NULL: the store always supplies a value (empty string for None).
    provider_instance_id TEXT        NOT NULL DEFAULT '',
    -- external_subject_id: stable per-provider subject id (OAuth `sub`,
    -- channel actor id). Required — the row is meaningless without it.
    external_subject_id  TEXT        NOT NULL,
    -- user_id: the resolved canonical Reborn UserId for this identity.
    user_id              TEXT        NOT NULL,
    -- email / email_verified: stored for cross-provider linking and audit.
    -- Nullable: channel actors carry no email.
    email                TEXT,
    email_verified       BOOLEAN     NOT NULL DEFAULT false,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
    -- No deleted_at: the store uses CAS-on-absent semantics (not soft-delete).
    -- Re-binding (bind()) overwrites in place via CAS. Records are never
    -- logically deleted — only re-pointed to a different user_id.
);
-- The natural key is the full 5-part tuple. This is the uniqueness constraint
-- the filesystem store enforces via the identity path + CasExpectation::Absent.
CREATE UNIQUE INDEX IF NOT EXISTS brassclaw_identities_key_idx
    ON brassclaw_identities
    (tenant_id, surface_kind, provider_kind, provider_instance_id, external_subject_id);
-- Fast lookup of all identities for a given user (e.g. show all linked
-- providers on a user profile page, or list all identities to migrate).
CREATE INDEX IF NOT EXISTS brassclaw_identities_user_idx
    ON brassclaw_identities (tenant_id, user_id);
CREATE TRIGGER brassclaw_identities_updated_at
    BEFORE UPDATE ON brassclaw_identities
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Canonical user records: one per Reborn UserId. Created by resolve_or_create
-- before the identity record is written (so an identity always points at an
-- existing user). Orphan rows may exist when the identity CAS was lost to a
-- concurrent writer (see StoredUser / put_identity_reconciling comments in
-- brassclaw_reborn_identity). GC of unreferenced rows is out of scope.
CREATE TABLE IF NOT EXISTS brassclaw_identity_users (
    user_id              TEXT        NOT NULL PRIMARY KEY,
    email                TEXT,
    display_name         TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_identity_users_email_idx
    ON brassclaw_identity_users (email) WHERE email IS NOT NULL;
CREATE TRIGGER brassclaw_identity_users_updated_at
    BEFORE UPDATE ON brassclaw_identity_users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Verified-email cross-provider link index: one row per (tenant, lower(email))
-- pair for OAuth-surface identities with a verified email. Enables a later
-- login through a different provider to find the canonical user without a full
-- identity table scan. Written before the identity record (index invariant: if
-- an identity record exists for a verified-email OAuth identity, its index row
-- must also exist). The store uses CasExpectation::Absent on insert — first
-- writer wins; losers adopt the winner's user_id (no second user minted).
-- Tenant-scoped: linking is confined to a single tenant.
CREATE TABLE IF NOT EXISTS brassclaw_identity_email_index (
    tenant_id            TEXT        NOT NULL,
    -- email_lower: lowercased email, the canonical form used as the index key.
    -- NOT NULL: rows are only created for non-empty verified emails.
    email_lower          TEXT        NOT NULL,
    user_id              TEXT        NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, email_lower)
    -- No updated_at: this table is append-only (first writer wins, no updates).
    -- No deleted_at: cross-provider links are permanent within a tenant.
);
```

Replaces: `FilesystemRebornIdentityStore` VFS identity records (`/tenant-shared/reborn-identity/external/…`), user records (`/tenant-shared/reborn-identity/users/…`), and verified-email index records (`/tenant-shared/reborn-identity/verified-email/…`).

### 4.24 Triggers

> **Both backends already exist.** `LibSqlTriggerRepository` (libSQL) and
> `PostgresTriggerRepository` (Postgres) both exist in `brassclaw_triggers`
> and implement `TriggerRepository`. The libSQL path is retired here; the
> Postgres path is promoted to unconditional. The DDL below is lifted verbatim
> from `POSTGRES_TRIGGER_SCHEMA` in
> `crates/brassclaw_triggers/src/postgres.rs` (lines ~968–1002), then wrapped
> in `IF NOT EXISTS` guards and an `ALTER TABLE … RENAME` for the table rename.
> The `libsql` feature gate is removed from `brassclaw_triggers/Cargo.toml`.
>
> **Table rename: `trigger_records` → `brassclaw_triggers`.** The existing
> `PostgresTriggerRepository` uses the table name `trigger_records` (constant
> `TRIGGER_TABLE` at line 16, `TRIGGER_COLUMNS` at line 17). V021 renames it
> to `brassclaw_triggers` for namespace consistency with all other plan tables.
> After V021 runs, the `TRIGGER_TABLE` and `TRIGGER_COLUMNS` constants in
> `postgres.rs` **must** be updated to reference `brassclaw_triggers`. All
> query strings in `PostgresTriggerRepository` use these two constants via
> format strings, so they will all update automatically; no per-query edits
> are needed beyond updating the two constants.
>
> **Column types: TEXT for all timestamps.** The existing code (both
> `postgres.rs` and `libsql.rs`) stores all date/time values as formatted
> RFC-3339 TEXT strings (`fmt_ts()` / `SecondsFormat::Secs`). The DDL here
> preserves that convention — no TIMESTAMPTZ columns — so `PostgresTriggerRepository`
> can keep its existing `row_to_record` deserialization with zero changes.
> Using TIMESTAMPTZ would require all query parameters and row reads to change
> from `&str` to `DateTime<Utc>` with format/parse round-trips.
>
> **`local_reborn_access` is local-dev only.** `RebornLibSqlLocalTriggerAccessStore`
> stores bootstrap access grants for trigger-fire authorization in local-dev
> environments only (no production path, no `TriggerRepository` trait). It is
> migrated to a Postgres table during §8.1 step 7 (synthesised from
> `boot_tenant` + `boot_user`), but only as a no-op convenience — the store
> itself is re-implemented as `PgLocalTriggerAccessStore` with the same
> local-dev-only scope.

```sql
-- V021__triggers.sql

-- Rename trigger_records to brassclaw_triggers on existing deployments.
-- On fresh deployments the table does not yet exist; the DO block skips the
-- rename and the CREATE TABLE IF NOT EXISTS below creates it directly.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'trigger_records'
          AND table_schema = current_schema()
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'brassclaw_triggers'
          AND table_schema = current_schema()
    ) THEN
        ALTER TABLE trigger_records RENAME TO brassclaw_triggers;
        ALTER INDEX IF EXISTS trigger_records_state_next_run_at_idx
            RENAME TO brassclaw_triggers_state_next_run_at_idx;
        ALTER INDEX IF EXISTS trigger_records_tenant_created_at_idx
            RENAME TO brassclaw_triggers_tenant_created_at_idx;
        ALTER INDEX IF EXISTS trigger_records_scoped_list_idx
            RENAME TO brassclaw_triggers_scoped_list_idx;
        ALTER INDEX IF EXISTS trigger_records_active_fire_slot_idx
            RENAME TO brassclaw_triggers_active_fire_slot_idx;
    END IF;
END $$;

-- All columns match the existing POSTGRES_TRIGGER_SCHEMA in postgres.rs exactly.
-- Date columns are TEXT (RFC-3339 formatted) — do NOT change to TIMESTAMPTZ;
-- PostgresTriggerRepository serialises/deserialises via fmt_ts() string round-trips.
-- TriggerState wire values (snake_case): 'scheduled', 'paused', 'completed'.
CREATE TABLE IF NOT EXISTS brassclaw_triggers (
    trigger_id             TEXT        NOT NULL,
    tenant_id              TEXT        NOT NULL,
    creator_user_id        TEXT        NOT NULL,
    agent_id               TEXT,
    project_id             TEXT,
    name                   TEXT        NOT NULL,
    source                 TEXT        NOT NULL,
    schedule_expression    TEXT        NOT NULL,
    completion_policy      TEXT        NOT NULL,
    prompt                 TEXT        NOT NULL,
    -- state: TriggerState snake_case — 'scheduled' | 'paused' | 'completed'
    state                  TEXT        NOT NULL,
    -- next_run_at / created_at: TEXT (RFC-3339) — matches existing query layer
    next_run_at            TEXT        NOT NULL,
    last_run_at            TEXT,
    last_fired_slot        TEXT,
    last_status            TEXT,
    active_fire_slot       TEXT,
    active_run_ref         TEXT,
    created_at             TEXT        NOT NULL,
    PRIMARY KEY (tenant_id, trigger_id)
);

-- Indexes match POSTGRES_TRIGGER_SCHEMA verbatim (column sets, ordering).
-- The due-fire query filters WHERE state = $1 (TriggerState::Scheduled wire
-- value 'scheduled') AND next_run_at <= $2 — no partial index by state because
-- any state value can be queried; the composite index covers the hot path.
CREATE INDEX IF NOT EXISTS brassclaw_triggers_state_next_run_at_idx
    ON brassclaw_triggers (state, next_run_at, tenant_id, trigger_id);
-- Tenant-scoped list by creation time (creator panel queries).
CREATE INDEX IF NOT EXISTS brassclaw_triggers_tenant_created_at_idx
    ON brassclaw_triggers (tenant_id, created_at, trigger_id);
-- Scoped list by (tenant, creator, agent, project) — covers list_scoped_triggers.
CREATE INDEX IF NOT EXISTS brassclaw_triggers_scoped_list_idx
    ON brassclaw_triggers (tenant_id, creator_user_id, agent_id, project_id, created_at, trigger_id);
-- Active-fire scan: only rows with an in-progress fire slot (sparse).
CREATE INDEX IF NOT EXISTS brassclaw_triggers_active_fire_slot_idx
    ON brassclaw_triggers (active_fire_slot, tenant_id, trigger_id)
    WHERE active_fire_slot IS NOT NULL;

-- updated_at trigger (added on top of the existing schema — not in POSTGRES_TRIGGER_SCHEMA
-- because the original `run_migrations()` predates the shared set_updated_at() trigger).
-- Add it here to bring the table in line with all other plan tables.
-- (The trigger creation is conditional; run after the rename to avoid
-- duplicating it on upgrade paths where the trigger already exists under the
-- old table name.)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_trigger WHERE tgname = 'brassclaw_triggers_updated_at'
    ) THEN
        -- updated_at column added here for new table; existing trigger_records rows
        -- have no updated_at — add the column with a backfill default if missing.
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_name = 'brassclaw_triggers'
              AND column_name = 'updated_at'
              AND table_schema = current_schema()
        ) THEN
            ALTER TABLE brassclaw_triggers
                ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
        END IF;
        CREATE TRIGGER brassclaw_triggers_updated_at
            BEFORE UPDATE ON brassclaw_triggers
            FOR EACH ROW EXECUTE FUNCTION set_updated_at();
    END IF;
END $$;

-- Local-dev bootstrap access table. Local-dev only — no multi-tenant scope needed
-- in practice, but tenant_id is included for schema consistency.
-- Column types match local_trigger_access.rs DDL exactly (all TEXT, no TIMESTAMPTZ).
CREATE TABLE IF NOT EXISTS brassclaw_local_access (
    tenant_id   TEXT        NOT NULL,
    user_id     TEXT        NOT NULL,
    agent_id    TEXT        NOT NULL,
    project_id  TEXT        NOT NULL,
    role        TEXT        NOT NULL,
    -- status: 'active' | 'inactive'  (LocalTriggerAccessStatus wire values)
    status      TEXT        NOT NULL
        CHECK (status IN ('active','inactive')),
    source      TEXT        NOT NULL,
    created_at  TEXT        NOT NULL,
    updated_at  TEXT        NOT NULL,
    PRIMARY KEY (tenant_id, user_id, agent_id, project_id)
);
```

Replaces: libSQL `trigger_records` + `local_reborn_access` tables.

### 4.25 Conversation state

> **Design note.** `FilesystemConversationStateStore` persists the entire in-memory
> conversation state as a single JSON document at `/conversations/state.json` under
> a CAS revision counter. The document contains actor pairings, thread records,
> accepted messages, reply targets, idempotency keys, and external event routes —
> all of which are small and change atomically together (the store serialises the
> whole blob on every mutation). The PG model normalises this into a single JSONB
> column (`state_blob`) with an optimistic-lock `revision` counter, preserving the
> atomic-replacement semantics. Fine-grained normalisation is a future concern.
> `tenant_id` is the natural scope key — the store's mount alias is tenant-scoped.
>
> **CAS contract for `PgConversationStateStore`.** The
> `ConversationStateRepository::save_state(expected_revision, state)` trait
> (defined in `state_store.rs`) requires optimistic concurrency: the write
> must succeed only if the stored revision equals `expected_revision`, and must
> return the new revision on success. The Postgres implementation must use:
>
> ```sql
> -- Load (in load_state):
> SELECT state_blob, revision FROM brassclaw_conversation_state
> WHERE tenant_id = $1;
>
> -- First-write (no row yet — INSERT with revision = 0):
> INSERT INTO brassclaw_conversation_state (tenant_id, state_blob, revision)
> VALUES ($tenant_id, $state_blob, 1)
> ON CONFLICT (tenant_id) DO NOTHING
> RETURNING revision;
> -- If rows_affected = 0 → concurrent writer won; caller must retry from load_state.
>
> -- CAS update (row exists — only update if revision matches expected):
> UPDATE brassclaw_conversation_state
> SET state_blob = $state_blob, revision = revision + 1
> WHERE tenant_id = $tenant_id AND revision = $expected_revision
> RETURNING revision;
> -- If rows_affected = 0 → concurrent writer advanced the revision; caller retries.
> ```
>
> The caller (`save_state`) must check rows-affected (or the RETURNING clause) and
> return `Err(InboundTurnError::DurableState { reason: "PG CAS conflict: concurrent writer" })`
> when zero rows are affected. **Note:** `InboundTurnError` has no `ConflictRetry` variant —
> `DurableState { reason: String }` is the correct variant for all durable-state persistence
> failures, including optimistic-lock conflicts (confirmed in `error.rs`: 9 variants, no
> `ConflictRetry`; the filesystem store uses `DurableState` at lines 263-274 when CAS retries
> are exhausted). `PgConversationStateStore` must use the same variant. Callers of `save_state`
> already handle this: the filesystem store uses `CasExpectation` which maps to the same
> semantics.

```sql
-- V022__conversation_state.sql
CREATE TABLE IF NOT EXISTS brassclaw_conversation_state (
    tenant_id   TEXT        NOT NULL PRIMARY KEY,
    -- state_blob: serialised StoredConversationState (JSON, same wire format the
    -- filesystem store currently writes). Contains actor pairings, thread records,
    -- accepted messages, reply targets, idempotency keys, and external event routes.
    state_blob  JSONB       NOT NULL DEFAULT '{}',
    -- revision: monotonically incrementing CAS counter. Mirrors the i64 revision
    -- field in StoredConversationState. PgConversationStateStore must check
    -- rows_affected on UPDATE and retry if 0 (concurrent writer won).
    -- See CAS contract in the design note above.
    revision    BIGINT      NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER brassclaw_conversation_state_updated_at
    BEFORE UPDATE ON brassclaw_conversation_state
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemConversationStateStore` / `/conversations/state.json`.

### 4.26 Outbound state

> **Design note.** `FilesystemOutboundStateStore` persists four distinct data
> shapes under `/outbound/*`:
> - **policies** — per-thread notification policies (small, keyed by thread scope)
> - **subscriptions** — projection subscription cursors (keyed by subscription id)
> - **deliveries** — delivery attempt records (keyed by delivery id, indexed by scope)
> - **preferences** — communication preference documents (keyed by `CommunicationPreferenceKey`)
>
> These are modelled as four separate tables to preserve query isolation and allow
> targeted indexes. Each implements `tenant_id` scoping for multi-tenant isolation.
>
> **Policy key normalisation.** The filesystem store uses an opaque SHA-256 hash of
> the `ThreadScopeKey` as the path component. The PG table normalises the key into
> its four component columns (`tenant_id`, `agent_id`, `project_id`, `thread_id`)
> matching the `ThreadScopeKey` struct in `crates/brassclaw_outbound/src/memory.rs`
> (lines 38–42). This enables efficient index-only queries by any combination of
> tenant, agent, project, or thread without materialising the hash.
>
> **Preference key normalisation.** The filesystem store uses a SHA-256 hash of the
> `CommunicationPreferenceKey` struct as the path component (see
> `filesystem_store.rs` comment: `sha256(v1-length-prefixed-key)`). The real key is
> `CommunicationPreferenceKey { tenant_id, user_id }` (defined in
> `crates/brassclaw_outbound/src/communication_preferences.rs`). The PG table uses
> `(tenant_id, user_id)` as the primary key directly — no hash is needed because PG
> can index structured columns. The `CommunicationPreferenceRecord` has five typed
> preference fields (`final_reply_target`, `progress_target`, `approval_prompt_target`,
> `auth_prompt_target`, `default_modality`) stored as JSONB (they are
> `Option<ReplyTargetBindingRef>` / `Option<CommunicationModality>` — non-trivial
> types that do not benefit from further column decomposition).

```sql
-- V023__outbound_state.sql

-- Thread notification policies: one row per (tenant_id, agent_id, project_id, thread_id).
-- Key columns match ThreadScopeKey in brassclaw_outbound/src/memory.rs.
-- agent_id and project_id are nullable (None = exact no-scope match, not wildcard).
-- PostgreSQL PRIMARY KEY columns must be NOT NULL; nullable scope columns are part of a
-- UNIQUE constraint using IS NOT DISTINCT FROM semantics instead (nullable-safe uniqueness).
CREATE TABLE IF NOT EXISTS brassclaw_outbound_policies (
    tenant_id   TEXT        NOT NULL,
    -- agent_id: NULL means no agent scope (exact match, not wildcard)
    agent_id    TEXT,
    -- project_id: NULL means no project scope (exact match, not wildcard)
    project_id  TEXT,
    thread_id   TEXT        NOT NULL,
    policy      JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Cannot use PRIMARY KEY on nullable columns; use a partial unique index instead.
    -- The NOT DISTINCT FROM semantics ensure two NULLs compare as equal (matching
    -- the ThreadScopeKey Eq behaviour where None == None for agent_id/project_id).
    CONSTRAINT brassclaw_outbound_policies_scope_unique
        UNIQUE NULLS NOT DISTINCT (tenant_id, agent_id, project_id, thread_id)
);
CREATE INDEX IF NOT EXISTS brassclaw_outbound_policies_tenant_thread_idx
    ON brassclaw_outbound_policies (tenant_id, thread_id);
CREATE TRIGGER brassclaw_outbound_policies_updated_at
    BEFORE UPDATE ON brassclaw_outbound_policies
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Projection subscription cursors: one row per subscription id.
CREATE TABLE IF NOT EXISTS brassclaw_outbound_subscriptions (
    id          TEXT        NOT NULL PRIMARY KEY,   -- ULID
    tenant_id   TEXT        NOT NULL,
    cursor      JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_outbound_subscriptions_tenant_idx
    ON brassclaw_outbound_subscriptions (tenant_id);
CREATE TRIGGER brassclaw_outbound_subscriptions_updated_at
    BEFORE UPDATE ON brassclaw_outbound_subscriptions
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Delivery attempt records: one row per delivery_id.
-- scope_key is a deterministic hash of (tenant, agent?, project?, thread) —
-- the same hash the FilesystemOutboundStateStore stores as the index key
-- (DELIVERY_SCOPE_INDEX_KEY). The hash is preserved here because delivery
-- records are looked up by scope but not queried by agent/project individually;
-- keeping the hash avoids a four-column PK on a high-volume table.
CREATE TABLE IF NOT EXISTS brassclaw_outbound_deliveries (
    id          TEXT        NOT NULL PRIMARY KEY,   -- ULID
    tenant_id   TEXT        NOT NULL,
    scope_key   TEXT        NOT NULL,
    payload     JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_outbound_deliveries_tenant_scope_idx
    ON brassclaw_outbound_deliveries (tenant_id, scope_key);
CREATE TRIGGER brassclaw_outbound_deliveries_updated_at
    BEFORE UPDATE ON brassclaw_outbound_deliveries
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Communication preferences: one row per (tenant_id, user_id).
-- Key matches CommunicationPreferenceKey in communication_preferences.rs.
-- The filesystem store uses sha256(key) as the path; PG uses structured columns.
-- Each preference field is JSONB because the types (ReplyTargetBindingRef,
-- CommunicationModality) are non-trivial structs not suited to further decomposition.
CREATE TABLE IF NOT EXISTS brassclaw_outbound_preferences (
    tenant_id               TEXT        NOT NULL,
    user_id                 TEXT        NOT NULL,
    -- Option<ReplyTargetBindingRef> — null means unset
    final_reply_target      JSONB,
    progress_target         JSONB,
    approval_prompt_target  JSONB,
    auth_prompt_target      JSONB,
    -- Option<CommunicationModality> — null means unset
    default_modality        JSONB,
    -- updated_by: UserId of the last writer (from CommunicationPreferenceRecord.updated_by,
    -- typed UserId — confirmed at communication_preferences.rs:35). This is a required field
    -- in the record; PgOutboundStateStore MUST always write the real UserId value from the
    -- record and never rely on the empty-string default. DEFAULT '' exists only as a
    -- schema-level fallback; it must never be the actual written value at the app layer.
    updated_by              TEXT        NOT NULL DEFAULT '',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id)
);
CREATE TRIGGER brassclaw_outbound_preferences_updated_at
    BEFORE UPDATE ON brassclaw_outbound_preferences
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemOutboundStateStore` / `/outbound/*`.

### 4.27 Subagent goals

> **Design note.** `FilesystemSubagentGoalStore` stores one subagent goal JSON blob
> per `(scope, run_id)` under `/turns/subagent-goals/<agent?>/projects/<project?>/threads/<thread>/<run_id>.json`
> (the same `/turns` mount as `FilesystemTurnStateStore`; see `goal_path()` in
> `crates/brassclaw_reborn/src/subagent/goal_store.rs`).
>
> **Schema normalisation.** `SubagentGoal` (defined at line 19 of `goal_store.rs`)
> has exactly two fields: `task: String` and `handoff: Option<String>`. There is no
> benefit to storing these in an opaque JSONB blob — both are plain strings. The PG
> table stores them as TEXT columns directly, which allows the DB to enforce NOT NULL
> on `task`, enables text-length checks, and avoids a serde round-trip on read.
>
> **Store key.** `SubagentGoalStore::put_goal(scope, run_id, goal)` uniquely
> identifies a goal by `(tenant_id, run_id)` — one goal per run (the filesystem
> path encodes scope components but `run_id` is the ULID that distinguishes goals
> within a tenant). The table uses `(tenant_id, run_id)` as the unique constraint
> (natural key), with a surrogate `id` ULID as the physical primary key for stable
> references.
>
> **No FK to `brassclaw_runs`.** Same rationale as `brassclaw_events.run_id`: goals
> arrive before the run row is fully committed. App layer resolves the association.
>
> **`filesystem-goal-store` feature gate.** This feature must be removed alongside
> the other filesystem feature removals in Phase 6. It wraps the
> `FilesystemSubagentGoalStore` struct and its `SubagentGoalStore` + `SubagentSpawnGoalStore`
> impls; the in-memory `InMemoryBoundedSubagentGoalStore` (test double) has no
> feature gate and is retained for testing.

```sql
-- V024__subagent_goals.sql
CREATE TABLE IF NOT EXISTS brassclaw_subagent_goals (
    id          TEXT        NOT NULL PRIMARY KEY,   -- ULID (row id)
    tenant_id   TEXT        NOT NULL,
    -- run_id: TurnRunId ULID — one goal per run (see SubagentGoalStore::put_goal).
    -- Soft reference to brassclaw_runs(id): no FK because the goal row may be
    -- inserted before the run row is committed during spawn sequencing.
    run_id      TEXT        NOT NULL,
    -- task: SubagentGoal.task — the goal description text (required, max 64KiB
    -- per MAX_GOAL_BYTES). NOT NULL enforced here; app-layer validation also runs.
    task        TEXT        NOT NULL,
    -- handoff: SubagentGoal.handoff — optional context string passed to the child
    -- agent at spawn time. NULL means no handoff.
    handoff     TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Uniqueness enforced by the store's DuplicateKey error path.
    CONSTRAINT brassclaw_subagent_goals_tenant_run_unique UNIQUE (tenant_id, run_id)
);
CREATE INDEX IF NOT EXISTS brassclaw_subagent_goals_tenant_idx
    ON brassclaw_subagent_goals (tenant_id);
CREATE TRIGGER brassclaw_subagent_goals_updated_at
    BEFORE UPDATE ON brassclaw_subagent_goals
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

Replaces: `FilesystemSubagentGoalStore` / `/turns/subagent-goals/*`.

### 4.28 Interceptor store

> **Current state:** `brassclaw_interceptor` defines an `InterceptorStore` trait
> with `save`, `get`, and `list_recent` (see `crates/brassclaw_interceptor/src/store.rs`).
> The only current implementation is `NoopInterceptorStore` — all writes are
> discarded and reads return empty results. **There is no durable store today;
> no migration data exists.** A proper `PgInterceptorStore` is added here
> (V026) so forensic packets are durably stored alongside the chat-memory
> records they produce.
>
> **`ForensicPacket` struct** (from `crates/brassclaw_interceptor/src/packet.rs`):
> - `id: PacketId` — a `Uuid::new_v4()` wrapped in a newtype. Stored as TEXT (UUID string).
> - `status: PacketStatus` — enum with `#[serde(rename_all = "snake_case")]`;
>   wire values: `'awaiting_kohai'` | `'complete'` | `'sempai_reviewed'`.
> - `run_id: String` — the turn's run ID (matches `brassclaw_memory_chat_records.run_id`).
> - `iteration: u32` — the prompt-assembly iteration counter within the run.
> - `captured_at: DateTime<Utc>` — when the prompt was captured (after PromptStage).
> - `completed_at: Option<DateTime<Utc>>` — when the Kohai response arrived.
> - `prompt: CapturedPrompt` — the assembled prompt + segment breakdown (JSONB).
> - `kohai_response: Option<String>` — the Kohai response text.
> - `kohai_usage: Option<KohaiUsage>` — `{input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens}`.
> - `sempai_review: Option<SempaiReviewOutcome>` — `{adjusted_messages, composition_summary, proposed_recipe_updates, settings_adjustments}`.
>
> **Link to chat-memory records.** A forensic packet is created before Kohai
> is called via `ForensicPacket::new(run_id, iteration, prompt)` (confirmed in
> `crates/brassclaw_interceptor/src/packet.rs:179`) — `run_id` and `iteration`
> are the stable correlation key for the retroactive link. `memory_write` tool
> calls happen after the Kohai response, during tool dispatch. The link is
> therefore **populated retroactively**: after Path A writes a
> `brassclaw_memory_chat_records` row (with the turn's `run_id`),
> `PgInterceptorStore` (or the memory-write path) can look up the packet for
> the same `(tenant_id, run_id, iteration)` and store the `chat_record_id` on
> the packet row. In the other direction, `brassclaw_memory_chat_records` stores a
> `forensic_packet_id` column (added in §4.29 DDL below) so the memory record can
> find its originating prompt packet.
>
> Both links are soft references — no FK — because:
> 1. Packets and memory records are written at different points in the turn (packet
>    first, memory record after Kohai response + tool execution).
> 2. Either store may be absent (`NoopInterceptorStore` running, or a memory_write
>    that predates the interceptor's durable store).
> 3. The `run_id` is the reliable correlating key; the ULID links are a convenience.

```sql
-- V026__forensic_packets.sql
-- Durable store for ForensicPacket records (one per agent-loop turn/iteration).
-- Replaces NoopInterceptorStore. PgInterceptorStore is the new implementation.
--
-- PacketStatus wire values (serde rename_all = "snake_case"):
--   'awaiting_kohai' | 'complete' | 'sempai_reviewed'
--
-- All prompt/response/review fields are JSONB — the structures are non-trivial
-- and evolve with the agent-loop internals; JSONB preserves forward-compatibility.
-- kohai_usage is the exception: its four counter fields are mapped to typed
-- INTEGER columns for efficient query/aggregation (token usage analytics).
CREATE TABLE IF NOT EXISTS brassclaw_forensic_packets (
    -- id: PacketId (Uuid::new_v4() as TEXT). Stable cross-restart reference.
    id                TEXT        NOT NULL PRIMARY KEY,
    tenant_id         TEXT        NOT NULL,
    -- run_id: matches ForensicPacket.run_id and brassclaw_memory_chat_records.run_id.
    -- Soft reference — no FK (turns table has brassclaw_runs.id but packet IDs are
    -- minted before run rows are fully committed; same rationale as brassclaw_events).
    run_id            TEXT        NOT NULL,
    -- iteration: prompt-assembly iteration counter within the run (u32).
    -- Together with run_id this uniquely identifies which Kohai call this packet covers.
    iteration         INTEGER     NOT NULL DEFAULT 0,
    -- status: PacketStatus snake_case wire values.
    status            TEXT        NOT NULL DEFAULT 'awaiting_kohai'
        CHECK (status IN ('awaiting_kohai','complete','sempai_reviewed')),
    -- captured_at: when the prompt was captured (after PromptStage). NOT NULL
    -- because the packet is created before the Kohai call and always has this.
    captured_at       TIMESTAMPTZ NOT NULL,
    -- completed_at: when the Kohai response was received. NULL while awaiting_kohai.
    completed_at      TIMESTAMPTZ,
    -- prompt: CapturedPrompt as JSONB — includes messages[], segments[], token_accounting,
    -- capability_surface_version, visible_capability_count. Written at packet creation.
    prompt            JSONB       NOT NULL DEFAULT '{}',
    -- kohai_response: response text from the Kohai model. NULL while awaiting_kohai.
    kohai_response    TEXT,
    -- kohai_usage: KohaiUsage counter fields. NULL while awaiting_kohai.
    -- Stored as typed INTEGER columns (not JSONB) for token-usage analytics queries.
    kohai_input_tokens             INTEGER,
    kohai_output_tokens            INTEGER,
    kohai_cache_read_input_tokens       INTEGER,
    -- Field name matches KohaiUsage.cache_creation_input_tokens exactly
    -- (crates/brassclaw_interceptor/src/packet.rs line 173).
    kohai_cache_creation_input_tokens   INTEGER,
    -- sempai_review: SempaiReviewOutcome as JSONB. NULL unless status = 'sempai_reviewed'.
    -- Contains: adjusted_messages[], composition_summary, proposed_recipe_updates[],
    -- settings_adjustments[].
    sempai_review     JSONB,
    -- chat_record_id: soft reference to brassclaw_memory_chat_records(id).
    -- Populated retroactively after a memory_write call completes for this run.
    -- NULL when no memory was written during the turn, or when Path A was not active.
    -- One packet may produce multiple chat-memory records (one per memory_write call
    -- within the same iteration); this column stores the FIRST chat_record_id written
    -- in the iteration. For multi-memory turns, query brassclaw_memory_chat_records
    -- by (run_id, iteration) — the run_id + iteration link is always reliable.
    chat_record_id    TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Per-tenant recent-packets query (backs InterceptorStore::list_recent — captured_at DESC).
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_tenant_captured_idx
    ON brassclaw_forensic_packets (tenant_id, captured_at DESC);
-- Per-run lookup — correlate all packets for a run (multi-iteration runs).
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_run_idx
    ON brassclaw_forensic_packets (tenant_id, run_id, iteration);
-- Pending packets awaiting Kohai response — narrow scan for stuck-turn diagnostics.
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_awaiting_idx
    ON brassclaw_forensic_packets (tenant_id, captured_at)
    WHERE status = 'awaiting_kohai';
-- chat_record_id back-link — find the packet that produced a given chat-memory record.
CREATE INDEX IF NOT EXISTS brassclaw_forensic_packets_chat_record_idx
    ON brassclaw_forensic_packets (chat_record_id)
    WHERE chat_record_id IS NOT NULL;
CREATE TRIGGER brassclaw_forensic_packets_updated_at
    BEFORE UPDATE ON brassclaw_forensic_packets
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

**`PgInterceptorStore` implementation notes:**
- `save(packet)`: UPSERT on `(id)` — `ON CONFLICT (id) DO UPDATE` — overwrites the row on status transitions (AwaitingKohai → Complete/SempaiReviewed). Maps `KohaiUsage` fields to the four typed integer columns.
- `get(packet_id)`: SELECT by `id`; reconstruct `ForensicPacket` from typed columns + JSONB.
- `list_recent(limit)`: SELECT from `brassclaw_forensic_packets WHERE tenant_id = $1 ORDER BY captured_at DESC LIMIT $2`. Requires the `tenant_id` to be thread-local or passed via a tenant-scoped store wrapper — `InterceptorStore` trait has no `tenant_id` parameter; the PG implementation must be constructed with a `tenant_id` at wire-up time (same pattern as other tenant-scoped stores).
- **Populate `chat_record_id`:** After `PgChatMemoryRecordStore` writes a memory record, the memory-write path should call `PgInterceptorStore::link_chat_record(run_id, iteration, chat_record_id)` to fill the `chat_record_id` column. This is a best-effort UPDATE — if no packet exists for `(tenant_id, run_id, iteration)` (e.g. interceptor was in noop mode during that turn), it is a no-op.

Replaces: `NoopInterceptorStore` (noop → no stored data; new: `PgInterceptorStore`).
No migration data (previous packets were discarded by `NoopInterceptorStore`).

### 4.29 Chat-memory records (Path A — readable storage, default-on)

> **Upgrade A + C — always active.** This table is the authoritative persistent
> store for chat-memory entries produced by `memory_write` tool calls. It is
> populated unconditionally on every `memory_write` regardless of whether the
> `embedding` provider role is assigned. It is the primary source for scoring,
> reinforcement, success control, recipe creation, and skill creation. The key
> `chat_record_id` is a ULID generated by the memory-write path; it is also
> used to derive the canonical chunk subtree path
> `/memory/chat/<chat_record_id>` that the chunk system (§4.30) writes to. The
> `source_ref` column on this row stores that path so the chunk system can
> join back to the Path A row for scoring/reinforcement, and so the retention
> sweep can cascade chunk deletion when a Path A row is pruned (§4.21).

```sql
-- V025__memory_chat_records.sql
-- Path A: human-readable, structured chat-memory records.
-- Written unconditionally on every memory_write call.
-- The authoritative source for all non-retrieval use cases.
CREATE TABLE IF NOT EXISTS brassclaw_memory_chat_records (
    -- chat_record_id: ULID generated by the memory-write path.
    -- Also used to derive the chunk subtree path /memory/chat/<id> (§4.30).
    id               TEXT        NOT NULL PRIMARY KEY,  -- ULID (= chat_record_id)
    tenant_id        TEXT        NOT NULL,
    user_id          TEXT        NOT NULL,
    -- project_id: the project scope (nullable — not all memory is project-scoped).
    -- Matches brassclaw_memory_docs.project_id for consistency.
    project_id       TEXT,
    -- agent_id: the agent that wrote this memory (nullable — system-generated
    -- memories have no agent). Matches brassclaw_session_threads.agent_id.
    agent_id         TEXT,
    -- session_thread_id: the thread under which this memory was recorded.
    -- Soft reference (no FK) — memory records may outlive session threads.
    session_thread_id TEXT,
    -- run_id: the run that triggered the memory_write call.
    -- Soft reference (no FK) — same rationale as brassclaw_events.run_id.
    run_id           TEXT,
    -- kind: the memory kind string (e.g. 'observation', 'fact', 'preference').
    -- Open-ended (no CHECK) — new kinds may be added without a schema migration.
    kind             TEXT        NOT NULL DEFAULT 'observation',
    -- content: the human-readable memory text (the primary stored value).
    content          TEXT        NOT NULL,
    -- summary: optional short summary extracted from content.
    summary          TEXT,
    -- context: optional structured context (caller-supplied key/value pairs).
    context          JSONB       NOT NULL DEFAULT '{}',
    -- importance: optional numeric importance score (0.0–1.0).
    -- CHECK enforces the valid range; NUMERIC(5,4) allows 0.0000–9.9999 but
    -- the CHECK clamps to 0.0–1.0 as documented.
    importance       NUMERIC(5,4)
        CHECK (importance IS NULL OR (importance >= 0.0 AND importance <= 1.0)),
    -- tags: free-form text tags for filtering.
    tags             TEXT[]      NOT NULL DEFAULT '{}',
    -- source_ref (revision 17): canonical VFS path of the chunk set derived
    -- from this record (e.g. /memory/chat/<chat_record_id>). NULL when Path B
    -- has not yet run (no embedding role assigned, or the record predates the
    -- chunk-system wiring). Set by Path B (§4.30) after the chunk subtree is
    -- written. Indexed for the retention sweep's chunk-cascade lookup.
    source_ref       TEXT,
    -- forensic_packet_id: soft reference to brassclaw_forensic_packets(id).
    -- Populated retroactively by the memory-write path after PgInterceptorStore
    -- saves the packet. NULL when no forensic packet exists for this turn (e.g.
    -- interceptor was in noop mode when this memory was written, or the memory was
    -- written by a system path not associated with an agent-loop turn).
    -- Use this to navigate from a memory record to the full prompt that produced it.
    -- To find all memories produced by a given run, query by run_id; this column
    -- is the shortcut to the specific packet within a potentially multi-iteration run.
    forensic_packet_id TEXT,
    -- tsv: generated full-text search vector over content + summary.
    -- Auto-maintained on every INSERT/UPDATE (§4.17 rationale).
    -- Uses 'english' configuration — for multi-language deployments, the
    -- implementer may switch to 'simple' or make the configuration configurable
    -- via brassclaw_config (retention.memory_fts_language).
    tsv              tsvector    GENERATED ALWAYS AS (
                         to_tsvector('english',
                             coalesce(content, '') || ' ' || coalesce(summary, ''))
                     ) STORED,
    -- success_score / reinforcement_score: updated by the scoring pipeline.
    -- Both are 0.0–1.0 range (CHECK enforces).
    success_score    NUMERIC(5,4)
        CHECK (success_score IS NULL OR (success_score >= 0.0 AND success_score <= 1.0)),
    reinforcement    NUMERIC(5,4)
        CHECK (reinforcement IS NULL OR (reinforcement >= 0.0 AND reinforcement <= 1.0)),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_tenant_user_idx
    ON brassclaw_memory_chat_records (tenant_id, user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_thread_idx
    ON brassclaw_memory_chat_records (tenant_id, session_thread_id)
    WHERE session_thread_id IS NOT NULL;
-- Project-scoped lookup: "all memories for project X" — needed for
-- multi-project deployments where memories are project-scoped.
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_project_idx
    ON brassclaw_memory_chat_records (tenant_id, project_id, created_at DESC)
    WHERE project_id IS NOT NULL;
-- source_ref lookup: used by the retention sweep to cascade chunk deletion
-- when a Path A row is pruned (§4.21, §4.30 chunk-cascade invariant).
-- Partial index — only rows where Path B has run (source_ref IS NOT NULL).
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_source_ref_idx
    ON brassclaw_memory_chat_records (tenant_id, source_ref)
    WHERE source_ref IS NOT NULL;
-- forensic_packet_id lookup: find the memory record(s) produced by a given packet.
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_packet_idx
    ON brassclaw_memory_chat_records (forensic_packet_id)
    WHERE forensic_packet_id IS NOT NULL;
-- run_id lookup: used by link_chat_record(run_id, iteration, chat_record_id) UPDATE
-- and for turn-scoped memory retrieval; also used by the forensic-packet retention sweep
-- to null-out forensic_packet_id on linked memory records.
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_run_idx
    ON brassclaw_memory_chat_records (tenant_id, run_id, created_at DESC)
    WHERE run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_fts_idx
    ON brassclaw_memory_chat_records USING GIN (tsv);
CREATE INDEX IF NOT EXISTS brassclaw_memory_chat_records_tags_idx
    ON brassclaw_memory_chat_records USING GIN (tags);
CREATE TRIGGER brassclaw_memory_chat_records_updated_at
    BEFORE UPDATE ON brassclaw_memory_chat_records
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
```

New table — no migration data from existing stores. `PgChatMemoryRecordStore`
is the new store type added to the memory-write path (§5.3, Phase 4). The
`source_ref` column is populated by Path B (§4.30) after the chunk subtree is
written; it remains NULL when Path B is inactive (no `embedding`-role provider)
or when the record predates the chunk-system wiring (the
`backfill-embeddings` command, §8.1 step 10, populates it retroactively).

### 4.30 Path B — chunk embedding system reuse (file-less chunk creation, `source_ref` link, pgvector via VFS)

> **Upgrade B + C — optional, skipped when no `embedding`-role provider is active.**
> Revision 17 removed the standalone `brassclaw_memory_embeddings` V026 table.
> Path B now reuses the **existing `brassclaw_memory` chunk embedding system**
> — the same system that indexes workspace documents under `/memory/docs/*`.
> The chunk system already has: a word-overlap chunker (`chunk_document`,
> 800-word default, 15% overlap), an embedding-provider seam
> (`brassclaw_memory::EmbeddingProvider` trait), a chunk row layout (VFS
> entries under `<doc>/*.chunks/<index>` with `content` + `embedding` +
> `chunk_index` + `doc_relative_path` indexed keys), and a hybrid search
> request (`MemorySearchRequest` with RRF / weighted fusion of FTS + vector
> results). The chunk `embedding` indexed key is translated by
> `PostgresRootFilesystem` into a pgvector `vector`-typed column + HNSW index
> on the VFS backing table, so **pgvector IS the vector database for the chunk
> system** — there is no separate vector table.
>
> The existing system is currently **not wired** in the `memory_write` /
> `memory_search` tool dispatch path: `build_backend()` at
> `crates/brassclaw_host_runtime/src/first_party_tools/memory.rs:278-298`
> creates `ChunkingMemoryDocumentIndexer::new()` without calling
> `with_embedding_provider(...)`, and `dispatch_search()` at line 337-341
> forces `.with_vector(false)`. Revision 17 **activates** the system by wiring
> the embedding provider and flipping the vector flag when an `embedding`-role
> provider is active, and **extends** it with a file-less chunk creation path
> so chat-memory records can be chunked + embedded without persisting a parent
> document to the filesystem.

**No new SQL migration in this section.** The chunk system stores its rows in
the existing `brassclaw_root_filesystem` VFS backing table (§4.19) + sibling
index tables. The `vector` extension is created in V000 (§4.20); the
`source_ref` column on `brassclaw_memory_chat_records` is added in V025 (§4.29).
V026 is `brassclaw_forensic_packets` (§4.28 — the interceptor store), **not** an
embedding table. Path B (this section) adds no new SQL migration files.

#### 4.30.1 File-less chunk creation — `MemoryDocumentIndexer::index_content`

The existing `MemoryDocumentIndexer` trait
(`crates/brassclaw_memory/src/indexer.rs:22`) has only `reindex_document(path)`
and `reindex_document_with_audit_context(path, audit_context)`, both of which
read the document body from the repository via `read_document(path)`. This
requires a parent document file to exist on the VFS —
`replace_document_chunks_if_current()` returns `SkippedMissingDocument` if the
document is absent. Chat-memory records do NOT persist a parent document (the
authoritative content lives in `brassclaw_memory_chat_records.content`), so the
existing path cannot index them.

A new trait method is added:

```rust
#[async_trait]
pub trait MemoryDocumentIndexer: Send + Sync {
    async fn reindex_document(&self, path: &MemoryDocumentPath) -> Result<(), FilesystemError>;
    async fn reindex_document_with_audit_context(
        &self,
        path: &MemoryDocumentPath,
        audit_context: Option<&MemoryAuditContext>,
    ) -> Result<(), FilesystemError> {
        let _ = audit_context;
        self.reindex_document(path).await
    }

    /// File-less chunk creation (revision 17). Chunks + embeds `content`
    /// directly from an in-memory string, without requiring a parent
    /// document file to exist on the VFS. The chunks are written under the
    /// synthetic subtree `<source_ref>/*.chunks/<index>` with a synthetic
    /// `doc_relative_path` derived from `source_ref`.
    ///
    /// `scope` carries the tenant_id, user_id, agent_id, and project_id
    /// needed to construct the `MemoryDocumentPath` from `source_ref`.
    /// The caller (`PgChatMemoryRecordStore`) passes the same scope as
    /// the Path A row being indexed. `ChunkingMemoryDocumentIndexer` does
    /// NOT hold a scope at construction time — it is per-call here because
    /// different memory writes belong to different users/agents/projects.
    ///
    /// Use this for chat-memory records (source_ref = /memory/chat/<id>) and
    /// for transient document ingestion (source_ref = /memory/docs/<path>,
    /// content = document text — the document is chunked + embedded and
    /// never persisted to the filesystem).
    ///
    /// If a parent document DOES exist at `source_ref`, the existing
    /// `reindex_document` path is preferred (it reads the file and
    /// hash-guards against concurrent writes). This method is for the
    /// no-parent-document case.
    ///
    /// The `chat_record_id` (when supplied) is stored as an indexed key on
    /// each chunk row so the chunk system can join back to the Path A row.
    async fn index_content(
        &self,
        scope: &MemoryDocumentScope,
        source_ref: &str,
        content: &str,
        chat_record_id: Option<&str>,
    ) -> Result<(), FilesystemError>;
}
```

`ChunkingMemoryDocumentIndexer` implements `index_content` as follows:

1. **Derive a synthetic `MemoryDocumentPath`** from `scope` + `source_ref`.
   `scope` (tenant_id, user_id, agent_id, project_id) is passed directly as
   the method's first argument by the caller (`PgChatMemoryRecordStore`
   passes the scope from the Path A row). `source_ref` is a canonical VFS
   path (e.g. `/memory/chat/<chat_record_id>`); parse `relative_path` as the
   suffix after `/memory/` (e.g. `chat/<chat_record_id>`), then call
   `MemoryDocumentPath::new(scope, relative_path)` to produce the full path.
2. **Chunk the content** using the existing `chunk_document(content,
   chunk_config)` — same chunker, same defaults (800 words, 15% overlap).
3. **Embed each chunk** via `self.embedding_provider` (if wired). On embedding
   API error, degrade to text-only chunks (embedding=NULL) — the existing
   indexer degrade behaviour (`indexer.rs:200-228`) is preserved verbatim.
   The error is returned to the caller so the Path B write path can `warn!`
   log it (§7.4).
4. **Write chunk rows** via
   `replace_document_chunks_if_current(path, hash, chunks)`. The
   `expected_content_hash` is `content_sha256(content)` — since there is no
   parent document, the hash guard degenerates to "replace if no chunks exist
   yet OR the existing chunks were written for the same content hash". This
   keeps the operation idempotent: re-running `index_content` with the same
   content is a no-op; re-running with different content replaces the chunk
   set.
5. **Set `source_ref` on the Path A row.** After the chunk subtree is written,
   the caller (`PgChatMemoryRecordStore::write_path_b`) updates
   `brassclaw_memory_chat_records.source_ref` to the `source_ref` value. This
   is the cascade link for the retention sweep (§4.21).
6. **Store `chat_record_id` as an indexed key** on each chunk row (new indexed
   key `fs_keys::CHAT_RECORD_ID = "chat_record_id"`). This lets the chunk
   system join back to the Path A row without parsing the `doc_relative_path`.
   The existing `fs_keys` module (`repo/filesystem.rs:67-77`) gains a new
   constant `CHAT_RECORD_ID`.

**No parent document is ever persisted to the filesystem for chat-memory
records.** The authoritative content lives in
`brassclaw_memory_chat_records.content`; the chunk subtree is a derived index.
For transient document ingestion (`memory_write --kind=document` with a
`content` payload but no `target` file path), the document is chunked +
embedded from memory and never written to the VFS — the operator explicitly
opts out of persistence by omitting `target`.

#### 4.30.2 Chunk-cascade invariant

When a `brassclaw_memory_chat_records` row is pruned by the retention sweep
(§4.21), the sweep MUST also delete the corresponding chunk subtree under
`/memory/chat/<chat_record_id>/*.chunks/`. The sweep resolves the
`source_ref` column, lists all chunk records under the `<source_ref>/*.chunks/`
prefix, and deletes them. This maintains the invariant that every chunk row
with a `chat_record_id` indexed key has a corresponding Path A row.

The cascade is implemented in the retention sweep task (Phase 4), not via a
Postgres `ON DELETE CASCADE` FK (chunk rows live in the VFS backing table, not
in a dedicated chunk table, so a FK is not possible). The sweep MUST be
transactional with the Path A row deletion: delete the chunk subtree first,
then delete the Path A row. If the chunk deletion fails, the Path A deletion
MUST be rolled back (otherwise the chunk subtree becomes an orphan with no
owning record).

#### 4.30.3 Vector search retrieval — `memory_search` with vector enabled

The existing `dispatch_search()` at
`crates/brassclaw_host_runtime/src/first_party_tools/memory.rs:337-341`
currently forces `.with_vector(false)`. Revision 17 changes this to:

```rust
let request = MemorySearchRequest::new(query)
    .map_err(|_| input_error())?
    .with_limit(limit)
    .with_pre_fusion_limit(limit.max(20))
    .with_vector(services.embedding_active);
```

where `services.embedding_active` is a new boolean on `MemoryServices` that
`build_backend()` sets to `true` when an embedding provider was wired into
the indexer, and `false` otherwise (the current behaviour). When
`embedding_active` is `false`, the search degenerates to FTS-only (the
current behaviour) — no vector query is issued, no pgvector call is made.

When `embedding_active` is `true`, the search issues both an FTS query (over
the chunk `content` indexed key) and a vector query (over the chunk
`embedding` indexed key, translated by `PostgresRootFilesystem` to a pgvector
`<=>` cosine-distance query). The two result sets are fused via the existing
`FusionStrategy::Rrf` (Reciprocal Rank Fusion, the default) or
`FusionStrategy::WeightedScore` — see `crates/brassclaw_memory/src/search.rs`.

The vector query requires a query embedding. `dispatch_search()` calls
`services.embedding_provider.embed(query)` to produce it — where
`services.embedding_provider` is the `Option<Arc<dyn brassclaw_memory::EmbeddingProvider>>`
field on `MemoryServices` that `build_backend()` sets to `Some(adapter)` when
an embedding provider is wired, and `None` otherwise. When `embedding_active`
is `true`, this field is always `Some`; the `embed` call is guarded by the
same flag. On embedding API error, the search degrades to FTS-only (the
vector branch is skipped, a `debug!` log is emitted — `debug!` not `warn!`
because search is a hot path and a transient embedding outage should not spam
the operator).

#### 4.30.4 Dimension configuration

The chunk system's `embedding` indexed key is stored as a `vector(N)` column
in the VFS backing table, where `N` is the dimension of the wired embedding
provider. The dimension is resolved at composition startup from the
`embedding`-role provider's model metadata (via
`default_dimension_for_model` if the provider definition does not specify a
dimension). `PostgresRootFilesystem::ensure_index` is called with
`IndexKind::Vector { dim: N }` when the embedding provider is wired — this
matches the existing `ensure_search_indexes` path in
`repo/filesystem.rs:125-153`.

If the operator changes the embedding model to one with a different dimension,
the existing chunk rows (with the old dimension) become invalid. The
`backfill-embeddings` command (§8.1 step 10) handles this: it deletes the old
chunk subtree and re-indexes with the new dimension. The
`brassclaw_memory_chat_records.source_ref` column is preserved across the
re-index (it points to the same path; only the chunk contents change).

#### 4.30.5 Backfill contract

The backfill-embeddings command must satisfy this contract
(the three operations constitute the idempotency guarantee):

```text
get_chunks_without_embeddings  — list brassclaw_memory_chat_records rows whose
                                  source_ref IS NULL OR whose chunk subtree has
                                  VFS rows with embedding = NULL indexed key.
update_chunk_embedding         — for a given chunk row, set its embedding indexed
                                  key to the newly-computed vector value.
backfill_embeddings(batch_size) — iterate get_chunks_without_embeddings in
                                  batches, call embedding_provider.embed() on
                                  the chunk content, call update_chunk_embedding;
                                  safe to interrupt and resume.
```

Revision 17 reuses this contract verbatim. The `backfill-embeddings` CLI
command (§8.1 step 10) reads `brassclaw_memory_chat_records` rows whose
`source_ref` is NULL or whose chunk subtree has chunks with `embedding =
NULL`, reconstructs the `MemoryDocumentScope` from the row's
`(tenant_id, user_id, agent_id, project_id)` columns, calls
`indexer.index_content(scope, source_ref, content, chat_record_id)` for each,
and is idempotent. No V026 table is involved.

### 4.31 Event store guard replacement

The event store's `build_reborn_event_stores()` at
`crates/brassclaw_reborn_event_store/src/lib.rs:148` currently takes a
`profile: RebornProfile` parameter and has three `RebornProfile::Production`
branches:

1. **Line 154** — `InMemory` + `Production` → error `ProductionInMemoryDisabled`
2. **Line 166** — `Jsonl` + `Production` + no `accept_single_node_durable` → error `ProductionJsonlRequiresAcceptance`
3. **Line 195** — `Libsql` + `Production` → `validate_production_libsql_target()` (SSL check)

**Replacement design:** Remove the `profile: RebornProfile` parameter entirely.
The `RebornEventStoreConfig` variant itself is the guard — no profile string
needed:

- **`InMemory`**: Only constructable behind `#[cfg(test)]`. In serve mode,
  the caller never constructs this variant (the serve path always constructs
  `Postgres` or `Jsonl`). If somehow constructed in non-test code, the
  existing `InMemory` path returns the in-memory store without a profile check
  — the guard is that the caller must not construct it, enforced by the
  serve-path code only building `Postgres`/`Jsonl` configs. Optionally, add a
  `debug_assert!` that `cfg!(test)` is true if `InMemory` is reached.

- **`Jsonl`**: The `accept_single_node_durable: bool` flag already gates this.
  Remove the `profile == Production` check; require
  `accept_single_node_durable == true` unconditionally in serve mode (the
  caller sets it). The flag's name is self-documenting — no profile string
  needed.

- **`Libsql`**: Removed entirely by Phase 6. The variant is deleted from
  `RebornEventStoreConfig` and the match arm is removed. The
  `validate_production_libsql_target()` function is deleted.

- **`Postgres`**: No profile check needed — this is always durable.

**Event store's own `RebornProfile` enum** (lib.rs line 91: `LocalDev`, `Test`,
`Production`): Remove the `Production` variant. The enum collapses to
`{ LocalDev, Test }` or is removed entirely if no code references it after
the `build_reborn_event_stores()` signature change. The
`to_event_store_profile()` stub in `crates/brassclaw_reborn_composition/src/profile.rs`
returns `brassclaw_reborn_event_store::RebornProfile::LocalDev` (the
test/default variant) until the enum is removed.

**Call site update:** `factory.rs:2536` currently calls:
```rust
.with_reborn_event_store_config(profile.to_event_store_profile(), stores.event_store)
```
After the change, the `profile.to_event_store_profile()` argument is removed
(the parameter is gone from `build_reborn_event_stores()`), and the call
becomes:
```rust
.with_reborn_event_store_config(stores.event_store)
```
or equivalent (depending on how the wiring method signature changes).

> **Sequencing note for this call site:** `factory.rs:2536` is inside
> `build_production_shaped()`. Phase 11 (Path B Phase 3) removes the
> `Production | MigrationDryRun → build_production_shaped()` branch and deletes
> the function — which eliminates `factory.rs:2536` entirely. The call-site
> update above is therefore only relevant if Phase 11 lands **before**
> integrate-postgres.md Phase 4. If it lands after Phase 4 (the preferred
> sequencing — after Phases 1–6 have landed), the call site is already gone:
> Phase 4 rewrites event store wiring so that `RebornEventStoreConfig::Postgres
> { url }` is retired and stores are wired with `Arc<PgPool>` directly (see
> Phase 4 checklist, M7 note). In that case the only work left in this section
> is removing the `profile: RebornProfile` parameter from
> `build_reborn_event_stores()` in `lib.rs` and cleaning up the `Production`
> guard branches.

---

## 5. Crate Changes

### 5.1 `brassclaw_filesystem`

> **M6 — `PostgresRootFilesystem` already exists:** `brassclaw_filesystem`
> already contains a `PostgresRootFilesystem` type behind
> `#[cfg(feature = "postgres")]`. This does **not** need to be created from
> scratch. Phase 6 work here is to: remove the `postgres` optional feature gate
> (making `PostgresRootFilesystem` unconditional), remove `LibSqlRootFilesystem`
> and `InMemoryBackend`, and strip any `#[cfg(feature = "libsql")]` /
> `#[cfg(feature = "postgres")]` guards from the crate.

- The `postgres` and `libsql` feature gates are removed.
- `LibSqlRootFilesystem`, `LibSqlBackend`, `InMemoryBackend` are removed.
  `PostgresRootFilesystem` (already exists behind `#[cfg(feature = "postgres")]`) becomes
  the unconditional backend, backed by `brassclaw_root_filesystem` table (for any VFS
  path not yet promoted to a domain table).
- The `RootFilesystem` and `ScopedFilesystem` traits stay unchanged.
- Domain-specific Filesystem stores (`FilesystemRunStateStore`, etc.) are
  replaced one-by-one by Postgres-native store types. See §5.3 below.

### 5.2 `brassclaw_hooks_postgres` and `brassclaw_hooks_libsql`

- `brassclaw_hooks_libsql` is **deleted entirely** — the crate is removed from
  the workspace.
- `brassclaw_hooks_postgres` loses its `postgres` optional feature gate.
  `deadpool-postgres` and `tokio-postgres` become mandatory deps (no longer
  optional). The crate is renamed to `brassclaw_hooks_pg` for clarity.
- `brassclaw_hooks_parity` (cross-backend parity tests) **must not be deleted
  outright** — see note below.

> **H3 — cfg guard stripping required:** `brassclaw_hooks_postgres` currently
> gates its public symbols at the *module level* in `lib.rs`:
> `#[cfg(feature = "postgres")] mod backend;` / `mod hashing;` / `mod schema;`
> (plus the re-exports). The files themselves are NOT wrapped in
> `#[cfg(feature = "postgres")]` internally. Removing the `postgres` *optional
> feature* from `Cargo.toml` is **not sufficient** — the module-level `#[cfg]`
> declarations in `lib.rs` AND the feature-gated re-exports must also be stripped
> so the modules are compiled unconditionally. An implementer who only changes
> `Cargo.toml` without stripping the `lib.rs` cfg attributes will produce a crate
> that exposes no public symbols. The Phase 5 checklist item for this rename
> explicitly covers stripping the `lib.rs` cfg declarations.

> **`brassclaw_hooks_parity` — port tests, do not simply delete:** The parity
> crate contains production-quality adversarial tests that exercise
> `PostgresPredicateStateBackend` under concurrent load: a deterministic
> cross-backend parity matrix (`tests/parity_matrix.rs`) and an adversarial suite
> covering N concurrent writers, cross-host replay, LRU eviction races, per-key
> cap under flood, and clock-skew (`tests/multi_host_adversarial.rs`). These
> prove correctness invariants that simple unit tests cannot cover. **The
> adversarial and parity tests must be ported into `brassclaw_hooks_pg`** (e.g. as
> a `tests/` directory in the renamed crate) before `brassclaw_hooks_parity` is
> deleted. Deleting the crate without porting the test coverage removes the only
> regression gate for concurrent-write correctness in the hooks backend. Phase 5
> checklist must include a port-and-verify step before the deletion step.

### 5.3 Store crates: replaced implementations

Each of these stores gains a `brassclaw_*_pg` module implementing the same
public trait against Postgres via `brassclaw_pg::PgPool`:

Verified source locations for every old type (via `grep -rn "pub struct Filesystem.*Store"`):

| Old type | Source crate (verified) | New type | New crate |
|---|---|---|---|
| `FilesystemRunStateStore` | `brassclaw_run_state` (lib.rs:555) | `PgRunStateStore` | `brassclaw_run_state` |
| `FilesystemApprovalRequestStore` | `brassclaw_run_state` (lib.rs:787) | `PgApprovalRequestStore` | `brassclaw_approvals` ⬡ |
| `FilesystemTurnStateStore` | `brassclaw_turns` (filesystem_store.rs:80) | `PgTurnStateStore` | `brassclaw_turns` |
| `FilesystemCheckpointStateStore` | `brassclaw_loop_support` (filesystem_checkpoint_state.rs:44) | `PgCheckpointStateStore` | `brassclaw_loop_support` ⬡ |
| `InMemoryCheckpointStateStore` | `brassclaw_turns` (checkpoint_state.rs:216) | `PgCheckpointStateStore` (shared) | `brassclaw_loop_support` |
| `InMemoryLoopCheckpointStore` | `brassclaw_turns` (checkpoint_state.rs:221) | `PgLoopCheckpointStore` | `brassclaw_turns` |
| `FilesystemCapabilityLeaseStore` | `brassclaw_authorization` (lib.rs:417) | `PgCapabilityLeaseStore` | `brassclaw_authorization` |
| `FilesystemSessionThreadService` | `brassclaw_threads` (filesystem_service.rs:150) | `PgSessionThreadService` | `brassclaw_threads` — implements `SessionThreadService` trait (service.rs:21); the trait has **16 required methods** (confirm at service.rs before implementation to ensure full coverage; not just 1-2 obvious methods) |
| `FilesystemResourceGovernorStore` | `brassclaw_resources` (filesystem_store.rs:68) | `PgResourceGovernorStore` | `brassclaw_resources` |
| `FilesystemProcessStore` | `brassclaw_processes` (filesystem_store.rs:54) | `PgProcessStore` | `brassclaw_processes` |
| `FilesystemProcessResultStore` | `brassclaw_processes` (filesystem_store.rs:415) | `PgProcessResultStore` | `brassclaw_processes` |
| `FilesystemExtensionInstallationStore` | `brassclaw_reborn_composition` (extension_installation_store.rs:14, `pub(crate)`) | `PgExtensionInstallationStore` | `brassclaw_extensions` ⬡ |
| `LibSqlTriggerRepository` | `brassclaw_triggers` (libsql.rs:74) | `PostgresTriggerRepository` (already exists at postgres.rs:63) | `brassclaw_triggers` — promote to unconditional; remove libsql feature gate |
| `RebornLibSqlLocalTriggerAccessStore` | `brassclaw_reborn` (local_trigger_access.rs:116) | `PgLocalTriggerAccessStore` | `brassclaw_reborn` (local-dev only scope preserved) |
| `FilesystemConversationStateStore` | `brassclaw_conversations` (filesystem_store.rs:77) | `PgConversationStateStore` | `brassclaw_conversations` |
| `FilesystemOutboundStateStore` | `brassclaw_outbound` (filesystem_store.rs:94) | `PgOutboundStateStore` | `brassclaw_outbound` (implements both `OutboundStateStore` and `CommunicationPreferenceRepository`) |
| `FilesystemSubagentGoalStore` | `brassclaw_reborn` (subagent/goal_store.rs:79, `#[cfg(feature = "filesystem-goal-store")]`) | `PgSubagentGoalStore` | `brassclaw_reborn` (remove `filesystem-goal-store` feature gate) |
| `FilesystemDurableEventLog` | `brassclaw_reborn_event_store` (verified) | `PgDurableEventLog` | `brassclaw_reborn_event_store` |
| `FilesystemDurableAuditLog` | `brassclaw_reborn_event_store` (verified) | `PgDurableAuditLog` | `brassclaw_reborn_event_store` |
| `DbTokenSettingsStore` | `brassclaw_reborn_composition` | `PgTokenSettingsStore` | `brassclaw_reborn_composition` |
| `SqliteSafetyConfigStore` | `brassclaw_product_workflow` | `PgSafetyConfigStore` | `brassclaw_product_workflow` |
| `MemoryDocLibSqlStore` | `brassclaw_reborn_composition` | `PgMemoryDocStore` | `brassclaw_reborn_composition` |
| `FilesystemAuthProductServices` | `brassclaw_reborn_composition` (`product_auth_durable.rs`) | `PgAuthProductServices` | `brassclaw_reborn_composition` |
| `FilesystemCredentialBroker` | `brassclaw_secrets` | `PgCredentialBroker` | `brassclaw_secrets` |
| `FilesystemSecretStore` | `brassclaw_secrets` | `PgSecretStore` | `brassclaw_secrets` |
| `FilesystemBudgetGateStore` | `brassclaw_resources` (`/resources/budget-gates.json`) | `PgBudgetGateStore` | `brassclaw_resources` |
| `FilesystemRebornIdentityStore` | `brassclaw_reborn_identity` (`filesystem_store.rs`, wired in composition `factory.rs`) | `PgRebornIdentityStore` | `brassclaw_reborn_identity` |
| `NoopInterceptorStore` | `brassclaw_interceptor` (store.rs) | `PgInterceptorStore` | `brassclaw_interceptor` (replaces `NoopInterceptorStore` in production composition; noop retained for tests) |
| *(new — Upgrade A)* in-process `memory_write` path (chat memory, no prior durable store) | — | `PgChatMemoryRecordStore` | `brassclaw_reborn_composition` (or a new `brassclaw_memory` crate — see Phase 4) |
| *(re-wired — Upgrade B/C, revision 17)* in-process chunk + embedding path (reuses existing `brassclaw_memory` chunk system; no new store type) | — | `ChunkingMemoryDocumentIndexer` (existing, re-wired with `with_embedding_provider`) + `EmbeddingRoleAdapter` (new, in `brassclaw_reborn_composition`) | `brassclaw_reborn_composition` (wiring layer) + `brassclaw_memory` (trait extension — `index_content` method) |

**⬡ Relocation notes:**
- `PgApprovalRequestStore` moves to `brassclaw_approvals` (the dedicated crate where the
  trait and domain logic belong); `brassclaw_run_state` will delegate to it.
- `PgCheckpointStateStore` moves to `brassclaw_loop_support` (where `FilesystemCheckpointStateStore`
  actually lives, not `brassclaw_turns` as the original plan stated).
- `PgExtensionInstallationStore` moves from `brassclaw_reborn_composition` (where the
  `pub(crate)` filesystem impl lives) into `brassclaw_extensions` (where the trait and
  `InMemoryExtensionInstallationStore` already live), making it properly crate-public.

> **`FilesystemCredentialBroker` implements two traits:** `CredentialAccountStore`
> AND `CredentialSessionStore` (in `brassclaw_secrets/src/filesystem_store.rs`).
> `PgCredentialBroker` must implement both. Implementing only one trait leaves the
> other store interface unresolved and will fail at the composition wiring site.

> **`FilesystemRebornIdentityStore` location clarification:** The filesystem store is
> in `crates/brassclaw_reborn_identity/src/filesystem_store.rs`. There is no
> `factory.rs` in `brassclaw_reborn_identity`; the wiring is in
> `brassclaw_reborn_composition/src/factory.rs` (the composition factory). `PgRebornIdentityStore`
> goes into `brassclaw_reborn_identity`; the factory wiring change is in
> `brassclaw_reborn_composition/src/factory.rs`. These are two separate files.

> **M2 — recipe_store / reduction_rules_store — confirmed no direct libSQL:**
> `recipe_store.rs` and `reduction_rules_store.rs` in `brassclaw_reborn_composition`
> have been audited and contain **no direct libSQL queries**. Both delegate to
> `Arc<dyn Store>` — the concrete store implementation is injected at factory time.
> No porting of these files is needed. The sole requirement is that the store
> injected at `factory.rs` construction time is the Postgres variant
> (`PgMemoryDocStore`) rather than the libSQL variant. This is handled by the
> Phase 4 `PgMemoryDocStore` item and the Phase 5 factory rewrite.

The `In-Memory*` variants are kept behind `#[cfg(test)]` only, used in
unit tests that do not need a live database.

### 5.4 `brassclaw_reborn_config`

`brassclaw_reborn_config` remains a **pure, synchronous, no-workspace-deps boundary
crate** — the `reborn_dependency_boundaries.rs` architecture test is not changed.
DB access must not be added here.

- `config_file.rs` — `RebornConfigFile` is retained as a **parse/serialize type only**
  for the long-term serve path. **`load()` is NOT removed in Phase 8** — it is retained
  behind the `migrate-from-libsql` feature flag, because two upgrade-path call sites
  depend on reading `config.toml` from disk at a point when `brassclaw_config` rows do
  not yet exist:
  - **§4.4 `rewrap` tenant-resolution step 2**: resolves `boot_tenant` from
    `config.toml` during manual pre-serve upgrade invocation.
  - **§8.1 step 3**: parses `config.toml` to migrate it into `brassclaw_config` rows.
  At that point `db_config::load_config_snapshot` cannot substitute — it reads from
  `brassclaw_config` rows that are still being written. `load()` removal lands in the
  **next release**, together with the `migrate-from-libsql` feature removal (see §9.1
  and Phase 8 checklist). Runtime serve-path callers are replaced by
  `db_config::load_config_snapshot` in Phase 2 as planned.
- `home.rs` — `RebornHome::config_file_path()`, `providers_file_path()`,
  `sempai_provider_file_path()` are removed. `path()` is kept.
- `secrets_guard.rs` — Kept unchanged.
- `Cargo.toml` — Remove `toml_edit`, `fs4`, `tempfile`. **Keep `toml`** (required
  by `config export` and `config show-all` serialization in §6.3). Keep `serde`.
  Do **not** add `deadpool-postgres` here.

### 5.5 `brassclaw_reborn_composition`

- **New file: `db_config.rs`** — DB-backed config read/write. This is where
  the DB access lives (not in `brassclaw_reborn_config`). Public API:
  ```rust
  pub async fn load_config_snapshot(pool: &PgPool, tenant_id: &str) -> Result<RebornConfigFile>;
  pub async fn save_config_key(
      pool: &PgPool,
      tenant_id: &str,
      key: &str,
      value: &str,
      caller: ConfigWriteContext,
  ) -> Result<()>;
  ```
  where `ConfigWriteContext` is:
  ```rust
  pub enum ConfigWriteContext { Operator, AgentSession }
  ```
  `save_config_key` enforces two unconditional guards before any DB write:
  1. **Inline-secret guard (SECURITY):** calls
     `brassclaw_reborn_config::secrets_guard::reject_inline_secret(value)`
     and returns `ConfigError::InlineSecretForbidden { key }` if the value
     looks like a literal secret (API key prefix, JWT, long hex). This guard
     applies regardless of `ConfigWriteContext` — no operator or agent session
     may store a raw secret value in `brassclaw_config`. The existing
     `RebornConfigFile::parse_text()` (config_file.rs:639) and `llm_catalog.rs`
     (lines 315, 409, 425, 438) already enforce this invariant on the TOML path;
     the DB path must enforce it identically. Failure to call this guard allows
     an operator to accidentally write `"sk-abc123"` as the value for
     `llm.default.api_key_env`, storing a literal API key in the config table
     and defeating the env-only security model.
  2. **`_env` suffix gate:** rejects any `key` ending in `_env` when
     `caller == ConfigWriteContext::AgentSession`, returning
     `ConfigError::EnvKeyWriteForbidden { key }` (see §1c security note on
     agent write-gate). This is structural: no caller that holds only a pool
     handle and a session context can reroute which env variable the serve
     process reads for auth or identity.
  Assembles a `RebornConfigFile` from rows and hands it to the composition
  layer.
- `provider_repo.rs` — `ProviderRepo` rewritten to read/write `brassclaw_llm_providers`
  instead of `providers.json`.
- `llm_config_service.rs` — Updated to call `db_config::load_config_snapshot`
  and the DB-backed `ProviderRepo`; file paths removed.
- `llm_key_store.rs` — `LlmKeyStore` already wraps the secret store; unchanged.
- `llm_catalog.rs` — `resolve_against_registry` now loads providers from
  `brassclaw_llm_providers` at start instead of reading `providers.json`.
- `factory.rs` — Simplified dramatically:
  - The libSQL bundle (`LocalDevRootFilesystemBundle`, `libsql::Database` arc)
    is removed.
  - `build_local_dev_root_filesystem` is removed.
  - All `#[cfg(feature = "libsql")]` / `#[cfg(feature = "postgres")]` guards
    are removed — there is only one path now.
  - The Postgres pool is built from `brassclaw_embedded_postgres::ManagedPostgres`
    or `BRASSCLAW_PG_URL`, then passed to every store constructor.
  - `RebornLocalRuntimeServices` loses the `identity_substrate_db` and all
    libSQL-specific fields.
  - Pool drop happens before `managed_pg.shutdown().await` is called.
- `hooks/` — Any libSQL hook backend wiring is removed.
- `Cargo.toml` — **`brassclaw_reborn_composition/Cargo.toml` currently defaults to
  `libsql`, not `postgres`** (H4). Phase 6 must update this crate's own `[features]`
  `default` list to remove `libsql` and ensure `postgres` (or no feature gate) is the
  only active path. Updating only the workspace root `Cargo.toml` is not sufficient —
  this crate's own `Cargo.toml` defaults must be changed here.

---

## 6. Onboarding / First-Run Wizard

The current `config init` command (which writes `config.toml` + `providers.json`)
is replaced by an interactive first-run wizard that writes to Postgres.

### 6.1 Trigger

First-run is detected by querying `brassclaw_config` for the tenant's
`boot.initialized = true` key. If absent and stdin is a TTY, the wizard runs
automatically the first time `brassclaw serve` or `brassclaw run` is called.

**Non-interactive guard:** If `boot.initialized` is absent and stdin is **not**
a TTY (e.g. a systemd service), `brassclaw serve` must **not** launch the
interactive wizard — it must fail immediately with a clear, non-zero exit:

```
brassclaw: first-run setup required. Run 'brassclaw config init' before starting the service.
```

This prevents the Restart=on-failure loop that would otherwise occur when the
interactive wizard hangs with no TTY.

### 6.2 Wizard steps (CLI, interactive, all skippable with `--yes`)

```
┌─ BrassClaw First-Run Setup ───────────────────────────────────────┐
│                                                                    │
│  Step 1/5  LLM Provider                                            │
│    Choose a provider: [openai / anthropic / ollama / custom / skip]│
│    Model [gpt-4o-mini]:                                            │
│    API key env var name [OPENAI_API_KEY]:                          │
│    (Only the env var NAME is stored here.                          │
│     The value is read from the environment at runtime —            │
│     set it in the systemd unit or your shell profile.)             │
│                                                                    │
│  Step 2/5  WebUI Access                                            │
│    Bearer token env var name [BRASSCLAW_REBORN_WEBUI_TOKEN]:       │
│    (Only the env var NAME is stored. Set the value in the          │
│     secrets file — see §7.)                                        │
│    WebUI user-id env var name [BRASSCLAW_REBORN_WEBUI_USER_ID]:    │
│    (brassclaw serve hard-errors if this env var is unset at start) │
│                                                                    │
│  Step 3/5  Identity                                                │
│    Tenant ID [default]:                                            │
│    Default owner ID [admin]:                                       │
│    (Stored in brassclaw_config as identity.default_owner —        │
│     this is a CONFIG value used for new session defaults.          │
│     It is separate from the BRASSCLAW_REBORN_WEBUI_USER_ID env    │
│     var set in Step 2, which is the identity asserted for bearer   │
│     auth at serve time. They should match in single-user setups;   │
│     the wizard warns if they differ.)                              │
│                                                                    │
│  Step 4/5  Budget                                                  │
│    Daily user budget in USD [5.00]:                                │
│    (0 = unlimited)                                                 │
│                                                                    │
│  Step 5/5  SSO (optional — skip if using bearer-token auth only)   │
│    WebUI base URL [skip]:                                          │
│    Allowed email domains (comma-separated) [skip]:                 │
│    (Non-secrets: stored in brassclaw_config, no env var needed.)   │
│                                                                    │
│  Writing to PostgreSQL...  ✓                                       │
│  Run `brassclaw serve` to start.                                   │
└────────────────────────────────────────────────────────────────────┘
```

The wizard writes each answer as a `brassclaw_config` row, then sets
`boot.initialized = true`. It never writes a file. API key values are
always read at runtime from the env var named in the config (env-only by
security policy, consistent with §1b–§1c).

### 6.2a Provider-config UI — three-role model (Upgrade B)

The provider-configuration UI (WebUI v2 provider settings panel) exposes three
action buttons per provider, laid out in the following order:

```
[use as kohai]  [use as sempai]  [use for embedding]
```

The **"use for embedding"** button (third, right-most) is the mechanism by which
an operator assigns the `embedding` role to a provider. The existing "use as
kohai" and "use as sempai" buttons are unchanged. All three buttons follow the
same single-assignment semantics: activating a role on one provider automatically
removes that role from the provider that previously held it.

**Config keys stored in `brassclaw_config` for roles:**

| Role | Config key written on button press | Cleared on | Notes |
|---|---|---|---|
| `kohai` | `llm.kohai.provider_id`, `llm.kohai.model` | kohai role assigned to another provider | Existing behaviour — not changed |
| `sempai` | `llm.sempai.provider_id`, `llm.sempai.model` | sempai role assigned to another provider | Existing behaviour — not changed |
| `embedding` | `embedding.provider_id`, `embedding.model` | embedding role assigned to another provider | **New — Upgrade B** |

**`embedding` role UI behaviour:**
- The "use for embedding" button is always shown next to the other two role
  buttons in the provider config panel.
- Pressing it writes `embedding.provider_id = <provider_id>` and
  `embedding.model = <selected_model>` to `brassclaw_config` and marks the
  provider as holding the `embedding` role.
- The button is visually indicated as "active" (e.g. highlighted) when the
  current provider already holds the `embedding` role.
- Pressing it on a provider that already holds the role is a no-op.
- Pressing it on a different provider clears the previous `embedding.*` config
  keys and writes new ones pointing at the new provider.
- Removing or deactivating a provider that holds the `embedding` role silently
  clears `embedding.provider_id` and `embedding.model` — Path B of `memory_write`
  will then be silently skipped until the role is re-assigned. The change takes
  effect on the next `brassclaw serve` restart (the embedding provider is
  resolved at composition startup; a running serve process keeps the
  previously-resolved provider until restart — see "activation wiring" below).

**Activation logic for memory embedding (settings layer):**
Memory embedding (Path B) is enabled if and only if:
1. A provider has been assigned the `embedding` role via the "use for embedding"
   button (i.e. `embedding.provider_id` is set in `brassclaw_config`), **AND**
2. That provider is currently active (not deleted or deactivated).

No separate settings toggle exists for this feature. The role assignment IS the
activation switch.

**Activation wiring (revision 17 — `build_backend()` re-wiring):** The
activation is realised at composition startup by `build_backend()` in
`crates/brassclaw_host_runtime/src/first_party_tools/memory.rs:278-298`. The
current code creates `ChunkingMemoryDocumentIndexer::new()` without calling
`with_embedding_provider(...)`, and `dispatch_search()` forces
`.with_vector(false)`. Revision 17 changes `build_backend()` to:

1. Resolve `embedding.provider_id` + `embedding.model` from `brassclaw_config`
   at composition startup (alongside `llm.kohai.*` / `llm.sempai.*` resolution).
2. If the `embedding`-role provider is active, construct an
   `EmbeddingRoleAdapter` (§3) that calls the provider's embedding endpoint,
   wraps it in `CachedEmbeddingProvider`, and returns an
   `Arc<dyn brassclaw_memory::EmbeddingProvider>`.
3. Call `ChunkingMemoryDocumentIndexer::with_embedding_provider(...)` and
   `RepositoryMemoryBackend::with_embedding_provider(...)` with the adapter.
4. Set `services.embedding_active = true` so `dispatch_search()` issues
   `.with_vector(true)` and produces a query embedding via
   `services.embedding_provider.embed(query)`.

If the `embedding`-role provider is NOT active (no `embedding.provider_id`, or
the provider is deleted/deactivated), `build_backend()` skips steps 2-4 — the
indexer is created without an embedding provider (the current behaviour) and
`services.embedding_active = false`. Path B of `memory_write` is silently
skipped (the indexer's `embedding_provider` is `None`, so `index_content`
writes text-only chunks with `embedding = NULL`); `PgChatMemoryRecordStore`
(Path A) is always active. Search degenerates to FTS-only (the current
behaviour).

**Live re-assignment limitation.** The embedding provider is resolved at
composition startup. If the operator presses "use for embedding" on a different
provider while `brassclaw serve` is running, the running process keeps the
previously-resolved provider until the next restart. This matches the existing
behaviour for `llm.kohai.*` / `llm.sempai.*` (which also take effect on
restart, not mid-session). The UI should display a "restart required" hint
when the `embedding` role is re-assigned.

### 6.3 `brassclaw config` subcommands (replaces file editing)

```
brassclaw config get <key>
brassclaw config set <key> <value>
brassclaw config list [--section <section>]
brassclaw config unset <key>
brassclaw config show-all          # prints all config as TOML for inspection
brassclaw config export > config.toml   # export to file for backup
brassclaw config import < config.toml  # import from file
```

`config show-all` renders the DB rows back into the `RebornConfigFile` shape,
making existing operator documentation still applicable for reference.

### 6.4 CLI command Postgres lifecycle

Every DB-touching CLI command — `config init`, `secrets rewrap`,
`config set/get/list/unset/show-all/export/import`, `maintenance prune-old-data`
— follows this lifecycle:

1. Start embedded Postgres **or connect to an already-running instance.** Uses
   the §2.2 orphaned-server detection: check `postmaster.pid` PID liveness
   (kill -0). If a live postmaster is found, reuse it — do not start a second
   instance. Record whether this command started PG or merely connected to an
   existing one.
2. Run `brassclaw_pg::migrations::run_migrations` — **idempotent** (all DDL
   uses `CREATE TABLE IF NOT EXISTS`; the §3 history-reconciliation bootstrap
   ensures pre-existing `hooks_*` and legacy tables don't trip refinery).
3. Perform the operation.
4. **Conditional shutdown:** shut down embedded PG **only if this command
   started it** (step 1 found no live postmaster). If a running PG was detected
   and reused (e.g. `brassclaw serve` is already running and owns the embedded
   PG), leave it running — shutting it down would crash the live server. For
   external PG (`BRASSCLAW_PG_URL`), always release the pool connection (no
   process to shut down).

Only `brassclaw serve` keeps Postgres running for the entire process lifetime
without ever triggering the conditional shutdown.

**This is why `secrets rewrap` can write `brassclaw_secrets_master` before
`brassclaw serve` has ever started**, and why `§8.1 step 6` can rely on that
row already existing at serve time: `rewrap` ran the schema first in step 2.
It is also why `brassclaw config init` in `§7.1 step 1` starts embedded PG
automatically and runs migrations before writing `brassclaw_config`.

### 6.5 `brassclaw config init --yes` flag mapping

The `--yes` flag bypasses all interactive prompts and applies the flag
defaults. Each wizard step maps to one or more flags:

| Wizard step | Flag(s) | Default |
|---|---|---|
| Step 1 — LLM provider | `--provider <id>` or `--no-llm` | required; use `--no-llm` to skip LLM setup |
| Step 1 — LLM model | `--model <name>` | required when `--provider` is set |
| Step 1 — API key env var | `--api-key-env <VAR>` | Provider-specific default: `openai` → `OPENAI_API_KEY`; `anthropic` → `ANTHROPIC_API_KEY`; `ollama` → *(none, no key needed)*; any other provider → required |
| Step 2 — WebUI token env var | `--webui-token-env <VAR>` | `BRASSCLAW_REBORN_WEBUI_TOKEN` |
| Step 2 — WebUI user-id env var | `--webui-user-id-env <VAR>` | `BRASSCLAW_REBORN_WEBUI_USER_ID` |
| Step 3 — Tenant ID | `--tenant <id>` | `default` |
| Step 3 — Default owner ID | `--owner <id>` | `admin` |
| Step 4 — Daily budget (USD) | `--budget-usd <n>` | `5.00` |
| Step 5 — WebUI base URL | `--webui-base-url <url>` | *(skipped)* |
| Step 5 — Allowed email domains | `--webui-allowed-domains <list>` | *(skipped)* |

When `--yes` is given and a required flag is omitted, `config init` exits with
a clear error (no interactive fallback). Running `config init --yes` with all
required flags is idempotent (upsert behaviour) — safe to re-run.

---

## 7. Systemd Service File

> **This is a template.** Operators must complete the `EnvironmentFile=` before
> starting the service. See the sequences below for fresh-install vs upgrade.
>
> **File ownership rules:**
> - `secrets.env` is read by **systemd** (as root) — `root:root 0600` is correct.
> - `master.key` (the Argon2id passphrase) is opened by the **service process**
>   (`User=brassclaw`) at per-boot unwrap time — it must be `brassclaw:brassclaw 0600`
>   (or `root:brassclaw 0640`). If the file is `root:root 0600` the service gets
>   `EACCES` and boot fails. See C2 fix in the setup sequences below.
> - The embedded PG data directory (`$REBORN_HOME/postgres/data/`) is created by
>   `initdb` which hard-refuses to run as root. All setup commands that touch the
>   data dir or `master.key` must run as `brassclaw`, not as root.
>
> The env var names in `secrets.env` (e.g. `OPENAI_API_KEY=sk-...`) must match the
> `api_key_env` values set in `brassclaw_config` during `config init`. A hardcoded
> `OPENAI_API_KEY` line is correct only when the operator kept the default name.

### 7.0 Prerequisites (fresh host)

Before running §7.1 or §7.2, ensure the service user, directories, and binary
are in place. On a fresh Debian/Ubuntu-family host:

```bash
# 1. Create the service user (no home-dir login, no shell):
useradd -r -d /var/lib/brassclaw -s /usr/sbin/nologin brassclaw  # Debian/Ubuntu
# On RHEL/Fedora, use: useradd -r -d /var/lib/brassclaw -s /sbin/nologin brassclaw

# 2. Create the required directories:
install -d -m 0750 -o brassclaw -g brassclaw /var/lib/brassclaw
install -d -m 0750 -o root      -g root      /etc/brassclaw
install -d -m 0755 -o root      -g root      /opt/brassclaw

# 3. Install the binary:
install -m 0755 ./target/release/brassclaw /usr/local/bin/brassclaw
```

The Phase 9 operator guide must include these steps.

### 7.1 Fresh-install sequence (no prior BrassClaw state)

```bash
# All commands that touch the data dir or master.key MUST run as the service user.
# initdb refuses root; master.key must be brassclaw-readable at per-boot unwrap.

# 1. Write initial config as the service user.
#    config init starts embedded PG, runs schema migrations (idempotent — see §6.4),
#    then writes brassclaw_config rows and sets boot.initialized = true:
sudo -u brassclaw brassclaw config init --yes \
    --provider openai --model gpt-4o-mini \
    --api-key-env OPENAI_API_KEY \
    --owner admin --tenant default \
    --budget-usd 5.00

# 2. (Optional — passphrase ceremony only) Wrap master key as the service user
#    (so master.key → brassclaw:brassclaw 0600).
#    Skip this step if you want raw-key-on-disk ceremony (the default).
#    --tenant must match the --tenant passed to config init above (§4.4 tenant-
#    resolution rule: explicit flag beats config.toml lookup, avoids any ambiguity):
#
#    IF you want passphrase-wrapped ceremony:
sudo -u brassclaw brassclaw secrets rewrap \
    --tenant default \
    --strategy passphrase-file=/var/lib/brassclaw/master.key
#    IF you want raw-key-on-disk ceremony: skip step 2 entirely.

# 3. Populate secrets.env — read by systemd as root, so root:root 0600 is correct:
install -m 0600 /dev/null /etc/brassclaw/secrets.env
# BRASSCLAW_SECRETS_PASSPHRASE_FILE: ONLY add this line if you ran step 2 (rewrap).
# Operators using raw-key-on-disk ceremony (step 2 skipped) must OMIT this line
# to avoid spurious boot warnings (see §4.4 ceremony-selector). An empty value is
# also treated as absent.
# echo "BRASSCLAW_SECRETS_PASSPHRASE_FILE=/var/lib/brassclaw/master.key" >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_TOKEN=your-bearer-token"                  >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_USER_ID=admin"                            >> /etc/brassclaw/secrets.env
echo "OPENAI_API_KEY=sk-..."                                            >> /etc/brassclaw/secrets.env

# 4. Start the service:
systemctl start brassclaw
```

### 7.2 Upgrade-from-file/libSQL sequence (existing BrassClaw installation)

> **Do NOT run `brassclaw config init`** on an upgrade — §8.1 step 3 migrates
> `config.toml` and step 7 migrates `reborn-local-dev.db` automatically on first
> serve. Running `config init --yes` would clobber the migrated config with
> defaults.
>
> `rewrap` starts embedded PG and runs schema migrations (§6.4), so
> `brassclaw_secrets_master` exists before `brassclaw serve` ever starts.
> §8.1 step 6 therefore finds the row created by `rewrap` and does not exit
> non-zero.
>
> **Existing persisted secrets:** any OAuth tokens or credential-broker secrets
> stored in the old libSQL DB are encrypted under the existing master key at
> `.reborn-local-dev-secrets-master-key`. `rewrap` reads that file (see the
> key-source rule in §4.4) and wraps *the same key* — so migrated secrets remain
> decryptable after §8.1 step 7 migrates the rows to PG. `rewrap` then zeroes
> and deletes the old file, preventing §8.1 step 6 from aborting on first serve.

```bash
# 1. (Optional — passphrase ceremony only) Wrap the master key as the service user.
#    rewrap reads .reborn-local-dev-secrets-master-key (preserving decryptability
#    of migrated persisted secrets), runs schema migrations, writes a
#    brassclaw_secrets_master row, then zeroes/deletes the old key file.
#    Skip this step if you want raw-key-on-disk ceremony (the default).
#    §8.1 step 6 will auto-migrate the raw key file at first serve if step 1 is skipped.
#
#    IF you want passphrase-wrapped ceremony:
#    IMPORTANT: --tenant must match identity.tenant in config.toml (§4.4
#    tenant-resolution rule). Passing --tenant explicitly avoids the risk of
#    rewrap defaulting to "default" when config.toml has a different value.
#    config.toml uses TOML section syntax ([identity] + tenant = "..."), not
#    dot-notation, so grep for the key inside the section:
#    grep tenant $BRASSCLAW_REBORN_HOME/config.toml
sudo -u brassclaw brassclaw secrets rewrap \
    --tenant <identity.tenant from config.toml> \
    --strategy passphrase-file=/var/lib/brassclaw/master.key
#    IF you want raw-key-on-disk ceremony: skip step 1 entirely.

# 2. Populate secrets.env:
install -m 0600 /dev/null /etc/brassclaw/secrets.env
# BRASSCLAW_SECRETS_PASSPHRASE_FILE: ONLY add this line if you ran step 1 (rewrap).
# Operators using raw-key-on-disk ceremony (step 1 skipped) must OMIT this line
# to avoid spurious boot warnings (see §4.4 ceremony-selector).
# echo "BRASSCLAW_SECRETS_PASSPHRASE_FILE=/var/lib/brassclaw/master.key" >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_TOKEN=your-bearer-token"                  >> /etc/brassclaw/secrets.env
echo "BRASSCLAW_REBORN_WEBUI_USER_ID=admin"                            >> /etc/brassclaw/secrets.env
echo "OPENAI_API_KEY=sk-..."                                            >> /etc/brassclaw/secrets.env

# 3. Start the service — §8.1 migration runs automatically on first serve:
systemctl start brassclaw
```

### 7.3 Service unit file

> **`BRASSCLAW_PG_URL` is REQUIRED for external/production deployments.**
> Pre-Phase-11: required when `BRASSCLAW_REBORN_PROFILE=production`.
> Post-Phase-11: required for all non-local `RuntimeProfile` values
> (`hosted_*`, `enterprise_*`, `secure_default`, `sandboxed`, `experiment`).
> Embedded Postgres is durable storage but is designed for single-host local
> deployments only and must not be used for multi-tenant or internet-facing
> deployments.

Two deployment variants are provided. Choose the one that matches your deployment shape.

#### Variant 1 — Single-host with embedded Postgres (local profile)

```ini
# /etc/systemd/system/brassclaw.service — single-host with embedded PG
[Unit]
Description=BrassClaw Reborn Agent
After=network.target

[Service]
Type=simple
User=brassclaw
WorkingDirectory=/opt/brassclaw

# Bootstrap tier — non-secret; safe as inline Environment=
Environment=BRASSCLAW_REBORN_HOME=/var/lib/brassclaw
# NOTE: Use BRASSCLAW_REBORN_PROFILE until Phase 11 ships. Phase 11 renames this
# to BRASSCLAW_RUNTIME_PROFILE (and expands the value set to 12 RuntimeProfile
# variants). Until then, valid values are: local-dev, local-dev-yolo, production.
# local-dev maps to the LocalSafe policy for single-host serve deployments.
Environment=BRASSCLAW_REBORN_PROFILE=local-dev
Environment=BRASSCLAW_REBORN_LOG=brassclaw=info
# BRASSCLAW_PG_URL is OPTIONAL for local-* profiles — omit to use embedded Postgres:
# Environment=BRASSCLAW_PG_URL=postgresql://brassclaw@127.0.0.1:5434/brassclaw
# Optional — override embedded PG port if 5434 is taken:
# Environment=BRASSCLAW_EMBEDDED_PG_PORT=5435

# Operator-trusted tier (secrets + identity values, never inline) — read by
# systemd as root. File must be root:root 0600.
# Contents: BRASSCLAW_SECRETS_PASSPHRASE_FILE (path to brassclaw-readable file,
#   set ONLY if you ran 'brassclaw secrets rewrap --strategy passphrase-file=...'),
#   BRASSCLAW_REBORN_WEBUI_TOKEN, BRASSCLAW_REBORN_WEBUI_USER_ID, API keys.
EnvironmentFile=/etc/brassclaw/secrets.env

ExecStart=/usr/local/bin/brassclaw serve
Restart=on-failure
RestartSec=5

# Hardening — AGENTS.md: "Review any change touching listeners, auth, secrets with
# a security mindset." Running an embedded DB server requires appropriate isolation.
# NOTE: MemoryDenyWriteExecute=yes requires jit=off in postgresql.conf (§2.2).
# If you remove MDWE, also remove jit=off — see §2.2 for the tandem-change note.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
# AF_INET covers TCP to 127.0.0.1:5434 (embedded PG); AF_UNIX covers PG unix sockets if used.
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
# SystemCallFilter=@system-service covers the baseline. PostgreSQL may need
# additional syscalls (clone, mmap, semget, setrlimit, ioprio_*). If the
# hardened-unit integration test (Phase 10) shows PG requires a syscall outside
# @system-service, extend with that specific syscall rather than weakening the
# filter globally (e.g. SystemCallFilter=@system-service semget).
SystemCallFilter=@system-service
# /etc/brassclaw is read-only to the service (ProtectSystem=strict covers it).
# secrets.env is read by systemd-manager (root) via EnvironmentFile= and
# injected as environment — the service process never opens /etc/brassclaw.
ReadWritePaths=/var/lib/brassclaw
CapabilityBoundingSet=
AmbientCapabilities=
LimitNOFILE=4096
TasksMax=512

[Install]
WantedBy=multi-user.target
```

**Note on profile naming:** The unit template above uses `BRASSCLAW_REBORN_PROFILE=local-dev` (the pre-Phase-11 name). After Phase 11 ships, replace it with `BRASSCLAW_RUNTIME_PROFILE=local_safe` (the cautious single-host production choice: ask-on-write, ask-on-shell). `local_dev` is the developer default (ask only on dangerous actions). Operators who want the developer-grade approval policy can keep `local-dev`. Both are local profiles and work with embedded PG.

#### Variant 2 — Multi-tenant with external Postgres (non-local profile)

```ini
# /etc/systemd/system/brassclaw.service — multi-tenant with external PG
[Unit]
Description=BrassClaw Reborn Agent
After=network.target

[Service]
Type=simple
User=brassclaw
WorkingDirectory=/opt/brassclaw

# Bootstrap tier — non-secret; safe as inline Environment=
Environment=BRASSCLAW_REBORN_HOME=/var/lib/brassclaw
Environment=BRASSCLAW_RUNTIME_PROFILE=hosted_safe
Environment=BRASSCLAW_REBORN_LOG=brassclaw=info
# BRASSCLAW_PG_URL is REQUIRED for non-local profiles (hosted_*, enterprise_*,
# secure_default, sandboxed, experiment). Omitting it triggers fail-closed.
Environment=BRASSCLAW_PG_URL=postgresql://brassclaw@db.internal:5432/brassclaw
# Optional — override embedded PG port (not used when PG_URL is set):
# Environment=BRASSCLAW_EMBEDDED_PG_PORT=5435

# Operator-trusted tier (secrets + identity values, never inline) — read by
# systemd as root. File must be root:root 0600.
# Contents: BRASSCLAW_SECRETS_PASSPHRASE_FILE (REQUIRED if master key is
#   passphrase-wrapped — run 'brassclaw secrets rewrap --strategy passphrase-file=...'
#   once before first boot),
#   BRASSCLAW_REBORN_WEBUI_TOKEN, BRASSCLAW_REBORN_WEBUI_USER_ID, API keys.
EnvironmentFile=/etc/brassclaw/secrets.env

ExecStart=/usr/local/bin/brassclaw serve
Restart=on-failure
RestartSec=5

# Hardening — AGENTS.md: "Review any change touching listeners, auth, secrets with
# a security mindset." Running an embedded DB server requires appropriate isolation.
# NOTE: MemoryDenyWriteExecute=yes requires jit=off in postgresql.conf (§2.2).
# If you remove MDWE, also remove jit=off — see §2.2 for the tandem-change note.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
# AF_INET covers TCP to 127.0.0.1:5434 (embedded PG); AF_UNIX covers PG unix sockets if used.
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
# SystemCallFilter=@system-service covers the baseline. PostgreSQL may need
# additional syscalls (clone, mmap, semget, setrlimit, ioprio_*). If the
# hardened-unit integration test (Phase 10) shows PG requires a syscall outside
# @system-service, extend with that specific syscall rather than weakening the
# filter globally (e.g. SystemCallFilter=@system-service semget).
SystemCallFilter=@system-service
# /etc/brassclaw is read-only to the service (ProtectSystem=strict covers it).
# secrets.env is read by systemd-manager (root) via EnvironmentFile= and
# injected as environment — the service process never opens /etc/brassclaw.
ReadWritePaths=/var/lib/brassclaw
CapabilityBoundingSet=
AmbientCapabilities=
LimitNOFILE=4096
TasksMax=512

[Install]
WantedBy=multi-user.target
```

**Why external-PG deployments require `BRASSCLAW_PG_URL`:** In the pre-Phase-11 codebase, `BRASSCLAW_REBORN_PROFILE=production` triggers the equivalent fail-closed behaviour — embedded PG on a single host is not appropriate for multi-tenant deployments. After Phase 11 ships, `hosted_safe` (and all other non-local `RuntimeProfile` values) enforce the same requirement via the fail-closed guard (`!is_local() && pg_url.is_none()`). The operator must provide an external Postgres URL regardless of which generation of the env var is in use.

**Ceremony note for both variants:** `BRASSCLAW_SECRETS_PASSPHRASE_FILE` in
`secrets.env` is required only if the operator ran `rewrap --strategy
passphrase-file=...`. If the operator did not run rewrap (raw-key-on-disk
ceremony), omit this var. The boot path checks `brassclaw_secrets_master.algorithm`
for consistency (see §4.4 ceremony-selector).

All other configuration (LLM provider id, model, WebUI settings, budget, etc.)
is in the DB, set via `brassclaw config set` or the first-run wizard.

### 7.4 Memory write data-flow — dual-path sequence (Upgrades A + B + C, revision 17)

The following describes the complete execution path triggered by a single
`memory_write` tool call inside the agent loop. Revision 17 replaced the
standalone `PgMemoryEmbeddingStore` write with a call into the existing
`ChunkingMemoryDocumentIndexer` via the new `index_content` method (§4.30.1):

```
memory_write(content, target, append, metadata, old_string, new_string,
             replace_all, timezone,
             kind?, tags?, importance?, context?, summary?)
        │  (existing params: content, target, append, metadata, old_string,
        │   new_string, replace_all, timezone — unchanged)
        │  (new optional params: kind, tags, importance, context, summary —
        │   used to populate brassclaw_memory_chat_records structured columns;
        │   default kind='observation', tags=[], importance=NULL, context={},
        │   summary=NULL when not provided by the LLM)
        │
        ├─ [Existing filesystem path — unchanged]
        │       write to MemoryBackend (MEMORY.md / daily_log / HEARTBEAT.md)
        │       ─── FilesystemMemoryDocumentRepository / PostgresRootFilesystem ──
        │       (this path continues to work as before; the structured Path A
        │        write below is a NEW parallel write, not a replacement)
        │
        ├─ [Path A — always executes]
        │       generate ULID → chat_record_id
        │       INSERT INTO brassclaw_memory_chat_records
        │           (id, tenant_id, user_id, project_id, agent_id,
        │            session_thread_id, run_id,
        │            kind, content, summary, context, importance, tags,
        │            source_ref = NULL)  -- set by Path B after chunk write
        │       ─── PgChatMemoryRecordStore::write() ───────────────────
        │       Returns: chat_record_id (used to derive source_ref for Path B)
        │
        └─ [Path B — only when services.embedding_active IS TRUE
        │           (set at composition startup when embedding.provider_id
        │            is set in brassclaw_config AND that provider IS ACTIVE)]
                source_ref = /memory/chat/<chat_record_id>
                call indexer.index_content(
                        scope      = <scope from Path A row>,
                        source_ref = source_ref,
                        content    = <memory text>,
                        chat_record_id = Some(chat_record_id))
                    │
                    ├─ chunk_document(content, chunk_config)
                    │       → Vec<String>  (800-word chunks, 15% overlap)
                    │
                    ├─ for each chunk: embedding_provider.embed(chunk)
                    │       → Vec<f32>  (dim from provider, e.g. 1536)
                    │       (on API error → degrade to embedding=NULL
                    │        for ALL chunks in this batch — preserves the
                    │        existing indexer degrade behaviour; the error
                    │        is propagated to the caller)
                    │
                    └─ replace_document_chunks_if_current(
                            synthetic_path, content_sha256(content), chunks)
                            writes chunk rows under
                                /memory/chat/<chat_record_id>/*.chunks/<index>
                            with indexed keys:
                                content, embedding, chunk_index,
                                doc_relative_path = "chat/<chat_record_id>",
                                chat_record_id = <chat_record_id>,
                                tenant_id, user_id, [agent_id], [project_id]
                            ─── PostgresRootFilesystem ──────────────────
                            (Filter::VectorNearest on the embedding indexed
                             key translates to a pgvector <=> cosine query)
                UPDATE brassclaw_memory_chat_records
                    SET source_ref = <source_ref>
                    WHERE id = <chat_record_id>
                ─── PgChatMemoryRecordStore::write_path_b_posthook() ────
                (on services.embedding_active = FALSE → silently skipped,
                 no log — this is the normal "feature off" path; the indexer
                 was created without an embedding provider at composition
                 startup, so index_content would write text-only chunks with
                 embedding=NULL — but Path B is not even called when
                 embedding_active is FALSE, so no chunk rows are written)
                (on embedding API error → log warn! with chat_record_id +
                 error reason; Path A row is already committed and
                 unaffected; text-only chunk rows MAY have been written by
                 the indexer's degrade behaviour (§4.30.1 step 3) — they
                 keep FTS current while vector search degrades; the operator
                 can run `brassclaw maintenance backfill-embeddings` to
                 retry failed embeddings and populate the embedding column)
```

**Activation wiring note (revision 17).** The activation is realised at
composition startup by `build_backend()` re-wiring the indexer + backend with
the resolved embedding provider (§6.2a "Activation wiring"). There is no
separate `PgMemoryEmbeddingStore` to preconfigure — the existing
`ChunkingMemoryDocumentIndexer` is the only indexer, and it is either wired
with an embedding provider (Path B active) or not (Path B inactive). The
`services.embedding_active` boolean gates both the Path B write branch (above)
and the `memory_search` vector branch (§4.30.3).

**On embedding activation (existing → Path B enabled):** When an operator
presses "use for embedding" and activates a provider, `embedding.provider_id`
is written to `brassclaw_config`. The change takes effect on the next
`brassclaw serve` restart (§6.2a "Live re-assignment limitation"). From the
first `memory_write` after restart, Path B executes. Existing
`brassclaw_memory_chat_records` rows (written while Path B was inactive) have
`source_ref = NULL` and no chunk subtree — they remain queryable via the GIN
FTS index on `brassclaw_memory_chat_records.tsv`. Operators who want ANN
search over historical memories can run a one-off back-fill job
(`brassclaw maintenance backfill-embeddings`) — this is a Phase 4 task.

**On embedding model change (dimension change):** If the operator re-assigns
the `embedding` role to a provider whose model has a different vector
dimension, the existing chunk rows (with the old dimension) become invalid.
The `backfill-embeddings` command (§8.1 step 10) handles this: it deletes the
old chunk subtree and re-indexes with the new dimension via `index_content`.
The `source_ref` column is preserved (it points to the same path; only the
chunk contents change).

---

## 8. Migration from Existing State

The migration runs automatically on the first boot after upgrading. It is
implemented in `brassclaw_reborn_composition::migration` and runs before
any request is served.

### 8.1 Migration order

> **`migrate-from-libsql` feature gate (R8-L1):** Steps 3–7 are all compiled
> behind the `migrate-from-libsql` feature flag. They depend on
> `RebornConfigFile::load()` (§5.4) and file/libSQL I/O that are removed in
> the next release alongside the feature. The entire
> `brassclaw_reborn_composition::migration` module should be
> `#[cfg(feature = "migrate-from-libsql")]`. Steps 1–2 are unconditional
> (schema migrations run on every boot regardless of upgrade state).

1. Start embedded Postgres (or connect to external).
2. Run all schema migrations (`brassclaw_pg::migrations::run_migrations`),
   including the history-reconciliation bootstrap (§3).
3. **Migrate `config.toml`:** If the file exists, parse it with
   `RebornConfigFile::load`, translate each field to a `(key, value)` pair,
   insert into `brassclaw_config`. Rename the file to `config.toml.migrated`.
   Record that migration occurred.
4. **Migrate `providers.json`:** If the file exists, parse it, upsert each
   `ProviderDefinition` into `brassclaw_llm_providers`. Rename to
   `providers.json.migrated`. Record that migration occurred.
5. **Migrate `sempai_provider.json`:** If the file exists, parse it, write
   `sempai.provider_id` and `sempai.model` into `brassclaw_config`. Rename.
   Record that migration occurred.
6. **Migrate secrets master key:** Handle per ceremony selector
   (`BRASSCLAW_SECRETS_PASSPHRASE_FILE` presence):
   - *`BRASSCLAW_SECRETS_PASSPHRASE_FILE` is absent (or empty — raw-key-on-disk
     ceremony):* If `.reborn-local-dev-secrets-master-key` exists, copy it to
     `$REBORN_HOME/.secrets-master-key` (0600), then upsert the
     `brassclaw_secrets_master` row:
     ```sql
     INSERT INTO brassclaw_secrets_master (tenant_id, version, wrapped_key, algorithm)
     VALUES (<boot_tenant>, 1, '', 'raw-key-on-disk')
     ON CONFLICT (tenant_id, version)
     DO UPDATE SET wrapped_key = '', algorithm = 'raw-key-on-disk';
     ```
     Both `wrapped_key` and `algorithm` are explicitly set — **not** just
     `wrapped_key`. This is required for the ceremony-switch-back case
     (passphrase-wrapped → raw-key-on-disk): if a prior `rewrap` left `algorithm =
     'aes256gcm-argon2id'`, an UPDATE that sets only `wrapped_key = ''` would
     leave the wrong algorithm sentinel and cause the unwrap branch to attempt
     decryption of an empty ciphertext. Zero and delete the old file.
   - *`BRASSCLAW_SECRETS_PASSPHRASE_FILE` is present (passphrase-wrapped
     ceremony):* If `.reborn-local-dev-secrets-master-key` exists and
     `brassclaw_secrets_master` has no row for this tenant, **do not
     auto-migrate**. Print:
     `"Run 'brassclaw secrets rewrap --strategy passphrase-file=<path>' before starting."` and exit with a non-zero code. The operator runs `brassclaw secrets rewrap`
     interactively once, which writes `algorithm = 'aes256gcm-argon2id'`, then
     restarts.
7. **Migrate libSQL database:** If `reborn-local-dev.db` exists, open it
   with the libSQL crate (gated behind a `migrate-from-libsql` feature — see
   §9 for why this feature must be **enabled by default** in the migration
   release), read every table, and write the rows into the corresponding
   Postgres tables using upsert-or-ignore semantics. Rename the file to
   `reborn-local-dev.db.migrated` when done. Record that migration occurred.

   **Tenant/user/project synthesis for libSQL rows that lack these columns
   (H1):** Several libSQL tables have no `tenant_id` (or `user_id` /
   `project_id`) column because the old schema was single-tenant. The
   migration must synthesise these values for every migrated row:

   | libSQL table | Missing columns | Synthesised from |
   |---|---|---|
   | `safety_config` | `tenant_id` | `boot_tenant` (see below) |
   | `settings` (token settings) | `tenant_id`, `user_id` | `boot_tenant`, `boot_user` |
   | `memory_docs` | `tenant_id`, `user_id`, `project_id` | `boot_tenant`, `boot_user`, `"default"` |
   | `root_filesystem_entries` | `tenant_id` | `boot_tenant` |
   | `root_filesystem_index_specs` | `tenant_id` | `boot_tenant` |
   | `root_filesystem_events` | `tenant_id` | `boot_tenant` |
   | `capability_permissions` | `tenant_id` | `boot_tenant` |
   | `hooks_predicate_invocations/values` | *(none — no tenant column in source)* | n/a; appended verbatim |
   | `trigger_records` | *(none — already has `tenant_id` in both libSQL and PG schemas)* | n/a; upsert verbatim into `brassclaw_triggers` — column-for-column match; **no type cast needed** because the PG table also stores all date columns as TEXT (RFC-3339 strings, see §4.24 column-type note). The migration is a straight TEXT-to-TEXT upsert. |
   | `local_reborn_access` | *(none — already has `tenant_id`)* | n/a; upsert verbatim into `brassclaw_local_access`; **no type cast needed** — the PG table also stores `created_at`/`updated_at` as TEXT (§4.24). |

   `boot_tenant` = value of `brassclaw_config.identity.tenant` if already
   written by step 3 (migrated from `config.toml`), otherwise the literal
   string `"default"`. `boot_user` = `brassclaw_config.identity.owner` if
   written by step 3, otherwise `"admin"`. These are the same defaults the
   first-run wizard uses (§6.2 Step 3/4). The synthesised values are written
   into the PG rows at insert time; the libSQL rows are not modified.
8. **Set `boot.initialized = true`** in `brassclaw_config` — **only if at
   least one migration step (3–7) actually found and processed a source
   artifact.** For a completely fresh install (no pre-existing files), leave
   `boot.initialized` absent so the first-run wizard runs (§6.1). The wizard
   itself sets `boot.initialized = true` at the end.

9. **Enable default persistent chat-memory storage (Upgrade A).** This step
   is **unconditional and not gated behind `migrate-from-libsql`**. After the
   schema migrations in step 2, `brassclaw_memory_chat_records` (V025) exists.
   The memory-write path (`PgChatMemoryRecordStore`) is activated unconditionally
   by the composition layer at startup — no config key is required to enable it.
   **No data migration is needed**: chat-memory records were not durably stored
   before this plan; the table starts empty and fills from the first
   `memory_write` call after the upgrade.

10. **Create the `embedding` provider role entry in `brassclaw_config` (Upgrade B).**
    This step is **unconditional and not gated behind `migrate-from-libsql`**.
    The three role config keys (`llm.kohai.*`, `llm.sempai.*`, `embedding.*`)
    use the same `brassclaw_config` table. No separate migration is needed for
    the `embedding` role config keys — they are absent until an operator presses
    "use for embedding" in the UI (§6.2a). The absence of `embedding.provider_id`
    is the correct initial state: Path B is silently skipped until the operator
    assigns the role.

    **No V026 migration (revision 17):** The standalone
    `brassclaw_memory_embeddings` table is NOT created — revision 17 removed it.
    Path B reuses the existing `brassclaw_memory` chunk system (§4.30). The
    chunk rows live in the existing `brassclaw_root_filesystem` VFS backing
    table (§4.19) + sibling index tables; the `vector` extension is created in
    V000 (§4.20); the `source_ref` column on
    `brassclaw_memory_chat_records` is added in V025 (§4.29). No V026 file is
    ever created. The `embedding`-role provider is wired into
    `build_backend()` at composition startup (§6.2a "Activation wiring").

    **Backfill of existing memories (optional):** If an operator assigns the
    `embedding` role after the upgrade and wants ANN search over memories written
    before the upgrade, they can run:
    ```
    brassclaw maintenance backfill-embeddings [--tenant <id>]
    ```
    This command (Phase 4 task) reads all `brassclaw_memory_chat_records` rows
    whose `source_ref` is NULL or whose chunk subtree (under
    `/memory/chat/<chat_record_id>/*.chunks/`) has chunk rows with
    `embedding = NULL`, reconstructs the `MemoryDocumentScope` from the row's
    `(tenant_id, user_id, agent_id, project_id)` columns, calls
    `indexer.index_content(scope, source_ref, content, chat_record_id)` for
    each (§4.30.1), which chunks + embeds the content and
    writes the chunk rows. It is idempotent and safe to re-run. If the
    `embedding` role is not assigned, the command exits immediately with no
    writes. If the embedding model dimension has changed since the last
    backfill, the command deletes the old chunk subtree and re-indexes with
    the new dimension (§4.30.4).

All steps are idempotent (upsert / rename). Re-running the migration after
a crash is safe.

**`brassclaw migrate --dry-run`:** Steps 3–10 run in read-only simulation mode —
no DB writes, no file renames. Results are printed and the process exits.

---

## 9. Feature Flag Cleanup

After the migration, these Cargo features are **removed**:

- `libsql` on every crate
- `postgres` on every crate (Postgres is now the only backend, not a feature)

The `embedded-postgres` feature on `brassclaw_reborn_composition` remains so
callers that supply their own `BRASSCLAW_PG_URL` do not pay the binary-size
cost of bundling `postgresql_embedded`.

### 9.1 `migrate-from-libsql` feature lifecycle

The `migrate-from-libsql` feature (workspace `Cargo.toml`) gates the libSQL
read path used in §8.1 step 7. It must be **on by default** in the single
upgrade release so that no user with an existing `reborn-local-dev.db` silently
loses data. Concretely: the release `Cargo.toml` must have:

```toml
[features]
# Upgrade release: migrate-from-libsql is on by default.
# libsql is pulled transitively by migrate-from-libsql = ["dep:libsql"].
# The postgres/html-to-markdown/tui features remain in default as before.
default = ["migrate-from-libsql", "postgres", "html-to-markdown", "tui"]
migrate-from-libsql = ["dep:libsql"]
# M3 — backward-compat alias: any caller that activates "libsql" directly
# (e.g. `cargo build --features libsql`) must not get a hard compile error.
# The alias routes to migrate-from-libsql so those builds continue to work
# for the duration of the upgrade release. Remove this alias together with
# migrate-from-libsql in the following release.
libsql = ["migrate-from-libsql"]
# postgres, html-to-markdown, tui defined as before (unchanged)
```

The feature and all code behind it are removed in the **following** release
after one full upgrade cycle. The integration test gating on this feature
(`seed_libsql_then_migrate_asserts_all_rows_in_pg`) must be green in CI
before the migration release ships.

### 9.2 `replay` and `import` features

The root `Cargo.toml` currently defines:
```toml
replay = ["libsql"]         # memory-substrate regression tests
import = ["dep:json5", "libsql"]  # OpenClaw import
```

Both depend on `libsql`, which is being removed. Decision:

- **`replay`:** Rebase the replay-gate test harness onto the embedded Postgres
  test rig (Phase 6 work item). The `replay-gate.yml` CI job is updated to
  spin up embedded PG instead of libSQL. The `replay` feature becomes
  `replay = ["postgres"]` (or is removed if the test rig is always-on).
- **`import`:** Port the OpenClaw import path off libSQL. Until ported, the
  `import` feature is **removed** from the default set and marked deprecated
  in `CHANGELOG.md`. A follow-up issue is filed.

Both must be resolved explicitly in Phase 6; the plan does not leave them
referencing a non-existent `libsql` feature.

---

## 10. Implementation Phases

> **Phase ordering note:** "Phase 2 — Config migration" is the first *content*
> phase but not the first *execution* phase. Phase 0 (embed PG) and Phase 1
> (schema runner) are prerequisites; implementation starts there.

### Phase 0 — Embedded Postgres crate (no existing code touched)
- [ ] Create `crates/brassclaw_embedded_postgres/`
- [ ] `postgresql_embedded` integration, pinned PG 16
- [ ] `checksums.rs`: compiled-in SHA-256 list; verify after download; suppress `POSTGRESQL_VERSION` env override
- [ ] `initdb`, `pg_ctl` lifecycle, `health.rs` retry loop
- [ ] Orphaned-server detection: check `postmaster.pid` PID liveness on startup
- [ ] Explicit `shutdown()` method; `Drop` is best-effort fallback only
- [ ] Log rotation config in `postgresql.conf` (§2.2)
- [ ] Unit tests (mock pg_ctl, verify postgresql.conf tuning)

### Phase 1 — Schema and migration runner
- [ ] Create `crates/brassclaw_pg/`
- [ ] Write all `V000__` … `V026__` SQL migration files (all `IF NOT EXISTS`; all `CREATE TRIGGER`): V000 shared triggers + `CREATE EXTENSION IF NOT EXISTS vector` (§4.20, pgvector must be first), V001–V020 original tables, V021 triggers + local_access, V022 conversation_state, V023 outbound_state (4 tables), V024 subagent_goals, **V025 memory_chat_records** (§4.29, including `source_ref` + `forensic_packet_id` columns), **V026 forensic_packets** (§4.28, `brassclaw_forensic_packets` DDL with `chat_record_id` link). V018 covers 3 sibling root_filesystem tables.
- [ ] Wire `refinery` runner with history-reconciliation bootstrap (§3)
- [ ] `PgPool` builder (from URL or from `ManagedPostgres` handle)
- [ ] Add `pgvector` crate to `brassclaw_pg/Cargo.toml`; verify the `vector` type + HNSW index compile against a local pgvector-enabled PG instance (the chunk system's `embedding` indexed key is stored as a `vector(N)` column in the VFS backing table via `PostgresRootFilesystem::ensure_index`)
- [ ] Update `brassclaw_embedded_postgres/src/initdb.rs` to install the pgvector shared library into the embedded PG data directory after `initdb` (§3 pgvector dependency note)
- [ ] Test: fresh DB gets all tables (including V025 with `source_ref` + `forensic_packet_id`, and V026 `brassclaw_forensic_packets`); re-run is idempotent
- [ ] Test: pre-existing hooks/settings tables don't cause refinery to fail
- [ ] Test: `brassclaw_root_filesystem` VFS backing table accepts `IndexKind::Vector { dim }` from `PostgresRootFilesystem::ensure_index` and that `Filter::VectorNearest` translates to a pgvector `<=>` cosine query (prerequisite for the chunk system's vector search path, §4.30.3)

### Phase 2 — Config migration (start content work here, after Phase 0+1)
- [ ] `brassclaw_reborn_composition::db_config` module (`load_config_snapshot`, `save_config_key`)
- [ ] Confirm `db_config.rs` is NOT added to `brassclaw_reborn_config` — boundary crate stays pure (§5.4)
- [ ] Replace **runtime serve-path** `RebornConfigFile::load` callers with `db_config::load_config_snapshot`; retain `load()` behind `migrate-from-libsql` for §8.1 step 3 + §4.4 rewrap step 2 (§5.4 — removal deferred to next release)
- [ ] First-run wizard (`brassclaw config init --interactive`)
- [ ] `brassclaw config` CRUD subcommands (get/set/unset/list/show-all/export/import)
- [ ] `ProviderRepo` → DB-backed
- [ ] `sempai_provider.json` → `brassclaw_config` rows
- [ ] Test: round-trip all `RebornConfigFile` sections through DB
- [ ] Test: `save_config_key(…, ConfigWriteContext::AgentSession)` returns `EnvKeyWriteForbidden` for keys ending in `_env` (§5.5 / §1c write-gate)
- [ ] Test: `save_config_key(…, ConfigWriteContext::AgentSession)` succeeds for non-`*_env` keys (gate is scoped to `_env` suffix only — not a blanket `AgentSession` write ban)
- [ ] Test: `save_config_key(…, ConfigWriteContext::Operator)` succeeds for `*_env` keys (operator path not blocked)
- [ ] Test: `save_config_key(…, value = "sk-abc123")` returns `InlineSecretForbidden` for BOTH `Operator` and `AgentSession` contexts (inline-secret guard is unconditional — §5.5 security note)
- [ ] Test: `save_config_key(…, value = "OPENAI_API_KEY")` succeeds — env-var NAME is not a secret, must not be rejected by the inline-secret guard
- [ ] Test: boolean/integer/decimal config values survive DB round-trip (serialization contract §4.2)
- [ ] Remove `toml_edit`, `fs4`, `tempfile` from `brassclaw_reborn_config`; keep `toml`

### Phase 3 — Secrets migration
- [ ] `PgSecretStore` and `PgCredentialBroker` — **`PgCredentialBroker` must implement both `CredentialAccountStore` and `CredentialSessionStore`** (two traits; see §5.3 note)
- [ ] `brassclaw_secrets_master` with `key_version` (§4.4 schema)
- [ ] local-dev: 0600 raw key file at `$REBORN_HOME/.secrets-master-key`
- [ ] `brassclaw secrets rewrap --strategy passphrase|passphrase-file=<path>|keychain [--tenant <id>]` (§4.4)
- [ ] `rewrap` tenant resolution: `--tenant` flag → `config.toml identity.tenant` → `brassclaw_config` DB → `"default"` (§4.4 R6-MH1)
- [ ] `rewrap --old-passphrase-file=<path>` flag for interactive passphrase-change in shell (§4.4 R6-L1)
- [ ] `rewrap` key-source rule: check old filename `.reborn-local-dev-secrets-master-key` first, then `.secrets-master-key`; fail-closed if neither found but encrypted rows exist (§4.4)
- [ ] `rewrap` passphrase-change path: unwrap existing wrapped key; read old passphrase from `--old-passphrase-file` → `BRASSCLAW_SECRETS_PASSPHRASE_FILE` → `$CREDENTIALS_DIRECTORY` (§4.4)
- [ ] Per-boot unwrap: ceremony-selector — absent → raw-key-on-disk, present → passphrase-wrapped; boot path checks `brassclaw_secrets_master.algorithm` against `BRASSCLAW_SECRETS_PASSPHRASE_FILE` presence for consistency (see §4.4 ceremony derivation); check is skipped on fresh install before wizard runs (ordering invariant)
- [ ] Fail-closed if master key absent AND no raw key file AND no passphrase file (drop "in production profile" — the guard is now ceremony-based, not profile-based)
- [ ] Abstract secret-value reads to check `$CREDENTIALS_DIRECTORY` (systemd LoadCredential) first, env second (§7)
- [ ] Migration from `.reborn-local-dev-secrets-master-key` (§8.1 step 6)

### Phase 4 — Runtime store migrations (one crate at a time)
- [ ] `PgRunStateStore` (in `brassclaw_run_state`)
- [ ] `PgApprovalRequestStore` (in `brassclaw_approvals`)
- [ ] `PgTurnStateStore` + `PgLoopCheckpointStore` (in `brassclaw_turns`): **L3 — `FilesystemTurnStateStore` implements 5 traits**: `TurnStateStore`, `TurnSpawnTreeStateStore`, `TurnEventProjectionSource`, `LoopCheckpointStore`, and `TurnRunTransitionPort`. `PgTurnStateStore` must implement all five; implementing only `TurnStateStore` and `LoopCheckpointStore` (the two named in earlier plan drafts) leaves three trait impls missing.
- [ ] `PgCheckpointStateStore` (in `brassclaw_loop_support`)
- [ ] `PgSessionThreadService` (in `brassclaw_threads`)
- [ ] `PgCapabilityLeaseStore` (in `brassclaw_authorization`)
- [ ] `PgResourceGovernorStore` (in `brassclaw_resources`)
- [ ] `PgProcessStore` + `PgProcessResultStore` (in `brassclaw_processes`)
- [ ] `PgExtensionInstallationStore` (in `brassclaw_extensions`) + extension manifests table
- [ ] `PgDurableEventLog` + `PgDurableAuditLog` (in `brassclaw_reborn_event_store`): **M7 — two-part rewrite, three wiring sites.** (1) **Query path:** `RebornEventStoreConfig::Postgres` currently routes all writes through `PostgresRootFilesystem` as a VFS fabric — NOT direct SQL. Replace with direct `INSERT INTO brassclaw_events` / `brassclaw_audit_log` queries against the shared `PgPool` (§4.14). (2) **Pool consolidation + wiring sites:** `RebornEventStoreConfig::Postgres { url }` opens its **own** `deadpool_postgres::Pool` (confirmed). After the rewrite, `PgDurableEventLog` / `PgDurableAuditLog` accept `Arc<PgPool>` (shared with composition) — no separate pool. There are **three** factory wiring sites to update: (a) `build_postgres_production()` at factory.rs:2671 — replace `RebornEventStoreConfig::Postgres { url }` with `PgDurableEventLog::new(Arc::clone(&pool))`; (b) `build_local_dev()` at factory.rs:1785-1787 — currently calls `FilesystemDurableEventLog::new()` / `InMemoryDurableEventLog::new()` directly (no `RebornEventStoreConfig` used at all); must be updated to also use `PgDurableEventLog::new(Arc::clone(&pool))` after Phase 0+1 (local-dev also gets embedded PG); (c) any test-path `build_local_dev()` invocations that use `InMemoryDurableEventLog` — these must be preserved behind `#[cfg(test)]` if needed. The `RebornEventStoreConfig` enum loses its `Postgres`, `Libsql`, `InMemory`, and `Jsonl` variants in Phase 6; the enum may be removed entirely if no variants remain.
- [ ] `PgTokenSettingsStore` (in `brassclaw_reborn_composition`)
- [ ] `PgSafetyConfigStore` (in `brassclaw_product_workflow`) — **one struct, two traits**: `SqliteSafetyConfigStore` implements both `SafetyConfigStore` and `CapabilityPermissionStore` in a single struct; `PgSafetyConfigStore` must do the same (M1)
- [ ] `PgMemoryDocStore` with generated-column GIN FTS (in `brassclaw_reborn_composition`)
- [ ] Background retention sweep task in `brassclaw serve` only (§4.21); add `brassclaw maintenance prune-old-data` CLI command
- [ ] `PgResourceGovernorStore`: implement CAS via `version` column conditional UPDATE; return `BudgetConflict` on 0-rows-affected; integration test for concurrent increments (§4.12)
- [ ] `PgBudgetGateStore` (in `brassclaw_resources`) — implements `BudgetGateStore`; replaces `/resources/budget-gates.json` path (§4.22)
- [ ] `PgRebornIdentityStore` (in `brassclaw_reborn_identity`) — implements `RebornIdentityResolver` trait; wired in `brassclaw_reborn_composition/src/factory.rs` (not in `brassclaw_reborn_identity/factory.rs` — that file does not exist; §4.23)
- [ ] `PostgresTriggerRepository` is already complete in `brassclaw_triggers/src/postgres.rs` — promote to unconditional (remove `#[cfg(feature = "postgres")]` gate); remove `LibSqlTriggerRepository`; move DDL to `brassclaw_pg` V021 (§4.24); **update all three string constants**: `TRIGGER_TABLE` (line 16) → `"brassclaw_triggers"`, `TRIGGER_COLUMNS` (line 17) → same column list, and `POSTGRES_TRIGGER_SCHEMA` (line 968) → replace `CREATE TABLE IF NOT EXISTS trigger_records` with `CREATE TABLE IF NOT EXISTS brassclaw_triggers` (and update all four index names to `brassclaw_triggers_*`). Without updating `POSTGRES_TRIGGER_SCHEMA`, `run_migrations()` would re-create a new `trigger_records` table after V021 has already renamed it.
- [ ] `PgLocalTriggerAccessStore` (in `brassclaw_reborn`) — replaces `RebornLibSqlLocalTriggerAccessStore`; local-dev only (§4.24)
- [ ] `PgConversationStateStore` (in `brassclaw_conversations`) — replaces `FilesystemConversationStateStore`; implements `ConversationStateRepository`; CAS via `revision` column (§4.25)
- [ ] `PgOutboundStateStore` (in `brassclaw_outbound`) — replaces `FilesystemOutboundStateStore`; implements both `OutboundStateStore` AND `CommunicationPreferenceRepository` (§4.26)
- [ ] `PgSubagentGoalStore` (in `brassclaw_reborn`) — replaces `FilesystemSubagentGoalStore`; remove `filesystem-goal-store` feature gate (§4.27)
- [ ] `PgInterceptorStore` (in `brassclaw_interceptor`) — replaces `NoopInterceptorStore` in production composition (§4.28); implements `InterceptorStore` trait (`save`, `get`, `list_recent`); add `link_chat_record(run_id, iteration, chat_record_id)` helper UPDATE method (no `tenant_id` parameter — the store is constructed with `tenant_id` at wire-up time per §4.28 implementation note; `self.tenant_id` is used internally); wire in production composition factory; `NoopInterceptorStore` is retained for tests and noop mode; integration test: `save(packet)` → `get(packet_id)` round-trips all `ForensicPacket` fields including `kohai_cache_creation_input_tokens`; integration test: `list_recent(limit)` returns packets ordered by `captured_at DESC`
- [ ] **Chat-memory ↔ forensic packet linking** — after `PgChatMemoryRecordStore` writes a row, call `PgInterceptorStore::link_chat_record(run_id, iteration, chat_record_id)` to populate `brassclaw_forensic_packets.chat_record_id`; also set `brassclaw_memory_chat_records.forensic_packet_id` on the newly written memory row; both are best-effort (no-op if no matching packet row); integration test: `memory_write` → assert `brassclaw_memory_chat_records.forensic_packet_id` is set + `brassclaw_forensic_packets.chat_record_id` is set
- [ ] Verify `brassclaw_root_filesystem` queries in `PostgresRootFilesystem` always scope to `tenant_id`; extend `PostgresRootFilesystem` to support the two sibling tables (`brassclaw_root_filesystem_index_specs`, `brassclaw_root_filesystem_events`) (§4.19)
- [ ] **`PgChatMemoryRecordStore` (Upgrade A)** — add to memory-write path (§4.29); ensure every `memory_write` tool call unconditionally inserts a row into `brassclaw_memory_chat_records` (with `source_ref = NULL` — Path B populates it after the chunk write); no feature flag; replaces transient in-process storage for chat-memory entries; integrations test: `memory_write` → assert row in `brassclaw_memory_chat_records`
- [ ] **`brassclaw_memory` trait extension (Upgrade B/C, revision 17)** — extend `MemoryDocumentIndexer` trait with `index_content(scope, source_ref, content, chat_record_id)` method (§4.30.1 — full signature: `async fn index_content(&self, scope: &MemoryDocumentScope, source_ref: &str, content: &str, chat_record_id: Option<&str>)`); implement on `ChunkingMemoryDocumentIndexer`; add `fs_keys::CHAT_RECORD_ID` constant to `repo/filesystem.rs`; store `chat_record_id` as an indexed key on chunk rows written by `index_content`; unit test: `index_content` with no parent document writes chunk rows under the synthetic subtree; unit test: `index_content` is idempotent (re-run with same content is a no-op); unit test: `index_content` with different content replaces the chunk set
- [ ] **`brassclaw_embeddings` crate refactor (Upgrade B/C, revision 17)** — remove `EmbeddingsConfig`, `create_provider()`, concrete HTTP provider impls (`OpenAiEmbeddings` / `NearAiEmbeddings` / `OllamaEmbeddings` / `BedrockEmbeddings`), binary-side resolver `src/config/embeddings.rs::resolve_embeddings_config`, and `Workspace::with_embeddings_cached` / `with_embeddings_uncached` wiring; RETAIN `EmbeddingProvider` trait + `EmbeddingError`, `CachedEmbeddingProvider` + `EmbeddingCacheConfig`, `url_check::check_base_url`, `default_dimension_for_model`, `MockEmbeddings`; update `crates/brassclaw_embeddings/AGENTS.md` to reflect the refactor; grep `src/app.rs`, `src/cli/mod.rs`, `src/cli/doctor.rs::check_embeddings` for call sites and update them
- [ ] **`EmbeddingRoleAdapter` (Upgrade B/C, revision 17)** — new type in `brassclaw_reborn_composition` that resolves the `embedding`-role provider from `brassclaw_config` + `brassclaw_llm_providers` at composition startup, constructs an HTTP client calling the provider's embedding endpoint, runs `url_check::check_base_url` on the resolved base URL, wraps the result in `CachedEmbeddingProvider`, and exposes `Arc<dyn brassclaw_memory::EmbeddingProvider>` for wiring into `RepositoryMemoryBackend::with_embedding_provider(...)` and `ChunkingMemoryDocumentIndexer::with_embedding_provider(...)` (§3); unit test: adapter returns `None` when `embedding.provider_id` is absent; unit test: adapter returns `Some(provider)` when the role is assigned and the provider is active; unit test: adapter calls `url_check::check_base_url` and rejects non-http(s) / AlwaysBlocked URLs
- [ ] **`build_backend()` re-wiring (Upgrade B/C, revision 17)** — update `crates/brassclaw_host_runtime/src/first_party_tools/memory.rs:278-298` to resolve the `embedding`-role provider at composition startup; if active, wire the `EmbeddingRoleAdapter` into `ChunkingMemoryDocumentIndexer::with_embedding_provider(...)` and `RepositoryMemoryBackend::with_embedding_provider(...)` and set `services.embedding_active = true`; if not active, leave the indexer without an embedding provider (current behaviour) and set `services.embedding_active = false`; update `dispatch_search()` at line 337-341 to use `.with_vector(services.embedding_active)` instead of `.with_vector(false)`; add `embedding_active: bool` + `embedding_provider: Option<Arc<dyn brassclaw_memory::EmbeddingProvider>>` to `MemoryServices` (§4.30.3, §6.2a)
- [ ] **Chunk-cascade in retention sweep (revision 17)** — update the background retention sweep task (§4.21) so that when a `brassclaw_memory_chat_records` row is pruned, the sweep resolves `source_ref`, lists all chunk records under `<source_ref>/*.chunks/`, deletes them, then deletes the Path A row; the deletion MUST be transactional (delete chunks first, then the Path A row; rollback if chunk deletion fails) (§4.30.2)
- [ ] **Three-role provider preconfiguration** — update composition layer to resolve `embedding.provider_id` alongside `llm.kohai.*` and `llm.sempai.*` at startup; wire the `EmbeddingRoleAdapter` (no `PgMemoryEmbeddingStore` — revision 17 removed it)
- [ ] **`brassclaw maintenance backfill-embeddings` CLI command** (§8.1 step 10 / §7.4) — reads `brassclaw_memory_chat_records` rows whose `source_ref` is NULL or whose chunk subtree has chunks with `embedding = NULL`; reconstructs `MemoryDocumentScope` from the row's `(tenant_id, user_id, agent_id, project_id)` columns; calls `indexer.index_content(scope, source_ref, content, chat_record_id)` for each; idempotent; no-op if no `embedding`-role provider assigned; handles dimension change by deleting the old chunk subtree and re-indexing (§4.30.4)
- [ ] **Integration test (revision 17 end-to-end)** — assign `embedding` role to a provider (use `MockEmbeddings` or a local Ollama model), trigger `memory_write`, assert: (1) row in `brassclaw_memory_chat_records` with `source_ref` set, (2) chunk rows under `/memory/chat/<chat_record_id>/*.chunks/` with `embedding` indexed key non-NULL, (3) `memory_search` with vector enabled returns the memory via vector similarity; this test gates the chunk-system wiring activation — it surfaces latent bugs in the existing (currently-unwired) indexer path (R17-M)

### Phase 5 — Hooks and auth
- [x] Rename `brassclaw_hooks_postgres` → `brassclaw_hooks_pg`: update workspace `members` array in root `Cargo.toml` and all dependent `[dependencies]` entries that reference `brassclaw_hooks_postgres` by name (L5)
- [x] Strip the `#[cfg(feature = "postgres")]` module declarations from `lib.rs` in the renamed crate (`mod backend`, `mod hashing`, `mod schema`, and their re-exports are currently gated at the module level in `lib.rs` — making deps mandatory in `Cargo.toml` alone is insufficient; the `lib.rs` cfg attributes must also be removed so the modules are compiled unconditionally) (H3)
- [x] Remove the `postgres` optional feature from `brassclaw_hooks_pg/Cargo.toml`; make `deadpool-postgres` and `tokio-postgres` unconditional deps
- [x] **Port `brassclaw_hooks_parity` tests into `brassclaw_hooks_pg`** before deleting the crate: copy `tests/parity_matrix.rs` (deterministic cross-backend parity matrix) and `tests/multi_host_adversarial.rs` (concurrent-write correctness: N concurrent writers, cross-host replay, LRU eviction races, per-key cap under flood, clock-skew) into `crates/brassclaw_hooks_pg/tests/`. These are the only regression gate for concurrent-write correctness; deleting without porting removes that gate.
- [x] Delete `brassclaw_hooks_libsql` and `brassclaw_hooks_parity` **after** the port above is complete and CI is green
- [x] `PgAuthProductServices`
- [x] Wire `brassclaw_reborn_composition::factory` to single Postgres path; wire `PostgresPredicateStateBackend` via `PredicateEvaluator::with_state_backend(...)` (currently the production hooks factory uses an in-memory predicate backend — L6)
- [x] Pool drop before `managed_pg.shutdown().await` — `serve.rs` now starts `ManagedPostgres` (or uses `BRASSCLAW_PG_URL`), builds a pool, upgrades the build input to `RebornBuildInput::postgres_with_resolved_secret_master_key`, and calls `managed_pg.shutdown().await` only after `runtime.shutdown().await` has consumed (and dropped) the runtime and its pool; see `start_postgres_and_upgrade_input` in `brassclaw_reborn_cli/src/commands/serve.rs`

### Phase 6 — libSQL removal
- [ ] Rebase `replay` feature onto embedded Postgres test rig (§9.2)
- [ ] Deprecate/remove `import` feature; file follow-up issue for OpenClaw port (§9.2)
- [ ] Delete all `#[cfg(feature = "libsql")]` and `#[cfg(not(feature = "libsql"))]` blocks
- [ ] Remove `libsql` from all `Cargo.toml` files (except `migrate-from-libsql` for upgrade release)
- [ ] **Remove `libsql` from the workspace `Cargo.toml` `default` feature array** — the root `Cargo.toml` currently has `default = ["postgres", "libsql", "html-to-markdown", "tui"]`; `libsql` must be removed from this list (it stays only as the alias entry `libsql = ["migrate-from-libsql"]` for the upgrade release — §9.1). Removing only the `libsql = [...]` feature definition without removing it from `default` leaves libSQL compiled in by default.
- [ ] Remove remaining `libsql` feature gate declarations from workspace `Cargo.toml` (other than the alias)
- [ ] **Update `brassclaw_reborn_composition/Cargo.toml` `[features]` default list** — remove `libsql`; this crate currently defaults to `libsql` not `postgres`, and its own `Cargo.toml` must be updated separately from the workspace root (H4, §5.5)
- [ ] **Update `brassclaw_reborn_cli/Cargo.toml` `[features]` default list** — remove `libsql`; this crate currently has `default = ["root-llm-provider", "libsql"]` and its own `Cargo.toml` must be updated separately from the workspace root
- [ ] Delete `brassclaw_hooks_libsql` crate directory
- [ ] Delete `RebornLibSqlIdempotencyLedger` from `brassclaw_product_workflow_storage` — `RebornPostgresIdempotencyLedger` already exists in the same crate and is the replacement (L4; see §1a). Note: `RebornPostgresIdempotencyLedger::new()` takes `Arc<PostgresRootFilesystem>` and internally delegates through `FilesystemIdempotencyLedger` backed by the VFS — no direct SQL rewrite needed; the VFS-backed implementation is correct and complete as-is.
- [ ] Remove `RebornEventStoreConfig::Libsql`, `RebornEventStoreConfig::InMemory`, and `RebornEventStoreConfig::Jsonl` variants from `brassclaw_reborn_event_store/src/lib.rs`; remove `libsql` dep from `brassclaw_reborn_event_store/Cargo.toml`; if no variants remain after removing all non-Postgres paths, remove the `RebornEventStoreConfig` enum entirely (the stores are wired directly with `Arc<PgPool>` after Phase 4)
- [ ] Remove `#[cfg(feature = "filesystem-goal-store")]` guard from `brassclaw_reborn/src/subagent/goal_store.rs` — this feature gate must be stripped alongside the other filesystem/libSQL feature removals
- [ ] Update `brassclaw_architecture` boundary tests
- [ ] **WebUI v2 — "use for embedding" button (Upgrade B):** add the third action button to the provider-config panel in `brassclaw_webui_v2` / `brassclaw_webui_v2_static`; button label must be verbatim **"use for embedding"**; placed to the right of "use as sempai"; writes `embedding.provider_id` and `embedding.model` to `brassclaw_config` via a new provider-role endpoint; UI must indicate active/inactive state (button highlighted when current provider holds the `embedding` role); clearing the role when the provider is deactivated or removed; display a "restart required" hint when the `embedding` role is re-assigned (the embedding provider is resolved at composition startup, §6.2a "Live re-assignment limitation")

### Phase 7 — libSQL → Postgres data migration at boot

> **Phase ordering constraint (L1):** Phase 7 depends on Phase 6 for the
> `migrate-from-libsql` feature flag. However, Phase 6 (libSQL removal) must
> NOT be merged before Phase 7's migration code is complete and green in CI.
> The correct sequence is:
> 1. Implement Phase 7 (migration code + integration test) gated behind
>    `migrate-from-libsql` feature.
> 2. Merge Phase 6 **only after** Phase 7 is complete. Phase 6 removes the
>    unconditional libSQL dep; Phase 7's `migrate-from-libsql` transitively
>    keeps the dep available for the upgrade release only.
> 3. In the release after the upgrade release, remove Phase 7 code + feature.
> If Phase 6 is merged before Phase 7 is complete, the `migrate-from-libsql`
> feature will reference a non-existent crate and the upgrade migration code
> won't compile. The CI gate on `seed_libsql_then_migrate_asserts_all_rows_in_pg`
> enforces this — it must be green before Phase 6 merges.

- [ ] `brassclaw_reborn_composition::migration` module
- [ ] Steps 3–7 from §8.1 (including profile-aware secrets step)
- [ ] `migrate-from-libsql` is **default-on** in the upgrade release (§9.1)
- [ ] Integration test: seed a libSQL DB, run migration, verify all rows land in PG
- [ ] **Integration test (upgrade-flow decryption, gate migration release):** seed a
      libSQL DB with an encrypted secret (OAuth token encrypted under the old raw master
      key); run `rewrap` (reads old key, wraps it); run `serve` (§8.1 migrates rows);
      assert the migrated secret decrypts correctly with the wrapped key. Must be green
      before migration release ships.
- [ ] Test: `boot.initialized` is NOT set on a completely fresh install (wizard runs)
- [ ] Test: `boot.initialized` IS set when any source artifact was found
- [ ] **Integration test (non-default tenant upgrade, gate migration release — R6-MH1):**
      seed a config.toml with `identity.tenant = "mycorp"` (not `"default"`); run
      `rewrap --tenant mycorp --strategy passphrase-file=...`; assert the
      `brassclaw_secrets_master` row has `tenant_id = "mycorp"`; run `serve`; assert
      §8.1 step 6 finds the row (no non-zero exit); assert a migrated encrypted
      `root_filesystem_entries` row decrypts correctly under the wrapped key.
      Must be green before migration release ships.
- [ ] Phase 3: implement `rewrap --tenant <id>` flag and 4-step tenant-resolution
      logic (`--tenant` → `config.toml` → `brassclaw_config` DB → `"default"`)
- [ ] Phase 9: document `--old-passphrase-file=<path>` in passphrase-rotation runbook
- [ ] Rename `.migrated` files; remove `migrate-from-libsql` feature in next release
- [ ] Remove `RebornConfigFile::load()` in this same next release (gated removal — §5.4)

### Phase 8 — File-based config removal
- [ ] Remove `config_file_path()`, `providers_file_path()`, `sempai_provider_file_path()`
      from `RebornHome`
- [ ] Remove `config.toml.lock`, `providers.json.lock` discipline from `ProviderRepo`
      and `DefaultLlmSlotUpdateSession`
- [ ] **Remove or hollow out `DefaultLlmSlotUpdateSession`** (struct at line ~476,
      impl body through ~974 of `config_file.rs`): this struct uses `toml_edit`
      for in-place TOML rewriting (field `doc: toml_edit::DocumentMut` at struct
      definition) and `fs4` for advisory file locking (used in the impl body,
      ~line 854). Both deps are removed from `brassclaw_reborn_config/Cargo.toml`
      in Phase 2. Phase 8 must delete the struct definition at line ~476 (or remove
      the struct entirely if no other call site exists — verify with
      `grep -rn DefaultLlmSlotUpdateSession`), not just remove the dep entries (M4).
- [ ] Update `brassclaw_reborn_cli` `config init` → wizard

### Phase 9 — Systemd unit and documentation
- [ ] Write `brassclaw.service` systemd unit template (§7.1/§7.2/§7.3 including hardening directives)
- [ ] Update `AGENTS.md` Database Rules section (retire dual-backend mandate — §0a)
- [ ] **Update `crates/brassclaw_interceptor/src/store.rs` module-level doc comment** — the current comment says "Implementations must support both PostgreSQL and libSQL"; this contradicts Phase 0a's retirement of the dual-backend mandate; update to state that `PgInterceptorStore` is the sole durable implementation and `NoopInterceptorStore` is retained for tests/noop mode only
- [ ] **Purge stale v1 `src/` sections from `CLAUDE.md`** — `src/db/`, `src/channels/`,
      `src/agent/`, `src/workspace/`, `src/sandbox/`, `src/registry/`, `src/tunnel/`
      all describe code removed in Phase 6 that must not mislead new contributors
- [ ] Update `CLAUDE.md` env var table: document the two-tier model (bootstrap tier 6 fixed vars + operator-trusted env tier data-driven); remove retired `DATABASE_BACKEND`/`LIBSQL_*`/`LLM_*`/`GOOGLE_CLIENT_ID` vars
- [ ] Write operator guide covering: prerequisites (§7.0), fresh-install sequence (§7.1),
      upgrade sequence (§7.2), `master.key` ownership requirements, `master.key` DR backup
      mandate (§11), CLI-only users and `brassclaw maintenance prune-old-data` (§4.21),
      `rewrap` vs `rotate` distinction (§4.4)
- [ ] Update all per-crate `CLAUDE.md`/`AGENTS.md` spec files
- [ ] Update `CHANGELOG.md`
- [ ] Add architecture test: no `std::fs::read_to_string` / `File::open` in any
      non-migration production path

### Phase 10 — Integration tests and E2E
- [ ] Integration test: full boot cycle from scratch (embedded PG starts, wizard runs,
      agent serves a turn, graceful shutdown stops PG — including explicit `shutdown()`)
- [ ] Integration test: restart resumes existing state from Postgres
- [ ] Integration test: `BRASSCLAW_PG_URL` override (no embedded PG spawned)
- [ ] Integration test: SIGKILL → restart → orphaned-server detection and reuse
- [ ] E2E: provider add/edit/delete via WebUI persists across restart
- [ ] **Hardened-unit integration test (gate migration release):** embedded PG starts
      and serves a query under the §7 hardening directives
      (`MemoryDenyWriteExecute=yes`, `SystemCallFilter=@system-service`,
      `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX`). Validates that
      `jit=off` in `postgresql.conf` is sufficient to prevent the MDWE JIT crash.
      This test must be green before the migration release ships.
- [ ] Integration test: `brassclaw config get <key>` against a running `brassclaw serve`
      does not stop embedded PG (conditional-shutdown rule, §6.4 step 4).

---

## 11. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| `postgresql_embedded` download fails in offline/air-gapped env | Detect no-network at startup; print: "set `BRASSCLAW_PG_URL` to an external Postgres". Ship a `--download-pg` CLI helper that pre-caches the binary. |
| Supply-chain: zonky binary compromise | Compiled-in SHA-256 per version (§2.2); `POSTGRESQL_VERSION` env override suppressed; production deployments use `BRASSCLAW_PG_URL` to an operator-managed PG. |
| Large binary download on first run (~40 MB) | Cached in `$REBORN_HOME/postgres/bin/` after the first download. Progress bar shown. |
| Embedded PG orphaned after SIGKILL | On next start: check `postmaster.pid` liveness (kill -0). If alive, reuse the server. If PID is dead, remove stale PID file and restart (§2.2). |
| Port 5434 already in use | TCP-probe at startup; fail: "port 5434 in use — set `BRASSCLAW_PG_URL` or `BRASSCLAW_EMBEDDED_PG_PORT`". |
| PG log dir fills disk | Log rotation configured in `postgresql.conf`: 50 MB cap, daily rotation (§2.2). |
| Pool still open when `Drop` tries to stop PG | `managed_pg.shutdown()` called explicitly after pool close; `Drop` is last-resort only (§2.2, §5.5). |
| Data migration from libSQL loses rows | `migrate-from-libsql` default-on in upgrade release (§9.1); upsert-or-ignore; original `.db` renamed, not deleted; fail-loud if feature is off (§8.1). |
| `boot.initialized` set on fresh install, wizard skipped | Step 8 is conditional: only set if a source artifact was found (§8.1). |
| `config.toml` edited by operators who don't know about the change | `brassclaw config import < config.toml` import command; doc the new workflow. |
| Architecture tests break during libSQL removal | Remove in Phase 6 explicitly; add Postgres-boundary replacements. |
| `brassclaw_reborn_config` boundary test broken | `db_config.rs` lives in `brassclaw_reborn_composition` (§5.4/§5.5); boundary test unchanged. |
| Production headless boot: no passphrase source | `BRASSCLAW_SECRETS_PASSPHRASE_FILE` (bootstrap tier) required for unattended boot. `keychain` strategy requires a desktop session. See §4.4 full strategy table and §7 first-boot sequences. |
| Interactive wizard hangs under systemd | `brassclaw serve` checks `isatty(stdin)` before launching wizard; fails with clear error if not a TTY (§6.1). Use §7.2 upgrade or §7.1 fresh-install sequence instead. |
| Dual-backend AGENTS.md rule violated | Explicitly documented in §0a; rule rewrite is a first-class Phase 9 deliverable requiring sign-off. |
| PG JIT crashes under `MemoryDenyWriteExecute=yes` | `jit=off` set in `postgresql.conf` (§2.2); hardened-unit integration test in Phase 10 gates the migration release. |
| `config init` on upgrade clobbers migrated config | §7 documents fresh-install (§7.1) vs upgrade (§7.2) sequences explicitly. Upgrade sequence omits `config init`. |
| `rewrap` on upgrade fails: `brassclaw_secrets_master` does not exist | `rewrap` starts embedded PG and runs schema migrations (§6.4) before writing; table exists before `serve` ever starts. |
| `rewrap` generates new key on upgrade → migrated secrets undecryptable | Key-source rule (§4.4): `rewrap` checks `.reborn-local-dev-secrets-master-key` first, then `.secrets-master-key`; fails closed if neither found but encrypted rows exist. Upgrade-flow decryption integration test in Phase 7 gates release. |
| Concurrent CLI (`config get`) crashes `serve` via conditional PG shutdown | §6.4 step 4: CLI shuts down PG only if it started it; if a live postmaster is detected (from `serve`), leave it running. Phase 10 integration test gates release. |
| Passphrase change (`rewrap`) fails: old passphrase unavailable in interactive shell | §4.4: use `--old-passphrase-file=<path>` for shell invocation; `BRASSCLAW_SECRETS_PASSPHRASE_FILE` / `$CREDENTIALS_DIRECTORY` are the systemd-injected fallbacks. Document in passphrase-rotation runbook (Phase 9). |
| `rewrap` tenant ≠ `boot_tenant` → boot failure + orphaned ciphertext (R6-MH1) | `rewrap` resolves `tenant_id` via: `--tenant` flag → `config.toml` `identity.tenant` → `brassclaw_config` DB → `"default"`. §7.1/§7.2 runbook commands pass `--tenant` explicitly. Phase 7 non-default-tenant upgrade integration test gates release. Since `rewrap` zeros the raw key file on success, a tenant mismatch combined with a missing `brassclaw_secrets_master` row causes a non-recoverable boot failure — never suppress the fail-closed exit. |
| master.key is root-owned → EACCES at per-boot unwrap | All setup steps that write `master.key` must run as the service user (`sudo -u brassclaw`) — see §7.0/§7.1/§7.2. |
| master.key lost in disaster recovery | The passphrase file (`master.key`) **must be in the operator's DR backup set**, separately from the Postgres data directory. Loss of `master.key` without a backup means all `brassclaw_secrets` rows (OAuth tokens, credential-broker creds) are permanently unrecoverable. Document in operator guide (Phase 9). |
| Passphrase-file wrap provides no extra protection on same-host embedded PG | Wrapping earns its threat-model value when PG is remote (`BRASSCLAW_PG_URL`): a DB-only breach (backup leak, remote PG compromise) cannot decrypt secrets without `master.key` on the app host. On single-host embedded PG, the security model is equivalent to a raw 0600 key file. Operators who require stronger isolation should use `BRASSCLAW_PG_URL`. |
| Chunk embedding system wiring activation surfaces latent bugs in the existing (currently-unwired) indexer path (R17-M) | The existing `ChunkingMemoryDocumentIndexer` + `EmbeddingProvider` infrastructure has never been driven end-to-end in the `memory_write` / `memory_search` tool dispatch path (`build_backend()` creates the indexer without an embedding provider; `dispatch_search()` forces `.with_vector(false)`). Revision 17 activates it. Mitigated by the Phase 4 integration test that drives `memory_write` → `memory_search` with vector enabled end-to-end (asserts chunk rows + vector similarity retrieval). |
| Chunk-cascade retention sweep leaves orphan chunk subtree if transaction rolls back mid-deletion (R17) | The retention sweep deletes chunk rows under `<source_ref>/*.chunks/` before deleting the `brassclaw_memory_chat_records` row, in a single transaction. If the transaction rolls back, both deletions are undone — no orphan. If the chunk deletion succeeds but the Path A deletion fails (should not happen in the same transaction, but defensive), the sweep logs `warn!` and retries on the next run. The `source_ref` partial index (§4.29) makes the cascade lookup fast. |
| Embedding model dimension change invalidates existing chunk vectors (R17) | If the operator re-assigns the `embedding` role to a provider whose model has a different vector dimension, the existing chunk rows (with the old dimension) become invalid for vector search. FTS search still works (the `content` indexed key is unaffected). The `backfill-embeddings` command (§8.1 step 10) handles this by deleting the old chunk subtree and re-indexing with the new dimension via `index_content`. The UI should warn the operator when a dimension change is detected (compare `default_dimension_for_model(new_model)` against the existing chunk rows' dimension). |
| `brassclaw_embeddings` crate refactor breaks downstream non-memory callers (R17) | The crate retains `EmbeddingProvider` trait + `EmbeddingError` + `CachedEmbeddingProvider` + `url_check` + `default_dimension_for_model` (the public surface). The removed `create_provider()` factory + `EmbeddingsConfig` are only called from `src/app.rs`, `src/cli/mod.rs`, `src/cli/doctor.rs::check_embeddings` (per `crates/brassclaw_embeddings/AGENTS.md`). Phase 4 task explicitly updates these three call sites. The `MockEmbeddings` test double is retained for tests. |

---

## 12. Files Modified Summary (anticipated)

| File / path | Change |
|---|---|
| `Cargo.toml` (workspace) | Add `brassclaw_embedded_postgres`, `brassclaw_pg`; add `migrate-from-libsql` (default-on for upgrade release); remove `brassclaw_hooks_libsql`; rebase `replay`, remove `import` (§9.2) |
| `crates/brassclaw_embedded_postgres/` | **New crate** |
| `crates/brassclaw_pg/` | **New crate** (migrations + pool) |
| `crates/brassclaw_hooks_pg/` | ✅ Renamed from `brassclaw_hooks_postgres`; `#[cfg(feature = "postgres")]` declarations stripped from `lib.rs`; deps made unconditional |
| `crates/brassclaw_hooks_libsql/` | ✅ **Deleted** |
| `crates/brassclaw_hooks_parity/` | ✅ **Adversarial+parity tests ported into `brassclaw_hooks_pg/tests/`, crate deleted** (see §5.2) |
| `crates/brassclaw_reborn_config/src/config_file.rs` | Remove `write()`; retain `load()` behind `migrate-from-libsql` (needed by §8.1 step 3 + §4.4 rewrap step 2 — removed in the same next release that drops `migrate-from-libsql`, per §5.4) |
| `crates/brassclaw_reborn_config/src/home.rs` | Remove `config_file_path()`, `providers_file_path()`, `sempai_provider_file_path()` |
| `crates/brassclaw_reborn_config/Cargo.toml` | Remove `toml_edit`, `fs4`, `tempfile`. **Keep `toml`**. Do NOT add `deadpool-postgres`. |
| `crates/brassclaw_reborn_composition/src/db_config.rs` | **New file** — `load_config_snapshot`, `save_config_key(…, caller: ConfigWriteContext)`, `ConfigWriteContext` enum; `*_env` key write-gate (§5.5, §1c) |
| `crates/brassclaw_reborn_composition/src/factory.rs` | Remove libSQL/file branches; single Postgres path; explicit `shutdown()` before pool drop |
| `crates/brassclaw_reborn_composition/src/provider_repo.rs` | DB-backed rewrite |
| `crates/brassclaw_reborn_composition/src/llm_config_service.rs` | Use `db_config::load_config_snapshot`; remove file paths |
| `crates/brassclaw_reborn_composition/Cargo.toml` | Remove `libsql` dep **and** update `[features] default` to remove `libsql` (crate currently defaults to `libsql`, not `postgres` — H4); add `brassclaw_embedded_postgres` |
| `crates/brassclaw_reborn_cli/Cargo.toml` | Update `[features] default` — remove `libsql`; crate currently has `default = ["root-llm-provider", "libsql"]` |
| `crates/brassclaw_reborn_cli/src/commands/` | Add first-run wizard; rewrite `config init` |
| `crates/brassclaw_filesystem/` | Remove libSQL backend; add Postgres fallback backend |
| `crates/brassclaw_run_state/` | Add `PgRunStateStore`; delegate approvals to `brassclaw_approvals` |
| `crates/brassclaw_approvals/` | Add `PgApprovalRequestStore` (dedicated crate — see §5.3 ⬡) |
| `crates/brassclaw_turns/` | Add `PgTurnStateStore`, `PgLoopCheckpointStore`; remove libSQL impls |
| `crates/brassclaw_loop_support/` | Add `PgCheckpointStateStore`; remove libSQL impl (see §5.3 ⬡) |
| `crates/brassclaw_threads/` | Add `PgSessionThreadService`; remove libSQL impl |
| `crates/brassclaw_authorization/` | Add `PgCapabilityLeaseStore`; remove libSQL impl |
| `crates/brassclaw_resources/` | Add `PgResourceGovernorStore` + `PgBudgetGateStore`; remove libSQL impl |
| `crates/brassclaw_reborn_identity/` | Add `PgRebornIdentityStore` implementing `RebornIdentityResolver` (no `factory.rs` in this crate; wiring is in `brassclaw_reborn_composition/src/factory.rs`) |
| `crates/brassclaw_processes/` | Add `PgProcessStore`, `PgProcessResultStore` (with `tenant_id`); remove libSQL impl |
| `crates/brassclaw_extensions/` | Add `PgExtensionInstallationStore` (moved from composition — see §5.3 ⬡) |
| `crates/brassclaw_reborn_event_store/` | Add `PgDurableEventLog`, `PgDurableAuditLog` (verified home of `FilesystemDurableEventLog`/`FilesystemDurableAuditLog`) |
| `crates/brassclaw_secrets/` | Add `PgSecretStore`; add `PgCredentialBroker` implementing **both** `CredentialAccountStore` and `CredentialSessionStore`; add `key_version` support |
| `crates/brassclaw_product_workflow/` | Replace `SqliteSafetyConfigStore` with `PgSafetyConfigStore` (natural-key UNIQUE) |
| `crates/brassclaw_triggers/` | Promote `PostgresTriggerRepository` to unconditional; remove `LibSqlTriggerRepository`; add `PgLocalTriggerAccessStore`; remove libsql feature gate |
| `crates/brassclaw_conversations/` | Add `PgConversationStateStore`; remove `FilesystemConversationStateStore` |
| `crates/brassclaw_outbound/` | Add `PgOutboundStateStore` implementing `OutboundStateStore` + `CommunicationPreferenceRepository`; remove `FilesystemOutboundStateStore` |
| `crates/brassclaw_reborn/src/subagent/goal_store.rs` | Add `PgSubagentGoalStore`; remove `FilesystemSubagentGoalStore`; strip `filesystem-goal-store` feature gate |
| `crates/brassclaw_reborn/src/local_trigger_access.rs` | Replace `RebornLibSqlLocalTriggerAccessStore` with `PgLocalTriggerAccessStore` |
| `crates/brassclaw_filesystem/` | Extend `PostgresRootFilesystem` to use sibling tables `brassclaw_root_filesystem_index_specs` + `brassclaw_root_filesystem_events` (§4.19) |
| `crates/brassclaw_architecture/` | Update boundary tests |
| `CLAUDE.md` | Update database section; purge stale v1 `src/` docs; update env var table |
| `AGENTS.md` | Retire dual-backend rule (§0a) |
| `CHANGELOG.md` | Entry for this migration |
| `crates/brassclaw_pg/Cargo.toml` | Add `pgvector` dependency (§3) |
| `crates/brassclaw_pg/migrations/V000__shared_triggers.sql` | Add `CREATE EXTENSION IF NOT EXISTS vector;` as the first statement (§4.20, §3) |
| `crates/brassclaw_pg/migrations/V025__memory_chat_records.sql` | **New migration** — `brassclaw_memory_chat_records` table with generated tsvector, GIN FTS + tags indexes, `run_id` partial index, `updated_at` trigger, `source_ref` column + partial index, `forensic_packet_id` column (§4.29, Upgrade A + revision 17 `source_ref` link + revision 23 `run_id` index) |
| `crates/brassclaw_embedded_postgres/src/initdb.rs` | Install pgvector shared library into embedded PG data directory after `initdb` (§3 pgvector dependency note) |
| `crates/brassclaw_memory/src/indexer.rs` | **Trait extension (revision 17)** — add `index_content(scope, source_ref, content, chat_record_id)` method to `MemoryDocumentIndexer` trait (full signature: `async fn index_content(&self, scope: &MemoryDocumentScope, source_ref: &str, content: &str, chat_record_id: Option<&str>)`); implement on `ChunkingMemoryDocumentIndexer` (file-less chunk creation, §4.30.1); preserve existing degrade-to-text-only behaviour on embedding API error |
| `crates/brassclaw_memory/src/repo/filesystem.rs` | **New indexed key (revision 17)** — add `fs_keys::CHAT_RECORD_ID = "chat_record_id"` constant; store `chat_record_id` as an indexed key on chunk rows written by `index_content` so the chunk system can join back to the Path A row |
| `crates/brassclaw_embeddings/` | **Crate refactor (revision 17)** — REMOVE `EmbeddingsConfig`, `create_provider()`, concrete HTTP provider impls (`OpenAiEmbeddings` / `NearAiEmbeddings` / `OllamaEmbeddings` / `BedrockEmbeddings`), binary-side resolver `src/config/embeddings.rs::resolve_embeddings_config`, `Workspace::with_embeddings_cached` / `with_embeddings_uncached` wiring; RETAIN `EmbeddingProvider` trait + `EmbeddingError`, `CachedEmbeddingProvider` + `EmbeddingCacheConfig`, `url_check::check_base_url`, `default_dimension_for_model`, `MockEmbeddings`; update `crates/brassclaw_embeddings/AGENTS.md` to reflect the refactor (§3) |
| `crates/brassclaw_host_runtime/src/first_party_tools/memory.rs` | **`build_backend()` re-wiring (revision 17)** — resolve `embedding`-role provider at composition startup; if active, wire `EmbeddingRoleAdapter` into `ChunkingMemoryDocumentIndexer::with_embedding_provider(...)` + `RepositoryMemoryBackend::with_embedding_provider(...)` + set `services.embedding_active = true`; if not active, leave indexer without provider (current behaviour) + set `services.embedding_active = false`; update `dispatch_search()` to use `.with_vector(services.embedding_active)` instead of `.with_vector(false)`; add `embedding_active: bool` + `embedding_provider: Option<Arc<dyn brassclaw_memory::EmbeddingProvider>>` to `MemoryServices` (§4.30.3, §6.2a) |
| `crates/brassclaw_reborn_composition/src/embedding_role_adapter.rs` | **New file (revision 17)** — `EmbeddingRoleAdapter` that resolves the `embedding`-role provider from `brassclaw_config` + `brassclaw_llm_providers` at composition startup, constructs an HTTP client calling the provider's embedding endpoint, runs `url_check::check_base_url`, wraps in `CachedEmbeddingProvider`, exposes `Arc<dyn brassclaw_memory::EmbeddingProvider>` (§3) |
| `crates/brassclaw_reborn_composition/src/memory/` (or `crates/brassclaw_memory/`) | **New module / crate** — `PgChatMemoryRecordStore` (Path A, unconditional); wired into memory-write path (§4.29, §5.3). No `PgMemoryEmbeddingStore` (revision 17 — Path B reuses the existing `ChunkingMemoryDocumentIndexer` via `index_content`, §4.30) |
| `crates/brassclaw_reborn_composition/src/factory.rs` | Wire `PgChatMemoryRecordStore` + `EmbeddingRoleAdapter`; resolve `embedding.provider_id` / `embedding.model` from `brassclaw_config` at startup alongside `kohai` / `sempai` resolution; wire the adapter into `build_backend()` (Phase 4 three-role preconfiguration, revision 17) |
| `crates/brassclaw_webui_v2/` and/or `crates/brassclaw_webui_v2_static/` | **"use for embedding" button** — add third provider-role action button (Upgrade B, §6.2a); new provider-role endpoint; active/inactive highlight state; "restart required" hint when the `embedding` role is re-assigned |
| `crates/brassclaw_reborn_webui_ingress/` (or equivalent API surface) | **New provider-role endpoint** — `PUT /providers/{id}/role/embedding` (or equivalent REST path) that writes `embedding.provider_id` + `embedding.model` to `brassclaw_config` via `save_config_key(…, ConfigWriteContext::Operator)` (§6.2a) |
| `crates/brassclaw_reborn_cli/src/commands/maintenance.rs` (or equivalent) | Add `brassclaw maintenance backfill-embeddings` subcommand (§7.4, §8.1 step 10); reads `brassclaw_memory_chat_records` rows with `source_ref = NULL` or chunks with `embedding = NULL`, calls `indexer.index_content(...)`; idempotent; handles dimension change |
| `src/app.rs`, `src/cli/mod.rs`, `src/cli/doctor.rs::check_embeddings` | **Update call sites (revision 17)** — remove `brassclaw_embeddings::create_provider` + `Workspace::with_embeddings_cached` / `with_embeddings_uncached` calls; embedding wiring now goes through `EmbeddingRoleAdapter` in `brassclaw_reborn_composition`; `check_embeddings` reports the `embedding`-role provider status from `brassclaw_config` instead of `EmbeddingsConfig` |

---

## Phase 11 — Remove `BRASSCLAW_REBORN_PROFILE` (Path B: three independent knobs)

> **Sequencing:** Phase 11 slots after Phases 1–6 (schema, embedded PG, config
> table, migration, libSQL removal). The CLI boot path changes belong here
> alongside Phase 8 (file-based config removal). Phases 7–10 can proceed
> independently of Phase 11.
>
> **Compatibility:** All security properties are preserved. `RuntimeProfile`,
> `brassclaw_runtime_policy::resolve()`, the yolo disclosure gate, org-policy
> max-profile ceiling, and the `--confirm-host-access` CLI flag are unchanged.
> The §4.4 secret ceremony (Argon2id wrapping, raw-key-on-disk fallback, rewrap
> command, key-source invariant) is unchanged.

### Architecture of the two systems being merged

**Layer 1 — `RebornProfile` / `RebornCompositionProfile` (TO BE REMOVED)**

```
BRASSCLAW_REBORN_PROFILE env var
        ↓
RebornProfile (4 variants)               crates/brassclaw_reborn_config/src/profile.rs
        ↓ composition_profile()          crates/brassclaw_reborn_cli/src/runtime/mod.rs:541
        ↓
RebornCompositionProfile (5 variants)    crates/brassclaw_reborn_composition/src/profile.rs
        ↓ match arm in build_reborn_services()   crates/brassclaw_reborn_composition/src/factory.rs:531
        ├── Disabled                       → RebornServices::disabled()
        ├── LocalDev | LocalDevYolo        → build_local_dev()
        └── Production | MigrationDryRun   → build_production_shaped()
```

**Layer 2 — `RuntimeProfile` (TO BE KEPT ENTIRELY, elevated to primary)**

```
BRASSCLAW_RUNTIME_PROFILE env var (NEW) → RuntimeProfile (12 variants)
        ↓ brassclaw_runtime_policy::resolve()
        ↓ inputs: DeploymentMode, RuntimeProfile, yolo_disclosure_acknowledged, org_policy
        ↓
EffectiveRuntimePolicy          crates/brassclaw_host_api/src/runtime_policy.rs
```

### Fail-closed guard (replaces profile-branched storage guard)

```
if RuntimeProfile.is_local() == false AND BRASSCLAW_PG_URL is unset:
    fail: "Non-local runtime profile '{profile}' requires BRASSCLAW_PG_URL.
           Embedded Postgres is for single-host local deployments only.
           Set BRASSCLAW_PG_URL to an external Postgres URL or use a local
           runtime profile (local_dev, local_safe, local_yolo)."
```

The `is_local()` predicate already exists at
`crates/brassclaw_host_api/src/runtime_policy.rs:212`. Local variants:
`LocalSafe`, `LocalDev`, `LocalYolo`. All other variants are non-local and
require `BRASSCLAW_PG_URL`.

**Warning (not fail) for non-local + no passphrase file:** If
`RuntimeProfile.is_local() == false` AND `BRASSCLAW_SECRETS_PASSPHRASE_FILE`
is unset AND `algorithm = 'raw-key-on-disk'`: warn that the operator should
consider running `rewrap --strategy passphrase-file=<path>` for
defense-in-depth. Not a hard requirement.

### Deprecation shim (old → new knob translation)

During Phase 11a, `BRASSCLAW_REBORN_PROFILE` is still accepted but emits a
deprecation warning and translates to the new knobs:

| Old profile value | `BRASSCLAW_RUNTIME_PROFILE` | `BRASSCLAW_PG_URL` | Behavior |
|---|---|---|---|
| `local-dev` | `local_dev` (default) | unchanged | identical |
| `local-dev-yolo` | `local_yolo` | unchanged | identical (yolo gate enforced by resolver) |
| `production` | unchanged (default `local_dev`) | **not required** — embedded PG is durable | boots + deprecation warning + runtime-profile warning |
| `migration-dry-run` | n/a | n/a | **error:** `"BRASSCLAW_REBORN_PROFILE=migration-dry-run is removed. Use 'brassclaw migrate --dry-run' instead."` |

When `production` is detected AND `BRASSCLAW_RUNTIME_PROFILE` is not set,
emit this warning:
```
"WARNING: BRASSCLAW_REBORN_PROFILE=production is deprecated and no longer
implies a security policy. Defaulting to BRASSCLAW_RUNTIME_PROFILE=local_dev.
Set BRASSCLAW_RUNTIME_PROFILE explicitly for your deployment tier."
```

### Phase 11a — Add new knobs, keep old (no breakage)

1. Add `BRASSCLAW_RUNTIME_PROFILE` parsing in
   `crates/brassclaw_reborn_cli/src/runtime/mod.rs`. When set, call
   `brassclaw_runtime_policy::resolve()` directly. When absent, derive policy
   from existing `effective_profile()` shim (unchanged behavior).
2. Add `BRASSCLAW_PG_URL` parsing (if not already present from Phase 2).
3. Add the fail-closed guard: if `BRASSCLAW_RUNTIME_PROFILE` resolves to a
   non-local profile AND `BRASSCLAW_PG_URL` is unset → fail with the message
   in the fail-closed section above. Only fires when `BRASSCLAW_RUNTIME_PROFILE`
   is explicitly set to a non-local value.
4. Add the ceremony-consistency check at boot: read
   `brassclaw_secrets_master.algorithm`, compare to
   `BRASSCLAW_SECRETS_PASSPHRASE_FILE` presence, fail/warn per the §4.4
   ceremony derivation. **Hard prerequisite:** Phase 3 must have landed
   (creates `brassclaw_secrets_master` table). Skipped on fresh install before
   wizard runs (ordering invariant).
5. Add `brassclaw migrate --dry-run` CLI flag.

### Phase 11b — Deprecate old env var

1. When `BRASSCLAW_REBORN_PROFILE` is present, print deprecation warning.
2. Translate old profile values per the deprecation shim table above.
3. When both vars are set, `BRASSCLAW_RUNTIME_PROFILE` wins.
4. Repurpose `profile list` command to show available `RuntimeProfile` values.
5. Update `AGENTS.md` Key Environment Variables table.

### Phase 11c — Remove old boot profile code

Ordered steps (compiler guides you at each step):

1. **Delete** `crates/brassclaw_reborn_config/src/profile.rs`
2. **Remove** `pub mod profile;` from `crates/brassclaw_reborn_config/src/lib.rs`
3. **Remove** `RebornCompositionProfile::LocalDevYolo`, `Production`,
   `MigrationDryRun` from `crates/brassclaw_reborn_composition/src/profile.rs`
3a. **Remove `to_event_store_profile()`** from `profile.rs` and inline
    the constant `brassclaw_reborn_event_store::RebornProfile::LocalDev`
    directly at the call site in `factory.rs:2536` (§4.31 sequencing note
    applies — if Phase 4 landed first, the call site is already gone).
4. **Remove** the `Production | MigrationDryRun → build_production_shaped()`
   branch from `build_reborn_services()` in `factory.rs`
5. **Delete** `build_production_shaped()` function
6. **Remove** `profile: RebornCompositionProfile` parameters from
   `RebornBuildInput` constructors (or collapse to `Active`/`Disabled`)
7. **Remove** `composition_profile()` and `effective_profile()` from `mod.rs`;
   remove `print_runtime_banner()`'s `profile:` line (line 155); remove
   `use brassclaw_reborn_config::{REBORN_PROFILE_ENV, RebornProfile}` import
7a. **Remove** `profile: RebornProfile` field from `RebornBootConfig` in
    `crates/brassclaw_reborn_config/src/boot.rs` — remove `profile()` accessor,
    `from_env()` reading of `REBORN_PROFILE_ENV`, `resolve_from_env_parts()`
    `profile` parameter, and `into_parts()` profile return.
7b. **Remove** `boot.profile` field from `RebornConfigFile` boot struct in
    `crates/brassclaw_reborn_config/src/config_file.rs` (line ~671 validation +
    line ~1102 read). `deny_unknown_fields` will reject any operator TOML that
    still has `profile = "..."` — add migration note in `config init`.
7c. **Remove** `profile: RebornProfile` field from `RebornDoctorReport` in
    `crates/brassclaw_reborn_config/src/doctor.rs` — remove `profile()` accessor.
    Update CLI consumers: `commands/doctor.rs:16` and `commands/config/mod.rs:53`
    — remove the `println!("profile: {}", report.profile())` lines or replace
    with `println!("runtime_profile: {}", ...)`.
7d. **Remove** `println!("profile: {}", config.profile())` from
    `crates/brassclaw_reborn_cli/src/commands/run.rs:54`.
7e. **Update** `crates/brassclaw_reborn_cli/src/commands/skills.rs` — remove
    `build_skill_list_config()`'s call to `effective_profile()` (line 94) and
    the `match profile { ... Production | MigrationDryRun => bail!() }`
    rejection (lines 95-102); remove `profile: RebornProfile` field from
    `SkillListConfig` (line 89); remove `"profile"` JSON field in skill output
    (line 53).
7f. **Update** `crates/brassclaw_reborn_cli/src/commands/config/init.rs` —
    remove `profile = "local-dev"` (line 149) and the comment (line 146);
    add migration comment:
    `# [boot].profile removed — use BRASSCLAW_RUNTIME_PROFILE env var instead.`
7g. **Remove or rewrite** `crates/brassclaw_reborn_config/tests/profile_contract.rs`.
    Update `crates/brassclaw_reborn_config/tests/doctor_contract.rs:16` to not
    assert on `report.profile()`. Update all `mod.rs` test callers of
    `resolve_from_env_parts()` (lines 802, 835, 857, 881, 930, 966, 1005,
    1129, 1158) to not pass the `profile` parameter.
8. **Remove** `requires_production_shape()` from `RebornCompositionProfile`
9. **Update** `crates/brassclaw_reborn_composition/src/local_runtime_profile.rs`
   — migrate all four public functions + private helper + error type from
   `RebornCompositionProfile` to `RuntimeProfile`:
   - `local_runtime_build_input(profile: RebornCompositionProfile, …)` (line 24)
     → `runtime_profile: RuntimeProfile`
   - `local_runtime_build_input_with_options(profile, …)` (line 39)
     → `runtime_profile: RuntimeProfile`
   - `local_dev_runtime_policy()` (line 53) — delete and inline at call sites
     with direct `brassclaw_runtime_policy::resolve(...)` calls. Update test
     call site at `local_dev_host_tests.rs:222`.
   - `local_dev_yolo_runtime_policy(confirm_host_access: bool)` (line 68) —
     delete and inline at call sites. Update test call site at
     `local_dev_host_tests.rs:218`.
   - `local_runtime_policy(profile: RebornCompositionProfile, …)` (line 85,
     private) → `runtime_profile: RuntimeProfile`
   - `RebornLocalRuntimeProfileError::UnsupportedProfile` (line 12) →
     **remove the variant** (no longer reachable after the migration).
10. **Rename** `crates/brassclaw_reborn_cli/src/commands/profile.rs` →
    `runtime_profile.rs`; rename `ProfileCommand` → `RuntimeProfileCommand`,
    `ProfileSubcommand` → `RuntimeProfileSubcommand`,
    `ProfileListCommand` → `RuntimeProfileListCommand`; update CLI subcommand
    registration from `profile` to `runtime-profile`; list 12 `RuntimeProfile`
    variants + the three new env vars.
11. **Update smoke tests** — update
    `profile_list_shows_supported_profiles_without_reborn_home` and
    `profile_list_json_is_stable_and_does_not_resolve_reborn_home` to use
    `runtime-profile list` and assert the 12 `RuntimeProfile` variants;
    replace `skills_list_rejects_unsupported_profiles` with a test that sets
    `BRASSCLAW_RUNTIME_PROFILE=hosted_safe` + no `BRASSCLAW_PG_URL` → fail-closed
    error.
12. **Run** `cargo clippy --all --benches --tests --examples --all-features -- -D warnings`
    — must be zero warnings.
13. **Run** `cargo test` — all tests must pass.

### Phase 11d — Files to change

#### Files to Delete

| File | Reason |
|------|--------|
| `crates/brassclaw_reborn_config/src/profile.rs` | `RebornProfile` enum and env-var parsing |

#### Files to Modify

| File | Change |
|------|--------|
| `crates/brassclaw_reborn_config/src/lib.rs` | Remove `pub mod profile;` and re-exports of `RebornProfile`, `REBORN_PROFILE_ENV` |
| `crates/brassclaw_reborn_config/src/boot.rs` | Remove `profile: RebornProfile` field, `profile()` accessor, `from_env()` reading of `REBORN_PROFILE_ENV`, `resolve_from_env_parts()` `profile` parameter |
| `crates/brassclaw_reborn_config/src/config_file.rs` | Remove `boot.profile` field (line ~671 validation + line ~1102 read) |
| `crates/brassclaw_reborn_config/src/doctor.rs` | Remove `profile: RebornProfile` field from `RebornDoctorReport`; remove `profile()` accessor |
| `crates/brassclaw_reborn_config/tests/profile_contract.rs` | Remove or rewrite — tests `RebornProfile` parsing; obsolete once `RebornProfile` is deleted |
| `crates/brassclaw_reborn_config/tests/doctor_contract.rs` | Remove `report.profile() == RebornProfile::MigrationDryRun` assertion (line 16) |
| `crates/brassclaw_reborn_composition/src/profile.rs` | Collapse to `{ Disabled, Active }`; remove `requires_production_shape()`; keep `to_event_store_profile()` as stub until §4.31 cleanup |
| `crates/brassclaw_reborn_composition/src/factory.rs` | Remove `Production | MigrationDryRun → build_production_shaped()` branch; delete `build_production_shaped()` |
| `crates/brassclaw_reborn_composition/src/input.rs` | Remove `profile: RebornCompositionProfile` param from constructors |
| `crates/brassclaw_reborn_cli/src/runtime/mod.rs` | Remove `composition_profile()`, `effective_profile()`; add `runtime_profile_from_env()`; add fail-closed guard; add ceremony-consistency check |
| `crates/brassclaw_reborn_cli/src/commands/profile.rs` → `runtime_profile.rs` | Rename; rename types; update CLI subcommand registration |
| `crates/brassclaw_reborn_cli/src/commands/skills.rs` | Remove `effective_profile()` call (line 94) and profile-based rejection (lines 95-102); remove `profile` field from `SkillListConfig` and JSON output |
| `crates/brassclaw_reborn_cli/src/commands/run.rs` | Remove `println!("profile: {}", config.profile())` (line 54) |
| `crates/brassclaw_reborn_cli/src/commands/doctor.rs` | Remove `println!("profile: {}", report.profile())` (line 16) |
| `crates/brassclaw_reborn_cli/src/commands/config/mod.rs` | Remove `println!("profile: {}", report.profile())` (line 53) |
| `crates/brassclaw_reborn_cli/src/commands/config/init.rs` | Remove `profile = "local-dev"` (line 149) and comment (line 146); add migration comment |
| `crates/brassclaw_reborn_event_store/src/lib.rs` | Remove `profile: RebornProfile` parameter from `build_reborn_event_stores()` (line 149); remove `Production` branches (lines 154, 166, 195); remove `Production` variant from event store's own `RebornProfile` enum (line 91) — see §4.31 |
| `crates/brassclaw_reborn_composition/src/local_runtime_profile.rs` | Migrate all four public functions + error type from `RebornCompositionProfile` to `RuntimeProfile` — see Phase 11c step 9 |
| `AGENTS.md` | Update Key Environment Variables table: remove `BRASSCLAW_REBORN_PROFILE`; add `BRASSCLAW_RUNTIME_PROFILE` |

### Phase 11 checklist

- [ ] Add `BRASSCLAW_RUNTIME_PROFILE` env var parsing (Phase 11a step 1)
- [ ] Add fail-closed guard: `!is_local() && pg_url.is_none()` → fail with message (Phase 11a step 3)
- [ ] Add ceremony-consistency check at boot (§4.4 algorithm vs passphrase-file; Phase 11a step 4) — requires Phase 3 to have landed
- [ ] Add `brassclaw migrate --dry-run` CLI flag (Phase 11a step 5)
- [ ] Add deprecation warning and translation for `BRASSCLAW_REBORN_PROFILE` (Phase 11b)
- [ ] Repurpose `profile list` → `runtime-profile list` showing 12 variants (Phase 11b step 4)
- [ ] Delete `crates/brassclaw_reborn_config/src/profile.rs` (Phase 11c step 1)
- [ ] Remove `RebornCompositionProfile` extra variants; collapse factory match (Phase 11c steps 3–5)
- [ ] Remove `profile` fields from `RebornBootConfig`, `RebornConfigFile`, `RebornDoctorReport` (Phase 11c steps 7a–7c)
- [ ] Remove profile print lines from CLI commands (Phase 11c steps 7d–7e)
- [ ] Update `config/init.rs` template; remove or rewrite profile tests (Phase 11c steps 7f–7g)
- [ ] Migrate `local_runtime_profile.rs` functions to `RuntimeProfile` (Phase 11c step 9)
- [ ] Rename `profile.rs` → `runtime_profile.rs`; update CLI subcommand (Phase 11c step 10)
- [ ] Update smoke tests (Phase 11c step 11)
- [ ] `cargo clippy --all --benches --tests --examples --all-features -- -D warnings` — zero warnings
- [ ] `cargo test` — all tests pass
