//! Gate-1 (Q1) validation orchestrator — Phase N rewrite (§0.23.2 / §0.23.9).
//!
//! Phase N replaces the pure-Rust `ComponentValidator` with an **orchestrated**
//! Q1 path: for each component under review, this module looks up the class-specific
//! validation Recipe (a `reborn_recipes` row tagged with `'05:validator'` in
//! `consumer_tags` AND `validates_class_code = <class>` from V079), then runs the
//! Tier-0 execution channel (`execute_tier_zero_channel`) with the Recipe's
//! orchestrator content in a restricted sandbox.
//!
//! # Graceful defer
//!
//! Before validation Recipes are seeded (`validates_class_code` rows, Phase L §0.23.3),
//! `find_validator_recipe` returns `None` and `run_q1_validation` returns
//! `Q1Outcome::Deferred`. The component stays at queue state 1 (`Q1_pending`) until
//! a Recipe is available.
//!
//! # Column fix (V079 — FIND-P0-01)
//!
//! The original query filtered `reborn_recipes.class_code = $class` but
//! `class_code` on `reborn_recipes` is always 21 (enforced by CHECK constraint) —
//! it means "this row IS a Recipe", not "this Recipe validates class X".
//! V079 adds `validates_class_code SMALLINT` to carry that meaning.
//! `find_validator_recipe` now filters on `validates_class_code = $class`.
//!
//! # State-2 write invariant (FIND-P9-01 / FIND-P9-08)
//!
//! Only this module (`run_q1_validation`) may write state 2 by calling
//! `gate1_pass`. Both `gate1_pass` and `gate1_fail` are `pub(crate)` on
//! [`ValidationQueueStore`], preventing any API layer from calling them directly.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

#![forbid(unsafe_code)]

use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_engine::run_python_code_body;
use brassclaw_pg::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::validation_queue::{ValidationQueueError, ValidationQueueStore};

// ---------------------------------------------------------------------------
// Outcome
// ---------------------------------------------------------------------------

/// The outcome of a Gate 1 (Q1) validation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Q1Outcome {
    /// Gate 1 ran and the component passed all checks.  `gate1_pass` has been
    /// called — the queue row is now at state 2 (awaiting Q2).
    Passed,

    /// Gate 1 ran and the component failed one or more checks. `gate1_fail` has
    /// been called — the queue row stays at state 1 with recorded errors.
    Failed { errors: Vec<String> },

    /// No validation Recipe was found for this class code. Q1 was **not** run;
    /// the queue row remains at state 1 until a Recipe is seeded.  This is the
    /// expected result between Phase N going live and the validation-system
    /// trusted-root being fully seeded (Phase P.0).
    Deferred { reason: String },
}

impl Q1Outcome {
    /// Convenience constructor — clean pass.
    pub fn passed() -> Self {
        Self::Passed
    }

    /// Convenience constructor — failure with a list of human-readable errors.
    pub fn failed(errors: Vec<String>) -> Self {
        Self::Failed { errors }
    }

    /// Convenience constructor — deferred because no Recipe was available.
    pub fn deferred(reason: impl Into<String>) -> Self {
        Self::Deferred {
            reason: reason.into(),
        }
    }

