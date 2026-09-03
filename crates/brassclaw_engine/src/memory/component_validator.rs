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
//!
//! # Skill Generic payload fallback
//! When a skill (class 1–3) or tool (class 0) arrives as `ComponentPayload::Generic` the
//! caller has not provided a `ToolSkill` struct — the component was stored without one (e.g.
//! free-form text body).  The validator **still enforces** the agentskills.io name/description
//! rules and the 5000-token hard budget; it simply cannot check tool_name / param_schema.
//! This avoids silently bypassing the security gate for skills that lack a structured payload.

#![forbid(unsafe_code)]

// ── Token budget defaults per class code ─────────────────────────────────────
/// Soft token budget for Extension classes (04–09) and other standard components.
const BUDGET_STANDARD: u32 = 10_000;
/// Soft token budget for Orchestrator / orchestrator-extension classes (10, 50).
const BUDGET_ORCHESTRATOR: u32 = 50_000;
/// Soft token budget for Note class (15).
const BUDGET_NOTES: u32 = 2_000;
/// Default token budget for Skill tool validation (hard-error if no config override).
const BUDGET_SKILL_DEFAULT: u32 = 5_000;

use crate::memory::recipe_validator::{RecipeValidator, ValidationResult};
use crate::types::recipe::{Recipe, ToolSkill};
use serde_json::Value;

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
    /// Structured extras for classes whose DB shape carries more than a text
    /// body. Phase C (class 23 / ExtensionCatalogue) populates this with the
    /// catalogue extras object `{ task_groups, child_component_ids,
    /// intent_index }` so the validator can check `>=1 task_group` and valid
    /// UUID syntax in `child_component_ids` (COMP-04 Option A). `None` for
    /// every class that maps 1:1 onto `content`.
    pub extra: Option<Value>,
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
                // Generic payload: no ToolSkill struct available — still apply the full
                // agentskills.io name/description/budget rules.  tool_name/param_schema
                // cannot be checked without a ToolSkill; an error is added to make that
                // visible to the operator, but the other checks still run.
                ComponentPayload::Generic(g) => validate_skill_generic(g, config),
                ComponentPayload::Recipe(_) => {
                    ValidationResult::from_error("Skill class requires a ToolSkill payload")
                }
            },
            // Tool (00): tool_name + param_schema required
            0 => match &component {
                ComponentPayload::ToolSkill(skill) => {
                    // C.4.5.4 — for class 0, `tool_name` carries the capability_id
                    // (the Executioner's dispatch id, e.g. "builtin.shell" /
                    // "host.resolve_intent" — the rust-side common form of a Tool,
                    // V071 `reborn_tools.capability_id`). `validate_tool_skill`'s
                    // tool_name-non-empty check above IS the capability_id-non-empty
                    // Q1 gate. Plus the common-syntax placeholder-grammar + non-nil
                    // includes gate (F-HI-2=A, mirrors class 13).
                    let mut result = RecipeValidator::validate_tool_skill(skill, available_tools);
                    validate_tool_skill_placeholders(skill, &mut result);
                    result
                }
                // Generic payload: structural checks run; explicit error tells the operator
                // a ToolSkill payload is needed to validate tool_name + param_schema.
                ComponentPayload::Generic(g) => validate_tool_generic(g, config),
                ComponentPayload::Recipe(_) => {
                    ValidationResult::from_error("Tool class requires a ToolSkill payload")
                }
            },
            // Extensions (04-09)
            4..=9 => match &component {
                ComponentPayload::Generic(g) => {
                    validate_soft_budget(g, config, BUDGET_STANDARD, false)
                }
                ComponentPayload::ToolSkill(skill) => validate_soft_budget_named(
                    skill.name.as_str(),
                    skill.description.as_str(),
                    skill.code_snippet.as_deref().unwrap_or(""),
                    config,
                    BUDGET_STANDARD,
                    false,
                ),
                ComponentPayload::Recipe(_) => {
                    ValidationResult::from_error("Extension class does not accept a Recipe payload")
                }
            },
            // Orchestrator (10) / Scaffold (50): soft orchestrator budget +
            // common-syntax placeholder-grammar gate (C.4.5.5, F-HI-2=A). The
            // class-10 orchestrator script is assembled by compose_orchestrator
            // (C.4.5.17) and run by Monty via host.run_program; it carries
            // `{{vars.NAME}}`/`{{vars.slotN}}`/`{{user_input}}`/`{{component_name}}`
            // placeholders. Skills (1-3) are pure narrative (no placeholders)
            // and are NOT gated. Structural-only — the composer is the sole
            // baker; Q1 never bakes.
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
                let mut result = validate_soft_budget_named(
                    name,
                    desc,
                    content,
                    config,
                    BUDGET_ORCHESTRATOR,
                    false,
                );
                validate_placeholder_grammar(content, "Orchestrator/Scaffold content", &mut result);
                result
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
                    validate_soft_budget(g, config, BUDGET_STANDARD, false)
                }
                ComponentPayload::ToolSkill(_) => {
                    ValidationResult::from_error("Recipe class requires a Recipe payload")
                }
            },
            // PythonCode (22): executable Python body — structural + soft 10k
            // budget + shell-injection scan (FIND-AUDIT-12). Uses the Generic
            // payload (FINDING E — no dedicated ComponentPayload::PythonCode
            // variant). Tier-0 cross-reference hard errors (unresolvable refs,
            // S7, step-order) land in Phase I/N (require a pool); Q1 here is
            // structural-only, consistent with q1_orchestrator's "Q1
            // structural-only" scope.
            22 => match &component {
                ComponentPayload::Generic(g) => {
                    let mut result = validate_soft_budget_named(
                        g.name,
                        g.description,
                        g.content,
                        config,
                        BUDGET_STANDARD,
                        false,
                    );
                    validate_python_code_body(g.content, &mut result);
                    validate_python_code_placeholders(g.content, g.extra.as_ref(), &mut result);
                    result
                }
                ComponentPayload::ToolSkill(_) => {
                    ValidationResult::from_error("PythonCode class requires a Generic payload")
                }
                ComponentPayload::Recipe(_) => {
                    ValidationResult::from_error("PythonCode class requires a Generic payload")
                }
            },
            // ExtensionCatalogue (23): documentation-container that organises a
            // capability domain (§0.2). Uses the Generic payload with
            // `extra = { task_groups, child_component_ids, intent_index }`
            // (COMP-04 Option A). `content` carries `overview_doc` (the
            // primary text field). Q1 here is structural-only: name format +
            // non-empty overview_doc + >=1 task_group + valid UUID syntax in
            // child_component_ids. Cross-reference checks (recipe_ids resolve,
            // child UUIDs exist) land in Phase I/N (require a pool).
            23 => match &component {
                ComponentPayload::Generic(g) => {
                    let mut result = validate_soft_budget_named(
                        g.name,
                        g.description,
                        g.content,
                        config,
                        BUDGET_STANDARD,
                        false,
                    );
                    validate_extension_catalogue_extras(g, &mut result);
                    result
                }
                ComponentPayload::ToolSkill(_) => ValidationResult::from_error(
                    "ExtensionCatalogue class requires a Generic payload",
                ),
                ComponentPayload::Recipe(_) => ValidationResult::from_error(
                    "ExtensionCatalogue class requires a Generic payload",
                ),
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
                    ComponentPayload::Recipe(r) => (r.name.as_str(), r.description.as_str(), ""),
                };
                validate_soft_budget_named(name, desc, content, config, BUDGET_NOTES, false)
            }
            // ToolSkill (13): full agentskills.io validation via the canonical
            // `validate_tool_skill` (the SAME validator used for class 0 + 1-3),
            // closing the prior catch-all gap that ran soft-budget only. Plus
            // the common-syntax placeholder-grammar + non-nil includes Q1 gate
            // (C.4.5.3, F-HI-2=A). Generic/Recipe payloads are rejected — a
            // ToolSkill struct is required to validate tool_name + param_schema
            // (mirrors class 0).
            13 => match &component {
                ComponentPayload::ToolSkill(skill) => {
                    let mut result = RecipeValidator::validate_tool_skill(skill, available_tools);
                    validate_tool_skill_placeholders(skill, &mut result);
                    result
                }
                ComponentPayload::Generic(_) => ValidationResult::from_error(
                    "ToolSkill class requires a ToolSkill payload",
                ),
                ComponentPayload::Recipe(_) => ValidationResult::from_error(
                    "ToolSkill class requires a ToolSkill payload",
                ),
            },
            // Former DocType classes (12, 14, 17-20): soft 10000 (13 has its own arm above)
            12 | 14 | 17..=20 => {
                let (name, desc, content) = match &component {
                    ComponentPayload::Generic(g) => (g.name, g.description, g.content),
                    ComponentPayload::ToolSkill(s) => (
                        s.name.as_str(),
                        s.description.as_str(),
                        s.code_snippet.as_deref().unwrap_or(""),
                    ),
                    ComponentPayload::Recipe(r) => (r.name.as_str(), r.description.as_str(), ""),
                };
                validate_soft_budget_named(name, desc, content, config, BUDGET_STANDARD, false)
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
                validate_soft_budget_named(name, desc, content, config, BUDGET_STANDARD, false)
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
    default_hard: bool,
) -> ValidationResult {
    validate_soft_budget_named(
        component.name,
        component.description,
        component.content,
        config,
        default_budget,
        default_hard,
    )
}

