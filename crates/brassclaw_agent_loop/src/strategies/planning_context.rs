//! `PlanningContextStrategy` — two-phase planning context for small models.
//!
//! ## Phase 0 (iteration == 0, no plan yet)
//!
//! Requests minimal context (`max_messages = 2`) and injects a planning
//! instruction inline. The instruction asks the model to produce a step list
//! before acting. Capped at `PLAN_TOKEN_BUDGET` tokens.
//!
//! ## Phase 1+ (plan present)
//!
//! Injects the current step text as an inline `User` message. Limits history
//! to `max_execution_messages` (default 2) to keep context tight.
//!
//! ## Fallback / pending conversion
//!
//! If iteration 0 returned prose (model couldn't produce JSON or a list), the
//! executor sets `state.pending_prose_conversion = Some(prose_text)`. On the
//! very next context request the strategy injects a JSON-reformat instruction
//! alongside the prose, giving the model one deterministic retry with a clear
//! format instruction. The executor clears `pending_prose_conversion` after
//! it has parsed the plan from the subsequent reply.

use async_trait::async_trait;
use brassclaw_turns::run_profile::{
    LoopInlineMessage, LoopInlineMessageRole, LoopPromptBundleRequest, LoopSafeSummary, PromptMode,
};

use crate::state::LoopExecutionState;
use crate::strategies::context::{ContextPlan, ContextStrategy, loop_control_inline_messages};

/// `LoopSafeSummary` has a 512-character hard limit; we stay 2 chars under it.
const SAFE_SUMMARY_MAX_CHARS: usize = 510;

/// Strip characters that `LoopSafeSummary` rejects and truncate to its 512-char limit.
fn sanitize_for_safe_summary(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '{' | '}' | '[' | ']' | '`' | '<' | '>' | '/' | '\\'))
        .take(SAFE_SUMMARY_MAX_CHARS)
        .collect()
}

/// Max tokens for the planning phase (iteration 0).
const PLAN_TOKEN_BUDGET: usize = 1500;

/// Default max messages during plan execution (iterations 1+).
const DEFAULT_EXECUTION_MAX_MESSAGES: u32 = 2;

/// Inline planning instruction injected at iteration 0.
/// Note: must not contain { } [ ] ` < > / \ (LoopSafeSummary constraint).
const PLANNING_INSTRUCTION: &str = "Before acting, output a numbered step-by-step plan. \
     Use format: 1. step one, 2. step two, 3. step three. \
     Do not perform any actions yet - plan only.";

/// Inline numbered-list reformat instruction injected when pending_prose_conversion is set.
const NUMBERED_REFORMAT_INSTRUCTION: &str = "Your previous response was not structured as a step list. \
     Please restate the plan as a numbered list: 1. first step, 2. second step, and so on. \
     Do not perform any actions yet.";

/// Configuration for [`PlanningContextStrategy`].
#[derive(Debug, Clone)]
pub struct PlanningContextConfig {
    /// Max history messages during plan-execution iterations. Default: 2.
    pub max_execution_messages: u32,
    /// Token budget for the planning iteration. Default: 1500.
    pub plan_token_budget: usize,
}

impl Default for PlanningContextConfig {
    fn default() -> Self {
        Self {
            max_execution_messages: DEFAULT_EXECUTION_MAX_MESSAGES,
            plan_token_budget: PLAN_TOKEN_BUDGET,
        }
    }
}

/// Two-phase context strategy for small models.
///
/// See module docs for the full algorithm.
pub struct PlanningContextStrategy {
    config: PlanningContextConfig,
}

impl PlanningContextStrategy {
    pub fn new(config: PlanningContextConfig) -> Self {
        Self { config }
    }
}

impl Default for PlanningContextStrategy {
    fn default() -> Self {
        Self::new(PlanningContextConfig::default())
    }
}

