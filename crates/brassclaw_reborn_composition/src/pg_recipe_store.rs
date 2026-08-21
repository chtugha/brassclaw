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

// Phase-5 postgres wiring — items unused until factory wiring lands.
#![allow(dead_code)]
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

/// Hard cap on how many recipes `list_all` / `fetch_validated` may return in a
/// single call.  Guards against accidental full-table scans on large tenants.
const MAX_RECIPE_LIST_ROWS: i64 = 1_000;
/// Consumer tag that marks a recipe as being evaluated by the validator;
/// delivery filter excludes rows carrying this tag (SEC-01, §3.9).
const VALIDATOR_CONSUMER_TAG: &str = "05:validator";
/// Maximum regex size in bytes accepted by the pattern-match scorer.
/// Guards against ReDoS via pathological patterns.
const PATTERN_SCORER_REGEX_SIZE_LIMIT: usize = 10_000;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by `reborn_recipes` store operations.
#[derive(Debug, Error)]
pub(crate) enum PgRecipeStoreError {
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
pub(crate) struct PgRecipe {
    pub(crate) id: Uuid,
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    /// Trigger JSON — `{type: "exact"|"pattern"|"keyword", payload: ...}`.
    pub(crate) trigger: Option<Value>,
    /// Ordered step array — `[{skill: name, params: {...}}]`.
    pub(crate) steps: Value,
    pub(crate) status: String,
    pub(crate) prior_knowledge_content: Option<String>,
    pub(crate) override_prompt_creation: bool,
    pub(crate) class_code: i16,
    pub(crate) prompt_uid: i64,
    pub(crate) consumer_tags: Vec<String>,
    pub(crate) intent_examples: Option<Value>,
    pub(crate) tier: String,
    pub(crate) usage_count: i32,
    pub(crate) success_count: i32,
    pub(crate) failure_count: i32,
    pub(crate) wilson_lower: f64,
    pub(crate) validation_status: String,
    pub(crate) validation_errors: Vec<String>,
    pub(crate) review_feedback: Option<String>,
    pub(crate) review_attempts: i16,
    pub(crate) rejected_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) queue_code: Option<String>,
    pub(crate) source: String,
    pub(crate) content_hash: Option<String>,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
    pub(crate) updated_at: chrono::DateTime<chrono::Utc>,

    // v3 authoring model (Phase A / V050). NULLable JSONB → Option so legacy
    // rows (NULL) decode without a runtime NULL-into-non-Option panic
    // (FIND-IBS-07: the plan's "Files to modify" text said non-Option
    // `serde_json::Value`, but V050's columns are NULLable, so Option is
    // required for correctness).
    pub(crate) step_descriptions: Option<Value>,
    pub(crate) variants: Option<Value>,
    pub(crate) dependency_registry: Option<Value>,
}

impl PgRecipe {
    /// Returns true iff the row carries the `05:validator` consumer tag.
    pub(crate) fn has_validator_tag(&self) -> bool {
        self.consumer_tags.iter().any(|t| t == "05:validator")
    }

    /// Returns true iff this recipe is deliverable to consumers (validated + no
    /// validator tag, per §3.9 SEC-01).
    pub(crate) fn is_deliverable(&self) -> bool {
        self.validation_status == "validated" && !self.has_validator_tag()
    }

    /// Returns true iff this recipe is eligible for Tier-0 direct execution.
    ///
    /// Requires the same `wilson_lower >= 0.70` guard that the engine-domain
    /// [`brassclaw_engine::types::recipe::Recipe::is_tier0_eligible`] applies,
    /// so a `mature`/`candidate` row that has never been used (wilson 0.0) is
    /// never silently escalated to Tier 0 (FIND-P7-11 / FIND-05).
    pub(crate) fn is_tier0_eligible(&self) -> bool {
        self.is_deliverable()
            && matches!(self.tier.as_str(), "mature" | "candidate")
            && self.wilson_lower >= 0.70
    }
}

/// Minimal data required to insert a new recipe row.
#[derive(Debug, Clone)]
pub(crate) struct NewPgRecipe {
    pub(crate) tenant_id: String,
    pub(crate) user_id: String,
    pub(crate) agent_id: String,
    pub(crate) project_id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) trigger: Option<Value>,
    pub(crate) steps: Value,
    pub(crate) prior_knowledge_content: Option<String>,
    pub(crate) override_prompt_creation: bool,
    /// Consumer tags — caller must include `05:validator` for new rows.
    pub(crate) consumer_tags: Vec<String>,
    pub(crate) intent_examples: Option<Value>,
    pub(crate) source: String,
    // v3 authoring model (Phase A / V050). None = leave column NULL.
    pub(crate) step_descriptions: Option<Value>,
    pub(crate) variants: Option<Value>,
    pub(crate) dependency_registry: Option<Value>,
}

// ---------------------------------------------------------------------------
// Parameter structs (keep argument counts under the clippy threshold)
// ---------------------------------------------------------------------------

/// Grouped parameters for [`PgRecipeStore::update_validation_status`].
#[derive(Debug)]
pub(crate) struct RecipeValidationStatusUpdate<'a> {
    pub(crate) validation_status: &'a str,
    pub(crate) validation_errors: Vec<String>,
    pub(crate) review_feedback: Option<String>,
    pub(crate) queue_code: Option<String>,
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
pub(crate) struct PgRecipeStore {
    pool: Arc<PgPool>,
}

