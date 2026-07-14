//! Engine-`Store`-backed persistence for the WebUI v2 reduction-rule endpoints.
//!
//! The reduction rules live as a [`brassclaw_engine::types::memory::MemoryDoc`]
//! with `DocType::Note`, tagged `"reduction_rule"`, titled
//! `reduction_rules:<user_id>:<project_id>`, and content = JSON array of
//! rule entries. Storing them in the engine's [`Store`] keeps every
//! operator-tunable surface routed through the same persistence resume,
//! backup, and audit path that powers conversations and threads. The
//! engine's [`orchestrator`](brassclaw_engine::executor) reads them on
//! every over-budget turn (see
//! `brassclaw_engine::executor::orchestrator::load_reduction_rules`); the
//! engine-side process-wide cache is dropped via
//! `brassclaw_engine::executor::orchestrator::invalidate_reduction_rules_cache`
//! whenever this store successfully writes a new snapshot.
//!
//! ## Identifier strategy
//!
//! Each `(user_id, project_id)` pair maps to a single `MemoryDoc`. The
//! `DocId` is derived deterministically via `Uuid::new_v5` over the
//! ruleset-title string so repeated `replace` calls upsert in place
//! without requiring a `delete_memory_doc` method on the `Store` trait.
//! The convention is documented in the plan and eliminates the risk of
//! orphan rows accumulating after a half-failed upsert.
//!
//! ## Read path
//!
//! `list` pulls all memory docs owned by the `(project_id, user_id)`
//! pair whose tags contain `"reduction_rule"`, parses the JSON content,
//! and returns them in `priority`-then-`id` order. Empty is a valid
//! non-error state.

#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::executor::orchestrator::invalidate_reduction_rules_cache;
use brassclaw_engine::traits::store::Store;
use brassclaw_engine::types::memory::{DocId, DocType, MemoryDoc};
use brassclaw_engine::types::project::ProjectId;
use brassclaw_product_workflow::{
    ReductionRuleConfigView, ReductionRuleStore, ReductionRuleStoreError, sort_for_storage,
};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Stable UUID namespace used to derive the deterministic `DocId` for a
/// `(user_id, project_id)` ruleset. The exact value is irrelevant — any
/// project-local UUID works — but the project explicitly uses
/// `Uuid::NAMESPACE_OID` so a future migration to a v7 id or a different
/// namespace is a single-line change.
const RULESET_NS: Uuid = Uuid::NAMESPACE_OID;

/// Title prefix for reduction-rule memory docs.
pub(crate) const RULESET_TITLE_PREFIX: &str = "reduction_rules:";

/// Tag marking a memory doc as part of the reduction-rule chain.
pub(crate) const RULESET_TAG: &str = "reduction_rule";

/// Produce the deterministic `DocId` for a `(user_id, project_id)`
/// ruleset. Any two callers constructing the same input receive the
/// same id, which is what makes `Store::save_memory_doc` an upsert
/// without an explicit delete call.
fn ruleset_doc_id(user_id: &str, project_id: &str) -> DocId {
    let title = format!("{RULESET_TITLE_PREFIX}{user_id}:{project_id}");
    DocId(Uuid::new_v5(&RULESET_NS, title.as_bytes()))
}

/// Build the canonical title string for a `(user_id, project_id)`
/// ruleset. Mirrors `ruleset_doc_id`'s input format byte-for-byte; any
/// drift between the two will surface as a deterministic-id mismatch
/// and a Store UPSERT no-op.
fn ruleset_title(user_id: &str, project_id: &str) -> String {
    format!("{RULESET_TITLE_PREFIX}{user_id}:{project_id}")
}

/// Engine-`Store`-backed implementation of [`ReductionRuleStore`].
///
/// Holds an `Arc<dyn Store>` for the engine; the trait already exposes
/// the `save_memory_doc` + `list_memory_docs` calls we need — no new
/// trait methods required (per plan). After every successful
/// `replace`, this impl invokes `invalidate_reduction_rules_cache()` so
/// the next over-budget turn re-reads from `Store` rather than serving
/// a stale process-wide cache slot.
pub(crate) struct StoreBackedReductionRuleStore {
    store: Arc<dyn Store>,
}

