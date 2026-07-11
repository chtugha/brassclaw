//! BrassClaw — shared library for the legacy compatibility layer.
//!
//! This crate provides the persistence, workspace, config, and import
//! infrastructure shared between the v1 compatibility layer and the Reborn
//! runtime. The v1 agentic loop, channels, and CLI commands have been removed;
//! all runtime functionality now lives in `crates/brassclaw_reborn_*`.

pub mod bootstrap;
pub mod bridge;

pub mod cli;
pub mod code_challenge;
pub mod config;
pub mod db;
pub mod document_extraction;
pub mod error;
pub(crate) mod generated_images;
#[cfg(feature = "import")]
pub mod import;
pub mod logging;
pub mod safety;
pub mod secrets;
pub mod settings;
pub mod timezone;
pub mod tracing_fmt;
pub mod util;

pub mod workspace;

#[cfg(test)]
pub mod testing;

pub use config::Config;
pub use error::{Error, Result};

/// Re-export commonly used types.
pub mod prelude {
    pub use crate::config::Config;
    pub use crate::error::{Error, Result};
    pub use crate::workspace::{MemoryDocument, Workspace};
    pub use brassclaw_llm::LlmProvider;
    pub use brassclaw_safety::{SanitizedOutput, Sanitizer};
}
