//! BrassClaw V2 Main Binary
//!
//! This is the default entry point for `cargo run`. It delegates to the
//! brassclaw-reborn CLI implementation which provides the full V2 runtime
//! with automatic database migrations.
//!
//! The V2 system uses the Reborn architecture with:
//! - Automatic database migrations via Refinery
//! - Event-sourced state management
//! - Product adapter system for multi-channel support
//! - Capability-based security model

use std::process;

/// Known reborn subcommands. When the first arg is one of these, pass
/// straight through to the reborn CLI without any compat translation.
const REBORN_SUBCOMMANDS: &[&str] = &[
    "serve",
    "run",
    "onboard",
    "config",
    "registry",
    "channels",
    "routines",
    "mcp",
    "memory",
    "pairing",
    "profile",
    "service",
    "skills",
    "hooks",
    "models",
    "doctor",
    "logs",
    "status",
    "completion",
    "import",
    "login",
    "acp",
    "help",
    "--help",
    "-h",
    "--version",
    "-V",
];

/// Legacy top-level flags that the v1 binary accepted when running as a server.
/// Strip these when translating a bare invocation into `brassclaw serve`.
const LEGACY_SERVER_FLAGS: &[&str] = &["--no-onboard", "--cli-only", "--no-db", "--auto-approve"];

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Handle legacy subcommands that are implemented in the brassclaw lib crate
    // but not (yet) in brassclaw_reborn_cli.
    if args.get(1).map(String::as_str) == Some("status") {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for status command")
            .block_on(brassclaw::cli::run_status_command());
        match result {
            Ok(()) => process::exit(0),
            Err(e) => {
                eprintln!("Error: {:#}", e);
                process::exit(1);
            }
        }
    }

    // Compat shim: translate a bare `brassclaw --no-onboard` (or similar
    // legacy top-level flags with no subcommand) into `brassclaw serve`.
    // This preserves E2E test fixtures that were written against the v1
    // binary interface where the gateway started by default.
    let first_non_flag = args[1..].iter().find(|a| !a.starts_with('-'));
    let is_reborn_subcommand = first_non_flag
        .map(|a| REBORN_SUBCOMMANDS.contains(&a.as_str()))
        .unwrap_or(false);
    let has_legacy_server_flags = args[1..]
        .iter()
        .any(|a| LEGACY_SERVER_FLAGS.contains(&a.as_str()));

    if !is_reborn_subcommand && has_legacy_server_flags {
        // Legacy invocation like `brassclaw --no-onboard`: delegate directly
        // to the reborn serve path without any argv rewriting.
        match brassclaw_reborn_cli::run_serve() {
            Ok(()) => process::exit(0),
            Err(e) => {
                eprintln!("Error: {:#}", e);
                process::exit(1);
            }
        }
    }

    // Forward everything else to the Reborn CLI.
    // brassclaw_reborn_cli::run() is synchronous — it builds its own
    // multi-thread runtime internally. It must NOT be called from inside
    // an existing tokio runtime (that would panic with "Cannot start a
    // runtime from within a runtime").
    match brassclaw_reborn_cli::run() {
        Ok(()) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {:#}", e);
            process::exit(1);
        }
    }
}
