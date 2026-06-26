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

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Handle legacy subcommands that are implemented in the brassclaw lib crate
    // but not (yet) in brassclaw_reborn_cli.
    if args.get(1).map(String::as_str) == Some("status") {
        match brassclaw::cli::run_status_command().await {
            Ok(()) => process::exit(0),
            Err(e) => {
                eprintln!("Error: {:#}", e);
                process::exit(1);
            }
        }
    }

    // Forward everything else to the Reborn CLI.
    match brassclaw_reborn_cli::run() {
        Ok(()) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {:#}", e);
            process::exit(1);
        }
    }
}
