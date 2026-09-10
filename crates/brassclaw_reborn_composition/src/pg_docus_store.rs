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

// ---------------------------------------------------------------------------
// Tests (Phase P Step 11 — store-level security guard + pipeline)
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use std::sync::Arc;

    use deadpool_postgres::Manager;
    use uuid::Uuid;

    use super::*;
    use crate::validation_queue::ValidationQueueStore;

    // ── minimal Postgres test-rig ────────────────────────────────────────────

    struct PgRig {
        _container: testcontainers_modules::testcontainers::ContainerAsync<
            testcontainers_modules::postgres::Postgres,
        >,
        pool: Arc<brassclaw_pg::PgPool>,
    }

    /// Start a Postgres-16 testcontainer and run migrations.
    /// Returns `None` (skip) when docker / testcontainers is unavailable.
    async fn pg_rig_or_skip() -> Option<PgRig> {
        use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

        let image = testcontainers_modules::postgres::Postgres::default()
            .with_db_name("brassclaw_test")
            .with_user("postgres")
            .with_password("postgres")
            .with_tag("16-alpine");
        let container = match image.start().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("skipping pg_docus_store tests: docker unavailable ({e})");
                return None;
            }
        };
        let host = match container.get_host().await {
            Ok(h) => h,
            Err(e) => {
                eprintln!("skipping pg_docus_store tests: no host ({e})");
                return None;
            }
        };
        let port = match container.get_host_port_ipv4(5432).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skipping pg_docus_store tests: no port ({e})");
                return None;
            }
        };
        let url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");
        let cfg: tokio_postgres::Config = url.parse().expect("url parses");
        let mgr = Manager::new(cfg, tokio_postgres::NoTls);
        let pool = deadpool_postgres::Pool::builder(mgr)
            .max_size(4)
            .build()
            .expect("pool builds");
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .expect("migrations must succeed");
        Some(PgRig {
            _container: container,
            pool: Arc::new(pool),
        })
    }

    /// Insert a minimal `reborn_docus` row and return its UUID.
    async fn insert_docus_row(pool: &Arc<brassclaw_pg::PgPool>, tenant: &str) -> Uuid {
        let client = pool.get().await.expect("pool client");
        let name = format!("test-doc-{}", Uuid::new_v4());
        let row = client
            .query_one(
                "INSERT INTO reborn_docus
                     (tenant_id, user_id, agent_id, project_id, name, content,
                      source, validation_status)
                 VALUES ($1, 'u', 'a', 'p', $2, 'initial content',
                         'system', 'validated')
                 RETURNING id",
                &[&tenant, &name],
            )
            .await
            .expect("insert docus row");
        row.get(0)
    }

    /// Read the current `validation_status` of a `reborn_docus` row by id.
    async fn read_status(
        pool: &Arc<brassclaw_pg::PgPool>,
        tenant: &str,
        id: Uuid,
    ) -> String {
        let client = pool.get().await.expect("pool client");
        let row = client
            .query_one(
                "SELECT validation_status FROM reborn_docus
                 WHERE tenant_id = $1 AND id = $2",
                &[&tenant, &id],
            )
            .await
            .expect("read status");
        row.get(0)
    }

    // ── test: update_docus_content never writes 'validated' ─────────────────

    /// **Security guard (Phase P §0.22 invariant):**
    /// `update_docus_content` must always set `validation_status = 'pending'`
    /// even when the row starts as `'validated'`. It must never write
    /// `'validated'` directly (all edits go through Q1+Q2).
    #[tokio::test]
    async fn update_content_always_sets_pending_never_validated() {
        let Some(rig) = pg_rig_or_skip().await else {
            return;
        };
        let tenant = format!("t-{}", Uuid::new_v4());
        let id = insert_docus_row(&rig.pool, &tenant).await;

        // Precondition: row starts as 'validated'.
        assert_eq!(read_status(&rig.pool, &tenant, id).await, "validated");

        let queue = Arc::new(ValidationQueueStore::new(Arc::clone(&rig.pool)));
        let store = PgDocusStore::new(Arc::clone(&rig.pool), queue, &tenant);

        store
            .update_docus_content(id, "updated content")
            .await
            .expect("update_docus_content must succeed");

        // The row must now be 'pending' — never 'validated'.
        let status = read_status(&rig.pool, &tenant, id).await;
        assert_eq!(
            status, "pending",
            "update_docus_content must always set validation_status = 'pending', got: {status}"
        );
    }

    // ── test: update_docus_content submits to validation queue ──────────────

    /// After `update_docus_content`, the validation queue must contain a row
    /// for the updated doc (state = 1 = Q1 pending). This proves the
    /// Q1+Q2 pipeline is entered.
    #[tokio::test]
    async fn update_content_submits_to_validation_queue() {
        let Some(rig) = pg_rig_or_skip().await else {
            return;
        };
        let tenant = format!("t-{}", Uuid::new_v4());
        let id = insert_docus_row(&rig.pool, &tenant).await;

        let queue = Arc::new(ValidationQueueStore::new(Arc::clone(&rig.pool)));
        let store = PgDocusStore::new(Arc::clone(&rig.pool), queue.clone(), &tenant);

        store
            .update_docus_content(id, "new content for queue test")
            .await
            .expect("update must succeed");

        // Validation queue must contain an entry for this doc (class 17).
        let scope = brassclaw_engine::memory::retrieval_source::ComponentScope {
            tenant_id: tenant.clone(),
            user_id: "u".to_string(),
            agent_id: "a".to_string(),
            project_id: "p".to_string(),
        };
        let rows = queue.list(&scope, None).await.expect("list queue");
        let entry = rows.iter().find(|r| r.component_id == id);
        assert!(
            entry.is_some(),
            "validation queue must contain an entry for the updated docus row"
        );
        let entry = entry.unwrap();
        assert_eq!(
            entry.component_class, 17,
            "queue entry class_code must be 17 (Docu)"
        );
        assert_eq!(
            entry.state, 1,
            "queue entry state must be 1 (Q1 pending) after update"
        );
    }

    // ── test: second save is idempotent (AlreadyQueued is swallowed) ────────

    /// A second `update_docus_content` while a pending queue entry already
    /// exists must not fail — `AlreadyQueued` is swallowed silently so the
    /// operator can re-save without an error (the pending queue entry remains,
    /// but the content is overwritten).
    #[tokio::test]
    async fn second_update_with_pending_queue_entry_is_idempotent() {
        let Some(rig) = pg_rig_or_skip().await else {
            return;
        };
        let tenant = format!("t-{}", Uuid::new_v4());
        let id = insert_docus_row(&rig.pool, &tenant).await;

        let queue = Arc::new(ValidationQueueStore::new(Arc::clone(&rig.pool)));
        let store = PgDocusStore::new(Arc::clone(&rig.pool), queue.clone(), &tenant);

        // First save — queues the row.
        store
            .update_docus_content(id, "first edit")
            .await
            .expect("first update must succeed");

        // Second save — must not error even though a pending entry already exists.
        store
            .update_docus_content(id, "second edit — should be idempotent")
            .await
            .expect("second update must succeed (AlreadyQueued swallowed)");

        // Content must reflect the latest write.
        let row = store
            .get_docus(id)
            .await
            .expect("get must succeed")
            .expect("row must exist");
        assert_eq!(
            row.content, "second edit — should be idempotent",
            "content must reflect the latest write"
        );
        assert_eq!(row.validation_status, "pending");
    }

    // ── test: content_hash is updated on each save ───────────────────────────

    /// `update_docus_content` must recompute and persist a new `content_hash`
    /// for the updated content. This proves the hash-change-detection path in
    /// `doc-sync` will see a different stored hash after an edit.
    #[tokio::test]
    async fn update_content_recomputes_content_hash() {
        let Some(rig) = pg_rig_or_skip().await else {
            return;
        };
        let tenant = format!("t-{}", Uuid::new_v4());
        let id = insert_docus_row(&rig.pool, &tenant).await;

        let queue = Arc::new(ValidationQueueStore::new(Arc::clone(&rig.pool)));
        let store = PgDocusStore::new(Arc::clone(&rig.pool), queue, &tenant);

        // Capture the original hash (may be None since the row was inserted
        // without one).
        let original = store.get_docus(id).await.expect("get").expect("row");
        let original_hash = original.content_hash.clone();

        store
            .update_docus_content(id, "hash-change-test content v2")
            .await
            .expect("update must succeed");

        let updated = store.get_docus(id).await.expect("get").expect("row");
        let new_hash = updated.content_hash.clone();

        assert!(
            new_hash.is_some(),
            "content_hash must be set after update_docus_content"
        );
        assert_ne!(
            original_hash, new_hash,
            "content_hash must change when content changes"
        );
        // Verify the hash is a valid 64-char lowercase hex string (SHA-256).
        let h = new_hash.unwrap();
        assert_eq!(h.len(), 64, "SHA-256 hex digest must be 64 chars, got: {h}");
        assert!(
            h.chars().all(|c| c.is_ascii_hexdigit()),
            "content_hash must be hex, got: {h}"
        );
    }

    // ── test: get_docus returns None for unknown id ──────────────────────────

    #[tokio::test]
    async fn get_docus_returns_none_for_unknown_id() {
        let Some(rig) = pg_rig_or_skip().await else {
            return;
        };
        let tenant = format!("t-{}", Uuid::new_v4());
        let queue = Arc::new(ValidationQueueStore::new(Arc::clone(&rig.pool)));
        let store = PgDocusStore::new(Arc::clone(&rig.pool), queue, &tenant);

        let result = store
            .get_docus(Uuid::new_v4())
            .await
            .expect("get must not error on missing row");
        assert!(result.is_none(), "get_docus must return None for an unknown id");
    }

    // ── test: update_docus_content fails with NotFound for unknown id ────────

    #[tokio::test]
    async fn update_content_fails_with_not_found_for_unknown_id() {
        let Some(rig) = pg_rig_or_skip().await else {
            return;
        };
        let tenant = format!("t-{}", Uuid::new_v4());
        let queue = Arc::new(ValidationQueueStore::new(Arc::clone(&rig.pool)));
        let store = PgDocusStore::new(Arc::clone(&rig.pool), queue, &tenant);

        let err = store
            .update_docus_content(Uuid::new_v4(), "content")
            .await
            .expect_err("update for unknown id must fail");
        assert!(
            matches!(err, PgDocusStoreError::NotFound { .. }),
            "expected NotFound, got: {err:?}"
        );
    }
}
