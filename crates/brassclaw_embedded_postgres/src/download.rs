use std::path::Path;

use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::debug;

use crate::checksums::{Checksums, PG_VERSION};
use crate::error::EmbeddedPostgresError;

/// Error returned when checksum verification fails.
#[derive(Debug, Error)]
#[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
pub struct ChecksumMismatch {
    pub path: String,
    pub expected: String,
    pub actual: String,
}

/// Return the Rust target triple for the current build target.
///
/// This matches the filename component used by theseus-rs:
/// `postgresql-{version}-{target}.tar.gz`
fn platform_key() -> &'static str {
    // Use the same `TARGET` constant that `postgresql_archive` uses to select
    // the archive, so our platform key always agrees with the downloaded filename.
    // E.g. "aarch64-apple-darwin", "x86_64-unknown-linux-gnu", etc.
    ::target_triple::TARGET
}

/// Verify the SHA-256 checksum of a downloaded archive against the compiled-in
/// expected value. Returns `Ok(())` if the digest matches, or an error
/// describing the mismatch. On mismatch the caller must delete the archive.
pub fn verify_archive(archive_path: &Path) -> Result<(), EmbeddedPostgresError> {
    let platform = platform_key();
    let expected = Checksums::for_platform(platform)
        .ok_or_else(|| EmbeddedPostgresError::UnsupportedPlatform(platform.to_string()))?;

    let data = std::fs::read(archive_path).map_err(|e| EmbeddedPostgresError::Io {
        path: archive_path.display().to_string(),
        reason: e.to_string(),
    })?;

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let digest = hex::encode(hasher.finalize());

    if digest != expected {
        // Delete the corrupt archive before returning the error so a retry
        // attempt does not reuse the bad file.
        let _ = std::fs::remove_file(archive_path);
        return Err(EmbeddedPostgresError::ChecksumMismatch {
            path: archive_path.display().to_string(),
            expected: expected.to_string(),
            actual: digest,
        });
    }

    debug!(
        pg_version = PG_VERSION,
        platform, digest, "archive checksum verified"
    );
    Ok(())
}

/// Suppress the `POSTGRESQL_VERSION` and `GITHUB_TOKEN` environment variables
/// that `postgresql_embedded` reads by default. This prevents an attacker who
/// can inject env vars from changing the downloaded Postgres version or
/// authenticating as the service's GitHub identity.
///
/// Must be called before `postgresql_embedded` is initialised.
pub fn suppress_postgresql_embedded_env() {
    // SAFETY: Called at startup before any other threads are spawned.
    // We deliberately remove env vars that could allow a version-substitution
    // attack via environment injection (`POSTGRESQL_VERSION`, `GITHUB_TOKEN`
    // are read by `postgresql_embedded` to pick the download target).
    unsafe {
        std::env::remove_var("POSTGRESQL_VERSION");
        std::env::remove_var("GITHUB_TOKEN");
    }
}