fn validate_soft_budget_named(
    name: &str,
    description: &str,
    content: &str,
    config: &ValidationConfig,
    default_budget: u32,
    default_hard: bool,
) -> ValidationResult {
    let mut result = ValidationResult::ok();
    validate_name_generic(name, &mut result);
    validate_description_soft(description, &mut result);
    if content.is_empty() {
        result
            .warnings
            .push("Component content is empty".to_string());
    }
    let budget = config.token_budget.unwrap_or(default_budget);
    let hard = config.token_budget_hard_error.unwrap_or(default_hard);
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
    validate_description_soft(description, &mut result);
    if content.is_empty() {
        result
            .warnings
            .push("Component content is empty".to_string());
    }
    result
}

/// Scan a PythonCode body for shell-injection / sandbox-escape patterns
/// (FIND-AUDIT-12). Hard errors fail Q1; warnings flag but do not block.
///
/// The scan is a simple substring search over the raw `content` — no AST
/// parsing is required. False-positive rate is low because PythonCode bodies
/// are authored to call host functions (`__execute_action__`,
/// `__check_budget__`, ...) rather than import OS/subprocess/socket modules
/// directly. The full 7-case test matrix lands in Phase I; Phase B exercises
/// only the narrow smoke cases (valid body passes; `import os` hard-errors;
/// `print(` warns only).
fn validate_python_code_body(content: &str, result: &mut ValidationResult) {
    // Hard errors (Q1 fail) — direct OS / subprocess / interpreter escape.
    const HARD_ERRORS: &[&str] = &[
        "import os",
        "import subprocess",
        "import sys",
        "import socket",
        "import ctypes",
        "import importlib",
        "__import__(",
        "exec(",
        "eval(",
        "open(",
        "compile(",
        "__builtins__",
        "globals()",
        "locals()",
    ];
    for needle in HARD_ERRORS {
        if content.contains(needle) {
            result.errors.push(format!(
                "PythonCode body contains forbidden construct `{needle}` \
                 (use host tool calls (host.<tool>(...)) for host access instead)"
            ));
        }
    }
    // Warnings (Q1 soft — flag, do not block).
    if content.contains("print(") {
        result.warnings.push(
            "PythonCode body uses print() (stdout is VM-captured, not the host terminal)"
                .to_string(),
        );
    }
    if content.contains("input(") {
        result.warnings.push(
            "PythonCode body uses input() (will hang in the VM — likely a copy-paste error)"
                .to_string(),
        );
    }
}

