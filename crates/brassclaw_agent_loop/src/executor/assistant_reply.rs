use async_trait::async_trait;
use brassclaw_turns::run_profile::{AssistantReply, FinalizeAssistantMessage, LoopModelUsage};

use crate::plan_state::{AgentPlanState, classify};
use crate::{state::LoopExecutionState, strategies::TurnSummary};

use super::{
    AgentLoopExecutorError, CancelCheck, CheckpointStage, ExecutorStage, HostStage, StageContext,
    TurnCompletedStep,
};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AssistantReplyStage;

pub(super) struct AssistantReplyInput {
    pub(super) state: LoopExecutionState,
    pub(super) reply: AssistantReply,
    pub(super) usage: Option<LoopModelUsage>,
}

#[async_trait]
impl ExecutorStage<AssistantReplyInput> for AssistantReplyStage {
    type Output = TurnCompletedStep;

    async fn process(
        &self,
        ctx: StageContext<'_>,
        input: AssistantReplyInput,
    ) -> Result<TurnCompletedStep, AgentLoopExecutorError> {
        let mut state = input.state;
        // Capture content before moving the reply into FinalizeAssistantMessage.
        let reply_content = input.reply.content.clone();
        let output_tokens = input
            .usage
            .map(|usage| usage.output_tokens)
            .unwrap_or_else(|| estimate_output_tokens(&input.reply));
        let reply_ref = ctx
            .host
            .finalize_assistant_message(FinalizeAssistantMessage { reply: input.reply })
            .await
            .map_err(|_| AgentLoopExecutorError::HostUnavailable {
                stage: HostStage::Transcript,
            })?;
        state.assistant_refs.push(reply_ref.clone());
        state.recent_output_token_counts.push(output_tokens);

        // ── Planning hook ────────────────────────────────────────────────
        // Called on every assistant reply. On iteration 0 (no plan yet),
        // tries to extract a structured plan from the reply content.
        // If `pending_prose_conversion` is set, this is a fallback reply
        // after the reformat nudge — try again, then always clear the slot.
        apply_plan_hook(&mut state, &reply_content);

        state = match CheckpointStage.cancel_if_requested(ctx, state).await? {
            CancelCheck::Continue(state) => *state,
            CancelCheck::Exit(exit) => return Ok(TurnCompletedStep::Exit(exit)),
        };

        Ok(TurnCompletedStep::Continue {
            state: Box::new(state),
            summary: TurnSummary::reply_only(reply_ref),
        })
    }
}

/// Post-reply planning hook: deterministic, no I/O.
///
/// - If `plan_state` is already set: advance the step counter if the reply
///   looks like a completion signal (non-empty, not a tool call). The step
///   is advanced only when `pending_prose_conversion` is clear to avoid
///   double-advancing on the fallback path.
/// - If `plan_state` is None and `pending_prose_conversion` is None:
///   attempt to extract a plan from `content`. On success, write `plan_state`.
///   On failure, set `pending_prose_conversion` so the strategy injects a
///   reformat nudge on the next iteration.
/// - If `pending_prose_conversion` is Some: this is a fallback reply.
///   Try to extract once more, then always clear `pending_prose_conversion`.
fn apply_plan_hook(state: &mut LoopExecutionState, content: &str) {
    let content = content.trim();
    if content.is_empty() {
        return;
    }

    if let Some(ref mut plan) = state.plan_state {
        // Advance step on every non-empty assistant reply (the model has
        // completed the current step and may proceed to the next).
        if state.pending_prose_conversion.is_none() {
            plan.advance();
            tracing::debug!(
                step = plan.current_step,
                total = plan.steps.len(),
                "planning hook: advanced to next step"
            );
        }
        return;
    }

    // No plan yet — try to extract one.
    let is_fallback = state.pending_prose_conversion.is_some();

    // Classify using the raw content as a proxy for the user message
    // (we don't have the original user message here; good-enough approximation).
    let plan_type = classify(content, &[]);
    if let Some(plan) = AgentPlanState::from_model_reply(content, plan_type) {
        tracing::debug!(
            step_count = plan.steps.len(),
            plan_type = ?plan.plan_type,
            is_fallback,
            "planning hook: extracted plan from model reply"
        );
        state.plan_state = Some(plan);
        state.pending_prose_conversion = None;
    } else if !is_fallback {
        // First iteration failed to produce a parseable plan — set the slot
        // so the context strategy injects a reformat nudge next iteration.
        tracing::debug!(
            content_len = content.len(),
            "planning hook: no plan extracted, setting pending_prose_conversion"
        );
        state.pending_prose_conversion = Some(content.to_owned());
    } else {
        // Fallback also failed — clear the slot so the loop continues
        // without getting stuck in a conversion loop.
        tracing::debug!(
            "planning hook: fallback conversion failed, clearing pending_prose_conversion"
        );
        state.pending_prose_conversion = None;
    }
}

fn estimate_output_tokens(reply: &AssistantReply) -> u32 {
    if reply.content.is_empty() {
        return 0;
    }
    let estimated = reply.content.len().div_ceil(4).max(1);
    estimated.min(u32::MAX as usize) as u32
}
