//! Recipe and ToolSkill data types for the Recipe-Skill-Tool learning pipeline.
//!
//! Both types are stored as MemoryDocs with `DocType::Recipe` / `DocType::ToolSkill`
//! so they share the existing dual-backend persistence (PostgreSQL + libSQL) that
//! `brassclaw_engine::memory::store` already implements — no new SQL migrations
//! are required for the v2 design.
//!
//! Recipes are **an ordered sequence of ToolSkill invocations** keyed on a
//! trigger configuration. Once a Recipe's success/failure history crosses the
//! Wilson lower-bound threshold, it becomes eligible for Tier 0 (direct
//! execution with no LLM round-trip). Below that threshold but with a match,
//! ToolSkill summaries are injected into the prompt so the LLM can follow
//! known-good patterns (Tier 1). With no match, the request falls through to
//! full LLM reasoning (Tier 2) and — on success — extraction lifts the thread
//! into a new Recipe + ToolSkill pair.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::project::ProjectId;
use crate::types::ibs::VariablePattern;

/// How a Recipe was sourced.
///
/// `Pattern` triggers are intentionally restricted to human-authored recipes —
/// LLMs are unreliable at writing safe regex (catastrophic backtracking / ReDoS
/// risk) and `RecipeValidator` rejects extracted recipes that try to use them.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecipeSource {
    /// Lifted automatically from a successful thread.
    #[default]
    Extracted,
    /// Hand-written by a user.
    Authored,
    /// Imported from an external source.
    Imported,
}

/// Validation lifecycle for a Recipe or ToolSkill.
///
/// The pipeline is: `Pending → AutoPassed/UpgradeQueued → Validated` (or
/// `Rejected` after 3 failed review cycles, then `Garbage` after the 30-day
/// re-review window elapses without a fix).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    #[default]
    Pending,
    UpgradeQueued,
    AutoFailed,
    AutoPassed,
    Validated,
    ReviewRequested,
    Rejected,
    Garbage,
}

/// Trigger that fires a Recipe.
///
/// Stored as `serde_json::Value::Object("type": …)` so old payloads that
/// stored triggers as plain strings continue to load through the optional
/// `string` deserializer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecipeTrigger {
    /// Case-insensitive equality with the user input.
    Exact { command: String },
    /// Regex pattern (human-authored only — validator blocks this for
    /// extracted recipes to avoid LLM-generated catastrophic backtracking).
    Pattern { patterns: Vec<String> },
    /// Set-overlap matching: Jaccard coefficient vs. trigger keywords.
    Keyword {
        keywords: Vec<String>,
        threshold: f64,
    },
}

impl RecipeTrigger {
    /// Stable, signature used for similarity detection and duplicate checks.
    pub fn signature(&self) -> String {
        match self {
            Self::Exact { command } => format!("exact:{}", command.to_lowercase()),
            Self::Pattern { patterns } => {
                let mut sorted: Vec<&String> = patterns.iter().collect();
                sorted.sort();
                let joined: Vec<&str> = sorted.iter().map(|s| s.as_str()).collect();
                format!("pattern:{}", joined.join("|"))
            }
            Self::Keyword { keywords, .. } => {
                let mut sorted: Vec<&String> = keywords.iter().collect();
                sorted.sort();
                let joined: Vec<&str> = sorted.iter().map(|s| s.as_str()).collect();
                format!("keyword:{}", joined.join("|"))
            }
        }
    }

    /// Flat keyword set used for FTS5 / Jaccard matching.
    pub fn trigger_tokens(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        match self {
            Self::Exact { command } => out.push(command.clone()),
            Self::Pattern { patterns } => out.extend(patterns.iter().cloned()),
            Self::Keyword { keywords, .. } => out.extend(keywords.iter().cloned()),
        }
        out
    }
}

/// Validation check that runs after a Recipe executes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecipeValidation {
    #[default]
    None,
    ShellCheck {
        command: String,
    },
    FileExists {
        path: String,
    },
    Custom {
        code: String,
    },
}

