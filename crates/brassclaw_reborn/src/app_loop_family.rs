use std::sync::Arc;

use brassclaw_agent_loop::{
    CapabilityFocusConfig, LiveTokenBudget, families,
    family::{LoopFamilyRegistry, LoopFamilyRegistryError},
    strategies::planning_context::{PlanningContextConfig, PlanningContextStrategy},
};

/// Configuration forwarded from `DefaultPlannedRuntimeConfig` to the loop
/// family registry builder.
#[derive(Debug, Clone, Default)]
pub struct LoopFamilyConfig {
    /// Live token budget slot for conversation history context.
    ///
    /// The caller clones and retains the `LiveTokenBudget`; calling `.set()`
    /// on it updates the baked-in `DefaultContextStrategy` on the next turn
    /// — no restart or registry rebuild required.
    pub conversation_token_budget: Option<LiveTokenBudget>,
    /// Token budget for the visible capability surface (tool descriptions).
    pub capability_surface_tokens: Option<usize>,
    /// Provider context window in tokens (live-updatable). Used by
    /// `DefaultContextStrategy` (via `TurnContextBudget`) and by
    /// `DefaultCompactionStrategy` to set `context_limit_tokens`.
    /// `None` → compiled defaults apply.
    pub context_window_tokens: Option<LiveTokenBudget>,
    /// Optional ceiling for inline loop-control message tokens (live-updatable).
    /// Forwarded to `DefaultContextStrategy.inline_control_tokens`.
    pub inline_control_tokens: Option<LiveTokenBudget>,
    /// When true, `FocusedCapabilityStrategy` is wired instead of
    /// `DefaultCapabilityStrategy`. The strategy narrows the visible tool
    /// surface to recently-used capabilities each iteration.
    pub capability_focus_enabled: bool,
    /// When true, `PlanningContextStrategy` is wired instead of
    /// `DefaultContextStrategy`. Injects a planning phase on iteration 0
    /// and step injection on subsequent iterations.
    pub planning_mode_enabled: bool,
}

/// Build the production loop-family registry.
///
/// This is the Reborn composition root for loop families. Adding another
/// Builtin family means adding its factory here; the framework crate exports
/// family factories but does not decide which ones are bound in production.
pub fn build_loop_family_registry() -> Result<Arc<LoopFamilyRegistry>, LoopFamilyRegistryError> {
    build_loop_family_registry_with_full_config(LoopFamilyConfig::default())
}

/// Build the production loop-family registry with full token budget config.
///
/// Pass a [`LiveTokenBudget`] in `config.conversation_token_budget` to enable
/// live-updating: call `.set()` on the retained clone and the next turn picks
/// up the new value automatically.
pub fn build_loop_family_registry_with_full_config(
    config: LoopFamilyConfig,
) -> Result<Arc<LoopFamilyRegistry>, LoopFamilyRegistryError> {
    let capability_focus = if config.capability_focus_enabled {
        tracing::debug!(
            "capability focus strategy enabled: narrowing tool surface to recently-used capabilities"
        );
        Some(CapabilityFocusConfig {
            max_tools: 4,
            // fetch_cached_content will be added in subtask 4; hardcode
            // the expected capability ID here as always_allow so it is
            // available the moment the tool is registered.
            always_allow: vec!["brassclaw.fetch_cached_content".to_owned()],
        })
    } else {
        None
    };

    let planning_context = if config.planning_mode_enabled {
        tracing::debug!("planning mode enabled: two-phase planning context strategy wired");
        Some(PlanningContextStrategy::new(
            PlanningContextConfig::default(),
        ))
    } else {
        None
    };

    LoopFamilyRegistry::with_families(vec![
        Arc::new(families::default_with_full_config(
            config.conversation_token_budget,
            capability_focus,
            planning_context,
            config.context_window_tokens,
            config.inline_control_tokens,
        )),
        Arc::new(families::subagent()),
    ])
}

#[cfg(test)]
mod tests {
    use brassclaw_agent_loop::family::LoopFamilyId;

    use super::*;

    #[test]
    fn production_registry_binds_default_and_subagent_families() {
        let registry = build_loop_family_registry().expect("valid production registry");

        assert!(registry.get(&LoopFamilyId::DEFAULT).is_some());
        assert!(registry.get(&LoopFamilyId::SUBAGENT).is_some());
        assert!(
            registry
                .get(&LoopFamilyId::new("unknown").expect("valid test id"))
                .is_none()
        );
        assert_eq!(registry.ids().count(), 2);
    }
}
