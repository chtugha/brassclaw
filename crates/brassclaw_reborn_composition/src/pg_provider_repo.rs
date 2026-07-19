//! DB-backed LLM provider repository — reads/writes `brassclaw_llm_providers`.
//!
//! This is the Postgres-native replacement for the file-based [`ProviderRepo`]
//! which read/wrote `providers.json`. Built-in providers are never stored here —
//! they are compiled in and merged at registry construction time. Only
//! user-overlay (custom) providers land in `brassclaw_llm_providers`.
//!
//! # Upsert semantics
//!
//! `brassclaw_llm_providers` uses a soft-delete column (`deleted_at`).
//! To support re-adding a previously-deleted provider:
//! ```sql
//! INSERT INTO brassclaw_llm_providers (tenant_id, id, definition, deleted_at)
//! VALUES ($tenant, $id, $definition, NULL)
//! ON CONFLICT (tenant_id, id) DO UPDATE
//! SET definition = excluded.definition, deleted_at = NULL, updated_at = now()
//! ```
//!
//! This un-soft-deletes and updates in one operation.
//!
//! # API key security
//!
//! `ProviderDefinition` has no `api_key` field — only `api_key_env` (an env
//! var name). API key *values* are never stored here; they live in the scoped
//! secret store and are injected at provider-build time by `LlmKeyStore`.

use brassclaw_llm::ProviderDefinition;
use brassclaw_pg::PgPool;
use thiserror::Error;

/// Postgres-backed provider overlay repository.
///
/// Owns the read/write path for user-overlay provider definitions in
/// `brassclaw_llm_providers`. Built-in providers (compiled in) are never
/// stored here.
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

    /// Load all active (non-deleted) user-overlay provider definitions.
    pub async fn load(&self) -> Result<Vec<ProviderDefinition>, PgProviderRepoError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT definition FROM brassclaw_llm_providers \
                 WHERE tenant_id = $1 AND deleted_at IS NULL \
                 ORDER BY id",
                &[&self.tenant_id],
            )
            .await?;

        let mut providers = Vec::with_capacity(rows.len());
        for row in &rows {
            let json: serde_json::Value = row.try_get("definition").map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
            let def: ProviderDefinition =
                serde_json::from_value(json).map_err(|e| PgProviderRepoError::Parse {
                    reason: e.to_string(),
                })?;
            providers.push(def);
        }
        Ok(providers)
    }

    /// Insert or replace a custom provider definition, unconditionally.
    ///
    /// Returns `true` if an existing (non-deleted) entry was replaced,
    /// `false` if the definition was inserted fresh.
    pub async fn upsert(
        &self,
        definition: ProviderDefinition,
    ) -> Result<bool, PgProviderRepoError> {
        let json = serde_json::to_value(&definition).map_err(|e| PgProviderRepoError::Parse {
            reason: e.to_string(),
        })?;

        let client = self.pool.get().await?;

        // Check whether an active row exists before the upsert for the return value.
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
                "INSERT INTO brassclaw_llm_providers (tenant_id, id, definition, deleted_at)
                 VALUES ($1, $2, $3, NULL)
                 ON CONFLICT (tenant_id, id) DO UPDATE
                 SET definition = excluded.definition,
                     deleted_at = NULL,
                     updated_at = now()",
                &[&self.tenant_id, &definition.id, &json],
            )
            .await?;

        Ok(existing_active)
    }

    /// Soft-delete a provider definition by `id`.
    ///
    /// Returns `true` if an active entry was found and soft-deleted,
    /// `false` if no active entry existed.
    pub async fn delete(&self, id: &str) -> Result<bool, PgProviderRepoError> {
        let client = self.pool.get().await?;
        let rows_affected = client
            .execute(
                "UPDATE brassclaw_llm_providers
                 SET deleted_at = now(), updated_at = now()
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
                &[&self.tenant_id, &id],
            )
            .await?;
        Ok(rows_affected > 0)
    }

    /// Get a single provider definition by `id` (including soft-deleted).
    ///
    /// Returns `None` if no row exists for the given id.
    pub async fn get(
        &self,
        id: &str,
    ) -> Result<Option<ProviderDefinition>, PgProviderRepoError> {
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
                let json: serde_json::Value = r.try_get("definition").map_err(|e| PgProviderRepoError::Db(e.to_string()))?;
                let def =
                    serde_json::from_value(json).map_err(|e| PgProviderRepoError::Parse {
                        reason: e.to_string(),
                    })?;
                Ok(Some(def))
            }
        }
    }
}

/// Errors surfaced by `PgProviderRepo` operations.
#[derive(Debug, Error)]
pub enum PgProviderRepoError {
    #[error("database error: {0}")]
    Db(String),

    #[error("failed to parse provider definition JSON: {reason}")]
    Parse { reason: String },
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
