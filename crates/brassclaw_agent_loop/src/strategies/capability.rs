use async_trait::async_trait;
use brassclaw_host_api::CapabilityId;
use serde::{Deserialize, Serialize};

use crate::state::LoopExecutionState;

// ── FocusedCapabilityStrategy ────────────────────────────────────────────────

/// Configuration for [`FocusedCapabilityStrategy`].
///
/// `always_allow` lists capability IDs that are unconditionally included in
/// every `AllowOnly` set, regardless of scoring. Use this to guarantee that
/// meta-tools (e.g. `fetch_cached_content`) are always reachable.
#[derive(Debug, Clone)]
pub struct CapabilityFocusConfig {
    /// Maximum number of scored capabilities to include per iteration.
    /// The `always_allow` set is added on top of this limit.
    pub max_tools: usize,
    /// Capability IDs that bypass scoring and are always exposed.
    pub always_allow: Vec<String>,
}

impl Default for CapabilityFocusConfig {
    fn default() -> Self {
        Self {
            max_tools: 4,
            always_allow: vec![],
        }
    }
}

/// A `CapabilityStrategy` that narrows the visible tool surface each iteration
/// to the tools most likely needed given recent execution history.
///
/// ## Scoring
///
/// On iteration 0 (no tool calls yet in this turn) the strategy returns
/// [`CapabilityFilter::All`] — there is no signal to score against.
///
/// On iterations 1+ the strategy:
/// 1. Collects the distinct `CapabilityId`s from `state.recent_call_signatures`
///    (the last ≤8 tool invocations tracked by the executor).
/// 2. Takes the most-recent `config.max_tools` of those IDs as the "hot" set.
/// 3. Unions the hot set with `config.always_allow` (deduplicated).
/// 4. Returns `CapabilityFilter::AllowOnly(union)`.
///
/// When the hot set is empty (no tool calls recorded, though iteration > 0)
/// the strategy falls back to `CapabilityFilter::All`.
///
/// This heuristic is deliberately simple and requires no external data: the
/// model's own recent tool-call pattern is the strongest predictor of what it
/// will need next on multi-step tasks.
pub struct FocusedCapabilityStrategy {
    config: CapabilityFocusConfig,
}

