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
    // Delegate to the brassclaw-reborn CLI binary
    // This provides a seamless experience where `cargo run` works out of the box
    let args: Vec<String> = std::env::args().collect();
    
    // If no subcommand provided, show help
    if args.len() == 1 {
        eprintln!("BrassClaw V2 - Reborn Runtime");
        eprintln!();
        eprintln!("Usage: cargo run -- <COMMAND>");
        eprintln!();
        eprintln!("Commands:");
        eprintln!("  serve      Start the Reborn WebUI service (recommended)");
        eprintln!("  run        Start the CLI REPL");
        eprintln!("  repl       Start the composed Reborn CLI REPL");
        eprintln!("  doctor     Check Reborn binary configuration");
        eprintln!("  config     Inspect Reborn configuration paths");
        eprintln!("  models     Inspect Reborn model slots and route status");
        eprintln!("  skills     Inspect configured Reborn skills");
        eprintln!("  extension  Manage local Reborn extension lifecycle");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  cargo run -- serve              # Start WebUI server");
        eprintln!("  cargo run -- run                # Start CLI REPL");
        eprintln!("  cargo run -- doctor             # Check configuration");
        eprintln!();
        eprintln!("For more information, run: cargo run -- --help");
        process::exit(1);
    }
    
    // Forward to brassclaw_reborn_cli
    // This is implemented as a library that we can call directly
    match brassclaw_reborn_cli::run() {
        Ok(()) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {:#}", e);
            process::exit(1);
        }
    }
}