impl PgRecipeStore {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
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
    created_at, updated_at,
    step_descriptions, variants, dependency_registry
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
        step_descriptions: row.get(31),
        variants: row.get(32),
        dependency_registry: row.get(33),
    })
}

impl PgRecipeStore {
    /// Insert a new recipe.  Returns the assigned UUID.
    pub(crate) async fn insert(&self, row: NewPgRecipe) -> Result<Uuid, PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let db_row = client
            .query_one(
                "INSERT INTO reborn_recipes
                    (tenant_id, user_id, agent_id, project_id,
                     name, description, trigger, steps,
                     prior_knowledge_content, override_prompt_creation,
                     consumer_tags, intent_examples, source,
                     step_descriptions, variants, dependency_registry)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
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
                    &row.step_descriptions,
                    &row.variants,
                    &row.dependency_registry,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(db_row.get(0))
    }

    /// Fetch a single recipe by id + scope.
    pub(crate) async fn get(
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
    pub(crate) async fn get_by_name(
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
    pub(crate) async fn list_all(
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
             ORDER BY prompt_uid ASC
             LIMIT {MAX_RECIPE_LIST_ROWS}"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_recipe_row).collect()
    }

    /// List deliverable recipes for a consumer (§3.9 SEC-01 delivery filter).
    pub(crate) async fn fetch_validated(
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
             ORDER BY prompt_uid ASC
             LIMIT {MAX_RECIPE_LIST_ROWS}"
        );
        let rows = client
            .query(&q, &[&tenant_id, &user_id, &agent_id, &project_id])
            .await
            .map_err(map_pg)?;
        rows.iter().map(decode_recipe_row).collect()
    }

    /// Update validation status + queue code.
    pub(crate) async fn update_validation_status(
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
    ///
    /// Returns `Err(PgRecipeStoreError::NotFound)` if no row matched the full
    /// scope tuple — guards against silent no-op on scope mismatch.
    pub(crate) async fn pop_validator_tag(
        &self,
        tenant_id: &str,
        user_id: &str,
        agent_id: &str,
        project_id: &str,
        id: Uuid,
    ) -> Result<(), PgRecipeStoreError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let sql = format!(
            "UPDATE reborn_recipes
             SET consumer_tags = array_remove(consumer_tags, '{VALIDATOR_CONSUMER_TAG}')
             WHERE id = $1
               AND tenant_id = $2 AND user_id = $3
               AND agent_id  = $4 AND project_id = $5"
        );
        let affected = client
            .execute(
                sql.as_str(),
                &[&id, &tenant_id, &user_id, &agent_id, &project_id],
            )
            .await
            .map_err(map_pg)?;
        if affected == 0 {
            return Err(PgRecipeStoreError::NotFound { id: id.to_string() });
        }
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
    pub(crate) async fn record_outcome(
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
            .query_opt(
                sql_increment,
                &[&id, &tenant_id, &user_id, &agent_id, &project_id],
            )
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
        let w = wilson_lower_bound(
            new_success.max(0) as u64,
            new_failure.max(0) as u64,
            crate::plan_library::DEFAULT_WILSON_Z,
        );
        let tier = classify_tier(
            (new_success.max(0) as u64).saturating_add(new_failure.max(0) as u64),
            w,
            crate::plan_library::DEFAULT_PROMOTION_THRESHOLD,
        );
        let tier_str = tier_label(tier);
        tx.execute(
            "UPDATE reborn_recipes
             SET wilson_lower = $1, tier = $2
             WHERE id = $3
               AND tenant_id = $4 AND user_id = $5
               AND agent_id  = $6 AND project_id = $7",
            &[
                &w,
                &tier_str,
                &id,
                &tenant_id,
                &user_id,
                &agent_id,
                &project_id,
            ],
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
    pub(crate) async fn upsert(
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
                     step_descriptions, variants, dependency_registry,
                     validation_status)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,'pending')
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
                         step_descriptions       = EXCLUDED.step_descriptions,
                         variants                = EXCLUDED.variants,
                         dependency_registry     = EXCLUDED.dependency_registry,
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
                    &row.step_descriptions,
                    &row.variants,
                    &row.dependency_registry,
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
pub(crate) struct PgRecipeLibrary {
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
    pub(crate) fn new(
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
    pub(crate) fn local_dev(pool: Arc<PgPool>) -> Self {
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
            let kw_set: std::collections::HashSet<String> = keywords.into_iter().collect();
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
                .size_limit(PATTERN_SCORER_REGEX_SIZE_LIMIT)
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
            if score >= PG_RECIPE_MIN_MATCH
                && best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, recipe));
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
        debug!(
            recipe_id,
            success, "pg_recipe_library: recipe outcome recorded"
        );
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

// ---------------------------------------------------------------------------
// PgRecipeStoreFacade — implements brassclaw_product_workflow::RecipeStore
// ---------------------------------------------------------------------------
//
// The trait methods use `(user_id, project_id)` as scope.  Production wiring
// pins `tenant_id = "default"` and `agent_id = "default"` to match the
// existing local-dev scope used by RecipeLibrary / PgRecipeLibrary.  Custom
// scopes construct PgRecipeStore directly.

/// Scoped facade over [`PgRecipeStore`] that implements the
/// `brassclaw_product_workflow::RecipeStore` trait.
///
/// All trait methods use `(user_id, project_id)` from the call site;
/// `tenant_id` and `agent_id` are fixed at construction time.
#[cfg(feature = "postgres")]
pub(crate) struct PgRecipeStoreFacade {
    inner: PgRecipeStore,
    tenant_id: String,
    agent_id: String,
}

#[cfg(feature = "postgres")]
impl PgRecipeStoreFacade {
    /// Create a facade with explicit tenant + agent.
    pub(crate) fn new(
        pool: Arc<PgPool>,
        tenant_id: impl Into<String>,
        agent_id: impl Into<String>,
    ) -> Self {
        Self {
            inner: PgRecipeStore::new(pool),
            tenant_id: tenant_id.into(),
            agent_id: agent_id.into(),
        }
    }

    /// Convenience constructor matching the local-dev defaults used by
    /// `PgRecipeLibrary::local_dev`.
    pub(crate) fn local_dev(pool: Arc<PgPool>) -> Self {
        Self::new(pool, "local", "default")
    }
}

#[cfg(feature = "postgres")]
fn map_pg_recipe_error(e: PgRecipeStoreError) -> brassclaw_product_workflow::RecipeStoreError {
    match e {
        PgRecipeStoreError::NotFound { id } => {
            brassclaw_product_workflow::RecipeStoreError::NotFound(id)
        }
        PgRecipeStoreError::Invalid { reason } => {
            brassclaw_product_workflow::RecipeStoreError::Invalid(reason)
        }
        PgRecipeStoreError::Pool { reason } | PgRecipeStoreError::Db { reason } => {
            brassclaw_product_workflow::RecipeStoreError::Unavailable(reason)
        }
        PgRecipeStoreError::Serialize { reason } => {
            brassclaw_product_workflow::RecipeStoreError::Internal(reason)
        }
    }
}

#[cfg(feature = "postgres")]
fn recipe_to_summary(r: &PgRecipe) -> brassclaw_product_workflow::RecipeSummary {
    let step_count = r.steps.as_array().map(|a| a.len() as u32).unwrap_or(0);
    brassclaw_product_workflow::RecipeSummary {
        id: r.id.to_string(),
        name: r.name.clone(),
        description: r.description.clone(),
        category: "recipe".to_string(),
        trigger: r.trigger.clone().unwrap_or(serde_json::Value::Null),
        step_count,
        usage_count: r.usage_count.max(0) as u64,
        success_count: r.success_count.max(0) as u64,
        failure_count: r.failure_count.max(0) as u64,
        wilson_lower: r.wilson_lower,
        tier: r.tier.clone(),
        tier0_eligible: r.is_tier0_eligible(),
        validation_status: r.validation_status.clone(),
        validation_errors: r.validation_errors.clone(),
        review_attempts: r.review_attempts.max(0) as u32,
        source: r.source.clone(),
        created_at: r.created_at.to_rfc3339(),
        updated_at: r.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "postgres")]
fn recipe_to_detail(r: &PgRecipe) -> brassclaw_product_workflow::RecipeDetail {
    let recipe_json = serde_json::json!({
        "id": r.id.to_string(),
        "name": r.name,
        "description": r.description,
        "trigger": r.trigger,
        "steps": r.steps,
        "status": r.status,
        "prior_knowledge_content": r.prior_knowledge_content,
        "override_prompt_creation": r.override_prompt_creation,
        "class_code": r.class_code,
        "prompt_uid": r.prompt_uid,
        "consumer_tags": r.consumer_tags,
        "intent_examples": r.intent_examples,
        "tier": r.tier,
        "usage_count": r.usage_count,
        "success_count": r.success_count,
        "failure_count": r.failure_count,
        "wilson_lower": r.wilson_lower,
        "validation_status": r.validation_status,
        "validation_errors": r.validation_errors,
        "review_feedback": r.review_feedback,
        "review_attempts": r.review_attempts,
        "rejected_at": r.rejected_at.map(|t| t.to_rfc3339()),
        "queue_code": r.queue_code,
        "source": r.source,
        "content_hash": r.content_hash,
        "created_at": r.created_at.to_rfc3339(),
        "updated_at": r.updated_at.to_rfc3339(),
    });
    brassclaw_product_workflow::RecipeDetail {
        id: r.id.to_string(),
        recipe: recipe_json,
    }
}

#[cfg(feature = "postgres")]
fn recipe_to_queue_item(r: &PgRecipe) -> brassclaw_product_workflow::ValidationQueueItem {
    let queue_code = r.queue_code.clone().unwrap_or_else(|| {
        // Derive queue code from validation_status when not stored.
        match r.validation_status.as_str() {
            "pending" | "auto_failed" => "q1_auto".to_string(),
            "auto_passed" | "review_requested" | "upgrade_queued" => "q2_manual".to_string(),
            "rejected" if r.review_attempts < 3 => "q3_revision".to_string(),
            "rejected" => "q4_rejection".to_string(),
            "garbage" => "garbage".to_string(),
            _ => "q2_manual".to_string(),
        }
    });
    let trigger_summary = r
        .trigger
        .as_ref()
        .and_then(|t| t.get("payload"))
        .and_then(|p| {
            if let Some(s) = p.as_str() {
                Some(s.to_string())
            } else {
                serde_json::to_string(p).ok()
            }
        })
        .unwrap_or_default();
    brassclaw_product_workflow::ValidationQueueItem {
        id: r.id.to_string(),
        name: r.name.clone(),
        item_type: brassclaw_product_workflow::RecipeKind::Recipe,
        category: "recipe".to_string(),
        description: r.description.clone(),
        trigger_summary,
        estimated_tokens: None,
        validation_status: r.validation_status.clone(),
        validation_errors: r.validation_errors.clone(),
        review_feedback: r.review_feedback.clone(),
        review_attempts: r.review_attempts.max(0) as u32,
        similarity_parent_id: None,
        created_at: r.created_at.to_rfc3339(),
        source: r.source.clone(),
        class_code: 21,
        class_label: "RECIPE".to_string(),
        queue_code,
        validator_tag_present: r.has_validator_tag(),
        consumer_tags: r.consumer_tags.clone(),
        // reborn_recipes has no llm_audit_status column — always not_applicable.
        llm_audit_status: "not_applicable".to_string(),
        llm_audit_findings: vec![],
    }
}

/// Map ValidationQueueFilter → the set of validation_status values to query.
#[cfg(feature = "postgres")]
fn queue_filter_statuses(
    filter: brassclaw_product_workflow::ValidationQueueFilter,
) -> &'static [&'static str] {
    match filter {
        brassclaw_product_workflow::ValidationQueueFilter::Auto => &["pending", "auto_failed"],
        brassclaw_product_workflow::ValidationQueueFilter::Manual => {
            &["auto_passed", "review_requested", "upgrade_queued"]
        }
        brassclaw_product_workflow::ValidationQueueFilter::Revision => &["rejected"],
        brassclaw_product_workflow::ValidationQueueFilter::Rejection => &["rejected"],
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl brassclaw_product_workflow::RecipeStore for PgRecipeStoreFacade {
    async fn list_recipes(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<
        Vec<brassclaw_product_workflow::RecipeSummary>,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        let rows = self
            .inner
            .list_all(&self.tenant_id, user_id, &self.agent_id, project_id)
            .await
            .map_err(map_pg_recipe_error)?;
        Ok(rows.iter().map(recipe_to_summary).collect())
    }

    async fn list_tool_skills(
        &self,
        _user_id: &str,
        _project_id: &str,
    ) -> Result<
        Vec<brassclaw_product_workflow::ToolSkillSummary>,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        // ToolSkills have their own table (V037, Phase 5).  Until that migration
        // lands, return empty so the WebUI shows no tool skills rather than erroring.
        tracing::debug!(
            "PgRecipeStoreFacade::list_tool_skills: V037 not yet applied — returning empty"
        );
        Ok(vec![])
    }

    async fn get_recipe(
        &self,
        user_id: &str,
        project_id: &str,
        recipe_id: &str,
    ) -> Result<
        Option<brassclaw_product_workflow::RecipeDetail>,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        let uuid: uuid::Uuid = recipe_id.parse().map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Invalid(format!(
                "invalid recipe_id UUID: {e}"
            ))
        })?;
        let row = self
            .inner
            .get(&self.tenant_id, user_id, &self.agent_id, project_id, uuid)
            .await
            .map_err(map_pg_recipe_error)?;
        Ok(row.as_ref().map(recipe_to_detail))
    }

    async fn get_tool_skill(
        &self,
        _user_id: &str,
        _project_id: &str,
        _skill_id: &str,
    ) -> Result<
        Option<brassclaw_product_workflow::ToolSkillDetail>,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        // V037 not yet applied.
        tracing::debug!(
            "PgRecipeStoreFacade::get_tool_skill: V037 not yet applied — returning None"
        );
        Ok(None)
    }

    async fn list_validation_queue(
        &self,
        user_id: &str,
        project_id: &str,
        filter: brassclaw_product_workflow::ValidationQueueFilter,
    ) -> Result<
        Vec<brassclaw_product_workflow::ValidationQueueItem>,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        let statuses = queue_filter_statuses(filter);
        let client = self.inner.pool.get().await.map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
        })?;

        // Build a query with a dynamic IN clause.
        let placeholders: String = statuses
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 5))
            .collect::<Vec<_>>()
            .join(", ");
        let q = format!(
            "SELECT {RECIPE_SELECT} FROM reborn_recipes
             WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
               AND validation_status IN ({placeholders})
             ORDER BY created_at ASC
             LIMIT {MAX_RECIPE_LIST_ROWS}"
        );

        // Build params: 4 scope values + dynamic status values.
        let tenant_id = self.tenant_id.as_str();
        let agent_id = self.agent_id.as_str();
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            vec![&tenant_id, &user_id, &agent_id, &project_id];
        for s in statuses.iter() {
            params.push(s);
        }

        let rows = client.query(q.as_str(), &params).await.map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
        })?;

        let mut items: Vec<brassclaw_product_workflow::ValidationQueueItem> =
            Vec::with_capacity(rows.len());
        for row in &rows {
            let recipe = decode_recipe_row(row).map_err(|e| {
                brassclaw_product_workflow::RecipeStoreError::Internal(e.to_string())
            })?;
            // For Rejection filter: only include rows with review_attempts >= 3.
            // For Revision filter: only include rows with review_attempts < 3.
            let include = match filter {
                brassclaw_product_workflow::ValidationQueueFilter::Rejection => {
                    recipe.review_attempts >= 3
                }
                brassclaw_product_workflow::ValidationQueueFilter::Revision => {
                    recipe.review_attempts < 3
                }
                _ => true,
            };
            if include {
                items.push(recipe_to_queue_item(&recipe));
            }
        }
        Ok(items)
    }

    async fn count_by_status(
        &self,
        user_id: &str,
        project_id: &str,
        status: &str,
    ) -> Result<u32, brassclaw_product_workflow::RecipeStoreError> {
        let client = self.inner.pool.get().await.map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
        })?;
        let row = client
            .query_one(
                "SELECT COUNT(*) FROM reborn_recipes
                 WHERE tenant_id = $1 AND user_id = $2 AND agent_id = $3 AND project_id = $4
                   AND validation_status = $5",
                &[
                    &self.tenant_id.as_str(),
                    &user_id,
                    &self.agent_id.as_str(),
                    &project_id,
                    &status,
                ],
            )
            .await
            .map_err(|e| {
                brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
            })?;
        let count: i64 = row.get(0);
        Ok(count.max(0) as u32)
    }

    async fn update_recipe_validation_status(
        &self,
        user_id: &str,
        project_id: &str,
        recipe_id: &str,
        new_status: &str,
        feedback: Option<&str>,
    ) -> Result<
        brassclaw_product_workflow::UpdateValidationStatusResponse,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        let uuid: uuid::Uuid = recipe_id.parse().map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Invalid(format!(
                "invalid recipe_id UUID: {e}"
            ))
        })?;
        // Fetch current status for the response.
        let current = self
            .inner
            .get(&self.tenant_id, user_id, &self.agent_id, project_id, uuid)
            .await
            .map_err(map_pg_recipe_error)?
            .ok_or_else(|| {
                brassclaw_product_workflow::RecipeStoreError::NotFound(recipe_id.to_string())
            })?;
        let previous_status = current.validation_status.clone();
        let new_review_attempts = if new_status == "rejected" {
            current.review_attempts + 1
        } else {
            current.review_attempts
        };
        let queue_code = derive_queue_code(new_status, new_review_attempts);
        self.inner
            .update_validation_status(
                &self.tenant_id,
                user_id,
                &self.agent_id,
                project_id,
                uuid,
                RecipeValidationStatusUpdate {
                    validation_status: new_status,
                    validation_errors: vec![],
                    review_feedback: feedback.map(|s| s.to_string()),
                    queue_code: Some(queue_code),
                },
            )
            .await
            .map_err(map_pg_recipe_error)?;
        // Pop validator tag when moving to validated.
        if new_status == "validated" {
            let _ = self
                .inner
                .pop_validator_tag(&self.tenant_id, user_id, &self.agent_id, project_id, uuid)
                .await;
        }
        Ok(brassclaw_product_workflow::UpdateValidationStatusResponse {
            id: recipe_id.to_string(),
            item_type: brassclaw_product_workflow::RecipeKind::Recipe,
            previous_status,
            new_status: new_status.to_string(),
            review_attempts: new_review_attempts.max(0) as u32,
        })
    }

    async fn update_skill_validation_status(
        &self,
        _user_id: &str,
        _project_id: &str,
        skill_id: &str,
        _new_status: &str,
        _feedback: Option<&str>,
    ) -> Result<
        brassclaw_product_workflow::UpdateValidationStatusResponse,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        // V037 not yet applied — no-op.
        tracing::debug!(
            "PgRecipeStoreFacade::update_skill_validation_status: V037 not yet applied"
        );
        Ok(brassclaw_product_workflow::UpdateValidationStatusResponse {
            id: skill_id.to_string(),
            item_type: brassclaw_product_workflow::RecipeKind::ToolSkill,
            previous_status: "unknown".to_string(),
            new_status: "unknown".to_string(),
            review_attempts: 0,
        })
    }

    async fn update_component_validation_status(
        &self,
        user_id: &str,
        project_id: &str,
        class_code: u16,
        component_id: &str,
        new_status: &str,
        feedback: Option<&str>,
    ) -> Result<
        brassclaw_product_workflow::UpdateValidationStatusResponse,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        match class_code {
            21 => {
                self.update_recipe_validation_status(
                    user_id,
                    project_id,
                    component_id,
                    new_status,
                    feedback,
                )
                .await
            }
            13 => {
                self.update_skill_validation_status(
                    user_id,
                    project_id,
                    component_id,
                    new_status,
                    feedback,
                )
                .await
            }
            _ => {
                tracing::debug!(
                    class_code,
                    component_id,
                    "update_component_validation_status: unsupported class_code"
                );
                Err(brassclaw_product_workflow::RecipeStoreError::Invalid(
                    format!("class_code {class_code} is not handled by PgRecipeStoreFacade"),
                ))
            }
        }
    }

    async fn re_review_component(
        &self,
        user_id: &str,
        project_id: &str,
        class_code: u16,
        component_id: &str,
        feedback: Option<&str>,
    ) -> Result<
        brassclaw_product_workflow::UpdateValidationStatusResponse,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        if class_code != 21 {
            return Err(brassclaw_product_workflow::RecipeStoreError::Invalid(
                format!("re_review_component: class_code {class_code} not handled"),
            ));
        }
        let uuid: uuid::Uuid = component_id.parse().map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Invalid(format!(
                "invalid component_id UUID: {e}"
            ))
        })?;
        let current = self
            .inner
            .get(&self.tenant_id, user_id, &self.agent_id, project_id, uuid)
            .await
            .map_err(map_pg_recipe_error)?
            .ok_or_else(|| {
                brassclaw_product_workflow::RecipeStoreError::NotFound(component_id.to_string())
            })?;
        // Only allow re-review from Q4 (rejected, review_attempts >= 3).
        if current.validation_status != "rejected" || current.review_attempts < 3 {
            return Err(brassclaw_product_workflow::RecipeStoreError::Invalid(
                "re_review_component requires the component to be in Q4 (rejected, review_attempts >= 3)".to_string(),
            ));
        }
        let previous_status = current.validation_status.clone();
        self.inner
            .update_validation_status(
                &self.tenant_id,
                user_id,
                &self.agent_id,
                project_id,
                uuid,
                RecipeValidationStatusUpdate {
                    validation_status: "pending",
                    validation_errors: vec![],
                    review_feedback: feedback.map(|s| s.to_string()),
                    queue_code: Some("q1_auto".to_string()),
                },
            )
            .await
            .map_err(map_pg_recipe_error)?;
        // Re-add validator tag so it re-enters Q1 auto-validation.
        let client = self.inner.pool.get().await.map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
        })?;
        client
            .execute(
                "UPDATE reborn_recipes
                 SET consumer_tags = array_append(
                     array_remove(consumer_tags, '05:validator'), '05:validator'
                 )
                 WHERE id = $1 AND tenant_id = $2 AND user_id = $3
                   AND agent_id = $4 AND project_id = $5",
                &[
                    &uuid,
                    &self.tenant_id.as_str(),
                    &user_id,
                    &self.agent_id.as_str(),
                    &project_id,
                ],
            )
            .await
            .map_err(|e| {
                brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
            })?;
        Ok(brassclaw_product_workflow::UpdateValidationStatusResponse {
            id: component_id.to_string(),
            item_type: brassclaw_product_workflow::RecipeKind::Recipe,
            previous_status,
            new_status: "pending".to_string(),
            review_attempts: current.review_attempts.max(0) as u32,
        })
    }

    async fn delete_component(
        &self,
        user_id: &str,
        project_id: &str,
        class_code: u16,
        component_id: &str,
    ) -> Result<(), brassclaw_product_workflow::RecipeStoreError> {
        if class_code != 21 {
            return Err(brassclaw_product_workflow::RecipeStoreError::Invalid(
                format!("delete_component: class_code {class_code} not handled"),
            ));
        }
        let uuid: uuid::Uuid = component_id.parse().map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Invalid(format!(
                "invalid component_id UUID: {e}"
            ))
        })?;
        let client = self.inner.pool.get().await.map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
        })?;
        // Mark as garbage — terminal state; sweep task handles physical removal.
        let affected = client
            .execute(
                "UPDATE reborn_recipes
                 SET validation_status = 'garbage', queue_code = 'garbage'
                 WHERE id = $1 AND tenant_id = $2 AND user_id = $3
                   AND agent_id = $4 AND project_id = $5",
                &[
                    &uuid,
                    &self.tenant_id.as_str(),
                    &user_id,
                    &self.agent_id.as_str(),
                    &project_id,
                ],
            )
            .await
            .map_err(|e| {
                brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
            })?;
        if affected == 0 {
            return Err(brassclaw_product_workflow::RecipeStoreError::NotFound(
                component_id.to_string(),
            ));
        }
        Ok(())
    }

    async fn get_component_audit_status(
        &self,
        _user_id: &str,
        _project_id: &str,
        _class_code: u16,
        _component_id: &str,
    ) -> Result<
        brassclaw_product_workflow::ComponentAuditStatus,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        // reborn_recipes has no llm_audit_status column — always not_applicable.
        Ok(brassclaw_product_workflow::ComponentAuditStatus::not_applicable())
    }

    async fn record_outcome(
        &self,
        user_id: &str,
        project_id: &str,
        request: brassclaw_product_workflow::RecordOutcomeRequest,
    ) -> Result<
        brassclaw_product_workflow::RecordOutcomeResponse,
        brassclaw_product_workflow::RecipeStoreError,
    > {
        match request.kind {
            brassclaw_product_workflow::OutcomeKind::Recipe => {
                let uuid: uuid::Uuid = request.id.parse().map_err(|e| {
                    brassclaw_product_workflow::RecipeStoreError::Invalid(format!(
                        "invalid recipe_id UUID: {e}"
                    ))
                })?;
                self.inner
                    .record_outcome(
                        &self.tenant_id,
                        user_id,
                        &self.agent_id,
                        project_id,
                        uuid,
                        request.success,
                    )
                    .await
                    .map_err(map_pg_recipe_error)?;
                Ok(brassclaw_product_workflow::RecordOutcomeResponse {
                    id: request.id,
                    kind: request.kind,
                    recorded: true,
                })
            }
            brassclaw_product_workflow::OutcomeKind::ToolSkill => {
                // V037 not yet applied.
                tracing::debug!(
                    "PgRecipeStoreFacade::record_outcome(ToolSkill): V037 not yet applied"
                );
                Ok(brassclaw_product_workflow::RecordOutcomeResponse {
                    id: request.id,
                    kind: request.kind,
                    recorded: false,
                })
            }
        }
    }

    /// Q1 auto-validation sweep for `reborn_recipes` rows.
    ///
    /// Fetches all rows with `validation_status = 'pending'` and
    /// `queue_code = 'q1_auto'` for the given `(user_id, project_id)` scope,
    /// runs [`brassclaw_engine::memory::ComponentValidator::validate_by_class`]
    /// against each, then writes either `auto_passed` or `auto_failed` back.
    ///
    /// Available Rusty tools are fetched once per call via the
    /// `reborn_tools` table (same scope tuple).  An empty tool registry is a
    /// valid transient state — ToolSkill validation still runs; the
    /// `tool_name` cross-reference check is skipped when the registry is
    /// empty (same contract as passing `&[]` to `validate_by_class`).
    ///
    /// **Feature gate:** only compiled when both `postgres` and `skills-db`
    /// features are active — the `DbToolSource` type lives behind `skills-db`.
    #[cfg(feature = "skills-db")]
    async fn auto_validate_pending(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<u32, brassclaw_product_workflow::RecipeStoreError> {
        use brassclaw_capabilities::tool_registry::{ToolRegistryStore, ToolScopeKey};
        use brassclaw_engine::capability::DbToolSource;
        use brassclaw_engine::memory::{
            ComponentPayload, ComponentValidator, GenericComponent, ValidationConfig,
        };

        // ── 1. Fetch available tool names for this scope ──────────────────
        let tool_scope = ToolScopeKey {
            tenant_id: self.tenant_id.clone(),
            user_id: user_id.to_string(),
            agent_id: self.agent_id.clone(),
            project_id: project_id.to_string(),
        };
        let tool_source = DbToolSource::new((*self.inner.pool).clone());
        let available_tools = tool_source
            .fetch_tool_names(&tool_scope)
            .await
            .map_err(|e| {
                brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
            })?;

        // ── 2. Fetch all pending rows in q1_auto ──────────────────────────
        let client = self.inner.pool.get().await.map_err(|e| {
            brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
        })?;

        let rows = client
            .query(
                "SELECT id, name, description, class_code, steps
                 FROM reborn_recipes
                 WHERE tenant_id  = $1
                   AND user_id    = $2
                   AND agent_id   = $3
                   AND project_id = $4
                   AND validation_status = 'pending'
                   AND (queue_code = 'q1_auto' OR queue_code IS NULL)
                 ORDER BY created_at ASC
                 LIMIT 500",
                &[
                    &self.tenant_id.as_str(),
                    &user_id,
                    &self.agent_id.as_str(),
                    &project_id,
                ],
            )
            .await
            .map_err(|e| {
                brassclaw_product_workflow::RecipeStoreError::Unavailable(e.to_string())
            })?;

        let mut processed: u32 = 0;

        for row in &rows {
            let id: uuid::Uuid = row.get(0);
            let name: String = row.get(1);
            let description: String = row.get(2);
            let class_code_raw: i16 = row.get(3);
            let steps_json: serde_json::Value = row.get(4);
            let class_code = class_code_raw.max(0) as u16;

            // Build a generic payload from name + description + steps content.
            let steps_str = serde_json::to_string(&steps_json).unwrap_or_default();
            let content_combined = format!("{description}\n{steps_str}");
            let component = ComponentPayload::Generic(GenericComponent {
                name: &name,
                description: &description,
                content: &content_combined,
                extra: None,
            });

            let result = ComponentValidator::validate_by_class(
                class_code,
                component,
                &ValidationConfig::default(),
                &available_tools,
                &[],
            );

            let (new_status, new_queue_code, errors) = if result.errors.is_empty() {
                ("auto_passed", "q2_manual", vec![])
            } else {
                ("auto_failed", "q1_auto", result.errors)
            };

            // ── 3. Write result back ──────────────────────────────────────
            let update_result = client
                .execute(
                    "UPDATE reborn_recipes
                     SET validation_status = $1,
                         queue_code        = $2,
                         validation_errors = $3,
                         updated_at        = NOW()
                     WHERE id         = $4
                       AND tenant_id  = $5
                       AND user_id    = $6
                       AND agent_id   = $7
                       AND project_id = $8
                       AND validation_status = 'pending'",
                    &[
                        &new_status,
                        &new_queue_code,
                        &errors,
                        &id,
                        &self.tenant_id.as_str(),
                        &user_id,
                        &self.agent_id.as_str(),
                        &project_id,
                    ],
                )
                .await;

            match update_result {
                Ok(n) if n > 0 => {
                    processed += 1;
                    debug!(
                        recipe_id = %id,
                        class_code,
                        new_status,
                        "q1_auto_validate: processed component"
                    );
                }
                Ok(_) => {
                    // Row was updated concurrently — skip silently.
                    debug!(recipe_id = %id, "q1_auto_validate: row already updated, skipping");
                }
                Err(e) => {
                    debug!(
                        recipe_id = %id,
                        error = %e,
                        "q1_auto_validate: DB update failed, skipping row"
                    );
                }
            }
        }

        debug!(
            processed,
            user_id, project_id, "q1_auto_validate: sweep complete"
        );
        Ok(processed)
    }
}

