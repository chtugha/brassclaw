//! Structural validator work uses the existing message-free Work admission,
//! TurnRunnerWorker and global Monty. Only the actual opaque native completion
//! can return a result to Q1; SQL terminal metadata cannot restore that proof.
#![forbid(unsafe_code)]

use crate::bootstrap_reply_validation::invalid;
use brassclaw_pg::PgPool;
use brassclaw_turns::{
    TurnRunId,
    private_work::{PrivateOriginalRef, PrivateWorkStatus},
    run_profile::AgentLoopDriverError,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ValidatorDriverError {
    #[error("validator task admission failed: {0}")]
    Admission(crate::RebornBuildError),
    #[error("validator task drive failed: {0}")]
    Drive(#[from] AgentLoopDriverError),
}

pub(crate) struct ValidatorTaskDriver {
    driver: Arc<crate::global_monty_driver::GlobalMontyDriver>,
}
impl ValidatorTaskDriver {
    pub(crate) fn new(driver: Arc<crate::global_monty_driver::GlobalMontyDriver>) -> Self {
        Self { driver }
    }

    pub(crate) async fn submit(
        &self,
        pool: Arc<PgPool>,
        submission_id: Uuid,
    ) -> Result<crate::global_monty_driver::DrivenMontyTask, ValidatorDriverError> {
        let run = TurnRunId::from_uuid(submission_id);
        // Registration precedes the producer's atomic Ready publication. Late
        // callers join the running producer's same slot; disposal has no effect
        // on the Work, its native host, claim or cancellation.
        let waiter = self.driver.private_completion_waiter(run)?;
        let state = self
            .driver
            .prepare_private_validator(submission_id)
            .await
            .map_err(ValidatorDriverError::Admission)?;
        if state.request.run_id() != run
            || !matches!(state.request.original(),
            PrivateOriginalRef::ValidatorStructural { submission_id: original, .. } if *original == submission_id)
        {
            return Err(ValidatorDriverError::Admission(invalid(
                "validator private original differs",
            )));
        }
        if !waiter.has_result() {
            let live = state.status == PrivateWorkStatus::Ready
                || self.driver.private_completion_live(run)?;
            if !live {
                return Err(ValidatorDriverError::Admission(invalid(
                    "validator original has no live completion; reconciliation required",
                )));
            }
        }
        #[cfg(test)]
        self.driver.private_consumer_waiting(run);
        let driven = waiter.wait().await?;
        let completion = driven.internal_completion.as_ref().ok_or_else(|| {
            ValidatorDriverError::Admission(invalid("validator native completion missing"))
        })?;
        let mut client = pool
            .get()
            .await
            .map_err(|error| ValidatorDriverError::Admission(invalid(error)))?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|error| ValidatorDriverError::Admission(invalid(error)))?;
        let original = crate::pg_monty_admission::PgMontyAdmission::verify_private_live_completion(
            &tx, completion,
        )
        .await?
        .ok_or_else(|| {
            ValidatorDriverError::Admission(invalid("validator completion is not private Work"))
        })?;
        if original.request() != &state.request
            || completion.attempt().run_id != run
            || state.request.original_workflow_checksum()
                != Some(original.definition().workflow_checksum())
        {
            return Err(ValidatorDriverError::Admission(invalid(
                "validator native completion belongs to another original",
            )));
        }
        tx.commit()
            .await
            .map_err(|error| ValidatorDriverError::Admission(invalid(error)))?;
        Ok(driven)
    }
}
