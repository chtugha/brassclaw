use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use brassclaw_turns::run_profile::{
    LoopInlineMessage, LoopInlineMessageRole, LoopPromptBundleRequest, LoopSafeSummary, PromptMode,
};

use crate::state::{LoopExecutionState, RepeatedCallWarningPhase};
use crate::strategies::reply_admission::reply_admission_control_message;

pub(crate) const REPEATED_CALL_WARNING_CONTROL_TEXT: &str = "loop control repeated capability call detected change strategy explain new evidence or answer from current evidence";

/// Decides what context the host should materialize for the next model call.
///
/// Pure policy: returns the request value the executor will pass to
/// `LoopPromptPort::build_prompt_bundle`. Does NOT mutate state.
///
/// Inline messages flow through the `inline_messages` field of
/// `LoopPromptBundleRequest`. There is no separate nudge strategy; loop
/// families that need nudges extend their context strategy to populate this
/// field from `state`.
#[async_trait]
pub(crate) trait ContextStrategy: Send + Sync {
    async fn plan_context_request(&self, state: &LoopExecutionState) -> ContextPlan;
}

#[allow(dead_code)]
fn _assert_object_safe(_: &dyn ContextStrategy) {}

pub(crate) struct ContextPlan {
    pub(crate) request: LoopPromptBundleRequest,
    pub(crate) emitted_admission_control: bool,
    pub(crate) emitted_repeated_call_warning: bool,
}

/// A live-updatable token budget slot shared between the context strategy and
/// the settings service.
///
/// Stored as an `AtomicUsize` so reads are lock-free.  Sentinel value `0`
/// means "use the compiled default"; any non-zero value is the active limit.
///
/// Update it with [`LiveTokenBudget::set`]; the next `plan_context_request`
/// call picks up the new value immediately — no restart required.
#[derive(Clone, Debug)]
pub struct LiveTokenBudget(Arc<AtomicUsize>);

impl LiveTokenBudget {
    /// Create a live slot initialised to `initial` (or compiled default when `None`).
    pub fn new(initial: Option<usize>) -> Self {
        Self(Arc::new(AtomicUsize::new(initial.unwrap_or(0))))
    }

    /// Read the current limit.  Returns `None` when the sentinel `0` is set,
    /// meaning "use the caller's compiled default".
    #[inline]
    pub fn get(&self) -> Option<usize> {
        match self.0.load(Ordering::Relaxed) {
            0 => None,
            n => Some(n),
        }
    }

    /// Atomically update the limit.  Pass `None` to revert to the compiled default.
    pub fn set(&self, value: Option<usize>) {
        self.0.store(value.unwrap_or(0), Ordering::Relaxed);
    }
}

/// Reference baseline `ContextStrategy` implementation.
///
/// Requests `PromptMode::TextOnly` with at most [`Self::DEFAULT_MAX_MESSAGES`]
/// transcript messages and no inline nudges. Loop families that want
/// CodeAct-shaped prompts or want to inject nudges swap this strategy
/// rather than mutating state.
///
/// Token-aware budgeting: if `token_budget` is set, the strategy reads the
/// live slot on every call and reduces `max_messages` accordingly.  Updating
/// the [`LiveTokenBudget`] takes effect on the very next turn — no restart.
#[derive(Debug, Clone)]
pub struct DefaultContextStrategy {
    /// Max messages to ask the host to include in the bundle. Default
    /// [`Self::DEFAULT_MAX_MESSAGES`].
    pub max_messages: u32,

    /// Live token budget slot.  `None` inside means "use compiled default".
    /// The `Arc` is shared with the settings service so UI changes propagate
    /// immediately without restarting.
    pub token_budget: Option<LiveTokenBudget>,
}

impl DefaultContextStrategy {
    /// Default ceiling on transcript messages requested per turn.
    pub const DEFAULT_MAX_MESSAGES: u32 = 16;

    /// Default maximum context tokens (8000 tokens ≈ 32KB of text).
    /// This is a conservative limit that works well with most models.
    pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 8000;

    /// Create a new strategy with custom message and token limits.
    pub fn new(max_messages: u32, max_context_tokens: Option<usize>) -> Self {
        Self {
            max_messages,
            token_budget: max_context_tokens.map(|n| LiveTokenBudget::new(Some(n))),
        }
    }