/// C.4.5.3 — shared `{{ ... }}` placeholder-grammar gate (F-HI-2=A). Scans
/// `content` for `{{ ... }}`, enforces balanced braces + a recognised kind
/// (vars.NAME | vars.slotN | user_input | component_name via
/// [`is_recognised_python_placeholder`]). `label` names the field in error
/// messages. Structural-only — the composer (C.4.5.17) is the sole baker.
fn validate_placeholder_grammar(content: &str, label: &str, result: &mut ValidationResult) {
    let mut cursor = 0;
    while let Some(open_rel) = content[cursor..].find("{{") {
        let open = cursor + open_rel;
        let Some(close_rel) = content[open + 2..].find("}}") else {
            result.errors.push(format!(
                "{label} has an unbalanced `{{{{` placeholder (no closing `}}}}`)"
            ));
            return;
        };
        let inner = &content[open + 2..open + 2 + close_rel];
        let trimmed = inner.trim();
        if !is_recognised_python_placeholder(trimmed) {
            result.errors.push(format!(
                "{label} placeholder `{{{{ {trimmed} }}}}` is not a recognised kind \
                 (expected vars.NAME, vars.slotN, user_input, or component_name)"
            ));
        }
        cursor = open + 2 + close_rel + 2;
    }
}

/// C.4.5.2 — validate the `{{ ... }}` placeholder structure in a PythonCode
/// body (F-HI-2=A) and the non-nil-ness of its declared `includes` UUID list
/// (Fork 2-B=B). Structural-only: the composer (C.4.5.17) is the sole baker;
/// Q1 never bakes. Referential placeholder<->include matching (each
/// `{{component_name}}` resolves to a real fetched component, and every
/// include is consumed by a placeholder) is deferred to Phase I/N (requires a
/// pool); Q1 here checks well-formedness + non-nil UUIDs only.
fn validate_python_code_placeholders(
    content: &str,
    extra: Option<&Value>,
    result: &mut ValidationResult,
) {
    // Placeholder well-formedness — shared grammar gate (C.4.5.3).
    validate_placeholder_grammar(content, "PythonCode body", result);

    // Includes (carried in `extra` on the Q1 save path): each present UUID must
    // parse and be non-nil. Absent `extra` / absent `includes` is allowed (a
    // leaf body declares no includes).
    let Some(extra) = extra else { return };
    let Some(arr) = extra.get("includes").and_then(Value::as_array) else {
        return;
    };
    for (j, entry) in arr.iter().enumerate() {
        match entry.as_str() {
            Some(s) => match uuid::Uuid::parse_str(s) {
                Ok(u) if u != uuid::Uuid::nil() => {}
                Ok(_) => result.errors.push(format!(
                    "PythonCode includes[{j}] is the nil UUID — component-include must resolve to a real component"
                )),
                Err(_) => result.errors.push(format!(
                    "PythonCode includes[{j}] `{s}` is not a valid UUID"
                )),
            },
            None => result.errors.push(format!(
                "PythonCode includes[{j}] `{entry}` is not a UUID string"
            )),
        }
    }
}