impl FocusedCapabilityStrategy {
    /// Create a strategy with the given configuration.
    pub fn new(config: CapabilityFocusConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl CapabilityStrategy for FocusedCapabilityStrategy {
    async fn filter(&self, state: &LoopExecutionState) -> CapabilityFilter {
        // On the very first iteration there is no execution history to score
        // against — expose everything so the model can pick the right starter.
        if state.recent_call_signatures.is_empty() {
            return CapabilityFilter::All;
        }

        // Collect recently-used capability IDs (most recent first, deduplicated).
        let mut seen = std::collections::HashSet::new();
        let mut hot: Vec<CapabilityId> = Vec::new();
        for sig in state.recent_call_signatures.iter().rev() {
            if seen.insert(sig.name.as_str().to_owned()) {
                hot.push(sig.name.clone());
                if hot.len() >= self.config.max_tools {
                    break;
                }
            }
        }

        if hot.is_empty() {
            return CapabilityFilter::All;
        }

        // Union with always_allow (deduplicated).
        for always in &self.config.always_allow {
            if let Ok(id) = CapabilityId::new(always)
                && !hot.contains(&id)
            {
                hot.push(id);
            }
        }

        CapabilityFilter::AllowOnly(hot)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CapabilityFilter {
    /// Allow everything the host would otherwise expose.
    #[default]
    All,
    /// Only the capabilities whose IDs appear in the set.
    AllowOnly(Vec<CapabilityId>),
    /// Everything except the capabilities whose IDs appear in the set.
    Deny(Vec<CapabilityId>),
}

impl CapabilityFilter {
    pub(crate) fn permits(&self, capability_id: &CapabilityId) -> bool {
        match self {
            Self::All => true,
            Self::AllowOnly(allowed) => allowed.contains(capability_id),
            Self::Deny(denied) => !denied.contains(capability_id),
        }
    }
}

/// Decides which capabilities are visible to the model this iteration.
///
/// Pure policy: returns a filter the executor passes to the host when
/// requesting the visible capability surface. Does NOT mutate state.
///
/// The host is the source of truth for the catalog and applies its own
/// scope/grant/auth filters AFTER the strategy filter; the strategy can only
/// narrow, never expand.
#[async_trait]
pub(crate) trait CapabilityStrategy: Send + Sync {
    async fn filter(&self, state: &LoopExecutionState) -> CapabilityFilter;
}

#[allow(dead_code)]
fn _assert_object_safe(_: &dyn CapabilityStrategy) {}

/// Reference baseline `CapabilityStrategy`: never narrow the host surface.
///
/// The host applies its own scope/grant/auth filters on top — this default
/// strategy declines to filter further, leaving capability visibility entirely
/// to the host's authoritative policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultCapabilityStrategy;

#[async_trait]
impl CapabilityStrategy for DefaultCapabilityStrategy {
    async fn filter(&self, _state: &LoopExecutionState) -> CapabilityFilter {
        CapabilityFilter::All
    }
}

#[cfg(test)]
mod tests {
    use brassclaw_host_api::{CapabilityId, TenantId, ThreadId};
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

    use super::{CapabilityFilter, CapabilityStrategy, DefaultCapabilityStrategy};
    use crate::state::LoopExecutionState;

    #[allow(dead_code)]
    fn _check(_: &dyn CapabilityStrategy) {}

    fn test_run_context() -> LoopRunContext {
        let scope = TurnScope::new(
            TenantId::new("tenant-default-cap").expect("valid"),
            None,
            None,
            ThreadId::new("thread-default-cap").expect("valid"),
        );
        let descriptor = AgentLoopDriverDescriptor {
            id: LoopDriverId::new("default_cap_test_driver").expect("valid"),
            version: RunProfileVersion::new(1),
            checkpoint_schema_id: Some(
                CheckpointSchemaId::new("default_cap_test_checkpoint").expect("valid"),
            ),
            checkpoint_schema_version: Some(RunProfileVersion::new(1)),
        };
        let resolved_run_profile = ResolvedRunProfile {
            run_class_id: RunClassId::new("default_cap_test_class").expect("valid"),
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
            model_profile_id: ModelProfileId::new("default_cap_test_model").expect("valid"),
            capability_surface_profile_id: CapabilitySurfaceProfileId::new(
                "default_cap_test_capabilities",
            )
            .expect("valid"),
            context_profile_id: ContextProfileId::new("default_cap_test_context").expect("valid"),
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
                tier: ResourceBudgetTier::new("default_cap_test_tier").expect("valid"),
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
            resolution_fingerprint: RunProfileFingerprint::new("default-cap-test-fingerprint")
                .expect("valid"),
            provenance: RedactedRunProfileProvenance {
                sources: vec![],
                effective_privileges: vec![],
            },
        };
        LoopRunContext::new(scope, TurnId::new(), TurnRunId::new(), resolved_run_profile)
    }

    #[tokio::test]
    async fn default_capability_strategy_returns_all() {
        let strategy = DefaultCapabilityStrategy;
        let state = LoopExecutionState::initial_for_run(&test_run_context());

        assert_eq!(strategy.filter(&state).await, CapabilityFilter::All);
    }

    #[test]
    fn default_filter_allows_all() {
        assert_eq!(CapabilityFilter::default(), CapabilityFilter::All);
    }

    #[test]
    fn filter_round_trips_through_json() {
        let capability_id = CapabilityId::new("test.echo").expect("valid capability id");
        let filters = vec![
            CapabilityFilter::All,
            CapabilityFilter::AllowOnly(vec![capability_id.clone()]),
            CapabilityFilter::Deny(vec![capability_id]),
        ];

        for filter in filters {
            let encoded = serde_json::to_string(&filter).expect("serialize filter");
            let decoded: CapabilityFilter =
                serde_json::from_str(&encoded).expect("deserialize filter");
            assert_eq!(decoded, filter);
        }
    }

    #[test]
    fn filter_serializes_with_snake_case_wire_form() {
        let capability_id = CapabilityId::new("test.echo").expect("valid capability id");

        assert_eq!(
            serde_json::to_string(&CapabilityFilter::All).expect("serialize all"),
            "\"all\""
        );
        assert_eq!(
            serde_json::to_string(&CapabilityFilter::AllowOnly(vec![capability_id.clone()]))
                .expect("serialize allow_only"),
            "{\"allow_only\":[\"test.echo\"]}"
        );
        assert_eq!(
            serde_json::to_string(&CapabilityFilter::Deny(vec![capability_id]))
                .expect("serialize deny"),
            "{\"deny\":[\"test.echo\"]}"
        );
    }
}