    /// Create a strategy backed by an externally-owned live budget slot.
    /// The caller retains a clone of the slot and can call `set()` at any time.
    pub fn with_live_budget(max_messages: u32, budget: LiveTokenBudget) -> Self {
        Self {
            max_messages,
            token_budget: Some(budget),
        }
    }

    /// Create a strategy with a one-shot token budget (not live-updatable).
    pub fn with_token_budget(max_messages: u32, max_context_tokens: usize) -> Self {
        Self {
            max_messages,
            token_budget: Some(LiveTokenBudget::new(Some(max_context_tokens))),
        }
    }
}

impl Default for DefaultContextStrategy {
    fn default() -> Self {
        Self {
            max_messages: Self::DEFAULT_MAX_MESSAGES,
            token_budget: Some(LiveTokenBudget::new(Some(Self::DEFAULT_MAX_CONTEXT_TOKENS))),
        }
    }
}

#[async_trait]
impl ContextStrategy for DefaultContextStrategy {
    async fn plan_context_request(&self, state: &LoopExecutionState) -> ContextPlan {
        let loop_control = loop_control_inline_messages(state);

        // Token-aware message limit adjustment — reads live atomic slot each call
        // so UI changes take effect on the very next turn without a restart.
        let max_messages = if let Some(max_tokens) = self.token_budget.as_ref().and_then(|b| b.get()) {
            // Estimate tokens from inline control messages
            let control_tokens: usize = loop_control
                .inline_messages
                .iter()
                .map(|msg| crate::token_budget::estimate_tokens(msg.safe_body.as_str()))
                .sum();

            // Reserve tokens for system instructions (skills, memory, etc.)
            // Assume ~50% of budget goes to conversation, rest to instructions
            let conversation_budget = max_tokens.saturating_sub(control_tokens) / 2;

            // Estimate average tokens per message (~200 tokens per message)
            // This is a rough heuristic; actual messages vary widely
            const AVG_TOKENS_PER_MESSAGE: usize = 200;
            let estimated_messages = conversation_budget / AVG_TOKENS_PER_MESSAGE.max(1);

            // Use the smaller of configured max_messages or token-based estimate
            self.max_messages.min(estimated_messages as u32)
        } else {
            self.max_messages
        };

        // `max(1)` keeps the host's "zero is rejected" invariant from
        // `LoopPromptBundleRequest` even if a loop family overrides
        // `max_messages` to zero by accident or token budget is exhausted.
        ContextPlan {
            request: LoopPromptBundleRequest {
                mode: PromptMode::TextOnly,
                context_cursor: None,
                surface_version: None,
                checkpoint_state_ref: None,
                max_messages: Some(max_messages.max(1)),
                inline_messages: loop_control.inline_messages,
                capability_view: None,
            },
            emitted_admission_control: loop_control.emitted_admission_control,
            emitted_repeated_call_warning: loop_control.emitted_repeated_call_warning,
        }
    }
}

pub(crate) struct LoopControlInlineMessages {
    pub(crate) inline_messages: Vec<LoopInlineMessage>,
    pub(crate) emitted_admission_control: bool,
    pub(crate) emitted_repeated_call_warning: bool,
}

pub(crate) fn loop_control_inline_messages(state: &LoopExecutionState) -> LoopControlInlineMessages {
    let mut inline_messages = Vec::new();
    let mut emitted_admission_control = false;
    if let Some(rejection) = state.reply_admission_state.pending_rejection.as_ref()
        && !state.reply_admission_state.pending_rejection_rendered
    {
        inline_messages.push(reply_admission_control_message(rejection));
        emitted_admission_control = true;
    }

    let emitted_repeated_call_warning = state
        .stop_state
        .repeated_call_warning
        .as_ref()
        .is_some_and(|warning| warning.phase == RepeatedCallWarningPhase::PendingRender);
    if emitted_repeated_call_warning {
        inline_messages.push(repeated_call_warning_control_message());
    }

    LoopControlInlineMessages {
        inline_messages,
        emitted_admission_control,
        emitted_repeated_call_warning,
    }
}

pub(crate) fn repeated_call_warning_control_message() -> LoopInlineMessage {
    LoopInlineMessage {
        role: LoopInlineMessageRole::System,
        safe_body: LoopSafeSummary::new(REPEATED_CALL_WARNING_CONTROL_TEXT)
            .expect("static loop-control text is non-empty and safe"), // safety: static safe ASCII words.
    }
}

