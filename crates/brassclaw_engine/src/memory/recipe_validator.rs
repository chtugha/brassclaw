//! Auto-validation for newly-extracted Recipes and ToolSkills (Step 1 of
//! the two-step validation pipeline).
//!
//! Returns a [`ValidationResult`] distinguishing hard **errors** (blocking
//! the item from going live) from soft **warnings** (cosmetic / review-able).
//!
//! Standards aligned with [agentskills.io](https://agentskills.io/specification):
//! - Name format: `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$`, 1–64 chars
//! - Description: 1–1024 chars, must contain at least one actionable verb
//! - Token budget: ≤ 5 000 tokens (progressive disclosure)
//! - Coherent units: each Skill covers ONE tool usage pattern

use crate::types::recipe::{
    Recipe, RecipeSource, RecipeTrigger, RecipeValidation, ToolSkill, ToolSkillParam,
    ValidationStatus,
};

#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn ok() -> Self {
        Self::default()
    }

    pub fn from_error(error: impl Into<String>) -> Self {
        Self {
            errors: vec![error.into()],
            warnings: vec![],
        }
    }
}

/// Pure-function validator — no I/O, no LLM.
pub struct RecipeValidator;

impl RecipeValidator {
    /// Validate a ToolSkill.
    ///
    /// `available_tools` should be the list of tool names registered in
    /// the current capability surface; empty list allows a "structural"
    /// validation pass that callers can re-run when tool inventory changes.
    pub fn validate_tool_skill(skill: &ToolSkill, available_tools: &[String]) -> ValidationResult {
        let mut result = ValidationResult::ok();

        check_name_format(&skill.name, "ToolSkill", &mut result);
        check_description_length(&skill.description, "ToolSkill", &mut result);
        check_description_actionable(&skill.description, "ToolSkill", &mut result);

        let tokens = skill.estimated_tokens();
        if tokens > 5000 {
            result.errors.push(format!(
                "ToolSkill exceeds 5000 token budget ({tokens} tokens). Split into smaller skills or move detail to reference files."
            ));
        }

        if skill.tool_name.is_empty() {
            result
                .errors
                .push("ToolSkill.tool_name must not be empty".to_string());
        } else if !available_tools.is_empty()
            && !available_tools.iter().any(|t| t == &skill.tool_name)
        {
            result.errors.push(format!(
                "ToolSkill.tool_name '{}' is not present in the capability surface",
                skill.tool_name
            ));
        }

        if !skill.param_template.is_object() {
            result
                .errors
                .push("ToolSkill.param_template must be a JSON object".to_string());
        }

        for (i, p) in skill.param_schema.iter().enumerate() {
            check_param_schema_entry(p, i, &mut result);
        }

        if tool_name_count(&skill.description) > 3 {
            result.warnings.push(
                "ToolSkill may cover too many tools. Consider splitting into focused units."
                    .to_string(),
            );
        }

        if !matches!(
            skill.validation_status,
            ValidationStatus::Validated | ValidationStatus::Pending
        ) {
            result.warnings.push(format!(
                "ToolSkill has unexpected validation_status {:?} for re-validation",
                skill.validation_status
            ));
        }

        result
    }

    /// Validate a Recipe.
    ///
    /// `existing_skill_names` is the list of currently-validated Skill
    /// names; a Recipe referencing an unknown skill is hard-failed.
    pub fn validate_recipe(recipe: &Recipe, existing_skill_names: &[String]) -> ValidationResult {
        let mut result = ValidationResult::ok();

        check_name_format(&recipe.name, "Recipe", &mut result);
        check_description_length(&recipe.description, "Recipe", &mut result);

        if recipe.steps.is_empty() {
            result
                .errors
                .push("Recipe must have at least one step".to_string());
        }
        for (i, step) in recipe.steps.iter().enumerate() {
            if step.skill.is_empty() {
                result
                    .errors
                    .push(format!("step #{i} has empty skill name"));
                continue;
            }
            if !existing_skill_names.is_empty()
                && !existing_skill_names.iter().any(|n| n == &step.skill)
            {
                result.errors.push(format!(
                    "step #{i} references unknown skill '{}'",
                    step.skill
                ));
            }
            if step.tool.is_empty() {
                result
                    .errors
                    .push(format!("step #{i} tool must not be empty"));
            }
        }

        check_trigger(&recipe.trigger, &recipe.source, &mut result);

        if matches!(recipe.tier.as_str(), "growing" | "mature" | "candidate")
            && matches!(recipe.validation, RecipeValidation::None)
        {
            result.warnings.push(
                "Recipe at Growing+ tier has no validation — risky for Tier 0 direct execution"
                    .to_string(),
            );
        }

        result
    }
}

