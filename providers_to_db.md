# Plan: Move All Providers to the Database

> **Implementation status:** Steps 1–10 of the migration sequence are ✅ committed.
> Next: Step 11 (LlmConfigServiceError::CannotDeleteBuiltin) and beyond.

**Goal:** Remove the dual-source (compiled-in JSON + DB) provider architecture. Every
provider lives in `brassclaw_llm_providers`. The binary only seeds the table on first boot.
No file-based fallback is retained: DB is the exclusive runtime source of truth.

**Status:** Implementation in progress — steps marked ✅ are committed and pushed.

---

## Why This Is the Right Move

The hybrid architecture is the root of every provider-config bug so far:

- `build_snapshot` reads builtins from the embedded registry and custom providers from the
  DB in two separate passes. Builtin overlays written to `brassclaw_llm_providers` are never
  read back in the builtin pass.
- `build_overlay_definition` must preserve protocol/setup hints when a builtin is
  configured — complexity that disappears if the full definition is always in the DB.
- `RebornProviderAdmin` reads `config.toml` for the active provider while
  `set_provider_async` writes to `brassclaw_config`; the two diverge under overlay scenarios.
- Five methods in `llm_config_service.rs` carry `#[cfg(feature = "postgres")]` guards with
  file-based fallbacks that are never exercised in production and add dead complexity.

With all providers in the DB the code collapses to: **read from DB, write to DB**.

---

## Verified Codebase Facts

Every item below was read directly from the source.

| Fact | File:line | Evidence |
|------|-----------|----------|
| `providers.json` embedded via `include_str!("../../../providers.json")` | `registry.rs:446` | `fn builtin_provider_definitions()` |
| `ProviderDefinition` uses `#[serde(deny_unknown_fields)]` | `registry.rs:365` | JSONB `\|\|` merge would break deserialisation |
| Migrations V044–V046 are taken; next available: V047 | `crates/brassclaw_pg/migrations/` | `V046__reborn_component_tables_solution_columns.sql` |
| `brassclaw_llm_providers` has NO `is_builtin` column yet | `V002__llm_providers.sql` | Only: tenant_id, id, definition, created_at, updated_at, deleted_at |
| `pg_provider_repo` is `Option<Arc<PgProviderRepo>>` under `#[cfg(feature = "postgres")]` | `llm_config_service.rs:131–132` | builder `with_pg_provider_repo` |
| `RebornLlmConfigService::new()` always creates `repo: ProviderRepo` from a file path | `llm_config_service.rs:148–149` | `ProviderRepo::new(boot.home().path().join("providers.json"))` |
| `RebornLlmConfigService` has `repo: ProviderRepo` as a non-cfg struct field at line 107 | `llm_config_service.rs:107` | always present regardless of feature flags |
| `PgProviderRepo::upsert()` returns `Ok(bool)` meaning "was it an UPDATE (true) vs INSERT (false)" | `pg_provider_repo.rs:81–115` | queries EXISTS before upsert |
| `load_provider_by_id` already has a postgres-first path, file fallback | `llm_config_service.rs:867–885` | `#[cfg(feature = "postgres")] if let Some(pg_repo)` |
| `upsert_provider_definition` already has postgres-first path, file fallback | `llm_config_service.rs:888–903` | same pattern |
| `delete_provider_definition` already has postgres-first path, file fallback | `llm_config_service.rs:908–920` | same pattern |
| `probe_matches_persisted_provider` already has DB-first path, then builtin-registry fallback | `llm_config_service.rs:756–781` | the `else` branch calls `try_load_from_path(None)` |
| `build_sempai_provider` already has DB-first path, then builtin-registry fallback | `llm_config_service.rs:414–452` | `.or_else(|| brassclaw_llm::ProviderRegistry...` |
| `set_provider_async` postgres path is complete; non-postgres is no-op `let _ = (id, model);` | `llm_config_service.rs:794–837` | postgres block returns after DB write |
| `admin_list_async` calls `spawn_blocking(|| admin.list(None, true))` | `llm_config_service.rs:783–792` | uses `self.admin()` which needs `boot` field |
| `build_snapshot` reads builtins via `admin_list_async` then custom via `pg_repo.load()` | `llm_config_service.rs:454–689` | dual-pass loop |
| Tests at lines 1995–2052 and 2139–2165 assert on `ProviderRepo::new(...).load()` | confirmed | must be rewritten |
| Tests at lines 2090–2112 and 2114–2137 call `snapshot()` which calls `build_snapshot` which needs `admin_list_async` | confirmed | all call `new(boot, keys)` — must change |
| `read_role_sel_from_db_or_file` reads embedding from DB-first then legacy file fallback | `llm_config_service.rs:310–345` | `read_role_selection(file_path)` fallback |
| `provider_protocol_wire_name()` already exists at `provider_admin.rs:318–323` | confirmed | reusable for snapshot wire name |
| `resolve_pg_embedding_provider` calls `provider_repo.load()` (custom-only) | `factory.rs:~1990` | misses builtins |
| `runtime.rs` reads `context_window_tokens` from embedded registry only | `runtime.rs:1956–1966` | `try_load_from_path(None)` call |
| `runtime.rs` reads `cache_retention` from embedded registry as final fallback | `runtime.rs:1968–2003` | last `.or_else(|| try_load_from_path...` |
| `LlmConfigServiceError` has 5 variants; NO `CannotDeleteBuiltin` yet | `brassclaw_product_workflow/src/reborn_services/llm_config.rs:346–367` | must add |
| CLI `models.rs` delegates to subcommands: List, Status, Set, SetProvider | `models.rs:58–67` | `execute()` dispatches to `command.execute()` |
| `RebornProviderAdmin::load_registry()` is already a method that loads the builtin registry | `provider_admin.rs:90–97` | used by `list()` |

