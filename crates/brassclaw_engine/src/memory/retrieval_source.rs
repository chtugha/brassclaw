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

/// Result of an intent-driven `fetch_for_turn` call.
///
/// Either a list of assembled components (the common case) or a disambiguation
/// request (when multiple near-equal candidates exist in `reborn_intent_inputs`).
#[derive(Debug)]
pub enum FetchForTurnResult {
    /// One or more components retrieved and ready to assemble.
    Components(Vec<ComponentItem>),
    /// Multiple near-equal intent candidates — the orchestrator should surface
    /// a disambiguation message to the user (spec §3.12 Q11).
    Disambiguation(Vec<crate::memory::intent_system::IntentCandidate>),
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

    /// Intent-driven retrieval for a live turn (Step 6.7).
    ///
    /// 1. Attempt intent resolution via `reborn_intent_inputs`.
    /// 2. On a unique match: fetch the specific component by ID + increment score.
    /// 3. On disambiguation: return the candidates for UX surfacing.
    /// 4. On no-match / DB-less: fall back to `fetch_for_consumer` (keyword path).
    ///
    /// The default implementation delegates to `fetch_for_consumer` (used by
    /// `RamSource` which has no intent store). DB-backed sources override this.
    async fn fetch_for_turn(
        &self,
        scope: &ComponentScope,
        query: &str,
        token_budget: usize,
        sender_class_code: &str,
    ) -> Result<FetchForTurnResult, RetrievalSourceError> {
        let items = self
            .fetch_for_consumer(scope, query, token_budget, sender_class_code)
            .await?;
        Ok(FetchForTurnResult::Components(items))
    }
}

// ---------------------------------------------------------------------------
// RamSource — DB-less keyword-retrieval path
// ---------------------------------------------------------------------------

/// Environment variable for the path to the fallback-content file.
///
/// When set, `RamSource` loads the file at construction time and uses it
/// for keyword retrieval when the live store returns no results.
/// Format: JSONL — one `FallbackEntry` JSON object per line.
pub const FALLBACK_CONTENT_FILE_ENV: &str = "BRASSCLAW_FALLBACK_CONTENT_FILE";

/// A single entry from the fallback-content file (JSONL format).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FallbackEntry {
    pub class_code: i32,
    pub prompt_uid: i64,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub override_prompt_creation: bool,
}

/// DB-less retrieval source.
///
/// Wraps the legacy `Store`-based `RetrievalEngine` and maps `MemoryDoc` rows
/// to `ComponentItem` using the `doc_type_class_code` table from `orchestrator.rs`.
/// Also supports a static fallback-content file (`BRASSCLAW_FALLBACK_CONTENT_FILE`)
/// for fully offline / DB-less deployments.
///
/// Priority:
/// 1. Live `Store` keyword results (when the store has docs for the scope).
/// 2. Fallback-content file keyword search (when store returns nothing).
pub struct RamSource {
    engine: super::RetrievalEngine,
    /// Pre-loaded fallback entries from the static fallback-content file.
    /// Empty when no file is configured or the file could not be read.
    fallback_entries: Vec<FallbackEntry>,
}

impl RamSource {
    /// Create a `RamSource` backed by `store`.
    ///
    /// Automatically tries to load the fallback-content file from the path
    /// specified by `BRASSCLAW_FALLBACK_CONTENT_FILE` (if set).
    pub fn new(store: Arc<dyn Store>) -> Self {
        let fallback_entries = load_fallback_file_from_env();
        Self {
            engine: super::RetrievalEngine::new(store),
            fallback_entries,
        }
    }

    /// Create a `RamSource` with explicit fallback entries (for testing).
    pub fn new_with_fallback(store: Arc<dyn Store>, entries: Vec<FallbackEntry>) -> Self {
        Self {
            engine: super::RetrievalEngine::new(store),
            fallback_entries: entries,
        }
    }
}

/// Load fallback entries from the file path in `BRASSCLAW_FALLBACK_CONTENT_FILE`.
/// Returns an empty vec (silent) if the env var is not set or the file cannot be read.
fn load_fallback_file_from_env() -> Vec<FallbackEntry> {
    let path = match std::env::var(FALLBACK_CONTENT_FILE_ENV) {
        Ok(p) if !p.is_empty() => p,
        _ => return vec![],
    };
    load_fallback_file(&path)
}

