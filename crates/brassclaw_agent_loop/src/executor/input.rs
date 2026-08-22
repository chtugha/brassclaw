use async_trait::async_trait;
use brassclaw_turns::{
    LoopCancelledReasonKind, LoopExit, LoopMessageRef,
    run_profile::{LoopInput, LoopInputAckToken, LoopInputBatch},
};
use tracing::debug;

use crate::state::{CheckpointKind, LoopExecutionState};

use super::{
    AgentLoopExecutorError, CancelCheck, CheckpointStage, DrainedInputs, ExecutorStage, HostStage,
    MAX_INPUT_DRAIN, PendingInputAck, StageContext, cancelled_exit_with_reason,
};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct InputStage;

#[derive(Debug, Clone, Copy)]
pub(super) enum UserFacingInputDrainMode {
    Steering,
    FollowUp,
}

pub(super) struct DrainInput {
    pub(super) state: LoopExecutionState,
    pub(super) pending_input_ack: PendingInputAck,
    pub(super) mode: UserFacingInputDrainMode,
}

pub(super) enum InputStep {
    Continue {
        state: Box<LoopExecutionState>,
        pending_input_ack: PendingInputAck,
        drained: bool,
    },
    Exit(LoopExit),
}

#[async_trait]
impl ExecutorStage<DrainInput> for InputStage {
    type Output = InputStep;

    async fn process(
        &self,
        ctx: StageContext<'_>,
        input: DrainInput,
    ) -> Result<InputStep, AgentLoopExecutorError> {
        let mut state = input.state;
        let mut pending_input_ack = input.pending_input_ack;

        let should_drain = match input.mode {
            UserFacingInputDrainMode::Steering => {
                pending_input_ack.is_empty() && ctx.planner.drain().drain_steering(&state).await
            }
            UserFacingInputDrainMode::FollowUp => ctx.planner.drain().drain_followup(&state).await,
        };

        if should_drain {
            state = match CheckpointStage.cancel_if_requested(ctx, state).await? {
                CancelCheck::Continue(state) => *state,
                CancelCheck::Exit(exit) => return Ok(InputStep::Exit(exit)),
            };
            let drained = self.drain(ctx, state, input.mode).await?;
            state = drained.state;
            pending_input_ack.replace(drained.ack_tokens)?;
            if let Some(reason_kind) = drained.cancelled_reason_kind {
                let checked = CheckpointStage
                    .write(ctx, state, CheckpointKind::Final)
                    .await?;
                pending_input_ack.ack(ctx.host).await?;
                return Ok(InputStep::Exit(cancelled_exit_with_reason(
                    ctx.host,
                    checked.state,
                    reason_kind,
                    Some(checked.checkpoint_id),
                )?));
            }
            state = match CheckpointStage
                .cancel_if_requested_after_pending_input_ack(ctx, state, &mut pending_input_ack)
                .await?
            {
                CancelCheck::Continue(state) => *state,
                CancelCheck::Exit(exit) => return Ok(InputStep::Exit(exit)),
            };
            return Ok(InputStep::Continue {
                state: Box::new(state),
                pending_input_ack,
                drained: drained.drained,
            });
        }

        if matches!(input.mode, UserFacingInputDrainMode::Steering) {
            state = match CheckpointStage
                .cancel_if_requested_after_pending_input_ack(ctx, state, &mut pending_input_ack)
                .await?
            {
                CancelCheck::Continue(state) => *state,
                CancelCheck::Exit(exit) => return Ok(InputStep::Exit(exit)),
            };
        }

        Ok(InputStep::Continue {
            state: Box::new(state),
            pending_input_ack,
            drained: false,
        })
    }
}

impl InputStage {
    #[cfg(test)]
    pub(super) async fn drain_user_inputs(
        &self,
        ctx: StageContext<'_>,
        state: LoopExecutionState,
    ) -> Result<DrainedInputs, AgentLoopExecutorError> {
        self.drain(ctx, state, UserFacingInputDrainMode::Steering)
            .await
    }

    #[cfg(test)]
    pub(super) async fn drain_followup(
        &self,
        ctx: StageContext<'_>,
        state: LoopExecutionState,
    ) -> Result<DrainedInputs, AgentLoopExecutorError> {
        self.drain(ctx, state, UserFacingInputDrainMode::FollowUp)
            .await
    }