    /// `true` when the component cleared Gate 1.
    pub fn passed_gate(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors raised by [`run_q1_validation`].
#[derive(Debug, Error)]
pub enum Q1Error {
    #[error("validation queue error: {0}")]
    Queue(#[from] ValidationQueueError),
    #[error("database error: {reason}")]
    Db { reason: String },
    #[error("invalid class code {class_code}: out of i16 range")]
    InvalidClassCode { class_code: i32 },
}

// ---------------------------------------------------------------------------
// Recipe lookup
// ---------------------------------------------------------------------------

/// Look up a validated validation Recipe for `class_code` within the given
/// scope.
///
/// Searches `reborn_recipes` for a row where:
/// - `validation_status = 'validated'`
/// - `'05:validator' = ANY(consumer_tags)`
/// - `validates_class_code = $class_code`  ← V079 column (not `class_code`
///   which is always 21 by DDL constraint and means "this IS a Recipe row")
///
/// Returns the recipe UUID on success, or `None` when no Recipe is available
/// (graceful-defer path — before Phase L §0.23.3 seeding).
async fn find_validator_recipe(
    pool: &PgPool,
    scope: &ComponentScope,
    class_code: i16,
) -> Result<Option<Uuid>, Q1Error> {
    let client = pool.get().await.map_err(|e| Q1Error::Db {
        reason: e.to_string(),
    })?;

    let row = client
        .query_opt(
            "SELECT id FROM reborn_recipes
              WHERE tenant_id           = $1
                AND user_id             = $2
                AND agent_id            = $3
                AND project_id          = $4
                AND validates_class_code = $5
                AND validation_status   = 'validated'
                AND '05:validator'      = ANY(consumer_tags)
              LIMIT 1",
            &[
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
                &class_code,
            ],
        )
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;

    Ok(row.map(|r| r.get::<_, Uuid>(0)))
}

// ---------------------------------------------------------------------------
// Helpers: fetch component content fields for the structural check
// ---------------------------------------------------------------------------

/// The three content fields fetched from a component row for structural
/// validation: name, description, and the main content/body/steps text.
struct ComponentContentFields {
    name: String,
    description: String,
    /// The "content" field — column name varies by class:
    /// - class 0 (Tool): `capability_id` (the registered tool name)
    /// - class 1/2/3 (Skill): `body`
    /// - class 13 (ToolSkill): `content`
    /// - class 21 (Recipe): `description` (steps are JSONB; description is the proxy)
    /// - class 22 (PythonCode): `content`
    /// - class 23 (Catalogue): `overview_doc`
    content: String,
}

/// Fetch the three content fields for a component by `(scope, id, class_code)`.
///
/// Each component class stores its primary text content in a different column.
/// For classes whose "content" is opaque JSONB (`steps` on Recipes) the
/// description is used as a proxy — structural validators check that the
/// description is non-empty, which is sufficient for this tier.
///
/// Returns `None` when the row is not found (wrong scope or id).
async fn fetch_component_content_fields(
    pool: &PgPool,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i16,
) -> Result<Option<ComponentContentFields>, Q1Error> {
    let client = pool.get().await.map_err(|e| Q1Error::Db {
        reason: e.to_string(),
    })?;

    // SELECT the three fields from the class-appropriate table.
    // All tables share the same (tenant_id, user_id, agent_id, project_id, id) PK shape.
    let (table, content_col) = match class_code {
        0 => ("reborn_tools", "COALESCE(capability_id, '') AS content"),
        1..=3 => ("reborn_skills", "body AS content"),
        13 => ("reborn_tool_skills", "COALESCE(content, '') AS content"),
        21 => ("reborn_recipes", "description AS content"),
        22 => ("reborn_python_code", "content"),
        23 => ("reborn_extension_catalogues", "COALESCE(overview_doc, '') AS content"),
        _ => {
            // Unknown class — cannot validate, defer.
            return Ok(None);
        }
    };
    let sql = format!(
        "SELECT name, COALESCE(description, '') AS description, {content_col} \
         FROM {table} \
         WHERE id = $1 \
           AND tenant_id = $2 AND user_id = $3 \
           AND agent_id  = $4 AND project_id = $5 \
         LIMIT 1"
    );
    let row = client
        .query_opt(
            &sql,
            &[
                &component_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;
    Ok(row.map(|r| ComponentContentFields {
        name: r.get::<_, String>(0),
        description: r.get::<_, String>(1),
        content: r.get::<_, String>(2),
    }))
}

/// Fetch the PythonCode body for the first orchestrator step of a validator
/// Recipe.  The seeded validator Recipes (Phase L §0.23.3) store one
/// `step_descriptions` entry whose `steps[0].include[0]` is the PythonCode UUID.
/// This function extracts that UUID and fetches the PythonCode `content` column.
///
/// Returns `None` when the Recipe has no parseable orchestrator step or the
/// referenced PythonCode row is missing.
async fn fetch_validator_pc_body(
    pool: &PgPool,
    scope: &ComponentScope,
    recipe_id: Uuid,
) -> Result<Option<String>, Q1Error> {
    let client = pool.get().await.map_err(|e| Q1Error::Db {
        reason: e.to_string(),
    })?;

    // 1. Fetch the recipe's step_descriptions JSONB.
    let recipe_row = client
        .query_opt(
            "SELECT step_descriptions \
             FROM reborn_recipes \
             WHERE id = $1 \
               AND tenant_id = $2 AND user_id = $3 \
               AND agent_id  = $4 AND project_id = $5 \
             LIMIT 1",
            &[
                &recipe_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;

    let Some(recipe_row) = recipe_row else {
        return Ok(None);
    };

    // 2. Extract the PythonCode UUID from step_descriptions[0].steps[0].include[0].
    let step_desc: Option<serde_json::Value> = recipe_row.get(0);
    let Some(step_desc) = step_desc else {
        return Ok(None);
    };
    let pc_id_str = step_desc
        .get(0)
        .and_then(|d| d.get("steps"))
        .and_then(|s| s.get(0))
        .and_then(|s| s.get("include"))
        .and_then(|inc| inc.get(0))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if pc_id_str.is_empty() {
        return Ok(None);
    }
    let pc_id: Uuid = match pc_id_str.parse() {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };

    // 3. Fetch the PythonCode body.
    let pc_row = client
        .query_opt(
            "SELECT content \
             FROM reborn_python_code \
             WHERE id = $1 \
               AND tenant_id = $2 AND user_id = $3 \
               AND agent_id  = $4 AND project_id = $5 \
             LIMIT 1",
            &[
                &pc_id,
                &scope.tenant_id,
                &scope.user_id,
                &scope.agent_id,
                &scope.project_id,
            ],
        )
        .await
        .map_err(|e| Q1Error::Db {
            reason: e.to_string(),
        })?;

    Ok(pc_row.map(|r| r.get::<_, String>(0)))
}

/// Run the validator PythonCode body in the lightweight Monty sandbox.
///
/// Bakes `{{vars.slot0}}` / `{{vars.slot1}}` / `{{vars.slot2}}` into the body
/// with the component's name / description / content respectively (the IBS
/// substitution the full engine loop would normally perform).  Then calls
/// [`run_python_code_body`] — a pure-logic, no-host-call executor.
///
/// Returns `(passed: bool, errors: Vec<String>)` from the body's
/// `{"pass": bool, "errors": [...]}` return value.  On any execution error
/// the result is treated as a failure with the error message as the sole error.
fn run_validator_python(
    pc_body: &str,
    name: &str,
    description: &str,
    content: &str,
) -> (bool, Vec<String>) {
    // Bake IBS slot substitutions into the body (literal text replacement).
    // The validator bodies use {{vars.slot0/1/2}} as placeholder text inside
    // string literals: `_name = "{{vars.slot0}}"`.  We replace the whole
    // placeholder including the surrounding quotes so the body gets a clean
    // Python string literal with the real value.
    let body = pc_body
        .replace("\"{{vars.slot0}}\"", &format!("{:?}", name))
        .replace("\"{{vars.slot1}}\"", &format!("{:?}", description))
        .replace("\"{{vars.slot2}}\"", &format!("{:?}", content));

    // Append `result` as the final expression so run_python_code_body returns it.
    let body_with_return = format!("{body}\nresult");

    match run_python_code_body(&body_with_return, &[]) {
        Ok(Some(val)) => {
            let passed = val
                .get("pass")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let errors: Vec<String> = val
                .get("errors")
                .and_then(|e| e.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            (passed, errors)
        }
        Ok(None) => (
            false,
            vec!["validator returned None — expected {pass, errors} dict".into()],
        ),
        Err(e) => (false, vec![format!("validator execution error: {e}")]),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run Gate 1 (Q1) validation for one component and record the result on the
/// validation queue (FIND-P9-01 / §0.23.9).
///
/// # Flow
///
/// 1. Convert `class_code` to `i16`; return `Q1Error::InvalidClassCode` if out
///    of range.
/// 2. Query `reborn_recipes` for a validated Recipe tagged `'05:validator'` for
///    `class_code` in the component's scope.
/// 3. **No Recipe found** → return `Q1Outcome::Deferred` without touching the
///    queue (component stays at state 1).
/// 4. **Recipe found** → fetch the component's content fields and the validator
///    PythonCode body; execute the pure-logic structural check via
///    [`run_python_code_body`] (no engine deps required for Tier-0 validators).
///    On pass → `queue_store.gate1_pass(…)` (state 1→2).
///    On fail → `queue_store.gate1_fail(…)` (state 1, errors recorded).
/// 5. Return the [`Q1Outcome`].
///
/// # Graceful degrade paths
///
/// - No Recipe found → `Deferred` (component stays at state 1).
/// - Recipe found but PythonCode body unavailable → `Deferred` (runner cannot
///   execute without a body; component stays at state 1).
/// - Component row not found (wrong scope/id) → `Deferred`.
pub async fn run_q1_validation(
    pool: &PgPool,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i32,
    queue_store: &ValidationQueueStore,
) -> Result<Q1Outcome, Q1Error> {
    let class_i16: i16 = class_code
        .try_into()
        .map_err(|_| Q1Error::InvalidClassCode { class_code })?;

    // Step 2 — look up validation Recipe for this class.
    let recipe_id = find_validator_recipe(pool, scope, class_i16).await?;

    let Some(recipe_id) = recipe_id else {
        // Graceful defer: no validation Recipe seeded for this class yet.
        // Q1 cannot run; component stays at state 1 until a Recipe arrives.
        tracing::debug!(
            component_id = %component_id,
            class_code   = class_code,
            "Q1: no validated validation Recipe found for class — deferring"
        );
        return Ok(Q1Outcome::deferred(format!(
            "no validated validation Recipe found for class {class_code}"
        )));
    };

    // Step 3 — fetch the validator PythonCode body from the Recipe.
    let Some(pc_body) = fetch_validator_pc_body(pool, scope, recipe_id).await? else {
        tracing::debug!(
            component_id = %component_id,
            class_code   = class_code,
            recipe_id    = %recipe_id,
            "Q1: validator Recipe found but PythonCode body unavailable — deferring"
        );
        return Ok(Q1Outcome::deferred(format!(
            "validator Recipe {recipe_id} found for class {class_code} \
             but PythonCode body could not be fetched"
        )));
    };

    // Step 4 — fetch the component's name / description / content fields.
    let Some(fields) =
        fetch_component_content_fields(pool, scope, component_id, class_i16).await?
    else {
        tracing::debug!(
            component_id = %component_id,
            class_code   = class_code,
            "Q1: component row not found — deferring"
        );
        return Ok(Q1Outcome::deferred(format!(
            "component {component_id} (class {class_code}) not found in scope"
        )));
    };

    // Step 5 — run the pure-logic structural validator in the Monty sandbox.
    let (passed, errors) = run_validator_python(&pc_body, &fields.name, &fields.description, &fields.content);

    tracing::debug!(
        component_id = %component_id,
        class_code   = class_code,
        recipe_id    = %recipe_id,
        passed,
        errors = ?errors,
        "Q1: structural validator ran"
    );

    // Step 6 — record the gate result on the queue.
    if passed {
        queue_store
            .gate1_pass(scope, component_id, &[])
            .await?;
        Ok(Q1Outcome::Passed)
    } else {
        queue_store
            .gate1_fail(scope, component_id, &errors)
            .await?;
        Ok(Q1Outcome::Failed { errors })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q1outcome_deferred_is_not_passed() {
        let d = Q1Outcome::deferred("no recipe");
        assert!(!d.passed_gate());
        assert!(matches!(d, Q1Outcome::Deferred { .. }));
    }

    #[test]
    fn q1outcome_passed_is_passed() {
        assert!(Q1Outcome::passed().passed_gate());
    }

    #[test]
    fn q1outcome_failed_is_not_passed() {
        let f = Q1Outcome::failed(vec!["oops".into()]);
        assert!(!f.passed_gate());
        assert!(matches!(f, Q1Outcome::Failed { .. }));
    }

    #[test]
    fn q1_invalid_class_code_is_i16_range_check() {
        // i32::MAX does not fit in i16 — the error variant is correct.
        let too_large: i32 = i32::MAX;
        let result: Result<i16, _> = too_large.try_into();
        assert!(
            result.is_err(),
            "i32::MAX should not fit in i16 — confirms the range check fires"
        );
    }

    // ---------------------------------------------------------------------------
    // run_validator_python unit tests (Phase P.0 Step 5)
    // ---------------------------------------------------------------------------

    /// The shared structural validator body (copy of PC_VALIDATOR_STRUCTURAL_BODY
    /// from builtin_bootstrap.rs — kept inline so the test is self-contained).
    const TEST_VALIDATOR_BODY: &str = r#"_name = "{{vars.slot0}}"
_desc = "{{vars.slot1}}"
_content = "{{vars.slot2}}"
_errors = []
if not _name or not _name.strip():
    _errors.append("name is empty")
if not _desc or not _desc.strip():
    _errors.append("description is empty")
if not _content or not _content.strip():
    _errors.append("content/steps is empty")
result = {"pass": len(_errors) == 0, "errors": _errors}
"#;

    #[test]
    fn run_validator_python_passes_when_all_fields_present() {
        let (passed, errors) =
            run_validator_python(TEST_VALIDATOR_BODY, "my-tool", "does something useful", "real body");
        assert!(passed, "all fields non-empty — should pass");
        assert!(errors.is_empty(), "no errors expected, got: {errors:?}");
    }

    #[test]
    fn run_validator_python_fails_when_name_empty() {
        let (passed, errors) = run_validator_python(TEST_VALIDATOR_BODY, "", "desc", "content");
        assert!(!passed);
        assert!(
            errors.iter().any(|e| e.contains("name")),
            "expected 'name is empty' error, got: {errors:?}"
        );
    }

    #[test]
    fn run_validator_python_fails_when_description_empty() {
        let (passed, errors) = run_validator_python(TEST_VALIDATOR_BODY, "name", "", "content");
        assert!(!passed);
        assert!(
            errors.iter().any(|e| e.contains("description")),
            "expected 'description is empty' error, got: {errors:?}"
        );
    }

    #[test]
    fn run_validator_python_fails_when_content_empty() {
        let (passed, errors) = run_validator_python(TEST_VALIDATOR_BODY, "name", "desc", "");
        assert!(!passed);
        assert!(
            errors.iter().any(|e| e.contains("content")),
            "expected 'content/steps is empty' error, got: {errors:?}"
        );
    }

    #[test]
    fn run_validator_python_fails_when_all_empty() {
        let (passed, errors) = run_validator_python(TEST_VALIDATOR_BODY, "", "", "");
        assert!(!passed);
        assert_eq!(errors.len(), 3, "expected 3 errors (name + desc + content), got: {errors:?}");
    }

    #[test]
    fn run_validator_python_treats_whitespace_only_as_empty() {
        let (passed, errors) = run_validator_python(TEST_VALIDATOR_BODY, "  ", "desc", "content");
        assert!(!passed, "whitespace-only name should fail the .strip() check");
        assert!(
            errors.iter().any(|e| e.contains("name")),
            "expected 'name is empty' error, got: {errors:?}"
        );
    }
}
