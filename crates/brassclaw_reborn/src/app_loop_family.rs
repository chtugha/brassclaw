use std::sync::Arc;

use brassclaw_agent_loop::{
    CapabilityFocusConfig,
    families,
    family::{LoopFamilyRegistry, LoopFamilyRegistryError},
    strategies::planning_context::{PlanningContextConfig, PlanningContextStrategy},
};

/// Configuration forwarded from `DefaultPlannedRuntimeConfig` to the loop
/// family registry builder.
#[derive(Debug, Clone, Default)]
pub struct LoopFamilyConfig {
    /// Token budget for conversation history context.
    pub conversation_context_tokens: Option<usize>,
    /// Token budget for the visible capability surface (tool descriptions).
    pub capability_surface_tokens: Option<usize>,
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
    build_loop_family_registry_with_config(None)
}

/// Build the production loop-family registry with an optional conversation
/// context token budget override.
///
/// When `conversation_context_tokens` is `Some(n)`, the default planner uses
/// a `DefaultContextStrategy` capped at `n` tokens for conversation history.
pub fn build_loop_family_registry_with_config(
    conversation_context_tokens: Option<usize>,
) -> Result<Arc<LoopFamilyRegistry>, LoopFamilyRegistryError> {
    build_loop_family_registry_with_full_config(LoopFamilyConfig {
        conversation_context_tokens,
        capability_surface_tokens: None,
        capability_focus_enabled: false,
        planning_mode_enabled: false,
    })
}

/// Build the production loop-family registry with full token budget config.
pub fn build_loop_family_registry_with_full_config(
    config: LoopFamilyConfig,
) -> Result<Arc<LoopFamilyRegistry>, LoopFamilyRegistryError> {
    let capability_focus = if config.capability_focus_enabled {
        tracing::debug!("capability focus strategy enabled: narrowing tool surface to recently-used capabilities");
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
        Some(PlanningContextStrategy::new(PlanningContextConfig::default()))
    } else {
        None
    };

    LoopFamilyRegistry::with_families(vec![
        Arc::new(families::default_with_full_config(
            config.conversation_context_tokens,
            capability_focus,
            planning_context,
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
