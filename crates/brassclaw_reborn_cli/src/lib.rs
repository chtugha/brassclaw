//! BrassClaw Reborn CLI Library
//!
//! This crate provides the standalone BrassClaw Reborn runtime as both
//! a binary (`brassclaw-reborn`) and a library that can be called from
//! the main `brassclaw` binary.

mod cli;
mod commands;
mod context;
mod runtime;

/// Run the BrassClaw Reborn CLI with arguments from std::env::args().
///
/// This is the main entry point that can be called from other binaries.
/// It handles command parsing, execution, and error reporting.
pub fn run() -> anyhow::Result<()> {
    // Mirror the v1 binary's behavior so dev workflows can keep LLM
    // keys / base URLs in `.env`. Silent on missing file — production
    // hosts use shell-exported env or systemd unit env, not `.env` —
    // but any other error (parse failure, permission denied) is
    // surfaced to stderr so a malformed file does not boot the host
    // with stale env. The boot itself still proceeds because
    // operators may have already exported the same keys in their
    // shell.
    if let Err(error) = dotenvy::dotenv()
        && !error.not_found()
    {
        eprintln!("warning: failed to load .env: {error}");
    }
    cli::run()
}

/// Run the `serve` subcommand directly, bypassing argv.
///
/// Called from the main `brassclaw` binary when legacy flags like
/// `--no-onboard` are detected with no recognised reborn subcommand, so
/// the gateway E2E tests keep working after the v1→v2 migration.
pub fn run_serve() -> anyhow::Result<()> {
    if let Err(error) = dotenvy::dotenv()
        && !error.not_found()
    {
        eprintln!("warning: failed to load .env: {error}");
    }
    // Parse with a synthetic argv of ["brassclaw", "serve"] so clap
    // constructs a default ServeCommand with no extra flags.
    use clap::Parser as _;
    cli::Cli::parse_from(["brassclaw", "serve"])
        .command
        .execute()
}

// Made with Bob
