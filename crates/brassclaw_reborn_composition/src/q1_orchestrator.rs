//! Q1 validation orchestrator — the cross-crate wiring that runs the
//! deterministic Gate 1 check and records the result on the validation queue
//! (FIND-P9-01).
//!
//! `ComponentValidator` lives in `brassclaw_engine` and is a pure, I/O-free
//! function. `gate1_pass` / `gate1_fail` are `pub(crate)` on
//! [`ValidationQueueStore`] in this crate (`brassclaw_reborn_composition`).
//! `brassclaw_engine` cannot call `pub(crate)` methods here, so the Q1
//! sequence (validate → record on the queue) MUST live in this crate. This
//! module is that wiring.
//!
//! # Feature gate
//!
//! Requires the `postgres` feature.

// Wired into the WebUI-save path for classes 22/23 (Phase B/C) and the
// boot-integrity pass for all classes (Phase N); the orchestration API itself
// is complete and tested here.
#![allow(dead_code)]
#![forbid(unsafe_code)]

use brassclaw_engine::memory::component_validator::{
    ComponentPayload, ComponentValidator, ValidationConfig,
};
use brassclaw_engine::memory::recipe_validator::ValidationResult;
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::validation_queue::{ValidationQueueError, ValidationQueueStore};

/// The outcome of a Gate 1 (Q1) validation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Q1Outcome {
    pub passed: bool,
    pub errors: Vec<String>,
}

impl Q1Outcome {
    pub fn passed() -> Self {
        Self {
            passed: true,
            errors: Vec::new(),
        }
    }

    pub fn failed(errors: Vec<String>) -> Self {
        Self {
            passed: false,
            errors,
        }
    }
}

/// Errors raised by [`run_q1_validation`].
#[derive(Debug, Error)]
pub enum Q1Error {
    #[error("validation queue error: {0}")]
    Queue(#[from] ValidationQueueError),
    #[error("invalid class code {class_code}: out of u16 range")]
    InvalidClassCode { class_code: i32 },
}

/// Classify a [`ValidationResult`] into a [`Q1Outcome`] — pure, no I/O.
///
/// Factored out of [`run_q1_validation`] so the pass/fail decision is unit
/// testable without a live Postgres: `is_ok` (no errors) → passed, otherwise
/// the error list is surfaced.
fn classify_q1_outcome(result: ValidationResult) -> Q1Outcome {
    if result.is_ok() {
        Q1Outcome::passed()
    } else {
        Q1Outcome::failed(result.errors)
    }
}

/// Run Gate 1 (Q1) validation for one component and record the result on the
/// validation queue.
///
/// Flow (FIND-P9-01):
/// 1. `ComponentValidator::validate_by_class` — a pure function from
///    `brassclaw_engine`, importable cross-crate. Q1 (Phase A.5) is
///    **structural-only**: empty `available_tools` / `existing_skill_names`
///    slices are passed so the structural + token-budget checks run without
///    cross-reference validation (tool-existence / skill-name cross-ref is
///    wired in Phase N's boot-integrity pass, which has the pool to fetch them).
/// 2. On a clean pass → `queue_store.gate1_pass(scope, component_id, &[])`
///    (transitions the queue row `1 → 2`).
/// 3. On failure → `queue_store.gate1_fail(scope, component_id, &errors)`
///    (row stays `1`, errors recorded).
/// 4. Return the [`Q1Outcome`].
///
/// `pool` is accepted per the plan signature for the Phase N boot-integrity
/// wiring (cross-reference fetches); it is not used by the Phase A.5
/// structural-only path.
pub async fn run_q1_validation(
    _pool: &PgPool,
    scope: &ComponentScope,
    component_id: Uuid,
    class_code: i32,
    payload: ComponentPayload<'_>,
    config: &ValidationConfig,
    queue_store: &ValidationQueueStore,
) -> Result<Q1Outcome, Q1Error> {
    let class_u16: u16 = class_code
        .try_into()
        .map_err(|_| Q1Error::InvalidClassCode { class_code })?;

    let result = ComponentValidator::validate_by_class(class_u16, payload, config, &[], &[]);
    let outcome = classify_q1_outcome(result);

    if outcome.passed {
        queue_store.gate1_pass(scope, component_id, &[]).await?;
    } else {
        queue_store
            .gate1_fail(scope, component_id, &outcome.errors)
            .await?;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::memory::component_validator::GenericComponent;

    fn valid_note_payload() -> ComponentPayload<'static> {
        ComponentPayload::Generic(GenericComponent {
            name: "test-note",
            description: "a note",
            content: "body text",
        })
    }

    fn invalid_note_payload() -> ComponentPayload<'static> {
        ComponentPayload::Generic(GenericComponent {
            name: "",
            description: "x",
            content: "y",
        })
    }

    #[test]
    fn classify_passes_on_clean_validation_result() {
        // Class 20 (notes) with a non-empty name/description/content and a
        // soft budget → no errors → passed.
        let result = ComponentValidator::validate_by_class(
            20,
            valid_note_payload(),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(result.is_ok(), "expected clean pass, got {result:?}");
        let outcome = classify_q1_outcome(result);
        assert!(outcome.passed);
        assert!(outcome.errors.is_empty());
    }

    #[test]
    fn classify_fails_with_error_list_on_invalid_payload() {
        // Empty name → "Component name must not be empty" → failed with errors.
        let result = ComponentValidator::validate_by_class(
            20,
            invalid_note_payload(),
            &ValidationConfig::default(),
            &[],
            &[],
        );
        assert!(!result.is_ok(), "expected failure, got {result:?}");
        let outcome = classify_q1_outcome(result);
        assert!(!outcome.passed);
        assert!(!outcome.errors.is_empty());
        assert!(
            outcome
                .errors
                .iter()
                .any(|e| e.contains("name must not be empty")),
            "error list should mention the empty name: {outcome:?}"
        );
    }

    #[test]
    fn q1outcome_constructors_are_consistent() {
        let p = Q1Outcome::passed();
        assert!(p.passed && p.errors.is_empty());
        let f = Q1Outcome::failed(vec!["boom".into()]);
        assert!(!f.passed && f.errors == vec!["boom".to_string()]);
    }
}