    async fn drain(
        &self,
        ctx: StageContext<'_>,
        mut state: LoopExecutionState,
        mode: UserFacingInputDrainMode,
    ) -> Result<DrainedInputs, AgentLoopExecutorError> {
        let batch = ctx
            .host
            .poll_inputs(state.input_cursor.clone(), MAX_INPUT_DRAIN)
            .await
            .map_err(|_| AgentLoopExecutorError::HostUnavailable {
                stage: HostStage::Input,
            })?;
        let (drained, ack_tokens, cancelled_reason_kind, last_message_ref) =
            consume_drainable_inputs(&batch, mode, &mut state)?;
        // v3 plan §H3: resolve the raw user-facing message text so RecipeStage
        // can run intent-driven retrieval without the assembled prompt. On any
        // error (host does not wire a resolver, or no text recorded for the
        // ref) leave `last_user_text = None` and fall through to Tier-2.
        if let Some(message_ref) = last_message_ref {
            match ctx
                .host
                .resolve_message_text(ctx.host.run_context(), &message_ref)
                .await
            {
                Ok(text) => state.last_user_text = Some(text),
                Err(error) => {
                    debug!(
                        kind = ?error.kind,
                        "resolve_message_text unavailable; leaving last_user_text None (Tier-2 fall-through)"
                    );
                    state.last_user_text = None;
                }
            }
        }
        Ok(DrainedInputs {
            state,
            drained,
            ack_tokens,
            cancelled_reason_kind,
        })
    }
}

/// Output of [`consume_drainable_inputs`]: whether any user-facing input was
/// drained, the ack tokens to confirm, an early cancellation reason (if a
/// cancel/interrupt was hit), and the last consumed user-facing `message_ref`
/// (so `drain` can resolve its raw text for intent-driven retrieval, v3 §H3).
type ConsumedDrainableInputs = (
    bool,
    Vec<LoopInputAckToken>,
    Option<LoopCancelledReasonKind>,
    Option<LoopMessageRef>,
);

pub(super) fn consume_drainable_inputs(
    batch: &LoopInputBatch,
    mode: UserFacingInputDrainMode,
    state: &mut LoopExecutionState,
) -> Result<ConsumedDrainableInputs, AgentLoopExecutorError> {
    let mut consumed_len = 0;
    let mut drained = false;
    let mut cancelled_reason_kind = None;
    let mut last_message_ref: Option<LoopMessageRef> = None;
    for input in &batch.inputs {
        if user_facing_input_matches_drain_mode(input, mode) {
            consumed_len += 1;
            drained = true;
            // Capture the last consumed user-facing message ref so `drain` can
            // resolve its raw text for intent-driven retrieval (v3 plan §H3).
            last_message_ref = user_facing_message_ref(input).cloned();
            continue;
        }
        match input {
            LoopInput::Cancel { .. } => {
                consumed_len += 1;
                cancelled_reason_kind = Some(LoopCancelledReasonKind::HostCancellation);
                break;
            }
            LoopInput::Interrupt { .. } => {
                consumed_len += 1;
                cancelled_reason_kind = Some(LoopCancelledReasonKind::HostInterrupt);
                break;
            }
            LoopInput::GateResolved { .. } | LoopInput::CapabilitySurfaceChanged { .. } => break,
            LoopInput::UserMessage { .. }
            | LoopInput::FollowUp { .. }
            | LoopInput::Steering { .. } => {
                break;
            }
        }
    }
    if consumed_len == 0 {
        return Ok((false, Vec::new(), None, None));
    }
    if batch.input_acks.len() < consumed_len {
        return Err(AgentLoopExecutorError::PlannerContract {
            detail: "input batch omitted ack metadata for consumed inputs",
        });
    }
    let last_ack = &batch.input_acks[consumed_len - 1];
    state.input_cursor = last_ack.cursor.clone();
    let ack_tokens = batch
        .input_acks
        .iter()
        .take(consumed_len)
        .map(|ack| ack.token.clone())
        .collect();
    Ok((drained, ack_tokens, cancelled_reason_kind, last_message_ref))
}

fn user_facing_input_matches_drain_mode(input: &LoopInput, mode: UserFacingInputDrainMode) -> bool {
    match mode {
        UserFacingInputDrainMode::Steering => {
            matches!(
                input,
                LoopInput::UserMessage { .. } | LoopInput::Steering { .. }
            )
        }
        UserFacingInputDrainMode::FollowUp => {
            matches!(
                input,
                LoopInput::FollowUp { .. } | LoopInput::UserMessage { .. }
            )
        }
    }
}

/// Extract the `message_ref` from a user-facing `LoopInput` variant
/// (`UserMessage` / `FollowUp` / `Steering`); `None` for all other variants.
fn user_facing_message_ref(input: &LoopInput) -> Option<&LoopMessageRef> {
    match input {
        LoopInput::UserMessage { message_ref }
        | LoopInput::FollowUp { message_ref }
        | LoopInput::Steering { message_ref } => Some(message_ref),
        _ => None,
    }
}
