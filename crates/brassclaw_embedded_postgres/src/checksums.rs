//! SHA-256 checksums for the PostgreSQL 16 binary archives downloaded by
//! `postgresql_embedded`. These are compiled into the binary so an attacker
//! who can set env vars (e.g. `POSTGRESQL_VERSION`) cannot swap in a different
//! archive version.
//!
//! # Updating checksums
//!
//! When the pinned PostgreSQL version changes, download the new archives for
//! all supported targets and compute their digests, update all entries below
//! and the `PG_VERSION` constant in `download.rs` in the same commit. The PR
//! description must include the download source URL and digest for each platform.

/// Pinned PostgreSQL 16 release used for embedded deployments.
/// Matches the version string expected by `postgresql_embedded`.
pub const PG_VERSION: &str = "16.4.0";

/// Compiled-in checksums keyed by `<os>-<arch>` platform string.
///
/// Platform keys follow the convention used by `postgresql_embedded`:
/// - `linux-x86_64`
/// - `darwin-aarch64`
/// - `darwin-x86_64`
pub struct Checksums;

impl Checksums {
    /// Return the expected SHA-256 hex digest for the given platform string.
    /// Returns `None` for unsupported platforms.
    pub fn for_platform(platform: &str) -> Option<&'static str> {
        match platform {
            "linux-x86_64" => {
                // sha256 of postgresql-16.4.0-1-linux-x64-binaries.tar.gz
                // Source: https://get.enterprisedb.com/postgresql/
                Some("d6a3c1c5db8867dbcb9ebd1ecabd8e03e0ad4e1c49e4ea59cde1c18a7a17a0bc")
            }
            "darwin-aarch64" => {
                // TODO: populate with the real SHA-256 of postgresql-16.4.0-1-osx-binaries.zip
                // (ARM64) before shipping macOS builds. Download from get.enterprisedb.com and
                // run: shasum -a 256 postgresql-16.4.0-1-osx-binaries.zip
                // Returning None causes build_pool to fail with UnsupportedPlatform rather than
                // silently accepting or rejecting archives with a placeholder digest.
                None
            }
            "darwin-x86_64" => {
                // TODO: populate with the real SHA-256 of postgresql-16.4.0-1-osx-binaries.zip
                // (x86_64) before shipping macOS builds. See darwin-aarch64 note above.
                None
            }
            _ => None,
        }
    }
}
