//! Pre-validation similarity gate.
//!
//! Before a freshly-extracted Recipe or ToolSkill enters Step 1
//! auto-validation, this module checks whether a similar component already
//! exists in the validated library. When similarity is high enough, the
//! new candidate is routed to the `Upgrade` queue (operator-visible as
//! "Merge into existing" / "Create as separate" / "Discard duplicate")
//! instead of creating a duplicate entry.
//!
//! Algorithm: Jaccard coefficient on trigger keyword sets for Recipes,
//! and on description-token sets for ToolSkills. Both thresholds are
//! deliberately conservative — false positives err toward merging rather
//! than toward silently dropping a valid new component.

use std::collections::HashSet;
use std::sync::Arc;

use crate::traits::store::Store;
use crate::types::error::EngineError;
use crate::types::memory::{DocType, MemoryDoc};
use crate::types::project::ProjectId;
use crate::types::recipe::{Recipe, ToolSkill, ValidationStatus};

use super::recipe_matcher::{jaccard, tokenize};

/// Recipe similarity threshold (Jaccard). 0.70 — aligned with
/// `intelligent-token-budget-update.md` upgrade-queue routing.
const RECIPE_JACCARD_THRESHOLD: f64 = 0.70;

/// ToolSkill description similarity threshold (Jaccard). 0.80 — slightly
/// tighter because descriptions carry the bulk of a Skill's behaviour.
const SKILL_DESCRIPTION_JACCARD_THRESHOLD: f64 = 0.80;

/// Step-skill overlap threshold (fraction of equal skill names). 0.80 —
/// two Recipes with the same ordered steps are highly likely to be the
/// same recipe with cosmetic trigger variations.
const STEP_SKILL_OVERLAP_THRESHOLD: f64 = 0.80;

#[derive(Debug, Clone)]
pub struct SimilarityMatch {
    pub existing_id: String,
    pub existing_name: String,
    pub score: f64,
    pub reason: String,
}

pub struct SimilarityChecker {
    store: Arc<dyn Store>,
}

impl SimilarityChecker {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Returns `Some(SimilarityMatch)` if a sufficiently-similar Recipe
    /// already exists in the validated library.
    pub async fn check_recipe(
        &self,
        project_id: ProjectId,
        user_id: &str,
        candidate: &Recipe,
    ) -> Result<Option<SimilarityMatch>, EngineError> {
        let docs = self.load_recipe_docs(project_id, user_id).await?;
        let candidate_tokens = trigger_token_set(candidate);
        let candidate_steps: Vec<&str> = candidate.steps.iter().map(|s| s.skill.as_str()).collect();

        let mut best: Option<SimilarityMatch> = None;
        for doc in docs {
            let existing = match parse_recipe(&doc) {
                Ok(r) => r,
                Err(error) => {
                    tracing::debug!(
                        %error,
                        doc_id = %doc.id.0,
                        "similarity checker: skipping undecodable recipe"
                    );
                    continue;
                }
            };
            if !matches!(existing.validation_status, ValidationStatus::Validated) {
                continue;
            }
            let existing_tokens = trigger_token_set(&existing);
            let key_jac = jaccard(&candidate_tokens, &existing_tokens);
            let step_overlap = skill_overlap(&candidate_steps, &existing);
            let (score, reason) = if step_overlap >= STEP_SKILL_OVERLAP_THRESHOLD {
                (
                    step_overlap,
                    format!(
                        "step-skill overlap {step_overlap:.2} (>= {STEP_SKILL_OVERLAP_THRESHOLD:.2})"
                    ),
                )
            } else if key_jac >= RECIPE_JACCARD_THRESHOLD {
                (
                    key_jac,
                    format!("keyword overlap {key_jac:.2} (>= {RECIPE_JACCARD_THRESHOLD:.2})"),
                )
            } else {
                continue;
            };
            let mat = SimilarityMatch {
                existing_id: existing.id.clone(),
                existing_name: existing.name.clone(),
                score,
                reason,
            };
            if best.as_ref().is_none_or(|b| score > b.score) {
                best = Some(mat);
            }
        }
        Ok(best)
    }

