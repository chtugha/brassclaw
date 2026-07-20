//! Recipe-Skill-Tool lookup stage (Phase 7).
//!
//! Runs after `input` (we have a pending user message) and before `prompt`
//! (we would otherwise spend tokens assembling an LLM request). The stage
//! consults the host's recipe library:
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
//! ## Structural debt
//!
//! The stage is positioned before `PromptStage` so that Tier 0 can avoid
//! prompt assembly entirely. However, the assembled prompt messages (which
//! carry the user's full text) are not yet available here — only
//! `LoopExecutionState` is in scope.
//!
//! To resolve this properly, one of the following is required:
//! 1. Add a cached `last_user_text: Option<String>` field to
//!    `LoopExecutionState` that `InputStage` populates when it drains
//!    user input — the cheapest option, no extra DB round-trip.
//! 2. Move the stage between `PromptStage` and `ModelStage` and change
//!    `RecipeInput` to wrap `PromptOutput` — enables Tier 1 injection
//!    directly into the assembled messages, but Tier 0 short-circuiting
//!    would still require a pre-prompt pass.
//!
//! Until one of the above paths is implemented, `find_recipe` / `find_skills`
//! are not called and the stage is a pipeline hook-point only (Tier 2 always).
use async_trait::async_trait;
use tracing::debug;

use crate::state::LoopExecutionState;

use super::{AgentLoopExecutorError, ExecutorStage, StageContext};

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
        // See module-level structural debt comment: user text is not yet
        // accessible from `LoopExecutionState` at this pipeline position.
        // Tier 0/1 dispatch requires `last_user_text` in state (option 1)
        // or stage repositioning (option 2). Until then, skip the lookup.
        if ctx.host.recipe_lookup().is_some() {
            debug!(
                iteration = input.state.iteration,
                "recipe stage: library wired but user text unavailable at this \
                 pipeline position — falling through to LLM (Tier 2). \
                 See module doc for resolution options."
            );
        } else {
            debug!(
                iteration = input.state.iteration,
                "recipe stage: no library wired"
            );
        }
        Ok(RecipeStep::Continue {
            state: Box::new(input.state),
        })
    }
}
