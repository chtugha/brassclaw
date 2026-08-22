//! Recipe-Skill-Tool lookup stage (Phase 7).
//!
//! Runs after `input` (we have a pending user message) and before `prompt`
//! (we would otherwise spend tokens assembling an LLM request). The stage
//! consults the host's retrieval lookup:
//!
//! - **Tier 0 — direct execution**: a Recipe crosses the Wilson
//!   lower-confidence threshold, so the host can perform the action
//!   chain without any LLM round-trip.
//! - **Tier 1 — guided execution**: a Recipe matches but is below the
//!   threshold; inject the matched ToolSkill summaries into the prompt
//!   so the LLM follows the proven pattern.
//! - **Tier 2 — full LLM reasoning**: no Recipe matches; fall through
//!   to the existing prompt/model/capability pipeline unchanged.
//!
//! ## v3 Phase E.0 (agent-loop adaptation)
//!
//! `InputStage::drain` populates `state.last_user_text` with the raw
//! accepted-message body (plan §H3, via `LoopContextPort::resolve_message_text`),
//! so the user's text is now reachable at this pipeline position without the
//! assembled prompt. When a `RetrievalLookup` is wired (`LoopRetrievalPort`,
//! plan §H4) AND `last_user_text` is present, this stage fires
//! `fetch_for_turn` against the engine `PostgresSource` (running `resolve_intent`
//! and the SEC-01-gated component fetch in a live turn) and stashes the result
//! into `state.last_retrieval_result` for the Phase H consumer.
//!
//! **E.0 deliberately does NOT consume the result**: Tier-0/Tier-1 dispatch
//! (`LoopOrchestratorPort` / `TierZeroExecutionStage`, plan §H5) is Phase H's
//! job. The stage therefore always returns [`RecipeStep::Continue`] (Tier-2
//! fall-through); retrieval is a producer-only side effect at this phase.
//! Retrieval errors are soft-failed (debug-logged, `last_retrieval_result`
//! left `None`) — a retrieval failure must never break a turn.
use async_trait::async_trait;
use tracing::debug;

use crate::state::LoopExecutionState;

use super::{AgentLoopExecutorError, ExecutorStage, StageContext};

/// Token budget forwarded to `RetrievalLookup::fetch_for_turn` (v3 Phase E.0).
/// Placeholder constant — Phase E refines this from the run-profile budget.
const RETRIEVAL_TOKEN_BUDGET: usize = 4096;

/// `sender_class_code` for the orchestrator channel (class 02) — the calling
/// component that drives intent-driven retrieval in the recipe stage.
const RECIPE_SENDER_CLASS_CODE: &str = "02";

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct RecipeStage;

pub(super) struct RecipeInput {
    pub(super) state: LoopExecutionState,
}

pub(super) enum RecipeStep {
    /// Fall through to `prompt` (Tier 2): either no recipe matched or
    /// the matched recipe is below the Wilson threshold.
    Continue { state: Box<LoopExecutionState> },
}

#[async_trait]
impl ExecutorStage<RecipeInput> for RecipeStage {
    type Output = RecipeStep;

    async fn process(
        &self,
        ctx: StageContext<'_>,
        input: RecipeInput,
    ) -> Result<RecipeStep, AgentLoopExecutorError> {
        let mut state = input.state;

        // v3 plan §H4: fire intent-driven retrieval when both the host wires a
        // `RetrievalLookup` AND `InputStage::drain` populated `last_user_text`
        // (plan §H3). Either slot missing → Tier-2 fall-through (correct
        // explicit behaviour — no intent-driven retrieval possible).
        let (Some(lookup), Some(user_text)) =
            (ctx.host.retrieval_lookup(), state.last_user_text.as_ref())
        else {
            debug!(
                iteration = state.iteration,
                has_lookup = ctx.host.retrieval_lookup().is_some(),
                has_user_text = state.last_user_text.is_some(),
                "recipe stage: no retrieval lookup or no user text — \
                 falling through to LLM (Tier 2)"
            );
            return Ok(RecipeStep::Continue {
                state: Box::new(state),
            });
        };

        match lookup
            .fetch_for_turn(
                ctx.host.run_context(),
                user_text,
                RETRIEVAL_TOKEN_BUDGET,
                RECIPE_SENDER_CLASS_CODE,
            )
            .await
        {
            Ok(Some(result)) => {
                debug!(
                    iteration = state.iteration,
                    tier0_eligible = result.tier0_eligible,
                    llm_call_required = result.llm_call_required,
                    routing_meta = ?result.routing_meta,
                    "recipe stage: retrieval produced a result — stashing for \
                     the Phase H consumer (Tier-2 fall-through preserved)"
                );
                state.last_retrieval_result = Some(result);
            }
            Ok(None) => {
                debug!(
                    iteration = state.iteration,
                    "recipe stage: retrieval soft-missed (no component matched) \
                     — falling through to LLM (Tier 2)"
                );
                state.last_retrieval_result = None;
            }
            Err(error) => {
                // Soft-fail: a retrieval backend failure must never break a
                // turn. Leave `last_retrieval_result = None` and fall through.
                debug!(
                    iteration = state.iteration,
                    error = %error,
                    "recipe stage: retrieval lookup errored — soft-failing to \
                     LLM (Tier 2)"
                );
                state.last_retrieval_result = None;
            }
        }

        // E.0: producer-only — Tier-0/Tier-1 dispatch is Phase H's consumer.
        Ok(RecipeStep::Continue {
            state: Box::new(state),
        })
    }
}
