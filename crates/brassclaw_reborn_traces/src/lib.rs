//! Trace Commons / TraceDAO client extracted from the BrassClaw monolith.
//!
//! This crate holds the trace contribution pipeline (`contribution`), the
//! host-facing trace client (`client`), the redaction helpers used to scrub
//! sensitive JSON before submission (`redaction`), and the shared
//! `ConversationMessage` type that the legacy monolith's `history` module now
//! re-exports for backward compatibility.

pub mod client;
pub mod contribution;
pub mod conversation_message;
pub mod redaction;

pub use conversation_message::ConversationMessage;

/// Recorded-trace deserialization surface for callers that load JSON traces
/// off disk (e.g. `brassclaw-reborn traces preview`). Re-exports from
/// `brassclaw_llm::recording` so reborn-cli does not need a direct
/// `brassclaw_llm` dependency, preserving the architectural boundary.
pub mod recording {
    pub use brassclaw_llm::recording::*;
}

/// Filesystem path resolution for trace-contribution storage. Re-exports
/// from `brassclaw_common::paths` so reborn-cli does not need a direct
/// `brassclaw_common` dependency.
pub mod paths {
    pub use brassclaw_common::paths::*;
}
