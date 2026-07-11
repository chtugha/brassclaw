//! BrassClaw Main Binary
//!
//! Delegates entirely to the Reborn CLI. All subcommands — including
//! `status`, `serve`, `run`, `onboard`, etc. — are implemented in
//! `brassclaw_reborn_cli`.

use std::process;

/// Legacy top-level flags that the v1 binary accepted when running as a server.
/// Translate a bare invocation that only carries these flags into `brassclaw serve`.
const LEGACY_SERVER_FLAGS: &[&str] = &["--no-onboard", "--cli-only", "--no-db", "--auto-approve"];

fn main() {
    // Compat shim: translate `brassclaw --no-onboard` (and similar bare
    // legacy flags with no subcommand) into `brassclaw serve`, preserving
    // E2E test fixtures written against the old v1 invocation shape.
    let args: Vec<String> = std::env::args().collect();
    let first_non_flag = args[1..].iter().find(|a| !a.starts_with('-'));
    let has_legacy_server_flags = args[1..]
        .iter()
        .any(|a| LEGACY_SERVER_FLAGS.contains(&a.as_str()));

    if first_non_flag.is_none() && has_legacy_server_flags {
        match brassclaw_reborn_cli::run_serve() {
            Ok(()) => process::exit(0),
            Err(e) => {
                eprintln!("Error: {:#}", e);
                process::exit(1);
            }
        }
    }

    // Forward everything to the Reborn CLI.
    // brassclaw_reborn_cli::run() is synchronous — it builds its own
    // multi-thread runtime internally.
    match brassclaw_reborn_cli::run() {
        Ok(()) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {:#}", e);
            process::exit(1);
        }
    }
}