/// Single step inside a Recipe — references a ToolSkill by name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeStep {
    /// Skill name (must match an existing `ToolSkill.name`).
    pub skill: String,
    /// Tool name to call (denormalized from the referenced skill for cheap
    /// lookup — `RecipeStage` doesn't have to load the full skill just to
    /// resolve the tool).
    pub tool: String,
    /// Parameter overrides used for Tier 0 direct execution.
    pub params: serde_json::Value,
    /// Human-readable description (used in Tier 1 prompt injection).
    pub description: String,
}

/// One variant of a Recipe — a distinct intent the recipe serves (§0.3, §0.16.1).
///
/// Persisted in the `variants` JSONB column of `reborn_recipes` (V050). The
/// canonical authoring model (FIND-P5-03): `variant_key` is the only
/// human-readable identifier; `step_link` is nullable because legacy Recipe
/// rows have no step_link until Phase D re-seeds their intent examples.
/// `variable_patterns` is empty = positional auto-extraction only (§0.17.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeVariant {
    /// Human-readable variant identifier (e.g. "ls-la"). Used by WebUI only.
    pub variant_key: String,
    /// Concise human-readable explanation of what this variant does — the
    /// human-readable side of the dual-nature recipe syntax (Step B). The
    /// machine-readable side is `step_link` + IBS `step_descriptions` →
    /// `build_instruction` (untouched). `#[serde(default)]` so legacy rows
    /// deserialise unchanged; the Q1 validation gate exempts legacy rows.
    #[serde(default)]
    pub description: Option<String>,
    /// Direct IBS input — the step_link formula for this variant.
    /// `None` for legacy variants not yet migrated to v3 intent inputs.
    pub step_link: Option<String>,
    /// Intent expressions for this variant — seeded into `reborn_intent_inputs`
    /// at Q2 graduation (Phase N, §0.23.5), not on save (FIND-NEW-17).
    #[serde(default)]
    pub intent_examples: Vec<String>,
    /// Optional post-extraction refinement for slot values (§0.17.3).
    /// Empty = positional auto-extraction only.
    #[serde(default)]
    pub variable_patterns: Vec<VariablePattern>,
}

/// Recipe: ordered sequence of ToolSkill invocations plus a trigger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: RecipeTrigger,
    pub steps: Vec<RecipeStep>,
    pub validation: RecipeValidation,
    pub category: String,

    // Metrics — Wilson-scored.
    pub usage_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub wilson_lower: f64,
    /// Maturity tier (Seedling / Growing / Mature / Candidate).
    pub tier: String,

    // Lifecycle.
    pub source: RecipeSource,
    pub source_thread_id: Option<String>,
    pub project_id: String,
    pub user_id: String,
    pub validation_status: ValidationStatus,
    pub validation_errors: Vec<String>,
    pub review_feedback: Option<String>,
    pub review_attempts: u32,
    pub rejected_at: Option<DateTime<Utc>>,
    pub similarity_parent_id: Option<String>,
    pub skip_similarity: bool,
    pub last_audit_at: Option<DateTime<Utc>>,
    pub audit_failure_count: u32,
    pub replaces_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // v3 authoring model (Phase A). All `#[serde(default)]` so legacy rows
    // stored as `MemoryDoc.metadata` (StoreBackedRecipeStore path) deserialise
    // unchanged (FIND-P6-10). `step_descriptions` + `dependency_registry` are
    // raw JSONB — `fetch_for_turn` parses `step_descriptions` into
    // `Vec<StepDescriptionEntry>` before calling `build_instruction` (Phase E).
    /// `Vec<RecipeVariant>` — the variant table for this recipe (§0.16.1).
    #[serde(default)]
    pub variants: Vec<RecipeVariant>,
    /// Authored StepDescription array (§0.4.1) — the IBS authoring model.
    #[serde(default)]
    pub step_descriptions: serde_json::Value,
    /// Per-component flat dependency graph (§0.19). Also on the other 12
    /// component tables from V055 (Phase J.2); on `reborn_recipes` from V050.
    #[serde(default)]
    pub dependency_registry: serde_json::Value,
}

