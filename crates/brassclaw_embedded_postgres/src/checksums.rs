//! SHA-256 checksums for the PostgreSQL 16 binary archives downloaded by
//! `postgresql_embedded`. These are compiled into the binary so an attacker
//! who can set env vars (e.g. `POSTGRESQL_VERSION`) cannot swap in a different
//! archive version.
//!
//! # Archive source
//!
//! `postgresql_embedded` 0.21 downloads from the **theseus-rs** GitHub releases:
//! `https://github.com/theseus-rs/postgresql-binaries/releases/download/{version}/postgresql-{version}-{target}.tar.gz`
//!
//! Platform keys are **Rust target triples** (`target_triple::TARGET`), matching
//! the filenames published by theseus-rs.
//!
//! # Updating checksums
//!
//! When the pinned PostgreSQL version changes:
//! 1. Download the `.sha256` sidecar for each target from the theseus-rs release:
//!    `curl -sL https://github.com/theseus-rs/postgresql-binaries/releases/download/{ver}/postgresql-{ver}-{target}.tar.gz.sha256`
//! 2. Update every entry below and the `PG_VERSION` constant in the same commit.
//! 3. The PR description must include the download source URL and digest for each platform.

/// Pinned PostgreSQL release used for embedded deployments.
/// Matches the version string expected by `postgresql_embedded`.
pub const PG_VERSION: &str = "16.4.0";

/// Compiled-in checksums keyed by **Rust target triple** string.
///
/// Supported targets:
/// - `x86_64-unknown-linux-gnu`  (Linux amd64, glibc)
/// - `x86_64-unknown-linux-musl` (Linux amd64, musl)
/// - `aarch64-unknown-linux-gnu` (Linux arm64, glibc)
/// - `aarch64-unknown-linux-musl`(Linux arm64, musl)
/// - `aarch64-apple-darwin`      (macOS Apple Silicon)
/// - `x86_64-apple-darwin`       (macOS Intel)
/// - `x86_64-pc-windows-msvc`    (Windows amd64)
///
/// All digests are SHA-256 of `postgresql-{PG_VERSION}-{target}.tar.gz`
/// from `https://github.com/theseus-rs/postgresql-binaries/releases/download/{PG_VERSION}/`.
pub struct Checksums;

impl Checksums {
    /// Return the expected SHA-256 hex digest for the given Rust target triple.
    /// Returns `None` for unsupported targets, which causes `verify_archive` to
    /// return `UnsupportedPlatform` rather than silently skipping verification.
    pub fn for_platform(target: &str) -> Option<&'static str> {
        match target {
            // Linux amd64 — glibc
            // Source: postgresql-16.4.0-x86_64-unknown-linux-gnu.tar.gz.sha256
            "x86_64-unknown-linux-gnu" => {
                Some("1059350056c24e6dd3974af7582199c2a4d06078ecb2beb9f4b26b6debea6d37")
            }
            // Linux amd64 — musl
            // Source: postgresql-16.4.0-x86_64-unknown-linux-musl.tar.gz.sha256
            "x86_64-unknown-linux-musl" => {
                Some("7cc4ba47dd1ceb61876b59094ed7e09bf118e903ef23dbc8ff453f4a3d782f17")
            }
            // Linux arm64 — glibc
            // Source: postgresql-16.4.0-aarch64-unknown-linux-gnu.tar.gz.sha256
            "aarch64-unknown-linux-gnu" => {
                Some("62736e25b44c92ca8621987df9a8940ccbad158ad30dcbda105b4587fa5db2b3")
            }
            // Linux arm64 — musl
            // Source: postgresql-16.4.0-aarch64-unknown-linux-musl.tar.gz.sha256
            "aarch64-unknown-linux-musl" => {
                Some("9013ef13b7677693ecf69cf09d6e4e64789809b910c02517deb118e995cac2e1")
            }
            // macOS Apple Silicon (arm64)
            // Source: postgresql-16.4.0-aarch64-apple-darwin.tar.gz.sha256
            "aarch64-apple-darwin" => {
                Some("0ec91e77eff381e43e3963f012aff3acb9de12ad3739a625e57cce9671b28b0f")
            }
            // macOS Intel (x86_64)
            // Source: postgresql-16.4.0-x86_64-apple-darwin.tar.gz.sha256
            "x86_64-apple-darwin" => {
                Some("3193b9747c610139990c9913ff5fd5ad73cd38cefcd5ffcdc46079fd1479406e")
            }
            // Windows amd64
            // Source: postgresql-16.4.0-x86_64-pc-windows-msvc.tar.gz.sha256
            "x86_64-pc-windows-msvc" => {
                Some("e01be1f09d72f989f998845765786c779f78128eae0edb99380285838d34c447")
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every platform this codebase targets must have a compiled-in checksum so
    /// `verify_archive` never silently skips verification on a supported target.
    #[test]
    fn all_supported_targets_have_checksums() {
        let targets = [
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-gnu",
            "aarch64-unknown-linux-musl",
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ];
        for target in targets {
            assert!(
                Checksums::for_platform(target).is_some(),
                "missing checksum for target: {target}"
            );
        }
    }

    /// The current compilation target must have a checksum entry so
    /// `verify_archive` works on every build target we ship.
    #[test]
    fn current_target_has_checksum() {
        let current = ::target_triple::TARGET;
        assert!(
            Checksums::for_platform(current).is_some(),
            "missing checksum for current build target: {current}"
        );
    }

    /// Unknown targets must return None (not panic or return a wrong checksum).
    #[test]
    fn unknown_target_returns_none() {
        assert!(Checksums::for_platform("mips-unknown-linux-gnu").is_none());
        assert!(Checksums::for_platform("unknown").is_none());
        assert!(Checksums::for_platform("").is_none());
    }

    /// Checksums must be 64-character lowercase hex strings.
    #[test]
    fn all_checksums_are_valid_hex64() {
        let targets = [
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-gnu",
            "aarch64-unknown-linux-musl",
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ];
        for target in targets {
            let digest = Checksums::for_platform(target).unwrap();
            assert_eq!(
                digest.len(),
                64,
                "checksum for {target} is not 64 chars: {digest}"
            );
            assert!(
                digest
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f')),
                "checksum for {target} contains non-lowercase-hex chars: {digest}"
            );
        }
    }
}
