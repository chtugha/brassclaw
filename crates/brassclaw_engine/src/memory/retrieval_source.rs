//! `RetrievalSource` — abstraction over DB-backed and DB-less prior-knowledge retrieval.
//!
//! Phase 5 (Step 6.1) replaces the `RetrievalEngine::retrieve_context` stub inside
//! `__assemble_prior_knowledge__` with this two-backend system:
//!
//! - [`PostgresSource`] — reads all validated component tables via a single UNION ALL
//!   query (PERF-05 "single-query fetch"). Available when the `skills-db` feature is
//!   active and a `PgPool` is wired in.
//! - [`RamSource`] — falls back to the existing MemoryDoc keyword-retrieval path
//!   (`retrieve_context`). Used in DB-less mode.
//!
//! Both enforce:
//!   `validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)`
//!
//! # Token budget
//!
//! Components are accumulated in `(class_code ASC, prompt_uid ASC)` order until
//! `token_budget` is exhausted (estimated at `TOKENS_PER_BYTE` tokens per byte).
//! The entire budget is honoured — partial rows are not split.

use std::sync::Arc;

use async_trait::async_trait;

use crate::types::error::EngineError;
use crate::types::project::ProjectId;
use crate::traits::store::Store;

/// Approximate tokens-per-byte for English prose (~4 bytes/token).
const TOKENS_PER_BYTE: f64 = 0.25;

/// A single retrieved component row normalised across all class tables.
#[derive(Debug, Clone)]
pub struct ComponentItem {
    /// Unique component ID (UUID).
    pub id: uuid::Uuid,
    /// Class code (0–21, 50).  See `intent_system::class_label`.
    pub class_code: i32,
    /// Monotonic prompt-assembly ordinal.
    pub prompt_uid: i64,
    /// Human-readable name.
    pub name: String,
    /// Short description (may be empty).
    pub description: String,
    /// Effective prior-knowledge text.
    ///
    /// If the DB row has a non-NULL `prior_knowledge_content`, that is used
    /// (Solution Override path §3.13). Otherwise this is the row's `content`
    /// / `body` column value.
    pub effective_content: String,
    /// When true, this component's content replaces the normal assembly and
    /// should be returned verbatim to `default.py` (§3.13 Solution Override).
    pub override_prompt_creation: bool,
}

/// Scope for component retrieval — must match the 4-part scope tuple on all
/// component tables.
#[derive(Debug, Clone)]
pub struct ComponentScope {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
}

/// Error type for retrieval-source operations.
#[derive(Debug, thiserror::Error)]
pub enum RetrievalSourceError {
    #[error("retrieval DB error: {0}")]
    Db(String),
    #[error("retrieval engine error: {0}")]
    Engine(String),
}

impl From<EngineError> for RetrievalSourceError {
    fn from(e: EngineError) -> Self {
        Self::Engine(e.to_string())
    }
}

/// Trait for prior-knowledge component retrieval.
///
/// Both backends return components in `(class_code ASC, prompt_uid ASC)` order,
/// capped at `token_budget` tokens.
#[async_trait]
pub trait RetrievalSource: Send + Sync {
    /// Fetch validated components for the given scope, query, and consumer tag.
    ///
    /// `consumer_tag` is the numeric class-code prefix of the calling component
    /// (e.g. `"02"` for the orchestrator). The DB backend requires it in the row's
    /// `consumer_tags[]`; the RAM backend ignores it and returns all matching docs.
    ///
    /// Returns at most enough components to fill `token_budget` estimated tokens,
    /// ordered by `(class_code ASC, prompt_uid ASC)`.
    async fn fetch_for_consumer(
        &self,
        scope: &ComponentScope,
        query: &str,
        token_budget: usize,
        consumer_tag: &str,
    ) -> Result<Vec<ComponentItem>, RetrievalSourceError>;
}

// ---------------------------------------------------------------------------
// RamSource — DB-less keyword-retrieval path
// ---------------------------------------------------------------------------

/// DB-less retrieval source.
///
/// Wraps the legacy `Store`-based `RetrievalEngine` and maps `MemoryDoc` rows
/// to `ComponentItem` using the `doc_type_class_code` table from `orchestrator.rs`.
/// Used when no Postgres pool is available.
pub struct RamSource {
    engine: super::RetrievalEngine,
}

impl RamSource {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self {
            engine: super::RetrievalEngine::new(store),
        }
    }
}

#[async_trait]
impl RetrievalSource for RamSource {
    async fn fetch_for_consumer(
        &self,
        scope: &ComponentScope,
        query: &str,
        token_budget: usize,
        _consumer_tag: &str,
    ) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
        // Parse project_id UUID for the Store query.
        let project_id = scope
            .project_id
            .parse::<uuid::Uuid>()
            .map(ProjectId)
            .unwrap_or_else(|_| ProjectId::new());

        // Use a generous upper bound — we'll truncate by token budget ourselves.
        const RAM_MAX_DOCS: usize = 200;
        let docs = self
            .engine
            .retrieve_context(project_id, &scope.user_id, query, RAM_MAX_DOCS)
            .await
            .map_err(RetrievalSourceError::from)?;

