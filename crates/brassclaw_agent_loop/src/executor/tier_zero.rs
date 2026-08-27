//! Tier-0 deterministic execution stage (v3 Phase H.11).
//!
//! Reached when [`RecipeStage`](super::recipe::RecipeStage) returns
//! `RecipeStep::TierZero` (the retrieval result crossed the Wilson
//! lower-confidence threshold AND does not require an LLM call). The stage
//! drives the host's orchestrator bridge (`LoopOrchestratorPort` →
//! [`OrchestratorLookup`]) to run the recipe's baked-in PythonCode orchestrator
//! channel with NO LLM (`run_tier_zero`), then either hands the reply text to
//! [`AssistantReplyStage`](super::assistant_reply::AssistantReplyStage) (the
//! `canonical.rs` caller emits it directly, skipping `PromptStage`/
//! `ModelStage`) or degrades to the Tier-2 LLM path when no bridge is wired or
//! the channel produced no reply.
//!
//! v3 architecture (re-think): the Python orchestrator is the SOLE execution
//! authority — tools are invoked inside the Monty sandbox via
//! `__execute_action__`, never directly from Rust by an LLM (no classical
//! MCP). Tier-0 recipes bake the tool calls into their PythonCode, so this
//! stage only needs the stashed `recipe_hint` + `recipe_rust_context`; no
//! `instruction` or Rust-channel executor fn is involved.
//!
//! The stash is consumed (one-shot semantics, plan §H5 SEC-02): `recipe_hint`
//! and `recipe_rust_context` are taken regardless of outcome, so a turn
//! resumed from a checkpoint never replays them.
use async_trait::async_trait;
use brassclaw_turns::run_profile::AssistantReply;
use tracing::debug;

use crate::state::LoopExecutionState;

use super::{AgentLoopExecutorError, ExecutorStage, StageContext};

/// Input to [`TierZeroExecutionStage::process`].
pub(super) struct TierZeroInput {
    pub(super) state: LoopExecutionState,
}

/// Outcome of a Tier-0 dispatch attempt.
pub(super) enum TierZeroStep {
    /// The orchestrator channel produced a reply — emit it directly via
    /// `AssistantReplyStage` (no LLM call). `matched_component_ids` is carried
    /// for Wilson scoring (`record_recipe_outcome`), wired in H.13.
    Reply {
        state: Box<LoopExecutionState>,
        reply: AssistantReply,
        #[allow(dead_code)]
        matched_component_ids: Vec<String>,
    },
    /// No orchestrator bridge is wired or the channel produced no reply —
    /// degrade to the Tier-2 LLM path.
    Degrade { state: Box<LoopExecutionState> },
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct TierZeroExecutionStage;

#[async_trait]
impl ExecutorStage<TierZeroInput> for TierZeroExecutionStage {
    type Output = TierZeroStep;

    async fn process(
        &self,
        ctx: StageContext<'_>,
        input: TierZeroInput,
    ) -> Result<TierZeroStep, AgentLoopExecutorError> {
        let mut state = input.state;

        let bridge = ctx.host.orchestrator_lookup();
        let recipe_hint = state.recipe_hint.take();

        let (Some(bridge), Some(recipe_hint)) = (bridge, recipe_hint) else {
            debug!(
                iteration = state.iteration,
                has_bridge = ctx.host.orchestrator_lookup().is_some(),
                "tier-zero stage: no orchestrator bridge or no stashed \
                 recipe_hint — degrading to Tier 2"
            );
            return Ok(TierZeroStep::Degrade {
                state: Box::new(state),
            });
        };

        let recipe_rust_context =
            serde_json::Value::Array(std::mem::take(&mut state.recipe_rust_context));

        let reply = bridge
            .run_tier_zero(ctx.host.run_context(), &recipe_hint, &recipe_rust_context)
            .await;

        match reply {
            Some(reply) => {
                debug!(
                    iteration = state.iteration,
                    matched = reply.matched_component_ids.len(),
                    "tier-zero stage: orchestrator channel produced a reply — \
                     emitting directly (no LLM)"
                );
                Ok(TierZeroStep::Reply {
                    state: Box::new(state),
                    reply: AssistantReply {
                        content: reply.text,
                    },
                    matched_component_ids: reply.matched_component_ids,
                })
            }
            None => {
                debug!(
                    iteration = state.iteration,
                    "tier-zero stage: orchestrator channel returned no reply — \
                     degrading to Tier 2"
                );
                Ok(TierZeroStep::Degrade {
                    state: Box::new(state),
                })
            }
        }
    }
}
