//! Engine bridge — connects `brassclaw_engine` to Reborn infrastructure.
//!
// V1 bridge modules (action_discovery, action_projector, capability_projector,
// effect_adapter, external_tools, gate_controller, router, tool_surface) were
// deleted as part of the Reborn migration. See git history for reference.

mod cost_guard_gate;
mod effect_adapter_v2;
mod engine_actions;
pub mod sandbox;
mod store_adapter;
mod workspace_reader;

pub use cost_guard_gate::CostGuardBudgetGate;
pub use workspace_reader::WorkspaceReaderAdapter;

pub use effect_adapter_v2::EffectBridgeAdapter as EffectBridgeAdapterV2; // alias kept for readability
pub use effect_adapter_v2::EffectBridgeAdapter;

// Re-export engine types needed by consumers
pub use brassclaw_engine::{EffectExecutor, ThreadExecutionContext};
