//! `RetrievalSource` — abstraction over DB-backed and DB-less prior-knowledge retrieval.
//!
//! Phase 5 (Step 6.1) replaces the `RetrievalEngine::retrieve_context` stub inside
//! `__assemble_prior_knowledge__` with this two-backend system:
//!
//! - [`PostgresSource`] — reads all validated component tables via a single UNION ALL
//!   query (PERF-05 "single-query fetch"). Available when the `skills-db` feature is
//!   active and a `PgPool` is wired in.
//! - [`RamSource`] — keyword-retrieval over a `Store` (in production the store is
//!   `PgMemoryDocStore`, i.e. keyword-retrieval **over postgres**, not a postgres-less
//!   path). The static filesystem fallback-content file that previously supported
//!   "fully offline / DB-less deployments" has been removed (Postgres is mandatory).
//!   This legacy keyword path is replaced by intent-driven `PostgresSource` in v3
//!   Phase K.
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
use serde::{Deserialize, Serialize};

use crate::traits::store::Store;
use crate::types::error::EngineError;
use crate::types::project::ProjectId;

/// Approximate tokens-per-byte for English prose (~4 bytes/token).
const TOKENS_PER_BYTE: f64 = 0.25;

/// A single retrieved component row normalised across all class tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Executable step list for class-16 Actions (the `reborn_actions.steps`
    /// JSONB, parsed). `None` for every other class — only
    /// [`fetch_component_by_id`] / [`fetch_component_by_name`] populate this
    /// (for class 16) so `execute_action_procedure` can run the real procedure
    /// (v3 Phase G / Q-G-STUB1 — `subplan_stub_stepG_action_steps`). The
    /// broad-scan assembly path does not fetch steps (it builds
    /// `orchestrator_content`, not an executable doc).
    pub steps: Option<serde_json::Value>,
    /// Allowed-tools list for class-16 Actions (the `reborn_actions.
    /// allowed_tools` TEXT[], as a JSON array of strings). `None` for every
    /// other class. See [`steps`].
    ///
    /// [`steps`]: ComponentItem::steps
    pub allowed_tools: Option<serde_json::Value>,
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

/// Tier-0 / Tier-1 routing signals harvested from the matched Recipe row by
/// `PostgresSource::fetch_for_turn` (v3 Phase E / plan §0.8).
///
/// Carried on [`FetchForTurnResult::SplitResult`] so the Phase-H Tier-0/Tier-1
/// consumer (the agent-loop `RecipeStage` via the composition
/// `PgRetrievalLookup` bridge) can dispatch without re-querying the DB. This is
/// a greenfield v3 type (FIND-08) — it does not extend any existing type.
#[derive(Debug, Clone)]
pub struct TurnRoutingSignals {
    /// From the matched Recipe row's own `override_prompt_column` column
    /// (Q4→A): when true the assembled prior knowledge replaces the normal
    /// prompt-assembly path (§3.13 Solution Override).
    pub override_prompt_creation: bool,
    /// Orchestrator-channel UUIDs (the `orchestrator_items` ids) serialized as
    /// strings — the orchestrator-channel identity set for the turn (Wilson
    /// scoring / `record_recipe_outcome`).
    pub matched_component_ids: Vec<String>,
    /// Human-readable label for the matched variant — the matched
    /// `RecipeVariant.variant_key` (no `label` field exists in the data model;
    /// Q1→A resolved this is the label source).
    pub variant_label: String,
    /// The `step_link` of the matched recipe variant (the IBS entry point).
    pub step_link: String,
    /// `!tier0_eligible` (Q3→A — always complements): when true the turn needs
    /// an LLM call (Tier-1); when false Tier-0 deterministic execution suffices.
    pub llm_call_required: bool,
    /// Wilson lower-bound from the matched Recipe row (for metrics / logging).
    pub wilson_lower: f64,
    /// Pre-computed Tier-0 eligibility. TRUE only when ALL of: tier ∈ {mature,
    /// candidate}, `wilson_lower ≥ 0.70`, `validation_status = 'validated'`,
    /// AND a validation hook is wired (`has_validation`). Computed by
    /// `fetch_for_turn` from the row via `Recipe::is_tier0_eligible()` (the
    /// FULL check in `types/recipe.rs`), NOT the stripped `PgRecipe::
    /// is_tier0_eligible()` which omits the `wilson_lower ≥ 0.70` guard
    /// (FIND-P9-09 / §0.8 discrepancy note). Under §0.23 `validated ⇒ has_
    /// validation`, so the `has_validation` guard is subsumed by the
    /// `validation_status = 'validated'` requirement (Q2→A).
    pub tier0_eligible: bool,
    /// The matched Recipe component UUID (class 21) as a string. `Some`
    /// whenever this `SplitResult` came from `fetch_recipe_split_result`
    /// (both construction sites there receive the `recipe_id` param), so
    /// the Model B/C agent-loop Tier-0 path can stamp it onto
    /// `recipe_tier_zero_*` events and the engine can surface it through
    /// the pkr dict + `OrchestratorResult.tier_zero_outcome` (v3 Phase
    /// H.4). Reused by Model B/C (the `LoopOrchestratorPort` driver + the
    /// engine `pub` fns extracted in H.8), NOT Model A — the `default.py`
    /// step-0 `tier_zero` branch that previously read this was removed in
    /// v3 Phase H.5 O3. `None` on non-recipe paths and in test fixtures
    /// (v3 Phase H4.3).
    pub recipe_id: Option<String>,
    /// The matched Recipe's display name (`reborn_recipes.name`, read at
    /// `fetch_recipe_split_result` line 810). Surfaced end-to-end (v3 Phase
    /// H4.5, Q-A) so the `recipe_tier_zero_*` `EventKind` variants carry a
    /// human-readable recipe name (not just the UUID) — the `variant_label`
    /// above is the matched variant key, NOT the recipe name. Always present
    /// on the recipe path (every `TurnRoutingSignals` is built inside
    /// `fetch_recipe_split_result` or a test fixture that supplies one).
    /// Reused by Model B/C (same emitter path as `recipe_id` above), NOT
    /// Model A (the `default.py` reader was removed in v3 Phase H.5 O3).
    pub recipe_name: String,
}

