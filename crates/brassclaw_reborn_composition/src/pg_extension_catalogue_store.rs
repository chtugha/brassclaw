//!
//! Documentation-container that organises a capability domain (§0.2). This is
//! the save/CRUD-side store (parallel to [`crate::pg_recipe_store::PgRecipeStore`]
//! and [`crate::pg_python_code_store::PgPythonCodeStore`]); retrieval-side
//! projection lives in
//! [`brassclaw_engine::memory::retrieval_source::PostgresSource`].
//!
//! Unlike `reborn_python_code`, this table has **no plain `content` column** —
//! `overview_doc` is the primary text field (maps to `effective_content` in
//! the `fetch_for_consumer` UNION ALL via
//! `COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)`). The
//! structured extras `task_groups` (JSONB), `child_component_ids` (UUID[]),
//! and `intent_index` (JSONB, audit-only) are the catalogue-specific columns.
//!
//! # Delivery filter
//!
//! [`PgExtensionCatalogueStore::fetch_validated`] only returns
//! `validation_status = 'validated'` rows that do NOT carry `05:validator` in
//! `consumer_tags` (SEC-01, §3.9 — same filter as the recipe / python_code
//! stores).
//!
//! # Scope
//!
//! All queries are scoped by `(tenant_id, user_id, agent_id, project_id)`.
//!
//! # Queue surface (§0.23.5)
//!
//! [`PgExtensionCatalogueStore::create_and_submit`] inserts a new row then
//! submits it to `reborn_validation_queue` (state 1) via
//! [`crate::validation_queue::ValidationQueueStore::submit`] with
//! `proposed_payload = None` (new-component submission). The actual save-path
//! *wiring* (WebUI manual authoring + Sempai auto-creation) lands in Phase K
//! (§0.23.6) for ALL component classes via a generic class→table dispatch;
//! Phase C only delivers this store + the `create_and_submit` surface ready
//! for Phase K to call.
//!
//! # Feature gate
//!
//! The CRUD surface compiles unconditionally (mirrors `pg_recipe_store`).
//! [`PgExtensionCatalogueStore::create_and_submit`] requires the `postgres`
//! feature because it references
//! [`crate::validation_queue::ValidationQueueStore`].

// Phase-C store — CRUD surface unused until the Phase-K save-path wiring
// lands (§0.23.6). Mirrors the `pg_recipe_store` / `pg_python_code_store`
// allow(dead_code) pattern.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::validation_queue::ValidationQueueStore;

