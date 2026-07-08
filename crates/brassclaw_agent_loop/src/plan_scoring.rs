//! Session scoring and skill maturity classification for the self-improving
//! plan library (subtask 5).
//!
//! All functions in this module are **pure arithmetic** — no I/O, no LLM,
//! no database access.  They take observable session signals and produce
//! numeric scores and tier labels that the post-turn hook uses to decide
//! whether to persist a plan or promote a skill.
//!
//! ## Wilson Lower Bound
//!
//! The confidence interval for a binomial proportion (Clopper–Pearson
//! approximation via the Wilson score):
//!
//! ```text
//! w_lower = (p̂ + z²/2n  −  z × sqrt((p̂(1−p̂) + z²/4n)/n))  /  (1 + z²/n)
//! ```
//!
//! where  `p̂ = successes / (successes + failures)`,  `n = successes + failures`,
//! and `z` is the quantile (1.96 → 95 % confidence).

use crate::content_cache::ContentCacheState;
use crate::plan_state::AgentPlanState;

// ── SkillMaturityTier ─────────────────────────────────────────────────────────

/// Maturity level of a plan/skill entry in the library.
///
/// Tiers are ordered: Seedling → Growing → Mature → Candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillMaturityTier {
    /// Newly created; only exists as a workspace SKILL.md.
    Seedling,
    /// Accumulating confidence; `activation.tags` carries `"growing"`.
    Growing,
    /// Promoted to TenantShared; visible to all agents in the workspace.
    Mature,
    /// GitHub PR candidate for upstream review.
    Candidate,
}

impl Default for SkillMaturityTier {
    fn default() -> Self {
        Self::Seedling
    }
}

// ── OutcomeVector ──────────────────────────────────────────────────────────────

/// A single tool call outcome.
#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub tool_id: String,
    /// `true` when the result text does NOT start with a failure prefix.
    pub success: bool,
}

impl ToolOutcome {
    /// Classify a tool result as success/failure by its textual content.
    ///
    /// Returns `false` (failure) when the text starts with one of the four
    /// prefixes: `"Error"`, `"error"`, `"Failed"`, or `"failed"`.
    pub fn from_result_text(tool_id: impl Into<String>, result: &str) -> Self {
        let success = !result.starts_with("Error")
            && !result.starts_with("error")
            && !result.starts_with("Failed")
            && !result.starts_with("failed");
        Self {
            tool_id: tool_id.into(),
            success,
        }
    }
}

/// Observable signals from a single agent session.
#[derive(Debug, Clone)]
pub struct OutcomeVector {
    /// Fraction of plan steps completed (0.0–1.0).
    pub plan_completion: f64,
    /// Fraction of tool calls that succeeded (0.0–1.0).
    pub tool_success_rate: f64,
    /// Steps-completed-to-iterations ratio (capped at 1.0).
    pub iteration_efficiency: f64,
    /// Cache utility: average fetch count per cached entry (capped at 1.0).
    pub cache_utility: f64,
}

impl OutcomeVector {
    /// Compute the weighted overall score (0.0–1.0).
    ///
    /// Weights mirror the plan specification:
    /// plan_completion (0.40) + tool_success_rate (0.30)
    /// + iteration_efficiency (0.20) + cache_utility (0.10).
    pub fn overall_score(&self) -> f64 {
        0.40 * self.plan_completion
            + 0.30 * self.tool_success_rate
            + 0.20 * self.iteration_efficiency
            + 0.10 * self.cache_utility
    }
}

// ── score_session ─────────────────────────────────────────────────────────────

/// Compute the [`OutcomeVector`] for a completed session and return its overall
/// weighted score.
///
/// Returns `0.0` when there is no plan state (unplanned sessions score 0 so
/// they are never counted as plan-library successes).
pub fn score_session(
    plan_state: Option<&AgentPlanState>,
    tool_outcomes: &[ToolOutcome],
    total_iterations: usize,
    content_cache: &ContentCacheState,
) -> f64 {
    // Plan completion — 0 when there is no plan.
    let plan_completion = match plan_state {
        None => return 0.0,
        Some(ps) if ps.steps.is_empty() => 0.0,
        Some(ps) => {
            let done = ps.current_step.min(ps.steps.len()) as f64;
            done / ps.steps.len() as f64
        }
    };

    // Tool success rate.
    let tool_success_rate = if tool_outcomes.is_empty() {
        1.0 // benefit of the doubt when no tools were called
    } else {
        let successes = tool_outcomes.iter().filter(|t| t.success).count();
        successes as f64 / tool_outcomes.len() as f64
    };

    // Iteration efficiency — steps completed per iteration, capped at 1.0.
    let steps_completed = plan_state
        .map(|ps| ps.current_step.min(ps.steps.len()))
        .unwrap_or(0);
    let iteration_efficiency = if total_iterations == 0 {
        0.0
    } else {
        (steps_completed as f64 / total_iterations as f64).min(1.0)
    };

    // Cache utility — average fetch count per entry, capped at 1.0.
    let cache_utility = if content_cache.entries.is_empty() {
        0.0
    } else {
        let total_fetches: u32 = content_cache.entries.values().map(|e| e.fetch_count).sum();
        let avg = total_fetches as f64 / content_cache.entries.len() as f64;
        avg.min(1.0)
    };

    let v = OutcomeVector {
        plan_completion,
        tool_success_rate,
        iteration_efficiency,
        cache_utility,
    };
    v.overall_score()
}

// ── wilson_lower_bound ────────────────────────────────────────────────────────

