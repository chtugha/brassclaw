//! DB-backed LLM provider repository — reads/writes `brassclaw_llm_providers`.
//!
//! As of V047/V048, this repo is the exclusive runtime source of truth for
//! ALL providers — both builtins (seeded from `providers.json` at boot) and
//! custom (operator-defined).
//!
//! ## `is_builtin` invariants
//!
//! - `is_builtin = TRUE` rows may NOT be soft-deleted via `delete()`.
//! - `upsert()` (operator path) never writes `is_builtin`; the column is
//!   preserved at whatever value the row was created with.
//! - Only `upsert_builtin()` (seeding path, called at service startup) sets
//!   `is_builtin = TRUE`.
//!
//! ## Upsert semantics
//!
//! `brassclaw_llm_providers` uses a soft-delete column (`deleted_at`).
//! `upsert()` un-soft-deletes and updates in one operation:
//!
//! ```sql
//! INSERT INTO brassclaw_llm_providers (tenant_id, id, definition, deleted_at)
//! VALUES ($tenant, $id, $definition, NULL)
//! ON CONFLICT (tenant_id, id) DO UPDATE
//! SET definition = excluded.definition, deleted_at = NULL, updated_at = now()
//! -- is_builtin NOT touched
//! ```
//!
//! ## API key security
//!
//! `ProviderDefinition` has no `api_key` field — only `api_key_env` (an env
//! var name). API key *values* are never stored here; they live in the scoped
//! secret store and are injected at provider-build time by `LlmKeyStore`.

use brassclaw_llm::ProviderDefinition;
use brassclaw_pg::PgPool;
use thiserror::Error;

/// Postgres-backed provider repository.
///
/// Owns the read/write path for all provider definitions (builtin and custom)
/// in `brassclaw_llm_providers`.
#[derive(Clone)]
pub struct PgProviderRepo {
    pool: PgPool,
    tenant_id: String,
}