#[cfg(test)]
mod tests {
    use brassclaw_host_api::{TenantId, ThreadId};
    use brassclaw_turns::{
        AgentLoopDriverDescriptor, RunProfileId, RunProfileVersion, TurnId, TurnRunId, TurnScope,
        run_profile::{
            CancellationPolicy, CapabilitySurfaceProfileId, CheckpointPolicy, CheckpointSchemaId,
            ConcurrencyClass, ContextProfileId, LoopDriverId, LoopRunContext, ModelProfileId,
            PromptMode, RedactedRunProfileProvenance, ResolvedRunProfile, ResourceBudgetPolicy,
            ResourceBudgetTier, RunClassId, RunProfileFingerprint, RuntimeProfileConstraints,
            SchedulingClass, SteeringPolicy,
        },
    };

    use super::{ContextStrategy, DefaultContextStrategy, LiveTokenBudget};
    use crate::state::{
        CapabilityCallSignature, LoopExecutionState, RepeatedCallWarningState,
        ReplyAdmissionRejection,
    };

    #[allow(dead_code)]
    fn _check(_: &dyn ContextStrategy) {}

    fn test_run_context() -> LoopRunContext {
        let scope = TurnScope::new(
            TenantId::new("tenant-default-context").expect("valid"),
            None,
            None,
            ThreadId::new("thread-default-context").expect("valid"),
        );
        let descriptor = AgentLoopDriverDescriptor {
            id: LoopDriverId::new("default_context_test_driver").expect("valid"),
            version: RunProfileVersion::new(1),
            checkpoint_schema_id: Some(
                CheckpointSchemaId::new("default_context_test_checkpoint").expect("valid"),
            ),
            checkpoint_schema_version: Some(RunProfileVersion::new(1)),
        };
        let resolved_run_profile = ResolvedRunProfile {
            run_class_id: RunClassId::new("default_context_test_class").expect("valid"),
            profile_id: RunProfileId::default_profile(),
            profile_version: RunProfileVersion::new(1),
            loop_driver: descriptor.clone(),
            checkpoint_schema_id: descriptor
                .checkpoint_schema_id
                .clone()
                .expect("descriptor checkpoint id"),
            checkpoint_schema_version: descriptor
                .checkpoint_schema_version
                .expect("descriptor checkpoint version"),
            model_profile_id: ModelProfileId::new("default_context_test_model").expect("valid"),
            capability_surface_profile_id: CapabilitySurfaceProfileId::new(
                "default_context_test_capabilities",
            )
            .expect("valid"),
            context_profile_id: ContextProfileId::new("default_context_test_context")
                .expect("valid"),
            steering_policy: SteeringPolicy {
                allow_steering: false,
                allow_interrupt: true,
                allow_driver_specific_nudges: false,
            },
            cancellation_policy: CancellationPolicy {
                allow_cancel: true,
                require_checkpoint_before_cancel: false,
            },
            checkpoint_policy: CheckpointPolicy {
                require_before_model: false,
                require_before_side_effect: false,
                require_before_block: true,
                max_checkpoint_bytes: 64 * 1024,
                require_final_checkpoint: false,
                allow_no_reply_completion: false,
            },
            resource_budget_policy: ResourceBudgetPolicy {
                tier: ResourceBudgetTier::new("default_context_test_tier").expect("valid"),
                max_model_calls: 32,
                max_capability_invocations: 64,
            },
            personal_context_policy: brassclaw_turns::run_profile::PersonalContextPolicy::Excluded,
            runtime_constraints: RuntimeProfileConstraints {
                allow_raw_runtime_backend_selection: false,
                allow_broad_capability_surface: false,
            },
            runner_pool_id: None,
            scheduling_class: SchedulingClass::new("interactive").expect("valid"),
            concurrency_class: ConcurrencyClass::new("thread_serial").expect("valid"),
            resolution_fingerprint: RunProfileFingerprint::new("default-context-test-fingerprint")
                .expect("valid"),
            provenance: RedactedRunProfileProvenance {
                sources: vec![],
                effective_privileges: vec![],
            },
        };
        LoopRunContext::new(scope, TurnId::new(), TurnRunId::new(), resolved_run_profile)
    }

