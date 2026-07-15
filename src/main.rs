//! BrassClaw binary entrypoint.
//!
//! Thin compat shim over `brassclaw_reborn_cli`. All subcommands — including
//! `status`, `serve`, `run`, `onboard`, etc. — are implemented in
//! `brassclaw_reborn_cli`.
//!
//! Phase 6 removed the v1 root source tree; this file survives only to
//! carry the legacy `--no-onboard` / `--cli-only` / `--no-db` /
//! `--auto-approve` shim that E2E fixtures still rely on.

use std::process;

const LEGACY_SERVER_FLAGS: &[&str] = &["--no-onboard", "--cli-only", "--no-db", "--auto-approve"];

fn main() {
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

    match brassclaw_reborn_cli::run() {
        Ok(()) => process::exit(0),
        Err(e) => {
            eprintln!("Error: {:#}", e);
            process::exit(1);
        }
    }
}
