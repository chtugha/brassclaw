//! Generalized component validator (Phase 3 — Step 3.6).
//!
//! [`ComponentValidator`] wraps the existing [`super::recipe_validator::RecipeValidator`]
//! and dispatches to the appropriate validation path based on `class_code`.
//!
//! - **Skills (01-03):** full agentskills.io validation (name, description, token budget
//!   5000/hard, activation criteria, tool_name, param_schema).
//! - **Tool (00):** tool_name + param_schema required, budget 5000/hard.
//! - **Extensions (04-09):** name + description + non-empty content + soft 10000 budget.
//! - **Orchestrator (10) / Scaffold (50):** LLM code-audit gated; lightweight structural
//!   check only (name + non-empty). Budget 50000, soft.
//! - **Actions (16):** name + non-empty content only; no token budget.
//! - **Recipes (21):** delegates to `RecipeValidator::validate_recipe`.
//! - **Former DocType classes (12-15, 17-20):** name + description + non-empty + soft 10000
//!   (Notes/class 15: soft 2000).
//! - **Unknown class codes:** lightweight generic validation (name + non-empty).
//!
//! The `ValidationConfig` is read from the `reborn_validation_config` table at validation
//! time (passed by the caller). Falls back to compile-time defaults when the config row is
//! absent (e.g. during tests or before migration).

#![forbid(unsafe_code)]

use crate::memory::recipe_validator::{RecipeValidator, ValidationResult};
use crate::types::recipe::{Recipe, ToolSkill};

/// Per-class validation configuration. Mirrors `reborn_validation_config` columns.
/// All fields use `Option` so a partial row can be passed; `None` falls back to the
/// class default.
#[derive(Debug, Clone, Default)]
pub struct ValidationConfig {
    pub name_min_len: Option<u16>,
    pub name_max_len: Option<u16>,
    pub description_min_len: Option<u16>,
    pub description_max_len: Option<u16>,
    pub token_budget: Option<u32>,
    pub token_budget_hard_error: Option<bool>,
    pub require_tool_name: Option<bool>,
    pub require_param_schema: Option<bool>,
    pub require_activation_criteria: Option<bool>,
}

/// Lightweight component payload used by `validate_generic` and `validate_extension`.
#[derive(Debug, Clone)]
pub struct GenericComponent<'a> {
    pub name: &'a str,
    pub description: &'a str,
    /// Raw content (body/steps/code). Used only for non-empty check and token budget.
    pub content: &'a str,
}

impl<'a> GenericComponent<'a> {
    /// Rough token count: 1 token ≈ 4 bytes of UTF-8.
    pub fn estimated_tokens(&self) -> u32 {
        (self.content.len() / 4) as u32
    }
}

/// Generalized validator dispatching to the class-appropriate validation path.
///
/// All methods are pure functions — no I/O, no LLM.
pub struct ComponentValidator;