---

## Caveats / Corrections vs Original Plan

### C1: `seed_builtin_providers` fast-path must re-run on every restart

**Original plan:** `if pg_repo.builtins_seeded() { return Ok(()); }`

**Problem:** On a binary upgrade that adds new builtin providers, seeding is skipped if
any builtin already exists in the DB. New builtins from the updated `providers.json` are
never added.

**Fix:** Always run the seeding loop. `upsert_builtin` is idempotent: for existing rows it
updates structural fields while preserving operator-owned fields; for new rows it inserts.
The `builtins_seeded()` check can be removed entirely — the per-provider `upsert_builtin`
call is cheap (one query each, all short-circuit on structural equality in practice).
The `builtins_seeded()` method is still useful for the `DELETE` guard in tests but is not
needed in the seeding path.

### C2: `upsert_builtin` ON CONFLICT WHERE clause must be tightened

**Original plan WHERE clause:**
```sql
WHERE brassclaw_llm_providers.is_builtin = TRUE
   OR brassclaw_llm_providers.deleted_at IS NOT NULL
```

**Problem:** `deleted_at IS NOT NULL` means a soft-deleted custom provider row can be
overwritten by a builtin seed — silently reviving it as a builtin row. This is a security
issue: an operator who deleted a custom provider named "openai" (intentionally) would find
it resurrected as a builtin.

**Fix:** The WHERE clause should be just `is_builtin = TRUE`. If a conflict exists with a
non-builtin (whether active or soft-deleted), the seeding is skipped and a warning logged.
The operator's choice is preserved.

```sql
ON CONFLICT (tenant_id, id) DO UPDATE
    SET definition  = excluded.definition,
        is_builtin  = TRUE,
        deleted_at  = NULL,
        updated_at  = now()
    WHERE brassclaw_llm_providers.is_builtin = TRUE
```

### C3: `PgProviderRepo::upsert()` return value semantics

The existing `upsert()` returns `Ok(true)` if the row was an UPDATE (existed before),
`Ok(false)` if it was an INSERT (new row). This semantic is used in `upsert_provider` to
know if we need to handle a "replace" vs "add" scenario. When hardening `upsert()` to not
flip `is_builtin`, we must preserve this `(existing_active: bool)` return meaning exactly.

### C4: Tests need `pg_pool` + `pg_provider_repo` OR must be restructured

All tests that call `RebornLlmConfigService::new(boot, keys)` will fail once `boot` and
`repo` are removed from the struct. Most unit tests do NOT need a real DB because they test
logic that does not involve DB reads. The fix:

- Tests that test `upsert_provider` / `build_snapshot` logic against a real store: rewrite
  to use `PgProviderRepo` backed by a test DB pool. Mark with `#[cfg(feature = "integration")]`.
- Tests that test pure logic (adapter parsing, overlay building, key sentinels, role routing):
  keep as unit tests but update the constructor to use the new signature.
- Tests that call `snapshot()` when no DB is available: since we are dropping non-postgres
  support, these tests must either be converted to integration tests or the service must
  return a meaningful empty snapshot when no builtins exist yet (acceptable — first-run state).

### C5: `upsert_provider` for a builtin must call `upsert_builtin`, not `upsert`

The existing `upsert_provider` calls `upsert_provider_definition` which calls `pg_repo.upsert()`
which does not preserve `is_builtin`. When an operator configures a builtin (`openai`,
`anthropic`, etc.), the call must go through `pg_repo.upsert_builtin()` (which preserves
`is_builtin = TRUE`). This is step 5 of the plan and requires detecting the existing row's
`is_builtin` flag via `get_full()`.

### C6: `build_snapshot` — `admin_list_async` provides active-from-config-toml which no longer exists

After migration, there is no `config.toml` for the active provider — it lives in
`brassclaw_config` (the DB table). The `admin_list_async` → `admin.list()` path reads from
`config.toml`. This entire path is replaced by `read_kohai_sel_from_db()`.

### C7: `LlmConfigServiceError` must gain `CannotDeleteBuiltin` in `brassclaw_product_workflow`