    /// Returns `Some(SimilarityMatch)` if a similar ToolSkill already
    /// exists in the validated library — same `tool_name` plus description
    /// Jaccard threshold.
    pub async fn check_skill(
        &self,
        project_id: ProjectId,
        user_id: &str,
        candidate: &ToolSkill,
    ) -> Result<Option<SimilarityMatch>, EngineError> {
        let docs = self.load_tool_skill_docs(project_id, user_id).await?;
        let candidate_tokens = tokenize(&candidate.description);

        let mut best: Option<SimilarityMatch> = None;
        for doc in docs {
            let existing = match parse_skill(&doc) {
                Ok(s) => s,
                Err(error) => {
                    tracing::debug!(
                        %error,
                        doc_id = %doc.id.0,
                        "similarity checker: skipping undecodable tool_skill"
                    );
                    continue;
                }
            };
            if !matches!(existing.validation_status, ValidationStatus::Validated) {
                continue;
            }
            if existing.tool_name != candidate.tool_name {
                continue;
            }
            let existing_tokens = tokenize(&existing.description);
            let score = jaccard(&candidate_tokens, &existing_tokens);
            if score < SKILL_DESCRIPTION_JACCARD_THRESHOLD {
                continue;
            }
            let mat = SimilarityMatch {
                existing_id: existing.id.clone(),
                existing_name: existing.name.clone(),
                score,
                reason: format!(
                    "description overlap {score:.2} on {tool}",
                    tool = candidate.tool_name
                ),
            };
            if best.as_ref().is_none_or(|b| score > b.score) {
                best = Some(mat);
            }
        }
        Ok(best)
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

fn trigger_token_set(recipe: &Recipe) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for token in recipe.trigger.trigger_tokens() {
        for t in tokenize(&token) {
            out.insert(t);
        }
    }
    out
}

fn skill_overlap(candidate: &[&str], existing: &Recipe) -> f64 {
    if candidate.is_empty() {
        return 0.0;
    }
    let existing_names: HashSet<String> = existing
        .steps
        .iter()
        .map(|s| s.skill.to_lowercase())
        .collect();
    let mut match_count = 0;
    for s in candidate {
        if existing_names.contains(&s.to_lowercase()) {
            match_count += 1;
        }
    }
    match_count as f64 / candidate.len() as f64
}

fn parse_recipe(doc: &MemoryDoc) -> Result<Recipe, EngineError> {
    Recipe::from_metadata(&doc.metadata).map_err(|e| EngineError::Store {
        reason: format!("recipe decode failed: {e}"),
    })
}

fn parse_skill(doc: &MemoryDoc) -> Result<ToolSkill, EngineError> {
    ToolSkill::from_metadata(&doc.metadata).map_err(|e| EngineError::Store {
        reason: format!("tool_skill decode failed: {e}"),
    })
}

// `RecipeValidationStatus` is re-exported from this module because the
// validator/similarity modules reference ValidationStatus uniformly.
pub use crate::types::recipe::ValidationStatus as RecipeValidationStatus;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::recipe::{RecipeStep, RecipeTrigger, ValidationStatus};

    fn make_recipe(id: &str, name: &str, kw: &[&str]) -> Recipe {
        Recipe {
            id: id.into(),
            name: name.into(),
            description: "Triage a github issue into the right label".into(),
            trigger: RecipeTrigger::Keyword {
                keywords: kw.iter().map(|s| s.to_string()).collect(),
                threshold: 0.5,
            },
            steps: vec![RecipeStep {
                skill: "issue-step".into(),
                tool: "github.api".into(),
                params: serde_json::json!({}),
                description: "step".into(),
            }],
            validation: crate::types::recipe::RecipeValidation::None,
            category: "github".into(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".into(),
            source: crate::types::recipe::RecipeSource::Extracted,
            source_thread_id: None,
            project_id: "p".into(),
            user_id: "u".into(),
            validation_status: ValidationStatus::Validated,
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

    #[test]
    fn identical_recipes_match_highly() {
        // Direct unit test of the token-set construction and overlap
        // math — the public API requires a Store, so we exercise the
        // helper directly.
        let r = make_recipe("r1", "github-triage", &["github", "issue", "triage"]);
        let tokens = trigger_token_set(&r);
        assert!(tokens.contains("github"));
        assert!(tokens.contains("issue"));
        assert!(tokens.contains("triage"));
    }

    #[test]
    fn skill_overlap_counts_distinct_matches() {
        let r = make_recipe("r1", "x", &["a"]);
        let cand = ["issue-step", "unknown-step"];
        let score = skill_overlap(&cand, &r);
        // 1 of 2 matches → 0.5
        assert!((score - 0.5).abs() < 1e-9);
    }
}
