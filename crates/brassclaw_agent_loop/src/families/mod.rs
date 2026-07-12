use std::sync::Arc;

use crate::context_budget::DEFAULT_FALLBACK_CONTEXT_WINDOW;
use crate::default_planner::DefaultPlanner;
use crate::family::{ComponentDigest, LoopFamily};
use crate::planner::AgentLoopPlanner;
use crate::strategies::{
    CapabilityFocusConfig, DefaultCompactionStrategy, FocusedCapabilityStrategy, LiveTokenBudget,
};
use crate::strategies::planning_context::PlanningContextStrategy;

mod subagent;

pub use subagent::{SUBAGENT_FAMILY_DIGEST, subagent};

#[cfg(test)]
const DEFAULT_FAMILY_FINGERPRINT: &[u8] = concat!(
    "brassclaw_agent_loop.default_family.v1:",
    "family_id=default;",
    "identity=component_identity_v1;",
    "planner=DefaultPlanner;",
    "strategies=",
    "context:DefaultContextStrategy(max_messages=16),",
    "compaction:DefaultCompactionStrategy(context_limit=8192,reserve=2048,preserve_tail=1024,deadline_ms=30000),",
    "capability:DefaultCapabilityStrategy(all),",
    "model:DefaultModelStrategy(primary_or_fallback_index),",
    "batch:DefaultBatchPolicyStrategy(exclusive_sequential),",
    "gate:DefaultGateHandlingStrategy(block),",
    "recovery:DefaultRecoveryStrategy(max_attempts_per_class=2),",
    "reply_admission:DefaultReplyAdmissionStrategy(reject_empty_and_provider_transcript_artifacts),",
    "stop:DefaultStopConditionStrategy(window=5,repeat=3,failure_run=3,rejected_reply=invalid_model_output),",
    "drain:DefaultInputDrainStrategy(steering=true,followup=true),",
    "budget:DefaultBudgetStrategy(iteration_limit=32,wall_clock_limit=none)"
)
.as_bytes();

/// Stable digest: BLAKE3-256 of `DEFAULT_FAMILY_FINGERPRINT`.
///
/// Update this digest when the default family composition, planner behavior, or
/// identity schema changes in a replay-relevant way.
pub const DEFAULT_FAMILY_DIGEST: ComponentDigest = ComponentDigest([
    0xdb, 0x08, 0xf5, 0x28, 0x22, 0x05, 0x25, 0x8e, 0x7f, 0x07, 0xff, 0x2b, 0x1a, 0xf0, 0xd7, 0x3e,
    0x49, 0x05, 0xb6, 0x0c, 0xc2, 0x61, 0xc7, 0x93, 0x6e, 0x53, 0xdd, 0x6a, 0x28, 0xfd, 0x78, 0x87,
]);

/// The default loop family: the text-tool-use baseline.
pub fn default() -> LoopFamily {
    let planner = DefaultPlanner::compose_default();
    let id = planner.id().clone();
    let version = planner.version().clone();

    LoopFamily::new(id, version, Arc::new(planner))
}

/// The default loop family with full config: context tokens, capability focus,
/// optional planning context strategy, and optional provider context window.
///
/// - `conversation_token_budget`: live-updatable slot; call `.set()` on the
///   retained clone to update the cap on the next turn without a restart.
///   `None` uses the compiled default.
/// - `capability_focus`: when `Some(cfg)`, wires `FocusedCapabilityStrategy`.
/// - `planning_context`: when `Some(strategy)`, wires `PlanningContextStrategy`
///   **instead of** any context-token-budget strategy (planning mode subsumes it).
/// - `context_window_tokens`: provider context window in tokens. Used by
///   `DefaultContextStrategy` (via `TurnContextBudget`) and by
///   `DefaultCompactionStrategy.context_limit_tokens`. `None` → compiled defaults.
/// - `inline_control_tokens`: optional ceiling for inline loop-control messages
///   (admission control, repeated-call warnings). `None` → no limit.
pub fn default_with_full_config(
    conversation_token_budget: Option<LiveTokenBudget>,
    capability_focus: Option<CapabilityFocusConfig>,
    planning_context: Option<PlanningContextStrategy>,
    context_window_tokens: Option<u32>,
    inline_control_tokens: Option<usize>,
) -> LoopFamily {
    use crate::strategies::context::DefaultContextStrategy;

    let mut slots = crate::default_planner::DefaultStrategySlots::default();

    if let Some(strategy) = planning_context {
        // Planning mode: wires PlanningContextStrategy, which already handles
        // token budgeting internally. The explicit token budget is ignored when
        // planning mode is active — the planning strategy owns iteration 0 context.
        slots = slots.with_context(Arc::new(strategy));
    } else if let Some(budget) = conversation_token_budget {
        let mut strategy = match context_window_tokens {
            Some(window) => DefaultContextStrategy::with_live_budget_and_window(
                DefaultContextStrategy::DEFAULT_MAX_MESSAGES,
                budget,
                window,
            ),
            None => DefaultContextStrategy::with_live_budget(
                DefaultContextStrategy::DEFAULT_MAX_MESSAGES,
                budget,
            ),
        };
        strategy.inline_control_tokens = inline_control_tokens;
        slots = slots.with_context(Arc::new(strategy));
    }

    if let Some(cfg) = capability_focus {
        slots = slots.with_capability(Arc::new(FocusedCapabilityStrategy::new(cfg)));
    }

    // Wire the compaction strategy with the provider-aware context window so
    // compaction does not trigger at 8 192 tokens on a 128K-window model.
    let window = context_window_tokens.unwrap_or(DEFAULT_FALLBACK_CONTEXT_WINDOW) as u64;
    slots = slots.with_compaction(Arc::new(DefaultCompactionStrategy {
        context_limit_tokens: window,
        ..DefaultCompactionStrategy::default()
    }));

    let planner = DefaultPlanner::compose(
        crate::family::LoopFamilyId::DEFAULT,
        crate::family::ComponentIdentity::from_static("default", DEFAULT_FAMILY_DIGEST),
        slots,
    );
    let id = planner.id().clone();
    let version = planner.version().clone();
    LoopFamily::new(id, version, Arc::new(planner))
}

#[cfg(test)]
mod tests {
    use crate::family::LoopFamilyId;

    use super::*;

    #[test]
    fn default_family_has_default_identity() {
        let family = default();

        assert_eq!(family.id(), &LoopFamilyId::DEFAULT);
        assert_eq!(family.version().id, "default");
        assert_ne!(family.version().digest, ComponentDigest([0; 32]));
        assert_eq!(family.version().digest, DEFAULT_FAMILY_DIGEST);
    }

    #[test]
    fn default_family_digest_matches_blake3_fingerprint() {
        assert_eq!(
            DEFAULT_FAMILY_DIGEST,
            ComponentDigest::from_blake3(DEFAULT_FAMILY_FINGERPRINT)
        );
    }
}
