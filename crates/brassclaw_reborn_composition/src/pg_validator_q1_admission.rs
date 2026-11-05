//! Durable Q1 validator task admission — Step 9/12 gate prerequisite.
//!
//! Provides `retain_validator_selection`, which prepares and durably stores the
//! sealed `component-validator-selection/1` record required before a trusted
//! validator task can be published as message-free private Work.
//!
//! Steps performed by `retain_validator_selection`:
//! 1. Calls `RetainedValidatorCoverage::selection_bytes` to derive the pinned
//!    `component-validator-selection/1` JSON.
//! 2. Retains it in `reborn_component_validator_selections` via
//!    `selection::retain_in_transaction` (idempotent).
//!
//! Actual task submission is handled by `ValidatorTaskDriver::submit`, which
//! publishes private Ready Work and awaits the existing Worker/native driver.
//!
//! # What this is NOT
//!
//! - Not a Recipe runner or a second Monty VM.
//! - Not a source of Q1 approval. A completed task outcome means `verify_receipt`
//!   ran; it is not yet Q1 evidence unless
//!   `pg_validator_q1_evidence::persist_validator_q1_evidence` succeeded.
//!
//! # Feature gate
//!
//! Requires the `postgres` and `skills-db` features.

#![forbid(unsafe_code)]

use brassclaw_engine::{
    executor::retained_source::InspectedRetainedProgram,
    memory::retained_validator::RetainedValidatorCoverage,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    review_submission_store::{RetainedValidatorSubject, selection},
};

/// Errors from the durable Q1 selection preparation step.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Q1AdmissionError {
    #[error("validator selection bytes construction failed: {0}")]
    SelectionBytes(&'static str),
    #[error("validator selection retention failed")]
    Retention,
    #[error("database pool unavailable")]
    Pool,
}

/// Prepare and retain the sealed validator selection for a submission.
///
/// Derives `component-validator-selection/1` bytes from the inspected programme
/// and the sealed subject, then retains them idempotently in
/// `reborn_component_validator_selections`. Returns the exact verified retained
/// selection read in that transaction for later post-settlement authentication.
///
/// This is preparation, not execution authority or Q1 evidence.
pub(crate) async fn retain_validator_selection(
    pool: &PgPool,
    subject: &RetainedValidatorSubject,
    inspected: &InspectedRetainedProgram,
) -> Result<selection::RetainedValidatorSelection, Q1AdmissionError> {
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
    let retained =
        selection::read_preparation_in_transaction(&tx, subject.submission_id(), REVISION_LIMITS)
            .await
            .map_err(|_| Q1AdmissionError::Retention)?
            .ok_or(Q1AdmissionError::Retention)?;
    tx.commit().await.map_err(|_| Q1AdmissionError::Pool)?;
    Ok(retained)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All Q1AdmissionError variants produce a non-empty Display string.
    #[test]
    fn q1_admission_error_variants_are_displayable() {
        let errors: &[Q1AdmissionError] = &[
            Q1AdmissionError::SelectionBytes("bad bytes"),
            Q1AdmissionError::Retention,
            Q1AdmissionError::Pool,
        ];
        for e in errors {
            let s = e.to_string();
            assert!(!s.is_empty(), "error Display must not be empty for {e:?}");
        }
    }
}
