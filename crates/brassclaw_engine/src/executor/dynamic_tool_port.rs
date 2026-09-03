//! Step C.3 — port trait bridging the engine orchestrator's dispatch
//! fallthrough to the Executioner's dynamic cdylib Tool loader.
//!
//! `brassclaw_engine` cannot depend on `brassclaw_host_runtime` (the host
//! runtime is a system-service crate downstream of the engine), so the engine
//! defines this port trait and the composition layer (C.5/C.6) implements it
//! over `brassclaw_host_runtime::DynamicToolLoader`. This mirrors the
//! [`crate::memory::RetrievalSource`] engine↔composition port precedent.
//!
//! # Two Tool Systems
//!
//! Built-in Tools are precompiled into the binary and dispatched by the engine
//! orchestrator's static `match call.function_name` (the C.1 arms). Dynamic
//! Tools — kohai/sempai-minted Tools+ToolSkills shipped as separate `cdylib`
//! crates — are dlopen'd at runtime on demand by `DynamicToolLoader` and bound
//! into the `host` namespace. When a `host.<name>(...)` call is not in the
//! static match, the dispatch fallthrough consults this port: if `is_loaded` is
//! true it routes the call through `invoke` (JSON-in/JSON-out); the returned
//! [`serde_json::Value`] is converted back to a Monty object for the
//! orchestrator. The composition layer wraps the (Send-but-not-Sync) loader in
//! a `std::sync::Mutex` so the `Send + Sync` bound holds.

use serde_json::Value;
use thiserror::Error;

/// Errors raised by a [`DynamicToolPort`] implementation.
#[derive(Debug, Clone, Error)]
pub enum DynamicToolPortError {
    /// The named dynamic Tool is not currently loaded into the `host` namespace.
    #[error("dynamic tool '{tool}' is not loaded")]
    NotLoaded { tool: String },
    /// A loaded dynamic Tool was invoked but returned an error (cdylib failure,
    /// bad response, etc.).
    #[error("dynamic tool '{tool}' failed: {reason}")]
    Invoke { tool: String, reason: String },
}

/// Engine-side port over the Executioner's dynamic cdylib Tool registry.
///
/// The implementation lives in the composition layer (C.5/C.6), delegating to
/// `brassclaw_host_runtime::DynamicToolLoader`. Until that wiring lands the
/// orchestrator passes `None` and the dispatch fallthrough is dormant.
pub trait DynamicToolPort: Send + Sync {
    /// Whether a dynamic cdylib Tool is currently loaded under `tool_name`. The
    /// engine orchestrator's dispatch fallthrough consults this before routing a
    /// `host.<name>` call.
    fn is_loaded(&self, tool_name: &str) -> bool;

    /// Invoke a loaded dynamic cdylib Tool by name with JSON `args`, returning
    /// the JSON `result` the cdylib produced.
    fn invoke(&self, tool_name: &str, args: Value) -> Result<Value, DynamicToolPortError>;
}
