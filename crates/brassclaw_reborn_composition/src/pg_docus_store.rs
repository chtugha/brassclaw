//! Postgres-backed store for `reborn_docus` (class 17 — Phase P Step 10).
//!
//! Minimal read+write surface for the WebUI Docs settings tab:
//! - [`PgDocusStore::list_docus`] — list all rows for a tenant scope.
//! - [`PgDocusStore::get_docus`] — fetch a single row by id.
//! - [`PgDocusStore::update_docus_content`] — update `content`, recompute
//!   `content_hash`, set `validation_status = 'pending'`, and submit the row
//!   to the validation queue (state 1). Never writes `'validated'` directly.
//!
//! # Scope
//!
//! All queries are scoped by `tenant_id`. The WebUI operator sees all docs
//! that belong to the tenant (across user/agent/project scope), mirroring the
//! behavior of the validation-queue tab.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature (needs `PgPool` + `ValidationQueueStore`).

#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::validation_queue::ValidationQueueStore;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by `reborn_docus` store operations.
#[derive(Debug, Error)]
pub(crate) enum PgDocusStoreError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("docus not found: {id}")]
    NotFound { id: Uuid },
    #[error("validation-queue submit failed: {reason}")]
    Queue { reason: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> PgDocusStoreError {
    PgDocusStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> PgDocusStoreError {
    PgDocusStoreError::Db {
        reason: e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Row type
// ---------------------------------------------------------------------------

/// A decoded `reborn_docus` row for the WebUI Docs tab.
#[derive(Debug, Clone)]
pub(crate) struct DocusRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) content: String,
    pub(crate) content_hash: Option<String>,
    pub(crate) source: String,
    pub(crate) validation_status: String,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    pub(crate) updated_at: chrono::DateTime<chrono::Utc>,
    /// Scope fields — needed to construct a [`ComponentScope`] for the
    /// validation-queue submit after an update.
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

const DOCUS_SELECT: &str = "
    id, name, description, content, content_hash,
    source, validation_status, created_at, updated_at,
    tenant_id, user_id, agent_id, project_id
";

fn decode_docus_row(row: &tokio_postgres::Row) -> Result<DocusRow, PgDocusStoreError> {
    Ok(DocusRow {
        id: row.get(0),
        name: row.get(1),
        description: row.get(2),
        content: row.get(3),
        content_hash: row.get(4),
        source: row.get(5),
        validation_status: row.get(6),
        created_at: row.get(7),
        updated_at: row.get(8),
        tenant_id: row.get(9),
        user_id: row.get(10),
        agent_id: row.get(11),
        project_id: row.get(12),
    })
}

// ---------------------------------------------------------------------------
// PgDocusStore
// ---------------------------------------------------------------------------

/// Postgres-backed store for `reborn_docus` (class 17).
#[derive(Clone)]
pub(crate) struct PgDocusStore {
    pool: Arc<PgPool>,
    queue: Arc<ValidationQueueStore>,
    tenant_id: String,
}

impl PgDocusStore {
    pub(crate) fn new(
        pool: Arc<PgPool>,
        queue: Arc<ValidationQueueStore>,
        tenant_id: impl Into<String>,
    ) -> Self {
        Self {
            pool,
            queue,
            tenant_id: tenant_id.into(),
        }
    }
}

impl PgDocusStore {
    /// List all `reborn_docus` rows for the tenant. Ordered by `updated_at`
    /// DESC so recently-changed docs surface first.
    pub(crate) async fn list_docus(&self) -> Result<Vec<DocusRow>, PgDocusStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                &format!(
                    "SELECT {DOCUS_SELECT}
                     FROM reborn_docus
                     WHERE tenant_id = $1
                     ORDER BY updated_at DESC"
                ),
                &[&self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_docus_row).collect()
    }

    /// Fetch a single `reborn_docus` row by id. Returns `None` when not found
    /// or when the id belongs to a different tenant.
    pub(crate) async fn get_docus(&self, id: Uuid) -> Result<Option<DocusRow>, PgDocusStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                &format!(
                    "SELECT {DOCUS_SELECT}
                     FROM reborn_docus
                     WHERE tenant_id = $1 AND id = $2"
                ),
                &[&self.tenant_id, &id],
            )
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_docus_row).transpose()
    }

    /// Update the `content` of a `reborn_docus` row.
    ///
    /// Steps:
    /// 1. Verify the row exists and belongs to the tenant.
    /// 2. Compute a SHA-256 hex digest of the new content.
    /// 3. UPDATE `content`, `content_hash`, `validation_status = 'pending'`,
    ///    `updated_at = now()`.
    /// 4. Submit the row to the validation queue (state 1).
    ///
    /// The method never writes `'validated'` directly — all edits go through
    /// the Q1 + Q2 pipeline. The queue submit tolerates `AlreadyQueued` (a
    /// pending copy already exists) and ignores it so a repeated save does
    /// not fail.
    pub(crate) async fn update_docus_content(
        &self,
        id: Uuid,
        content: &str,
    ) -> Result<(), PgDocusStoreError> {
        // Step 1 — verify the row exists and belongs to the tenant.
        let existing = self
            .get_docus(id)
            .await?
            .ok_or(PgDocusStoreError::NotFound { id })?;

        // Step 2 — SHA-256 hex digest.
        let new_hash = sha256_hex(content);

        // Step 3 — UPDATE the row.
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE reborn_docus
                 SET content           = $1,
                     content_hash      = $2,
                     validation_status = 'pending',
                     updated_at        = now()
                 WHERE tenant_id = $3 AND id = $4",
                &[&content, &new_hash, &self.tenant_id, &id],
            )
            .await
            .map_err(map_pg)?;

        // Step 4 — submit to validation queue. AlreadyQueued is non-fatal
        // (the row is already awaiting review — just leave it).
        let scope = ComponentScope {
            tenant_id: existing.tenant_id.clone(),
            user_id: existing.user_id.clone(),
            agent_id: existing.agent_id.clone(),
            project_id: existing.project_id.clone(),
        };
        match self.queue.submit(&scope, id, 17, None).await {
            Ok(()) => {}
            Err(crate::validation_queue::ValidationQueueError::AlreadyQueued { .. }) => {
                // Row is already in the queue — that's fine, the content
                // update is persisted. Leave the existing queue entry.
            }
            Err(e) => {
                return Err(PgDocusStoreError::Queue {
                    reason: e.to_string(),
                });
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute a SHA-256 hex digest without a heavy crypto dependency.
/// Uses the `sha2` crate that is already in the Cargo workspace.
fn sha256_hex(input: &str) -> String {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(digest)
}

// ---------------------------------------------------------------------------
// DocusStore trait impl
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl brassclaw_product_workflow::DocusStore for PgDocusStore {
    async fn list_docus(
        &self,
    ) -> Result<
        brassclaw_product_workflow::DocusListResponse,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let rows = self.list_docus().await?;
        let items = rows.into_iter().map(docu_row_to_item).collect();
        Ok(brassclaw_product_workflow::DocusListResponse { items })
    }

    async fn get_docus(
        &self,
        id: uuid::Uuid,
    ) -> Result<
        Option<brassclaw_product_workflow::DocusItem>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let row = self.get_docus(id).await?;
        Ok(row.map(docu_row_to_item))
    }

    async fn update_docus_content(
        &self,
        id: uuid::Uuid,
        content: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.update_docus_content(id, &content).await?;
        Ok(())
    }
}

fn docu_row_to_item(row: DocusRow) -> brassclaw_product_workflow::DocusItem {
    brassclaw_product_workflow::DocusItem {
        id: row.id.to_string(),
        name: row.name,
        description: row.description,
        content: row.content,
        content_hash: row.content_hash,
        source: row.source,
        validation_status: row.validation_status,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }
}
