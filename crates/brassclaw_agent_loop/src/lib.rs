//! Agent-loop framework state and strategy contracts for BrassClaw Reborn.
//!
//! This crate owns the framework layer above `brassclaw_turns`.

mod default_planner;
pub mod executor;
pub mod families;
pub mod family;
pub mod planner;
pub mod state;
pub mod strategies;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod token_budget;

/// Public re-exports for progress-detection primitives. The internal
/// strategy machinery stays crate-private; downstream consumers (turns,
/// reborn) only need the typed [`ParamHash`](progress::ParamHash) for
/// loop-stuck detection.
pub mod progress {
    pub use crate::strategies::progress::ParamHash;
}

pub use planner::AgentLoopPlanner;
pub use strategies::{
    PublicFocusedCapabilityStrategy as FocusedCapabilityStrategy,
    PublicCapabilityFocusConfig as CapabilityFocusConfig,
};