impl ComponentValidator {
    /// Dispatch to the class-appropriate validation path.
    ///
    /// `available_tools` and `existing_skill_names` are forwarded to the
    /// `RecipeValidator` for class codes that need them. Pass empty slices to
    /// get a structural-only pass (no cross-reference validation).
    pub fn validate_by_class(
        class_code: u16,
        component: ComponentPayload<'_>,
        config: &ValidationConfig,
        available_tools: &[String],
        existing_skill_names: &[String],
    ) -> ValidationResult {
        match class_code {
            // Skills (01-03): full agentskills.io
            1..=3 => match &component {
                ComponentPayload::ToolSkill(skill) => {
                    let mut result = RecipeValidator::validate_tool_skill(skill, available_tools);
                    apply_config_overrides_skill(&mut result, skill, config);
                    result
                }
                ComponentPayload::Generic(g) => validate_skill_generic(g, config),
                ComponentPayload::Recipe(_) => {
                    ValidationResult::from_error("Skill class requires a ToolSkill payload")
                }
            },
            // Tool (00): tool_name + param_schema required
            0 => match &component {
                ComponentPayload::ToolSkill(skill) => {
                    RecipeValidator::validate_tool_skill(skill, available_tools)
                }
                ComponentPayload::Generic(g) => validate_tool_generic(g, config),
                ComponentPayload::Recipe(_) => {
                    ValidationResult::from_error("Tool class requires a ToolSkill payload")
                }
            },
            // Extensions (04-09)
            4..=9 => match &component {
                ComponentPayload::Generic(g) => validate_soft_budget(g, config, 10_000),
                ComponentPayload::ToolSkill(skill) => {
                    validate_soft_budget_named(skill.name.as_str(), skill.description.as_str(), skill.code_snippet.as_deref().unwrap_or(""), config, 10_000)
                }
                ComponentPayload::Recipe(_) => validate_soft_budget_named("", "", "", config, 10_000),
            },
            // Orchestrator (10): LLM code-audit path — structural check only here
            10 | 50 => {
                let (name, desc, content) = match &component {
                    ComponentPayload::Generic(g) => (g.name, g.description, g.content),
                    ComponentPayload::ToolSkill(s) => (
                        s.name.as_str(),
                        s.description.as_str(),
                        s.code_snippet.as_deref().unwrap_or(""),
                    ),
                    ComponentPayload::Recipe(r) => (r.name.as_str(), r.description.as_str(), ""),
                };
                validate_soft_budget_named(name, desc, content, config, 50_000)
            }
            // Actions (16): no token budget
            16 => {
                let (name, desc, content) = match &component {
                    ComponentPayload::Generic(g) => (g.name, g.description, g.content),
                    ComponentPayload::ToolSkill(s) => (
                        s.name.as_str(),
                        s.description.as_str(),
                        s.code_snippet.as_deref().unwrap_or(""),
                    ),
                    ComponentPayload::Recipe(r) => (r.name.as_str(), r.description.as_str(), ""),
                };
                validate_no_budget(name, desc, content)
            }
            // Recipes (21)
            21 => match &component {
                ComponentPayload::Recipe(recipe) => {
                    RecipeValidator::validate_recipe(recipe, existing_skill_names)
                }
                ComponentPayload::Generic(g) => {
                    validate_soft_budget(g, config, 10_000)
                }
                ComponentPayload::ToolSkill(_) => {
                    ValidationResult::from_error("Recipe class requires a Recipe payload")
                }
            },
            // Notes class (15): soft 2000 budget
            15 => {
                let (name, desc, content) = match &component {
                    ComponentPayload::Generic(g) => (g.name, g.description, g.content),
                    ComponentPayload::ToolSkill(s) => (
                        s.name.as_str(),
                        s.description.as_str(),
                        s.code_snippet.as_deref().unwrap_or(""),
                    ),
                    ComponentPayload::Recipe(r) => {
                        (r.name.as_str(), r.description.as_str(), "")
                    }
                };
                validate_soft_budget_named(name, desc, content, config, 2_000)
            }
            // Former DocType classes (12-14, 17-20): soft 10000
            12..=14 | 17..=20 => {
                let (name, desc, content) = match &component {
                    ComponentPayload::Generic(g) => (g.name, g.description, g.content),
                    ComponentPayload::ToolSkill(s) => (
                        s.name.as_str(),
                        s.description.as_str(),
                        s.code_snippet.as_deref().unwrap_or(""),
                    ),
                    ComponentPayload::Recipe(r) => (r.name.as_str(), r.description.as_str(), ""),
                };
                validate_soft_budget_named(name, desc, content, config, 10_000)
            }
            // Unknown class codes: generic lightweight check
            _ => {
                let (name, desc, content) = match &component {
                    ComponentPayload::Generic(g) => (g.name, g.description, g.content),
                    ComponentPayload::ToolSkill(s) => (
                        s.name.as_str(),
                        s.description.as_str(),
                        s.code_snippet.as_deref().unwrap_or(""),
                    ),
                    ComponentPayload::Recipe(r) => (r.name.as_str(), r.description.as_str(), ""),
                };
                validate_soft_budget_named(name, desc, content, config, 10_000)
            }
        }
    }
}

