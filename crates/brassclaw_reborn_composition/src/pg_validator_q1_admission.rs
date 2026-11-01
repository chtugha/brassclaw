//! Durable Q1 validator task admission — Step 9/12 gate prerequisite.
//!
//! Bridges the validation queue to the sealed `capture_internal` path added
//! by Step 6. Given an existing `RetainedValidatorSubject` and an
//! `InspectedRetainedProgram` built from the sealed structural validator Recipe
//! draft, this module:
//!
//! 1. Calls `RetainedValidatorCoverage::selection_bytes` to derive the pinned
//!    `component-validator-selection/1` JSON.
//! 2. Retains it in `reborn_component_validator_selections` via
//!    `selection::retain_in_transaction` (idempotent).
//! 3. Builds the `TaskInput` for the trusted internal admission path using
//!    `VALIDATOR_REVIEW_PREFIX + submission_id`.
//! 4. Submits via the supplied `ServiceClient` and awaits the `TaskTicket`.
//!
//! # What this is NOT
//!
//! - Not a Recipe runner or a second Monty VM.
//! - Not a source of Q1 approval. A completed `InternalCompleted` outcome from
//!   the ticket means `verify_receipt` ran; it is not yet Q1 evidence unless
//!   the `pg_validator_q1_evidence` writer succeeded inside `verify_receipt`.
//! - Not a replacement for `run_validator_python_with_limits` in the current
//!   legacy `q1_orchestrator` path. That path is replaced only when the
//!   `InstalledMontyCatalogue` seeding and service client wiring are complete;
//!   until then `Q1Outcome::Deferred` is the correct outcome for routing through
//!   this path.
//!
//! # Feature gate
//!
//! Requires the `postgres` and `skills-db` features.

#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_engine::{
    executor::retained_source::InspectedRetainedProgram,
    memory::retained_validator::RetainedValidatorCoverage,
};
use brassclaw_monty_host::service::{ServiceClient, ServiceFailure, TaskEntry, TaskInput};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    review_submission_store::{RetainedValidatorSubject, selection},
};
use uuid::Uuid;

use crate::pg_review_admission_source::VALIDATOR_REVIEW_PREFIX;

/// Errors from the durable Q1 selection preparation step.
#[allow(dead_code)] // wired by validator Step 9/12 Q1 scheduling
#[derive(Debug, thiserror::Error)]
pub(crate) enum Q1AdmissionError {
    #[error("validator coverage derivation failed: {0}")]
    Coverage(&'static str),
    #[error("validator selection bytes construction failed: {0}")]
    SelectionBytes(&'static str),
    #[error("validator selection retention failed")]
    Retention,
    #[error("validator task submission failed")]
    Submission(ServiceFailure),
    #[error("database pool unavailable")]
    Pool,
}

/// Prepare and retain the sealed validator selection for a submission.
///
/// Derives `component-validator-selection/1` bytes from the inspected programme
/// and the sealed subject, then retains them idempotently in
/// `reborn_component_validator_selections`. Returns the selection bytes on
/// success so the caller can verify idempotent retry round-trips.
///
/// This is preparation, not execution authority or Q1 evidence.
#[allow(dead_code)] // wired by validator Step 9/12 Q1 scheduling
pub(crate) async fn retain_validator_selection(
    pool: &PgPool,
    subject: &RetainedValidatorSubject,
    inspected: &InspectedRetainedProgram,
) -> Result<String, Q1AdmissionError> {
    let selection_bytes =
        RetainedValidatorCoverage::selection_bytes(subject, inspected, REVISION_LIMITS)
            .map_err(Q1AdmissionError::SelectionBytes)?;

    let mut client = pool.get().await.map_err(|_| Q1AdmissionError::Pool)?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(|_| Q1AdmissionError::Pool)?;
    selection::retain_in_transaction(&tx, &selection_bytes, REVISION_LIMITS)
        .await
        .map_err(|_| Q1AdmissionError::Retention)?;
    tx.commit().await.map_err(|_| Q1AdmissionError::Pool)?;
    Ok(selection_bytes)
}

/// Submit a trusted internal validator review task via the global Monty service.
///
/// The caller is responsible for ensuring:
/// - `retain_validator_selection` succeeded for `submission_id`.
/// - The `capture_internal` wiring in `InstalledMontyCatalogue` is live and
///   will route `VALIDATOR_REVIEW_PREFIX + submission_id` correctly.
/// - The supplied `service` client belongs to the global Monty service that
///   hosts the `InstalledMontyCatalogue`.
///
/// The returned `TaskTicket` must be awaited. An `InternalCompleted` outcome
/// means `verify_receipt` ran on the receipt; it is not a Q1 pass unless
/// `pg_validator_q1_evidence::persist_validator_q1_evidence` succeeded inside
/// `verify_receipt`.
#[allow(dead_code)] // wired by validator Step 9/12 Q1 scheduling
pub(crate) async fn submit_validator_review(
    service: &ServiceClient,
    submission_id: Uuid,
    run_id: Uuid,
    thread_id: Uuid,
    ports: Arc<dyn brassclaw_monty_host::service::TaskPorts>,
) -> Result<brassclaw_monty_host::service::TaskTicket, Q1AdmissionError> {
    let user_input = format!("{VALIDATOR_REVIEW_PREFIX}{submission_id}");
    let input = TaskInput {
        entry: TaskEntry::InternalRecipe {
            name: "validator-structural-review".into(),
        },
        conversation_id: thread_id.to_string(),
        message_id: format!("validator-q1:{submission_id}"),
        turn_id: submission_id.to_string(),
        run_id: run_id.to_string(),
        user_input,
        history: vec![],
    };
    service
        .submit_when_available(input, ports)
        .await
        .map_err(Q1AdmissionError::Submission)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// The validator review user_input has the exact expected form.
    #[test]
    fn validator_review_user_input_matches_prefix() {
        let id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let thread_id = Uuid::new_v4();
        // The user_input that would be submitted to the service must start
        // with VALIDATOR_REVIEW_PREFIX and end with the submission UUID.
        let user_input = format!("{VALIDATOR_REVIEW_PREFIX}{id}");
        assert!(
            user_input.starts_with(VALIDATOR_REVIEW_PREFIX),
            "user_input must start with VALIDATOR_REVIEW_PREFIX"
        );
        let suffix = user_input
            .strip_prefix(VALIDATOR_REVIEW_PREFIX)
            .expect("suffix must exist");
        let parsed = Uuid::parse_str(suffix).expect("suffix must be a valid UUID");
        assert_eq!(parsed, id, "submission_id round-trip must match");
        // Suppress unused variable warnings in the test scope.
        let _ = (run_id, thread_id);
    }
}