impl PgProviderRepo {
    /// Construct a repo scoped to a specific tenant.
    pub fn new(pool: PgPool, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    // ─── Read operations ─────────────────────────────────────────────────────

    /// Load all active (non-deleted) provider definitions — both builtin and custom.
    ///
    /// Returns `(definition, is_builtin)` pairs, builtins ordered first.
    pub async fn load_all(&self) -> Result<Vec<(ProviderDefinition, bool)>, PgProviderRepoError> {
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
                let json: serde_json::Value = row
                    .try_get("definition")
                    .map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
                let def: ProviderDefinition =
                    serde_json::from_value(json).map_err(|e| PgProviderRepoError::Parse {
                        reason: e.to_string(),
                    })?;
                let is_builtin: bool = row.try_get("is_builtin").unwrap_or(false);
                Ok((def, is_builtin))
            })
            .collect()
    }

    /// Load all active custom (non-builtin) provider definitions.
    ///
    /// Retained for callers that only need custom providers (e.g. legacy
    /// paths being migrated). Prefer `load_all()` for new code.
    pub async fn load(&self) -> Result<Vec<ProviderDefinition>, PgProviderRepoError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT definition FROM brassclaw_llm_providers \
                 WHERE tenant_id = $1 AND deleted_at IS NULL AND is_builtin = FALSE \
                 ORDER BY id",
                &[&self.tenant_id],
            )
            .await?;

        let mut providers = Vec::with_capacity(rows.len());
        for row in &rows {
            let json: serde_json::Value = row
                .try_get("definition")
                .map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
            let def: ProviderDefinition =
                serde_json::from_value(json).map_err(|e| PgProviderRepoError::Parse {
                    reason: e.to_string(),
                })?;
            providers.push(def);
        }
        Ok(providers)
    }

    /// Get a single active provider definition by `id`.
    ///
    /// Returns `None` if no active row exists for the given id.
    pub async fn get(&self, id: &str) -> Result<Option<ProviderDefinition>, PgProviderRepoError> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT definition FROM brassclaw_llm_providers \
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
                &[&self.tenant_id, &id],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(r) => {
                let json: serde_json::Value = r
                    .try_get("definition")
                    .map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
                let def = serde_json::from_value(json).map_err(|e| PgProviderRepoError::Parse {
                    reason: e.to_string(),
                })?;
                Ok(Some(def))
            }
        }
    }

    /// Get a single active provider definition by `id` together with its
    /// `is_builtin` flag.
    ///
    /// Returns `None` if no active row exists.
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
                let json: serde_json::Value = r
                    .try_get("definition")
                    .map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
                let def = serde_json::from_value(json).map_err(|e| PgProviderRepoError::Parse {
                    reason: e.to_string(),
                })?;
                let is_builtin: bool = r.try_get("is_builtin").unwrap_or(false);
                Ok(Some((def, is_builtin)))
            }
        }
    }

    /// Returns `true` if at least one `is_builtin = TRUE` row exists for this
    /// tenant.  Useful for tests and diagnostics.
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

    // ─── Write operations ─────────────────────────────────────────────────────

    /// Insert or replace a custom provider definition.
    ///
    /// `is_builtin` is intentionally **not** written here — the column retains
    /// whatever value the row was created with.  Only `upsert_builtin()` may
    /// set `is_builtin = TRUE`.
    ///
    /// Returns `true` if an existing active row was replaced,
    /// `false` if the definition was inserted fresh (or a soft-deleted row was
    /// revived).
    pub async fn upsert(
        &self,
        definition: ProviderDefinition,
    ) -> Result<bool, PgProviderRepoError> {
        let json = serde_json::to_value(&definition).map_err(|e| PgProviderRepoError::Parse {
            reason: e.to_string(),
        })?;

        let client = self.pool.get().await?;

        // Query whether an active row already exists so we can return the
        // "was it an update?" boolean.  A single transaction would be cleaner
        // but the EXISTS pre-check is acceptable here (no TOCTOU risk for this
        // use case since double-upserts are idempotent).
        let existing_active: bool = client
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM brassclaw_llm_providers \
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL) AS exists",
                &[&self.tenant_id, &definition.id],
            )
            .await
            .map(|r| r.try_get::<_, bool>("exists").unwrap_or(false))
            .unwrap_or(false);

        client
            .execute(
                // is_builtin is intentionally omitted from SET — it preserves
                // whatever value the row was created with.
                "INSERT INTO brassclaw_llm_providers (tenant_id, id, definition, deleted_at)
                 VALUES ($1, $2, $3, NULL)
                 ON CONFLICT (tenant_id, id) DO UPDATE
                 SET definition  = excluded.definition,
                     deleted_at  = NULL,
                     updated_at  = now()",
                &[&self.tenant_id, &definition.id, &json],
            )
            .await?;

        Ok(existing_active)
    }

    /// Insert or update a builtin provider definition (seeding path).
    ///
    /// Called by `seed_builtin_providers()` on every service start.  The
    /// operation is idempotent: existing builtin rows are updated with
    /// structural fields from the current binary's definition (the caller
    /// performs the Rust-side merge to preserve operator-owned fields before
    /// calling here).
    ///
    /// If a row exists with `is_builtin = FALSE` for the same id (i.e. an
    /// operator created a custom provider whose id collides with a builtin),
    /// the `ON CONFLICT WHERE` predicate evaluates to `FALSE`, PostgreSQL
    /// treats the conflict as `DO NOTHING`, and this function returns
    /// `Ok(false)`.  The calling `seed_builtin_providers` logs a warning and
    /// the operator's row is left untouched.
    ///
    /// Returns `Ok(true)` if a row was inserted or updated, `Ok(false)` if
    /// the conflict was skipped due to an `is_builtin = FALSE` collision.
    pub async fn upsert_builtin(
        &self,
        definition: ProviderDefinition,
    ) -> Result<bool, PgProviderRepoError> {
        let json = serde_json::to_value(&definition).map_err(|e| PgProviderRepoError::Parse {
            reason: e.to_string(),
        })?;
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
                     -- Only update if the existing row is already a builtin.
                     -- A conflict against a non-builtin row (is_builtin = FALSE)
                     -- is treated as DO NOTHING — the operator's choice is preserved.
                     WHERE brassclaw_llm_providers.is_builtin = TRUE",
                &[&self.tenant_id, &definition.id, &json],
            )
            .await?;
        // rows_affected = 0 means ON CONFLICT WHERE was false → naming collision
        Ok(rows_affected > 0)
    }

    /// Soft-delete a provider definition by `id`.
    ///
    /// Returns `true` if an active entry was found and soft-deleted,
    /// `false` if no active entry existed.
    ///
    /// Returns `Err(CannotDeleteBuiltin)` if the row is a builtin — builtin
    /// providers may not be deleted; use the configure dialog to reset them.
    pub async fn delete(&self, id: &str) -> Result<bool, PgProviderRepoError> {
        let client = self.pool.get().await?;

        // Guard: check is_builtin before soft-deleting.  The UI hides the
        // Delete button for builtin providers; this is the server-side
        // enforcement that applies to all callers (HTTP, CLI, future clients).
        let maybe_row = client
            .query_opt(
                "SELECT is_builtin FROM brassclaw_llm_providers \
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
                &[&self.tenant_id, &id],
            )
            .await?;

        match maybe_row {
            None => return Ok(false), // not found / already deleted
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
                &[&self.tenant_id, &id],
            )
            .await?;
        Ok(rows > 0)
    }
}

/// Errors surfaced by `PgProviderRepo` operations.
#[derive(Debug, Error)]
pub enum PgProviderRepoError {
    #[error("database error: {0}")]
    Db(String),

    #[error("failed to parse provider definition JSON: {reason}")]
    Parse { reason: String },

    #[error("cannot delete a builtin provider; use the configure dialog to reset it")]
    CannotDeleteBuiltin,
}

impl From<deadpool_postgres::PoolError> for PgProviderRepoError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        Self::Db(e.to_string())
    }
}

impl From<tokio_postgres::Error> for PgProviderRepoError {
    fn from(e: tokio_postgres::Error) -> Self {
        Self::Db(e.to_string())
    }
}
