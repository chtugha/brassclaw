//! Durable task input for Monty, independent of the legacy UUID engine Thread.

use brassclaw_threads::{
    SessionThreadError, SessionThreadService, SubmittedTurnInput, SubmittedUserMessageRequest,
    ThreadMessageId, ThreadScope,
};
use brassclaw_turns::{AcceptedMessageRef, AgentLoopDriverError, TurnId, TurnRunId, TurnScope};

pub(crate) fn thread_scope_from_turn_scope(scope: &TurnScope) -> ThreadScope {
    ThreadScope {
        tenant_id: scope.tenant_id.clone(),
        agent_id: scope
            .agent_id
            .clone()
            .unwrap_or_else(|| brassclaw_host_api::AgentId::from_trusted("default".to_string())),
        project_id: scope.project_id.clone(),
        owner_user_id: scope.explicit_owner_user_id().cloned(),
    }
}

/// Read only the input admitted for this run, with all eligible predecessors.
/// Conversation IDs remain opaque. The transcript service enforces scope,
/// submission identity, redaction and the cutoff before the exact input.
pub(crate) async fn load_monty_task_input(
    service: &dyn SessionThreadService,
    scope: &TurnScope,
    accepted_message_ref: Option<&AcceptedMessageRef>,
    turn_id: TurnId,
    run_id: TurnRunId,
) -> Result<SubmittedTurnInput, AgentLoopDriverError> {
    let message_id = accepted_message_ref
        .and_then(|reference| reference.as_str().strip_prefix("msg:"))
        .and_then(|id| ThreadMessageId::parse(id).ok())
        .ok_or_else(|| AgentLoopDriverError::InvalidRequest {
            reason: "Monty task has no valid accepted message reference".to_owned(),
        })?;
    let input = service
        .submitted_turn_input(
            SubmittedUserMessageRequest {
                scope: thread_scope_from_turn_scope(scope),
                thread_id: scope.thread_id.clone(),
                message_id,
                turn_id: turn_id.to_string(),
                turn_run_id: run_id.to_string(),
            },
            None,
        )
        .await
        .map_err(|error| match error {
            SessionThreadError::SubmittedInputPending { .. } => {
                AgentLoopDriverError::InputAdmissionPending
            }
            _ => AgentLoopDriverError::Failed {
                reason_kind: "Monty admitted input lookup failed".to_owned(),
            },
        })?;
    if input.message.content.is_none() {
        return Err(AgentLoopDriverError::Failed {
            reason_kind: "Monty admitted input has no content".to_owned(),
        });
    }
    Ok(input)
}
