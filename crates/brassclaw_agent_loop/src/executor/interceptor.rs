//! Interceptor stage — captures assembled prompts for Sempai–Kohai telemetry.
//!
//! Sits between `PromptStage` and `ModelStage` in the executor pipeline.
//!
//! ## Routing state (no Sempai connected)
//! The host's `on_prompt_assembled` returns `None` and the stage is a
//! complete no-op: the prompt flows to the Kohai model unchanged and no
//! packet id is carried forward.
//!
//! ## Rerouting state (Sempai connected)
//! The host's `on_prompt_assembled` returns a `packet_id` string.  The
//! stage carries that id in `InterceptorPacketId` so `on_kohai_response`
//! can correlate the Kohai response to the persisted `ForensicPacket`.
//!
//! Both calls are fire-and-forget from the executor's point of view —
//! a failure inside the host impl must not abort the turn.

use async_trait::async_trait;
use serde_json::json;
use tracing::debug;

use crate::state::LoopExecutionState;

use super::{AgentLoopExecutorError, ExecutorStage, StageContext};

/// A captured packet id from `on_prompt_assembled`.  Absent when the host
/// has no interceptor wired (routing state).
#[derive(Debug, Default, Clone)]
pub(super) struct InterceptorPacketId(pub Option<String>);

/// Input to the interceptor stage — the final prompt output from `PromptStage`.
pub(super) struct InterceptorPromptInput {
    pub(super) state: LoopExecutionState,
    pub(super) messages: Vec<brassclaw_turns::run_profile::LoopModelMessage>,
    pub(super) capability_surface_version: String,
    pub(super) visible_capability_count: usize,
}

pub(super) struct InterceptorPromptOutput {
    pub(super) state: LoopExecutionState,
    pub(super) messages: Vec<brassclaw_turns::run_profile::LoopModelMessage>,
    /// The packet id minted by the host, if any.
    pub(super) packet_id: InterceptorPacketId,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct InterceptorStage;

#[async_trait]
impl ExecutorStage<InterceptorPromptInput> for InterceptorStage {
    type Output = InterceptorPromptOutput;

    async fn process(
        &self,
        ctx: StageContext<'_>,
        input: InterceptorPromptInput,
    ) -> Result<InterceptorPromptOutput, AgentLoopExecutorError> {
        let run_id = ctx.host.run_context().run_id.to_string();
        let iteration = input.state.iteration;

        // Build a lightweight prompt snapshot for the host.
        // The host impl (or a real interceptor service) can enrich this
        // further; the agent-loop crate intentionally keeps it thin to
        // avoid a dependency on `brassclaw_interceptor`.
        let snapshot = json!({
            "run_id": run_id,
            "iteration": iteration,
            "message_count": input.messages.len(),
            "capability_surface_version": input.capability_surface_version,
            "visible_capability_count": input.visible_capability_count,
            "messages": input.messages.iter().map(|m| json!({
                "role": m.role,
                "content_ref": m.content_ref.as_str(),
            })).collect::<Vec<_>>(),
        });

        let packet_id = ctx
            .host
            .on_prompt_assembled(&run_id, iteration, snapshot)
            .await;

        if let Some(ref id) = packet_id {
            debug!(
                run_id = %run_id,
                iteration,
                packet_id = %id,
                "interceptor: prompt captured"
            );
        }

        Ok(InterceptorPromptOutput {
            state: input.state,
            messages: input.messages,
            packet_id: InterceptorPacketId(packet_id),
        })
    }
}

/// Notify the interceptor after the Kohai model responds.
///
/// Called with the assembled `LoopModelResponse` text and token usage.
/// If no packet id was minted (routing state), this is a no-op.
pub(super) async fn notify_interceptor_kohai_response(
    ctx: StageContext<'_>,
    packet_id: &InterceptorPacketId,
    response_text: &str,
    usage: Option<brassclaw_turns::run_profile::LoopModelUsage>,
) {
    let Some(ref id) = packet_id.0 else {
        return;
    };
    let usage_json = usage.map(|u| json!({
        "input_tokens": u.input_tokens,
        "output_tokens": u.output_tokens,
        "cache_read_input_tokens": u.cache_read_input_tokens,
        "cache_creation_input_tokens": u.cache_creation_input_tokens,
    }));
    ctx.host.on_kohai_response(id, response_text, usage_json).await;
    debug!(packet_id = %id, "interceptor: kohai response captured");
}
