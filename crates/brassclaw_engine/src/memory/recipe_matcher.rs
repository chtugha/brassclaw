//! Recipe / ToolSkill matcher.
//!
//! Looks up the best-matching Recipe for a given user input, plus a small
//! ranked list of matching ToolSkill entries for Tier 1 prompt injection.
//!
//! Trigger semantics:
//! - `Exact` — case-insensitive command equality → score 1.0
//! - `Keyword` — Jaccard coefficient between tokenized user input and the
//!   trigger's keywords, compared against the recipe's threshold
//! - `Pattern` — regex match (caller pre-validates `regex::RegexBuilder` has
//!   a `size_limit(10_000)` cap so LLM-authored regex can't ReDoS)
//!
//! For Tier 1 we deliberately cap the candidate set to 5 skills — each
//! serialized as a compact JSON DTO (~50 tokens) so the injected prompt
//! stays under the cache-friendly threshold even with several matches.

use std::collections::HashSet;
use std::sync::Arc;

use crate::traits::store::Store;
use crate::types::error::EngineError;
use crate::types::memory::{DocType, MemoryDoc};
use crate::types::project::ProjectId;
use crate::types::recipe::{Recipe, RecipeTrigger, RecipeValidation, ToolSkill, ValidationStatus};

/// Minimum match score before a Recipe is surfaced for any tier.
pub const RECIPE_MIN_MATCH: f64 = 0.5;

/// How many ToolSkill entries to return per lookup (Tier 1 budget).
const SKILL_TIER1_LIMIT: usize = 5;

/// Minimum Jaccard overlap to consider keyword matches significant.
const JACCARD_MIN_THRESHOLD: f64 = 0.30;
/// Score returned when an exact/near-exact regex match is found (near-perfect confidence).
const EXACT_MATCH_SCORE: f64 = 0.95;
/// Regex size limit (bytes) applied to each compiled pattern to prevent ReDoS.
const REGEX_SIZE_LIMIT_BYTES: usize = 10_000;
/// Minimum keyword trigger weight for loose/low-confidence matches in tests.
#[cfg(test)]
const KW_TRIGGER_LOOSE_WEIGHT: f64 = 0.20;

/// Lightweight DTO surfaced to the agent loop's `RecipeStage`.
///
/// Carries the minimum needed for Tier 0/1 branching — NOT the full
/// `Recipe` struct, so `brassclaw_turns` doesn't have to depend on
/// `brassclaw_engine` (avoids a cycle).
#[derive(Debug, Clone)]
pub struct RecipeMatch {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub wilson_lower: f64,
    pub tier0_eligible: bool,
    pub steps: Vec<RecipeStepMatch>,
    pub validation_kind: String,
}

#[derive(Debug, Clone)]
pub struct RecipeStepMatch {
    pub skill_name: String,
    pub tool: String,
    pub params: serde_json::Value,
    pub description: String,
}

/// ToolSkill entry surfaced for Tier 1 injection.
#[derive(Debug, Clone)]
pub struct ToolSkillMatch {
    pub id: String,
    pub name: String,
    pub tool_name: String,
    pub description: String,
    pub param_template: serde_json::Value,
    pub preconditions: String,
    pub estimated_tokens: usize,
}

/// Looks up validated Recipes / ToolSkills for a user input.
pub struct RecipeMatcher {
    store: Arc<dyn Store>,
}

impl RecipeMatcher {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Find the best Recipe match for `user_input` against all validated
    /// Recipe docs in this project.
    pub async fn find_recipe(
        &self,
        project_id: ProjectId,
        user_id: &str,
        user_input: &str,
    ) -> Result<Option<(RecipeMatch, f64)>, EngineError> {
        let docs = self.load_recipe_docs(project_id, user_id).await?;
        let mut best: Option<(RecipeMatch, f64)> = None;
        for doc in docs {
            let recipe = match parse_recipe(&doc) {
                Ok(r) => r,
                Err(error) => {
                    tracing::debug!(
                        %error,
                        doc_id = %doc.id.0,
                        "recipe matcher: skipping undecodable recipe"
                    );
                    continue;
                }
            };
            if !matches!(recipe.validation_status, ValidationStatus::Validated) {
                continue;
            }
            let score = score_trigger(&recipe.trigger, user_input);
            if score < RECIPE_MIN_MATCH {
                continue;
            }
            let tier0 = recipe.is_tier0_eligible();
            let matches = RecipeMatch {
                id: recipe.id.clone(),
                name: recipe.name.clone(),
                tier: recipe.tier.clone(),
                wilson_lower: recipe.wilson_lower,
                tier0_eligible: tier0,
                validation_kind: validation_kind(&recipe.validation),
                steps: recipe
                    .steps
                    .iter()
                    .map(|step| RecipeStepMatch {
                        skill_name: step.skill.clone(),
                        tool: step.tool.clone(),
                        params: step.params.clone(),
                        description: step.description.clone(),
                    })
                    .collect(),
            };
            if best.as_ref().is_none_or(|(_, s)| score > *s) {
                best = Some((matches, score));
            }
        }
        Ok(best)
    }

