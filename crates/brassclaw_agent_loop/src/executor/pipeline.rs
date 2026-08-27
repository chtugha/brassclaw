use async_trait::async_trait;
use brassclaw_turns::run_profile::AgentLoopDriverHost;

use crate::planner::AgentLoopPlannerInternal;

use super::{
    AgentLoopExecutorError, AssistantReplyStage, BudgetStage, CapabilityStage, ExitStage,
    InputStage, InterceptorStage, ModelStage, PromptStage, RecipeStage, ReplyAdmissionStage,
    StopStage, TierZeroExecutionStage,
};

#[derive(Clone, Copy)]
pub(crate) struct StageContext<'a> {
    pub(crate) planner: &'a dyn AgentLoopPlannerInternal,
    pub(crate) host: &'a (dyn AgentLoopDriverHost + Send + Sync),
}

#[async_trait]
pub(crate) trait ExecutorStage<Input>: Send + Sync {
    type Output;

    async fn process(
        &self,
        ctx: StageContext<'_>,
        input: Input,
    ) -> Result<Self::Output, AgentLoopExecutorError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct DefaultExecutorPipeline {
    pub(crate) budget: BudgetStage,
    pub(crate) input: InputStage,
    pub(crate) recipe: RecipeStage,
    pub(crate) tier_zero: TierZeroExecutionStage,
    pub(crate) prompt: PromptStage,
    pub(crate) interceptor: InterceptorStage,
    pub(crate) model: ModelStage,
    pub(crate) reply_admission: ReplyAdmissionStage,
    pub(crate) assistant_reply: AssistantReplyStage,
    pub(crate) capabilities: CapabilityStage,
    pub(crate) stop: StopStage,
    pub(crate) exit: ExitStage,
}