/// Derive a queue_code string from new_status + review_attempts.
#[cfg(feature = "postgres")]
fn derive_queue_code(new_status: &str, review_attempts: i16) -> String {
    match new_status {
        "pending" | "auto_failed" => "q1_auto".to_string(),
        "auto_passed" | "review_requested" | "upgrade_queued" | "validated" => {
            "q2_manual".to_string()
        }
        "rejected" if review_attempts < 3 => "q3_revision".to_string(),
        "rejected" => "q4_rejection".to_string(),
        "garbage" => "garbage".to_string(),
        _ => "q2_manual".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    /// Phase A p7: `RECIPE_SELECT` must end with the three v3 authoring
    /// columns at indices 31/32/33, matching `decode_recipe_row`'s
    /// `row.get(31..=33)`. This catches the most error-prone p7 hazard — a
    /// column inserted in the middle of the SELECT would silently shift every
    /// downstream `row.get(N)` index, and a missing new column would orphan
    /// the round-trip. tokio-postgres checks these at runtime (not compile
    /// time), so this static assertion is the narrowest regression guard.
    #[test]
    fn recipe_select_round_trips_v3_authoring_columns() {
        let cols: Vec<&str> = RECIPE_SELECT.trim().split(',').map(|c| c.trim()).collect();
        assert_eq!(cols.len(), 34, "RECIPE_SELECT must select 34 columns");
        assert_eq!(cols[0], "id");
        assert_eq!(cols[30], "updated_at");
        assert_eq!(cols[31], "step_descriptions");
        assert_eq!(cols[32], "variants");
        assert_eq!(cols[33], "dependency_registry");
    }

    /// Build a `PgRecipe` that is Tier-0 eligible by default: validated, no
    /// `05:validator` tag, `tier = "mature"`, `wilson_lower = 0.80`. Each test
    /// mutates one field to confirm that single guard blocks eligibility.
    fn base_recipe() -> PgRecipe {
        PgRecipe {
            id: Uuid::nil(),
            tenant_id: "local".to_string(),
            user_id: "default".to_string(),
            agent_id: "default".to_string(),
            project_id: "default".to_string(),
            name: "test-recipe".to_string(),
            description: "test".to_string(),
            trigger: None,
            steps: serde_json::json!([]),
            status: "active".to_string(),
            prior_knowledge_content: None,
            override_prompt_creation: false,
            class_code: 21,
            prompt_uid: 0,
            consumer_tags: vec![],
            intent_examples: None,
            tier: "mature".to_string(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.80,
            validation_status: "validated".to_string(),
            validation_errors: vec![],
            review_feedback: None,
            review_attempts: 0,
            rejected_at: None,
            queue_code: None,
            source: "test".to_string(),
            content_hash: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            step_descriptions: None,
            variants: None,
            dependency_registry: None,
        }
    }

    #[test]
    fn tier0_eligible_when_all_guards_hold() {
        assert!(base_recipe().is_tier0_eligible());
        // candidate tier is also eligible with a high enough wilson score.
        let mut r = base_recipe();
        r.tier = "candidate".to_string();
        assert!(r.is_tier0_eligible());
    }

    #[test]
    fn tier0_blocked_when_wilson_below_threshold() {
        // FIND-P7-11 / FIND-05: a mature row that was never used (wilson 0.0)
        // must NOT be silently escalated to Tier 0.
        let mut r = base_recipe();
        r.wilson_lower = 0.0;
        assert!(!r.is_tier0_eligible(), "wilson 0.0 must block Tier 0");

        // Just under the threshold still blocks.
        r.wilson_lower = 0.69;
        assert!(!r.is_tier0_eligible(), "wilson 0.69 must block Tier 0");

        // Exactly 0.70 passes (>= comparison).
        r.wilson_lower = 0.70;
        assert!(r.is_tier0_eligible(), "wilson 0.70 must pass Tier 0");
    }

    #[test]
    fn tier0_blocked_when_tier_is_immature() {
        for tier in ["seedling", "growing", "rejected", ""] {
            let mut r = base_recipe();
            r.tier = tier.to_string();
            assert!(!r.is_tier0_eligible(), "tier={tier:?} must block Tier 0");
        }
    }

    #[test]
    fn tier0_blocked_when_not_validated() {
        for status in [
            "pending",
            "rejected",
            "auto_failed",
            "garbage",
            "upgrade_queued",
        ] {
            let mut r = base_recipe();
            r.validation_status = status.to_string();
            assert!(
                !r.is_tier0_eligible(),
                "validation_status={status:?} must block Tier 0"
            );
        }
    }

    #[test]
    fn tier0_blocked_when_validator_tag_present() {
        // SEC-01: a row carrying the 05:validator tag is under evaluation and
        // must not be delivered, hence not Tier-0 eligible.
        let mut r = base_recipe();
        r.consumer_tags = vec!["05:validator".to_string()];
        assert!(!r.is_tier0_eligible());
        // A non-validator tag does not block.
        let mut r = base_recipe();
        r.consumer_tags = vec!["misc".to_string()];
        assert!(r.is_tier0_eligible());
    }

    #[test]
    fn has_validator_tag_detects_tag_in_any_position() {
        let mut r = base_recipe();
        r.consumer_tags = vec!["a".to_string(), "05:validator".to_string(), "b".to_string()];
        assert!(r.has_validator_tag());
        r.consumer_tags = vec!["a".to_string(), "b".to_string()];
        assert!(!r.has_validator_tag());
    }

    #[test]
    fn is_deliverable_requires_validated_and_no_validator_tag() {
        let r = base_recipe();
        assert!(r.is_deliverable());

        let mut r = base_recipe();
        r.validation_status = "pending".to_string();
        assert!(!r.is_deliverable());

        let mut r = base_recipe();
        r.consumer_tags = vec!["05:validator".to_string()];
        assert!(!r.is_deliverable());
    }
}