    #[test]
    fn default_max_messages_is_sixteen() {
        assert_eq!(DefaultContextStrategy::default().max_messages, 16);
    }

    #[tokio::test]
    async fn plan_context_request_returns_text_only_bundle() {
        let strategy = DefaultContextStrategy::default();
        let state = LoopExecutionState::initial_for_run(&test_run_context());

        let request = strategy.plan_context_request(&state).await;

        assert_eq!(request.request.mode, PromptMode::TextOnly);
        assert_eq!(request.request.max_messages, Some(16));
        assert!(request.request.inline_messages.is_empty());
        assert!(!request.emitted_admission_control);
        assert!(!request.emitted_repeated_call_warning);
        assert!(request.request.context_cursor.is_none());
        assert!(request.request.surface_version.is_none());
        assert!(request.request.checkpoint_state_ref.is_none());
    }

    #[tokio::test]
    async fn plan_context_request_clamps_zero_to_one() {
        let strategy = DefaultContextStrategy {
            max_messages: 0,
            token_budget: Some(LiveTokenBudget::new(Some(DefaultContextStrategy::DEFAULT_MAX_CONTEXT_TOKENS))),
        };
        let state = LoopExecutionState::initial_for_run(&test_run_context());

        let request = strategy.plan_context_request(&state).await;

        assert_eq!(request.request.max_messages, Some(1));
    }

    #[tokio::test]
    async fn plan_context_request_emits_pending_reply_admission_control_once() {
        let strategy = DefaultContextStrategy::default();
        let mut state = LoopExecutionState::initial_for_run(&test_run_context());
        state.reply_admission_state.pending_rejection =
            Some(ReplyAdmissionRejection::stop_condition_not_met());

        let request = strategy.plan_context_request(&state).await;

        assert!(request.emitted_admission_control);
        assert!(!request.emitted_repeated_call_warning);
        assert_eq!(request.request.inline_messages.len(), 1);
        assert_eq!(
            request.request.inline_messages[0].safe_body.as_str(),
            "loop control reply rejected stop condition not met continue"
        );
    }

    #[tokio::test]
    async fn plan_context_request_suppresses_rendered_reply_admission_control() {
        let strategy = DefaultContextStrategy::default();
        let mut state = LoopExecutionState::initial_for_run(&test_run_context());
        state.reply_admission_state.pending_rejection =
            Some(ReplyAdmissionRejection::stop_condition_not_met());
        state.reply_admission_state.pending_rejection_rendered = true;

        let request = strategy.plan_context_request(&state).await;

        assert!(!request.emitted_admission_control);
        assert!(!request.emitted_repeated_call_warning);
        assert!(request.request.inline_messages.is_empty());
    }

    #[tokio::test]
    async fn plan_context_request_emits_pending_repeated_call_warning_once() {
        let strategy = DefaultContextStrategy::default();
        let mut state = LoopExecutionState::initial_for_run(&test_run_context());
        state.stop_state.repeated_call_warning = Some(RepeatedCallWarningState::pending_render(
            CapabilityCallSignature::from_call(
                brassclaw_host_api::CapabilityId::new("demo.echo").expect("valid"),
                &serde_json::json!({"x": 1}),
            )
            .expect("valid signature"),
        ));

        let request = strategy.plan_context_request(&state).await;

        assert!(!request.emitted_admission_control);
        assert!(request.emitted_repeated_call_warning);
        assert_eq!(request.request.inline_messages.len(), 1);
        assert_eq!(
            request.request.inline_messages[0].safe_body.as_str(),
            super::REPEATED_CALL_WARNING_CONTROL_TEXT
        );
    }

    #[tokio::test]
    async fn plan_context_request_suppresses_rendered_repeated_call_warning() {
        let strategy = DefaultContextStrategy::default();
        let mut state = LoopExecutionState::initial_for_run(&test_run_context());
        state.stop_state.repeated_call_warning = Some(RepeatedCallWarningState::rendered(
            CapabilityCallSignature::from_call(
                brassclaw_host_api::CapabilityId::new("demo.echo").expect("valid"),
                &serde_json::json!({"x": 1}),
            )
            .expect("valid signature"),
        ));

        let request = strategy.plan_context_request(&state).await;

        assert!(!request.emitted_admission_control);
        assert!(!request.emitted_repeated_call_warning);
        assert!(request.request.inline_messages.is_empty());
    }
}
