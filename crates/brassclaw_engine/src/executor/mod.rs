//! Step execution.
//!
//! - [`ExecutionLoop`] — core loop replacing `run_agentic_loop()`
//! - [`structured`] — Tier 0 action execution (structured tool calls)
//! - [`context`] — context building for LLM calls
//! - [`intent`] — tool intent nudge detection

pub mod code_audit;
pub mod composition_port;
pub mod context;
#[cfg(feature = "skills-db")]
pub mod db_skill_loader;
pub mod dynamic_tool_port;
pub mod kohai_port;
pub mod loop_engine;
pub mod orchestrator;
pub mod prompt;
pub mod scripting;
pub mod structured;
pub(crate) mod thread_context;
pub mod tier_zero_orchestrator;
pub mod trace;

pub use composition_port::{ComponentPort, ComponentPortError};
pub use dynamic_tool_port::{DynamicToolPort, DynamicToolPortError};
pub use kohai_port::{KohaiPort, KohaiPortError};
pub use loop_engine::ExecutionLoop;
pub use orchestrator::{
    PkrAssemblyResult, TierZeroChannelResult, assemble_prior_knowledge_with_hint,
    execute_tier_zero_channel,
};
pub use scripting::{run_python_code_body, validate_python_syntax};
pub use tier_zero_orchestrator::{TierZeroOrchestrator, TierZeroOrchestratorBuilder};