#[async_trait]
impl ContextStrategy for PlanningContextStrategy {
    async fn plan_context_request(&self, state: &LoopExecutionState) -> ContextPlan {
        let loop_control = loop_control_inline_messages(state);
        let mut inline_messages = loop_control.inline_messages;

        // ── Phase 0: no plan yet — request planning ──────────────────────────
        if state.plan_state.is_none() && state.pending_prose_conversion.is_none() {
            // Inject planning instruction
            inline_messages.push(LoopInlineMessage {
                role: LoopInlineMessageRole::System,
                safe_body: LoopSafeSummary::new(PLANNING_INSTRUCTION)
                    .expect("static planning instruction is valid"),
            });

            // Derive max_messages from token budget (same formula as DefaultContextStrategy)
            let estimated_messages = (self.config.plan_token_budget / 200).max(1) as u32;
            let max_messages = estimated_messages.clamp(1, 2);

            return ContextPlan {
                request: LoopPromptBundleRequest {
                    mode: PromptMode::TextOnly,
                    context_cursor: None,
                    surface_version: None,
                    checkpoint_state_ref: None,
                    max_messages: Some(max_messages),
                    inline_messages,
                    capability_view: None,
                    recipe_hint: None,
                },
                emitted_admission_control: loop_control.emitted_admission_control,
                emitted_repeated_call_warning: loop_control.emitted_repeated_call_warning,
            };
        }

        // ── Pending prose conversion: inject reformat nudge ──────────────────
        if state.pending_prose_conversion.is_some() {
            // Inject only the static reformat instruction (dynamic prose may
            // contain LoopSafeSummary-forbidden characters; sanitizing it for
            // the inline message loses too much signal — the instruction alone
            // is sufficient for the retry attempt).
            if let Ok(body) = LoopSafeSummary::new(NUMBERED_REFORMAT_INSTRUCTION) {
                inline_messages.push(LoopInlineMessage {
                    role: LoopInlineMessageRole::System,
                    safe_body: body,
                });
            }

            return ContextPlan {
                request: LoopPromptBundleRequest {
                    mode: PromptMode::TextOnly,
                    context_cursor: None,
                    surface_version: None,
                    checkpoint_state_ref: None,
                    max_messages: Some(2),
                    inline_messages,
                    capability_view: None,
                    recipe_hint: None,
                },
                emitted_admission_control: loop_control.emitted_admission_control,
                emitted_repeated_call_warning: loop_control.emitted_repeated_call_warning,
            };
        }

        // ── Phase 1+: plan present — inject current step ────────────────────
        if let Some(plan) = &state.plan_state
            && let Some(step_text) = plan.current_step_text()
        {
            let step_num = plan.current_step + 1;
            let total = plan.steps.len();
            // Sanitize step text: strip LoopSafeSummary-forbidden characters
            let safe_step = sanitize_for_safe_summary(step_text);
            let msg = sanitize_for_safe_summary(&format!(
                "Execute step {step_num} of {total}: {safe_step}"
            ));
            if let Ok(body) = LoopSafeSummary::new(msg) {
                inline_messages.push(LoopInlineMessage {
                    role: LoopInlineMessageRole::User,
                    safe_body: body,
                });
            }
        }

        ContextPlan {
            request: LoopPromptBundleRequest {
                mode: PromptMode::TextOnly,
                context_cursor: None,
                surface_version: None,
                checkpoint_state_ref: None,
                max_messages: Some(self.config.max_execution_messages.max(1)),
                inline_messages,
                capability_view: None,
                recipe_hint: None,
            },
            emitted_admission_control: loop_control.emitted_admission_control,
            emitted_repeated_call_warning: loop_control.emitted_repeated_call_warning,
        }
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
            RedactedRunProfileProvenance, ResolvedRunProfile, ResourceBudgetPolicy,
            ResourceBudgetTier, RunClassId, RunProfileFingerprint, RuntimeProfileConstraints,
            SchedulingClass, SteeringPolicy,
        },
    };

    use crate::plan_state::{AgentPlanState, PlanType};
    use crate::state::LoopExecutionState;
    use crate::strategies::ContextStrategy;

    use super::*;

    fn test_run_context() -> LoopRunContext {
        let scope = TurnScope::new(
            TenantId::new("tenant-planning-ctx").expect("valid"),
            None,
            None,
            ThreadId::new("thread-planning-ctx").expect("valid"),
        );
        let descriptor = AgentLoopDriverDescriptor {
            id: LoopDriverId::new("planning_ctx_test_driver").expect("valid"),
            version: RunProfileVersion::new(1),
            checkpoint_schema_id: Some(
                CheckpointSchemaId::new("planning_ctx_test_checkpoint").expect("valid"),
            ),
            checkpoint_schema_version: Some(RunProfileVersion::new(1)),
        };
        let resolved_run_profile = ResolvedRunProfile {
            run_class_id: RunClassId::new("planning_ctx_test_class").expect("valid"),
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
            model_profile_id: ModelProfileId::new("planning_ctx_test_model").expect("valid"),
            capability_surface_profile_id: CapabilitySurfaceProfileId::new(
                "planning_ctx_test_capabilities",
            )
            .expect("valid"),
            context_profile_id: ContextProfileId::new("planning_ctx_test_context").expect("valid"),
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
                tier: ResourceBudgetTier::new("planning_ctx_test_tier").expect("valid"),
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
            resolution_fingerprint: RunProfileFingerprint::new("planning-ctx-test-fingerprint")
                .expect("valid"),
            provenance: RedactedRunProfileProvenance {
                sources: vec![],
                effective_privileges: vec![],
            },
        };
        LoopRunContext::new(scope, TurnId::new(), TurnRunId::new(), resolved_run_profile)
    }

    #[tokio::test]
    async fn phase_0_injects_planning_instruction() {
        let strategy = PlanningContextStrategy::default();
        let state = LoopExecutionState::initial_for_run(&test_run_context());

        let plan = strategy.plan_context_request(&state).await;

        assert_eq!(plan.request.max_messages, Some(2));
        assert!(
            plan.request
                .inline_messages
                .iter()
                .any(|m| { m.safe_body.as_str().contains("plan") })
        );
    }

    #[tokio::test]
    async fn phase_1_injects_step_text() {
        let strategy = PlanningContextStrategy::default();
        let mut state = LoopExecutionState::initial_for_run(&test_run_context());
        state.plan_state = AgentPlanState::from_model_reply(
            r#"{"steps":["Read file","Write file","Verify"]}"#,
            PlanType::FileOperation,
        );

        let plan = strategy.plan_context_request(&state).await;

        assert_eq!(
            plan.request.max_messages,
            Some(DEFAULT_EXECUTION_MAX_MESSAGES)
        );
        assert!(plan.request.inline_messages.iter().any(|m| {
            m.safe_body.as_str().contains("step 1") || m.safe_body.as_str().contains("Read file")
        }));
    }

    #[tokio::test]
    async fn pending_prose_conversion_injects_reformat_nudge() {
        let strategy = PlanningContextStrategy::default();
        let mut state = LoopExecutionState::initial_for_run(&test_run_context());
        state.pending_prose_conversion =
            Some("I will first check the file and then update it and finally restart.".to_owned());

        let plan = strategy.plan_context_request(&state).await;

        assert!(plan.request.inline_messages.iter().any(|m| {
            m.safe_body.as_str().contains("step list") || m.safe_body.as_str().contains("numbered")
        }));
    }
}
