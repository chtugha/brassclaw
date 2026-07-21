//! Postgres-native store and [`RecipeLookup`] adapter for `reborn_recipes`
//! (Phase 4, Step 4.3 / Step 5 — class 21).
//!
//! Provides:
//! - [`PgRecipeStore`]: CRUD over `reborn_recipes` (V033).
//! - [`PgRecipeLibrary`]: implements `brassclaw_turns::run_profile::RecipeLookup`
//!   reading directly from `reborn_recipes`.  Replaces the MemoryDoc-backed
//!   `RecipeLibrary` for class-21 rows once V033 is applied.
//!
//! # Delivery filter
//!
//! `find_recipe` / `find_skills` only return `validation_status = 'validated'`
//! rows that do NOT carry `05:validator` in `consumer_tags` (SEC-01, §3.9).
//!
//! # Scope
//!
//! All queries are scoped by `(tenant_id, user_id, agent_id, project_id)`.
//! The `PgRecipeLibrary` is wired with the default local-dev scope (`local` /
//! `default`) to preserve backward compat with the existing `RecipeLibrary`
//! adapter.  Callers that need a different scope construct `PgRecipeStore`
//! directly and pass explicit scope parameters.
//!
//! # Feature gate
//!
//! Both types require the `postgres` feature.

#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_agent_loop::plan_scoring::{SkillMaturityTier, classify_tier, wilson_lower_bound};
use brassclaw_pg::PgPool;
use brassclaw_turns::run_profile::{
    RecipeLookup, RecipeLookupError, RecipeMatchDto, RecipeStepDto, ToolSkillMatchDto,
};
use serde_json::Value;
use thiserror::Error;
use tracing::debug;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by `reborn_recipes` store operations.
#[derive(Debug, Error)]
pub enum PgRecipeStoreError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("serialization error: {reason}")]
    Serialize { reason: String },
    #[error("recipe not found: {id}")]
    NotFound { id: String },
    #[error("invalid transition or data: {reason}")]
    Invalid { reason: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> PgRecipeStoreError {
    PgRecipeStoreError::Pool {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> PgRecipeStoreError {
    PgRecipeStoreError::Db {
        reason: e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

/// A fully-decoded `reborn_recipes` row.
#[derive(Debug, Clone)]
pub struct PgRecipe {
    pub id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    /// Trigger JSON — `{type: "exact"|"pattern"|"keyword", payload: ...}`.
    pub trigger: Option<Value>,
    /// Ordered step array — `[{skill: name, params: {...}}]`.
    pub steps: Value,
    pub status: String,
    pub prior_knowledge_content: Option<String>,
    pub override_prompt_creation: bool,
    pub class_code: i16,
    pub prompt_uid: i64,
    pub consumer_tags: Vec<String>,
    pub intent_examples: Option<Value>,
    pub tier: String,
    pub usage_count: i32,
    pub success_count: i32,
    pub failure_count: i32,
    pub wilson_lower: f64,
    pub validation_status: String,
    pub validation_errors: Vec<String>,
    pub review_feedback: Option<String>,
    pub review_attempts: i16,
    pub rejected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub queue_code: Option<String>,
    pub source: String,
    pub content_hash: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl PgRecipe {
    /// Returns true iff the row carries the `05:validator` consumer tag.
    pub fn has_validator_tag(&self) -> bool {
        self.consumer_tags.iter().any(|t| t == "05:validator")
    }

    /// Returns true iff this recipe is deliverable to consumers (validated + no
    /// validator tag, per §3.9 SEC-01).
    pub fn is_deliverable(&self) -> bool {
        self.validation_status == "validated" && !self.has_validator_tag()
    }

    /// Returns true iff this recipe is eligible for Tier-0 direct execution.
    pub fn is_tier0_eligible(&self) -> bool {
        self.is_deliverable() && matches!(self.tier.as_str(), "mature" | "candidate")
    }
}

/// Minimal data required to insert a new recipe row.
#[derive(Debug, Clone)]
pub struct NewPgRecipe {
    pub tenant_id: String,
    pub user_id: String,
    pub agent_id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub trigger: Option<Value>,
    pub steps: Value,
    pub prior_knowledge_content: Option<String>,
    pub override_prompt_creation: bool,
    /// Consumer tags — caller must include `05:validator` for new rows.
    pub consumer_tags: Vec<String>,
    pub intent_examples: Option<Value>,
    pub source: String,
}

// ---------------------------------------------------------------------------
// Parameter structs (keep argument counts under the clippy threshold)
// ---------------------------------------------------------------------------

/// Grouped parameters for [`PgRecipeStore::update_validation_status`].
#[derive(Debug)]
pub struct RecipeValidationStatusUpdate<'a> {
    pub validation_status: &'a str,
    pub validation_errors: Vec<String>,
    pub review_feedback: Option<String>,
    pub queue_code: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Map a [`SkillMaturityTier`] to the DB tier label used by `reborn_recipes`.
fn tier_label(tier: SkillMaturityTier) -> &'static str {
    match tier {
        SkillMaturityTier::Seedling => "seedling",
        SkillMaturityTier::Growing => "growing",
        SkillMaturityTier::Mature => "mature",
        SkillMaturityTier::Candidate => "candidate",
    }
}

// ---------------------------------------------------------------------------
// PgRecipeStore
// ---------------------------------------------------------------------------

/// Postgres-backed store for `reborn_recipes` (class 21).
#[derive(Clone)]
pub struct PgRecipeStore {
    pool: Arc<PgPool>,
}

impl PgRecipeStore {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

/// Canonical SELECT column list — order must match [`decode_recipe_row`].
const RECIPE_SELECT: &str = "
    id, tenant_id, user_id, agent_id, project_id,
    name, description, trigger, steps, status,
    prior_knowledge_content, override_prompt_creation,
    class_code, prompt_uid, consumer_tags, intent_examples,
    tier, usage_count, success_count, failure_count, wilson_lower,
    validation_status, validation_errors, review_feedback,
    review_attempts, rejected_at, queue_code, source, content_hash,
    created_at, updated_at
";

fn decode_recipe_row(row: &tokio_postgres::Row) -> Result<PgRecipe, PgRecipeStoreError> {
    Ok(PgRecipe {
        id: row.get(0),
        tenant_id: row.get(1),
        user_id: row.get(2),
        agent_id: row.get(3),
        project_id: row.get(4),
        name: row.get(5),
        description: row.get(6),
        trigger: row.get(7),
        steps: row.get(8),
        status: row.get(9),
        prior_knowledge_content: row.get(10),
        override_prompt_creation: row.get(11),
        class_code: row.get(12),
        prompt_uid: row.get(13),
        consumer_tags: row.get(14),
        intent_examples: row.get(15),
        tier: row.get(16),
        usage_count: row.get(17),
        success_count: row.get(18),
        failure_count: row.get(19),
        wilson_lower: row.get(20),
        validation_status: row.get(21),
        validation_errors: row.get(22),
        review_feedback: row.get(23),
        review_attempts: row.get(24),
        rejected_at: row.get(25),
        queue_code: row.get(26),
        source: row.get(27),
        content_hash: row.get(28),
        created_at: row.get(29),
        updated_at: row.get(30),
    })
}

impl PgRecipeStore {
    /// Insert a new recipe.  Returns the assigned UUID.
    pub async fn insert(&self, row: NewPgRecipe) -> Result<Uuid, PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let db_row = client
            .query_one(
                "INSERT INTO reborn_recipes
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, trigger, steps,
                     prior_knowledge_content, override_prompt_creation,
                     consumer_tags, intent_examples, source)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
                 RETURNING id",
                &[
                    &row.tenant_id,
                    &row.user_id,
                    &row.agent_id,
                    &row.project_id,
                    &row.name,
                    &row.description,
                    &row.trigger,
                    &row.steps,
                    &row.prior_knowledge_content,
                    &row.override_prompt_creation,
                    &row.consumer_tags,
                    &row.intent_examples,
                    &row.source,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(db_row.get(0))
    }

    /// Fetch a single recipe by id + scope.
    pub async fn get(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<Option<PgRecipe>, PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {RECIPE_SELECT} FROM reborn_recipes
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5"
        );
        let row = client
            .query_opt(&q, &[&id, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_recipe_row).transpose()
    }

    /// Fetch a single recipe by name + scope.
    pub async fn get_by_name(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        name: &str,
    ) -> Result<Option<PgRecipe>, PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {RECIPE_SELECT} FROM reborn_recipes
             WHERE name = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5
             LIMIT 1"
        );
        let row = client
            .query_opt(&q, &[&name, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        row.as_ref().map(decode_recipe_row).transpose()
    }

    /// List all recipes for the scope (admin / validation-queue path).
    pub async fn list_all(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<PgRecipe>, PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {RECIPE_SELECT} FROM reborn_recipes
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
             ORDER BY prompt_uid ASC"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_recipe_row).collect()
    }

    /// List deliverable recipes for a consumer (§3.9 SEC-01 delivery filter).
    pub async fn fetch_validated(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
    ) -> Result<Vec<PgRecipe>, PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let q = format!(
            "SELECT {RECIPE_SELECT} FROM reborn_recipes
             WHERE tenant_id = $1 AND user_id = $2
               AND agent_id  = $3 AND project_id = $4
               AND validation_status = 'validated'
               AND NOT ('05:validator' = ANY(consumer_tags))
             ORDER BY prompt_uid ASC"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_recipe_row).collect()
    }

    /// Update validation status + queue code.
    pub async fn update_validation_status(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        update: RecipeValidationStatusUpdate<'_>,
    ) -> Result<(), PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE reborn_recipes
                 SET validation_status = $1,
                     validation_errors = $2,
                     review_feedback   = COALESCE($3, review_feedback),
                     queue_code        = $4
                 WHERE id = $5
                   AND tenant_id = $6 AND user_id = $7
                   AND agent_id  = $8 AND project_id = $9",
                &[
                    &update.validation_status,
                    &update.validation_errors,
                    &update.review_feedback,
                    &update.queue_code,
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

    /// Remove `05:validator` from `consumer_tags` (Step-2 manual validation,
    /// §3.5.1).
    pub async fn pop_validator_tag(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<(), PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE reborn_recipes
                 SET consumer_tags = array_remove(consumer_tags, '05:validator')
                 WHERE id = $1
                   AND tenant_id = $2 AND user_id = $3
                   AND agent_id  = $4 AND project_id = $5",
                &[&id, &tenant_id, &user_id, &agent_id, &project_id],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    /// Increment outcome counters and recompute Wilson lower bound + tier label.
    ///
    /// To avoid a SELECT-then-UPDATE race while keeping all Wilson arithmetic in
    /// Rust (the Postgres functions do not exist), this method uses a single
    /// atomic `UPDATE … RETURNING` to increment the counters and read back the
    /// new values, then immediately applies a second UPDATE to write the
    /// computed `wilson_lower` and `tier`.  Both statements run in a
    /// transaction so no concurrent reader sees partially-updated rows.
    pub async fn record_outcome(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
        success: bool,
    ) -> Result<(), PgRecipeStoreError> {
        let mut client = self.pool.get().await.map_err(map_pool)?;
        let tx = client.transaction().await.map_err(map_pg)?;

        // Step 1: atomically increment counters, read back new values.
        // Uses query_opt so a missing recipe (wrong id or scope) returns
        // NotFound rather than a confusing RowCount DB error.
        let sql_increment = if success {
            "UPDATE reborn_recipes
             SET usage_count   = usage_count + 1,
                 success_count = success_count + 1
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5
             RETURNING success_count, failure_count"
        } else {
            "UPDATE reborn_recipes
             SET usage_count   = usage_count + 1,
                 failure_count = failure_count + 1
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5
             RETURNING success_count, failure_count"
        };
        let maybe_row = tx
            .query_opt(sql_increment, &[&id, &tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        let row = match maybe_row {
            Some(r) => r,
            None => {
                // Row not found under this scope — rollback and surface as NotFound.
                tx.rollback().await.map_err(map_pg)?;
                return Err(PgRecipeStoreError::NotFound { id: id.to_string() });
            }
        };
        let new_success: i32 = row.get(0);
        let new_failure: i32 = row.get(1);

        // Step 2: compute Wilson lower bound + tier in Rust, write back.
        // Use saturating casts: counters are NON-NULL ≥ 0 by schema, but
        // cast defensively to avoid wrap-around on any hypothetical negative value.
        let w = wilson_lower_bound(new_success.max(0) as u64, new_failure.max(0) as u64, 1.96);
        let tier = classify_tier(
            (new_success.max(0) as u64).saturating_add(new_failure.max(0) as u64),
            w,
            0.80,
        );
        let tier_str = tier_label(tier);
        tx.execute(
            "UPDATE reborn_recipes
             SET wilson_lower = $1, tier = $2
             WHERE id = $3
               AND tenant_id = $4 AND user_id = $5
               AND agent_id  = $6 AND project_id = $7",
            &[&w, &tier_str, &id, &tenant_id, &user_id, &agent_id, &project_id],
        )
        .await
        .map_err(map_pg)?;

        tx.commit().await.map_err(map_pg)?;
        Ok(())
    }

    /// Upsert a recipe by name (idempotent import — used by DocPlan dissector).
    ///
    /// If a row with the same `(scope, name)` already exists AND its
    /// `content_hash` matches, the update is skipped.  Otherwise the row's
    /// trigger, steps, and metadata are replaced and the validation status is
    /// reset to `pending` (re-enters Q1 for re-validation).
    pub async fn upsert(
        &self,
        row: NewPgRecipe,
        content_hash: &str,
    ) -> Result<Uuid, PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let db_row = client
            .query_one(
                "INSERT INTO reborn_recipes
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, trigger, steps,
                     prior_knowledge_content, override_prompt_creation,
                     consumer_tags, intent_examples, source, content_hash,
                     validation_status)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,'pending')
                 ON CONFLICT ON CONSTRAINT reborn_recipes_scope_name_unique DO UPDATE
                     SET description             = EXCLUDED.description,
                         trigger                 = EXCLUDED.trigger,
                         steps                   = EXCLUDED.steps,
                         prior_knowledge_content = EXCLUDED.prior_knowledge_content,
                         override_prompt_creation = EXCLUDED.override_prompt_creation,
                         consumer_tags           = CASE
                             WHEN reborn_recipes.content_hash = $14 THEN reborn_recipes.consumer_tags
                             ELSE EXCLUDED.consumer_tags
                         END,
                         intent_examples         = EXCLUDED.intent_examples,
                         source                  = EXCLUDED.source,
                         content_hash            = EXCLUDED.content_hash,
                         validation_status       = CASE
                             WHEN reborn_recipes.content_hash = $14 THEN reborn_recipes.validation_status
                             ELSE 'pending'
                         END
                 RETURNING id",
                &[
                    &row.tenant_id,
                    &row.user_id,
                    &row.agent_id,
                    &row.project_id,
                    &row.name,
                    &row.description,
                    &row.trigger,
                    &row.steps,
                    &row.prior_knowledge_content,
                    &row.override_prompt_creation,
                    &row.consumer_tags,
                    &row.intent_examples,
                    &row.source,
                    &content_hash,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(db_row.get(0))
    }
}

// ---------------------------------------------------------------------------
// PgRecipeLibrary — RecipeLookup implementation
// ---------------------------------------------------------------------------

/// Postgres-backed [`RecipeLookup`] adapter reading from `reborn_recipes`
/// (class 21).  Replaces the MemoryDoc-backed `RecipeLibrary` for Phase 4+.
#[derive(Clone)]
pub struct PgRecipeLibrary {
    store: PgRecipeStore,
    tenant_id: String,
    user_id: String,
    agent_id: String,
    project_id: String,
}

impl std::fmt::Debug for PgRecipeLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgRecipeLibrary")
            .field("tenant_id", &self.tenant_id)
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}

impl PgRecipeLibrary {
    /// Construct with explicit scope.
    pub fn new(
        pool: Arc<PgPool>,
        tenant_id: impl Into<String>,
        user_id: impl Into<String>,
        agent_id: impl Into<String>,
        project_id: impl Into<String>,
    ) -> Self {
        Self {
            store: PgRecipeStore::new(pool),
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
            agent_id: agent_id.into(),
            project_id: project_id.into(),
        }
    }

    /// Construct with the default local-dev scope used by the existing
    /// `RecipeLibrary` adapter.
    pub fn local_dev(pool: Arc<PgPool>) -> Self {
        Self::new(pool, "local", "default", "default", "default")
    }
}

/// Minimum match score before a Recipe is surfaced (matches `RECIPE_MIN_MATCH`).
const PG_RECIPE_MIN_MATCH: f64 = 0.5;

/// Score a recipe against user input using trigger semantics.
///
/// Returns a score in [0.0, 1.0]:
/// - `Exact`: 1.0 on case-insensitive full match, 0.0 otherwise.
/// - `Keyword`: Jaccard coefficient between tokenized user input and trigger
///   keywords.
/// - `Pattern`: 1.0 on regex match, 0.0 otherwise.
fn score_recipe(recipe: &PgRecipe, user_input: &str) -> f64 {
    let Some(trigger) = &recipe.trigger else {
        return 0.0;
    };
    let trigger_type = trigger.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match trigger_type {
        "exact" => {
            let payload = trigger
                .get("payload")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if payload.to_lowercase() == user_input.to_lowercase() {
                1.0
            } else {
                0.0
            }
        }
        "keyword" => {
            let keywords: Vec<String> = trigger
                .get("payload")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_lowercase())
                        .collect()
                })
                .unwrap_or_default();
            if keywords.is_empty() {
                return 0.0;
            }
            let input_tokens: std::collections::HashSet<String> = user_input
                .split_whitespace()
                .map(|t| t.to_lowercase())
                .collect();
            let kw_set: std::collections::HashSet<String> =
                keywords.into_iter().collect();
            let intersection = input_tokens.intersection(&kw_set).count();
            let union = input_tokens.union(&kw_set).count();
            if union == 0 {
                0.0
            } else {
                intersection as f64 / union as f64
            }
        }
        "pattern" => {
            let pattern = trigger
                .get("payload")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Safety: limit regex size to prevent ReDoS.
            match regex::RegexBuilder::new(pattern)
                .size_limit(10_000)
                .build()
            {
                Ok(re) if re.is_match(user_input) => 1.0,
                _ => 0.0,
            }
        }
        _ => 0.0,
    }
}

/// Extract recipe steps into [`RecipeStepDto`] list.
fn steps_to_dtos(steps: &Value) -> Vec<RecipeStepDto> {
    let Some(arr) = steps.as_array() else {
        return vec![];
    };
    arr.iter()
        .filter_map(|step| {
            let skill_name = step.get("skill").and_then(|v| v.as_str())?;
            let tool = step.get("tool").and_then(|v| v.as_str()).unwrap_or("");
            let params = step
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let description = step
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(RecipeStepDto {
                skill_name: skill_name.to_string(),
                tool: tool.to_string(),
                params,
                description,
            })
        })
        .collect()
}

#[async_trait]
impl RecipeLookup for PgRecipeLibrary {
    async fn find_recipe(
        &self,
        user_input: &str,
    ) -> Result<Option<RecipeMatchDto>, RecipeLookupError> {
        let recipes = self
            .store
            .fetch_validated(
                &self.tenant_id,
                &self.user_id,
                &self.agent_id,
                &self.project_id,
            )
            .await
            .map_err(|e| RecipeLookupError::Backend(e.to_string()))?;

        let mut best: Option<(f64, &PgRecipe)> = None;
        for recipe in &recipes {
            let score = score_recipe(recipe, user_input);
            if score >= PG_RECIPE_MIN_MATCH {
                if best.as_ref().map_or(true, |(best_score, _)| score > *best_score) {
                    best = Some((score, recipe));
                }
            }
        }

        match best {
            None => Ok(None),
            Some((score, recipe)) => {
                debug!(
                    recipe_name = %recipe.name,
                    score,
                    "pg_recipe_library: matched recipe from reborn_recipes"
                );
                Ok(Some(RecipeMatchDto {
                    id: recipe.id.to_string(),
                    name: recipe.name.clone(),
                    tier: recipe.tier.clone(),
                    wilson_lower: recipe.wilson_lower,
                    tier0_eligible: recipe.is_tier0_eligible(),
                    validation_kind: "validated".to_string(),
                    steps: steps_to_dtos(&recipe.steps),
                    match_score: score,
                }))
            }
        }
    }

    async fn find_skills(
        &self,
        _user_input: &str,
    ) -> Result<Vec<ToolSkillMatchDto>, RecipeLookupError> {
        // Phase 4: ToolSkills have their own table (class 13, V037 in Phase 5).
        // Until that migration runs, skills are surfaced through the existing
        // MemoryDoc-backed path.  Return empty here so callers can compose both
        // adapters if needed.
        Ok(vec![])
    }

    async fn record_recipe_outcome(
        &self,
        recipe_id: &str,
        success: bool,
    ) -> Result<(), RecipeLookupError> {
        let id: Uuid = recipe_id
            .parse()
            .map_err(|e| RecipeLookupError::Decode(format!("invalid recipe_id UUID: {e}")))?;
        self.store
            .record_outcome(
                &self.tenant_id,
                &self.user_id,
                &self.agent_id,
                &self.project_id,
                id,
                success,
            )
            .await
            .map_err(|e| RecipeLookupError::Backend(e.to_string()))?;
        debug!(recipe_id, success, "pg_recipe_library: recipe outcome recorded");
        Ok(())
    }

    async fn record_skill_outcome(
        &self,
        _skill_id: &str,
        _success: bool,
    ) -> Result<(), RecipeLookupError> {
        // ToolSkill outcomes recorded by the Phase-5 PgToolSkillStore.
        Ok(())
    }
}