        let mut items = Vec::new();
        let mut tokens_used = 0usize;

        for doc in docs {
            let (class_code, _label) = doc_type_to_class_code(doc.doc_type);
            let cost = estimate_tokens(doc.content.len());
            if tokens_used + cost > token_budget && !items.is_empty() {
                break;
            }
            tokens_used += cost;
            items.push(ComponentItem {
                id: doc.id.0,
                class_code,
                // MemoryDoc has no prompt_uid — use a monotonically increasing
                // counter so ordering is stable within a retrieval batch.
                prompt_uid: items.len() as i64,
                name: doc.title.clone(),
                description: String::new(),
                effective_content: doc.content.clone(),
                override_prompt_creation: false,
            });
        }

        // Sort by (class_code, prompt_uid) for deterministic assembly order.
        items.sort_by_key(|item| (item.class_code, item.prompt_uid));
        Ok(items)
    }
}

// ---------------------------------------------------------------------------
// PostgresSource — DB-backed UNION ALL retrieval (skills-db feature)
// ---------------------------------------------------------------------------

/// DB-backed retrieval source.
///
/// Issues a single UNION ALL query across all validated component tables
/// (PERF-05) and returns results ordered by `(class_code ASC, prompt_uid ASC)`.
/// Enforces `validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)`.
#[cfg(feature = "skills-db")]
pub struct PostgresSource {
    pool: Arc<brassclaw_pg::PgPool>,
}

#[cfg(feature = "skills-db")]
impl PostgresSource {
    pub fn new(pool: Arc<brassclaw_pg::PgPool>) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "skills-db")]
#[async_trait]
impl RetrievalSource for PostgresSource {
    async fn fetch_for_consumer(
        &self,
        scope: &ComponentScope,
        _query: &str,
        token_budget: usize,
        consumer_tag: &str,
    ) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
        use tokio_postgres::types::ToSql;

        let client = self
            .pool
            .get()
            .await
            .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

        // PERF-05: single UNION ALL across all component tables.
        // Each sub-select projects to (id, class_code, prompt_uid, name,
        // description, effective_content, override_prompt_creation).
        //
        // Tables with a plain `content` column use COALESCE(prior_knowledge_content, content).
        // reborn_skills uses `body` as the content column.
        // reborn_extensions_unified uses `description` as the fallback (no plain content).
        // reborn_actions uses `description` as the fallback (no plain content — steps is JSONB).
        //
        // reborn_tools (class 00) is excluded — no prompt text.
        // Scope params: $1=tenant_id, $2=user_id, $3=agent_id, $4=project_id,
        // $5=consumer_tag (must be in consumer_tags[]).
        let query_sql = "
            SELECT id, class_code, prompt_uid, name, description, effective_content,
                   override_prompt_creation
            FROM (
                -- reborn_skills (classes 1-3)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), body) AS effective_content,
                       override_prompt_creation
                FROM reborn_skills
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_extensions_unified (classes 4-9)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, description,
                       COALESCE(prior_knowledge_content, description) AS effective_content,
                       override_prompt_creation
                FROM reborn_extensions_unified
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_actions (class 16)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, description,
                       COALESCE(prior_knowledge_content, description) AS effective_content,
                       override_prompt_creation
                FROM reborn_actions
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_specs (class 12)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_specs
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_tool_skills (class 13)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_tool_skills
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_plans (class 14)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_plans
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_summaries (class 15)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_summaries
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_docus (class 17)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_docus
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_lessons (class 18)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_lessons
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_issues (class 19)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_issues
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_notes (class 20)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_notes
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)

                UNION ALL

                -- reborn_recipes (class 21)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), '') AS effective_content,
                       override_prompt_creation
                FROM reborn_recipes
                WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
            ) AS components
            ORDER BY class_code ASC, prompt_uid ASC
        ";

        let params: &[&(dyn ToSql + Sync)] = &[
            &scope.tenant_id,
            &scope.user_id,
            &scope.agent_id,
            &scope.project_id,
            &consumer_tag,
        ];

        let rows = client
            .query(query_sql, params)
            .await
            .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

        let mut items = Vec::new();
        let mut tokens_used = 0usize;

        for row in rows {
            let id_str: &str = row.get(0);
            let class_code: i32 = row.get(1);
            let prompt_uid: i64 = row.get(2);
            let name: &str = row.get(3);
            let description: &str = row.get(4);
            let effective_content: &str = row.get(5);
            let override_prompt_creation: bool = row.get(6);

            let cost = estimate_tokens(effective_content.len());
            if tokens_used + cost > token_budget && !items.is_empty() {
                break;
            }
            tokens_used += cost;

            let id = id_str
                .parse::<uuid::Uuid>()
                .unwrap_or_else(|_| uuid::Uuid::nil());

            items.push(ComponentItem {
                id,
                class_code,
                prompt_uid,
                name: name.to_string(),
                description: description.to_string(),
                effective_content: effective_content.to_string(),
                override_prompt_creation,
            });
        }

        Ok(items)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Estimate token cost from byte length.
