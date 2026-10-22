//! Transcript-side acknowledgement of coordinator-supplied references. This
//! does not admit, claim, resume or otherwise decide a turn's lifecycle.
use crate::{MessageKind, MessageStatus, SessionThreadError, ThreadMessageRecord};

pub(crate) fn ensure_user_accepted(
    message: &ThreadMessageRecord,
    attempted: &'static str,
) -> Result<(), SessionThreadError> {
    if message.kind == MessageKind::User
        && matches!(
            message.status,
            MessageStatus::Accepted | MessageStatus::DeferredBusy
        )
        && message.content.is_some()
        && message.turn_id.is_none()
        && message.turn_run_id.is_none()
    {
        return Ok(());
    }
    Err(SessionThreadError::InvalidMessageTransition {
        message_id: message.message_id,
        from: message.status,
        attempted,
    })
}

/// Return whether the first binding changed the record. An exact repeated
/// acknowledgement is a read-only success; another binding cannot replace it.
pub(crate) fn bind_submission(
    message: &mut ThreadMessageRecord,
    turn_id: &str,
    turn_run_id: &str,
) -> Result<bool, SessionThreadError> {
    if turn_id.is_empty() || turn_run_id.is_empty() {
        return Err(SessionThreadError::SubmittedInputMismatch {
            message_id: message.message_id,
        });
    }
    if message.kind == MessageKind::User
        && message.status == MessageStatus::Submitted
        && message.content.is_some()
    {
        return if message.turn_id.as_deref() == Some(turn_id)
            && message.turn_run_id.as_deref() == Some(turn_run_id)
        {
            Ok(false)
        } else {
            Err(SessionThreadError::SubmittedInputMismatch {
                message_id: message.message_id,
            })
        };
    }
    ensure_user_accepted(message, "mark_message_submitted")?;
    message.status = MessageStatus::Submitted;
    message.turn_id = Some(turn_id.to_owned());
    message.turn_run_id = Some(turn_run_id.to_owned());
    Ok(true)
}