The error type is defined in `brassclaw_product_workflow`. The `map_llm_config_error`
function there maps to HTTP status codes. We must add the new variant AND its mapping.

### C8: `provider_admin.rs::list()` is used by `admin_list_async` only for builtin list

After removing `admin_list_async`, `RebornProviderAdmin` is only used for the CLI `models`
command. The `list()` method that reads `config.toml` becomes CLI-only. The `list_from_db()`
method we add uses `pg_repo.load_all()`.

### C9: `read_role_sel_from_db_or_file` becomes `read_embedding_sel_from_db`

The file fallback path calls `read_role_selection(file_path)` where `file_path` comes from
`self.boot.home().embedding_provider_file_path()`. After removing `boot`, we remove the
file fallback entirely: `read_embedding_sel_from_db()` is DB-only.

---

## DB Changes

### Migration V047 — add `is_builtin` column with back-fill

**File:** `crates/brassclaw_pg/migrations/V047__llm_providers_is_builtin.sql`

```sql
ALTER TABLE brassclaw_llm_providers
    ADD COLUMN IF NOT EXISTS is_builtin BOOLEAN NOT NULL DEFAULT FALSE;

-- Back-fill: existing rows whose id matches a known builtin must be marked.
-- The set of builtin ids is fixed at migration time.  New builtins added in
-- future binary versions are handled by seed_builtin_providers() at boot.
UPDATE brassclaw_llm_providers
SET is_builtin = TRUE
WHERE id IN (
    'nearai', 'gemini_oauth', 'openai_codex', 'openai', 'anthropic',
    'ollama', 'groq', 'bedrock', 'openai_compatible', 'tinfoil', 'deepseek',
    'github_copilot', 'gemini'
)
AND deleted_at IS NULL;
-- Note: this list must match the ids in crates/brassclaw_llm/providers.json.
-- New entries are seeded at next boot via upsert_builtin() and receive is_builtin = TRUE.
```

### Migration V048 — seeding marker

**File:** `crates/brassclaw_pg/migrations/V048__seed_builtin_providers.sql`

```sql
-- Builtin provider seeding is performed in Rust at boot time, not in SQL.
-- This migration marks the schema version at which seeding is expected.
-- See: brassclaw_reborn_composition::webui::seed_builtin_providers()
--
-- Rationale: ProviderDefinition (including SetupHint enum variants) carries
-- #[serde(deny_unknown_fields)]. Inserting raw JSON from SQL that does not
-- exactly match the current struct would cause deserialisation errors.
-- All serialisation goes through serde_json in Rust.
```

---

## Rust Changes

### 1. `pg_provider_repo.rs` — additions and hardening

#### New: `load_all()`

```rust
/// Load all active (non-deleted) provider definitions — both builtin and custom.
/// Returns `(definition, is_builtin)` pairs, builtins ordered first.
pub async fn load_all(
    &self,
) -> Result<Vec<(ProviderDefinition, bool)>, PgProviderRepoError> {
    let client = self.pool.get().await?;
    let rows = client
        .query(
            "SELECT definition, is_builtin \
             FROM brassclaw_llm_providers \
             WHERE tenant_id = $1 AND deleted_at IS NULL \
             ORDER BY is_builtin DESC, id",
            &[&self.tenant_id],
        )
        .await?;
    rows.iter()
        .map(|row| {
            let json: serde_json::Value = row.try_get("definition")
                .map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
            let def: ProviderDefinition = serde_json::from_value(json)
                .map_err(|e| PgProviderRepoError::Parse { reason: e.to_string() })?;
            let is_builtin: bool = row.try_get("is_builtin").unwrap_or(false);
            Ok((def, is_builtin))
        })
        .collect()
}
```

#### New: `get_full()` — returns definition + is_builtin flag

```rust
/// Like `get()` but also returns the is_builtin flag.
pub async fn get_full(
    &self,
    id: &str,
) -> Result<Option<(ProviderDefinition, bool)>, PgProviderRepoError> {
    let client = self.pool.get().await?;
    let row = client
        .query_opt(
            "SELECT definition, is_builtin FROM brassclaw_llm_providers \
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            &[&self.tenant_id, &id],
        )
        .await?;
    match row {
        None => Ok(None),
        Some(r) => {
            let json: serde_json::Value = r.try_get("definition")
                .map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
            let def = serde_json::from_value(json)
                .map_err(|e| PgProviderRepoError::Parse { reason: e.to_string() })?;
            let is_builtin: bool = r.try_get("is_builtin").unwrap_or(false);
            Ok(Some((def, is_builtin)))
        }
    }
}
```

#### New: `upsert_builtin()`

Runs on every restart for every builtin (idempotent — see C1). The WHERE clause only
allows update if the existing row is already a builtin (see C2 — no `deleted_at IS NOT NULL`):

