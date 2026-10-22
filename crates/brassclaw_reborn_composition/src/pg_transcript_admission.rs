//! Coordinate existing transcript references with the actual admission commit.
use async_trait::async_trait;
use brassclaw_threads::{PgSessionThreadService, SubmittedUserMessageRequest, ThreadMessageId};
use brassclaw_turns::{
    TurnError, TurnRunRecord, pg_turn_state_store::PgTurnAdmissionCommitParticipant,
};

pub(crate) struct PgTranscriptAdmission {
    threads: PgSessionThreadService,
}

impl PgTranscriptAdmission {
    pub(crate) fn new(pool: std::sync::Arc<brassclaw_pg::PgPool>, tenant: &str) -> Self {
        Self {
            threads: PgSessionThreadService::new(pool, tenant),
        }
    }
}

#[async_trait]
impl PgTurnAdmissionCommitParticipant for PgTranscriptAdmission {
    async fn bind_admitted_run(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        run: &TurnRunRecord,
    ) -> Result<(), TurnError> {
        let unavailable = || TurnError::Unavailable {
            reason: "admitted transcript binding could not be committed".into(),
        };
        // Use the same canonical reference grammar and scope conversion as
        // the normal Monty input reader. No raw content enters turn state.
        let message_id = run
            .accepted_message_ref
            .as_str()
            .strip_prefix("msg:")
            .and_then(|id| ThreadMessageId::parse(id).ok())
            .ok_or_else(unavailable)?;
        self.threads
            .bind_submission_in_transaction(
                transaction,
                SubmittedUserMessageRequest {
                    scope: crate::monty_task_input::thread_scope_from_turn_scope(&run.scope),
                    thread_id: run.scope.thread_id.clone(),
                    message_id,
                    turn_id: run.turn_id.to_string(),
                    turn_run_id: run.run_id.to_string(),
                },
            )
            .await
            .map_err(|_| unavailable())?;
        Ok(())
    }
}
