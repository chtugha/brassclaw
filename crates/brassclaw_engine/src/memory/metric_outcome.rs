//! Atomic outcome recording for Recipes and ToolSkills.
//!
//! Each successful (or failed) execution increments the persisted metric
//! counters and recomputes the Wilson score lower bound + maturity tier,
//! then writes the new entity back as a `MemoryDoc` upsert. The
//! recomputation happens on the read-modify-write path, so concurrent
//! outcomes racing on the same `Recipe.id` can be lost; the engine
//! documents document this trade-off and treats Wilson lower-bound drift
//! as acceptable noise (mature-tier promotion is still gated on
//! `usage_count >= 20` which absorbs single-call jitter).
//!
//! We avoid touching the underlying SQL row directly because Recipes
//! are persisted as `MemoryDoc<DocType::Recipe>` entries — the
//! "atomic" guarantee here is "one Store::save_memory_doc per outcome",
//! which the libSQL upsert already serializes via `ON CONFLICT DO UPDATE`.

use std::sync::Arc;

use crate::traits::store::Store;
use crate::types::error::EngineError;
use crate::types::memory::{DocType, MemoryDoc};
use crate::types::project::ProjectId;
use crate::types::recipe::{Recipe, ToolSkill};

/// Outcome recorder for `Recipe` and `ToolSkill` MemoryDocs.
#[derive(Clone)]
pub struct MetricRecorder {
    store: Arc<dyn Store>,
}

impl MetricRecorder {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Atomically record a Recipe execution outcome.
    pub async fn record_recipe(
        &self,
        project_id: ProjectId,
        user_id: &str,
        recipe_id: &str,
        success: bool,
    ) -> Result<(), EngineError> {
        let mut doc = self
            .load_recipe_doc(project_id, user_id, recipe_id)
            .await?
            .ok_or_else(|| EngineError::Store {
                reason: format!("recipe outcome: id '{recipe_id}' not found"),
            })?;
        let mut recipe = Recipe::from_metadata(&doc.metadata).map_err(|e| EngineError::Store {
            reason: format!("recipe outcome: decode failed: {e}"),
        })?;
        apply_outcome(
            &mut recipe.usage_count,
            &mut recipe.success_count,
            &mut recipe.failure_count,
            &mut recipe.wilson_lower,
            &mut recipe.tier,
            success,
        );
        let updated_at = chrono::Utc::now();
        recipe.updated_at = updated_at;
        doc.metadata = recipe.to_metadata().map_err(|e| EngineError::Store {
            reason: format!("recipe outcome: encode failed: {e}"),
        })?;
        doc.updated_at = updated_at;
        self.store.save_memory_doc(&doc).await
    }

    /// Atomically record a ToolSkill execution outcome.
    pub async fn record_tool_skill(
        &self,
        project_id: ProjectId,
        user_id: &str,
        skill_id: &str,
        success: bool,
    ) -> Result<(), EngineError> {
        let mut doc = self
            .load_tool_skill_doc(project_id, user_id, skill_id)
            .await?
            .ok_or_else(|| EngineError::Store {
                reason: format!("tool skill outcome: id '{skill_id}' not found"),
            })?;
        let mut skill =
            ToolSkill::from_metadata(&doc.metadata).map_err(|e| EngineError::Store {
                reason: format!("tool skill outcome: decode failed: {e}"),
            })?;
        apply_outcome(
            &mut skill.usage_count,
            &mut skill.success_count,
            &mut skill.failure_count,
            &mut skill.wilson_lower,
            &mut skill.tier,
            success,
        );
        let updated_at = chrono::Utc::now();
        skill.updated_at = updated_at;
        doc.metadata = skill.to_metadata().map_err(|e| EngineError::Store {
            reason: format!("tool skill outcome: encode failed: {e}"),
        })?;
        doc.updated_at = updated_at;
        self.store.save_memory_doc(&doc).await
    }

    async fn load_recipe_doc(
        &self,
        project_id: ProjectId,
        user_id: &str,
        recipe_id: &str,
    ) -> Result<Option<MemoryDoc>, EngineError> {
        let docs = self
            .store
            .list_memory_docs_with_shared(project_id, user_id)
            .await?;
        Ok(docs
            .into_iter()
            .find(|d| d.doc_type == DocType::Recipe && recipe_id_matches(&d.metadata, recipe_id)))
    }

    async fn load_tool_skill_doc(
        &self,
        project_id: ProjectId,
        user_id: &str,
        skill_id: &str,
    ) -> Result<Option<MemoryDoc>, EngineError> {
        let docs = self
            .store
            .list_memory_docs_with_shared(project_id, user_id)
            .await?;
        Ok(docs
            .into_iter()
            .find(|d| d.doc_type == DocType::ToolSkill && recipe_id_matches(&d.metadata, skill_id)))
    }
}

/// True when the encoded entity's `id` field matches `target`.
///
/// We accept both `id` (canonical) and the legacy `name` field as a
/// fallback so callers can pass either identifier — matchers surface
/// `RecipeMatchDto.id` from `Recipe.id`, so `id` is the common path.
fn recipe_id_matches(metadata: &serde_json::Value, target: &str) -> bool {
    metadata
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s == target)
        .unwrap_or(false)
}

