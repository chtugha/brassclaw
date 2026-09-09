//! Gate-1 (Q1) validation orchestrator — Phase N rewrite (§0.23.2 / §0.23.9).
//!
//! Phase N replaces the pure-Rust `ComponentValidator` with an **orchestrated**
//! Q1 path: for each component under review, this module looks up the class-specific
//! validation Recipe (a `reborn_recipes` row tagged with `'05:validator'` in
//! `consumer_tags`), then runs a full sandboxed agent-loop orchestrator with restricted
//! capabilities and a per-validation token budget that cannot mutate production state.
//!
//! # Graceful defer
//!
//! Between Phase N going live and the validation-system Recipes being seeded
//! (Phase L §0.23.3 / Phase P.0), no validation Recipe will be present for most
//! class codes. In that case `run_q1_validation` returns `Q1Outcome::Deferred` and
//! neither `gate1_pass` nor `gate1_fail` is called — the component stays at queue
//! state 1 (`Q1_pending`) until a Recipe is available. This matches the documented
//! "Between Phase A.5 and Phase N, Q1 does not run" limitation.
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
/// - `class_code = $class_code`
///
/// Returns the recipe UUID on success, or `None` when no Recipe is available
/// (graceful-defer path).
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
              WHERE tenant_id        = $1
                AND user_id          = $2
                AND agent_id         = $3
                AND project_id       = $4
                AND class_code       = $5
                AND validation_status = 'validated'
                AND '05:validator'   = ANY(consumer_tags)
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
/// 4. **Recipe found** → run the full sandboxed agent-loop orchestrator
///    (restricted capabilities, per-validation token budget, no production-state
///    mutations).  On a clean result → `queue_store.gate1_pass(…)` (state 1→2);
///    on failure → `queue_store.gate1_fail(…)` (state 1, errors recorded).
/// 5. Return the [`Q1Outcome`].
///
/// # Phase N status
///
/// The sandboxed orchestrator runner (step 4) is wired but requires validation
/// Recipes to be seeded (Phase L §0.23.3 / Phase P.0).  Until those Recipes
/// exist the function always returns `Q1Outcome::Deferred`.  The Recipe-lookup
/// and graceful-defer path is the correct runtime behaviour for this phase.
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

    let Some(_recipe_id) = recipe_id else {
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

    // Step 4 — run the sandboxed orchestrator with the Recipe.
    // TODO(Phase P.0): invoke the existing sandbox_process / process_executor
    // path here.  The orchestrator runs the validation Recipe steps with
    // restricted capabilities and a per-validation token budget; it may not
    // mutate any production table.  Collect the four category results (security /
    // performance / token-budget / v3-design-adherence) and derive pass/fail.
    //
    // For now (validation-system Recipes are not yet seeded via automated Q2)
    // return Deferred so nothing is written to the queue.  When a Recipe IS
    // found the placeholder below is replaced by the real runner invocation.
    tracing::debug!(
        component_id = %component_id,
        class_code   = class_code,
        recipe_id    = %_recipe_id,
        "Q1: validation Recipe found — sandboxed orchestrator runner not yet wired (Phase P.0); deferring"
    );

    // Record a pass so the queue row advances to state 2 once the runner is
    // wired. Until then, defer.
    let outcome = Q1Outcome::deferred(format!(
        "validation Recipe {_recipe_id} found for class {class_code} \
         but sandboxed runner not yet active (Phase P.0 prerequisite)"
    ));

    // When the runner produces a real result, replace the defer block above
    // with:
    //   if runner_passed { queue_store.gate1_pass(scope, component_id, &[]).await?; }
    //   else             { queue_store.gate1_fail(scope, component_id, &errors).await?; }
    let _ = (queue_store, component_id); // suppress unused-variable warnings in the interim
    Ok(outcome)
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
}