/// Parse a JSONL fallback-content file into `FallbackEntry` rows.
///
/// Each non-empty line must be a valid JSON object matching `FallbackEntry`.
/// Malformed lines are silently skipped (logged at debug level).
pub fn load_fallback_file(path: &str) -> Vec<FallbackEntry> {
    use std::io::BufRead;
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::debug!(path, error = %e, "fallback-content file not found; DB-less path will have no static content");
            return vec![];
        }
    };
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();
    for (lineno, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        match serde_json::from_str::<FallbackEntry>(trimmed) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                tracing::debug!(path, lineno, error = %e, "skipping malformed fallback-content entry");
            }
        }
    }
    // Sort by (class_code, prompt_uid) for deterministic assembly.
    entries.sort_by_key(|e| (e.class_code, e.prompt_uid));
    entries
}

/// Keyword-search the fallback entries and return matching `ComponentItem`s
/// within the token budget.
fn search_fallback_entries(
    entries: &[FallbackEntry],
    query: &str,
    token_budget: usize,
) -> Vec<ComponentItem> {
    use super::retrieval_dbless::extract_keywords;

    let keywords = extract_keywords(query);
    let mut scored: Vec<(f64, &FallbackEntry)> = entries
        .iter()
        .filter_map(|entry| {
            let score = if keywords.is_empty() {
                // No keywords: return everything, scored by class priority.
                super::retrieval_dbless::doc_type_weight_by_class(entry.class_code)
            } else {
                let name_lower = entry.name.to_lowercase();
                let content_lower = entry.content.to_lowercase();
                let mut matched = 0usize;
                for kw in &keywords {
                    if name_lower.contains(kw.as_str()) {
                        matched += 2; // title match worth more
                    } else if content_lower.contains(kw.as_str()) {
                        matched += 1;
                    }
                }
                if matched == 0 {
                    return None;
                }
                matched as f64 / (keywords.len() * 2) as f64
                    + super::retrieval_dbless::doc_type_weight_by_class(entry.class_code)
            };
            Some((score, entry))
        })
        .collect();

    scored.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut items = Vec::new();
    let mut tokens_used = 0usize;
    for (_, entry) in scored {
        let cost = estimate_tokens(entry.content.len());
        if tokens_used + cost > token_budget && !items.is_empty() {
            break;
        }
        tokens_used += cost;
        items.push(ComponentItem {
            id: uuid::Uuid::nil(),
            class_code: entry.class_code,
            prompt_uid: entry.prompt_uid,
            name: entry.name.clone(),
            description: entry.description.clone(),
            effective_content: entry.content.clone(),
            override_prompt_creation: entry.override_prompt_creation,
        });
    }

    // Re-sort by (class_code, prompt_uid) for deterministic assembly.
    items.sort_by_key(|item| (item.class_code, item.prompt_uid));
    items
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

        // If the live store returned results, use them.
        if !docs.is_empty() {
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
            return Ok(items);
        }

        // Store returned nothing — fall back to the static fallback-content file.
        if !self.fallback_entries.is_empty() {
            return Ok(search_fallback_entries(
                &self.fallback_entries,
                query,
                token_budget,
            ));
        }

        Ok(vec![])
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

    /// Override: use the intent system (`resolve_intent`) before falling back
    /// to the full UNION ALL scan (Step 6.7).
    ///
    /// `resolve_intent` already atomically increments the matched row's score
    /// (PERF-03, SEC-05) before returning `Match` — no separate increment call
    /// is needed here.
    async fn fetch_for_turn(
        &self,
        scope: &ComponentScope,
        query: &str,
        token_budget: usize,
        sender_class_code: &str,
    ) -> Result<FetchForTurnResult, RetrievalSourceError> {
        use crate::memory::intent_system::{IntentResolution, IntentScope, resolve_intent};

        let intent_scope = IntentScope {
            tenant_id: scope.tenant_id.clone(),
            user_id: scope.user_id.clone(),
            agent_id: scope.agent_id.clone(),
            project_id: scope.project_id.clone(),
        };

        match resolve_intent(&self.pool, &intent_scope, query).await {
            Ok(IntentResolution::Match {
                component_id,
                component_class_code,
            }) => {
                // Score already incremented inside resolve_intent (PERF-03).
                // Fetch the specific component from its class table (SEC-01 gate).
                let items = fetch_component_by_id(
                    &self.pool,
                    scope,
                    component_id,
                    component_class_code,
                )
                .await?;
                Ok(FetchForTurnResult::Components(items))
            }
            Ok(IntentResolution::Disambiguation { candidates }) => {
                // Multiple near-equal matches — the orchestrator must surface
                // a disambiguation message before proceeding.
                Ok(FetchForTurnResult::Disambiguation(candidates))
            }
            Ok(IntentResolution::NoMatch) | Ok(IntentResolution::DbLessFallback) | Err(_) => {
                // No intent match (or DB error): fall back to the full UNION ALL.
                let items = self
                    .fetch_for_consumer(scope, query, token_budget, sender_class_code)
                    .await?;
                Ok(FetchForTurnResult::Components(items))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Estimate token cost from byte length.
fn estimate_tokens(byte_len: usize) -> usize {
    ((byte_len as f64 * TOKENS_PER_BYTE) as usize).max(1)
}

/// Fetch a single component from its class-specific table by ID (Step 6.7).
///
/// Enforces the SEC-01 validation gate:
///   `validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)`
///
/// Returns an empty vec if the component is not found or fails the gate (e.g.
/// it was demoted to pending/rejected between the intent lookup and this fetch).
#[cfg(feature = "skills-db")]
async fn fetch_component_by_id(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    component_id: uuid::Uuid,
    component_class_code: i32,
) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
    use tokio_postgres::types::ToSql;

    // Map class_code → (table_name, content_expr).
    // Tables with a `body` column (reborn_skills) use COALESCE(prior_knowledge_content, body).
    // Tables with a `content` column use COALESCE(NULLIF(prior_knowledge_content,''), content).
    // Tables with only description (extensions, actions) use COALESCE(prior_knowledge_content, description).
    // reborn_tools (class 0) has no prompt text and is excluded.
    let table_and_content: Option<(&'static str, &'static str)> = match component_class_code {
        1..=3 => Some(("reborn_skills", "COALESCE(NULLIF(prior_knowledge_content,''), body)")),
        4..=9 => Some(("reborn_extensions_unified", "COALESCE(prior_knowledge_content, description)")),
        10 | 50 => Some(("reborn_skills", "COALESCE(NULLIF(prior_knowledge_content,''), body)")),
        12 => Some(("reborn_specs", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        13 => Some(("reborn_tool_skills", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        14 => Some(("reborn_plans", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        15 => Some(("reborn_summaries", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        16 => Some(("reborn_actions", "COALESCE(prior_knowledge_content, description)")),
        17 => Some(("reborn_docus", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        18 => Some(("reborn_lessons", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        19 => Some(("reborn_issues", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        20 => Some(("reborn_notes", "COALESCE(NULLIF(prior_knowledge_content,''), content)")),
        21 => Some(("reborn_recipes", "COALESCE(NULLIF(prior_knowledge_content,''), '')")),
        _ => None,
    };

    let Some((table, content_expr)) = table_and_content else {
        // Class 0 (tools) or unknown — no prompt text to retrieve.
        return Ok(vec![]);
    };

    let client = pool
        .get()
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    let query_sql = format!(
        "SELECT id::text, class_code::int, prompt_uid::bigint,
                name, COALESCE(description,'') AS description,
                {content_expr} AS effective_content,
                override_prompt_creation
         FROM {table}
         WHERE id = $1
           AND tenant_id  = $2
           AND user_id    = $3
           AND agent_id   = $4
           AND project_id = $5
           AND validation_status = 'validated'
           AND '05:validator' != ALL(consumer_tags)"
    );

    let params: &[&(dyn ToSql + Sync)] = &[
        &component_id,
        &scope.tenant_id,
        &scope.user_id,
        &scope.agent_id,
        &scope.project_id,
    ];

    let rows = client
        .query(&query_sql, params)
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    let items: Vec<ComponentItem> = rows
        .iter()
        .map(|row| {
            let id_str: &str = row.get(0);
            let id = id_str
                .parse::<uuid::Uuid>()
                .unwrap_or_else(|_| uuid::Uuid::nil());
            ComponentItem {
                id,
                class_code: row.get(1),
                prompt_uid: row.get(2),
                name: row.get::<_, &str>(3).to_string(),
                description: row.get::<_, &str>(4).to_string(),
                effective_content: row.get::<_, &str>(5).to_string(),
                override_prompt_creation: row.get(6),
            }
        })
        .collect();

    Ok(items)
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

    // ── fallback-content file tests ─────────────────────────────────────────

    fn fallback_entries() -> Vec<FallbackEntry> {
        vec![
            FallbackEntry {
                class_code: 18,
                prompt_uid: 1,
                name: "lesson-about-errors".to_string(),
                description: String::new(),
                content: "Always check error codes before retrying".to_string(),
                override_prompt_creation: false,
            },
            FallbackEntry {
                class_code: 12,
                prompt_uid: 2,
                name: "spec-for-api".to_string(),
                description: String::new(),
                content: "The API returns JSON with a data field".to_string(),
                override_prompt_creation: false,
            },
            FallbackEntry {
                class_code: 3,
                prompt_uid: 3,
                name: "skill-review".to_string(),
                description: String::new(),
                content: "Review code for style and correctness".to_string(),
                override_prompt_creation: false,
            },
        ]
    }

    #[tokio::test]
    async fn ram_source_falls_back_to_file_when_store_empty() {
        let project = ProjectId::new();
        let store = make_store(vec![]);
        let source = RamSource::new_with_fallback(store, fallback_entries());
        let scope = test_scope(&project.to_string());

        let result = source
            .fetch_for_consumer(&scope, "error", 100_000, "02")
            .await
            .unwrap();

        // Should find the lesson about errors
        assert!(!result.is_empty(), "fallback file should be searched when store is empty");
        assert!(
            result.iter().any(|item| item.name == "lesson-about-errors"),
            "lesson matching 'error' query should be returned"
        );
    }

    #[tokio::test]
    async fn ram_source_prefers_live_store_over_fallback() {
        let project = ProjectId::new();
        let store = make_store(vec![MemoryDoc::new(
            project,
            "test-user",
            DocType::Spec,
            "Live spec",
            "live content about errors",
        )]);
        let source = RamSource::new_with_fallback(store, fallback_entries());
        let scope = test_scope(&project.to_string());

        let result = source
            .fetch_for_consumer(&scope, "error", 100_000, "02")
            .await
            .unwrap();

        // Live store result should be used, not the fallback
        assert!(
            result.iter().any(|item| item.name == "Live spec"),
            "live store result should be preferred over fallback file"
        );
        // Fallback-only entries should NOT be present
        assert!(
            !result.iter().any(|item| item.name == "lesson-about-errors"),
            "fallback entries should not appear when live store has results"
        );
    }

    #[test]
    fn load_fallback_file_parses_jsonl() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fallback.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"class_code":12,"prompt_uid":1,"name":"spec-a","content":"spec content"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"class_code":18,"prompt_uid":2,"name":"lesson-b","content":"lesson content"}}"#
        )
        .unwrap();
        writeln!(f, "// this is a comment, skip it").unwrap();
        writeln!(f).unwrap(); // blank line
        writeln!(f, "not-valid-json").unwrap(); // malformed — skipped

        let entries = load_fallback_file(path.to_str().unwrap());
        assert_eq!(entries.len(), 2);
        // Sorted by (class_code, prompt_uid): spec(12) < lesson(18)
        assert_eq!(entries[0].class_code, 12);
        assert_eq!(entries[1].class_code, 18);
    }

    #[test]
    fn load_fallback_file_returns_empty_for_missing_path() {
        let entries = load_fallback_file("/nonexistent/path/fallback.jsonl");
        assert!(entries.is_empty());
    }
}
