//! Coordinate existing transcript references with the actual admission commit.
use async_trait::async_trait;
use brassclaw_threads::{PgSessionThreadService, SubmittedUserMessageRequest, ThreadMessageId};
use brassclaw_turns::{
    TurnError, TurnRunRecord, pg_turn_state_store::PgTurnAdmissionCommitParticipant,
};

pub(crate) struct PgTranscriptAdmission {
    threads: PgSessionThreadService,
}

/// Canonical transcript scope shared by admission and the Monty input reader.
/// It is also required by PostgreSQL test-support without `skills-db`.
pub(crate) fn thread_scope_from_turn_scope(
    scope: &brassclaw_turns::TurnScope,
) -> brassclaw_threads::ThreadScope {
    brassclaw_threads::ThreadScope {
        tenant_id: scope.tenant_id.clone(),
        agent_id: scope
            .agent_id
            .clone()
            .unwrap_or_else(|| brassclaw_host_api::AgentId::from_trusted("default".to_string())),
        project_id: scope.project_id.clone(),
        owner_user_id: scope.explicit_owner_user_id().cloned(),
    }
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
                    scope: thread_scope_from_turn_scope(&run.scope),
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
