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
//! The third (default) branch is what this initial cut always returns;
//! Tier 0 and Tier 1 plumbing arrives in the next iteration once the
//! composition-side `RecipeLibrary` adapter is wired.
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
    Continue {
        state: Box<LoopExecutionState>,
    },
}

#[async_trait]
impl ExecutorStage<RecipeInput> for RecipeStage {
    type Output = RecipeStep;

    async fn process(
        &self,
        ctx: StageContext<'_>,
        input: RecipeInput,
    ) -> Result<RecipeStep, AgentLoopExecutorError> {
        let lookup = ctx.host.recipe_lookup();
        if lookup.is_none() {
            debug!(iteration = input.state.iteration, "recipe stage: no library wired");
        }
        Ok(RecipeStep::Continue {
            state: Box::new(input.state),
        })
    }
}
