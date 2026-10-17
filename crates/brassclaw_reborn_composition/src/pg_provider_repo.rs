//! Database-owned provider definitions and immutable revision history.
//! V140 installs initial definitions once for a new namespace, preserving an
//! existing inventory and every operator edit/tombstone on later startup.
//! `is_builtin` is origin metadata, not a restriction on editing/deactivation.
//! Production operator edits use exact revisions; legacy upsert consumers still
//! require the coordinated §10/Phase 8 cutover.
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

/// Stored provider configuration revision. Origin is metadata, not authority.
#[derive(Debug, Clone)]
pub(crate) struct ProviderDefinitionRevision {
    pub definition: ProviderDefinition,
    pub revision: u64,
    pub is_builtin: bool,
    pub inactive: bool,
}

impl PgProviderRepo {
    /// Construct a repo scoped to a specific tenant.
    pub fn new(pool: PgPool, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    /// Installation-owned data initialization before catalogue consumers. The
    /// SQL migration owns the definitions; this port contains no provider list.
    pub(crate) async fn initialize(&self) -> Result<bool, PgProviderRepoError> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                "SELECT brassclaw_initialize_provider_catalogue($1)",
                &[&self.tenant_id],
            )
            .await?;
        Ok(row.try_get(0)?)
    }

    pub(crate) async fn get_revision(
        &self,
        id: &str,
        include_inactive: bool,
    ) -> Result<Option<ProviderDefinitionRevision>, PgProviderRepoError> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT id, definition, revision, is_builtin, deleted_at IS NOT NULL AS inactive FROM brassclaw_llm_providers WHERE tenant_id = $1 AND id = $2 AND ($3 OR deleted_at IS NULL)",
            &[&self.tenant_id, &id, &include_inactive],
        ).await?;
        row.map(decode_provider_revision).transpose()
    }

    pub(crate) async fn load_revisions_from_client<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
    ) -> Result<Vec<ProviderDefinitionRevision>, PgProviderRepoError> {
        client.query("SELECT id, definition, revision, is_builtin, deleted_at IS NOT NULL AS inactive FROM brassclaw_llm_providers WHERE tenant_id=$1 ORDER BY is_builtin DESC, id", &[&self.tenant_id]).await?
            .into_iter().map(decode_provider_revision).collect()
    }

    /// Exact compare-and-replace, including explicit reactivation. No old role
    /// selection is restored. None requires a genuinely new stable identity.
    #[cfg(test)]
    pub(crate) async fn replace_revision(
        &self,
        definition: ProviderDefinition,
        expected_revision: Option<u64>,
    ) -> Result<ProviderDefinitionRevision, PgProviderRepoError> {
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        let retained = self
            .replace_revision_in_transaction(&transaction, definition, expected_revision)
            .await?;
        transaction.commit().await?;
        Ok(retained)
    }

    /// Composition-owned transaction boundary, allowing a definition and its
    /// separately encrypted credential to publish atomically.
    pub(crate) async fn replace_revision_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        definition: ProviderDefinition,
        expected_revision: Option<u64>,
    ) -> Result<ProviderDefinitionRevision, PgProviderRepoError> {
        let json =
            serde_json::to_value(&definition).map_err(|error| PgProviderRepoError::Parse {
                reason: error.to_string(),
            })?;
        let expected = expected_revision
            .map(i64::try_from)
            .transpose()
            .map_err(|_| PgProviderRepoError::RevisionConflict)?;
        if expected.is_some_and(|revision| revision <= 0) {
            return Err(PgProviderRepoError::RevisionConflict);
        }
        let row = if let Some(expected) = expected {
            transaction.query_opt(
                "UPDATE brassclaw_llm_providers SET definition=$3, deleted_at=NULL, updated_at=now() WHERE tenant_id=$1 AND id=$2 AND revision=$4 RETURNING id, definition, revision, is_builtin, deleted_at IS NOT NULL AS inactive",
                &[&self.tenant_id, &definition.id, &json, &expected],
            ).await?
        } else {
            transaction.query_opt(
                "INSERT INTO brassclaw_llm_providers (tenant_id,id,definition) VALUES ($1,$2,$3) ON CONFLICT (tenant_id,id) DO NOTHING RETURNING id, definition, revision, is_builtin, deleted_at IS NOT NULL AS inactive",
                &[&self.tenant_id, &definition.id, &json],
            ).await?
        }.ok_or(PgProviderRepoError::RevisionConflict)?;
        decode_provider_revision(row)
    }

    // ─── Read operations ─────────────────────────────────────────────────────

    /// Load all active (non-deleted) provider definitions — both builtin and custom.
    ///
    /// Returns `(definition, is_builtin)` pairs, builtins ordered first.
    pub async fn load_all(&self) -> Result<Vec<(ProviderDefinition, bool)>, PgProviderRepoError> {
        let client = self.pool.get().await?;
        self.load_all_from_client(&**client).await
    }

    pub(crate) async fn load_all_from_client<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
    ) -> Result<Vec<(ProviderDefinition, bool)>, PgProviderRepoError> {
        let rows = client
            .query(
                "SELECT id, definition, is_builtin \
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
                let stored_id: String = row.try_get("id")?;
                if def.id != stored_id {
                    return Err(PgProviderRepoError::Parse {
                        reason: "provider definition identity differs from its stored row identity"
                            .into(),
                    });
                }
                let is_builtin: bool = row.try_get("is_builtin")?;
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
        self.get_from_client(&**client, id).await
    }

    pub(crate) async fn get_from_client<C: tokio_postgres::GenericClient + Sync>(
        &self,
        client: &C,
        id: &str,
    ) -> Result<Option<ProviderDefinition>, PgProviderRepoError> {
        let row = client
            .query_opt(
                "SELECT id, definition FROM brassclaw_llm_providers \
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
                let def: ProviderDefinition =
                    serde_json::from_value(json).map_err(|e| PgProviderRepoError::Parse {
                        reason: e.to_string(),
                    })?;
                if def.id != r.try_get::<_, String>("id")? {
                    return Err(PgProviderRepoError::Parse {
                        reason: "provider identity differs from its row".into(),
                    });
                }
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
                let is_builtin: bool = r.try_get("is_builtin")?;
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
        Ok(row.try_get::<_, bool>("seeded")?)
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
            .await?
            .try_get("exists")?;

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

    /// Historical fixture seam. Product initialization uses V140 instead.
    #[cfg(test)]
    pub(crate) async fn upsert_builtin(
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
                     WHERE brassclaw_llm_providers.is_builtin = TRUE
                       AND brassclaw_llm_providers.deleted_at IS NULL",
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
    /// Deactivation and all affected role selections commit together. Preserve
    /// definitions and credentials so explicit reactivation remains possible.
    pub async fn delete(&self, id: &str) -> Result<bool, PgProviderRepoError> {
        self.deactivate(id, None).await
    }

    pub(crate) async fn delete_revision(
        &self,
        id: &str,
        revision: u64,
    ) -> Result<bool, PgProviderRepoError> {
        let revision =
            i64::try_from(revision).map_err(|_| PgProviderRepoError::RevisionConflict)?;
        if revision <= 0 {
            return Err(PgProviderRepoError::RevisionConflict);
        }
        self.deactivate(id, Some(revision)).await
    }

    async fn deactivate(
        &self,
        id: &str,
        expected: Option<i64>,
    ) -> Result<bool, PgProviderRepoError> {
        let mut client = self.pool.get().await?;
        let transaction = client.transaction().await?;
        let maybe_row = transaction
            .query_opt(
                "SELECT definition, revision FROM brassclaw_llm_providers \
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL FOR UPDATE",
                &[&self.tenant_id, &id],
            )
            .await?;

        let Some(row) = maybe_row else {
            return Ok(false);
        };
        if expected.is_some_and(|revision| revision != row.get::<_, i64>("revision")) {
            return Err(PgProviderRepoError::RevisionConflict);
        }
        let definition: ProviderDefinition = serde_json::from_value(row.try_get("definition")?)
            .map_err(|_| PgProviderRepoError::Parse {
                reason: "stored provider definition is invalid".into(),
            })?;
        if definition.id != id {
            return Err(PgProviderRepoError::Parse {
                reason: "stored provider identity differs from row identity".into(),
            });
        }
        let identities: Vec<String> = std::iter::once(&definition.id)
            .chain(&definition.aliases)
            .map(|identity| identity.to_lowercase())
            .collect();
        let selections = transaction
            .query(
                "SELECT key FROM brassclaw_config WHERE tenant_id = $1 \
             AND (key LIKE 'llm.%.provider_id' OR key = 'embedding.provider_id') \
             AND lower(value) = ANY($2) FOR UPDATE",
                &[&self.tenant_id, &identities],
            )
            .await?;
        let mut keys = Vec::new();
        for selection in selections {
            let key: String = selection.try_get("key")?;
            let prefix =
                key.strip_suffix("provider_id")
                    .ok_or_else(|| PgProviderRepoError::Parse {
                        reason: "provider selection key is invalid".into(),
                    })?;
            for field in ["provider_id", "model", "base_url", "api_key_env"] {
                keys.push(format!("{prefix}{field}"));
            }
        }
        transaction
            .execute(
                "DELETE FROM brassclaw_config WHERE tenant_id = $1 AND key = ANY($2)",
                &[&self.tenant_id, &keys],
            )
            .await?;
        let rows = transaction
            .execute(
                "UPDATE brassclaw_llm_providers
                 SET deleted_at = now(), updated_at = now()
                 WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
                &[&self.tenant_id, &id],
            )
            .await?;
        transaction.commit().await?;
        Ok(rows > 0)
    }
}

fn decode_provider_revision(
    row: tokio_postgres::Row,
) -> Result<ProviderDefinitionRevision, PgProviderRepoError> {
    let definition: ProviderDefinition = serde_json::from_value(row.try_get("definition")?)
        .map_err(|_| PgProviderRepoError::Parse {
            reason: "stored provider definition is invalid".into(),
        })?;
    let id: String = row.try_get("id")?;
    let revision = u64::try_from(row.try_get::<_, i64>("revision")?).map_err(|_| {
        PgProviderRepoError::Parse {
            reason: "stored provider revision is invalid".into(),
        }
    })?;
    if definition.id != id || revision == 0 {
        return Err(PgProviderRepoError::Parse {
            reason: "stored provider revision identity is invalid".into(),
        });
    }
    Ok(ProviderDefinitionRevision {
        definition,
        revision,
        is_builtin: row.try_get("is_builtin")?,
        inactive: row.try_get("inactive")?,
    })
}

/// Errors surfaced by `PgProviderRepo` operations.
#[derive(Debug, Error)]
pub enum PgProviderRepoError {
    #[error("provider configuration revision conflict")]
    RevisionConflict,
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