impl Recipe {
    /// Observed success rate. Returns `0.0` when no interactions have been
    /// recorded — no evidence means no confidence, not perfect confidence.
    /// Tier 0 eligibility is gated separately by `wilson_lower ≥ 0.70` and
    /// a minimum usage count, so callers that need to distinguish
    /// "new recipe" from "high-confidence recipe" should use `wilson_lower`.
    pub fn confidence(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            return 0.0;
        }
        self.success_count as f64 / total as f64
    }

    /// Tier 0 eligibility: a recipe may be executed without LLM round-trip
    /// when its maturity tier is mature/candidate AND the Wilson lower
    /// bound on its success rate is at least 70 % AND a validation hook
    /// is wired up.
    pub fn is_tier0_eligible(&self) -> bool {
        let tier_mature = matches!(self.tier.as_str(), "mature" | "candidate");
        let validated = matches!(self.validation_status, ValidationStatus::Validated);
        let has_validation = !matches!(self.validation, RecipeValidation::None);
        tier_mature && validated && self.wilson_lower >= 0.70 && has_validation
    }

    /// `Recipe::from_metadata` reverses `to_metadata` — used by
    /// `recipe_matcher` and `recipe_library` to lift a `MemoryDoc` back
    /// into the typed Recipe struct.
    pub fn from_metadata(meta: &serde_json::Value) -> Result<Self, RecipeError> {
        serde_json::from_value(meta.clone()).map_err(|e| RecipeError::Decode {
            reason: e.to_string(),
        })
    }

    /// `to_metadata` is the inverse — store as `MemoryDoc.metadata`.
    pub fn to_metadata(&self) -> Result<serde_json::Value, RecipeError> {
        serde_json::to_value(self).map_err(|e| RecipeError::Encode {
            reason: e.to_string(),
        })
    }
}

/// One schema entry on a ToolSkill parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSkillParam {
    pub name: String,
    pub param_type: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
}

/// Tight description of ONE tool usage pattern.
///
/// Token-budget target: keep under 5 000 tokens (agentskills.io progressive
/// disclosure) — `RecipeValidator` enforces the ceiling at extraction time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSkill {
    pub id: String,
    pub name: String,
    pub tool_name: String,
    pub description: String,
    pub param_template: serde_json::Value,
    pub param_schema: Vec<ToolSkillParam>,
    pub preconditions: String,
    pub error_handling: String,
    pub code_snippet: Option<String>,
    pub category: String,

    /// C.4.5.3 — component UUIDs the composer inlines for `{{component_name}}`
    /// structural-include placeholders in this ToolSkill's description text (a
    /// description may include another description). Mirrors PythonCode
    /// `includes` (C.4.5.2) + recipe `StepEntry.include`. Empty for leaf
    /// descriptions. Referential placeholder<->include matching is deferred to
    /// Phase I/N; Q1 validates non-nil UUIDs only.
    #[serde(default)]
    pub includes: Vec<uuid::Uuid>,

    pub usage_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub wilson_lower: f64,
    pub tier: String,

    pub source: RecipeSource,
    pub source_thread_id: Option<String>,
    pub project_id: String,
    pub user_id: String,
    pub validation_status: ValidationStatus,
    pub validation_errors: Vec<String>,
    pub review_feedback: Option<String>,
    pub review_attempts: u32,
    pub rejected_at: Option<DateTime<Utc>>,
    pub similarity_parent_id: Option<String>,
    pub skip_similarity: bool,
    pub last_audit_at: Option<DateTime<Utc>>,
    pub audit_failure_count: u32,
    pub replaces_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ToolSkill {
    /// Observed success rate. Returns `0.0` when no interactions have been
    /// recorded. See `Recipe::confidence` for the rationale.
    pub fn confidence(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            return 0.0;
        }
        self.success_count as f64 / total as f64
    }

    /// Approximate token cost when injected as Tier 1 prompt content.
    /// 4 chars ≈ 1 token (rough heuristic; matches `RecipeValidator` ceiling).
    pub fn estimated_tokens(&self) -> usize {
        let mut total_chars =
            self.description.len() + self.preconditions.len() + self.error_handling.len();
        if let Some(snippet) = &self.code_snippet {
            total_chars += snippet.len();
        }
        total_chars += self.param_template.to_string().len();
        for param in &self.param_schema {
            total_chars += param.name.len() + param.param_type.len() + param.description.len();
        }
        total_chars / 4
    }

    pub fn from_metadata(meta: &serde_json::Value) -> Result<Self, RecipeError> {
        serde_json::from_value(meta.clone()).map_err(|e| RecipeError::Decode {
            reason: e.to_string(),
        })
    }

    pub fn to_metadata(&self) -> Result<serde_json::Value, RecipeError> {
        serde_json::to_value(self).map_err(|e| RecipeError::Encode {
            reason: e.to_string(),
        })
    }
}

