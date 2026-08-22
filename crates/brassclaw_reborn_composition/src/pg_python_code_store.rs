//! Postgres-backed store for `reborn_python_code` (class 22 — Phase B / V052).
//!
//! Executable Python bodies for Tier-0 recipe orchestration. This is the
//! save/CRUD-side store (parallel to [`crate::pg_recipe_store::PgRecipeStore`]);
//! retrieval-side projection lives in
//! [`brassclaw_engine::memory::retrieval_source::PostgresSource`].
//!
//! # Delivery filter
//!
//! [`PgPythonCodeStore::fetch_validated`] only returns `validation_status =
//! 'validated'` rows that do NOT carry `05:validator` in `consumer_tags`
//! (SEC-01, §3.9 — same filter as the recipe store).
//!
//! # Scope
//!
//! All queries are scoped by `(tenant_id, user_id, agent_id, project_id)`.
//!
//! # Queue surface (§0.23.5)
//!
//! [`PgPythonCodeStore::create_and_submit`] inserts a new row then submits it
//! to `reborn_validation_queue` (state 1) via
//! [`crate::validation_queue::ValidationQueueStore::submit`] with
//! `proposed_payload = None` (new-component submission). The actual save-path
//! *wiring* (WebUI manual authoring + Sempai auto-creation) lands in Phase K
//! (§0.23.6) for ALL component classes via a generic class→table dispatch;
//! Phase B only delivers this store + the `create_and_submit` surface ready
//! for Phase K to call.
//!
//! # Feature gate
//!
//! The CRUD surface compiles unconditionally (mirrors `pg_recipe_store`).
//! [`PgPythonCodeStore::create_and_submit`] requires the `postgres` feature
//! because it references [`crate::validation_queue::ValidationQueueStore`].