/// Increment counters, recompute Wilson lower bound and tier.
///
/// `promotion_threshold` (default 0.80) overrides the Candidate tier's
/// Wilson requirement; the agent passes it explicitly when caller wants
/// to tune promotion knobs (today only the default `0.80` is used).
///
/// Tier thresholds (mirrors `agent_loop::plan_scoring::classify_tier` to
/// avoid a layer-inverted dependency between `brassclaw_engine` and
/// `brassclaw_agent_loop`):
///
/// | Tier      | usage_count | w_lower  |
/// |-----------|-------------|----------|
/// | Candidate | ≥ 50        | ≥ 0.80   |
/// | Mature    | ≥ 20        | ≥ 0.70   |
/// | Growing   | ≥ 5         | ≥ 0.50   |
/// | Seedling  | any         | any      |
fn apply_outcome(
    usage_count: &mut u64,
    success_count: &mut u64,
    failure_count: &mut u64,
    wilson_lower: &mut f64,
    tier: &mut String,
    success: bool,
) {
    *usage_count = usage_count.saturating_add(1);
    if success {
        *success_count = success_count.saturating_add(1);
    } else {
        *failure_count = failure_count.saturating_add(1);
    }
    *wilson_lower = recompute_wilson(*success_count, *failure_count);
    *tier = classify_tier(*usage_count, *wilson_lower);
}

/// Wilson score lower bound for a binomial proportion.
///
/// Identical math to `agent_loop::plan_scoring::wilson_lower_bound` —
/// kept here so the engine layer has no upward dependency on the loop
/// layer for outcome recording.
fn recompute_wilson(successes: u64, failures: u64) -> f64 {
    const Z: f64 = 1.96;
    /// Constant divisor inside the Wilson score square-root term (always 4).
    const WILSON_INNER_DIV: f64 = 4.0;
    let n = successes + failures;
    if n == 0 {
        return 0.0;
    }
    let n_f = n as f64;
    let p_hat = successes as f64 / n_f;
    let z2 = Z * Z;
    let numerator = p_hat + z2 / (2.0 * n_f)
        - Z * ((p_hat * (1.0 - p_hat) + z2 / (WILSON_INNER_DIV * n_f)) / n_f).sqrt();
    let denominator = 1.0 + z2 / n_f;
    (numerator / denominator).clamp(0.0, 1.0)
}

fn classify_tier(usage_count: u64, w_lower: f64) -> String {
    if usage_count >= 50 && w_lower >= 0.80 {
        return "candidate".to_string();
    }
    if usage_count >= 20 && w_lower >= 0.70 {
        return "mature".to_string();
    }
    if usage_count >= 5 && w_lower >= 0.50 {
        return "growing".to_string();
    }
    "seedling".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_zero_samples_returns_zero() {
        assert_eq!(recompute_wilson(0, 0), 0.0);
    }

    #[test]
    fn wilson_high_success_with_many_samples_is_high() {
        let v = recompute_wilson(95, 5);
        assert!(v >= 0.85, "expected high confidence, got {v}");
    }

    #[test]
    fn tier_promotion_progression() {
        assert_eq!(classify_tier(0, 0.0), "seedling");
        assert_eq!(classify_tier(4, 0.99), "seedling");
        assert_eq!(classify_tier(5, 0.50), "growing");
        assert_eq!(classify_tier(20, 0.70), "mature");
        assert_eq!(classify_tier(50, 0.80), "candidate");
        assert_eq!(classify_tier(50, 0.79), "mature");
    }

    #[test]
    fn apply_outcome_increments_and_recomputes() {
        let mut usage = 0;
        let mut success = 0;
        let mut failure = 0;
        let mut w = 0.0;
        let mut tier = String::from("seedling");
        apply_outcome(
            &mut usage,
            &mut success,
            &mut failure,
            &mut w,
            &mut tier,
            true,
        );
        assert_eq!(usage, 1);
        assert_eq!(success, 1);
        assert_eq!(failure, 0);
        assert_eq!(tier, "seedling");
        for _ in 0..19 {
            apply_outcome(
                &mut usage,
                &mut success,
                &mut failure,
                &mut w,
                &mut tier,
                true,
            );
        }
        assert_eq!(usage, 20);
        assert_eq!(success, 20);
        assert_eq!(failure, 0);
        assert_eq!(tier, "mature");
    }

    #[test]
    fn apply_outcome_failure_lowers_tier() {
        // Start with a borderline "growing" record: 5 successes, 0 failures.
        // Wilson lower bound ≈ 0.566, just above the 0.50 threshold for "growing".
        // One failure drops Wilson to ≈ 0.437, moving the tier to "seedling".
        let mut usage: u64 = 5;
        let mut success: u64 = 5;
        let mut failure = 0u64;
        let mut w = recompute_wilson(5, 0);
        let initial_tier = classify_tier(usage, w);
        assert_eq!(initial_tier, "growing");
        let mut tier = initial_tier;
        apply_outcome(
            &mut usage,
            &mut success,
            &mut failure,
            &mut w,
            &mut tier,
            false,
        );
        assert_eq!(usage, 6);
        assert_eq!(failure, 1);
        assert_eq!(tier, "seedling", "got: {tier}");
    }
}