fn estimate_tokens(byte_len: usize) -> usize {
    ((byte_len as f64 * TOKENS_PER_BYTE) as usize).max(1)
}

/// Map a `DocType` to its class code (mirrors `doc_type_class_code` in orchestrator.rs).
///
/// This is the authoritative table for DocType → class_code mapping in the
/// DB-less retrieval path.
fn doc_type_to_class_code(doc_type: crate::types::memory::DocType) -> (i32, &'static str) {
    use crate::types::memory::DocType;
    match doc_type {
        DocType::Skill     => (3,  "skill_llm"),
        DocType::Spec      => (12, "spec"),
        DocType::ToolSkill => (13, "tool_skill"),
        DocType::Plan      => (14, "plan"),
        DocType::Summary   => (15, "summary"),
        DocType::Lesson    => (18, "lesson"),
        DocType::Issue     => (19, "issue"),
        DocType::Note      => (20, "note"),
        DocType::Recipe    => (21, "recipe"),
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::memory::{DocType, MemoryDoc};

    fn make_store(docs: Vec<MemoryDoc>) -> Arc<crate::tests::InMemoryStore> {
        Arc::new(crate::tests::InMemoryStore::with_docs(docs))
    }

    fn test_scope(project_id: &str) -> ComponentScope {
        ComponentScope {
            tenant_id: "default".to_string(),
            user_id: "test-user".to_string(),
            agent_id: "test-agent".to_string(),
            project_id: project_id.to_string(),
        }
    }

    #[tokio::test]
    async fn ram_source_returns_empty_for_empty_store() {
        let project = ProjectId::new();
        let store = make_store(vec![]);
        let source = RamSource::new(store);
        let scope = test_scope(&project.to_string());

        let result = source
            .fetch_for_consumer(&scope, "anything", 100_000, "02")
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn ram_source_returns_docs_ordered_by_class_then_uid() {
        let project = ProjectId::new();
        let store = make_store(vec![
            MemoryDoc::new(project, "test-user", DocType::Note, "Note A", "note content"),
            MemoryDoc::new(project, "test-user", DocType::Spec, "Spec A", "spec content"),
            MemoryDoc::new(project, "test-user", DocType::Lesson, "Lesson A", "lesson content"),
        ]);
        let source = RamSource::new(store);
        let scope = test_scope(&project.to_string());

        let result = source
            .fetch_for_consumer(&scope, "content", 100_000, "02")
            .await
            .unwrap();

        // Should be sorted by class_code: Spec(12) < Lesson(18) < Note(20)
        assert!(!result.is_empty());
        let codes: Vec<i32> = result.iter().map(|item| item.class_code).collect();
        let mut sorted = codes.clone();
        sorted.sort();
        assert_eq!(codes, sorted, "results should be sorted by class_code");
    }

    #[tokio::test]
    async fn ram_source_respects_token_budget() {
        let project = ProjectId::new();
        // Create docs with large content that will exceed the budget
        let large_content = "x".repeat(50_000); // ~12,500 tokens
        let store = make_store(vec![
            MemoryDoc::new(project, "test-user", DocType::Lesson, "Lesson 1", &large_content),
            MemoryDoc::new(project, "test-user", DocType::Lesson, "Lesson 2", &large_content),
            MemoryDoc::new(project, "test-user", DocType::Lesson, "Lesson 3", &large_content),
        ]);
        let source = RamSource::new(store);
        let scope = test_scope(&project.to_string());

        // Budget of 15,000 tokens — only one doc should fit
        let result = source
            .fetch_for_consumer(&scope, "lesson", 15_000, "02")
            .await
            .unwrap();

        // First doc is always included regardless of budget (empty-items check)
        assert!(result.len() <= 2, "token budget should limit results");
    }

    #[test]
    fn estimate_tokens_never_zero() {
        assert!(estimate_tokens(0) >= 1);
        assert!(estimate_tokens(4) >= 1);
        assert_eq!(estimate_tokens(4000), 1000);
        assert_eq!(estimate_tokens(8000), 2000);
    }

    #[test]
    fn doc_type_class_codes_match_intent_system() {
        // Verify our mapping matches the authoritative table in intent_system::class_label.
        use crate::types::memory::DocType;
        assert_eq!(doc_type_to_class_code(DocType::Spec).0, 12);
        assert_eq!(doc_type_to_class_code(DocType::ToolSkill).0, 13);
        assert_eq!(doc_type_to_class_code(DocType::Plan).0, 14);
        assert_eq!(doc_type_to_class_code(DocType::Summary).0, 15);
        assert_eq!(doc_type_to_class_code(DocType::Lesson).0, 18);
        assert_eq!(doc_type_to_class_code(DocType::Issue).0, 19);
        assert_eq!(doc_type_to_class_code(DocType::Note).0, 20);
        assert_eq!(doc_type_to_class_code(DocType::Recipe).0, 21);
    }
}
