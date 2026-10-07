//! Context eligible for one exact admitted execution, including its own results.
//! Later inputs and other runs' results never enter an in-flight task's prompt.

use brassclaw_threads::{
    ContextMessages, LoadContextMessagesRequest, MessageKind, MessageStatus, SessionThreadService,
    SubmittedUserMessageRequest, ThreadMessageId, ThreadMessageRangeRequest, ThreadScope,
};
use brassclaw_turns::run_profile::{AgentLoopHostError, AgentLoopHostErrorKind, LoopRunContext};

pub(crate) async fn load_admitted_context<S: SessionThreadService + ?Sized>(
    service: &S,
    scope: &ThreadScope,
    run: &LoopRunContext,
    limit: Option<usize>,
) -> Result<ContextMessages, AgentLoopHostError> {
    let message_id = run
        .accepted_message_ref
        .as_ref()
        .and_then(|reference| reference.as_str().strip_prefix("msg:"))
        .and_then(|id| ThreadMessageId::parse(id).ok())
        .ok_or_else(|| {
            AgentLoopHostError::new(
                AgentLoopHostErrorKind::InvalidInvocation,
                "admitted input reference is invalid",
            )
        })?;
    let admitted = service
        .submitted_turn_input(
            SubmittedUserMessageRequest {
                scope: scope.clone(),
                thread_id: run.thread_id.clone(),
                message_id,
                turn_id: run.turn_id.to_string(),
                turn_run_id: run.run_id.to_string(),
            },
            None,
        )
        .await
        .map_err(crate::context_read_error)?;

    let run_id = run.run_id.to_string();
    let results = service
        .list_thread_messages_range(ThreadMessageRangeRequest {
            scope: scope.clone(),
            thread_id: run.thread_id.clone(),
            after_sequence: admitted.message.sequence,
            through_sequence: u64::MAX,
        })
        .await
        .map_err(crate::context_read_error)?;
    let mut ids = vec![message_id];
    ids.extend(
        results
            .messages
            .into_iter()
            .filter(|record| {
                record.sequence > admitted.message.sequence
                    && record.turn_run_id.as_deref() == Some(run_id.as_str())
                    && record.kind == MessageKind::ToolResultReference
                    && record.status == MessageStatus::Finalized
            })
            .map(|record| record.message_id),
    );
    let current = service
        .load_context_messages(LoadContextMessagesRequest {
            scope: scope.clone(),
            thread_id: run.thread_id.clone(),
            message_ids: ids,
        })
        .await
        .map_err(crate::context_read_error)?;
    // Visibility/redaction remains authoritative; losing the actual input must
    // fail instead of quietly issuing a model call with a different question.
    if !current
        .messages
        .iter()
        .any(|record| record.message_id == Some(message_id))
    {
        return Err(AgentLoopHostError::new(
            AgentLoopHostErrorKind::InvalidInvocation,
            "admitted input is no longer visible",
        ));
    }
    let mut context = admitted.prior_context;
    context.messages.extend(current.messages);
    context.messages.sort_by_key(|record| record.sequence);
    if let Some(limit) = limit {
        let discard = context.messages.len().saturating_sub(limit);
        context.messages.drain(..discard);
    }
    Ok(context)
}
