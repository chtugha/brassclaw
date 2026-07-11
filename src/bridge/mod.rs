//! Engine bridge — connects `brassclaw_engine` to Reborn infrastructure.
//!
// V1 bridge modules (effect_adapter_v2, engine_actions, cost_guard_gate,
// sandbox) were deleted as part of the v1 removal. See git history.

mod store_adapter;
mod workspace_reader;

pub use workspace_reader::WorkspaceReaderAdapter;

// Re-export engine types needed by consumers
pub use brassclaw_engine::{EffectExecutor, ThreadExecutionContext};
