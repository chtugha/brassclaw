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
//!
//! ## Phase I — Q1 hard-error rules
//!
//! Beyond the basic structural checks, `validate_recipe` enforces three
//! architectural invariants that prevent silently-broken Tier-0 recipes:
//!
//! * **§shell-guard** — A recipe with `llm_call_required = false` must not
//!   reference `builtin.shell` or `builtin.spawn_subagent` in any rust-channel
//!   `tool_bindings`. Open-ended shell/spawn cannot be a Tier-0 path.
//!
//! * **§tier0-orchestrator-channel Rule 1** — `llm_call_required = false` AND
//!   a Skill (class 1/2/3) UUID in `orchestrator_steps` → hard error.  In
//!   Tier 0 there is no LLM to interpret narrative prose; PythonCode is the
//!   deterministic replacement.
//!
//! * **§tier0-orchestrator-channel Rule 2 (S7-extension)** — `llm_call_required
//!   = false` AND non-empty `tool_bindings` in `rust_steps` AND empty
//!   `orchestrator_steps` → hard error.  A loaded gun nobody fires.
//!
//! Cross-reference UUID resolution (does the UUID actually exist in the DB?)
//! is a Phase I/N concern and requires a pool — it is NOT done here.  Q1 is
//! structural-only.

/// Maximum token budget for a ToolSkill (hard error above this).
const SKILL_MAX_TOKENS: usize = 5000;
/// Regex compiled-size limit in bytes, preventing ReDoS from LLM-authored patterns.
const RECIPE_REGEX_SIZE_LIMIT: usize = 10_000;
/// Maximum character length of a single `intent_examples` entry.
const INTENT_EXAMPLE_MAX_CHARS: usize = 512;
/// Maximum number of `intent_examples` per variant.
const INTENT_EXAMPLE_MAX_COUNT: usize = 20;

/// Shell-type tool names that force `llm_call_required = true` (§shell-guard).
const SHELL_GUARD_TOOLS: &[&str] = &["builtin.shell", "builtin.spawn_subagent"];