fn check_name_format(name: &str, kind: &str, result: &mut ValidationResult) {
    if name.is_empty() {
        result.errors.push(format!("{kind} name must not be empty"));
        return;
    }
    if name.len() > 64 {
        result.errors.push(format!(
            "{kind} name exceeds 64 chars ({} chars)",
            name.len()
        ));
    }
    if name.contains("--") {
        result
            .errors
            .push(format!("{kind} name contains consecutive hyphens '--'"));
    }
    if name.starts_with('-') || name.ends_with('-') {
        result
            .errors
            .push(format!("{kind} name must not start or end with '-'"));
    }
    for ch in name.chars() {
        let valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !valid {
            result.errors.push(format!(
                "{kind} name '{name}' contains invalid character '{ch}' — must match ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$"
            ));
            break;
        }
    }
}

fn check_description_length(desc: &str, kind: &str, result: &mut ValidationResult) {
    let trimmed_len = desc.trim().chars().count();
    if trimmed_len == 0 {
        result
            .errors
            .push(format!("{kind} description must not be empty"));
    } else if trimmed_len > 1024 {
        result.errors.push(format!(
            "{kind} description exceeds 1024 chars ({trimmed_len} chars)"
        ));
    }
}

fn check_description_actionable(desc: &str, kind: &str, result: &mut ValidationResult) {
    use std::sync::OnceLock;
    static VERB_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = VERB_RE.get_or_init(|| {
        regex::Regex::new(
            r"\b(use|run|create|check|extract|process|analyze|configure|list|fetch|send|compute|apply|build|deploy|format|validate|inspect|open|close|delete|update|render|compile|test|sign)\b",
        )
        .expect("actionable-verb regex is a compile-time literal — infallible")
    });
    let has_verb = re.is_match(&desc.to_lowercase());
    if !has_verb {
        result.warnings.push(format!(
            "{kind} description does not contain an actionable verb — consider 'Use when …' phrasing"
        ));
    }
}

fn check_trigger(trigger: &RecipeTrigger, source: &RecipeSource, result: &mut ValidationResult) {
    match trigger {
        RecipeTrigger::Exact { command } => {
            if command.is_empty() {
                result
                    .errors
                    .push("Exact trigger command must not be empty".to_string());
            } else if command.len() > 200 {
                result.errors.push(format!(
                    "Exact trigger command exceeds 200 chars ({} chars)",
                    command.len()
                ));
            }
        }
        RecipeTrigger::Pattern { patterns } => {
            if !matches!(source, RecipeSource::Authored) {
                result.errors.push(
                    "Pattern triggers are restricted to human-authored recipes (Extracted/Imported rejected)".to_string()
                );
            }
            for (i, p) in patterns.iter().enumerate() {
                if p.is_empty() {
                    result.errors.push(format!("Pattern[#{i}] is empty"));
                } else if let Err(error) = regex::RegexBuilder::new(p).size_limit(10_000).build() {
                    result
                        .errors
                        .push(format!("Pattern[#{i}] regex invalid: {error}"));
                }
            }
        }
        RecipeTrigger::Keyword {
            keywords,
            threshold,
        } => {
            if keywords.is_empty() {
                result
                    .errors
                    .push("Keyword trigger must have at least one keyword".to_string());
            }
            if !(0.0..=1.0).contains(threshold) {
                result.errors.push(format!(
                    "Keyword trigger threshold {threshold} out of [0.0, 1.0] range"
                ));
            }
        }
    }
}

fn check_param_schema_entry(param: &ToolSkillParam, index: usize, result: &mut ValidationResult) {
    if param.name.is_empty() {
        result
            .errors
            .push(format!("param_schema[#{index}].name empty"));
    }
    if param.param_type.is_empty() {
        result
            .errors
            .push(format!("param_schema[#{index}].param_type empty"));
    }
    if param.description.is_empty() {
        result.warnings.push(format!(
            "param_schema[#{index}].description is empty — LLM has no behavioural guidance"
        ));
    }
}