// Phase-B store — CRUD surface unused until the Phase-K save-path wiring
// lands (§0.23.6). Mirrors the `pg_recipe_store` allow(dead_code) pattern.
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
const MAX_PYTHON_CODE_LIST_ROWS: i64 = 1_000;
/// Consumer tag that marks a component as being evaluated by the validator;
/// delivery filter excludes rows carrying this tag (SEC-01, §3.9).
const VALIDATOR_CONSUMER_TAG: &str = "05:validator";

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by `reborn_python_code` store operations.
#[derive(Debug, Error)]
pub(crate) enum PgPythonCodeStoreError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("python_code not found: {id}")]
    NotFound { id: String },
    #[error("validation-queue submit failed: {reason}")]
    Queue { reason: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> PgPythonCodeStoreError {
    PgPythonCodeStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> PgPythonCodeStoreError {
    PgPythonCodeStoreError::Db {
        reason: e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// A fully-decoded `reborn_python_code` row.
///
/// Column order matches [`PYTHON_CODE_SELECT`] / [`decode_python_code_row`].
/// `reborn_python_code` has 25 columns: the 5 scope fields, the 3 content
/// fields, the 2 solution-override columns, class/uid/tags/intent, the
/// post-validation `validation_status`, source + similarity/replaces/audit
/// columns, the dependency registry, and created/updated timestamps. The five
/// queue-tracking columns (`queue_code`, `review_attempts`, `review_feedback`,
/// `rejected_at`, `validation_errors`) are NOT here — they are centralised on
/// `reborn_validation_queue` (§0.18 / V051).
#[derive(Debug, Clone)]
pub(crate) struct PgPythonCode {
    pub(crate) id: Uuid,
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,

    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) content: String,

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

    pub(crate) dependency_registry: Option<Value>,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    pub(crate) updated_at: chrono::DateTime<chrono::Utc>,
}

/// Minimal data required to insert a new `reborn_python_code` row.
///
/// `class_code` (22), `prompt_uid` (sequence default), `validation_status`
/// (`'pending'`), `content_hash` (NULL), and the similarity/replaces/audit
/// columns (NULL/0) are set by DDL defaults — the caller does not supply them.
#[derive(Debug, Clone)]
pub(crate) struct NewPgPythonCode {
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) content: String,
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

/// Canonical SELECT column list — order must match [`decode_python_code_row`].
const PYTHON_CODE_SELECT: &str = "
    id, tenant_id, user_id, agent_id, project_id,
    name, description, content,
    prior_knowledge_content, override_prompt_creation,
    class_code, prompt_uid, consumer_tags, intent_examples,
    validation_status, source, content_hash,
    similarity_parent_id, replaces_id, parent_version,
    last_audit_at, audit_failure_count,
    dependency_registry, created_at, updated_at
";

fn decode_python_code_row(
    row: &tokio_postgres::Row,
) -> Result<PgPythonCode, PgPythonCodeStoreError> {
    Ok(PgPythonCode {
        id: row.get(0),
        tenant_id: row.get(1),
        user_id: row.get(2),
        agent_id: row.get(3),
        project_id: row.get(4),
        name: row.get(5),
        description: row.get(6),
        content: row.get(7),
        prior_knowledge_content: row.get(8),
        override_prompt_creation: row.get(9),
        class_code: row.get(10),
        prompt_uid: row.get(11),
        consumer_tags: row.get(12),
        intent_examples: row.get(13),
        validation_status: row.get(14),
        source: row.get(15),
        content_hash: row.get(16),
        similarity_parent_id: row.get(17),
        replaces_id: row.get(18),
        parent_version: row.get(19),
        last_audit_at: row.get(20),
        audit_failure_count: row.get(21),
        dependency_registry: row.get(22),
        created_at: row.get(23),
        updated_at: row.get(24),
    })
}

// ---------------------------------------------------------------------------
// PgPythonCodeStore
// ---------------------------------------------------------------------------

/// Postgres-backed store for `reborn_python_code` (class 22).
#[derive(Clone)]
pub(crate) struct PgPythonCodeStore {
    pool: Arc<PgPool>,
}

impl PgPythonCodeStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

impl PgPythonCodeStore {
    /// Insert a new python_code row. Returns the assigned UUID.
    ///
    /// `validation_status` defaults to `'pending'` (DDL); `class_code` defaults
    /// to 22; `prompt_uid` defaults to the sequence. The caller-controlled
    /// `consumer_tags` should include `05:validator` for new rows.
    pub(crate) async fn insert(
        &self,
        row: NewPgPythonCode,
    ) -> Result<Uuid, PgPythonCodeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let db_row = client
            .query_one(
                "INSERT INTO reborn_python_code
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, content,
                     prior_knowledge_content, override_prompt_creation,
                     consumer_tags, intent_examples, source, dependency_registry)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
                 RETURNING id",
                &[
                    &row.tenant_id,
                    &row.user_id,
                    &row.agent_id,
                    &row.project_id,
                    &row.name,
                    &row.description,
                    &row.content,
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

    /// Fetch a single python_code row by id + scope.
    pub(crate) async fn get(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<Option<PgPythonCode>, PgPythonCodeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {PYTHON_CODE_SELECT} FROM reborn_python_code
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5"
        );
        let row = client
            .query_opt(&q, &[&id, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_python_code_row).transpose()
    }

    /// Fetch a single python_code row by name + scope.
    pub(crate) async fn get_by_name(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        name: &str,
    ) -> Result<Option<PgPythonCode>, PgPythonCodeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {PYTHON_CODE_SELECT} FROM reborn_python_code
             WHERE name = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5
             LIMIT 1"
        );
        let row = client
            .query_opt(&q, &[&name, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_python_code_row).transpose()
    }

    /// List all python_code rows for the scope (admin / validation-queue path).
    pub(crate) async fn list_all(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<PgPythonCode>, PgPythonCodeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {PYTHON_CODE_SELECT} FROM reborn_python_code
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
             ORDER BY prompt_uid ASC
             LIMIT {MAX_PYTHON_CODE_LIST_ROWS}"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_python_code_row).collect()
    }

    /// List deliverable python_code rows for a consumer (§3.9 SEC-01 filter).
    pub(crate) async fn fetch_validated(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<PgPythonCode>, PgPythonCodeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {PYTHON_CODE_SELECT} FROM reborn_python_code
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
               AND validation_status = 'validated'
               AND NOT ('{VALIDATOR_CONSUMER_TAG}' = ANY(consumer_tags))
             ORDER BY prompt_uid ASC
             LIMIT {MAX_PYTHON_CODE_LIST_ROWS}"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_python_code_row).collect()
    }

    /// Update the post-validation `validation_status` gate on a python_code
    /// row (the gate that STAYS on the component table — §0.18). The
    /// queue-tracking columns live on `reborn_validation_queue`, not here.
    pub(crate) async fn update_validation_status(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        validation_status: &str,
    ) -> Result<(), PgPythonCodeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .query_opt(
                "UPDATE reborn_python_code
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

    /// Remove the `05:validator` consumer tag from a python_code row.
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
    ) -> Result<(), PgPythonCodeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .query_opt(
                "UPDATE reborn_python_code
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
impl PgPythonCodeStore {
    /// Insert a new python_code row AND submit it to the Q1 validation queue
    /// (state 1) in one call — the save-path surface Phase K wires for both
    /// WebUI manual authoring and Sempai auto-creation (§0.23.6).
    ///
    /// `proposed_payload` is `None` (new-component submission per §0.23.5);
    /// the upgrade-copy path (edit of a validated row with a `proposed_payload`)
    /// lands in Phase K/N. Returns the new component UUID.
    pub(crate) async fn create_and_submit(
        &self,
        row: NewPgPythonCode,
        queue_store: &ValidationQueueStore,
    ) -> Result<Uuid, PgPythonCodeStoreError> {
        let scope = ComponentScope {
            tenant_id: row.tenant_id.clone(),
            user_id: row.user_id.clone(),
            agent_id: row.agent_id.clone(),
            project_id: row.project_id.clone(),
        };
        let id = self.insert(row).await?;
        queue_store
            .submit(&scope, id, 22, None)
            .await
            .map_err(|e| PgPythonCodeStoreError::Queue {
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

    /// `PYTHON_CODE_SELECT` must list exactly the 25 `reborn_python_code`
    /// columns in the order [`decode_python_code_row`] reads them. A mismatch
    /// (missing/extra/reordered column) would silently mis-decode every row —
    /// this test pins the contract without needing a live Postgres pool.
    #[test]
    fn python_code_select_round_trips_columns() {
        let cols: Vec<&str> = PYTHON_CODE_SELECT
            .split(',')
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect();
        assert_eq!(cols.len(), 25, "PYTHON_CODE_SELECT must list 25 columns");
        assert_eq!(cols[0], "id");
        assert_eq!(cols[7], "content");
        assert_eq!(cols[14], "validation_status");
        assert_eq!(cols[22], "dependency_registry");
        assert_eq!(cols[24], "updated_at");
    }

    /// The validator consumer tag is load-bearing for the SEC-01 delivery
    /// filter in [`PgPythonCodeStore::fetch_validated`] and
    /// [`PgPythonCodeStore::pop_validator_tag`]; pin the literal.
    #[test]
    fn validator_consumer_tag_is_stable() {
        assert_eq!(VALIDATOR_CONSUMER_TAG, "05:validator");
    }

    // ── Postgres integration tests (skip when docker is unavailable) ──────
    //
    // Mirrors the `validation_queue.rs` harness: each test starts an isolated
    // Postgres-16 testcontainer, runs the full migration set (V000–V052, so
    // `reborn_python_code` and `reborn_validation_queue` both exist), and
    // returns early (pass) when docker/testcontainers is unavailable. They
    // run under the default `postgres` feature and add no failures in a
    // docker-less `cargo test -p brassclaw_reborn_composition` run.

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
        /// every migration (V000–V052). Returns `None` (skip) when docker is
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
                        "skipping pg_python_code_store pg tests: docker/testcontainers unavailable ({error})"
                    );
                    return None;
                }
            };
            let host = match container.get_host().await {
                Ok(h) => h,
                Err(error) => {
                    eprintln!("skipping pg_python_code_store pg tests: no host ({error})");
                    return None;
                }
            };
            let port = match container.get_host_port_ipv4(5432).await {
                Ok(p) => p,
                Err(error) => {
                    eprintln!("skipping pg_python_code_store pg tests: no port ({error})");
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

        /// Build a `NewPgPythonCode` for `scope` with a UUID-derived `name`
        /// (so parallel tests never hit `UNIQUE(scope, name)`) and the
        /// canonical new-row consumer tags `{02:orchestrator, 05:validator}`.
        fn new_row(scope: &ComponentScope) -> NewPgPythonCode {
            NewPgPythonCode {
                tenant_id: scope.tenant_id.clone(),
                user_id: scope.user_id.clone(),
                agent_id: scope.agent_id.clone(),
                project_id: scope.project_id.clone(),
                name: format!("py-leaf-{}", Uuid::new_v4()),
                description: "Reads a file via the host read_file action".into(),
                content:
                    "result = __execute_action__(\"read_file\", {\"path\": path})\nreturn result"
                        .into(),
                prior_knowledge_content: None,
                override_prompt_creation: false,
                consumer_tags: vec!["02:orchestrator".into(), "05:validator".into()],
                intent_examples: None,
                source: "authored".into(),
                dependency_registry: None,
            }
        }

        #[tokio::test]
        async fn python_code_store_round_trip() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = PgPythonCodeStore::new(rig.pool.clone());
            let row = new_row(&scope);
            let expected_name = row.name.clone();
            let expected_content = row.content.clone();

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
            assert_eq!(fetched.content, expected_content);
            assert_eq!(fetched.class_code, 22);
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
        }

        #[tokio::test]
        async fn python_code_create_and_submit_enqueues() {
            let Some(rig) = pg_rig_or_skip().await else {
                return;
            };
            let scope = test_scope();
            let store = PgPythonCodeStore::new(rig.pool.clone());
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
            // state 1 (Q1_pending) with component_class 22 (§0.23.5).
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
                .expect("queue row exists for the new python_code");
            let state: i16 = qrow.get(0);
            let class: i16 = qrow.get(1);
            assert_eq!(state, STATE_Q1_PENDING);
            assert_eq!(class, 22);
        }
    }
}