```rust
/// Insert or update a builtin provider definition.
///
/// Returns Ok(true) if a row was inserted or updated, Ok(false) if the
/// ON CONFLICT WHERE predicate was false (naming collision with a non-builtin).
pub async fn upsert_builtin(
    &self,
    definition: ProviderDefinition,
) -> Result<bool, PgProviderRepoError> {
    let json = serde_json::to_value(&definition)
        .map_err(|e| PgProviderRepoError::Parse { reason: e.to_string() })?;
    let client = self.pool.get().await?;
    let rows_affected = client
        .execute(
            "INSERT INTO brassclaw_llm_providers
                 (tenant_id, id, definition, is_builtin, deleted_at)
             VALUES ($1, $2, $3, TRUE, NULL)
             ON CONFLICT (tenant_id, id) DO UPDATE
                 SET definition  = excluded.definition,
                     is_builtin  = TRUE,
                     deleted_at  = NULL,
                     updated_at  = now()
                 WHERE brassclaw_llm_providers.is_builtin = TRUE",
            &[&self.tenant_id, &definition.id, &json],
        )
        .await?;
    Ok(rows_affected > 0)
}
```

#### New: `builtins_seeded()`

```rust
/// Returns true if at least one is_builtin = TRUE row exists for this tenant.
/// Useful for tests and diagnostics; not used in the seeding hot-path
/// (seeding always runs; upsert_builtin is idempotent).
pub async fn builtins_seeded(&self) -> Result<bool, PgProviderRepoError> {
    let client = self.pool.get().await?;
    let row = client
        .query_one(
            "SELECT EXISTS (
                 SELECT 1 FROM brassclaw_llm_providers
                 WHERE tenant_id = $1
                   AND is_builtin = TRUE
                   AND deleted_at IS NULL
             ) AS seeded",
            &[&self.tenant_id],
        )
        .await?;
    Ok(row.try_get::<_, bool>("seeded").unwrap_or(false))
}
```

#### Harden: `upsert()` — never flip `is_builtin`

Change the ON CONFLICT SET clause to exclude `is_builtin` (preserves whatever the row had):

```sql
ON CONFLICT (tenant_id, id) DO UPDATE
    SET definition  = excluded.definition,
        deleted_at  = NULL,
        updated_at  = now()
    -- is_builtin intentionally omitted
```

Note: `upsert()` return semantics must stay `Ok(bool)` = "was an existing active row updated"
(to preserve behaviour in `upsert_provider`).

#### Harden: `delete()` — reject builtin rows

```rust
pub async fn delete(&self, id: &str) -> Result<bool, PgProviderRepoError> {
    let client = self.pool.get().await?;
    let maybe_row = client
        .query_opt(
            "SELECT is_builtin FROM brassclaw_llm_providers \
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            &[&self.tenant_id, id],
        )
        .await?;
    match maybe_row {
        None => return Ok(false),
        Some(r) if r.try_get::<_, bool>("is_builtin").unwrap_or(false) => {
            return Err(PgProviderRepoError::CannotDeleteBuiltin);
        }
        _ => {}
    }
    let rows = client
        .execute(
            "UPDATE brassclaw_llm_providers
             SET deleted_at = now(), updated_at = now()
             WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
            &[&self.tenant_id, id],
        )
        .await?;
    Ok(rows > 0)
}
```

#### New error variant

```rust
#[error("cannot delete a builtin provider; use the configure dialog to reset it")]
CannotDeleteBuiltin,
```

Map to `LlmConfigServiceError::CannotDeleteBuiltin` → `RebornServicesError` with status 422
(mapping lives in `brassclaw_product_workflow`).

### 2. `webui.rs` — boot-time seeding

`seed_builtin_providers` always runs on every restart (idempotent — C1). Add it in
`webui.rs` immediately after the `with_pg_provider_repo` wiring and before
`api.with_llm_config_service(...)`:

```rust
/// Seed (or update) builtin provider definitions into the DB.
///
/// Called on every service start — upsert_builtin is idempotent so
/// existing rows are updated with structural fields from the current binary
/// while operator-owned fields (base_url, model, description, etc.) are preserved.
/// This ensures new builtins added in a binary upgrade are automatically available.
pub async fn seed_builtin_providers(
    pg_repo: &crate::pg_provider_repo::PgProviderRepo,
) -> Result<(), crate::pg_provider_repo::PgProviderRepoError> {
    let registry = brassclaw_llm::ProviderRegistry::try_load_from_path(None)
        .map_err(|e| crate::pg_provider_repo::PgProviderRepoError::Db(e.to_string()))?;
    // Load existing builtin rows for the Rust-merge (preserve operator-owned fields).
    let existing = pg_repo.load_all().await?;
    let existing_map: std::collections::HashMap<String, ProviderDefinition> = existing
        .into_iter()
        .filter(|(_, is_builtin)| *is_builtin)
        .map(|(def, _)| (def.id.clone(), def))
        .collect();
    let mut seeded = 0usize;
    for new_def in registry.all() {
        // Merge: start from new binary definition; preserve operator-owned
        // fields from any existing builtin row.
        let merged = if let Some(existing_def) = existing_map.get(&new_def.id) {
            let mut merged = new_def.clone();
            merged.default_base_url = existing_def.default_base_url.clone();
            merged.default_model   = existing_def.default_model.clone();
            merged.description     = existing_def.description.clone();
            merged.api_key_required = existing_def.api_key_required;
            merged.token_budget    = existing_def.token_budget.clone();
            merged
        } else {
            new_def.clone()
        };
        match pg_repo.upsert_builtin(merged).await {
            Ok(true)  => seeded += 1,
            Ok(false) => tracing::warn!(
                provider_id = %new_def.id,
                "builtin provider skipped: a non-builtin provider with the same id exists"
            ),
            Err(e) => tracing::warn!(
                provider_id = %new_def.id, error = %e,
                "failed to seed builtin provider"
            ),
        }
    }
    tracing::debug!(count = seeded, "seeded builtin LLM providers into DB");
    Ok(())
}
```