/// Result of an intent-driven `fetch_for_turn` call.
///
/// The common cases are a list of assembled components or a disambiguation
/// request (when multiple near-equal candidates exist in `reborn_intent_inputs`).
/// Phase E (§0.8) adds two Tier-0 routing variants: `ActionShortCircuit` (an
/// Action intent match executes directly, no LLM) and `SplitResult` (a Recipe
/// intent match with a `step_link` whose IBS-compiled steps are pre-fetched
/// into Rust / orchestrator channels with routing signals).
#[derive(Debug)]
pub enum FetchForTurnResult {
    /// No-match UNION ALL path or non-recipe intent match (existing behaviour
    /// unchanged): one or more components retrieved and ready to assemble.
    Components(Vec<ComponentItem>),
    /// Multiple near-equal intent candidates — the orchestrator should surface
    /// a disambiguation message to the user (spec §3.12 Q11).
    Disambiguation(Vec<crate::memory::intent_system::IntentCandidate>),
    /// Intent matched an Action (class 16) — execute directly, no LLM. The
    /// `name` comes from `IntentResolution::Match.component_name` (populated by
    /// `resolve_intent`'s `reborn_actions` LEFT JOIN, FIND-P5-06) so no second
    /// DB fetch is needed. Returned BEFORE any `fetch_component_by_id` call.
    ActionShortCircuit {
        component_id: uuid::Uuid,
        name: String,
    },
    /// Intent matched a Recipe (class 21) with a `step_link`. The IBS compiled
    /// the recipe's `step_descriptions` + the matched variant's
    /// `variable_patterns` into Rust-only (`rust_steps`) and orchestrator
    /// (`orchestrator_steps`) channels; the component items for each channel
    /// are pre-fetched here, and `routing` carries the Tier-0/Tier-1 dispatch
    /// signals for the Phase-H consumer.
    SplitResult {
        /// ToolSkill bodies — Rust-only channel (Tier-0 deterministic dispatch).
        rust_items: Vec<ComponentItem>,
        /// Skill + PythonCode bodies — orchestrator channel (LLM prior knowledge).
        orchestrator_items: Vec<ComponentItem>,
        routing: TurnRoutingSignals,
        /// The compiled `BuildInstruction` carrying the per-step structure
        /// (channel split, `include` UUIDs, `tool_bindings` with
        /// `{{vars.name}}` already substituted into `params`) so the Phase-H
        /// `RecipeStage` / `TierZeroExecutionStage` consumer can dispatch
        /// Tier-0 tool invocations without re-compiling. `None` when
        /// `build_instruction` soft-failed (§7.4: empty channels +
        /// `llm_call_required = true`). Upgrade per subplan §7.5.
        /// Boxed to keep `FetchForTurnResult::SplitResult` under clippy's
        /// `large_enum_variant` threshold — `BuildInstruction` carries four
        /// `Vec`s (~96 bytes inline) and, together with the routing signals
        /// added in v3 Phase H.4, would otherwise dominate the whole enum's
        /// size. `FetchForTurnResult` is a one-shot transient return from
        /// `fetch_for_turn` (built once per turn, never bulk-stored), so the
        /// single heap indirection has no throughput cost. Matches the
        /// `ThreadOutcome::GatePaused.paused_lease` boxing precedent.
        instruction: Option<Box<crate::memory::instruction_builder::BuildInstruction>>,
    },
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
// RamSource — keyword-retrieval over a Store (postgres-backed in production)
// ---------------------------------------------------------------------------

/// Keyword-retrieval source.
///
/// Wraps the legacy `Store`-based `RetrievalEngine` and maps `MemoryDoc` rows
/// to `ComponentItem` using the `doc_type_to_class_code` table below.
/// In production the `Store` is `PgMemoryDocStore` (postgres-backed), so this is
/// keyword-retrieval **over postgres** — not a postgres-less backend. The static
/// filesystem fallback-content file (`BRASSCLAW_FALLBACK_CONTENT_FILE`) that
/// previously supported "fully offline / DB-less deployments" has been removed:
/// Postgres is mandatory.
///
/// This legacy keyword path does NOT use the intent system (`resolve_intent`); it
/// is replaced by intent-driven `PostgresSource` in v3 Phase K.
pub struct RamSource {
    engine: super::RetrievalEngine,
}

impl RamSource {
    /// Create a `RamSource` backed by `store`.
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
                    // RamSource is a prompt-assembly path; executable Action
                    // steps are not surfaced here (Q-G-STUB1).
                    steps: None,
                    allowed_tools: None,
                });
            }

            // Sort by (class_code, prompt_uid) for deterministic assembly order.
            items.sort_by_key(|item| (item.class_code, item.prompt_uid));
            return Ok(items);
        }

        // No filesystem fallback (Postgres is mandatory). When the live store
        // returns nothing, retrieval is simply empty for this scope/query.
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
        //
        // Builtin union (Phase C.2): each sub-select also returns
        // `source = 'system' AND validation_status = 'validated'` rows
        // tenant-globally — i.e. tenant-anchored ($1) but agnostic on
        // user_id/agent_id/project_id — so seeded builtins are visible to
        // every turn under the tenant without a per-scope seed. The
        // validator-tag + consumer-tag filters still apply to system rows.
        // Tenant isolation is preserved (no cross-tenant leak).
        let query_sql = "
            SELECT id, class_code, prompt_uid, name, description, effective_content,
                   override_prompt_creation
            FROM (
                -- reborn_skills (classes 1-3)
                -- `reborn_skills` has no `prior_knowledge_content` / `override_prompt_creation`
                -- columns (V027 lacks them; V046 added them to 8 other component tables but
                -- NOT reborn_skills). `body` is the content column; solution-override is N/A.
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, description,
                       body AS effective_content,
                       false AS override_prompt_creation
                FROM reborn_skills
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_extensions_unified (classes 4-9)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, description,
                       COALESCE(prior_knowledge_content, description) AS effective_content,
                       override_prompt_creation
                FROM reborn_extensions_unified
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_actions (class 16)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, description,
                       COALESCE(prior_knowledge_content, description) AS effective_content,
                       override_prompt_creation
                FROM reborn_actions
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_specs (class 12)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_specs
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_tool_skills (class 13)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_tool_skills
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_plans (class 14)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_plans
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_summaries (class 15)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_summaries
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_docus (class 17)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_docus
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_lessons (class 18)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_lessons
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_issues (class 19)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_issues
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_notes (class 20)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_notes
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_recipes (class 21)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), '') AS effective_content,
                       override_prompt_creation
                FROM reborn_recipes
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_python_code (class 22)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), content) AS effective_content,
                       override_prompt_creation
                FROM reborn_python_code
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )

                UNION ALL

                -- reborn_extension_catalogues (class 23)
                SELECT id::text, class_code::int, prompt_uid::bigint,
                       name, '' AS description,
                       COALESCE(NULLIF(prior_knowledge_content, ''), overview_doc) AS effective_content,
                       override_prompt_creation
                FROM reborn_extension_catalogues
                WHERE tenant_id = $1
                  AND validation_status = 'validated'
                  AND '05:validator' != ALL(consumer_tags)
                  AND $5 = ANY(consumer_tags)
                  AND ( (user_id = $2 AND agent_id = $3 AND project_id = $4)
                        OR source = 'system' )
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
                // Broad-scan Components path builds orchestrator_content, not
                // an executable doc; Action steps are not surfaced here
                // (Q-G-STUB1).
                steps: None,
                allowed_tools: None,
            });
        }

        Ok(items)
    }

    /// Intent-driven retrieval (v3 Phase E / plan §0.8 + subplan §7).
    ///
    /// Runs `resolve_intent` first. On a single `Match` it dispatches by
    /// class code BEFORE any component fetch (FIND-P5-06):
    ///   - class 16 (Action) → `ActionShortCircuit { component_id, name }`
    ///     (execute directly, no LLM). `name` comes from the intent match's
    ///     `component_name` (populated by `resolve_intent`'s `reborn_actions`
    ///     LEFT JOIN) — no second DB fetch.
    ///   - class 21 (Recipe) with a `step_link` → `SplitResult` via
    ///     [`PostgresSource::fetch_recipe_split_result`] (IBS compile +
    ///     batched channel fetch + `{{vars.name}}` substitution).
    ///   - anything else (legacy / non-variant recipe / other class) → the
    ///     existing per-UUID `fetch_component_by_id` → `Components` path.
    /// `Disambiguation` is surfaced as-is; `NoMatch` / DB error fall back to
    /// the full UNION ALL scan (`fetch_for_consumer`). `resolve_intent`
    /// already atomically increments the matched row's score (PERF-03,
    /// SEC-05) — no separate increment is needed.
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
                step_link,
                component_name,
            }) => {
                // Score already incremented inside resolve_intent (PERF-03).
                if component_class_code == 16 {
                    // Action intent — execute directly, no LLM, no fetch.
                    return Ok(FetchForTurnResult::ActionShortCircuit {
                        component_id,
                        name: component_name,
                    });
                }
                if component_class_code == 21
                    && let Some(step_link) = step_link
                {
                    return self
                        .fetch_recipe_split_result(
                            scope,
                            component_id,
                            step_link,
                            query,
                            token_budget,
                            sender_class_code,
                        )
                        .await;
                }
                // Legacy / non-variant recipe / other class: fetch the
                // specific component from its class table (SEC-01 gate).
                let items =
                    fetch_component_by_id(&self.pool, scope, component_id, component_class_code)
                        .await?;
                Ok(FetchForTurnResult::Components(items))
            }
            Ok(IntentResolution::Disambiguation { candidates }) => {
                // Multiple near-equal matches — the orchestrator must surface
                // a disambiguation message before proceeding.
                Ok(FetchForTurnResult::Disambiguation(candidates))
            }
            Ok(IntentResolution::NoMatch) | Err(_) => {
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
// PostgresSource inherent helpers — Phase E SplitResult assembly
// ---------------------------------------------------------------------------

#[cfg(feature = "skills-db")]
impl PostgresSource {
    /// Phase E (§0.8 + subplan §7): build the `SplitResult` for a class-21
    /// recipe intent match with a `step_link`.
    ///
    /// 1. Fetch the recipe row — scope filter ONLY (§7.2: NO SEC-01 hard
    ///    gate; SELECT `validation_status`/`tier`/`wilson_lower` to compute
    ///    `tier0_eligible`). The per-UUID sub-component fetches still enforce
    ///    the full SEC-01 gate. If the row is absent (TOCTOU — deleted
    ///    between the intent match and this fetch; unreachable in practice)
    ///    → fall back to the broad-scan `Components` path (§7.2).
    /// 2. Compute `tier0_eligible` (tier ∈ {mature, candidate} AND
    ///    `validation_status = 'validated'` AND `wilson_lower ≥ 0.70`; the
    ///    `has_validation` guard is subsumed by `validated ⇒ has_validation`
    ///    per §0.23 / Q2→A). `llm_call_required = !tier0_eligible` (Q3→A).
    /// 3. Deserialise `variants` → find the variant whose `step_link` matches
    ///    (§7.3). Fall back to `variable_patterns = vec![]` and
    ///    `variant_label = recipe.name`.
    /// 4. Deserialise `step_descriptions` → `Vec<StepDescriptionEntry>`.
    /// 5. IBS `build_instruction` — soft-fail on error (§7.4) → empty
    ///    `SplitResult`, `llm_call_required = true`, `tier0_eligible = false`,
    ///    `instruction = None`.
    /// 6. `capture_variables(query, query, &variable_patterns)` (§7.1: exact-
    ///    match intent ⇒ `input_text == query`, so `template = user_text =
    ///    query`; inert until Phase M switches to `%`-template matching).
    /// 7. Gather channel `include` UUIDs; resolve each via
    ///    `lookup_component_class` (PERF-02: one indexed SELECT per UUID).
    /// 8. Batched `fetch_components_by_ids` (FIND-P9-04) — O(tables).
    /// 9. Partition into channels + substitute `{{vars.name}}` into every
    ///    fetched `ComponentItem.effective_content` (§0.20.3).
    /// 10. Substitute `{{vars.name}}` into every `tool_bindings[].params`
    ///     (§0.4.1) on BOTH channels (no-op on empty orchestrator bindings),
    ///     mutating the `BuildInstruction` in place.
    /// 11. Assemble `TurnRoutingSignals` + `SplitResult { instruction: Some }`.
    async fn fetch_recipe_split_result(
        &self,
        scope: &ComponentScope,
        recipe_id: uuid::Uuid,
        step_link: String,
        query: &str,
        token_budget: usize,
        sender_class_code: &str,
    ) -> Result<FetchForTurnResult, RetrievalSourceError> {
        use crate::memory::instruction_builder::{
            StepDescriptionEntry, build_instruction, capture_variables, substitute_vars,
            substitute_vars_in_value,
        };
        use crate::types::recipe::RecipeVariant;
        use tokio_postgres::types::ToSql;

        // 1. Recipe row — scope filter only (§7.2). JSONB read as text +
        //    serde_json::from_str (engine idiom; avoids relying on
        //    tokio_postgres serde_json feature availability in this crate).
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
        let row = client
            .query_opt(
                "SELECT name, tier, wilson_lower, validation_status,
                        override_prompt_creation,
                        COALESCE(step_descriptions::text, 'null') AS step_descriptions_text,
                        COALESCE(variants::text, 'null') AS variants_text
                 FROM reborn_recipes
                 WHERE id = $1
                   AND tenant_id  = $2
                   AND user_id    = $3
                   AND agent_id   = $4
                   AND project_id = $5",
                &[
                    &recipe_id as &(dyn ToSql + Sync),
                    &scope.tenant_id,
                    &scope.user_id,
                    &scope.agent_id,
                    &scope.project_id,
                ],
            )
            .await
            .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

        let Some(row) = row else {
            // TOCTOU absent row → broad-scan Components (§7.2).
            let items = self
                .fetch_for_consumer(scope, query, token_budget, sender_class_code)
                .await?;
            return Ok(FetchForTurnResult::Components(items));
        };

        let recipe_name: String = row.get(0);
        let tier: String = row.get(1);
        let wilson_lower: f64 = row.get(2);
        let validation_status: String = row.get(3);
        let override_prompt_creation: bool = row.get(4);
        let step_descriptions_text: String = row.get(5);
        let variants_text: String = row.get(6);

        // 2. Tier-0 eligibility (has_validation subsumed by validated, §0.23).
        let tier0_eligible = matches!(tier.as_str(), "mature" | "candidate")
            && validation_status == "validated"
            && wilson_lower >= 0.70;
        let llm_call_required = !tier0_eligible;

        // 3. Matched variant (§7.3).
        let variants: Vec<RecipeVariant> = serde_json::from_str(&variants_text).unwrap_or_default();
        let matched_variant = variants
            .iter()
            .find(|v| v.step_link.as_deref() == Some(step_link.as_str()));
        let (variable_patterns, variant_label) = match matched_variant {
            Some(v) => (v.variable_patterns.clone(), v.variant_key.clone()),
            None => (Vec::new(), recipe_name.clone()),
        };

        // 4. StepDescriptions.
        let step_descs: Vec<StepDescriptionEntry> =
            serde_json::from_str(&step_descriptions_text).unwrap_or_default();

        // 5. IBS compile (soft-fail §7.4).
        let Ok(mut instruction) = build_instruction(
            &step_link,
            &step_descs,
            &variable_patterns,
            llm_call_required,
        ) else {
            return Ok(FetchForTurnResult::SplitResult {
                rust_items: Vec::new(),
                orchestrator_items: Vec::new(),
                routing: TurnRoutingSignals {
                    override_prompt_creation,
                    matched_component_ids: Vec::new(),
                    variant_label: recipe_name.clone(),
                    step_link: step_link.clone(),
                    llm_call_required: true,
                    wilson_lower,
                    tier0_eligible: false,
                    recipe_id: Some(recipe_id.to_string()),
                    recipe_name: recipe_name.clone(),
                },
                instruction: None,
            });
        };

        // 6. Capture {{vars.name}} (§7.1: template = user_text = query).
        let vars = capture_variables(query, query, &variable_patterns);

        // 7. Per-channel include UUIDs (deduped within a channel) → registry
        //    class resolution (PERF-02: one indexed SELECT per UUID).
        let mut rust_uuids: std::collections::HashSet<uuid::Uuid> =
            std::collections::HashSet::new();
        for step in &instruction.rust_steps {
            for id in &step.include {
                rust_uuids.insert(*id);
            }
        }
        let mut orch_uuids: std::collections::HashSet<uuid::Uuid> =
            std::collections::HashSet::new();
        for step in &instruction.orchestrator_steps {
            for id in &step.include {
                orch_uuids.insert(*id);
            }
        }
        let mut rust_pairs: Vec<(uuid::Uuid, i32)> = Vec::with_capacity(rust_uuids.len());
        for id in &rust_uuids {
            if let Some(class_code) = lookup_component_class(&self.pool, scope, *id).await? {
                rust_pairs.push((*id, class_code));
            }
        }
        let mut orch_pairs: Vec<(uuid::Uuid, i32)> = Vec::with_capacity(orch_uuids.len());
        for id in &orch_uuids {
            if let Some(class_code) = lookup_component_class(&self.pool, scope, *id).await? {
                orch_pairs.push((*id, class_code));
            }
        }

        // 8. Two batched fetches — one per channel (PERF-02 / §0.8: "two
        //    batched fetches (one per channel) replace N per-UUID queries").
        //    A UUID included by both channels is fetched per-channel and so
        //    appears in both result lists (§0.8 fetches per-channel).
        let mut rust_items: Vec<ComponentItem> =
            fetch_components_by_ids(&self.pool, scope, &rust_pairs).await?;
        let mut orchestrator_items: Vec<ComponentItem> =
            fetch_components_by_ids(&self.pool, scope, &orch_pairs).await?;

        // 9. Substitute {{vars.name}} into every fetched body (§0.20.3).
        for item in rust_items.iter_mut() {
            item.effective_content = substitute_vars(&item.effective_content, &vars);
        }
        for item in orchestrator_items.iter_mut() {
            item.effective_content = substitute_vars(&item.effective_content, &vars);
        }

        // 10. Substitute vars into tool_bindings[].params (§0.4.1), both
        //     channels (no-op on empty orchestrator bindings).
        for step in instruction.rust_steps.iter_mut() {
            for tb in step.tool_bindings.iter_mut() {
                tb.params = substitute_vars_in_value(&tb.params, &vars);
            }
        }
        for step in instruction.orchestrator_steps.iter_mut() {
            for tb in step.tool_bindings.iter_mut() {
                tb.params = substitute_vars_in_value(&tb.params, &vars);
            }
        }

        // 11. Routing signals + SplitResult. matched_component_ids are the
        //     orchestrator-channel UUIDs (the orchestrator-channel identity
        //     set for the turn — Wilson scoring / record_recipe_outcome).
        let matched_component_ids: Vec<String> = orchestrator_items
            .iter()
            .map(|i| i.id.to_string())
            .collect();

        Ok(FetchForTurnResult::SplitResult {
            rust_items,
            orchestrator_items,
            routing: TurnRoutingSignals {
                override_prompt_creation,
                matched_component_ids,
                variant_label,
                step_link,
                llm_call_required,
                wilson_lower,
                tier0_eligible,
                recipe_id: Some(recipe_id.to_string()),
                recipe_name: recipe_name.clone(),
            },
            instruction: Some(Box::new(instruction)),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Estimate token cost from byte length.
fn estimate_tokens(byte_len: usize) -> usize {
    ((byte_len as f64 * TOKENS_PER_BYTE) as usize).max(1)
}

/// Map a component `class_code` to its `(table_name, content_expr)` pair.
///
/// Shared by `fetch_component_by_id` and `fetch_components_by_ids` so the
/// literal class→table/column mapping lives in exactly one place (PERF-02 /
/// plan §0.8: "Extract the helper ... so both functions share the same mapping
/// — no duplication").
///
/// Tables with a `body` column (`reborn_skills`) use
/// `COALESCE(NULLIF(prior_knowledge_content,''), body)`; tables with a
/// `content` column use `COALESCE(NULLIF(prior_knowledge_content,''), content)`;
/// description-only tables (extensions, actions) use
/// `COALESCE(prior_knowledge_content, description)`; `reborn_extension_
/// catalogues` uses its `overview_doc` column. `reborn_tools` (class 0) has no
/// prompt text and returns `None`.
///
/// SECURITY: `table_name` and `content_expr` are ALWAYS `&'static str` literals
/// selected by this `match` — never user input. The callers interpolate them
/// into SQL via `format!()`, which is safe ONLY under this invariant. NEVER
/// extend this function (or its callers) to accept user-supplied table names or
/// column expressions. The `class_code` itself is an `i32` from the DB
/// (`reborn_components.class_code`), not from user input, so the dispatch is
/// safe. Document this constraint in any new caller.
#[cfg(feature = "skills-db")]
fn class_code_to_table(code: i32) -> Option<(&'static str, &'static str)> {
    match code {
        0 => None, // Tool — no prompt text in the component table.
        1..=3 => Some((
            "reborn_skills",
            "COALESCE(NULLIF(prior_knowledge_content,''), body)",
        )),
        4..=9 => Some((
            "reborn_extensions_unified",
            "COALESCE(prior_knowledge_content, description)",
        )),
        // ⚠️ FIND-NEW-AUDIT-06: classes 10 (Orchestrator) and 50 (Scaffold) map
        // to reborn_skills — confirmed from the live fetch_component_by_id.
        // MUST be present or Phase E silently loses retrieval for these classes.
        10 | 50 => Some((
            "reborn_skills",
            "COALESCE(NULLIF(prior_knowledge_content,''), body)",
        )),
        12 => Some((
            "reborn_specs",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        13 => Some((
            "reborn_tool_skills",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        14 => Some((
            "reborn_plans",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        15 => Some((
            "reborn_summaries",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        16 => Some((
            "reborn_actions",
            "COALESCE(prior_knowledge_content, description)",
        )),
        17 => Some((
            "reborn_docus",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        18 => Some((
            "reborn_lessons",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        19 => Some((
            "reborn_issues",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        20 => Some((
            "reborn_notes",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        21 => Some((
            "reborn_recipes",
            "COALESCE(NULLIF(prior_knowledge_content,''), '')",
        )),
        22 => Some((
            "reborn_python_code",
            "COALESCE(NULLIF(prior_knowledge_content,''), content)",
        )),
        23 => Some((
            "reborn_extension_catalogues",
            "COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)",
        )),
        // ⚠️ WHEN ADDING A NEW CLASS CODE: ADD A MATCH ARM HERE.
        // A missing arm silently returns None → fetch_for_turn produces an empty
        // SplitResult item list → the recipe executes without the component.
        // There is NO compile-time enforcement. Always add the arm AND a test.
        _ => None,
    }
}

/// Build a [`ComponentItem`] from a 9-column fetch row produced by
/// [`fetch_component_by_id`] / [`fetch_component_by_name`] (Q-G-STUB1 —
/// `subplan_stub_stepG_action_steps`).
///
/// Row shape (columns 0–8): `id::text, class_code::int, prompt_uid::bigint,
/// name, description, effective_content, override_prompt_creation,
/// steps_json, allowed_tools_arr`. Columns 7–8 are the executable `steps`
/// (JSONB) + `allowed_tools` (TEXT[]) for **class-16 Actions** and SQL NULL
/// for every other class, so they are read as `Option` to stay NULL-safe;
/// `steps` is surfaced only when the JSONB value is an array (class 16).
#[cfg(feature = "skills-db")]
fn component_item_from_row(row: &tokio_postgres::Row) -> ComponentItem {
    let id_str: &str = row.get(0);
    let id = id_str
        .parse::<uuid::Uuid>()
        .unwrap_or_else(|_| uuid::Uuid::nil());
    let steps_val: Option<serde_json::Value> = row.get(7);
    let allowed_tools_val: Option<Vec<String>> = row.get(8);
    ComponentItem {
        id,
        class_code: row.get(1),
        prompt_uid: row.get(2),
        name: row.get::<_, &str>(3).to_string(),
        description: row.get::<_, &str>(4).to_string(),
        effective_content: row.get::<_, &str>(5).to_string(),
        override_prompt_creation: row.get(6),
        steps: steps_val.filter(|v| v.is_array()),
        allowed_tools: allowed_tools_val.map(|v| {
            serde_json::Value::Array(v.into_iter().map(serde_json::Value::String).collect())
        }),
    }
}

/// Fetch a single component from its class-specific table by ID (Step 6.7).
///
/// Enforces the SEC-01 validation gate:
///   `validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)`
///
/// Returns an empty vec if the component is not found or fails the gate (e.g.
/// it was demoted to pending/rejected between the intent lookup and this fetch).
#[cfg(feature = "skills-db")]
pub async fn fetch_component_by_id(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    component_id: uuid::Uuid,
    component_class_code: i32,
) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
    use tokio_postgres::types::ToSql;

    // Map class_code → (table_name, content_expr) via the shared helper so the
    // literal class→table/column mapping lives in exactly one place
    // (see `class_code_to_table`; PERF-02 / plan §0.8).
    let Some((table, content_expr)) = class_code_to_table(component_class_code) else {
        // Class 0 (tools) or unknown — no prompt text to retrieve.
        return Ok(vec![]);
    };

    let client = pool
        .get()
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    // Class-16 Actions carry the executable `steps` (JSONB) + `allowed_tools`
    // (TEXT[]) so `execute_action_procedure` can run the real procedure
    // (Q-G-STUB1 — `subplan_stub_stepG_action_steps`). These columns exist only
    // on `reborn_actions`, so they are SELECTed via a class-16-specific
    // projection; for every other class the two extra columns are NULL, giving
    // a uniform 9-column row shape.
    let (steps_expr, allowed_tools_expr) = if component_class_code == 16 {
        ("steps", "allowed_tools")
    } else {
        ("NULL::jsonb", "NULL::text[]")
    };

    let query_sql = format!(
        "SELECT id::text, class_code::int, prompt_uid::bigint,
                name, COALESCE(description,'') AS description,
                {content_expr} AS effective_content,
                override_prompt_creation,
                {steps_expr} AS steps_json,
                {allowed_tools_expr} AS allowed_tools_arr
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

    let items: Vec<ComponentItem> = rows.iter().map(component_item_from_row).collect();

    Ok(items)
}

/// Fetch a single validated component from its class-specific table by
/// **name** (v3 Phase G.2 / Q-G4 — the Option B fallback the §0.9
/// `call_action` uses when it holds a step **name**, not a UUID).
///
/// Mirrors [`fetch_component_by_id`]: same `class_code_to_table` mapping,
/// the same SEC-01 validation gate
/// (`validation_status = 'validated' AND '05:validator' != ALL(consumer_tags)`),
/// and the same scope tuple — only the lookup key differs (`name = $1`
/// instead of `id = $1`). `LIMIT 1` so a name that is unique within a scope
/// resolves to exactly one component; bind order is **name first**, then the
/// scope tuple, matching the §0.9 plan.
///
/// Returns an empty vec if no validated component with that name exists
/// under the scope or the class code is unmapped (e.g. class 0 tools).
#[cfg(feature = "skills-db")]
pub async fn fetch_component_by_name(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    name: &str,
    component_class_code: i32,
) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
    use tokio_postgres::types::ToSql;

    let Some((table, content_expr)) = class_code_to_table(component_class_code) else {
        return Ok(vec![]);
    };

    let client = pool
        .get()
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    // Class-16 Actions carry the executable `steps` (JSONB) + `allowed_tools`
    // (TEXT[]), mirroring `fetch_component_by_id` (Q-G-STUB1). Same
    // class-16-specific projection; for every other class the two extra
    // columns are NULL.
    let (steps_expr, allowed_tools_expr) = if component_class_code == 16 {
        ("steps", "allowed_tools")
    } else {
        ("NULL::jsonb", "NULL::text[]")
    };

    let query_sql = format!(
        "SELECT id::text, class_code::int, prompt_uid::bigint,
                name, COALESCE(description,'') AS description,
                {content_expr} AS effective_content,
                override_prompt_creation,
                {steps_expr} AS steps_json,
                {allowed_tools_expr} AS allowed_tools_arr
         FROM {table}
         WHERE name = $1
           AND tenant_id  = $2
           AND user_id    = $3
           AND agent_id   = $4
           AND project_id = $5
           AND validation_status = 'validated'
           AND '05:validator' != ALL(consumer_tags)
         LIMIT 1"
    );

    let params: &[&(dyn ToSql + Sync)] = &[
        &name,
        &scope.tenant_id,
        &scope.user_id,
        &scope.agent_id,
        &scope.project_id,
    ];

    let rows = client
        .query(&query_sql, params)
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    let items: Vec<ComponentItem> = rows.iter().map(component_item_from_row).collect();

    Ok(items)
}

/// Batch-fetch multiple components in O(tables) round-trips instead of O(N)
/// (PERF-02 / FIND-P9-04 — a Phase E requirement, not a future optimisation).
///
/// Groups `ids_by_class` by `(table, content_expr)` using the same
/// [`class_code_to_table`] match arm as [`fetch_component_by_id`], then emits
/// one `WHERE id = ANY($1) AND scope… AND validation_status = 'validated'`
/// query per group. Unknown class codes are silently skipped (same behaviour
/// as `fetch_component_by_id` returning `None` — the caller handles missing
/// items via the returned `Vec` length).
///
/// SECURITY: `table_name` and `content_expr` are ALWAYS `&'static str`
/// literals from the `class_code_to_table` match arm — never user input. This
/// is the same invariant as `fetch_component_by_id`. NEVER extend this
/// function to accept user-supplied table names or column expressions. The
/// `class_code` is an `i32` from the DB (`reborn_components.class_code`), not
/// from user input. Uses `tokio_postgres` directly (`pool.get()` +
/// `client.query()`) — this codebase does NOT use sqlx.
#[cfg(feature = "skills-db")]
pub async fn fetch_components_by_ids(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    ids_by_class: &[(uuid::Uuid, i32)],
) -> Result<Vec<ComponentItem>, RetrievalSourceError> {
    use tokio_postgres::types::ToSql;

    // 1. Group by (table, content_expr) using the shared match arm. Unknown
    //    class codes are skipped (class 0 tools / unallocated codes have no
    //    prompt text).
    let mut groups: std::collections::HashMap<(&'static str, &'static str), Vec<uuid::Uuid>> =
        std::collections::HashMap::new();
    for (id, class_code) in ids_by_class {
        if let Some((table, content_expr)) = class_code_to_table(*class_code) {
            groups.entry((table, content_expr)).or_default().push(*id);
        }
    }

    if groups.is_empty() {
        return Ok(Vec::new());
    }

    let client = pool
        .get()
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    let mut results = Vec::new();
    for ((table, content_expr), ids) in &groups {
        // Empty id vec would produce an empty ANY array; skip (no rows anyway).
        if ids.is_empty() {
            continue;
        }
        // SECURITY: `table` + `content_expr` are &'static str literals from
        // class_code_to_table (never user input) — safe to interpolate.
        let sql = format!(
            "SELECT id::text, class_code::int, prompt_uid::bigint,
                    name, COALESCE(description,'') AS description,
                    {content_expr} AS effective_content,
                    override_prompt_creation
             FROM {table}
             WHERE id = ANY($1)
               AND tenant_id  = $2
               AND user_id    = $3
               AND agent_id   = $4
               AND project_id = $5
               AND validation_status = 'validated'
               AND '05:validator' != ALL(consumer_tags)"
        );
        let params: &[&(dyn ToSql + Sync)] = &[
            ids,
            &scope.tenant_id,
            &scope.user_id,
            &scope.agent_id,
            &scope.project_id,
        ];
        let rows = client
            .query(&sql, params)
            .await
            .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
        for row in rows {
            let id_str: &str = row.get(0);
            let id = id_str
                .parse::<uuid::Uuid>()
                .unwrap_or_else(|_| uuid::Uuid::nil());
            results.push(ComponentItem {
                id,
                class_code: row.get(1),
                prompt_uid: row.get(2),
                name: row.get::<_, &str>(3).to_string(),
                description: row.get::<_, &str>(4).to_string(),
                effective_content: row.get::<_, &str>(5).to_string(),
                override_prompt_creation: row.get(6),
                // Batched IBS-include fetch is a prompt-assembly path; the
                // executable `steps` of an included Action are fetched
                // separately by `__fetch_component__` / `__resolve_component_
                // by_name__` at execution time (Q-G-STUB1).
                steps: None,
                allowed_tools: None,
            });
        }
    }
    Ok(results)
}

/// Resolve a component UUID to its `class_code` via the `reborn_components`
/// registry (V061).
///
/// The registry is a flat UUID -> class_code + scope lookup kept in sync with
/// all 14 class tables by `maintain_components_registry` triggers. This closes
/// the FIND-IBS-02 gap: the IBS emits `IbsRecipeStep.include: Vec<Uuid>` with
/// no per-UUID `class_code`, but `fetch_component_by_id` needs the class to
/// pick the class-specific table. `fetch_for_turn` calls this once per step
/// include UUID (PERF-02: one indexed `SELECT` per UUID) before calling
/// `fetch_component_by_id`.
///
/// Scoped so a foreign-tenant UUID never resolves (SEC-01 tenant isolation):
/// the `WHERE` clause simply returns no row. Returns `Ok(None)` when the UUID
/// is absent from the registry (caller skips that step's item rather than
/// failing the turn — a missing include is a soft authoring gap, not a hard
/// error).
#[cfg(feature = "skills-db")]
pub async fn lookup_component_class(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    component_id: uuid::Uuid,
) -> Result<Option<i32>, RetrievalSourceError> {
    let client = pool
        .get()
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
    let row = client
        .query_opt(
            "SELECT class_code::int FROM reborn_components \
             WHERE id = $1 AND tenant_id = $2 AND user_id = $3 AND agent_id = $4 AND project_id = $5",
            &[
                &component_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;
    Ok(row.map(|r| r.get::<_, i32>(0)))
}

/// Map a `DocType` to its class code.
///
/// This is the authoritative table for DocType → class_code mapping in the
/// DB-less retrieval path.
fn doc_type_to_class_code(doc_type: crate::types::memory::DocType) -> (i32, &'static str) {
    use crate::types::memory::DocType;
    match doc_type {
        DocType::Skill => (3, "skill_llm"),
        DocType::Spec => (12, "spec"),
        DocType::ToolSkill => (13, "tool_skill"),
        DocType::Plan => (14, "plan"),
        DocType::Summary => (15, "summary"),
        DocType::Lesson => (18, "lesson"),
        DocType::Issue => (19, "issue"),
        DocType::Note => (20, "note"),
        DocType::Recipe => (21, "recipe"),
    }
}

// ---------------------------------------------------------------------------
// resolve_dependencies — Phase J.3 (§0.19)
// ---------------------------------------------------------------------------

/// One entry in a component's `dependency_registry` JSONB array.
///
/// Schema mirrors §0.19:
/// `[{ "idx": 0, "component_id": "<uuid>", "class_code": N, "label": "..." }, ...]`
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DependencyEntry {
    /// Positional index in this component's `dependency_registry`.
    pub idx: usize,
    /// UUID of the dependent component.
    pub component_id: uuid::Uuid,
    /// Class code of the dependent component (drives channel routing).
    pub class_code: i32,
    /// Human-readable label (authoring aid; not used at runtime).
    #[serde(default)]
    pub label: String,
}

/// Resolve the dependency traversal expression `expr` rooted at `root_id`
/// against the `dependency_registry` JSONB column on the root component's row
/// in the `reborn_components` registry (V061).
///
/// # Algorithm (§0.19)
/// For each [`DependencyNode`] in `expr`:
/// 1. Query the root component's `dependency_registry[node.idx]` → `(dep_uuid, dep_class)`.
/// 2. If `dep_uuid ∈ visited` → skip (dedup / cycle guard).
/// 3. `visited.insert(dep_uuid)`
/// 4. `fetch_component_by_id(dep_uuid, dep_class)` → [`ComponentItem`].
/// 5. If `node.sub == All` → fetch dep's full registry and recurse over each entry.
/// 6. If `node.sub == Selective(sub_nodes)` → recurse with `dep_uuid` + `sub_nodes`.
///
/// Returns `(orchestrator_items, rust_items)` — split by class code using the
/// same rule as `fetch_recipe_split_result` (class 13 ToolSkill → rust; all
/// others → orchestrator).
///
/// `visited` is shared across a whole `fetch_for_turn` call so a UUID fetched
/// by any earlier step is not re-fetched. Pass the same `visited` set through
/// the entire turn.
#[cfg(feature = "skills-db")]
pub async fn resolve_dependencies(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    root_id: uuid::Uuid,
    expr: &crate::memory::instruction_builder::DependencyExpr,
    visited: &mut std::collections::HashSet<uuid::Uuid>,
) -> Result<(Vec<ComponentItem>, Vec<ComponentItem>), RetrievalSourceError> {
    use crate::memory::instruction_builder::DependencySubExpr;

    let mut orchestrator_items: Vec<ComponentItem> = Vec::new();
    let mut rust_items: Vec<ComponentItem> = Vec::new();

    // Fetch the root component's dependency_registry from the reborn_components
    // registry table (V061 — maintains a class_code-indexed mirror of all
    // component IDs).  We then join to the class-specific table to get the
    // actual dependency_registry JSONB column.
    //
    // To load the root's registry we must know its class_code (so we can read
    // the correct table).  `lookup_component_class` provides that.
    let root_class = match lookup_component_class(pool, scope, root_id).await? {
        Some(c) => c,
        None => return Ok((orchestrator_items, rust_items)), // root not found → no deps
    };

    let root_registry: Vec<DependencyEntry> =
        fetch_dependency_registry(pool, scope, root_id, root_class).await?;

    for node in expr.iter() {
        let entry = match root_registry.iter().find(|e| e.idx == node.idx) {
            Some(e) => e,
            None => continue, // index out of range for this component's registry
        };

        let dep_uuid = entry.component_id;
        if !visited.insert(dep_uuid) {
            continue; // already collected or cycle
        }

        // Fetch the dependent component (returns Vec — empty when not found).
        let mut dep_items = fetch_component_by_id(pool, scope, dep_uuid, entry.class_code).await?;
        let dep_item = match dep_items.pop() {
            Some(item) => item,
            None => continue, // component missing from DB → soft skip
        };

        // Channel routing: class 13 (ToolSkill) → rust; everything else → orchestrator.
        let is_rust = dep_item.class_code == 13;

        // Recurse into sub-dependencies before pushing the item, so that
        // dependencies appear before the component that references them.
        match &node.sub {
            None => {} // leaf node — no recursion
            Some(DependencySubExpr::All) => {
                // Recursively load the dep's full registry.
                let dep_class = entry.class_code;
                let dep_registry =
                    fetch_dependency_registry(pool, scope, dep_uuid, dep_class).await?;
                if !dep_registry.is_empty() {
                    // Build a DependencyExpr covering all indices.
                    let all_nodes: crate::memory::instruction_builder::DependencyExpr =
                        dep_registry
                            .iter()
                            .map(|e| crate::memory::instruction_builder::DependencyNode {
                                idx: e.idx,
                                sub: Some(DependencySubExpr::All),
                            })
                            .collect();
                    let (sub_orch, sub_rust) = Box::pin(resolve_dependencies(
                        pool, scope, dep_uuid, &all_nodes, visited,
                    ))
                    .await?;
                    orchestrator_items.extend(sub_orch);
                    rust_items.extend(sub_rust);
                }
            }
            Some(DependencySubExpr::Selective(sub_nodes)) => {
                let (sub_orch, sub_rust) = Box::pin(resolve_dependencies(
                    pool, scope, dep_uuid, sub_nodes, visited,
                ))
                .await?;
                orchestrator_items.extend(sub_orch);
                rust_items.extend(sub_rust);
            }
        }

        if is_rust {
            rust_items.push(dep_item);
        } else {
            orchestrator_items.push(dep_item);
        }
    }

    Ok((orchestrator_items, rust_items))
}

/// Fetch the `dependency_registry` JSONB for a component of known class.
///
/// Returns an empty `Vec` when the column is `NULL` or the component is not
/// found. On JSON deserialization error, logs at `debug!` and returns empty
/// (soft-fail — a malformed registry is an authoring issue, not a hard error
/// at fetch time).
#[cfg(feature = "skills-db")]
async fn fetch_dependency_registry(
    pool: &brassclaw_pg::PgPool,
    scope: &ComponentScope,
    component_id: uuid::Uuid,
    class_code: i32,
) -> Result<Vec<DependencyEntry>, RetrievalSourceError> {
    let table = match class_code_to_table(class_code) {
        Some((t, _)) => t,
        None => return Ok(vec![]), // unknown class — no registry
    };

    let client = pool
        .get()
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    let sql = format!(
        "SELECT dependency_registry \
         FROM {table} \
         WHERE id = $1 AND tenant_id = $2 AND user_id = $3 AND agent_id = $4 AND project_id = $5"
    );
    let row = client
        .query_opt(
            &sql,
            &[
                &component_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .map_err(|e| RetrievalSourceError::Db(e.to_string()))?;

    let row = match row {
        Some(r) => r,
        None => return Ok(vec![]),
    };

    let json: Option<serde_json::Value> = row.get(0);
    let json = match json {
        Some(j) => j,
        None => return Ok(vec![]),
    };

    match serde_json::from_value::<Vec<DependencyEntry>>(json) {
        Ok(entries) => Ok(entries),
        Err(e) => {
            tracing::debug!(
                component_id = %component_id,
                "dependency_registry JSON deserialization failed: {e}; treating as empty"
            );
            Ok(vec![])
        }
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
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Note,
                "Note A",
                "note content",
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Spec,
                "Spec A",
                "spec content",
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "Lesson A",
                "lesson content",
            ),
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
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "Lesson 1",
                &large_content,
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "Lesson 2",
                &large_content,
            ),
            MemoryDoc::new(
                project,
                "test-user",
                DocType::Lesson,
                "Lesson 3",
                &large_content,
            ),
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

    // ⚠️ WHEN ADDING A NEW CLASS CODE: ADD A MATCH ARM in `class_code_to_table`
    // AND extend this test with the new arm's expected (table, content_expr).
    // A missing arm silently returns None → fetch_for_turn drops the component.
    #[cfg(feature = "skills-db")]
    #[test]
    fn class_code_to_table_covers_every_class_and_includes_10_50() {
        // Class 0 (Tool) has no prompt text in the component table → None.
        assert_eq!(class_code_to_table(0), None);

        // Skills (classes 1-3) + Orchestrator/Scaffold (10, 50) share
        // reborn_skills with the body-fallback content expression.
        // ⚠️ FIND-NEW-AUDIT-06: 10 | 50 MUST be present or Phase E silently
        // loses retrieval for these classes (regression guard).
        let skills_table = (
            "reborn_skills",
            "COALESCE(NULLIF(prior_knowledge_content,''), body)",
        );
        for code in [1, 2, 3, 10, 50] {
            assert_eq!(
                class_code_to_table(code),
                Some(skills_table),
                "class {code} must map to reborn_skills (FIND-NEW-AUDIT-06)"
            );
        }

        // Extensions (classes 4-9) → reborn_extensions_unified, description fallback.
        let exts_table = (
            "reborn_extensions_unified",
            "COALESCE(prior_knowledge_content, description)",
        );
        for code in 4..=9 {
            assert_eq!(
                class_code_to_table(code),
                Some(exts_table),
                "class {code} must map to reborn_extensions_unified"
            );
        }

        // Description-only / content-column tables.
        assert_eq!(
            class_code_to_table(16),
            Some((
                "reborn_actions",
                "COALESCE(prior_knowledge_content, description)"
            ))
        );

        // Content-column tables (specs/tool_skills/plans/summaries/docus/
        // lessons/issues/notes + Phases B/C additions python_code/catalogues).
        let content_pairs = [
            (12, "reborn_specs", "content"),
            (13, "reborn_tool_skills", "content"),
            (14, "reborn_plans", "content"),
            (15, "reborn_summaries", "content"),
            (17, "reborn_docus", "content"),
            (18, "reborn_lessons", "content"),
            (19, "reborn_issues", "content"),
            (20, "reborn_notes", "content"),
            (22, "reborn_python_code", "content"),
        ];
        for (code, table, col) in content_pairs {
            let expected_expr = format!("COALESCE(NULLIF(prior_knowledge_content,''), {col})");
            assert_eq!(
                class_code_to_table(code),
                Some((table, expected_expr.as_str())),
                "class {code} must map to {table}"
            );
        }

        // Recipes (class 21) — empty-string fallback (no body/content column).
        assert_eq!(
            class_code_to_table(21),
            Some((
                "reborn_recipes",
                "COALESCE(NULLIF(prior_knowledge_content,''), '')"
            ))
        );

        // Extension catalogues (class 23, Phase C) — overview_doc column.
        assert_eq!(
            class_code_to_table(23),
            Some((
                "reborn_extension_catalogues",
                "COALESCE(NULLIF(prior_knowledge_content,''), overview_doc)"
            ))
        );

        // Unknown / unallocated class codes → None (no silent fall-through).
        assert_eq!(class_code_to_table(11), None);
        assert_eq!(class_code_to_table(24), None);
        assert_eq!(class_code_to_table(49), None);
        assert_eq!(class_code_to_table(51), None);
        assert_eq!(class_code_to_table(-1), None);
        assert_eq!(class_code_to_table(999), None);
    }

    #[test]
    fn turn_routing_signals_recipe_id_carried_through_split_result() {
        // v3 Phase H4.3: a SplitResult built on the recipe path carries
        // `recipe_id == Some(uuid)` so the Tier-0 consumers downstream of
        // this struct can read it: H4.4 surfaces it into the pkr dict,
        // H4.5 stamps it onto `recipe_tier_zero_*` events, and H4.6 builds
        // `OrchestratorResult.tier_zero_outcome` from it. Both
        // `fetch_recipe_split_result` construction sites set
        // `Some(recipe_id.to_string())`; non-recipe paths + fixtures use
        // `None`. The DB-backed assertion that the recipe sites actually
        // populate `Some` is the Phase-H.5 composition integration test;
        // this unit test locks in the field's presence + accessibility
        // (guards against accidental removal / silent drop).
        let recipe_uuid = uuid::Uuid::nil();
        let routing = TurnRoutingSignals {
            override_prompt_creation: false,
            matched_component_ids: Vec::new(),
            variant_label: "default".to_string(),
            step_link: "recipe#greet".to_string(),
            llm_call_required: false,
            wilson_lower: 0.85,
            tier0_eligible: true,
            recipe_id: Some(recipe_uuid.to_string()),
            recipe_name: "greet-recipe".to_string(),
        };
        let split = FetchForTurnResult::SplitResult {
            rust_items: Vec::new(),
            orchestrator_items: Vec::new(),
            routing,
            instruction: None,
        };
        match split {
            FetchForTurnResult::SplitResult { routing, .. } => {
                assert_eq!(
                    routing.recipe_id.as_deref(),
                    Some(recipe_uuid.to_string()).as_deref(),
                    "recipe_id must be carried through the SplitResult variant"
                );
                assert_eq!(
                    routing.recipe_name, "greet-recipe",
                    "recipe_name must be carried through the SplitResult variant"
                );
            }
            _ => panic!("expected SplitResult"),
        }
    }

    // ── DependencyEntry (Phase J.2 / §0.19) ────────────────────────────────

    /// DependencyEntry deserializes from the canonical JSONB registry shape.
    #[test]
    fn dependency_entry_deserializes_from_registry_json() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let json = serde_json::json!([
            {
                "idx": 0,
                "component_id": uuid_str,
                "class_code": 1,
                "label": "pipe-skill"
            }
        ]);
        let entries: Vec<DependencyEntry> = serde_json::from_value(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].idx, 0);
        assert_eq!(entries[0].class_code, 1);
        assert_eq!(entries[0].label, "pipe-skill");
        assert_eq!(entries[0].component_id.to_string(), uuid_str);
    }

    /// DependencyEntry: missing label defaults to empty string.
    #[test]
    fn dependency_entry_missing_label_defaults_to_empty() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440001";
        let json = serde_json::json!([
            {"idx": 2, "component_id": uuid_str, "class_code": 22}
        ]);
        let entries: Vec<DependencyEntry> = serde_json::from_value(json).unwrap();
        assert_eq!(entries[0].label, "");
        assert_eq!(entries[0].idx, 2);
        assert_eq!(entries[0].class_code, 22);
    }

    /// Empty dependency_registry array deserializes to empty Vec.
    #[test]
    fn dependency_entry_empty_array_gives_empty_vec() {
        let json = serde_json::json!([]);
        let entries: Vec<DependencyEntry> = serde_json::from_value(json).unwrap();
        assert!(entries.is_empty());
    }
}