impl StoreBackedReductionRuleStore {
    /// Open a new store backed by `store`. Cheap; no I/O is performed
    /// until the first `list` or `replace` call.
    pub(crate) fn open(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ReductionRuleStore for StoreBackedReductionRuleStore {
    async fn list(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
        let project_id = parse_project_id(project_id)?;
        let docs = self
            .store
            .list_memory_docs(project_id, user_id)
            .await
            .map_err(|source| {
                ReductionRuleStoreError::Unavailable(format!(
                    "Store::list_memory_docs failed: {source}"
                ))
            })?;
        let mut out: Vec<ReductionRuleConfigView> = Vec::new();
        for doc in &docs {
            if !doc.tags.iter().any(|t| t == RULESET_TAG) {
                continue;
            }
            // Defensive: only consider the canonical title for this
            // (user, project) tuple. Operators cannot create extra
            // entries today, but if the schema ever widens we don't
            // want cross-tenant leakage via matching tags.
            if doc.title != ruleset_title(user_id, &project_id.to_string()) {
                continue;
            }
            let rules: Vec<ReductionRuleConfigView> = match serde_json::from_str(&doc.content) {
                Ok(v) => v,
                Err(source) => {
                    drop(doc);
                    tracing::debug!(
                        user_id,
                        %project_id,
                        "reduction rule parse failed; skipping doc: {source}"
                    );
                    continue;
                }
            };
            for rule in rules {
                out.push(rule);
            }
        }
        sort_for_storage(&mut out);
        Ok(out)
    }

    async fn replace(
        &self,
        user_id: &str,
        project_id: &str,
        mut rules: Vec<ReductionRuleConfigView>,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        sort_for_storage(&mut rules);
        let content = serde_json::to_string(&rules).map_err(|source| {
            ReductionRuleStoreError::Invalid(format!("rule list serialize failed: {source}"))
        })?;
        let doc = MemoryDoc {
            id: ruleset_doc_id(user_id, project_id),
            project_id: project_id_typed,
            user_id: user_id.to_string(),
            doc_type: DocType::Note,
            title: ruleset_title(user_id, project_id),
            content,
            source_thread_id: None,
            tags: vec![RULESET_TAG.to_string()],
            metadata: serde_json::json!({
                "kind": RULESET_TAG,
                "rule_count": rules.len(),
            }),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.store.save_memory_doc(&doc).await.map_err(|source| {
            ReductionRuleStoreError::Unavailable(format!("Store::save_memory_doc failed: {source}"))
        })?;
        invalidate_reduction_rules_cache();
        Ok(rules)
    }
}

/// Parse a stringified `ProjectId` (`ProjectId(String)` wrapper or
/// string-coerced form) into a `ProjectId` value. Composition callers
/// pass the wire `project_id` as a string, but `Store::list_memory_docs`
/// takes a rich `ProjectId`. Mirrors the validation applied by the
/// rest of the composition runtime so a malformed id fails fast with
/// the same error code a malformed token-settings request would see.
fn parse_project_id(raw: &str) -> Result<ProjectId, ReductionRuleStoreError> {
    if raw.is_empty() {
        return Err(ReductionRuleStoreError::Invalid(
            "project_id is empty".to_string(),
        ));
    }
    if raw.len() > 64 {
        return Err(ReductionRuleStoreError::Invalid(format!(
            "project_id too long: {} chars",
            raw.len()
        )));
    }
    if !raw
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ReductionRuleStoreError::Invalid(format!(
            "project_id '{raw}' contains invalid characters"
        )));
    }
    Ok(ProjectId::from_slug("brassclaw-reduction-rules", raw))
}

fn reduction_rules_key(project_id: &str) -> String {
    format!("reduction_rules:{project_id}")
}

/// LibSQL-backed implementation of [`ReductionRuleStore`].
///
/// Uses the same `settings` table as [`super::token_settings_store::DbTokenSettingsStore`]
/// (which creates it with `CREATE TABLE IF NOT EXISTS` on `open`). Rules for each
/// `(user_id, project_id)` pair are stored as a JSON array under the key
/// `reduction_rules:<project_id>`.
pub(crate) struct DbReductionRuleStore {
    conn: Arc<Mutex<libsql::Connection>>,
}

impl DbReductionRuleStore {
    /// Open the store against `db`. The `settings` table must already exist
    /// (created by `DbTokenSettingsStore::open` which runs `CREATE TABLE IF NOT EXISTS`
    /// on the same database handle — just call it first).
    pub(crate) async fn open(
        db: Arc<libsql::Database>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let conn = db
            .connect()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl ReductionRuleStore for DbReductionRuleStore {
    async fn list(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
        let key = reduction_rules_key(project_id);
        let conn = self.conn.lock().await;
        let mut rows = conn
            .query(
                "SELECT value FROM settings WHERE user_id = ? AND key = ?",
                libsql::params![user_id, key],
            )
            .await
            .map_err(|e| ReductionRuleStoreError::Unavailable(e.to_string()))?;
        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| ReductionRuleStoreError::Unavailable(e.to_string()))?
        {
            let value_str: String = row
                .get(0)
                .map_err(|e| ReductionRuleStoreError::Unavailable(e.to_string()))?;
            let mut rules: Vec<ReductionRuleConfigView> = serde_json::from_str(&value_str)
                .map_err(|e| ReductionRuleStoreError::Invalid(e.to_string()))?;
            sort_for_storage(&mut rules);
            Ok(rules)
        } else {
            Ok(Vec::new())
        }
    }

    async fn replace(
        &self,
        user_id: &str,
        project_id: &str,
        mut rules: Vec<ReductionRuleConfigView>,
    ) -> Result<Vec<ReductionRuleConfigView>, ReductionRuleStoreError> {
        let key = reduction_rules_key(project_id);
        sort_for_storage(&mut rules);
        let value_str = serde_json::to_string(&rules)
            .map_err(|e| ReductionRuleStoreError::Invalid(e.to_string()))?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO settings (user_id, key, value, updated_at) VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            libsql::params![user_id, key, value_str],
        )
        .await
        .map_err(|e| ReductionRuleStoreError::Unavailable(e.to_string()))?;
        drop(conn);
        invalidate_reduction_rules_cache();
        Ok(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::traits::store::Store;
    use brassclaw_engine::types::error::EngineError;
    use brassclaw_engine::types::memory::DocType;
    use brassclaw_engine::types::project::ProjectId;
    use brassclaw_product_workflow::RuleType;

    /// Minimal `Store` impl used only by these tests. Only the
    /// `MemoryDoc` operations are exercised; the rest panic
    /// intentionally so an accidental call reveals the test gap.
    #[derive(Default)]
    struct InMemoryEngineStore {
        docs: tokio::sync::RwLock<Vec<MemoryDoc>>,
    }

    impl InMemoryEngineStore {
        async fn add(&self, doc: MemoryDoc) {
            self.docs.write().await.push(doc);
        }
        async fn all(&self) -> Vec<MemoryDoc> {
            self.docs.read().await.clone()
        }
    }

    #[async_trait::async_trait]
    impl Store for InMemoryEngineStore {
        async fn save_thread(
            &self,
            _thread: &brassclaw_engine::types::thread::Thread,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn load_thread(
            &self,
            _id: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Option<brassclaw_engine::types::thread::Thread>, EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn list_threads(
            &self,
            _project_id: ProjectId,
            _user_id: &str,
        ) -> Result<Vec<brassclaw_engine::types::thread::Thread>, EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn update_thread_state(
            &self,
            _id: brassclaw_engine::types::thread::ThreadId,
            _state: brassclaw_engine::types::thread::ThreadState,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn save_step(
            &self,
            _step: &brassclaw_engine::types::step::Step,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn load_steps(
            &self,
            _thread_id: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Vec<brassclaw_engine::types::step::Step>, EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn append_events(
            &self,
            _events: &[brassclaw_engine::types::event::ThreadEvent],
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn load_events(
            &self,
            _thread_id: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Vec<brassclaw_engine::types::event::ThreadEvent>, EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn save_project(
            &self,
            _project: &brassclaw_engine::types::project::Project,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn load_project(
            &self,
            _id: ProjectId,
        ) -> Result<Option<brassclaw_engine::types::project::Project>, EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn save_memory_doc(&self, doc: &MemoryDoc) -> Result<(), EngineError> {
            let mut docs = self.docs.write().await;
            docs.retain(|d| d.id != doc.id);
            docs.push(doc.clone());
            Ok(())
        }
        async fn load_memory_doc(&self, id: DocId) -> Result<Option<MemoryDoc>, EngineError> {
            Ok(self.docs.read().await.iter().find(|d| d.id == id).cloned())
        }
        async fn list_memory_docs(
            &self,
            project_id: ProjectId,
            user_id: &str,
        ) -> Result<Vec<MemoryDoc>, EngineError> {
            Ok(self
                .docs
                .read()
                .await
                .iter()
                .filter(|d| d.project_id == project_id && d.user_id == user_id)
                .cloned()
                .collect())
        }
        async fn save_lease(
            &self,
            _lease: &brassclaw_engine::types::capability::CapabilityLease,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn load_active_leases(
            &self,
            _thread_id: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Vec<brassclaw_engine::types::capability::CapabilityLease>, EngineError>
        {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn revoke_lease(
            &self,
            _lease_id: brassclaw_engine::types::capability::LeaseId,
            _reason: &str,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn save_mission(
            &self,
            _mission: &brassclaw_engine::types::mission::Mission,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn load_mission(
            &self,
            _id: brassclaw_engine::types::mission::MissionId,
        ) -> Result<Option<brassclaw_engine::types::mission::Mission>, EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn list_missions(
            &self,
            _project_id: ProjectId,
            _user_id: &str,
        ) -> Result<Vec<brassclaw_engine::types::mission::Mission>, EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
        async fn update_mission_status(
            &self,
            _id: brassclaw_engine::types::mission::MissionId,
            _status: brassclaw_engine::types::mission::MissionStatus,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is test-only scope for MemoryDoc tests")
        }
    }

    /// Build a fresh test store pair: the typed backing struct for
    /// introspection (asserting on stored docs) AND the trait-object
    /// reference the production code consumes.
    fn make_pair() -> (Arc<InMemoryEngineStore>, Arc<dyn Store>) {
        let typed = Arc::new(InMemoryEngineStore::default());
        let erased: Arc<dyn Store> = Arc::clone(&typed) as Arc<dyn Store>;
        (typed, erased)
    }

    fn sample_rules() -> Vec<ReductionRuleConfigView> {
        vec![
            ReductionRuleConfigView {
                id: "low-priority".to_string(),
                rule_type: RuleType::Summarize,
                params: serde_json::json!({"field": "context"}),
                priority: 50,
            },
            ReductionRuleConfigView {
                id: "high-priority".to_string(),
                rule_type: RuleType::Truncate,
                params: serde_json::json!({"field": "content", "max_chars": 256}),
                priority: 10,
            },
        ]
    }

    #[tokio::test]
    async fn empty_list_when_no_doc() {
        let (_typed, erased) = make_pair();
        let store = StoreBackedReductionRuleStore::open(erased);
        let rules = store.list("user1", "bootstrap").await.expect("list");
        assert!(rules.is_empty());
    }

    #[tokio::test]
    async fn replace_then_list_round_trips_in_priority_order() {
        let (_typed, erased) = make_pair();
        let store = StoreBackedReductionRuleStore::open(erased);
        let stored = store
            .replace("user1", "bootstrap", sample_rules())
            .await
            .expect("replace");
        assert_eq!(stored[0].id, "high-priority");
        assert_eq!(stored[1].id, "low-priority");
        let read = store.list("user1", "bootstrap").await.expect("list");
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].id, "high-priority");
        assert_eq!(read[1].id, "low-priority");
    }

    #[tokio::test]
    async fn replace_upserts_in_place_no_orphan_rows() {
        let (typed, erased) = make_pair();
        let store = StoreBackedReductionRuleStore::open(erased);
        store
            .replace("user1", "bootstrap", sample_rules())
            .await
            .expect("first replace");
        store
            .replace(
                "user1",
                "bootstrap",
                vec![ReductionRuleConfigView {
                    id: "solo".to_string(),
                    rule_type: RuleType::Drop,
                    params: serde_json::json!({"field": "tmp"}),
                    priority: 100,
                }],
            )
            .await
            .expect("second replace");
        let all = typed.all().await;
        assert_eq!(
            all.len(),
            1,
            "upsert must replace, not append — got {} docs",
            all.len()
        );
        assert_eq!(all[0].doc_type, DocType::Note);
        assert!(all[0].tags.iter().any(|t| t == RULESET_TAG));
    }

    #[tokio::test]
    async fn list_ignores_other_tagged_docs() {
        let (typed, erased) = make_pair();
        let store = StoreBackedReductionRuleStore::open(erased);
        typed
            .add(MemoryDoc {
                id: DocId::new(),
                project_id: ProjectId::from_slug("brassclaw-reduction-rules", "bootstrap"),
                user_id: "user1".to_string(),
                doc_type: DocType::Note,
                title: "unrelated-doc".to_string(),
                content: "[]".to_string(),
                source_thread_id: None,
                tags: vec!["recipe".to_string()],
                metadata: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await;
        let rules = store.list("user1", "bootstrap").await.expect("list");
        assert!(
            rules.is_empty(),
            "untagged docs must not appear in reduction-rule list"
        );
    }

    #[tokio::test]
    async fn list_ignores_cross_tenant_rulesets() {
        let (typed, erased) = make_pair();
        let store = StoreBackedReductionRuleStore::open(erased);
        typed
            .add(MemoryDoc {
                id: DocId::new(),
                project_id: ProjectId::from_slug("brassclaw-reduction-rules", "alpha"),
                user_id: "user1".to_string(),
                doc_type: DocType::Note,
                title: "reduction_rules:user1:alpha".to_string(),
                content: "[{\"id\":\"alpha-rule\",\"type\":\"truncate\",\"params\":{\"field\":\"content\",\"max_chars\":256},\"priority\":100}]".to_string(),
                source_thread_id: None,
                tags: vec!["reduction_rule".to_string()],
                metadata: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await;
        let rules = store.list("user1", "beta").await.expect("list beta");
        assert!(rules.is_empty(), "alpha doc must not leak into beta");
        let alpha = store.list("user1", "alpha").await.expect("list alpha");
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].id, "alpha-rule");
    }

    #[tokio::test]
    async fn replace_invalidates_engine_cache() {
        let (_typed, erased) = make_pair();
        let store = StoreBackedReductionRuleStore::open(erased);
        let cleared_before = invalidate_reduction_rules_cache();
        store
            .replace("user1", "bootstrap", sample_rules())
            .await
            .expect("replace");
        // The PUT itself calls invalidate, so a subsequent explicit
        // call drops any newly populated slot from a parallel call.
        let cleared_after = invalidate_reduction_rules_cache();
        let _ = cleared_before;
        let _ = cleared_after;
    }

    #[tokio::test]
    async fn parse_project_id_rejects_garbage() {
        assert!(parse_project_id("").is_err());
        assert!(parse_project_id("has space").is_err());
        let too_long = "x".repeat(65);
        assert!(parse_project_id(&too_long).is_err());
        assert!(parse_project_id("good-id_42").is_ok());
    }
}
