//! Instruction-Building-System (IBS) **data-model** types.
//!
//! This module is the sole home for the IBS data-model types that are persisted
//! in JSONB and referenced by `RecipeVariant` (Decision 1 / FIND-P6-03 /
//! FIND-NEW-01). It is a sibling of [`super::recipe`] and imports only `serde`
//! — there is NO dependency back to `crate::memory::` or `crate::types::recipe`,
//! so the `memory -> types` direction stays clean and cycle-free.
//!
//! The IBS **builder / output** types live in
//! [`crate::memory::instruction_builder`], which imports from this module via
//! `crate::types::ibs`.

use serde::{Deserialize, Serialize};

/// Slot-variable refinement rule stored on a `RecipeVariant`.
///
/// Persisted in the `variants` JSONB column of `reborn_recipes` (§0.4.1,
/// §0.17.3). Empty = positional auto-extraction only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariablePattern {
    /// Slot name — e.g. "dir", "filename". Matches `{{vars.NAME}}` expressions.
    pub name: String,
    /// Optional regex applied after positional extraction to validate/transform
    /// the value.
    pub pattern: Option<String>,
    /// Human description of the slot's expected value (WebUI help text only).
    pub description: Option<String>,
}

/// Error-handling policy for a single [`ToolBinding`].
///
/// Persisted nested inside [`ToolBinding`] in rust-channel `IbsRecipeStep`
/// `tool_bindings` (§0.4.1).
///
/// Canonical definition (FIND-AUDIT-11): matches §0.4.1 exactly. Do NOT use
/// `Propagate` / `Retry { max_attempts: u8 }` / `Fallback { message }` — that
/// was an earlier wrong draft.
///
/// The canonical §0.4.1 block writes the `Default` impl by hand
/// (`impl Default for ErrorPolicy { fn default() -> Self { ErrorPolicy::Fail } }`);
/// the implementation uses `#[derive(Default)]` + `#[default]` on `Fail` instead
/// — semantically identical, and clippy-clean under `derivable_impls` which the
/// repo's zero-warning rule mandates.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum ErrorPolicy {
    /// Fail the turn immediately — hard error, no retry.
    #[default]
    Fail,
    /// Ignore the error and continue — the orchestrator receives an empty result.
    Ignore,
    /// Retry up to `max_attempts` times before falling through to [`Fail`].
    Retry { max_attempts: u32 },
    /// On error, jump to the step with id `step_id` within the same
    /// `BuildInstruction`.
    Fallback { step_id: String },
}

/// Binding from a rust-channel IBS step to a specific tool invocation.
///
/// Persisted nested inside rust-channel `IbsRecipeStep` `tool_bindings`
/// (§0.4.1, FIND-IBS-05: authored on `StepEntry` and passed through by the IBS).
///
/// Canonical definition (FIND-AUDIT-10): matches §0.4.1 exactly. Do NOT omit
/// `tool_name` or `params` — both are required for runtime `__execute_action__`
/// dispatch and `{{vars.name}}` substitution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolBinding {
    /// UUID of the Tool (class 0) row — used by the Rust execution layer for
    /// capability dispatch.
    pub tool_id: uuid::Uuid,
    /// Denormalized tool name (e.g. "read_file"). Needed for `__execute_action__`
    /// calls without an extra DB fetch. Must match the registered capability name.
    pub tool_name: String,
    /// Parameter values for this tool call. `{{vars.name}}` substitution applied
    /// before use.
    pub params: serde_json::Value,
    /// How to handle a tool invocation error.
    #[serde(default)]
    pub error_policy: ErrorPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_policy_default_is_fail() {
        assert_eq!(ErrorPolicy::default(), ErrorPolicy::Fail);
    }

    #[test]
    fn error_policy_serde_roundtrips_all_variants() {
        let cases = vec![
            ErrorPolicy::Fail,
            ErrorPolicy::Ignore,
            ErrorPolicy::Retry { max_attempts: 3 },
            ErrorPolicy::Fallback {
                step_id: "fallback-1".into(),
            },
        ];
        for p in cases {
            let v = serde_json::to_value(&p).unwrap();
            let back: ErrorPolicy = serde_json::from_value(v).unwrap();
            assert_eq!(p, back);
        }
    }

    #[test]
    fn error_policy_uses_tagged_policy_field() {
        let retry = ErrorPolicy::Retry { max_attempts: 5 };
        let v = serde_json::to_value(&retry).unwrap();
        assert_eq!(v["policy"], "retry");
        assert_eq!(v["max_attempts"], 5);

        let fail = ErrorPolicy::Fail;
        let v = serde_json::to_value(&fail).unwrap();
        assert_eq!(v["policy"], "fail");
    }

    #[test]
    fn tool_binding_serde_roundtrips() {
        let binding = ToolBinding {
            tool_id: uuid::Uuid::nil(),
            tool_name: "ls".into(),
            params: serde_json::json!({ "flags": "-la", "dir": "{{vars.dir}}" }),
            error_policy: ErrorPolicy::Retry { max_attempts: 2 },
        };
        let v = serde_json::to_value(&binding).unwrap();
        let back: ToolBinding = serde_json::from_value(v).unwrap();
        assert_eq!(binding, back);
    }

    #[test]
    fn variable_pattern_serde_roundtrips() {
        let vp = VariablePattern {
            name: "dir".into(),
            pattern: Some(r"in (?P<dir>[/\w.-]+)".into()),
            description: Some("target directory".into()),
        };
        let v = serde_json::to_value(&vp).unwrap();
        let back: VariablePattern = serde_json::from_value(v).unwrap();
        assert_eq!(vp, back);
    }
}
