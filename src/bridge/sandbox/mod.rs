//! Engine v2 per-project sandbox bridge.
//!
//! This submodule is the **host-side glue** between [`EffectBridgeAdapter`]
//! and the engine's [`WorkspaceMounts`] abstraction. It is the place where
//! the bridge decides — for any given tool call — whether to dispatch into
//! a sandbox backend (filesystem or, eventually, containerized) or fall
//! through to direct host tool execution.
//!
//! See `docs/plans/2026-03-20-engine-v2-architecture.md` (Phase 8) and
//! `docs/plans/2026-04-10-engine-v2-sandbox.md` for the full design rationale,
//! including the cross-reference with chtugha/brassclaw#1894.
//!
//! # Scope
//!
//! - [`maybe_intercept`] — given an action name, params, and a project's
//!   mount table, decide whether to handle the call via the mount backend
//!   and produce a `Result<String>` matching what `execute_tool_with_safety`
//!   would return. Handles `file_read`, `file_write`, `list_dir`,
//!   `apply_patch`, and `shell` for paths under `/project/`.
//!
//! - The five sandbox tool names live in [`SANDBOX_TOOL_NAMES`].
//!
//! - [`ProjectSandboxManager`] manages per-project Docker containers and
//!   their lifecycle. [`DockerTransport`] speaks NDJSON to the in-container
//!   `sandbox_daemon`.
//!
//! [`EffectBridgeAdapter`]: super::EffectBridgeAdapter
//! [`WorkspaceMounts`]: brassclaw_engine::WorkspaceMounts

pub mod protocol;
mod transport;
pub mod workspace_path;

/// Returns whether the per-project sandbox is enabled.
///
/// Reads `SANDBOX_ENABLED` env var.
pub fn sandbox_enabled() -> bool {
    is_truthy(std::env::var("SANDBOX_ENABLED").ok().as_deref())
}

fn is_truthy(value: Option<&str>) -> bool {
    value
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

#[cfg(test)]
mod env_tests {
    use super::is_truthy;

    #[test]
    fn truthy_values() {
        for v in [
            "1", "true", "TRUE", "True", "yes", "Yes", "YES", "on", "ON", "On",
        ] {
            assert!(
                is_truthy(Some(v)),
                "SANDBOX_ENABLED='{v}' should enable sandbox"
            );
        }
    }

    #[test]
    fn falsy_or_unset_disables() {
        for v in [
            None,
            Some(""),
            Some("0"),
            Some("false"),
            Some("no"),
            Some("off"),
        ] {
            assert!(!is_truthy(v), "expected {v:?} to disable sandbox");
        }
    }
}
