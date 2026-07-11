//! CLI utilities remaining in the v1 compatibility crate.
//!
//! All runtime subcommands are handled by `brassclaw_reborn_cli`.
//! This module retains only `fmt` (formatting helpers) and the
//! feature-gated `import` command used by integration tests.

pub mod fmt;
#[cfg(feature = "import")]
pub mod import;

#[cfg(feature = "import")]
pub use import::{ImportCommand, run_import_command};