/// Wilson score lower bound for a binomial proportion.
///
/// `z` is the quantile from the standard normal distribution.
/// Use `z = 1.96` for 95 % confidence (industry standard).
///
/// Returns `0.0` when `successes + failures == 0`.
pub fn wilson_lower_bound(successes: u64, failures: u64, z: f64) -> f64 {
    let n = successes + failures;
    if n == 0 {
        return 0.0;
    }
    let n_f = n as f64;
    let p_hat = successes as f64 / n_f;
    let z2 = z * z;
    let numerator = p_hat + z2 / (2.0 * n_f)
        - z * ((p_hat * (1.0 - p_hat) + z2 / (4.0 * n_f)) / n_f).sqrt();
    let denominator = 1.0 + z2 / n_f;
    numerator / denominator
}

// ── classify_tier ─────────────────────────────────────────────────────────────

/// Classify a skill into a maturity tier based on its usage and Wilson lower
/// bound.
///
/// Tier thresholds (from the architecture plan):
///
/// | Tier      | usage_count | w_lower  |
/// |-----------|-------------|----------|
/// | Candidate | ≥ 50        | ≥ threshold (default 0.80) |
/// | Mature    | ≥ 20        | ≥ 0.70   |
/// | Growing   | ≥ 5         | ≥ 0.50   |
/// | Seedling  | any         | any      |
///
/// `promotion_threshold` overrides the Candidate tier's `w_lower` requirement.
pub fn classify_tier(usage_count: u64, w_lower: f64, promotion_threshold: f64) -> SkillMaturityTier {
    if usage_count >= 50 && w_lower >= promotion_threshold {
        return SkillMaturityTier::Candidate;
    }
    if usage_count >= 20 && w_lower >= 0.70 {
        return SkillMaturityTier::Mature;
    }
    if usage_count >= 5 && w_lower >= 0.50 {
        return SkillMaturityTier::Growing;
    }
    SkillMaturityTier::Seedling
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_cache::CachedEntry;
    use crate::plan_state::{AgentPlanState, PlanType};

    fn make_plan(steps: usize, current: usize) -> AgentPlanState {
        AgentPlanState {
            steps: (0..steps).map(|i| format!("step {}", i)).collect(),
            current_step: current,
            raw_plan_text: "test".to_string(),
            plan_type: PlanType::Generic,
        }
    }

    #[test]
    fn wilson_zero_samples() {
        assert_eq!(wilson_lower_bound(0, 0, 1.96), 0.0);
    }

    #[test]
    fn wilson_all_success() {
        let w = wilson_lower_bound(100, 0, 1.96);
        // Should be close to 1.0 but below due to interval width
        assert!(w > 0.95, "w={}", w);
        assert!(w < 1.0);
    }

    #[test]
    fn wilson_half_success_50_trials() {
        let w = wilson_lower_bound(25, 25, 1.96);
        // Should be around 0.37
        assert!(w > 0.30 && w < 0.50, "w={}", w);
    }

    #[test]
    fn classify_tier_seedling() {
        assert_eq!(classify_tier(2, 0.9, 0.80), SkillMaturityTier::Seedling);
    }

    #[test]
    fn classify_tier_growing() {
        assert_eq!(classify_tier(10, 0.60, 0.80), SkillMaturityTier::Growing);
    }

    #[test]
    fn classify_tier_mature() {
        assert_eq!(classify_tier(25, 0.75, 0.80), SkillMaturityTier::Mature);
    }

    #[test]
    fn classify_tier_candidate() {
        assert_eq!(classify_tier(50, 0.85, 0.80), SkillMaturityTier::Candidate);
    }

    #[test]
    fn classify_tier_candidate_not_enough_usage() {
        assert_ne!(classify_tier(49, 0.90, 0.80), SkillMaturityTier::Candidate);
    }

    #[test]
    fn score_session_no_plan_returns_zero() {
        let score = score_session(None, &[], 5, &Default::default());
        assert_eq!(score, 0.0);
    }

    #[test]
    fn score_session_full_completion() {
        let plan = make_plan(3, 3);
        let outcomes = vec![
            ToolOutcome::from_result_text("builtin.shell", "ok"),
            ToolOutcome::from_result_text("builtin.shell", "done"),
        ];
        let score = score_session(Some(&plan), &outcomes, 3, &Default::default());
        // plan_completion=1.0, tool_success_rate=1.0, efficiency=1.0, cache=0.0
        // = 0.40 + 0.30 + 0.20 + 0.0 = 0.90
        assert!((score - 0.90).abs() < 0.001, "score={}", score);
    }

    #[test]
    fn score_session_counts_failure_tool() {
        let plan = make_plan(2, 2);
        let outcomes = vec![
            ToolOutcome::from_result_text("builtin.shell", "Error: command not found"),
        ];
        let score = score_session(Some(&plan), &outcomes, 2, &Default::default());
        // tool_success_rate = 0
        assert!(score < 0.90);
    }

    #[test]
    fn score_session_with_cache_utility() {
        let plan = make_plan(2, 2);
        let mut cache = ContentCacheState::default();
        let mut e = CachedEntry::new("k1".to_string(), "builtin.shell".to_string(), "data".to_string(), 0);
        e.fetch_count = 2;
        cache.entries.insert("k1".to_string(), e);
        let score = score_session(Some(&plan), &[], 2, &cache);
        // cache_utility = min(2.0, 1.0) = 1.0 → +0.10
        assert!(score > 0.90 - 0.001, "score={}", score);
    }
}