/// Convert a `Recipe` into a `MemoryDoc` with `DocType::Recipe`.
///
/// `project_id` is kept as `String` (the typed `ProjectId` lives in the
/// MemoryDoc struct, not the Recipe metadata) — when round-tripping through
/// `Recipe::from_metadata`, callers reconstruct the typed `ProjectId`.
pub fn recipe_to_memory_doc(
    recipe: &Recipe,
    project_id: ProjectId,
    content: impl Into<String>,
) -> crate::types::memory::MemoryDoc {
    let mut doc = crate::types::memory::MemoryDoc::new(
        project_id,
        recipe.user_id.clone(),
        crate::types::memory::DocType::Recipe,
        format!("recipe:{}", recipe.name),
        content,
    );
    doc.metadata = recipe
        .to_metadata()
        .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
    doc.source_thread_id = recipe
        .source_thread_id
        .as_ref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(crate::types::thread::ThreadId);
    doc.tags = vec!["recipe".to_string(), recipe.category.clone()];
    doc
}

/// Convert a `ToolSkill` into a `MemoryDoc` with `DocType::ToolSkill`.
pub fn tool_skill_to_memory_doc(
    skill: &ToolSkill,
    project_id: ProjectId,
    content: impl Into<String>,
) -> crate::types::memory::MemoryDoc {
    let mut doc = crate::types::memory::MemoryDoc::new(
        project_id,
        skill.user_id.clone(),
        crate::types::memory::DocType::ToolSkill,
        format!("skill:{}", skill.name),
        content,
    );
    doc.metadata = skill
        .to_metadata()
        .unwrap_or_else(|_| serde_json::Value::Object(serde_json::Map::new()));
    doc.source_thread_id = skill
        .source_thread_id
        .as_ref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(crate::types::thread::ThreadId);
    doc.tags = vec!["tool_skill".to_string(), skill.category.clone()];
    doc
}