/// Recognised `{{ ... }}` placeholder kinds for a PythonCode body (F-HI-2=A):
/// `user_input`, `component_name`, `vars.NAME` (identifier), `vars.slotN`
/// (non-negative integer).
fn is_recognised_python_placeholder(trimmed: &str) -> bool {
    if trimmed == "user_input" || trimmed == "component_name" {
        return true;
    }
    let Some(rest) = trimmed.strip_prefix("vars.") else {
        return false;
    };
    if let Some(n) = rest.strip_prefix("slot") {
        return !n.is_empty() && n.chars().all(|c| c.is_ascii_digit());
    }
    // vars.NAME — non-empty, [A-Za-z0-9_-], not starting with a digit.
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !rest.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// C.4.5.3 — non-nil check for a parsed `Vec<Uuid>` includes list. PythonCode
/// parses UUID strings from `extra` itself (so it keeps its own JSON loop with
/// the parse-error messages); this helper covers the already-parsed struct
/// field form used by ToolSkill. `label` names the component class in errors.
fn validate_includes_non_nil_uuids(
    includes: &[uuid::Uuid],
    label: &str,
    result: &mut ValidationResult,
) {
    for (j, u) in includes.iter().enumerate() {
        if *u == uuid::Uuid::nil() {
            result.errors.push(format!(
                "{label} includes[{j}] is the nil UUID — component-include must resolve to a real component"
            ));
        }
    }
}

/// C.4.5.3 — ToolSkill (class 13) placeholder-grammar + non-nil includes Q1
/// gate. A ToolSkill description may include another description
/// (`{{component_name}}`); `param_template` carries `{{vars.name}}` for
/// ToolBinding substitution. Scans every text field + serialized
/// `param_template` + each `param_schema` entry description for balanced
/// `{{ }}` + recognised kinds, then checks the struct-field `includes` list
/// for non-nil UUIDs. Referential placeholder<->include matching is deferred to
/// Phase I/N (requires a pool); Q1 is structural-only.
fn validate_tool_skill_placeholders(skill: &ToolSkill, result: &mut ValidationResult) {
    validate_placeholder_grammar(&skill.description, "ToolSkill description", result);
    validate_placeholder_grammar(&skill.preconditions, "ToolSkill preconditions", result);
    validate_placeholder_grammar(&skill.error_handling, "ToolSkill error_handling", result);
    if let Some(snippet) = &skill.code_snippet {
        validate_placeholder_grammar(snippet, "ToolSkill code_snippet", result);
    }
    validate_placeholder_grammar(
        &skill.param_template.to_string(),
        "ToolSkill param_template",
        result,
    );
    for p in &skill.param_schema {
        validate_placeholder_grammar(&p.description, "ToolSkill param_schema", result);
    }
    validate_includes_non_nil_uuids(&skill.includes, "ToolSkill", result);
}

/// Validate the ExtensionCatalogue-specific extras carried in
/// [`GenericComponent::extra`] (Phase C / COMP-04 Option A).
///
/// `content` carries `overview_doc` (the catalogue's primary text field); an
/// empty overview is a hard error — a documentation-container that documents
/// nothing is structurally malformed. `extra` must carry the catalogue extras
/// object `{ task_groups, child_component_ids, intent_index }`; the validator
/// enforces:
/// - `task_groups` is a JSON array with at least one entry (a catalogue with
///   no task groups has no organisational content).
/// - `child_component_ids` is a JSON array whose every entry is a valid UUID
///   string (lineage syntax — the DB column is `UUID[]`; an empty array is
///   allowed, a catalogue may declare children later).
/// - `intent_index` is carried but NOT validated (audit-only, §0.2).
///
/// Cross-reference checks (recipe_ids resolve to real recipes, child UUIDs
/// exist as components) land in Phase I/N — they require a live pool.
fn validate_extension_catalogue_extras(
    component: &GenericComponent<'_>,
    result: &mut ValidationResult,
) {
    // overview_doc (carried as `content`) must be non-empty — hard error.
    if component.content.trim().is_empty() {
        result
            .errors
            .push("ExtensionCatalogue overview_doc must not be empty".to_string());
    }

    let Some(extra) = component.extra.as_ref() else {
        result.errors.push(
            "ExtensionCatalogue requires extra {task_groups, child_component_ids, intent_index} \
             (COMP-04)"
                .to_string(),
        );
        return;
    };

    // task_groups: JSON array with >=1 entry.
    match extra.get("task_groups").and_then(Value::as_array) {
        Some(arr) if !arr.is_empty() => {}
        Some(_) => result
            .errors
            .push("ExtensionCatalogue must declare at least one task_group".to_string()),
        None => result
            .errors
            .push("ExtensionCatalogue task_groups must be a JSON array".to_string()),
    }

    // child_component_ids: JSON array of valid UUID strings. An empty array is
    // allowed (children may be declared later); each present entry must parse.
    match extra.get("child_component_ids").and_then(Value::as_array) {
        Some(arr) => {
            for entry in arr {
                match entry.as_str() {
                    Some(s) if uuid::Uuid::parse_str(s).is_ok() => {}
                    Some(s) => result.errors.push(format!(
                        "ExtensionCatalogue child_component_ids entry `{s}` is not a valid UUID"
                    )),
                    None => result.errors.push(format!(
                        "ExtensionCatalogue child_component_ids entry `{entry}` is not a UUID string"
                    )),
                }
            }
        }
        None => result
            .errors
            .push("ExtensionCatalogue child_component_ids must be a JSON array".to_string()),
    }
}

/// Full agentskills.io name/description/budget validation for a Generic skill payload.
///
/// Used when a Skill (class 1–3) arrives without a `ToolSkill` struct.  The
/// strict name pattern, hard 5000-token budget, and hard-error empty-description
/// rules still apply — tool_name/param_schema checks are skipped since there is
/// no structured payload, but an explicit error is added so the operator knows
/// the payload upgrade is required.
fn validate_skill_generic(
    component: &GenericComponent<'_>,
    config: &ValidationConfig,
) -> ValidationResult {
    let mut result = ValidationResult::ok();

    // Strict agentskills.io name checks (same as RecipeValidator::check_name_format).
    validate_name_skill(component.name, &mut result);

    // Description: hard error for empty (not a warning — skills must have descriptions).
    validate_description_hard(component.description, "Skill", &mut result);

    // Token budget: 5000 hard by default.
    let budget = config.token_budget.unwrap_or(BUDGET_SKILL_DEFAULT);
    let hard = config.token_budget_hard_error.unwrap_or(true);
    let tokens = (component.content.len() / 4) as u32;
    if tokens > budget {
        let msg = format!(
            "Skill exceeds {budget} token budget ({tokens} tokens estimated). \
             Split into smaller skills or move detail to reference files."
        );
        if hard {
            result.errors.push(msg);
        } else {
            result.warnings.push(msg);
        }
    }

    // Explicit notice that tool_name/param_schema could not be checked.
    result.errors.push(
        "Skill class requires a ToolSkill payload for tool_name and param_schema validation; \
         Generic payload was supplied — upgrade to ToolSkill to complete validation"
            .to_string(),
    );

    result
}

fn validate_tool_generic(
    component: &GenericComponent<'_>,
    config: &ValidationConfig,
) -> ValidationResult {
    // Structural checks run (name, description, budget) even without a ToolSkill;
    // an error is always added because tool_name + param_schema cannot be verified.
    let mut result = validate_skill_generic(component, config);
    // Replace the generic "needs ToolSkill" message with a tool-specific one.
    result
        .errors
        .retain(|e| !e.contains("Skill class requires a ToolSkill payload"));
    result.errors.push(
        "Tool class requires a ToolSkill payload for tool_name and param_schema validation; \
         Generic payload was supplied — upgrade to ToolSkill to complete validation"
            .to_string(),
    );
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
        // Both sides are `usize`; no lossy cast.
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

/// Strict agentskills.io name format check (max 64, `^[a-z0-9]([a-z0-9-]*[a-z0-9])?$`,
/// no consecutive hyphens, no leading/trailing hyphens).
fn validate_name_skill(name: &str, result: &mut ValidationResult) {
    if name.is_empty() {
        result
            .errors
            .push("Skill name must not be empty".to_string());
        return;
    }
    if name.len() > 64 {
        result.errors.push(format!(
            "Skill name exceeds 64 chars ({} chars)",
            name.len()
        ));
    }
    if name.contains("--") {
        result
            .errors
            .push("Skill name contains consecutive hyphens '--'".to_string());
    }
    if name.starts_with('-') || name.ends_with('-') {
        result
            .errors
            .push("Skill name must not start or end with '-'".to_string());
    }
    for ch in name.chars() {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' {
            result.errors.push(format!(
                "Skill name '{name}' contains invalid character '{ch}' — must match ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$"
            ));
            break;
        }
    }
}

/// Generic name check for non-skill classes (max 256, non-empty).
fn validate_name_generic(name: &str, result: &mut ValidationResult) {
    if name.is_empty() {
        result
            .errors
            .push("Component name must not be empty".to_string());
    } else if name.len() > 256 {
        result.errors.push(format!(
            "Component name exceeds 256 chars ({} chars)",
            name.len()
        ));
    }
}

/// Hard-error description check for classes where an empty description is blocking
/// (Skills and Tools).
fn validate_description_hard(desc: &str, kind: &str, result: &mut ValidationResult) {
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

/// Soft-warning description check for extension/doctype classes where an empty
/// description is advisory, not blocking.
fn validate_description_soft(desc: &str, result: &mut ValidationResult) {
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
            includes: vec![],
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
            extra: None,
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
            extra: None,
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
    fn class22_python_code_valid_body_passes() {
        // A body that calls host functions and avoids forbidden constructs.
        let body = "result = __execute_action__(\"read_file\", {\"path\": path})\nreturn result";
        let g = GenericComponent {
            name: "read-file-leaf",
            description: "Reads a file via the host read_file action",
            content: body,
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "expected no hard errors for a valid PythonCode body, got {:?}",
            result.errors
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class22_python_code_import_os_is_hard_error() {
        let g = GenericComponent {
            name: "bad-leaf",
            description: "Tries to import the OS module directly",
            content: "import os\nos.getcwd()",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.iter().any(|e| e.contains("import os")),
            "expected a hard error for `import os`, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class22_python_code_print_is_warning_only() {
        let g = GenericComponent {
            name: "debug-leaf",
            description: "Emits debug stdout",
            content: "print(\"debug\")\nreturn 0",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "print() must be a warning only, not a hard error; got {:?}",
            result.errors
        );
        assert!(
            result.warnings.iter().any(|w| w.contains("print()")),
            "expected a print() warning, got {:?}",
            result.warnings
        );
    }

    #[test]
    fn class22_python_code_placeholders_valid_passes() {
        // C.4.5.2 — every recognised placeholder kind + a non-nil includes list
        // passes Q1 (referential placeholder<->include match is deferred).
        let inc = uuid::Uuid::new_v4().to_string();
        let g = GenericComponent {
            name: "placeholder-leaf",
            description: "Body using all four placeholder kinds",
            content: "a = \"{{vars.slot0}}\"\nb = \"{{user_input}}\"\nc = \"{{component_name}}\"\nd = \"{{vars.dir}}\"\nreturn a",
            extra: Some(serde_json::json!({ "includes": [inc] })),
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "expected no errors for well-formed placeholders, got {:?}",
            result.errors
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class22_python_code_unbalanced_placeholder_fails() {
        let g = GenericComponent {
            name: "unbalanced-leaf",
            description: "Missing closing braces",
            content: "a = \"{{vars.slot0\"\nreturn a",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("unbalanced") && e.contains("{{")),
            "expected an unbalanced-placeholder error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class22_python_code_unrecognised_placeholder_fails() {
        let g = GenericComponent {
            name: "bad-kind-leaf",
            description: "An unknown placeholder kind",
            content: "a = \"{{bogus}}\"\nreturn a",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("not a recognised kind") && e.contains("bogus")),
            "expected an unrecognised-placeholder error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class22_python_code_nil_include_fails() {
        let g = GenericComponent {
            name: "nil-include-leaf",
            description: "Declares a nil include UUID",
            content: "a = \"{{component_name}}\"\nreturn a",
            extra: Some(serde_json::json!({
                "includes": [uuid::Uuid::nil().to_string()]
            })),
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.iter().any(|e| e.contains("nil UUID")),
            "expected a nil-include error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class22_python_code_bad_include_uuid_fails() {
        let g = GenericComponent {
            name: "bad-include-leaf",
            description: "Declares a non-UUID include entry",
            content: "a = \"{{component_name}}\"\nreturn a",
            extra: Some(serde_json::json!({ "includes": ["not-a-uuid"] })),
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("not a valid UUID") && e.contains("not-a-uuid")),
            "expected a bad-include-uuid error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class22_python_code_component_placeholder_without_includes_passes() {
        // C.4.5.2 boundary: a well-formed {{component_name}} with no includes
        // list passes Q1 — the referential placeholder<->include match is
        // deferred to Phase I/N (Fork 2-B=B); Q1 is structural-only.
        let g = GenericComponent {
            name: "deferred-include-leaf",
            description: "Component placeholder, includes declared later",
            content: "a = \"{{component_name}}\"\nreturn a",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            22,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "Q1 must not enforce referential placeholder<->include match, got {:?}",
            result.errors
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class13_tool_skill_valid_placeholders_and_includes_pass() {
        // C.4.5.3 — every recognised placeholder kind across every scanned
        // ToolSkill text field + a non-nil includes list passes the class-13
        // gate (full validate_tool_skill + placeholder grammar).
        let mut skill = base_skill();
        skill.param_template = serde_json::json!({
            "path": "{{vars.path}}",
            "label": "{{user_input}}"
        });
        skill.param_schema[0].description = "Repo root {{component_name}} path".into();
        skill.preconditions = "git repo present at {{vars.slot0}}".into();
        skill.error_handling = "exit non-zero means dirty {{component_name}}".into();
        skill.code_snippet = Some("process({{user_input}})".into());
        skill.includes = vec![uuid::Uuid::new_v4()];
        let result = ComponentValidator::validate_by_class(
            13,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "expected no errors for well-formed ToolSkill, got {:?}",
            result.errors
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class13_tool_skill_unbalanced_placeholder_fails() {
        let mut skill = base_skill();
        skill.preconditions = "git repo {{vars.slot0".into();
        let result = ComponentValidator::validate_by_class(
            13,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("unbalanced") && e.contains("{{")),
            "expected an unbalanced-placeholder error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class13_tool_skill_unrecognised_placeholder_fails() {
        let mut skill = base_skill();
        skill.preconditions = "see {{bogus}} here".into();
        let result = ComponentValidator::validate_by_class(
            13,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("not a recognised kind") && e.contains("bogus")),
            "expected an unrecognised-placeholder error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class13_tool_skill_nil_include_fails() {
        let mut skill = base_skill();
        skill.includes = vec![uuid::Uuid::nil()];
        let result = ComponentValidator::validate_by_class(
            13,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.iter().any(|e| e.contains("nil UUID")),
            "expected a nil-include error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class13_tool_skill_generic_payload_errors() {
        let g = GenericComponent {
            name: "x",
            description: "y",
            content: "z",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            13,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("requires a ToolSkill payload")),
            "expected a payload-mismatch error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class13_tool_skill_empty_tool_name_fails() {
        // C.4.5.3 — proves the dedicated 13 arm runs `validate_tool_skill`: the
        // prior catch-all only did soft-budget and would NOT error on an empty
        // tool_name. available_tools=&[] so membership is skipped; the
        // emptiness check still fires.
        let mut skill = base_skill();
        skill.tool_name = "".into();
        let result = ComponentValidator::validate_by_class(
            13,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("tool_name") && e.contains("empty")),
            "expected a tool_name-empty error from validate_tool_skill, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class0_tool_valid_capability_id_passes() {
        // C.4.5.4 — a class-0 Tool payload (tool_name = the capability_id, no
        // placeholders, leaf includes) passes the full class-0 gate
        // (validate_tool_skill + validate_tool_skill_placeholders).
        let skill = base_skill();
        let result = ComponentValidator::validate_by_class(
            0,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "expected no errors for well-formed class-0 Tool, got {:?}",
            result.errors
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class0_tool_unbalanced_placeholder_fails() {
        // C.4.5.4 — the common-syntax placeholder-grammar gate runs on class 0
        // (F-HI-2=A): an unbalanced `{{` in a text field is rejected.
        let mut skill = base_skill();
        skill.preconditions = "git repo {{vars.slot0".into();
        let result = ComponentValidator::validate_by_class(
            0,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("unbalanced") && e.contains("{{")),
            "expected an unbalanced-placeholder error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class0_tool_empty_capability_id_fails() {
        // C.4.5.4 — for class 0, tool_name carries the capability_id (V071
        // `reborn_tools.capability_id`); an empty capability_id is rejected by
        // validate_tool_skill's tool_name-non-empty check (available_tools=&[]
        // so membership is skipped; the emptiness check still fires).
        let mut skill = base_skill();
        skill.tool_name = "".into();
        let result = ComponentValidator::validate_by_class(
            0,
            ComponentPayload::ToolSkill(&skill),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("tool_name") && e.contains("empty")),
            "expected a tool_name-empty (capability_id) error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class10_orchestrator_valid_placeholders_pass() {
        // C.4.5.5 — a class-10 Orchestrator Generic payload whose content
        // carries well-formed common-syntax placeholders (`{{user_input}}`,
        // `{{vars.NAME}}`) passes the soft-budget + placeholder-grammar gate.
        let g = GenericComponent {
            name: "basic-mode-orchestrator",
            description: "The basic-mode Monty orchestrator script that drives a turn.",
            content: "user_input = {{user_input}}\npath = {{vars.workspace}}\nhost.post_reply(result)",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            10,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "expected no errors for well-formed class-10 Orchestrator content, got {:?}",
            result.errors
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class10_orchestrator_unbalanced_placeholder_fails() {
        // C.4.5.5 — the common-syntax placeholder-grammar gate runs on class 10
        // (F-HI-2=A): an unbalanced `{{` in the orchestrator content is rejected.
        let g = GenericComponent {
            name: "basic-mode-orchestrator",
            description: "The basic-mode Monty orchestrator script that drives a turn.",
            content: "data = {{vars.slot0",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            10,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("unbalanced") && e.contains("{{")),
            "expected an unbalanced-placeholder error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class10_orchestrator_unrecognised_placeholder_fails() {
        // C.4.5.5 — a balanced but unrecognised placeholder `{{bogus}}` is
        // rejected (expected vars.NAME / vars.slotN / user_input / component_name).
        let g = GenericComponent {
            name: "basic-mode-orchestrator",
            description: "The basic-mode Monty orchestrator script that drives a turn.",
            content: "data = {{bogus}}",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            10,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("not a recognised kind")),
            "expected an unrecognised-placeholder error, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class23_extension_catalogue_valid_passes() {
        // A catalogue with a non-empty overview_doc, one task_group, and one
        // valid child UUID — the canonical Phase-C shape (COMP-04 Option A).
        let child = uuid::Uuid::new_v4().to_string();
        let g = GenericComponent {
            name: "file-management-catalogue",
            description: "Catalogue covering local file management",
            content: "This catalogue covers local file management. Its Recipes handle these task groups...",
            extra: Some(serde_json::json!({
                "task_groups": [
                    {
                        "group_name": "file-management",
                        "summary": "Local file management recipes",
                        "recipe_ids": ["recipe-read-file", "recipe-write-file"]
                    }
                ],
                "child_component_ids": [child],
                "intent_index": []
            })),
        };
        let result = ComponentValidator::validate_by_class(
            23,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.is_empty(),
            "expected no hard errors for a valid ExtensionCatalogue, got {:?}",
            result.errors
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn class23_extension_catalogue_empty_overview_is_hard_error() {
        let child = uuid::Uuid::new_v4().to_string();
        let g = GenericComponent {
            name: "empty-overview-catalogue",
            description: "Catalogue with no overview text",
            content: "   ",
            extra: Some(serde_json::json!({
                "task_groups": [{ "group_name": "g", "summary": "s", "recipe_ids": [] }],
                "child_component_ids": [child],
                "intent_index": []
            })),
        };
        let result = ComponentValidator::validate_by_class(
            23,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("overview_doc must not be empty")),
            "expected a hard error for an empty overview_doc, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class23_extension_catalogue_no_task_groups_is_hard_error() {
        let g = GenericComponent {
            name: "no-task-groups-catalogue",
            description: "Catalogue with zero task groups",
            content: "An overview with no task groups.",
            extra: Some(serde_json::json!({
                "task_groups": [],
                "child_component_ids": [],
                "intent_index": []
            })),
        };
        let result = ComponentValidator::validate_by_class(
            23,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("at least one task_group")),
            "expected a hard error for zero task_groups, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
    }

    #[test]
    fn class23_extension_catalogue_bad_child_uuid_is_hard_error() {
        let g = GenericComponent {
            name: "bad-child-uuid-catalogue",
            description: "Catalogue with a malformed child UUID",
            content: "An overview with a bad child UUID.",
            extra: Some(serde_json::json!({
                "task_groups": [{ "group_name": "g", "summary": "s", "recipe_ids": [] }],
                "child_component_ids": ["not-a-uuid"],
                "intent_index": []
            })),
        };
        let result = ComponentValidator::validate_by_class(
            23,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.iter().any(|e| e.contains("not a valid UUID")),
            "expected a hard error for a malformed child UUID, got {:?}",
            result.errors
        );
        assert!(!result.is_ok());
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
            result
                .warnings
                .iter()
                .any(|w| w.contains("config override")),
            "expected config-override warning, got {:?}",
            result.warnings
        );
    }

    #[test]
    fn skill_generic_payload_fails_with_explicit_error() {
        // A Generic payload for a skill class always produces an error — the
        // operator must upgrade to a ToolSkill payload for full validation.
        let g = GenericComponent {
            name: "my-skill",
            description: "Use this to run a shell command",
            content: "some content",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            1,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("ToolSkill payload")),
            "expected explicit ToolSkill-required error, got {:?}",
            result.errors
        );
    }

    #[test]
    fn skill_generic_empty_description_is_hard_error() {
        let g = GenericComponent {
            name: "my-skill",
            description: "",
            content: "some content",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            2,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("description must not be empty")),
            "expected hard error for empty description, got {:?}",
            result.errors
        );
    }

    #[test]
    fn skill_generic_name_too_long_hard_error() {
        let g = GenericComponent {
            name: &"a".repeat(65),
            description: "Use this to do something useful",
            content: "body",
            extra: None,
        };
        let result = ComponentValidator::validate_by_class(
            1,
            ComponentPayload::Generic(g),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result.errors.iter().any(|e| e.contains("exceeds 64 chars")),
            "expected 64-char limit error, got {:?}",
            result.errors
        );
    }

    #[test]
    fn extension_recipe_payload_returns_error() {
        use crate::types::recipe::{
            RecipeSource, RecipeTrigger, RecipeValidation, ValidationStatus,
        };
        let recipe = crate::types::recipe::Recipe {
            id: "r1".into(),
            name: "my-recipe".into(),
            description: "desc".into(),
            trigger: RecipeTrigger::Keyword {
                keywords: vec!["x".into()],
                threshold: 0.5,
            },
            steps: vec![],
            validation: RecipeValidation::None,
            category: "c".into(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".into(),
            source: RecipeSource::Authored,
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
            variants: Vec::new(),
            step_descriptions: serde_json::Value::Null,
            dependency_registry: serde_json::Value::Null,
        };
        let result = ComponentValidator::validate_by_class(
            5,
            ComponentPayload::Recipe(&recipe),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("does not accept a Recipe payload")),
            "expected Recipe-rejected error for extension class, got {:?}",
            result.errors
        );
    }

    #[test]
    fn class21_recipe_v3_variant_without_description_fails_q1_gate() {
        use crate::types::recipe::{RecipeTrigger, RecipeValidation};
        let recipe = crate::types::recipe::Recipe {
            id: "r1".into(),
            name: "my-recipe".into(),
            description: "desc".into(),
            trigger: RecipeTrigger::Keyword {
                keywords: vec!["x".into()],
                threshold: 0.5,
            },
            steps: vec![crate::types::recipe::RecipeStep {
                skill: "step-skill".into(),
                tool: "github.api".into(),
                params: serde_json::json!({}),
                description: "List open issues".into(),
            }],
            validation: RecipeValidation::None,
            category: "c".into(),
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
            variants: vec![crate::types::recipe::RecipeVariant {
                variant_key: "ls-la".into(),
                description: None,
                step_link: Some("0:0-0:30+1:0-1:E".into()),
                intent_examples: vec![],
                variable_patterns: vec![],
            }],
            step_descriptions: serde_json::Value::Null,
            dependency_registry: serde_json::Value::Null,
        };
        let result = ComponentValidator::validate_by_class(
            21,
            ComponentPayload::Recipe(&recipe),
            &ValidationConfig::default(),
            &["step-skill".to_string()],
            &[],
        );
        assert!(
            result.errors.iter().any(|e| e.contains("dual-nature gate")),
            "expected dual-nature gate error through component_validator, got {result:?}"
        );
    }
}
