//! `PgComponentDbBackend` — Postgres implementation of [`ComponentDbBackend`].
//!
//! Implements the `builtin.component_db` tool backend for the composition layer.
//! Operates on `reborn_docus` rows (class 17) using the system-seed scope
//! (`SYSTEM_RESERVED_ID` + `"default"` agent + the runtime tenant).
//!
//! # Upsert invariant
//!
//! Every `upsert` call sets `validation_status='pending'` so the row enters
//! the Q1+Q2 queue — never writes `'validated'` directly (§7, Answer 2).
//!
//! # mark_stale
//!
//! Delegates to [`PgBasicPromptStore::mark_stale`] which invalidates the
//! cached prefix bundle for the given scope. Best-effort — errors are logged
//! but do not propagate to the caller.
//!
//! Feature-gated: `postgres`.

#![allow(dead_code)]
#![forbid(unsafe_code)]

#[cfg(feature = "postgres")]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use brassclaw_host_runtime::{
        ComponentDbBackend, ComponentDbError, ComponentDbRow, ComponentDbScope, ComponentDbUpsert,
        ComponentDbUpsertResult,
    };
    use brassclaw_pg::PgPool;
    use uuid::Uuid;

    use crate::pg_basic_prompt_store::PgBasicPromptStore;

    /// Postgres backend for `builtin.component_db`.
    ///
    /// Holds a pool + basic-prompt store. Tenant is fixed at construction time
    /// (matches the boot seed tenant).
    pub(crate) struct PgComponentDbBackend {
        pool: Arc<PgPool>,
        tenant_id: String,
        basic_prompt_store: PgBasicPromptStore,
    }

    impl PgComponentDbBackend {
        pub(crate) fn new(
            pool: Arc<PgPool>,
            tenant_id: impl Into<String>,
            basic_prompt_store: PgBasicPromptStore,
        ) -> Self {
            Self {
                pool,
                tenant_id: tenant_id.into(),
                basic_prompt_store,
            }
        }
    }

    #[async_trait]
    impl ComponentDbBackend for PgComponentDbBackend {
        async fn read_hash(
            &self,
            scope: &ComponentDbScope,
            name: &str,
        ) -> Result<Option<String>, ComponentDbError> {
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            let row = client
                .query_opt(
                    "SELECT content_hash FROM reborn_docus
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4
                       AND name = $5
                     LIMIT 1",
                    &[
                        &self.tenant_id,
                        &scope.user_id,
                        &"default",
                        &scope.project_id,
                        &name,
                    ],
                )
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            Ok(row.map(|r| r.get::<_, String>(0)))
        }

        async fn read_row(
            &self,
            scope: &ComponentDbScope,
            name: &str,
        ) -> Result<Option<ComponentDbRow>, ComponentDbError> {
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            let row = client
                .query_opt(
                    "SELECT id::text, name, content, content_hash, validation_status
                     FROM reborn_docus
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4
                       AND name = $5
                     LIMIT 1",
                    &[
                        &self.tenant_id,
                        &scope.user_id,
                        &"default",
                        &scope.project_id,
                        &name,
                    ],
                )
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;
            Ok(row.map(|r| ComponentDbRow {
                id: r.get(0),
                name: r.get(1),
                content: r.get(2),
                content_hash: r.get(3),
                validation_status: r.get(4),
            }))
        }

        async fn upsert(
            &self,
            scope: &ComponentDbScope,
            row: ComponentDbUpsert,
        ) -> Result<ComponentDbUpsertResult, ComponentDbError> {
            let client = self
                .pool
                .get()
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;

            // Check if the row already exists (to return is_new).
            let existing = client
                .query_opt(
                    "SELECT id::text FROM reborn_docus
                     WHERE tenant_id = $1 AND user_id = $2
                       AND agent_id  = $3 AND project_id = $4
                       AND name = $5",
                    &[
                        &self.tenant_id,
                        &scope.user_id,
                        &"default",
                        &scope.project_id,
                        &row.name,
                    ],
                )
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;

            let consumer_tags: Vec<&str> =
                row.consumer_tags.iter().map(String::as_str).collect();

            // Upsert — always sets validation_status='pending' (§7, Answer 2).
            let result_row = client
                .query_one(
                    "INSERT INTO reborn_docus
                       (tenant_id, user_id, agent_id, project_id,
                        name, description, content, content_hash,
                        source, validation_status, consumer_tags,
                        similarity_parent_id, replaces_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
                             'pending', $10::text[], $11::uuid, $12::uuid)
                     ON CONFLICT (tenant_id, user_id, agent_id, project_id, name)
                     DO UPDATE SET
                       description        = EXCLUDED.description,
                       content            = EXCLUDED.content,
                       content_hash       = EXCLUDED.content_hash,
                       source             = EXCLUDED.source,
                       validation_status  = 'pending',
                       consumer_tags      = EXCLUDED.consumer_tags,
                       similarity_parent_id = EXCLUDED.similarity_parent_id,
                       replaces_id        = EXCLUDED.replaces_id,
                       updated_at         = now()
                     RETURNING id::text, content_hash",
                    &[
                        &self.tenant_id,
                        &scope.user_id,
                        &"default",
                        &scope.project_id,
                        &row.name,
                        &row.description,
                        &row.content,
                        &row.content_hash,
                        &row.source,
                        &consumer_tags,
                        &row.similarity_parent_id
                            .as_deref()
                            .and_then(|s| s.parse::<Uuid>().ok()),
                        &row.replaces_id
                            .as_deref()
                            .and_then(|s| s.parse::<Uuid>().ok()),
                    ],
                )
                .await
                .map_err(|e| ComponentDbError::Db(e.to_string()))?;

            Ok(ComponentDbUpsertResult {
                id: result_row.get(0),
                content_hash: result_row.get(1),
                is_new: existing.is_none(),
            })
        }

        async fn mark_stale(
            &self,
            scope: &ComponentDbScope,
        ) -> Result<(), ComponentDbError> {
            // Best-effort — log the error but don't propagate (the caller always
            // uses this in a non-fatal path).
            if let Err(e) = self
                .basic_prompt_store
                .mark_stale(&scope.user_id, &scope.project_id)
                .await
            {
                tracing::debug!(
                    user_id = %scope.user_id,
                    project_id = %scope.project_id,
                    error = %e,
                    "component_db mark_stale: mark_stale failed (non-fatal)"
                );
            }
            Ok(())
        }
    }

    /// Convenience: build a [`PgComponentDbBackend`] from the system-reserved
    /// user_id + the runtime tenant. Called from `factory.rs` at wiring time.
    pub(crate) fn build_pg_component_db_backend(
        pool: Arc<PgPool>,
        tenant_id: impl Into<String>,
        basic_prompt_store: PgBasicPromptStore,
    ) -> PgComponentDbBackend {
        PgComponentDbBackend::new(pool, tenant_id, basic_prompt_store)
    }
}

#[cfg(feature = "postgres")]
pub(crate) use inner::build_pg_component_db_backend;
