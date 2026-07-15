//! Component type tags for the Sempai–Kohai system.
//!
//! Every recipe, skill, and tool declares which execution contexts it applies
//! to via a set of [`ComponentType`] tags.  The registry uses
//! [`ComponentTypeSet`] to filter components by the caller's role context so
//! that, e.g., Sempai-only audit skills never appear in Kohai prompt
//! assembly.

use serde::{Deserialize, Serialize};

/// Declares which execution context a recipe, skill, or tool is intended for.
///
/// A component may carry any combination of these types simultaneously.
/// The filtering predicate is set *intersection*: a component is included
/// when its type set intersects the target context's required types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentType {
    /// Available during general LLM interaction (both Kohai and Sempai contexts).
    Llm,
    /// Available only when assembling prompts for the Kohai inference model.
    Kohai,
    /// Available only when assembling prompts for the Sempai auditor.
    Sempai,
    /// Available to the agentic execution loop (tool calls, code, planning).
    Agent,
}

/// A newtype over `Vec<ComponentType>` that provides role-intersection helpers.
///
/// The default (when a SKILL.md omits the `types` field) is
/// `[Llm, Kohai, Agent]` — available everywhere except the Sempai audit
/// context. This conservative default prevents existing skills from
/// accidentally appearing in Sempai audit prompts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ComponentTypeSet(pub Vec<ComponentType>);

impl ComponentTypeSet {
    /// The default type set returned when a component omits the `types` field.
    ///
    /// `[Llm, Kohai, Agent]` — available in all contexts except Sempai audit.
    pub fn default_types() -> Vec<ComponentType> {
        vec![ComponentType::Llm, ComponentType::Kohai, ComponentType::Agent]
    }

    /// Returns `true` when this set contains `ty`.
    pub fn contains(&self, ty: ComponentType) -> bool {
        self.0.contains(&ty)
    }

    /// Returns `true` when this type set intersects (has at least one element
    /// in common with) `other`.
    pub fn intersects(&self, other: &[ComponentType]) -> bool {
        self.0.iter().any(|t| other.contains(t))
    }

    /// Returns `true` when this component should be included for Kohai prompt
    /// assembly.  A component is included when its set contains `Kohai` **or**
    /// `Llm` (which is available during general LLM interaction).
    pub fn is_kohai_visible(&self) -> bool {
        self.contains(ComponentType::Kohai) || self.contains(ComponentType::Llm)
    }

    /// Returns `true` when this component should be included for Sempai audit
    /// prompt assembly.  Only components that **explicitly** carry the `Sempai`
    /// tag are visible — this is the conservative default that prevents existing
    /// skills (which have `Llm` in their default type set) from accidentally
    /// appearing in Sempai audit prompts.
    pub fn is_sempai_visible(&self) -> bool {
        self.contains(ComponentType::Sempai)
    }

    /// Returns `true` when this component should be included for the agentic
    /// execution loop.
    pub fn is_agent_visible(&self) -> bool {
        self.contains(ComponentType::Agent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_type_roundtrips_json() {
        for (variant, wire) in [
            (ComponentType::Llm, r#""llm""#),
            (ComponentType::Kohai, r#""kohai""#),
            (ComponentType::Sempai, r#""sempai""#),
            (ComponentType::Agent, r#""agent""#),
        ] {
            let serialised = serde_json::to_string(&variant).unwrap();
            assert_eq!(serialised, wire);
            let back: ComponentType = serde_json::from_str(wire).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn component_type_yaml_roundtrip() {
        let types: Vec<ComponentType> = serde_yml::from_str("- llm\n- kohai\n- agent").unwrap();
        assert_eq!(types, vec![ComponentType::Llm, ComponentType::Kohai, ComponentType::Agent]);
    }

    #[test]
    fn default_types_excludes_sempai() {
        let set = ComponentTypeSet(ComponentTypeSet::default_types());
        assert!(set.is_kohai_visible());
        assert!(set.is_agent_visible());
        assert!(!set.is_sempai_visible());
    }

    #[test]
    fn sempai_only_type_set() {
        let set = ComponentTypeSet(vec![ComponentType::Sempai]);
        assert!(set.is_sempai_visible());
        assert!(!set.is_kohai_visible());
        assert!(!set.is_agent_visible());
    }

    #[test]
    fn llm_type_visible_to_kohai_only_by_default() {
        // Llm alone makes a component Kohai-visible (general LLM interaction)
        // but NOT Sempai-visible. To appear in Sempai audit prompts the component
        // must carry an explicit Sempai tag.
        let set = ComponentTypeSet(vec![ComponentType::Llm]);
        assert!(set.is_kohai_visible());
        assert!(!set.is_sempai_visible());
        assert!(!set.is_agent_visible());
    }

    #[test]
    fn llm_and_sempai_visible_to_both_roles() {
        let set = ComponentTypeSet(vec![ComponentType::Llm, ComponentType::Sempai]);
        assert!(set.is_kohai_visible());
        assert!(set.is_sempai_visible());
    }

    #[test]
    fn intersects_returns_true_for_overlap() {
        let set = ComponentTypeSet(vec![ComponentType::Kohai, ComponentType::Agent]);
        assert!(set.intersects(&[ComponentType::Agent]));
        assert!(!set.intersects(&[ComponentType::Sempai, ComponentType::Llm]));
    }
}