Call site (non-fatal):
```rust
if let Err(e) = seed_builtin_providers(&pg_repo).await {
    tracing::warn!(error = %e,
        "builtin provider seeding failed; providers may be missing until next restart");
}
```

### 3. `llm_config_service.rs` — struct surgery

Remove `boot: RebornBootConfig` and `repo: ProviderRepo`. Remove all `Option<>` wrappers
from `pg_pool` and `pg_provider_repo` — they become required (non-Option) fields.
Remove all `#[cfg(feature = "postgres")]` guards on these fields (DB is always present).

New minimal struct:

```rust
pub struct RebornLlmConfigService {
    keys: LlmKeyStore,
    reload: Option<Arc<dyn LlmReloadTrigger>>,
    nearai_session: Option<Arc<brassclaw_llm::SessionManager>>,
    nearai_login_states: Arc<NearAiLoginStateStore>,
    codex_login_attempts: Arc<tokio::sync::Mutex<HashMap<String, CodexLoginAttempt>>>,
    pg_pool: Arc<brassclaw_pg::PgPool>,
    pg_provider_repo: Arc<crate::pg_provider_repo::PgProviderRepo>,
    db_tenant_id: String,
    #[cfg(feature = "root-llm-provider")]
    sempai_swappable: Option<Arc<brassclaw_llm::SwappableLlmProvider>>,
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    interceptor_mode: Option<brassclaw_interceptor::SharedInterceptorMode>,
}
```

New constructor:
```rust
pub fn new(
    keys: LlmKeyStore,
    pg_pool: Arc<brassclaw_pg::PgPool>,
    pg_provider_repo: Arc<crate::pg_provider_repo::PgProviderRepo>,
    db_tenant_id: impl Into<String>,
) -> Self { ... }
```

Builder methods `with_pg_pool` and `with_pg_provider_repo` are **deleted** (now required
args). Remove the `#[cfg(feature = "postgres")]` from the remaining builder methods.

Remove `use crate::{LlmKeyStore, ProviderRepo, RebornProviderAdmin}` import; replace with
`use crate::LlmKeyStore`.

Update call site in `webui.rs` to pass the pool/repo/tenant_id directly to `new()`.

### 4. `llm_config_service.rs` — rewrite `build_snapshot`

Single DB pass — no `admin_list_async`, no embedded registry, concurrent key checks:

```rust
async fn build_snapshot(&self) -> Result<LlmConfigSnapshot, LlmConfigServiceError> {
    let all_defs = self.pg_provider_repo
        .load_all()
        .await
        .map_err(|_| LlmConfigServiceError::Unavailable)?;

    let kohai_sel  = self.read_kohai_sel_from_db().await;
    let sempai_sel = self.read_sempai_sel_from_db().await;
    let embedding_sel = self.read_embedding_sel_from_db().await;

    // Check key existence concurrently.
    let key_checks = all_defs.iter()
        .map(|(d, _)| self.keys.exists(d.id.as_str()))
        .collect::<Vec<_>>();
    let key_results: Vec<bool> = futures::future::join_all(key_checks)
        .await
        .into_iter()
        .map(|r| r.unwrap_or(false))
        .collect();

    let mut providers = Vec::with_capacity(all_defs.len());
    let mut active = None;

    for ((def, is_builtin), stored_key_set) in all_defs.into_iter().zip(key_results) {
        let env_key_set = def.api_key_env.as_ref()
            .is_some_and(|env| std::env::var(env).is_ok());
        let api_key_set = stored_key_set || env_key_set;
        let is_kohai = kohai_sel.as_ref()
            .is_some_and(|s| s.provider_id == def.id);
        let active_model = is_kohai
            .then(|| kohai_sel.as_ref().and_then(|s| s.model.clone()))
            .flatten();
        if is_kohai && active.is_none() {
            active = Some(LlmActiveSelection {
                provider_id: def.id.clone(),
                model: active_model.clone(),
            });
        }
        let can_list_models = def.setup.as_ref()
            .is_some_and(brassclaw_llm::registry::SetupHint::can_list_models);
        let accepts_api_key = def.api_key_env.is_some()
            || def.setup.as_ref()
                .is_some_and(brassclaw_llm::registry::SetupHint::accepts_api_key);
        let adapter = provider_protocol_wire_name(def.protocol);
        let token_budget = def.token_budget.as_ref().map(|b| ProviderTokenBudgetView {
            profile: b.profile.clone(),
            conversation_history: b.conversation_history,
            skills: b.skills,
            identity: b.identity,
            inline_control: b.inline_control,
            memory: b.memory,
            safety: b.safety,
            capability_surface: b.capability_surface,
            total_input: b.total_input,
            max_output: b.max_output,
        });
        providers.push(LlmProviderView {
            id: def.id.clone(),
            description: if def.description.is_empty() { def.id.clone() }
                         else { def.description.clone() },
            adapter,
            default_model: def.default_model.clone(),
            base_url: def.default_base_url.clone(),
            builtin: is_builtin,
            active: is_kohai,
            active_model,
            api_key_required: def.api_key_required,
            accepts_api_key,
            api_key_set,
            can_list_models,
            token_budget,
            context_window_tokens: def.context_window_tokens,
            is_kohai,
            is_sempai: sempai_sel.as_ref().is_some_and(|s| s.provider_id == def.id),
            is_embedding: embedding_sel.as_ref().is_some_and(|s| s.provider_id == def.id),
        });
    }

    Ok(LlmConfigSnapshot {
        providers,
        active: active.clone(),
        kohai_active: active,
        sempai_active: sempai_sel,
        embedding_active: embedding_sel,
    })
}
```

Add `read_kohai_sel_from_db()` and `read_embedding_sel_from_db()` following the exact same
pattern as `read_sempai_sel_from_db()`.

Remove `read_role_sel_from_db_or_file` and `admin_list_async` and `admin()`.

### 5. `llm_config_service.rs` — rewrite `upsert_provider`

Remove `ProviderRegistry::try_load_from_path(None)` and `build_overlay_definition`.
Use `pg_repo.get_full(id)` to determine if the existing row is a builtin:

```
1. validate_provider_id(&request.id)
2. pg_repo.get_full(id) → Option<(ProviderDefinition, bool)>
3. stored_key_present: check LlmKeyStore
4. If existing row and is_builtin = true:
   - Start from the existing DB definition (has protocol/setup/aliases intact)
   - Overlay operator-editable fields: default_base_url, default_model,
     description, api_key_required, token_budget
   - Write back with pg_repo.upsert_builtin(merged_def)
5. If no existing row, or existing is_builtin = false:
   - Build from request (same as current build_overlay_definition else-branch)
   - Write with pg_repo.upsert(def)
6. Handle token_budget merge (unchanged logic)
7. Store key if has_new_key
8. set_active if request.set_active
9. refresh_running_provider (simplified)
10. build_snapshot()
```

Edge case — upsert for a builtin id that is not yet seeded: Cannot happen because seeding
runs before the listener binds.

### 6. `llm_config_service.rs` — fix `refresh_running_provider`

Replace the `admin_list_async()` call with a DB read:

```rust
if let Some(sel) = self.read_kohai_sel_from_db().await {
    reload.on_provider_changed(&sel.provider_id).await;
}
```

### 7. `llm_config_service.rs` — fix `probe_matches_persisted_provider`

Remove the builtin-registry fallback. After seeding, all providers are in the DB:

```rust
async fn probe_matches_persisted_provider(
    &self,
    request: &LlmProbeRequest,
) -> Result<bool, LlmConfigServiceError> {
    let Some(definition) = self.load_provider_by_id(&request.provider_id).await? else {
        return Ok(false);
    };
    let Some(protocol) = parse_adapter(&request.adapter) else {
        return Ok(false);
    };
    Ok(protocol == definition.protocol
        && normalized_endpoint(request.base_url.as_deref())
            == normalized_endpoint(definition.default_base_url.as_deref()))
}
```

### 8. `llm_config_service.rs` — fix `build_sempai_provider`

Remove the builtin-registry fallback:

```rust
let definition = self
    .load_provider_by_id(provider_id)
    .await
    .map_err(|e| format!("provider load: {e}"))?
    .ok_or_else(|| format!("provider not found: {provider_id}"))?;
```

### 9. `llm_config_service.rs` — simplify helper methods

Remove all `#[cfg(feature = "postgres")]` guards and file fallbacks:
- `load_provider_by_id` → direct `pg_provider_repo.get(id)` call
- `upsert_provider_definition` → direct `pg_provider_repo.upsert(definition)` call
- `delete_provider_definition` → direct `pg_provider_repo.delete(id)` call (propagates `CannotDeleteBuiltin`)
- `set_provider_async` → remove the no-op fallback; body is just the postgres path, unconditional

Add `LlmConfigServiceError::CannotDeleteBuiltin` propagation in `delete_provider_definition`.