/// Approximate count of distinct tool names mentioned in freeform text.
///
/// Heuristic: split on whitespace/punctuation and look for snake_case
/// fragments with at least one underscore — agentskills.io tools follow
/// that convention (`builtin.shell`, `github.api`, etc.).
fn tool_name_count(description: &str) -> usize {
    let mut count = 0;
    let mut seen = std::collections::HashSet::new();
    for token in description.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.') {
        if token.len() < 3 || !token.contains('_') {
            continue;
        }
        if seen.insert(token.to_string()) {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::recipe::{RecipeSource, RecipeTrigger, RecipeValidation};

    fn valid_skill_name() -> &'static str {
        "git-status-summary"
    }

    fn base_skill() -> ToolSkill {
        ToolSkill {
            id: "s1".into(),
            name: valid_skill_name().into(),
            tool_name: "builtin.shell".into(),
            description: "Run git status to inspect the working tree and summarize dirty paths"
                .into(),
            param_template: serde_json::json!({}),
            param_schema: vec![ToolSkillParam {
                name: "path".into(),
                param_type: "string".into(),
                description: "Repo root path".into(),
                required: false,
            }],
            preconditions: "git repo".into(),
            error_handling: "exit non-zero => dirty".into(),
            code_snippet: None,
            category: "git".into(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".into(),
            source: RecipeSource::Extracted,
            source_thread_id: None,
            project_id: "p".into(),
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
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn valid_skill_passes() {
        let s = base_skill();
        let tools = vec!["builtin.shell".to_string()];
        let result = RecipeValidator::validate_tool_skill(&s, &tools);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn skill_with_uppercase_name_fails() {
        let mut s = base_skill();
        s.name = "Git-Status".into();
        let result = RecipeValidator::validate_tool_skill(&s, &[]);
        assert!(!result.is_ok());
        let joined = result.errors.join("|");
        assert!(
            joined.contains("invalid character"),
            "expected format error, got {joined}"
        );
    }

    #[test]
    fn skill_with_consecutive_hyphens_fails() {
        let mut s = base_skill();
        s.name = "git--status".into();
        let result = RecipeValidator::validate_tool_skill(&s, &[]);
        assert!(result.errors.iter().any(|e| e.contains("consecutive")));
    }

    #[test]
    fn skill_over_token_budget_fails() {
        let mut s = base_skill();
        s.description = "x".repeat(20_001);
        let tools = vec!["builtin.shell".to_string()];
        let result = RecipeValidator::validate_tool_skill(&s, &tools);
        assert!(result.errors.iter().any(|e| e.contains("token budget")));
    }

    #[test]
    fn skill_with_unknown_tool_fails() {
        let s = base_skill();
        let _result = RecipeValidator::validate_tool_skill(&s, &[]);
        // Empty tool list doesn't gate against unknown tools; supply a list
        // to exercise the gating path.
        let result = RecipeValidator::validate_tool_skill(&s, &["github.api".into()]);
        assert!(
            result.errors.iter().any(|e| e.contains("not present")),
            "{:?}",
            result
        );
    }

    #[test]
    fn recipe_missing_step_skill_reference_fails() {
        let mut r = base_recipe();
        r.steps[0].skill = "unknown-skill".into();
        let result = RecipeValidator::validate_recipe(&r, &["other-skill".into()]);
        assert!(result.errors.iter().any(|e| e.contains("unknown skill")));
    }

    #[test]
    fn recipe_with_pattern_trigger_extracted_fails() {
        let mut r = base_recipe();
        r.trigger = RecipeTrigger::Pattern {
            patterns: vec!["^npm +install".to_string()],
        };
        r.source = RecipeSource::Extracted;
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("Pattern triggers are restricted"))
        );
    }

    #[test]
    fn recipe_with_pattern_trigger_authored_passes() {
        let mut r = base_recipe();
        r.trigger = RecipeTrigger::Pattern {
            patterns: vec!["git (status|diff)".to_string()],
        };
        r.source = RecipeSource::Authored;
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn empty_recipe_steps_fail() {
        let mut r = base_recipe();
        r.steps.clear();
        let result = RecipeValidator::validate_recipe(&r, &[]);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("at least one step"))
        );
    }

    fn base_recipe() -> Recipe {
        Recipe {
            id: "r1".into(),
            name: "github-issue-triage".into(),
            description: "Triage new GitHub issues by severity and label them".into(),
            trigger: RecipeTrigger::Keyword {
                keywords: vec!["github".into(), "issue".into(), "triage".into()],
                threshold: 0.5,
            },
            steps: vec![crate::types::recipe::RecipeStep {
                skill: "step-skill".into(),
                tool: "github.api".into(),
                params: serde_json::json!({}),
                description: "List open issues".into(),
            }],
            validation: RecipeValidation::ShellCheck {
                command: "true".into(),
            },
            category: "github".into(),
            usage_count: 25,
            success_count: 23,
            failure_count: 2,
            wilson_lower: 0.78,
            tier: "mature".into(),
            source: RecipeSource::Extracted,
            source_thread_id: None,
            project_id: "p".into(),
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
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}
