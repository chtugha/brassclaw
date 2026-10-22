//! Compare the immutable input of an existing event, independent of its backend.
use crate::{
    AcceptInboundMessageRequest, AcceptedInboundMessage, MessageKind, MessageStatus,
    SessionThreadError, ThreadMessageRecord,
};

pub(crate) fn acknowledged_input(
    message: &ThreadMessageRecord,
    request: &AcceptInboundMessageRequest,
) -> Result<AcceptedInboundMessage, SessionThreadError> {
    if message.thread_id != request.thread_id {
        return Err(SessionThreadError::IdempotentReplayThreadMismatch {
            stored_thread_id: message.thread_id.clone(),
            requested_thread_id: request.thread_id.clone(),
        });
    }
    if message.actor_id.as_deref() != Some(request.actor_id.as_str()) {
        return Err(SessionThreadError::IdempotentReplayActorMismatch {
            stored_actor_id: message.actor_id.clone().unwrap_or_default(),
            requested_actor_id: request.actor_id.clone(),
        });
    }
    let coherent_binding = match message.status {
        MessageStatus::Accepted | MessageStatus::DeferredBusy => {
            message.turn_id.is_none() && message.turn_run_id.is_none()
        }
        MessageStatus::Submitted => {
            message.turn_id.as_ref().is_some_and(|id| !id.is_empty())
                && message
                    .turn_run_id
                    .as_ref()
                    .is_some_and(|id| !id.is_empty())
        }
        _ => false,
    };
    if message.kind != MessageKind::User
        || !coherent_binding
        || message.redaction_ref.is_some()
        || message.content.as_deref() != Some(request.content.as_text())
        || message.source_binding_id != request.source_binding_id
        || message.reply_target_binding_id != request.reply_target_binding_id
    {
        return Err(SessionThreadError::Backend(
            "inbound event conflicts with its original message".into(),
        ));
    }
    Ok(AcceptedInboundMessage {
        thread_id: message.thread_id.clone(),
        message_id: message.message_id,
        sequence: message.sequence,
        idempotent_replay: true,
    })
}
