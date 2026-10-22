//! Read-only reconciliation of the original, possibly committed child admission.
use super::*;
use brassclaw_threads::{SubmittedUserMessageRequest, ThreadMessageId};
use brassclaw_turns::TurnId;

pub(super) async fn committed_child(
    runtime: &ChildRuntime,
    operation: &ChildOperation,
    message_id: ThreadMessageId,
) -> Result<(TurnId, TurnRunId), Error> {
    let parent = operation.parent.claimed_run().ok_or(Error::Uncertain)?;
    let actor = parent.state.actor.as_ref().ok_or(Error::Uncertain)?;
    let record = runtime
        .state
        .get_run_record(&operation.scope, operation.run)
        .await
        .map_err(|_| Error::Uncertain)?
        .ok_or(Error::Uncertain)?;
    let parent_record = runtime
        .state
        .get_run_record(&parent.state.scope, parent.state.run_id)
        .await
        .map_err(|_| Error::Uncertain)?
        .ok_or(Error::Uncertain)?;
    let mut expected_profile = parent.resolved_run_profile.clone();
    // These are audit labels, as in capture_child_task; no execution setting
    // or selected profile version may change during the handoff.
    expected_profile.provenance = record.profile.resolved.provenance.clone();
    expected_profile.resolution_fingerprint =
        record.profile.resolved.resolution_fingerprint.clone();
    let expected_message = format!("msg:{message_id}");
    if record.run_id != operation.run
        || record.scope != operation.scope
        || record.accepted_message_ref.as_str() != expected_message
        || record.parent_run_id != Some(parent.state.run_id)
        || record.source_binding_ref.as_str() != operation.reference
        || record.reply_target_binding_ref.as_str() != operation.reference
        || record.received_at != operation.received_at
        || record.profile.resolved != expected_profile
        || parent_record.subagent_depth.checked_add(1) != Some(record.subagent_depth)
        || record.spawn_tree_root_run_id
            != Some(
                parent_record
                    .spawn_tree_root_run_id
                    .unwrap_or(parent.state.run_id),
            )
        || record.claimed_child_reservation_released.is_none()
        || (record.claimed_child_reservation_released == Some(true) && !record.status.is_terminal())
    {
        return Err(Error::Uncertain);
    }
    let state = runtime
        .coordinator
        .get_run_state(GetRunStateRequest {
            scope: operation.scope.clone(),
            run_id: operation.run,
        })
        .await
        .map_err(|_| Error::Uncertain)?;
    if state.actor.as_ref() != Some(actor) || state.turn_id != record.turn_id {
        return Err(Error::Uncertain);
    }
    // Atomic admission already bound this exact message. Never repair an
    // unbound/absent input here, re-submit, claim, resume or replay an effect.
    let message = runtime
        .threads
        .submitted_user_message(SubmittedUserMessageRequest {
            scope: thread_scope(&operation.scope)?,
            thread_id: operation.scope.thread_id.clone(),
            message_id,
            turn_id: record.turn_id.to_string(),
            turn_run_id: record.run_id.to_string(),
        })
        .await
        .map_err(|_| Error::Uncertain)?;
    if message.content.as_deref() != Some(operation.reference.as_str())
        || message.actor_id.as_deref() != Some(actor.user_id.as_str())
        || message.source_binding_id.as_deref() != Some(operation.reference.as_str())
        || message.reply_target_binding_id.as_deref() != Some(operation.reference.as_str())
    {
        return Err(Error::Uncertain);
    }
    Ok((record.turn_id, record.run_id))
}

/// Observe only the original empty thread after an unclear ensure write. No
/// second ensure, new conversation or reconstructed execution authority.
pub(super) async fn ensured_thread(
    runtime: &ChildRuntime,
    operation: &ChildOperation,
) -> Result<(), Error> {
    let actor = operation
        .parent
        .claimed_run()
        .and_then(|parent| parent.state.actor.as_ref())
        .ok_or(Error::Uncertain)?;
    let scope = thread_scope(&operation.scope)?;
    let history = runtime
        .threads
        .list_thread_history(brassclaw_threads::ThreadHistoryRequest {
            scope: scope.clone(),
            thread_id: operation.scope.thread_id.clone(),
        })
        .await
        .map_err(|_| Error::Uncertain)?;
    if history.thread.thread_id != operation.scope.thread_id
        || history.thread.scope != scope
        || history.thread.created_by_actor_id != actor.user_id.as_str()
        || history.thread.metadata_json.is_some()
        || history.thread.goal.is_some()
        || history.thread.title.is_some()
        || !history.messages.is_empty()
        || !history.summary_artifacts.is_empty()
    {
        return Err(Error::Uncertain);
    }
    Ok(())
}

/// Recover the actual original accepted message ID by its scoped event receipt.
/// Read-only evidence cannot repair a missing/partial input or admit a child.
pub(super) async fn accepted_input(
    runtime: &ChildRuntime,
    operation: &ChildOperation,
) -> Result<ThreadMessageId, Error> {
    let actor = operation
        .parent
        .claimed_run()
        .and_then(|parent| parent.state.actor.as_ref())
        .ok_or(Error::Uncertain)?;
    let scope = thread_scope(&operation.scope)?;
    let receipt = runtime
        .threads
        .replay_accepted_inbound_message(brassclaw_threads::ReplayAcceptedInboundMessageRequest {
            scope: scope.clone(),
            actor_id: actor.user_id.to_string(),
            source_binding_id: operation.reference.clone(),
            external_event_id: operation.reference.clone(),
        })
        .await
        .map_err(|_| Error::Uncertain)?
        .ok_or(Error::Uncertain)?;
    if receipt.scope != scope
        || receipt.thread_id != operation.scope.thread_id
        || receipt.status != brassclaw_threads::MessageStatus::Accepted
        || receipt.turn_run_id.is_some()
        || receipt.actor_id.as_deref() != Some(actor.user_id.as_str())
        || receipt.source_binding_id.as_deref() != Some(operation.reference.as_str())
        || receipt.reply_target_binding_id.as_deref() != Some(operation.reference.as_str())
    {
        return Err(Error::Uncertain);
    }
    let history = runtime
        .threads
        .list_thread_history(brassclaw_threads::ThreadHistoryRequest {
            scope,
            thread_id: operation.scope.thread_id.clone(),
        })
        .await
        .map_err(|_| Error::Uncertain)?;
    let mut messages = history
        .messages
        .iter()
        .filter(|message| message.message_id == receipt.message_id);
    let message = messages.next().ok_or(Error::Uncertain)?;
    if messages.next().is_some()
        || message.thread_id != operation.scope.thread_id
        || message.sequence != receipt.sequence
        || message.kind != brassclaw_threads::MessageKind::User
        || message.status != brassclaw_threads::MessageStatus::Accepted
        || message.turn_id.is_some()
        || message.turn_run_id.is_some()
        || message.redaction_ref.is_some()
        || message.tool_result_ref.is_some()
        || message.actor_id.as_deref() != Some(actor.user_id.as_str())
        || message.source_binding_id.as_deref() != Some(operation.reference.as_str())
        || message.reply_target_binding_id.as_deref() != Some(operation.reference.as_str())
        || message.content.as_deref() != Some(operation.reference.as_str())
    {
        return Err(Error::Uncertain);
    }
    Ok(message.message_id)
}
