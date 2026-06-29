use std::sync::Arc;

use brassclaw_agent_loop::{
    families,
    family::{LoopFamilyRegistry, LoopFamilyRegistryError},
};

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
    LoopFamilyRegistry::with_families(vec![
        Arc::new(families::default_with_context_tokens(conversation_context_tokens)),
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
