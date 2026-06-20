//! Shared filesystem path helpers.
//!
//! `brassclaw_base_dir()` resolves the BrassClaw base directory used for env
//! files, session tokens, the libsql database, and other per-instance state.
//! Override with the `BRASSCLAW_BASE_DIR` environment variable; defaults to
//! `~/.brassclaw`.

use std::path::PathBuf;
use std::sync::LazyLock;

const BRASSCLAW_BASE_DIR_ENV: &str = "BRASSCLAW_BASE_DIR";

static BRASSCLAW_BASE_DIR: LazyLock<PathBuf> = LazyLock::new(compute_brassclaw_base_dir);

/// Compute the BrassClaw base directory from the environment.
///
/// Bypasses the `LazyLock` cache. Use this in tests that mutate
/// `BRASSCLAW_BASE_DIR`; production callers should use [`brassclaw_base_dir`].
pub fn compute_brassclaw_base_dir() -> PathBuf {
    std::env::var(BRASSCLAW_BASE_DIR_ENV)
        .map(PathBuf::from)
        .map(|path| {
            if path.as_os_str().is_empty() {
                default_base_dir()
            } else if !path.is_absolute() {
                eprintln!(
                    "Warning: BRASSCLAW_BASE_DIR is a relative path '{}', resolved against current directory",
                    path.display()
                );
                path
            } else {
                path
            }
        })
        .unwrap_or_else(|_| default_base_dir())
}

fn default_base_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".brassclaw")
    } else {
        eprintln!("Warning: Could not determine home directory, using current directory");
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join(".brassclaw")
    }
}

/// Get the BrassClaw base directory.
///
/// Override with `BRASSCLAW_BASE_DIR`. Defaults to `~/.brassclaw` (or
/// `./.brassclaw` if the home directory cannot be determined).
///
/// Thread-safe: the value is computed once and cached in a `LazyLock`.
pub fn brassclaw_base_dir() -> PathBuf {
    BRASSCLAW_BASE_DIR.clone()
}
