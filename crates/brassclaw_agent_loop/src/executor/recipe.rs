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
//! ## v3 Phase H.9 (agent-loop adaptation; supersedes E.0)
//!
//! `InputStage::drain` populates `state.last_user_text` with the raw
//! accepted-message body (plan §H3, via `LoopContextPort::resolve_message_text`),
//! so the user's text is now reachable at this pipeline position without the
//! assembled prompt. When a `RetrievalLookup` is wired (`LoopRetrievalPort`,
//! plan §H4) AND `last_user_text` is present, this stage fires
//! `fetch_for_turn` against the engine `PostgresSource` (running `resolve_intent`
//! and the SEC-01-gated component fetch in a live turn) and stashes the
//! plan-literal split into `state.recipe_hint` (the `orchestrator_items`) and
//! `state.recipe_rust_context` (the `rust_items` array as `Vec<Value>`) for the
//! Phase H consumer.
//!
//! **H.9 deliberately does NOT consume the result**: Tier-0/Tier-1 dispatch
//! (`LoopOrchestratorPort` / `TierZeroExecutionStage`, plan §H5/H.10–H.12) is
//! Phase H's job. The stage therefore always returns [`RecipeStep::Continue`]
//! (Tier-2 fall-through) at H.9; retrieval is a producer-only side effect at
//! this phase. The routing booleans (`tier0_eligible`/`llm_call_required`) are
//! branched on inline by the H.10 consumer dispatch and are NOT stashed.
//! Retrieval errors are soft-failed (debug-logged, the stash left empty) — a
//! retrieval failure must never break a turn.
//!
//! **SEC-02 (plan §H5):** the stash is cleared at the START of every
//! `RecipeStage::process` so a turn resumed from a checkpoint never replays a
//! stale pre-fetched result.
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

        // SEC-02 (plan §H5): clear the recipe stash at the START of every
        // `RecipeStage::process` so a turn resumed from a checkpoint (which
        // serialised a stale `recipe_hint`/`recipe_rust_context` set by a prior
        // `RecipeStage` run that never reached the orchestrator consumer) does
        // NOT replay the stale pre-fetched result. The stage re-fetches fresh
        // below; the orchestrator consumer (`run_step_zero`/`run_tier_zero`,
        // Phase H.10–H.12) reads the freshly-stashed values.
        state.recipe_hint = None;
        state.recipe_rust_context = Vec::new();

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
                    "recipe stage: retrieval produced a result — stashing the \
                     recipe_hint/recipe_rust_context split for the Phase H \
                     consumer (Tier-2 fall-through preserved at H.9)"
                );
                // H.9 (plan §H5, Q-H1): migrate from the E.0
                // `last_retrieval_result` stash to the plan-literal split.
                // `recipe_hint` holds `orchestrator_items` (consumed by
                // `run_step_zero` Tier-1 / `run_tier_zero` Tier-0); the
                // routing booleans (`tier0_eligible`/`llm_call_required`) are
                // branched on inline by the H.10 consumer dispatch and are NOT
                // stashed. `recipe_rust_context` holds the `rust_items` array
                // split into the plan-literal `Vec<serde_json::Value>`
                // (Q-H9-2: a non-array `rust_items` degrades to an empty vec —
                // the retrieval source always emits an array).
                state.recipe_hint = Some(result.orchestrator_items.clone());
                state.recipe_rust_context =
                    result.rust_items.as_array().cloned().unwrap_or_default();
            }
            Ok(None) => {
                debug!(
                    iteration = state.iteration,
                    "recipe stage: retrieval soft-missed (no component matched) \
                     — falling through to LLM (Tier 2)"
                );
                // SEC-02 clear already ran at the top; stash stays empty.
            }
            Err(error) => {
                // Soft-fail: a retrieval backend failure must never break a
                // turn. Stash stays empty (SEC-02 clear already ran) and the
                // stage falls through to Tier 2.
                debug!(
                    iteration = state.iteration,
                    error = %error,
                    "recipe stage: retrieval lookup errored — soft-failing to \
                     LLM (Tier 2)"
                );
            }
        }

        // H.9: producer-only — Tier-0/Tier-1 consumer dispatch (branching on
        // `tier0_eligible`/`llm_call_required`) is H.10's job.
        Ok(RecipeStep::Continue {
            state: Box::new(state),
        })
    }
}