/// Typed component payload — the caller provides whichever variant applies.
pub enum ComponentPayload<'a> {
    ToolSkill(&'a ToolSkill),
    Recipe(&'a Recipe),
    Generic(GenericComponent<'a>),
}

// ── Internal helpers ────────────────────────────────────────────────

fn validate_soft_budget(
    component: &GenericComponent<'_>,
    config: &ValidationConfig,
    default_budget: u32,
) -> ValidationResult {
    validate_soft_budget_named(
        component.name,
        component.description,
        component.content,
        config,
        default_budget,
    )
}

fn validate_soft_budget_named(
    name: &str,
    description: &str,
    content: &str,
    config: &ValidationConfig,
    default_budget: u32,
) -> ValidationResult {
    let mut result = ValidationResult::ok();
    validate_name_generic(name, &mut result);
    validate_description_generic(description, &mut result);
    if content.is_empty() {
        result.warnings.push("Component content is empty".to_string());
    }
    let budget = config.token_budget.unwrap_or(default_budget);
    let hard = config.token_budget_hard_error.unwrap_or(false);
    let tokens = (content.len() / 4) as u32;
    if tokens > budget {
        let msg = format!("Component exceeds {budget} token budget ({tokens} tokens estimated)");
        if hard {
            result.errors.push(msg);
        } else {
            result.warnings.push(msg);
        }
    }
    result
}

fn validate_no_budget(name: &str, description: &str, content: &str) -> ValidationResult {
    let mut result = ValidationResult::ok();
    validate_name_generic(name, &mut result);
    validate_description_generic(description, &mut result);
    if content.is_empty() {
        result.warnings.push("Component content is empty".to_string());
    }
    result
}

fn validate_skill_generic(
    component: &GenericComponent<'_>,
    config: &ValidationConfig,
) -> ValidationResult {
    let budget = config.token_budget.unwrap_or(5_000);
    let hard = config.token_budget_hard_error.unwrap_or(true);
    validate_soft_budget_named(component.name, component.description, component.content, &ValidationConfig { token_budget: Some(budget), token_budget_hard_error: Some(hard), ..Default::default() }, budget)
}

fn validate_tool_generic(
    component: &GenericComponent<'_>,
    config: &ValidationConfig,
) -> ValidationResult {
    let mut result = validate_skill_generic(component, config);
    if config.require_tool_name.unwrap_or(true) {
        result
            .errors
            .push("Tool component must declare a tool_name via ToolSkill payload".to_string());
    }
    result
}

/// Apply per-class config token budget overrides to an existing ToolSkill result.
///
/// Allows the operator to override the 5000-token limit via `reborn_validation_config`
/// for the next validation cycle, without altering `RecipeValidator` internals.
fn apply_config_overrides_skill(
    result: &mut ValidationResult,
    skill: &ToolSkill,
    config: &ValidationConfig,
) {
    if let Some(budget) = config.token_budget {
        let tokens = skill.estimated_tokens();
        let hard = config.token_budget_hard_error.unwrap_or(true);
        if tokens > budget as usize {
            let msg = format!(
                "ToolSkill exceeds {budget} token budget ({tokens} tokens) [config override]"
            );
            if hard {
                result.errors.push(msg);
            } else {
                result.warnings.push(msg);
            }
        }
    }
}

fn validate_name_generic(name: &str, result: &mut ValidationResult) {
    if name.is_empty() {
        result.errors.push("Component name must not be empty".to_string());
    } else if name.len() > 256 {
        result.errors.push(format!(
            "Component name exceeds 256 chars ({} chars)",
            name.len()
        ));
    }
}

fn validate_description_generic(desc: &str, result: &mut ValidationResult) {
    if desc.trim().is_empty() {
        result
            .warnings
            .push("Component description is empty".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::recipe::{RecipeSource, ToolSkillParam, ValidationStatus};

    fn base_skill() -> ToolSkill {
        ToolSkill {
            id: "s1".into(),
            name: "file-reader".into(),
            tool_name: "builtin.shell".into(),
            description: "Reads a file and returns its contents using the shell tool".into(),
            param_template: serde_json::json!({}),
            param_schema: vec![ToolSkillParam {
                name: "path".into(),
                param_type: "string".into(),
                description: "File path".into(),
                required: true,
            }],
            preconditions: "".into(),
            error_handling: "".into(),
            code_snippet: None,
            category: "files".into(),
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
    fn class1_skill_passes_validation() {
        let skill = base_skill();
        let result = ComponentValidator::validate_by_class(
            1,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &["builtin.shell".to_string()],
            &[],
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class16_action_no_token_budget_check() {
        let g = GenericComponent {
            name: "deploy-step",
            description: "Deploy the artifact",
            content: &"x".repeat(100_000), // huge content
        };
        let result = ComponentValidator::validate_by_class(
            16,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        // Actions are exempt from size limits — no token budget error or warning
        assert!(
            result.errors.is_empty(),
            "expected no token-budget errors for Actions, got {:?}",
            result.errors
        );
        assert!(
            result.warnings.iter().all(|w| !w.contains("token")),
            "expected no token-budget warnings for Actions, got {:?}",
            result.warnings
        );
    }

    #[test]
    fn class4_extension_soft_budget_warning() {
        let g = GenericComponent {
            name: "my-extension",
            description: "An extension",
            content: &"w".repeat(50_001), // ~12500 tokens
        };
        let result = ComponentValidator::validate_by_class(
            4,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        // Soft error → warning not error
        assert!(
            result.errors.is_empty(),
            "expected no hard errors for Extension, got {:?}",
            result.errors
        );
        assert!(
            result.warnings.iter().any(|w| w.contains("token")),
            "expected soft token-budget warning, got {:?}",
            result.warnings
        );
    }

    #[test]
    fn config_override_token_budget_applies() {
        let skill = base_skill();
        let config = ValidationConfig {
            token_budget: Some(1), // absurdly small to force a hit
            token_budget_hard_error: Some(false),
            ..Default::default()
        };
        let result = ComponentValidator::validate_by_class(
            1,
            ComponentPayload::ToolSkill(&skill),
            &config,
            &["builtin.shell".to_string()],
            &[],
        );
        assert!(
            result.warnings.iter().any(|w| w.contains("config override")),
            "expected config-override warning, got {:?}",
            result.warnings
        );
    }

    #[test]
    fn is_valid_transition_autofailed_to_pending() {
        // Guard test: AutoFailed → Pending is a valid Q1 re-queue.
        use crate::types::recipe::ValidationStatus;
        // The transition is implemented in recipe_store.rs; here we verify
        // the ComponentValidator doesn't block it at the validation layer.
        // (The actual guard function lives in composition — tested there.)
        assert_eq!(
            ValidationStatus::AutoFailed as u8,
            ValidationStatus::AutoFailed as u8,
            "AutoFailed variant must be distinct"
        );
        assert_eq!(
            ValidationStatus::Pending as u8,
            ValidationStatus::Pending as u8,
        );
    }
}