### 10. Tests — update to use `PgProviderRepo`

Tests that test pure logic (adapter parsing, overlay building, key sentinels, etc.) can keep
their structure but must update the constructor call. Since `new()` now requires `pg_pool`
and `pg_provider_repo`, these tests need a test DB or must be split.

**Strategy:** The tests that need a real provider persistence store become integration tests
(`#[cfg(feature = "integration")]`). Tests that only test pure in-memory logic (key
sentinel, adapter parsing, etc.) keep their `#[tokio::test]` form but use a
`FakePgProviderRepo` or mock.

For the scope of this plan: rewrite the two key rollback tests as integration tests and
remove their `ProviderRepo::new(...).load()` assertions. All other tests that call
`RebornLlmConfigService::new(boot, keys)` must be updated to use the new constructor
and will need a test pool — they can be converted to integration tests.

### 11. `llm_catalog.rs` — DB-backed async variant

Add `resolve_llm_selection_against_catalog_db()` for the runtime path where a DB pool
is available. Keep the original sync variant for the CLI path.

### 12. `runtime.rs` — DB-first for context_window_tokens and cache_retention

Replace the two `try_load_from_path(None)` calls with DB reads + embedded fallback:

```rust
#[cfg(feature = "root-llm-provider")]
let resolved_context_window_tokens: Option<u32> = {
    let from_db = if let (Some(pool), Some(l)) = (pg_pool.as_ref(), llm.as_ref()) {
        let repo = PgProviderRepo::new((*pool).clone(), tenant_id.to_string());
        repo.get(l.provider_id()).await.ok().flatten()
            .and_then(|def| def.context_window_tokens)
    } else { None };
    from_db.or_else(|| {
        llm.as_ref().and_then(|l| {
            brassclaw_llm::ProviderRegistry::try_load_from_path(None).ok()
                .and_then(|r| r.find(l.provider_id()).and_then(|d| d.context_window_tokens))
        })
    })
};
```

Apply the same pattern for `cache_retention`.

### 13. `provider_admin.rs` — add `list_from_db()`

```rust
pub async fn list_from_db(
    &self,
    pg_repo: &PgProviderRepo,
    provider: Option<&str>,
    verbose: bool,
) -> Result<RebornProviderList, RebornProviderAdminError> {
    let all = pg_repo.load_all().await
        .map_err(|e| RebornProviderAdminError::LoadRegistry { reason: e.to_string() })?;
    // filter, build list from (ProviderDefinition, is_builtin) pairs
}
```

### 14. `models.rs` (CLI) — use `list_from_db`

The CLI `models list` / `models status` / `models set` subcommands delegate through
`RebornProviderAdmin`. After adding `list_from_db`, the `models list` path should prefer DB
when a pool is available. The existing sync `list()` remains for offline use.

### 15. `factory.rs` — `resolve_pg_embedding_provider`

Switch `provider_repo.load()` to `provider_repo.load_all()`:

```rust
let providers: Vec<ProviderDefinition> = match provider_repo.load_all().await {
    Ok(pairs) => pairs.into_iter().map(|(def, _)| def).collect(),
    Err(err) => {
        tracing::debug!(error = %err, "failed to load providers for embedding resolution");
        return None;
    }
};
```

### 16. `brassclaw_product_workflow` — map `CannotDeleteBuiltin`

Add the new error variant to `LlmConfigServiceError` and its mapping:

```rust
// In LlmConfigServiceError:
#[error("cannot delete a builtin provider")]
CannotDeleteBuiltin,

// In map_llm_config_error:
LlmConfigServiceError::CannotDeleteBuiltin => {
    RebornServicesError::from_status(RebornServicesErrorCode::InvalidRequest, 422, false)
},
```

---

## Security Properties

### `ProviderDefinition` is `deny_unknown_fields` — all merges happen in Rust
SQL JSONB `||` merge would produce objects with keys the struct does not recognise.
All merges are done in Rust. The DB stores only well-formed serialised structs.

### Builtin deletion is blocked at the repo layer
`delete()` reads `is_builtin` before soft-delete and returns `CannotDeleteBuiltin` if true.

### `is_builtin` flag is immutable via the operator path
`upsert()` (operator path) never writes `is_builtin`. Only `upsert_builtin()` sets it TRUE.

### Naming collision between custom and builtin ids
If a custom provider with a builtin id exists, `upsert_builtin` silently does nothing and
logs a warning. The operator's row is preserved.

### Soft-deleted rows are NOT revived by seeding
The `upsert_builtin` ON CONFLICT WHERE clause is `is_builtin = TRUE` only. A soft-deleted
custom provider named "openai" would not be revived as a builtin.

### Stored keys never in `definition` column
`ProviderDefinition` has no `api_key` field. Key values stay in `LlmKeyStore`.

### `probe_matches_persisted_provider` key-exfiltration guard is preserved
Logic unchanged; stored key only applied when probe targets the same base URL.