/// Hard cap on how many rows `list_all` / `fetch_validated` may return in a
/// single call. Guards against accidental full-table scans on large tenants.
const MAX_EXTENSION_CATALOGUE_LIST_ROWS: i64 = 1_000;
/// Consumer tag that marks a component as being evaluated by the validator;
/// delivery filter excludes rows carrying this tag (SEC-01, §3.9).
const VALIDATOR_CONSUMER_TAG: &str = "05:validator";

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by `reborn_extension_catalogues` store operations.
#[derive(Debug, Error)]
pub(crate) enum PgExtensionCatalogueStoreError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("extension_catalogue not found: {id}")]
    NotFound { id: String },
    #[error("validation-queue submit failed: {reason}")]
    Queue { reason: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> PgExtensionCatalogueStoreError {
    PgExtensionCatalogueStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> PgExtensionCatalogueStoreError {
    PgExtensionCatalogueStoreError::Db {
        reason: e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// A fully-decoded `reborn_extension_catalogues` row.
///
/// Column order matches [`EXTENSION_CATALOGUE_SELECT`] /
/// [`decode_extension_catalogue_row`]. `reborn_extension_catalogues` has 30
/// columns: the 5 scope fields, `name`/`description`/`version`, the primary
/// text `overview_doc`, the structured extras `task_groups`/`child_component_ids`/
/// `intent_index`, the 2 solution-override columns, class/uid/tags/intent, the
/// post-validation `validation_status`, source + similarity/replaces/audit
/// columns, the dependency registry, and created/updated timestamps. The five
/// queue-tracking columns (`queue_code`, `review_attempts`, `review_feedback`,
/// `rejected_at`, `validation_errors`) are NOT here — they are centralised on
/// `reborn_validation_queue` (§0.18 / V051).
#[derive(Debug, Clone)]
pub(crate) struct PgExtensionCatalogue {
    pub(crate) id: Uuid,
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,

    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) version: String,

    pub(crate) overview_doc: String,
    pub(crate) task_groups: Value,
    pub(crate) child_component_ids: Vec<Uuid>,
    pub(crate) intent_index: Option<Value>,

    pub(crate) prior_knowledge_content: Option<String>,
    pub(crate) override_prompt_creation: bool,

    pub(crate) class_code: i16,
    pub(crate) prompt_uid: i64,
    pub(crate) consumer_tags: Vec<String>,
    pub(crate) intent_examples: Option<Value>,

    pub(crate) validation_status: String,
    pub(crate) source: String,
    pub(crate) content_hash: Option<String>,
    pub(crate) similarity_parent_id: Option<Uuid>,
    pub(crate) replaces_id: Option<Uuid>,
    pub(crate) parent_version: Option<String>,
    pub(crate) last_audit_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) audit_failure_count: i16,
    pub(crate) parent_mission_id: Option<Uuid>,

    pub(crate) dependency_registry: Option<Value>,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    pub(crate) updated_at: chrono::DateTime<chrono::Utc>,
}

/// Minimal data required to insert a new `reborn_extension_catalogues` row.
///
/// `class_code` (23), `prompt_uid` (sequence default), `validation_status`
/// (`'pending'`), `content_hash` (NULL), and the similarity/replaces/audit
/// columns (NULL/0) are set by DDL defaults — the caller does not supply them.
#[derive(Debug, Clone)]
pub(crate) struct NewPgExtensionCatalogue {
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) version: String,
    pub(crate) overview_doc: String,
    pub(crate) task_groups: Value,
    pub(crate) child_component_ids: Vec<Uuid>,
    pub(crate) intent_index: Option<Value>,
    pub(crate) prior_knowledge_content: Option<String>,
    pub(crate) override_prompt_creation: bool,
    /// Consumer tags — caller must include `05:validator` for new rows so the
    /// SEC-01 delivery filter hides the row until it graduates (§3.9).
    pub(crate) consumer_tags: Vec<String>,
    pub(crate) intent_examples: Option<Value>,
    pub(crate) source: String,
    pub(crate) dependency_registry: Option<Value>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Canonical SELECT column list — order must match
/// [`decode_extension_catalogue_row`].
const EXTENSION_CATALOGUE_SELECT: &str = "
    id, tenant_id, user_id, agent_id, project_id,
    name, description, version,
    overview_doc, task_groups, child_component_ids, intent_index,
    prior_knowledge_content, override_prompt_creation,
    class_code, prompt_uid, consumer_tags, intent_examples,
    validation_status, source, content_hash,
    similarity_parent_id, replaces_id, parent_version,
    last_audit_at, audit_failure_count, parent_mission_id,
    dependency_registry, created_at, updated_at
";

fn decode_extension_catalogue_row(
    row: &tokio_postgres::Row,
) -> Result<PgExtensionCatalogue, PgExtensionCatalogueStoreError> {
    Ok(PgExtensionCatalogue {
        id: row.get(0),
        tenant_id: row.get(1),
        user_id: row.get(2),
        agent_id: row.get(3),
        project_id: row.get(4),
        name: row.get(5),
        description: row.get(6),
        version: row.get(7),
        overview_doc: row.get(8),
        task_groups: row.get(9),
        child_component_ids: row.get(10),
        intent_index: row.get(11),
        prior_knowledge_content: row.get(12),
        override_prompt_creation: row.get(13),
        class_code: row.get(14),
        prompt_uid: row.get(15),
        consumer_tags: row.get(16),
        intent_examples: row.get(17),
        validation_status: row.get(18),
        source: row.get(19),
        content_hash: row.get(20),
        similarity_parent_id: row.get(21),
        replaces_id: row.get(22),
        parent_version: row.get(23),
        last_audit_at: row.get(24),
        audit_failure_count: row.get(25),
        parent_mission_id: row.get(26),
        dependency_registry: row.get(27),
        created_at: row.get(28),
        updated_at: row.get(29),
    })
}

// ---------------------------------------------------------------------------
// PgExtensionCatalogueStore
// ---------------------------------------------------------------------------

/// Postgres-backed store for `reborn_extension_catalogues` (class 23).
#[derive(Clone)]
pub(crate) struct PgExtensionCatalogueStore {
    pool: Arc<PgPool>,
}

impl PgExtensionCatalogueStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

impl PgExtensionCatalogueStore {
    /// Insert a new extension_catalogue row. Returns the assigned UUID.
    ///
    /// `validation_status` defaults to `'pending'` (DDL); `class_code` defaults
    /// to 23; `prompt_uid` defaults to the sequence. The caller-controlled
    /// `consumer_tags` should include `05:validator` for new rows.
    pub(crate) async fn insert(
        &self,
        row: NewPgExtensionCatalogue,
    ) -> Result<Uuid, PgExtensionCatalogueStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let db_row = client
            .query_one(
                "INSERT INTO reborn_extension_catalogues
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, version,
                     overview_doc, task_groups, child_component_ids, intent_index,
                     prior_knowledge_content, override_prompt_creation,
                     consumer_tags, intent_examples, source, dependency_registry)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
                 RETURNING id",
                &[
                    &row.tenant_id,
                    &row.user_id,
                    &row.agent_id,
                    &row.project_id,
                    &row.name,
                    &row.description,
                    &row.version,
                    &row.overview_doc,
                    &row.task_groups,
                    &row.child_component_ids,
                    &row.intent_index,
                    &row.prior_knowledge_content,
                    &row.override_prompt_creation,
                    &row.consumer_tags,
                    &row.intent_examples,
                    &row.source,
                    &row.dependency_registry,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(db_row.get(0))
    }

    /// Fetch a single extension_catalogue row by id + scope.
    pub(crate) async fn get(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<Option<PgExtensionCatalogue>, PgExtensionCatalogueStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {EXTENSION_CATALOGUE_SELECT} FROM reborn_extension_catalogues
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5"
        );
        let row = client
            .query_opt(&q, &[&id, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_extension_catalogue_row).transpose()
    }

    /// Fetch a single extension_catalogue row by name + scope.
    pub(crate) async fn get_by_name(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        name: &str,
    ) -> Result<Option<PgExtensionCatalogue>, PgExtensionCatalogueStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {EXTENSION_CATALOGUE_SELECT} FROM reborn_extension_catalogues
             WHERE name = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5
             LIMIT 1"
        );
        let row = client
            .query_opt(&q, &[&name, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_extension_catalogue_row).transpose()
    }

    /// List all extension_catalogue rows for the scope (admin / validation-queue
    /// path).
    pub(crate) async fn list_all(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<PgExtensionCatalogue>, PgExtensionCatalogueStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {EXTENSION_CATALOGUE_SELECT} FROM reborn_extension_catalogues
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
             ORDER BY prompt_uid ASC
             LIMIT {MAX_EXTENSION_CATALOGUE_LIST_ROWS}"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_extension_catalogue_row).collect()
    }

    /// List deliverable extension_catalogue rows for a consumer (§3.9 SEC-01
    /// filter).
    pub(crate) async fn fetch_validated(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<PgExtensionCatalogue>, PgExtensionCatalogueStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {EXTENSION_CATALOGUE_SELECT} FROM reborn_extension_catalogues
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
               AND validation_status = 'validated'
               AND NOT ('{VALIDATOR_CONSUMER_TAG}' = ANY(consumer_tags))
             ORDER BY prompt_uid ASC
             LIMIT {MAX_EXTENSION_CATALOGUE_LIST_ROWS}"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_extension_catalogue_row).collect()
    }

    /// Update the post-validation `validation_status` gate on an
    /// extension_catalogue row (the gate that STAYS on the component table —
    /// §0.18). The queue-tracking columns live on `reborn_validation_queue`,
    /// not here.
    pub(crate) async fn update_validation_status(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        validation_status: &str,
    ) -> Result<(), PgExtensionCatalogueStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .query_opt(
                "UPDATE reborn_extension_catalogues
                 SET validation_status = $1
                 WHERE id = $2
                   AND tenant_id = $3 AND user_id = $4
                   AND agent_id  = $5 AND project_id = $6",
                &[
                    &validation_status,
                    &id,
                    &tenant_id,
                    &user_id,
                    &agent_id,
                    &project_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    /// Remove the `05:validator` consumer tag from an extension_catalogue row.
    ///
    /// Called at graduation (Phase N) once a component passes Q2 review: the
    /// row transitions from "hidden while under validation" to "deliverable to
    /// consumers" (SEC-01 delivery filter — §3.9).
    pub(crate) async fn pop_validator_tag(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<(), PgExtensionCatalogueStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .query_opt(
                "UPDATE reborn_extension_catalogues
                 SET consumer_tags = array_remove(consumer_tags, $1)
                 WHERE id = $2
                   AND tenant_id = $3 AND user_id = $4
                   AND agent_id  = $5 AND project_id = $6",
                &[
                    &VALIDATOR_CONSUMER_TAG,
                    &id,
                    &tenant_id,
                    &user_id,
                    &agent_id,
                    &project_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }
}

/// Phase-K-ready queue surface (§0.23.5). The whole module is gated to the
/// `postgres` feature, so [`ValidationQueueStore`] is in scope here.
impl PgExtensionCatalogueStore {
    /// Insert a new extension_catalogue row AND submit it to the Q1 validation
    /// queue (state 1) in one call — the save-path surface Phase K wires for
    /// both WebUI manual authoring and Sempai auto-creation (§0.23.6).
    ///
    /// `proposed_payload` is `None` (new-component submission per §0.23.5);
    /// the upgrade-copy path (edit of a validated row with a
    /// `proposed_payload`) lands in Phase K/N. Returns the new component UUID.
    pub(crate) async fn create_and_submit(
        &self,
        row: NewPgExtensionCatalogue,
        queue_store: &ValidationQueueStore,
    ) -> Result<Uuid, PgExtensionCatalogueStoreError> {
        let scope = ComponentScope {
            tenant_id: row.tenant_id.clone(),
            user_id: row.user_id.clone(),
            agent_id: row.agent_id.clone(),
            project_id: row.project_id.clone(),
        };
        let id = self.insert(row).await?;
        queue_store
            .submit(&scope, id, 23, None)
            .await
            .map_err(|e| PgExtensionCatalogueStoreError::Queue {
                reason: e.to_string(),
            })?;
        Ok(id)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// `EXTENSION_CATALOGUE_SELECT` must list exactly the 30
    /// `reborn_extension_catalogues` columns in the order
    /// [`decode_extension_catalogue_row`] reads them. A mismatch
    /// (missing/extra/reordered column) would silently mis-decode every row —
    /// this test pins the contract without needing a live Postgres pool.
    #[test]
    fn extension_catalogue_select_round_trips_columns() {
        let cols: Vec<&str> = EXTENSION_CATALOGUE_SELECT
            .split(',')
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect();
        assert_eq!(
            cols.len(),
            30,
            "EXTENSION_CATALOGUE_SELECT must list 30 columns"
        );
        assert_eq!(cols[0], "id");
        assert_eq!(cols[8], "overview_doc");
        assert_eq!(cols[18], "validation_status");
        assert_eq!(cols[27], "dependency_registry");
        assert_eq!(cols[29], "updated_at");
    }

    /// The validator consumer tag is load-bearing for the SEC-01 delivery
    /// filter in [`PgExtensionCatalogueStore::fetch_validated`] and
    /// [`PgExtensionCatalogueStore::pop_validator_tag`]; pin the literal.
    #[test]
    fn validator_consumer_tag_is_stable() {
        assert_eq!(VALIDATOR_CONSUMER_TAG, "05:validator");
    }

    // ── Postgres integration tests (skip when docker is unavailable) ──────
    //
    // Mirrors the `validation_queue.rs` / `pg_python_code_store.rs` harness:
    // each test starts an isolated Postgres-16 testcontainer, runs the full
    // migration set (V000–V053, so `reborn_extension_catalogues` and
    // `reborn_validation_queue` both exist), and returns early (pass) when
    // docker/testcontainers is unavailable. They run under the default
    // `postgres` feature and add no failures in a docker-less
    // `cargo test -p brassclaw_reborn_composition` run.

    mod pg {
        use super::*;
        use crate::validation_queue::{STATE_Q1_PENDING, ValidationQueueStore};
        use brassclaw_engine::memory::retrieval_source::ComponentScope;
        use brassclaw_pg::PgPool;

        struct PgRig {
            // Held for the test's lifetime so the container stays up.
            _container: testcontainers_modules::testcontainers::ContainerAsync<
                testcontainers_modules::postgres::Postgres,
            >,
            pool: Arc<PgPool>,
        }

        /// Start an isolated Postgres-16 testcontainer, build a pool, and run
        /// every migration (V000–V053). Returns `None` (skip) when docker is
        /// unavailable.
        async fn pg_rig_or_skip() -> Option<PgRig> {
            use deadpool_postgres::{Manager, Pool};
            use testcontainers_modules::testcontainers::{ImageExt, runners::AsyncRunner};

            let image = testcontainers_modules::postgres::Postgres::default()
                .with_db_name("brassclaw_test")
                .with_user("postgres")
                .with_password("postgres")
                .with_tag("16-alpine");
            let container = match image.start().await {
                Ok(c) => c,
                Err(error) => {
                    eprintln!(
                        "skipping pg_extension_catalogue_store pg tests: docker/testcontainers unavailable ({error})"
                    );
                    return None;
                }
            };
            let host = match container.get_host().await {
                Ok(h) => h,
                Err(error) => {
                    eprintln!("skipping pg_extension_catalogue_store pg tests: no host ({error})");
                    return None;
                }
            };
            let port = match container.get_host_port_ipv4(5432).await {
                Ok(p) => p,
                Err(error) => {
                    eprintln!("skipping pg_extension_catalogue_store pg tests: no port ({error})");
                    return None;
                }
            };
            let url = format!("postgres://postgres:postgres@{host}:{port}/brassclaw_test");
            let cfg: tokio_postgres::Config = url.parse().expect("testcontainer url parses");
            let manager = Manager::new(cfg, tokio_postgres::NoTls);
            let pool = Pool::builder(manager)
                .max_size(4)
                .build()
                .expect("Postgres pool must build");
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .expect("migrations must apply");
            Some(PgRig {
                _container: container,
                pool: Arc::new(pool),
            })
        }

        fn test_scope() -> ComponentScope {
            ComponentScope {
                tenant_id: "t".into(),
                user_id: "u".into(),
                agent_id: "a".into(),
                project_id: "p".into(),
            }
        }

        /// Build a `NewPgExtensionCatalogue` for `scope` with a UUID-derived
        /// `name` (so parallel tests never hit `UNIQUE(scope, name)`) and the
        /// canonical new-row consumer tags `{02:orchestrator, 05:validator}`.
        /// Includes one task group + one child component id, the catalogue
        /// shape required for Q1 validation (Phase C / COMP-04).
        fn new_row(scope: &ComponentScope) -> NewPgExtensionCatalogue {
            let task_groups = serde_json::json!([
                {
                    "group_name": "file-management",
                    "summary": "Local file management recipes",
                    "recipe_ids": ["recipe-read-file", "recipe-write-file"]
                }
            ]);
            let child = Uuid::new_v4();
            NewPgExtensionCatalogue {
                tenant_id: scope.tenant_id.clone(),
                user_id: scope.user_id.clone(),
                agent_id: scope.agent_id.clone(),
                project_id: scope.project_id.clone(),
                name: format!("cat-{}", Uuid::new_v4()),
                description: "Catalogue covering local file management".into(),
                version: "1.0".into(),
                overview_doc:
                    "This catalogue covers local file management. Its Recipes handle these task groups..."
                        .into(),
                task_groups,
                child_component_ids: vec![child],
                intent_index: None,
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
                intent_examples: None,
                source: "authored".into(),
                dependency_registry: None,
            }
        }

        #[tokio::test]
        async fn extension_catalogue_store_round_trip() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = PgExtensionCatalogueStore::new(rig.pool.clone());
            let row = new_row(&scope);
            let expected_name = row.name.clone();
            let expected_overview = row.overview_doc.clone();
            let expected_child = row.child_component_ids[0];

            let id = store.insert(row).await.expect("insert");
            let fetched = store
                .get(
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    id,
                )
                .await
                .expect("get")
                .expect("row present after insert");

            assert_eq!(fetched.id, id);
            assert_eq!(fetched.name, expected_name);
            assert_eq!(fetched.overview_doc, expected_overview);
            assert_eq!(fetched.class_code, 23);
            assert_eq!(fetched.version, "1.0");
            assert_eq!(fetched.validation_status, "pending");
            assert_eq!(fetched.source, "authored");
            assert!(!fetched.override_prompt_creation);
            assert_eq!(fetched.audit_failure_count, 0);
            assert!(fetched.prompt_uid > 0);
            assert!(
                fetched
                    .consumer_tags
                    .contains(&"02:orchestrator".to_string())
            );
            assert!(fetched.consumer_tags.contains(&"05:validator".to_string()));
            // Structured extras round-trip through JSONB / UUID[].
            assert_eq!(
                fetched.task_groups,
                serde_json::json!([
                    {
                        "group_name": "file-management",
                        "summary": "Local file management recipes",
                        "recipe_ids": ["recipe-read-file", "recipe-write-file"]
                    }
                ])
            );
            assert_eq!(fetched.child_component_ids, vec![expected_child]);
            assert!(fetched.intent_index.is_none());
        }

        #[tokio::test]
        async fn extension_catalogue_create_and_submit_enqueues() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = PgExtensionCatalogueStore::new(rig.pool.clone());
            let queue = ValidationQueueStore::new(rig.pool.clone());

            // `create_and_submit` inserts the component AND enqueues a Q1 row.
            let id = store
                .create_and_submit(new_row(&scope), &queue)
                .await
                .expect("create_and_submit");

            // The component row exists at 'pending'.
            let fetched = store
                .get(
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                    id,
                )
                .await
                .expect("get")
                .expect("row present");
            assert_eq!(fetched.validation_status, "pending");

            // A reborn_validation_queue row exists for this component at
            // state 1 (Q1_pending) with component_class 23 (§0.23.5).
            let client = rig.pool.get().await.expect("pool client");
            let qrow = client
                .query_one(
                    "SELECT state, component_class FROM reborn_validation_queue
                     WHERE component_id = $1
                       AND tenant_id = $2 AND user_id = $3
                       AND agent_id  = $4 AND project_id = $5",
                    &[
                        &id,
                        &scope.tenant_id,
                        &scope.user_id,
                        &scope.agent_id,
                        &scope.project_id,
                    ],
                )
                .await
                .expect("queue row exists for the new extension_catalogue");
            let state: i16 = qrow.get(0);
            let class: i16 = qrow.get(1);
            assert_eq!(state, STATE_Q1_PENDING);
            assert_eq!(class, 23);
        }
    }
}