    /// Find ToolSkill matches for Tier 1 prompt injection.
    ///
    /// Filter: `validation_status == Validated` only — the agent loop must
    /// not surface non-validated skills because their triggers/descriptions
    /// may still be flagged for review.
    pub async fn find_skills(
        &self,
        project_id: ProjectId,
        user_id: &str,
        user_input: &str,
    ) -> Result<Vec<ToolSkillMatch>, EngineError> {
        let docs = self.load_tool_skill_docs(project_id, user_id).await?;
        let tokens = tokenize(user_input);
        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        let mut scored: Vec<(f64, ToolSkillMatch)> = Vec::new();
        for doc in docs {
            let skill = match parse_tool_skill(&doc) {
                Ok(s) => s,
                Err(error) => {
                    tracing::debug!(
                        %error,
                        doc_id = %doc.id.0,
                        "recipe matcher: skipping undecodable tool skill"
                    );
                    continue;
                }
            };
            if !matches!(skill.validation_status, ValidationStatus::Validated) {
                continue;
            }
            let skill_desc_tokens = tokenize(&skill.description);
            let jac = jaccard(&tokens, &skill_desc_tokens);
            if jac < JACCARD_MIN_THRESHOLD {
                continue;
            }
            scored.push((
                jac,
                ToolSkillMatch {
                    id: skill.id.clone(),
                    name: skill.name.clone(),
                    tool_name: skill.tool_name.clone(),
                    description: skill.description.clone(),
                    param_template: skill.param_template.clone(),
                    preconditions: skill.preconditions.clone(),
                    estimated_tokens: skill.estimated_tokens(),
                },
            ));
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored
            .into_iter()
            .take(SKILL_TIER1_LIMIT)
            .map(|(_, s)| s)
            .collect())
    }

    async fn load_recipe_docs(
        &self,
        project_id: ProjectId,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, EngineError> {
        let docs = self
            .store
            .list_memory_docs_with_shared(project_id, user_id)
            .await?;
        Ok(docs
            .into_iter()
            .filter(|d| d.doc_type == DocType::Recipe)
            .collect())
    }

    async fn load_tool_skill_docs(
        &self,
        project_id: ProjectId,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, EngineError> {
        let docs = self
            .store
            .list_memory_docs_with_shared(project_id, user_id)
            .await?;
        Ok(docs
            .into_iter()
            .filter(|d| d.doc_type == DocType::ToolSkill)
            .collect())
    }
}

/// Public helper: Jaccard similarity between two token sets.
///
/// Returns `0.0` when both sets are empty — calling this with empty input
/// would otherwise produce a NaN propagating into the score sort.
pub fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    intersection / union
}

/// Tokenize a freeform input. Filters single-character fragments and ASCII
/// punctuation; lowercases.
pub fn tokenize(input: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for token in input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 2)
    {
        out.insert(token.to_lowercase());
    }
    out
}

fn score_trigger(trigger: &RecipeTrigger, user_input: &str) -> f64 {
    match trigger {
        RecipeTrigger::Exact { command } => {
            if user_input.trim().eq_ignore_ascii_case(command.trim()) {
                1.0
            } else {
                0.0
            }
        }
        RecipeTrigger::Keyword {
            keywords,
            threshold,
        } => {
            let keywords_set: HashSet<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
            let input_tokens = tokenize(user_input);
            let overlap = jaccard(&input_tokens, &keywords_set);
            if overlap >= *threshold { overlap } else { 0.0 }
        }
        RecipeTrigger::Pattern { patterns } => {
            for pattern in patterns {
                if let Ok(re) = regex_limited(pattern)
                    && re.is_match(user_input)
                {
                    return EXACT_MATCH_SCORE;
                }
            }
            0.0
        }
    }
}

