/// Compile-time-embedded PostgreSQL archive extraction.
///
/// The `build.rs` downloads the platform-specific Postgres tarball at compile
/// time and emits `EMBEDDED_PG_ARCHIVE` pointing at the file.  `include_bytes!`
/// bakes those bytes directly into the binary, eliminating any runtime network
/// access for Postgres binaries.
///
/// `ensure_pg_extracted` is the replacement for `postgresql_embedded`'s
/// `PostgreSQL::setup()` download step.  It checks whether the bin cache dir
/// already contains a valid `initdb` binary; if not it extracts from the
/// embedded bytes.
use std::path::Path;

use flate2::read::GzDecoder;
use tracing::debug;

use crate::error::EmbeddedPostgresError;

/// The Postgres binary archive, embedded at compile time by `build.rs`.
/// On platforms where `build.rs` wrote a non-empty archive the bytes are the
/// real tarball.  On unsupported targets / no-postgres feature builds the
/// dummy empty file is embedded (extraction is never called in that case
/// because `start()` will return an UnsupportedPlatform error first).
static EMBEDDED_PG_ARCHIVE: &[u8] = include_bytes!(env!("EMBEDDED_PG_ARCHIVE"));

/// Ensure the PostgreSQL binaries are extracted into `bin_cache_dir`.
///
/// If `initdb` already exists in `bin_cache_dir/bin/initdb` (or the
/// Windows equivalent) this is a no-op — the cache is valid.
///
/// Otherwise the embedded tarball is extracted into `bin_cache_dir`.
/// The tarball layout from theseus-rs is a single top-level directory
/// `postgresql-{version}-{target}/` whose contents map directly to the
/// installation root (`bin/`, `lib/`, `share/`, …).
pub async fn ensure_pg_extracted(bin_cache_dir: &Path) -> Result<(), EmbeddedPostgresError> {
    let initdb_path = if cfg!(windows) {
        bin_cache_dir.join("bin").join("initdb.exe")
    } else {
        bin_cache_dir.join("bin").join("initdb")
    };

    if initdb_path.exists() {
        debug!(
            path = %initdb_path.display(),
            "PostgreSQL binaries already present; skipping extraction"
        );
        return Ok(());
    }

    if EMBEDDED_PG_ARCHIVE.is_empty() {
        return Err(EmbeddedPostgresError::UnsupportedPlatform(
            ::target_triple::TARGET.to_string(),
        ));
    }

    debug!(
        dest = %bin_cache_dir.display(),
        archive_bytes = EMBEDDED_PG_ARCHIVE.len(),
        "extracting embedded PostgreSQL archive"
    );

    tokio::fs::create_dir_all(bin_cache_dir)
        .await
        .map_err(|e| EmbeddedPostgresError::Io {
            path: bin_cache_dir.display().to_string(),
            reason: e.to_string(),
        })?;

    // Extraction is CPU-bound; run on the blocking thread pool so we don't
    // stall the async executor.
    let dest = bin_cache_dir.to_path_buf();
    tokio::task::spawn_blocking(move || extract_tarball(EMBEDDED_PG_ARCHIVE, &dest))
        .await
        .map_err(|e| EmbeddedPostgresError::InitDb(format!("extraction task panicked: {e}")))?
}

/// Extract a gzip-compressed tar archive from `bytes` into `dest`.
///
/// The theseus-rs tarballs have a single top-level directory component
/// (`postgresql-{version}-{target}/`).  We strip it so the contents land
/// directly in `dest` (i.e. `bin/initdb`, `lib/libpq.so`, etc.).
fn extract_tarball(bytes: &[u8], dest: &Path) -> Result<(), EmbeddedPostgresError> {
    let gz = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);

    for entry in archive
        .entries()
        .map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?;
        let entry_path = entry
            .path()
            .map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?
            .into_owned();
        // entry.path() borrows from entry header; re-bind to owned value above.

        // Strip the first path component (the top-level directory in the tarball).
        let mut components = entry_path.components();
        components.next(); // discard top-level dir
        let relative: std::path::PathBuf = components.collect();
        if relative.as_os_str().is_empty() {
            continue; // skip the top-level dir entry itself
        }

        let out_path = dest.join(&relative);

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| EmbeddedPostgresError::Io {
                path: out_path.display().to_string(),
                reason: e.to_string(),
            })?;
        } else {
            // Ensure parent directory exists.
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| EmbeddedPostgresError::Io {
                    path: parent.display().to_string(),
                    reason: e.to_string(),
                })?;
            }
            // Capture the mode before consuming entry via copy.
            #[cfg(unix)]
            let mode = entry.header().mode().ok();

            let mut out_file =
                std::fs::File::create(&out_path).map_err(|e| EmbeddedPostgresError::Io {
                    path: out_path.display().to_string(),
                    reason: e.to_string(),
                })?;
            std::io::copy(&mut entry, &mut out_file).map_err(|e| EmbeddedPostgresError::Io {
                path: out_path.display().to_string(),
                reason: e.to_string(),
            })?;

            // Preserve executable bit on Unix.
            #[cfg(unix)]
            if let Some(mode) = mode {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(mode);
                let _ = std::fs::set_permissions(&out_path, perms);
            }
        }
    }

    debug!(dest = %dest.display(), "PostgreSQL archive extracted successfully");
    Ok(())
}
