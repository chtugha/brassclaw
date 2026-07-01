use std::sync::Arc;

use brassclaw_agent_loop::{
    families,
    family::{LoopFamilyRegistry, LoopFamilyRegistryError},
};

/// Configuration forwarded from `DefaultPlannedRuntimeConfig` to the loop
/// family registry builder.
#[derive(Debug, Clone, Default)]
pub struct LoopFamilyConfig {
    /// Token budget for conversation history context.
    pub conversation_context_tokens: Option<usize>,
    /// Token budget for the visible capability surface (tool descriptions).
    /// Stored here for future capability strategy enforcement; the current
    /// `DefaultCapabilityStrategy` does not filter by token count.
    pub capability_surface_tokens: Option<usize>,
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
    })
}

/// Build the production loop-family registry with full token budget config.
pub fn build_loop_family_registry_with_full_config(
    config: LoopFamilyConfig,
) -> Result<Arc<LoopFamilyRegistry>, LoopFamilyRegistryError> {
    if let Some(budget) = config.capability_surface_tokens {
        tracing::debug!(
            capability_surface_tokens = budget,
            "capability surface token budget configured (enforcement pending strategy upgrade)"
        );
    }
    LoopFamilyRegistry::with_families(vec![
        Arc::new(families::default_with_context_tokens(
            config.conversation_context_tokens,
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
