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
