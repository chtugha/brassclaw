//! Trusted internal driver for the retained Q1 validator task path.
//!
//! [`ValidatorTaskDriver`] submits a validator review task through the global
//! Monty service via `GlobalMontyDriver::drive_admitted`. This ensures:
//!
//! 1. `GlobalTaskPortsFactory::build` is called with a real `MontyTaskHost`.
//! 2. `build` calls `capture_internal`, which routes to `ReviewAdmissionSource`
//!    for the `VALIDATOR_REVIEW_PREFIX + submission_id` user-input.
//! 3. `build` creates `GlobalRecipePorts::for_internal` with that source.
//! 4. On `InternalCompleted`, `GlobalRecipePorts::finish` calls
//!    `source.verify_receipt`, which persists the Q1 evidence.
//!
//! The admission is created by `bootstrap_validation::prepare_admission` via
//! `AdmissionMode::ValidatorInternal`, which sets `trusted_internal_turn = true`
//! on the `LoopRunContext` before building the `MontyTaskHandoff`.
//!
//! **Lifecycle contract:** after `submit` returns, the factory's
//! `settle` / `settle_unsubmitted` path owns the full admission lifecycle.
//! `bootstrap_validation::settle_closed` must NOT be called after `submit`.

#![forbid(unsafe_code)]

use std::sync::Arc;

use brassclaw_pg::PgPool;
use brassclaw_turns::run_profile::AgentLoopDriverError;
use uuid::Uuid;

use crate::bootstrap_validation;

/// Errors from the validator task driver.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ValidatorDriverError {
    #[error("validator task admission failed: {0}")]
    Admission(crate::RebornBuildError),
    #[error("validator task drive failed: {0}")]
    Drive(#[from] AgentLoopDriverError),
}

/// Submits a validator review task through `GlobalMontyDriver::drive_admitted`.
///
/// Constructed once and stored in [`ValidatorServiceBridge`] after the global
/// Monty driver is started.
pub(crate) struct ValidatorTaskDriver {
    driver: Arc<crate::global_monty_driver::GlobalMontyDriver>,
}

impl ValidatorTaskDriver {
    pub(crate) fn new(driver: Arc<crate::global_monty_driver::GlobalMontyDriver>) -> Self {
        Self { driver }
    }

    /// Admit a trusted internal validator task and drive it through the global
    /// Monty service.
    ///
    /// Preconditions (caller's responsibility):
    /// - `retain_validator_selection` has succeeded for `submission_id`.
    /// - The live `InstalledMontyCatalogue` will route
    ///   `VALIDATOR_REVIEW_PREFIX + submission_id` to `ReviewAdmissionSource`.
    ///
    /// Returns the actual exit and, for a completed internal task, the opaque
    /// post-settlement proof issued by the shared driver. A ResultOnly exit
    /// alone is insufficient to graduate the original review.
    pub(crate) async fn submit(
        &self,
        pool: Arc<PgPool>,
        submission_id: Uuid,
    ) -> Result<crate::global_monty_driver::DrivenMontyTask, ValidatorDriverError> {
        // Admit through the trusted internal path.
        // AdmissionMode::ValidatorInternal sets trusted_internal_turn = true,
        // uses TRUSTED_INTERNAL_SOURCE_BINDING, and calls new_claimed on the handoff.
        // The reference becomes the user-input text that capture_internal matches
        // against VALIDATOR_REVIEW_PREFIX.
        let reference =
            crate::pg_review_admission_source::validator_review_reference(submission_id);
        let admitted =
            bootstrap_validation::admit_validator_handoff(pool, submission_id, &reference)
                .await
                .map_err(ValidatorDriverError::Admission)?;

        // Drive through the global service.
        // drive_admitted calls factory.build(host, input) which invokes capture_internal
        // because trusted_internal_turn = true. capture_internal returns
        // ReviewAdmissionSource for the VALIDATOR_REVIEW_PREFIX prefix.
        // On InternalCompleted, GlobalRecipePorts::finish calls verify_receipt.
        self.driver
            .drive_admitted(admitted.host)
            .await
            .map_err(ValidatorDriverError::Drive)
    }
}