### `validate_provider_id` runs before any DB write
`[a-z0-9_-]{1,64}` enforcement is unchanged.

---

## Performance Properties

### `build_snapshot`: single DB query replaces dual-pass
Before: `admin_list_async` spawns a blocking task; then another registry load for budgets.
After: one `pg_repo.load_all()` query.

### `build_snapshot`: key-exists calls are concurrent
Before: sequential `for` loop (N round trips).
After: `futures::future::join_all` — all checks run concurrently.

### `upsert_builtin`: idempotent Rust-merge, one DB write per provider on restart
Boot: `load_all()` once, then N individual upserts (short-circuit via ON CONFLICT WHERE).

### `refresh_running_provider`: direct async DB read
Before: `admin_list_async()` called `spawn_blocking`.
After: `read_kohai_sel_from_db()` is a direct async DB read.

---

## Migration Sequence

```
1.  ✅ Write V047 (is_builtin column + back-fill of known builtin ids)
2.  ✅ Write V048 (no-op marker)
3.  ✅ Add PgProviderRepoError::CannotDeleteBuiltin
4.  ✅ Add PgProviderRepo::load_all() returning Vec<(ProviderDefinition, bool)>
5.  ✅ Add PgProviderRepo::get_full() returning Option<(ProviderDefinition, bool)>
6.  ✅ Add PgProviderRepo::upsert_builtin() — idempotent, Rust-merge-then-write
7.  ✅ Add PgProviderRepo::builtins_seeded() — for tests/diagnostics
8.  ✅ Harden PgProviderRepo::upsert() to never write is_builtin
9.  ✅ Harden PgProviderRepo::delete() to reject is_builtin = TRUE rows
10. ✅ Add seed_builtin_providers() in webui.rs; wire into startup (always runs, idempotent)
11. ✅ Add LlmConfigServiceError::CannotDeleteBuiltin to brassclaw_product_workflow
12. ✅ Map CannotDeleteBuiltin → 422 in map_llm_config_error
13. ✅ Add read_kohai_sel_from_db() and read_embedding_sel_from_db() to llm_config_service.rs
14. ✅ Remove boot and repo fields from RebornLlmConfigService struct and new()
    (pg_pool and pg_provider_repo become required non-Option constructor args)
15. ✅ Update webui.rs call site: new() takes pool/repo/tenant_id directly
16. ✅ Rewrite build_snapshot(): single DB loop, concurrent key-exists, no admin_list_async
17. ✅ Rewrite upsert_provider(): DB-read-then-Rust-merge, no embedded registry
18. ✅ Fix refresh_running_provider(): replace admin_list_async with read_kohai_sel_from_db
19. ✅ Fix probe_matches_persisted_provider(): remove builtin-registry fallback
20. ✅ Fix build_sempai_provider(): remove builtin-registry fallback
21. ✅ Simplify load_provider_by_id, upsert_provider_definition, delete_provider_definition,
    set_provider_async: remove all #[cfg] guards and file fallbacks
22. ✅ Delete admin_list_async, admin(), build_overlay_definition, custom_definition,
    read_role_sel_from_db_or_file, with_pg_pool builder, with_pg_provider_repo builder
23. ✅ Update tests: rewrite ProviderRepo file assertions; update new() call sites
24. ✅ Update llm_catalog.rs: add resolve_llm_selection_against_catalog_db()
25. ✅ Update runtime.rs: context_window_tokens + cache_retention DB-first
26. ✅ Add RebornProviderAdmin::list_from_db(); update CLI models.rs
27. ✅ Update factory.rs resolve_pg_embedding_provider: load_all()
28. ✅ Clippy clean pass (zero warnings)
29. ✅ Integration tests pass
```

---

## Files Changed

```
crates/brassclaw_pg/migrations/
  V047__llm_providers_is_builtin.sql              [new]
  V048__seed_builtin_providers.sql                [new, no-op marker]

crates/brassclaw_reborn_composition/src/
  pg_provider_repo.rs          [extend + harden]
  webui.rs                     [add seed_builtin_providers(); call in wiring]
  llm_config_service.rs        [full rewrite of struct, new(), build_snapshot,
                                 upsert_provider, refresh_running_provider,
                                 probe_matches_persisted_provider,
                                 build_sempai_provider, helpers; remove
                                 admin_list_async, admin, build_overlay_definition]
  provider_admin.rs            [add async list_from_db()]
  llm_catalog.rs               [add resolve_llm_selection_against_catalog_db()]
  factory.rs                   [resolve_pg_embedding_provider: load_all()]
  provider_repo.rs             [keep; annotate as CLI-only legacy]
  runtime.rs                   [context_window_tokens + cache_retention: DB-first]

crates/brassclaw_reborn_cli/src/commands/
  models.rs                    [use list_from_db() for list subcommand]

crates/brassclaw_product_workflow/src/
  reborn_services/llm_config.rs  [add CannotDeleteBuiltin variant + 422 mapping]
```