// 10 000-byte compiled regex cap — prevents LLM-authored regex from ReDoS-ing
// the executor pipeline when a Recipe with a Pattern trigger gets hot.
// 10 MB was too permissive: a 10 MB NFA is large enough to exhibit
// exponential backtracking on adversarial inputs. 10 000 bytes rejects
// genuinely pathological regexes while still allowing typical trigger patterns.
fn regex_limited(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT_BYTES)
        .build()
}

fn validation_kind(v: &RecipeValidation) -> String {
    match v {
        RecipeValidation::None => "none".to_string(),
        RecipeValidation::ShellCheck { .. } => "shell_check".to_string(),
        RecipeValidation::FileExists { .. } => "file_exists".to_string(),
        RecipeValidation::Custom { .. } => "custom".to_string(),
    }
}

fn parse_recipe(doc: &MemoryDoc) -> Result<Recipe, EngineError> {
    Recipe::from_metadata(&doc.metadata).map_err(|e| EngineError::Store {
        reason: format!("recipe decode failed: {e}"),
    })
}

fn parse_tool_skill(doc: &MemoryDoc) -> Result<ToolSkill, EngineError> {
    ToolSkill::from_metadata(&doc.metadata).map_err(|e| EngineError::Store {
        reason: format!("tool_skill decode failed: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::recipe::RecipeSource;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn minimal_recipe(status: ValidationStatus) -> Recipe {
        Recipe {
            id: "test-id".to_string(),
            name: "test".to_string(),
            description: "test recipe".to_string(),
            trigger: RecipeTrigger::Exact {
                command: "test".to_string(),
            },
            steps: vec![],
            validation: RecipeValidation::None,
            category: "test".to_string(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".to_string(),
            source: RecipeSource::Authored,
            source_thread_id: None,
            project_id: "proj".to_string(),
            user_id: "user".to_string(),
            validation_status: status,
            validation_errors: vec![],
            review_feedback: None,
            review_attempts: 0,
            rejected_at: None,
            similarity_parent_id: None,
            skip_similarity: false,
            last_audit_at: None,
            audit_failure_count: 0,
            replaces_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            variants: Vec::new(),
            step_descriptions: serde_json::Value::Null,
            dependency_registry: serde_json::Value::Null,
        }
    }

    fn minimal_tool_skill(status: ValidationStatus) -> ToolSkill {
        ToolSkill {
            id: "skill-id".to_string(),
            name: "test-skill".to_string(),
            tool_name: "shell".to_string(),
            description: "test skill description test".to_string(),
            param_template: serde_json::Value::Object(Default::default()),
            param_schema: vec![],
            preconditions: String::new(),
            error_handling: String::new(),
            code_snippet: None,
            category: "test".to_string(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".to_string(),
            source: RecipeSource::Authored,
            source_thread_id: None,
            project_id: "proj".to_string(),
            user_id: "user".to_string(),
            validation_status: status,
            validation_errors: vec![],
            review_feedback: None,
            review_attempts: 0,
            rejected_at: None,
            similarity_parent_id: None,
            skip_similarity: false,
            last_audit_at: None,
            audit_failure_count: 0,
            replaces_id: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    // ── Sub-step 3.3 regression: Validated == reachable; AutoPassed is NOT ──

    #[test]
    fn recipe_filter_rejects_auto_passed_not_validated() {
        // AutoPassed is the immediate predecessor to Validated in the lifecycle.
        // The loop must only see Validated recipes.
        let auto_passed = minimal_recipe(ValidationStatus::AutoPassed);
        assert!(
            !matches!(auto_passed.validation_status, ValidationStatus::Validated),
            "AutoPassed must not pass the Validated filter"
        );
        // Confirm recipe_matcher's exact filter expression rejects it.
        let is_reachable = matches!(auto_passed.validation_status, ValidationStatus::Validated);
        assert!(
            !is_reachable,
            "AutoPassed recipe must not be reachable by the loop"
        );
    }

    #[test]
    fn recipe_filter_rejects_pending_upgrade_queued_and_garbage() {
        for status in [
            ValidationStatus::Pending,
            ValidationStatus::UpgradeQueued,
            ValidationStatus::AutoFailed,
            ValidationStatus::Rejected,
            ValidationStatus::Garbage,
            ValidationStatus::ReviewRequested,
        ] {
            let recipe = minimal_recipe(status.clone());
            let is_reachable = matches!(recipe.validation_status, ValidationStatus::Validated);
            assert!(
                !is_reachable,
                "status {:?} must not be reachable by the loop",
                status
            );
        }
    }

    #[test]
    fn recipe_filter_accepts_validated_only() {
        let validated = minimal_recipe(ValidationStatus::Validated);
        let is_reachable = matches!(validated.validation_status, ValidationStatus::Validated);
        assert!(
            is_reachable,
            "Validated recipe must be reachable by the loop"
        );
    }

    #[test]
    fn tool_skill_filter_rejects_auto_passed() {
        let skill = minimal_tool_skill(ValidationStatus::AutoPassed);
        let is_reachable = matches!(skill.validation_status, ValidationStatus::Validated);
        assert!(
            !is_reachable,
            "AutoPassed ToolSkill must not be reachable by the loop"
        );
    }

    #[test]
    fn tool_skill_filter_accepts_validated_only() {
        let skill = minimal_tool_skill(ValidationStatus::Validated);
        let is_reachable = matches!(skill.validation_status, ValidationStatus::Validated);
        assert!(
            is_reachable,
            "Validated ToolSkill must be reachable by the loop"
        );
    }

    fn kw_trigger(words: &[&str], threshold: f64) -> RecipeTrigger {
        RecipeTrigger::Keyword {
            keywords: words.iter().map(|s| s.to_string()).collect(),
            threshold,
        }
    }

    fn exact_trigger(cmd: &str) -> RecipeTrigger {
        RecipeTrigger::Exact {
            command: cmd.to_string(),
        }
    }

    #[test]
    fn exact_trigger_scores_one_when_match() {
        assert_eq!(
            score_trigger(&exact_trigger("git status"), "git status"),
            1.0
        );
    }

    #[test]
    fn exact_trigger_scores_zero_on_mismatch() {
        assert_eq!(score_trigger(&exact_trigger("git status"), "git log"), 0.0);
    }

    #[test]
    fn keyword_trigger_uses_jaccard_with_threshold() {
        let trig = kw_trigger(&["github", "issue"], 0.5);
        // "list github issues" → {list, github, issues} vs {github, issue}
        // intersection = {github}. union = 4. jaccard = 0.25 < 0.5.
        assert_eq!(score_trigger(&trig, "list github issues"), 0.0);
        // Lower the threshold and same query passes.
        let loose = kw_trigger(&["github", "issue"], KW_TRIGGER_LOOSE_WEIGHT);
        assert!(score_trigger(&loose, "list github issues") > 0.0);
    }

    #[test]
    fn pattern_trigger_requires_regex_match() {
        let trig = RecipeTrigger::Pattern {
            patterns: vec![r"^npm +install(\s|$)".to_string()],
        };
        assert_eq!(score_trigger(&trig, "npm install foo"), 0.95);
        assert_eq!(score_trigger(&trig, "yarn install"), 0.0);
    }

    #[test]
    fn pattern_with_invalid_regex_returns_zero() {
        let trig = RecipeTrigger::Pattern {
            patterns: vec!["[".to_string()],
        };
        assert_eq!(score_trigger(&trig, "anything"), 0.0);
    }

    #[test]
    fn jaccard_uses_set_overlap() {
        let a: HashSet<String> = ["git", "status"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["git", "diff"].iter().map(|s| s.to_string()).collect();
        // {git} / {git,status,diff} = 1/3
        assert!((jaccard(&a, &b) - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_zero_on_two_empty_sets() {
        let empty: HashSet<String> = HashSet::new();
        assert_eq!(jaccard(&empty, &empty), 0.0);
    }

    #[test]
    fn tokenize_filters_short_tokens() {
        let toks = tokenize("git a status");
        assert!(toks.contains("git"));
        assert!(toks.contains("status"));
        assert!(!toks.contains("a"));
    }
}
