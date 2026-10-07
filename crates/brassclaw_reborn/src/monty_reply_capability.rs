//! Task-bound adapter for the existing transcript reply primitive. Register it
//! behind the real kernel before exposing its selected PythonCode binding.
//! It neither advances a Recipe nor supplies approval, retry or completion.
use std::{sync::Arc, time::Instant};

use async_trait::async_trait;
use brassclaw_host_api::{ResourceUsage, RuntimeDispatchErrorKind};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, FirstPartyCapabilityHandler, FirstPartyCapabilityRequest,
    FirstPartyCapabilityResult,
};

use crate::monty_task_host::{MontyTaskHost, POST_REPLY_CAPABILITY_ID};

struct TaskReplyCapability(Arc<MontyTaskHost>);

impl MontyTaskHost {
    /// Register this exact admitted host's reply primitive in a captured
    /// first-party kernel view. The adapter performs one transcript operation;
    /// it creates no capability declaration, permission or component approval.
    pub fn reply_capability(
        self: &Arc<Self>,
    ) -> Arc<impl FirstPartyCapabilityHandler + 'static + use<>> {
        Arc::new(TaskReplyCapability(self.clone()))
    }
}

#[async_trait]
impl FirstPartyCapabilityHandler for TaskReplyCapability {
    async fn dispatch(
        &self,
        request: FirstPartyCapabilityRequest,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        let context = self.0.run_context();
        // These are data-address checks, not user/project role authorization.
        // The bound host owns its exact run/attempt and checks cancellation and
        // freshness again before and after actual transcript persistence.
        if request.capability_id.as_str() != POST_REPLY_CAPABILITY_ID
            || request.scope.tenant_id != context.scope.tenant_id
            || request.scope.agent_id != context.scope.agent_id
            || request.scope.project_id != context.scope.project_id
            || request.scope.thread_id.as_ref() != Some(&context.thread_id)
            || context
                .actor
                .as_ref()
                .is_none_or(|actor| actor.user_id != request.scope.user_id)
        {
            return Err(FirstPartyCapabilityError::new(
                RuntimeDispatchErrorKind::PolicyDenied,
            ));
        }
        let input = request.input.as_object().filter(|input| input.len() == 1);
        if input
            .and_then(|input| input.get("answer"))
            .and_then(serde_json::Value::as_str)
            .is_none_or(|answer| answer.trim().is_empty())
        {
            return Err(FirstPartyCapabilityError::new(
                RuntimeDispatchErrorKind::InputEncode,
            ));
        }
        let started = Instant::now();
        let actual = self.0.dispatch_port("post_reply", request.input).await;
        let usage = ResourceUsage {
            wall_clock_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            ..ResourceUsage::default()
        };
        match actual {
            Ok(output) => Ok(FirstPartyCapabilityResult::new(output, usage)),
            // Preserve the stable host classification without copying arbitrary
            // diagnostics into the Tool/model result. Cancellation or a failed
            // transcript write does not establish that no effect occurred.
            Err(error) => Err(FirstPartyCapabilityError::with_safe_summary(
                RuntimeDispatchErrorKind::OperationFailed,
                error.kind.as_str(),
            )
            .with_usage(usage)),
        }
    }
}