use crate::memory::instruction_builder::StepDescriptionEntry;
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
        if tokens > SKILL_MAX_TOKENS {
            result.errors.push(format!(
                "ToolSkill exceeds {SKILL_MAX_TOKENS} token budget ({tokens} tokens). Split into smaller skills or move detail to reference files."
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
        check_variant_descriptions(&recipe.variants, &mut result);
        check_variant_intent_examples(&recipe.variants, &mut result);
        check_variant_machine_form(&recipe.variants, &recipe.step_descriptions, &mut result);
        // Phase I — IBS pre-flight + §shell-guard + §tier0-orchestrator-channel.
        // Runs only when step_descriptions are present (v3 variants).
        check_recipe_ibs_preflight(recipe, &mut result);

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
                } else if let Err(error) = regex::RegexBuilder::new(p)
                    .size_limit(RECIPE_REGEX_SIZE_LIMIT)
                    .build()
                {
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

/// Q1 gate for the human-readable side of the dual-nature recipe syntax
/// (Step B). Each **v3-migrated** variant (`step_link` present) must carry a
/// concise, non-empty human-readable `description`. Legacy variants
/// (`step_link == None`) are exempt — they predate Step B.
fn check_variant_descriptions(
    variants: &[crate::types::recipe::RecipeVariant],
    result: &mut ValidationResult,
) {
    const MAX_CHARS: usize = 512;
    for (i, v) in variants.iter().enumerate() {
        // Legacy / un-migrated variants have no step_link → exempt.
        if v.step_link.is_none() {
            continue;
        }
        let trimmed = v.description.as_deref().map(str::trim).unwrap_or("");
        let len = trimmed.chars().count();
        if len == 0 {
            result.errors.push(format!(
                "Recipe variant #{i} ('{}') has no human-readable description — required for v3 variants (Step B dual-nature gate)",
                v.variant_key
            ));
        } else if len > MAX_CHARS {
            result.warnings.push(format!(
                "Recipe variant #{i} ('{}') description exceeds {MAX_CHARS} chars ({len} chars) — keep it concise",
                v.variant_key
            ));
        }
    }
}

/// Q1 gate for the machine-readable side of the dual-nature recipe syntax
/// (C.4.5.1 — common-syntax contract, items c/h). Validates the two machine
/// fields the composer (C.4.5.17) resolves `{{vars.NAME}}` / `{{component_name}}`
/// placeholders from:
///   1. `variable_patterns` — each v3 variant's slot rules: name non-empty +
///      pattern (if present) compiles under the ReDoS size limit. Legacy
///      (`step_link == None`) variants are exempt (mirrors
///      `check_variant_descriptions`).
///   2. `step_descriptions` — when authored, must parse into the IBS
///      `StepDescriptionEntry` shape and every `include` UUID must be non-nil
///      (the composer inlines them — F3=A). Absent (`Null`) / empty is allowed
///      (legacy + variant-only drafts); the IBS parses it again at runtime.
fn check_variant_machine_form(
    variants: &[crate::types::recipe::RecipeVariant],
    step_descriptions: &serde_json::Value,
    result: &mut ValidationResult,
) {
    for (i, v) in variants.iter().enumerate() {
        if v.step_link.is_none() {
            continue;
        }
        for (j, vp) in v.variable_patterns.iter().enumerate() {
            if vp.name.trim().is_empty() {
                result.errors.push(format!(
                    "Recipe variant #{i} ('{}') variable_patterns[{j}] has empty name — required for {{{{vars.NAME}}}} binding",
                    v.variant_key
                ));
                continue;
            }
            if let Some(p) = vp
                .pattern
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                && let Err(error) = regex::RegexBuilder::new(p)
                    .size_limit(RECIPE_REGEX_SIZE_LIMIT)
                    .build()
            {
                result.errors.push(format!(
                    "Recipe variant #{i} ('{}') variable_patterns[{j}] regex invalid: {error}",
                    v.variant_key
                ));
            }
        }
    }

    if step_descriptions.is_null() {
        return;
    }
    let Some(arr) = step_descriptions.as_array() else {
        result
            .errors
            .push("Recipe step_descriptions must be a JSON array (machine form)".into());
        return;
    };
    if arr.is_empty() {
        return;
    }
    let sds: Vec<StepDescriptionEntry> = match serde_json::from_value(step_descriptions.clone()) {
        Ok(v) => v,
        Err(error) => {
            result
                .errors
                .push(format!("Recipe step_descriptions malformed: {error}"));
            return;
        }
    };
    for sd in &sds {
        for (j, step) in sd.steps.iter().enumerate() {
            for (k, u) in step.include.iter().enumerate() {
                if *u == uuid::Uuid::nil() {
                    result.errors.push(format!(
                        "StepDescription {} step {j} include[{k}] is the nil UUID — component-include must resolve to a real component",
                        sd.desc_idx
                    ));
                }
            }
        }
    }
}

/// Phase I — §template-rules: validate a single intent expression template string.
///
/// A template may contain `%` slot markers.  Rules:
/// - Two adjacent `%` slots (no literal between them) → hard error (unextractable).
/// - Both `template_prefix` and `template_suffix` empty (e.g. `"%"` or `"% in %"`) → hard error.
///
/// `template_prefix` = everything before the first `%`; `template_suffix` =
/// everything after the last `%`.  A leading-`%` template (empty prefix) is
/// valid when the suffix is non-empty.
pub fn check_intent_expression_template(expr: &str, result: &mut ValidationResult) {
    let parts: Vec<&str> = expr.split('%').collect();
    let n_slots = parts.len().saturating_sub(1);
    if n_slots == 0 {
        // No slots — plain literal expression, always valid as a template rule.
        return;
    }

    let prefix = parts[0];
    let suffix = parts[parts.len() - 1];

    // Empty anchor on both ends: no literal to anchor on.
    if prefix.trim().is_empty() && suffix.trim().is_empty() {
        result.errors.push(format!(
            "Intent expression `{expr}` has no anchor — both prefix and suffix are empty. \
             Add literal text around each `%` slot marker (e.g. `read % file`)."
        ));
        return;
    }

    // Adjacent `%` slots: consecutive empty separators.
    for window in parts.windows(2) {
        if window[0].is_empty() && window[1].is_empty() {
            result.errors.push(format!(
                "Intent expression `{expr}` has two adjacent `%` slot markers with no literal between them — slots would be unextractable."
            ));
            return;
        }
    }
}

/// Phase I — §template-rules + §variable-rules: validate all intent_examples
/// across all variants for template well-formedness and variable consistency.
///
/// Rules applied per-variant:
/// 1. template-rule checks on each intent expression (see `check_intent_expression_template`).
/// 2. If any `ToolBinding.params` in `rust_steps` references `{{vars.NAME}}` but there is no
///    matching `%` in any intent expression AND no `variable_patterns` entry for that name →
///    hard error (variable defined in bindings but no source defined).
/// 3. A `variable_patterns` entry whose `name` does not appear in any `{{vars.NAME}}` reference
///    in `tool_bindings.params` → warning (pattern defined but never used).
///
/// Note: rules 2 and 3 require the compiled `BuildInstruction` channels, which are produced
/// by `check_recipe_ibs_preflight` only when the IBS compiles cleanly. These rules share the
/// same `step_descriptions` parse, so they live in `check_recipe_ibs_preflight` instead.
fn check_variant_intent_examples(
    variants: &[crate::types::recipe::RecipeVariant],
    result: &mut ValidationResult,
) {
    for (i, v) in variants.iter().enumerate() {
        if v.step_link.is_none() {
            // Legacy variant — exempt.
            continue;
        }
        let count = v.intent_examples.len();
        if count > INTENT_EXAMPLE_MAX_COUNT {
            result.errors.push(format!(
                "Recipe variant #{i} ('{}') has {count} intent_examples — maximum is {INTENT_EXAMPLE_MAX_COUNT}.",
                v.variant_key
            ));
        }
        for (j, expr) in v.intent_examples.iter().enumerate() {
            let chars = expr.chars().count();
            if chars > INTENT_EXAMPLE_MAX_CHARS {
                result.errors.push(format!(
                    "Recipe variant #{i} ('{}') intent_examples[{j}] exceeds {INTENT_EXAMPLE_MAX_CHARS} chars ({chars} chars).",
                    v.variant_key
                ));
            }
            check_intent_expression_template(expr, result);
        }
    }
}

/// Phase I — IBS pre-flight: compile the `step_descriptions` + each variant's
/// `step_link` via `build_instruction` and surface any [`IbsError`] as Q1 hard
/// errors.  Also enforces:
///
/// * **§no-snippet** — `IbsError::UnpromotedSnippet` from the IBS is a hard error.
/// * **§shell-guard** — Tier-0 recipe with `builtin.shell` / `builtin.spawn_subagent`
///   in rust-channel `tool_bindings` → hard error.
/// * **§tier0-orchestrator-channel Rule 1** — Tier-0 + Skill UUID in
///   `orchestrator_steps` → hard error.
/// * **§tier0-orchestrator-channel Rule 2 (S7-extension)** — Tier-0 + non-empty
///   `tool_bindings` in `rust_steps` + empty `orchestrator_steps` → hard error.
/// * **§tier0-orchestrator-channel Rule 3** — Tier-0 + no `tool_bindings` +
///   empty `orchestrator_steps` → pass (nothing to supervise).
///
/// `class_code_for_uuid` is **not** called here — UUID→class resolution requires
/// a pool and is deferred to Phase N.  The §tier0 checks operate on the
/// structural class stored in each compiled `IbsRecipeStep` (`knowledge` field +
/// `include` list), which is sufficient for structural-only Q1.
fn check_recipe_ibs_preflight(recipe: &Recipe, result: &mut ValidationResult) {
    use crate::memory::instruction_builder::build_instruction;

    // Only run when step_descriptions are present and non-null.
    if recipe.step_descriptions.is_null() {
        return;
    }
    let arr = match recipe.step_descriptions.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return,
    };
    let step_descriptions: Vec<StepDescriptionEntry> =
        match serde_json::from_value(recipe.step_descriptions.clone()) {
            Ok(v) => v,
            Err(_) => return, // already caught by check_variant_machine_form
        };

    // The §shell-guard and §tier0 rules are per-variant because `llm_call_required`
    // is determined by the Recipe row flag, which is stored in each compiled variant.
    // All variants in one recipe share the same `step_descriptions`; the `step_link`
    // selects which subset applies.
    //
    // `llm_call_required` is not on `Recipe` directly — it is emitted by the IBS
    // into `BuildInstruction` and historically comes from `recipe.tier` / validation.
    // For Q1 purposes we derive it as `!recipe.is_tier0_eligible()` (the same signal
    // `fetch_for_turn` uses at runtime per Phase E).
    let llm_call_required = !recipe.is_tier0_eligible();

    let has_v3_variants = recipe.variants.iter().any(|v| v.step_link.is_some());
    if !has_v3_variants {
        // No v3 variants — nothing to IBS-compile.  step_descriptions parse was
        // already checked in check_variant_machine_form.
        return;
    }

    for (vi, variant) in recipe.variants.iter().enumerate() {
        let step_link = match &variant.step_link {
            Some(sl) => sl,
            None => continue, // legacy variant, exempt
        };

        let bi = match build_instruction(
            step_link,
            &step_descriptions,
            &variant.variable_patterns,
            llm_call_required,
        ) {
            Ok(bi) => bi,
            Err(e) => {
                result.errors.push(format!(
                    "Recipe variant #{vi} ('{}'): IBS compile error — {e}",
                    variant.variant_key
                ));
                continue;
            }
        };

        // §shell-guard (Phase I): Tier-0 + shell/spawn tool in rust_steps → hard error.
        if !bi.llm_call_required {
            for step in &bi.rust_steps {
                for binding in &step.tool_bindings {
                    if SHELL_GUARD_TOOLS.contains(&binding.tool_name.as_str()) {
                        result.errors.push(format!(
                            "§shell-guard: Recipe variant #{vi} ('{}') references `{}` in rust_steps \
                             but llm_call_required is false. \
                             Shell/spawn tools must always have llm_call_required: true.",
                            variant.variant_key, binding.tool_name
                        ));
                    }
                }
            }
        }

        // §tier0-orchestrator-channel checks (Phase I).
        if !bi.llm_call_required {
            // Rule 2 (S7-extension): rust_steps has tool_bindings but orchestrator_steps is empty.
            // The orchestrator is always the supervisory layer; a tool_binding with no
            // orchestrator PythonCode to call it is a "loaded gun nobody fires" (§AGENTS.md).
            let rust_has_bindings = bi.rust_steps.iter().any(|s| !s.tool_bindings.is_empty());
            let orchestrator_has_steps = !bi.orchestrator_steps.is_empty();

            if rust_has_bindings && !orchestrator_has_steps {
                result.errors.push(format!(
                    "§tier0-orchestrator-channel Rule 2: Recipe variant #{vi} ('{}') has tool_bindings \
                     in rust_steps but orchestrator_steps is empty. \
                     The orchestrator must supervise tool execution — add a PythonCode component \
                     that calls host.<tool>(...) and assigns to `result`.",
                    variant.variant_key
                ));
            }
            // Rule 3: Tier-0 + no tool_bindings + empty orchestrator_steps → pass (nothing to
            // supervise). No error emitted; this branch falls through cleanly.

            // Rule 1: Tier-0 + Skill-class UUID in orchestrator_steps.
            // Full enforcement requires resolving UUIDs to class_codes via the DB (Phase N).
            // At structural Q1 we emit a warning when orchestrator_steps carry `include` UUIDs
            // that do NOT also appear in rust_steps — those are orchestrator-exclusive components
            // whose class we cannot verify without a pool.  Phase N will promote this to a hard
            // error if any such UUID resolves to class 1/2/3 (Skill).
            let rust_include_uuids: std::collections::HashSet<uuid::Uuid> = bi
                .rust_steps
                .iter()
                .flat_map(|s| s.include.iter().copied())
                .collect();
            for orch_step in &bi.orchestrator_steps {
                for uuid in &orch_step.include {
                    if !rust_include_uuids.contains(uuid) {
                        // Orchestrator-exclusive include — cannot verify class at Q1.
                        // Phase N resolves class_code; if it's 1/2/3 it becomes a hard error.
                        result.warnings.push(format!(
                            "§tier0-orchestrator-channel Rule 1 (deferred to Phase N): Recipe \
                             variant #{vi} ('{}') has orchestrator-exclusive include UUID `{uuid}` \
                             in orchestrator_steps. Ensure this UUID resolves to a PythonCode \
                             (class 22) component, not a Skill (class 1/2/3). Skills cannot \
                             supervise Tier-0 execution without an LLM.",
                            variant.variant_key
                        ));
                    }
                }
            }
        }

        // §variable-rules (Phase I §template-rules item 3/4): per-variant cross-check
        // between variable_patterns and ToolBinding params `{{vars.NAME}}` references.
        check_variant_variable_consistency(vi, variant, &bi, result);
    }

    // §no-snippet: already enforced by the IBS (IbsError::UnpromotedSnippet
    // surfaces as an IBS compile error above). Document it explicitly:
    // any `step_descriptions` entry with `"type": "snippet"` causes IbsError::UnpromotedSnippet,
    // which becomes a "Recipe variant #N: IBS compile error" hard error.
    // No additional scan is needed; the IBS is the sole authoritative checker.
    let _ = arr; // used for the empty-check guard above
}

/// Phase I §template-rules items 3/4 — cross-check `variable_patterns` against
/// `{{vars.NAME}}` references in rust_steps `tool_bindings.params`.
///
/// * Any `{{vars.NAME}}` in params but no `%` in intent_examples AND no
///   `variable_patterns` entry for that name → hard error.
/// * Any `variable_patterns` entry whose name is absent from ALL params
///   `{{vars.NAME}}` refs → warning.
fn check_variant_variable_consistency(
    vi: usize,
    variant: &crate::types::recipe::RecipeVariant,
    bi: &crate::memory::instruction_builder::BuildInstruction,
    result: &mut ValidationResult,
) {
    // Collect all {{vars.NAME}} references from rust_steps tool_binding params.
    let mut bound_vars: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in &bi.rust_steps {
        for binding in &step.tool_bindings {
            collect_vars_refs(&binding.params.to_string(), &mut bound_vars);
        }
    }
    // Also scan orchestrator_steps (PythonCode bodies carry {{vars.NAME}} too).
    for step in &bi.orchestrator_steps {
        // orchestrator steps carry `include` UUIDs; the body is not available at Q1
        // (it requires fetching the PythonCode component from DB). Skip.
        let _ = step;
    }

    // Collect all slot names that ARE provided by intent_examples `%` markers.
    // Positional: slot0, slot1, … — one per `%`.
    let max_slots = variant
        .intent_examples
        .iter()
        .map(|e| e.chars().filter(|c| *c == '%').count())
        .max()
        .unwrap_or(0);
    let mut provided_slot_names: std::collections::HashSet<String> =
        (0..max_slots).map(|i| format!("slot{i}")).collect();
    // Named vars come from variable_patterns.
    let vp_names: std::collections::HashSet<&str> = variant
        .variable_patterns
        .iter()
        .map(|vp| vp.name.as_str())
        .collect();
    provided_slot_names.extend(vp_names.iter().map(|s| s.to_string()));

    // Rule: bound var not provided by slots or variable_patterns → hard error.
    for var_name in &bound_vars {
        if !provided_slot_names.contains(var_name.as_str()) {
            result.errors.push(format!(
                "§template-rules: Recipe variant #{vi} ('{}') references `{{{{vars.{var_name}}}}}` in \
                 tool_bindings params but no matching `%` slot in intent_examples and no \
                 variable_patterns entry for '{var_name}' — variable has no source.",
                variant.variant_key
            ));
        }
    }

    // Rule: variable_patterns entry not used in any bound var → warning.
    for vp_name in &vp_names {
        if !bound_vars.contains(*vp_name) {
            result.warnings.push(format!(
                "§template-rules: Recipe variant #{vi} ('{}') variable_patterns entry '{vp_name}' \
                 is not referenced in any tool_bindings `{{{{vars.NAME}}}}` — pattern defined but never used.",
                variant.variant_key
            ));
        }
    }
}

/// Extract `{{vars.NAME}}` references from a string (e.g. a serialized params JSON).
fn collect_vars_refs(s: &str, out: &mut std::collections::HashSet<String>) {
    let mut cursor = 0;
    while let Some(open_rel) = s[cursor..].find("{{vars.") {
        let open = cursor + open_rel + "{{vars.".len();
        let Some(close_rel) = s[open..].find("}}") else {
            break;
        };
        let name = s[open..open + close_rel].trim();
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            out.insert(name.to_string());
        }
        cursor = open + close_rel + 2;
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
            variants: Vec::new(),
            step_descriptions: serde_json::Value::Null,
            dependency_registry: serde_json::Value::Null,
        }
    }

    fn v3_variant(key: &str, description: Option<&str>) -> crate::types::recipe::RecipeVariant {
        // step_link "0:0-0:E" = all steps in SD0. Single-SD form so it compiles
        // cleanly against the sd_json fixture (which produces one StepDescription
        // at desc_idx=0).  The old "0:0-0:30+1:0-1:E" referenced non-existent
        // stepnumber 30 and a second SD that sd_json never supplied; it only passed
        // before because check_recipe_ibs_preflight was not yet implemented.
        crate::types::recipe::RecipeVariant {
            variant_key: key.into(),
            description: description.map(str::to_string),
            step_link: Some("0:0-0:E".into()),
            intent_examples: vec![],
            variable_patterns: vec![],
        }
    }

    #[test]
    fn v3_variant_without_description_fails_q1() {
        let mut r = base_recipe();
        r.variants = vec![v3_variant("ls-la", None)];
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result.errors.iter().any(|e| e.contains("dual-nature gate")),
            "expected dual-nature gate error, got {result:?}"
        );
    }

    #[test]
    fn legacy_variant_without_description_is_exempt() {
        let mut r = base_recipe();
        // Legacy variant: step_link == None → exempt from the description gate.
        r.variants = vec![crate::types::recipe::RecipeVariant {
            variant_key: "ls-la".into(),
            description: None,
            step_link: None,
            intent_examples: vec![],
            variable_patterns: vec![],
        }];
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result.is_ok(),
            "legacy variant must be exempt, got {result:?}"
        );
    }

    #[test]
    fn v3_variant_with_description_passes_q1() {
        let mut r = base_recipe();
        r.variants = vec![v3_variant(
            "ls-la",
            Some("List a directory including hidden files."),
        )];
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(result.is_ok(), "expected pass, got {result:?}");
    }

    fn vp(name: &str, pattern: Option<&str>) -> crate::types::ibs::VariablePattern {
        crate::types::ibs::VariablePattern {
            name: name.into(),
            pattern: pattern.map(str::to_string),
            description: None,
        }
    }

    fn sd_json(include_uuids: &[&str]) -> serde_json::Value {
        serde_json::json!([
            {
                "desc_idx": 0,
                "label": "sd0",
                "yaml_source": "steps: []",
                "steps": [
                    {
                        "stepnumber": 1,
                        "knowledge": "orchestrator",
                        "goal": "g",
                        "content": "c",
                        "type": "component",
                        "include": include_uuids
                    }
                ]
            }
        ])
    }

    #[test]
    fn v3_variant_with_valid_machine_form_passes_q1() {
        let mut r = base_recipe();
        let mut v = v3_variant("ls-la", Some("List a directory including hidden files."));
        v.variable_patterns = vec![vp("dir", Some("^[a-z0-9/]+$"))];
        r.variants = vec![v];
        r.step_descriptions = sd_json(&["11111111-1111-1111-1111-111111111111"]);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(result.is_ok(), "expected pass, got {result:?}");
    }

    #[test]
    fn v3_variant_with_invalid_variable_pattern_regex_fails_q1() {
        let mut r = base_recipe();
        let mut v = v3_variant("ls-la", Some("List a directory including hidden files."));
        v.variable_patterns = vec![vp("dir", Some("("))];
        r.variants = vec![v];
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result.errors.iter().any(|e| e.contains("regex invalid")),
            "expected regex invalid error, got {result:?}"
        );
    }

    #[test]
    fn v3_variant_with_empty_variable_pattern_name_fails_q1() {
        let mut r = base_recipe();
        let mut v = v3_variant("ls-la", Some("List a directory including hidden files."));
        v.variable_patterns = vec![vp("  ", None)];
        r.variants = vec![v];
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result.errors.iter().any(|e| e.contains("empty name")),
            "expected empty-name error, got {result:?}"
        );
    }

    #[test]
    fn v3_variant_with_nil_include_uuid_fails_q1() {
        let mut r = base_recipe();
        r.variants = vec![v3_variant(
            "ls-la",
            Some("List a directory including hidden files."),
        )];
        r.step_descriptions = sd_json(&["00000000-0000-0000-0000-000000000000"]);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result.errors.iter().any(|e| e.contains("nil UUID")),
            "expected nil UUID error, got {result:?}"
        );
    }

    #[test]
    fn v3_variant_with_malformed_step_descriptions_fails_q1() {
        let mut r = base_recipe();
        r.variants = vec![v3_variant(
            "ls-la",
            Some("List a directory including hidden files."),
        )];
        r.step_descriptions = serde_json::json!([{"not": "the step_description shape"}]);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("step_descriptions malformed")),
            "expected malformed step_descriptions error, got {result:?}"
        );
    }

    // ── Phase I: §shell-guard, §tier0-orchestrator-channel, §no-snippet,
    //            §template-rules, intent_examples ──────────────────────

    // Helpers to build fixture step_descriptions with a single SD + given steps.
    fn make_sd(steps: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!([{
            "desc_idx": 0,
            "label": "sd0",
            "yaml_source": "steps: []",
            "steps": steps
        }])
    }

    fn step_orchestrator_component(stepnumber: u32, include: &str) -> serde_json::Value {
        serde_json::json!({
            "stepnumber": stepnumber,
            "knowledge": "orchestrator",
            "goal": "run python",
            "content": "executor body",
            "type": "component",
            "include": [include]
        })
    }

    fn step_rust_with_binding(
        stepnumber: u32,
        include: &str,
        tool_name: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "stepnumber": stepnumber,
            "knowledge": "rust",
            "goal": "run tool",
            "content": "invoke tool",
            "type": "component",
            "include": [include],
            "tool_bindings": [{
                "tool_id": "00000000-0000-0000-0000-000000000001",
                "tool_name": tool_name,
                "params": {},
                "error_policy": { "policy": "fail" }
            }]
        })
    }

    fn step_snippet(stepnumber: u32) -> serde_json::Value {
        serde_json::json!({
            "stepnumber": stepnumber,
            "knowledge": "orchestrator",
            "goal": "snippet step",
            "content": "draft content",
            "type": "snippet",
            "include": []
        })
    }

    fn tier0_recipe_with_sds(step_descriptions: serde_json::Value) -> Recipe {
        let mut r = base_recipe();
        // is_tier0_eligible() == true: tier=mature/candidate + validated + wilson>=0.70 + has_validation.
        r.tier = "candidate".into();
        r.validation_status = crate::types::recipe::ValidationStatus::Validated;
        r.wilson_lower = 0.75;
        // validation already set to ShellCheck in base_recipe().
        r.variants = vec![crate::types::recipe::RecipeVariant {
            variant_key: "default".into(),
            description: Some("Direct execution variant.".into()),
            step_link: Some("0:0-0:E".into()),
            intent_examples: vec![],
            variable_patterns: vec![],
        }];
        r.step_descriptions = step_descriptions;
        r
    }

    fn tier1_recipe_with_sds(step_descriptions: serde_json::Value) -> Recipe {
        let mut r = base_recipe();
        // is_tier0_eligible() == false: tier=growing, not validated.
        r.tier = "growing".into();
        r.variants = vec![crate::types::recipe::RecipeVariant {
            variant_key: "default".into(),
            description: Some("Tier-1 variant.".into()),
            step_link: Some("0:0-0:E".into()),
            intent_examples: vec![],
            variable_patterns: vec![],
        }];
        r.step_descriptions = step_descriptions;
        r
    }

    // ── §no-snippet ───────────────────────────────────────────────────

    #[test]
    fn recipe_with_snippet_step_fails_q1_ibs_error() {
        // §no-snippet: IbsError::UnpromotedSnippet surfaces as a Q1 hard error.
        let uuid = uuid::Uuid::new_v4().to_string();
        let r = tier0_recipe_with_sds(make_sd(vec![step_snippet(1)]));
        let _ = uuid;
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("IBS compile error") && e.contains("snippet")),
            "expected IBS UnpromotedSnippet hard error, got {result:?}"
        );
    }

    // ── §shell-guard ──────────────────────────────────────────────────

    #[test]
    fn shell_guard_tier0_with_builtin_shell_fails_q1() {
        // §shell-guard: Tier-0 + builtin.shell in rust_steps → Q1 hard error.
        let orch_uuid = uuid::Uuid::new_v4().to_string();
        let rust_uuid = uuid::Uuid::new_v4().to_string();
        let sds = make_sd(vec![
            step_rust_with_binding(1, &rust_uuid, "builtin.shell"),
            step_orchestrator_component(2, &orch_uuid),
        ]);
        let r = tier0_recipe_with_sds(sds);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("shell-guard") && e.contains("builtin.shell")),
            "expected §shell-guard hard error for builtin.shell Tier-0, got {result:?}"
        );
    }

    #[test]
    fn shell_guard_tier0_with_spawn_subagent_fails_q1() {
        // §shell-guard: Tier-0 + builtin.spawn_subagent in rust_steps → Q1 hard error.
        let orch_uuid = uuid::Uuid::new_v4().to_string();
        let rust_uuid = uuid::Uuid::new_v4().to_string();
        let sds = make_sd(vec![
            step_rust_with_binding(1, &rust_uuid, "builtin.spawn_subagent"),
            step_orchestrator_component(2, &orch_uuid),
        ]);
        let r = tier0_recipe_with_sds(sds);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("shell-guard") && e.contains("builtin.spawn_subagent")),
            "expected §shell-guard hard error for spawn_subagent Tier-0, got {result:?}"
        );
    }

    #[test]
    fn shell_guard_tier1_with_builtin_shell_passes_q1() {
        // §shell-guard: Tier-1 (llm_call_required=true) + builtin.shell → Q1 pass.
        let orch_uuid = uuid::Uuid::new_v4().to_string();
        let rust_uuid = uuid::Uuid::new_v4().to_string();
        let sds = make_sd(vec![
            step_rust_with_binding(1, &rust_uuid, "builtin.shell"),
            step_orchestrator_component(2, &orch_uuid),
        ]);
        let r = tier1_recipe_with_sds(sds);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result.errors.iter().all(|e| !e.contains("shell-guard")),
            "expected no §shell-guard error for Tier-1 recipe, got {result:?}"
        );
    }

    // ── §tier0-orchestrator-channel ────────────────────────────────────

    #[test]
    fn tier0_rule2_rust_bindings_no_orchestrator_steps_fails_q1() {
        // Rule 2 (S7-extension): Tier-0 + non-empty tool_bindings in rust_steps
        // + empty orchestrator_steps → hard error.
        //
        // The IBS S7 guard fires first (same invariant: rust has tool_bindings but
        // orchestrator has no `include`) and surfaces as an "IBS compile error — S7
        // violation" error.  Our check_recipe_ibs_preflight Rule 2 would also fire
        // if the IBS somehow passed, but the IBS is the earlier enforcer.  Accept
        // either message — the architectural invariant is enforced.
        let rust_uuid = uuid::Uuid::new_v4().to_string();
        let sds = make_sd(vec![step_rust_with_binding(
            1,
            &rust_uuid,
            "builtin.read_file",
        )]);
        let r = tier0_recipe_with_sds(sds);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result.errors.iter().any(|e| {
                e.contains("tier0-orchestrator-channel Rule 2")
                    || (e.contains("IBS compile error") && e.contains("S7"))
            }),
            "expected §tier0-orchestrator-channel Rule 2 or IBS S7 hard error, got {result:?}"
        );
    }

    #[test]
    fn tier0_rule2_satisfied_rust_bindings_with_orchestrator_step_passes_q1() {
        // Rule 2 satisfied: Tier-0 + tool_bindings + PythonCode in orchestrator_steps.
        let orch_uuid = uuid::Uuid::new_v4().to_string();
        let rust_uuid = uuid::Uuid::new_v4().to_string();
        let sds = make_sd(vec![
            step_rust_with_binding(1, &rust_uuid, "builtin.read_file"),
            step_orchestrator_component(2, &orch_uuid),
        ]);
        let r = tier0_recipe_with_sds(sds);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .all(|e| !e.contains("tier0-orchestrator-channel Rule 2")),
            "expected no Rule 2 error when orchestrator step present, got {result:?}"
        );
    }

    #[test]
    fn tier0_rule3_no_bindings_empty_orchestrator_passes_q1() {
        // Rule 3: Tier-0 + no tool_bindings in rust_steps + empty orchestrator_steps → pass.
        // A rust step with no tool_bindings is a context-loading step (no tool call).
        let rust_uuid = uuid::Uuid::new_v4().to_string();
        let step = serde_json::json!({
            "stepnumber": 1,
            "knowledge": "rust",
            "goal": "load context",
            "content": "context",
            "type": "component",
            "include": [&rust_uuid],
            "tool_bindings": []
        });
        let r = tier0_recipe_with_sds(make_sd(vec![step]));
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .all(|e| !e.contains("tier0-orchestrator-channel")),
            "expected no §tier0-orchestrator-channel error for no-binding Tier-0, got {result:?}"
        );
    }

    #[test]
    fn tier1_with_orchestrator_step_passes_q1() {
        // Tier-1 recipes may use Skills (class 1/2/3) or PythonCode freely in
        // orchestrator_steps — the §tier0-orchestrator-channel rules only apply to Tier-0.
        let orch_uuid = uuid::Uuid::new_v4().to_string();
        let sds = make_sd(vec![step_orchestrator_component(1, &orch_uuid)]);
        let r = tier1_recipe_with_sds(sds);
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .all(|e| !e.contains("tier0-orchestrator-channel")),
            "expected no §tier0-orchestrator-channel error for Tier-1, got {result:?}"
        );
    }

    // ── §template-rules ─────────────────────────────────────────────

    #[test]
    fn intent_example_over_512_chars_fails_q1() {
        let mut r = base_recipe();
        r.variants = vec![crate::types::recipe::RecipeVariant {
            variant_key: "v1".into(),
            description: Some("A variant.".into()),
            step_link: Some("0:0-0:E".into()),
            intent_examples: vec!["x".repeat(513)],
            variable_patterns: vec![],
        }];
        let result = RecipeValidator::validate_recipe(&r, &["step-skill".into()]);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("exceeds") && e.contains("512")),
            "expected intent_examples length hard error, got {result:?}"
        );
    }

    #[test]
    fn intent_example_no_anchor_fails_q1() {
        // §template-rules: `"%"` has no prefix or suffix anchor → hard error.
        let mut result = crate::memory::recipe_validator::ValidationResult::ok();
        check_intent_expression_template("%", &mut result);
        assert!(
            result.errors.iter().any(|e| e.contains("no anchor")),
            "expected no-anchor error for `%`, got {result:?}"
        );
    }

    #[test]
    fn intent_example_adjacent_slots_fails_q1() {
        // §template-rules: `"% %"` has two adjacent `%` with a space but the
        // windows check needs consecutive empty segments — test the exact failing
        // case of `"%%"` (no separator between two `%`s).
        let mut result = crate::memory::recipe_validator::ValidationResult::ok();
        check_intent_expression_template("search %%", &mut result);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("adjacent") || e.contains("no anchor")),
            "expected adjacent-slots error for `search %%`, got {result:?}"
        );
    }

    #[test]
    fn intent_example_valid_anchor_passes() {
        // §template-rules: `"read % file"` has prefix + suffix → valid.
        let mut result = crate::memory::recipe_validator::ValidationResult::ok();
        check_intent_expression_template("read % file", &mut result);
        assert!(
            result.errors.is_empty(),
            "expected no error for `read % file`, got {result:?}"
        );
    }

    #[test]
    fn intent_example_leading_percent_valid_with_suffix() {
        // §template-rules: `"% directory"` leading-`%` with suffix → valid (warning only, no hard error).
        let mut result = crate::memory::recipe_validator::ValidationResult::ok();
        check_intent_expression_template("% directory", &mut result);
        assert!(
            result.errors.is_empty(),
            "expected no hard error for leading-percent with suffix, got {result:?}"
        );
    }
}