/// Errors that can arise while (de)serializing Recipe/ToolSkill payloads.
#[derive(Debug, thiserror::Error)]
pub enum RecipeError {
    #[error("recipe/skill metadata decode failed: {reason}")]
    Decode { reason: String },
    #[error("recipe/skill metadata encode failed: {reason}")]
    Encode { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default "limit" param in GitHub issue action test fixtures.
    const TEST_ISSUE_LIMIT: u64 = 50;
    /// High Wilson lower bound (tier-candidate level) used in tier tests.
    const WILSON_HIGH: f64 = 0.95;
    /// Mid Wilson lower bound (tier-growing level) used in tier tests.
    const WILSON_MID: f64 = 0.50;
    /// Near-mature Wilson lower bound used in ordering tests.
    const WILSON_NEAR_MATURE: f64 = 0.78;

    fn make_recipe() -> Recipe {
        Recipe {
            id: "r1".into(),
            name: "github-issue-triage".into(),
            description:
                "Triage new GitHub issues by severity and triage them into the right label".into(),
            trigger: RecipeTrigger::Keyword {
                keywords: vec!["github".into(), "issue".into(), "triage".into()],
                threshold: 0.5,
            },
            steps: vec![RecipeStep {
                skill: "github-list-issues".into(),
                tool: "github.api".into(),
                params: serde_json::json!({"state": "open", "limit": TEST_ISSUE_LIMIT}),
                description: "List open issues".into(),
            }],
            validation: RecipeValidation::ShellCheck {
                command: "gh issue list --limit 1".into(),
            },
            category: "github".into(),
            usage_count: 25,
            success_count: 23,
            failure_count: 2,
            wilson_lower: 0.78,
            tier: "mature".into(),
            source: RecipeSource::Extracted,
            source_thread_id: None,
            project_id: "user-proj".into(),
            user_id: "user-1".into(),
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
            variants: Vec::new(),
            step_descriptions: serde_json::Value::Null,
            dependency_registry: serde_json::Value::Null,
        }
    }

    #[test]
    fn confidence_handles_zero_samples() {
        let mut r = make_recipe();
        r.usage_count = 0;
        r.success_count = 0;
        r.failure_count = 0;
        assert_eq!(r.confidence(), 0.0);
    }

    #[test]
    fn confidence_computes_proportion() {
        let r = make_recipe();
        assert!((r.confidence() - 23.0 / 25.0).abs() < 1e-9);
    }

    #[test]
    fn tier0_eligibility_requires_tier_wilson_and_validation() {
        let mut r = make_recipe();
        r.tier = "seedling".into();
        r.wilson_lower = WILSON_HIGH;
        r.validation = RecipeValidation::ShellCheck {
            command: "true".into(),
        };
        assert!(!r.is_tier0_eligible(), "tier=seedling must block");

        r.tier = "mature".into();
        assert!(r.is_tier0_eligible());

        r.wilson_lower = WILSON_MID;
        assert!(!r.is_tier0_eligible(), "wilson<0.70 must block");

        r.wilson_lower = WILSON_NEAR_MATURE;
        r.validation = RecipeValidation::None;
        assert!(
            !r.is_tier0_eligible(),
            "validation=None must block direct exec"
        );
    }

    #[test]
    fn trigger_signature_stable_across_keyword_order() {
        let a = RecipeTrigger::Keyword {
            keywords: vec!["b".into(), "a".into()],
            threshold: 0.5,
        };
        let b = RecipeTrigger::Keyword {
            keywords: vec!["a".into(), "b".into()],
            threshold: 0.9,
        };
        assert_eq!(a.signature(), b.signature());
    }

    #[test]
    fn tool_skill_token_estimate_uses_4_chars_per_token() {
        let s = ToolSkill {
            id: "s1".into(),
            name: "git-status".into(),
            tool_name: "builtin.shell".into(),
            description: "Run git status to inspect the working tree".into(),
            param_template: serde_json::json!({}),
            param_schema: vec![],
            preconditions: "git repo".into(),
            error_handling: "exit non-zero means dirty tree".into(),
            code_snippet: None,
            category: "git".into(),
            includes: vec![],
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".into(),
            source: RecipeSource::Extracted,
            source_thread_id: None,
            project_id: "u".into(),
            user_id: "u".into(),
            validation_status: ValidationStatus::Pending,
            validation_errors: vec![],
            review_feedback: None,
            review_attempts: 0,
            rejected_at: None,
            similarity_parent_id: None,
            skip_similarity: false,
            last_audit_at: None,
            audit_failure_count: 0,
            replaces_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        // Total chars ≈ 100+8+30+0+2=140; estimate = 35.
        let estimate = s.estimated_tokens();
        assert!(estimate > 0 && estimate < 200);
    }

    #[test]
    fn recipe_round_trips_through_metadata() {
        let r = make_recipe();
        let meta = r.to_metadata().unwrap();
        let r2 = Recipe::from_metadata(&meta).unwrap();
        assert_eq!(r.id, r2.id);
        assert_eq!(r.name, r2.name);
        assert_eq!(r.steps.len(), r2.steps.len());
        assert_eq!(r.trigger, r2.trigger);
        assert_eq!(r.validation, r2.validation);
    }

    #[test]
    fn recipe_variant_description_round_trips() {
        let v = RecipeVariant {
            variant_key: "ls-la".into(),
            description: Some("List a directory including hidden files.".into()),
            step_link: Some("0:0-0:30+1:0-1:E".into()),
            intent_examples: vec!["list files in {{dir}}".into()],
            variable_patterns: vec![],
        };
        let json = serde_json::to_string(&v).expect("serialize");
        let v2: RecipeVariant = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(v, v2);
        assert_eq!(
            v2.description.as_deref(),
            Some("List a directory including hidden files.")
        );
    }

    #[test]
    fn recipe_variant_description_legacy_defaults_to_none() {
        // Legacy rows persisted before Step B have no `description` key.
        let json = r#"{"variant_key":"ls-la","step_link":"0:0-0:30+1:0-1:E","intent_examples":[],"variable_patterns":[]}"#;
        let v: RecipeVariant = serde_json::from_str(json).expect("deserialize legacy");
        assert_eq!(v.variant_key, "ls-la");
        assert_eq!(v.description, None);
    }
}
