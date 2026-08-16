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
static EMBEDDED_PG_ARCHIVE: &[u8] = include_bytes!(env!("EMBEDDED_PG_ARCHIVE"));

/// The pgvector extension files archive, embedded at compile time by `build.rs`.
/// Contains `lib/vector.so` (or `.dylib`), `share/extension/vector.control`,
/// and `share/extension/vector--*.sql`. Empty on unsupported targets.
static EMBEDDED_PGVECTOR_ARCHIVE: &[u8] = include_bytes!(env!("EMBEDDED_PGVECTOR_ARCHIVE"));

/// Ensure the PostgreSQL binaries are extracted into `bin_cache_dir`.
///
/// If `initdb` already exists in `bin_cache_dir/bin/initdb` (or the
/// Windows equivalent) this is a no-op — the cache is valid.
///
/// Otherwise the embedded tarball is extracted into `bin_cache_dir`.
/// The tarball layout from theseus-rs is:
///   `postgresql-{version}-{target}/bin/initdb`
///   `postgresql-{version}-{target}/lib/libpq.so.5`
///   …
/// We strip the top-level directory so files land directly in `bin_cache_dir`:
///   `bin_cache_dir/bin/initdb`
///   `bin_cache_dir/lib/libpq.so.5`
///   …
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
    tokio::task::spawn_blocking(move || {
        extract_tarball(EMBEDDED_PG_ARCHIVE, &dest)?;
        // Install pgvector files if the archive is non-empty (Linux/macOS).
        if !EMBEDDED_PGVECTOR_ARCHIVE.is_empty() {
            // pgvector archive has NO top-level directory to strip — files are
            // directly `lib/vector.so`, `share/extension/vector.control`, etc.
            extract_flat_tarball(EMBEDDED_PGVECTOR_ARCHIVE, &dest)?;
            debug!(
                dest = %dest.display(),
                "pgvector extension files extracted"
            );
        }
        Ok(())
    })
    .await
    .map_err(|e| EmbeddedPostgresError::InitDb(format!("extraction task panicked: {e}")))?
}

/// Extract a gzip-compressed tar archive from `bytes` into `dest`.
///
/// The theseus-rs tarballs have a single top-level directory component
/// (`postgresql-{version}-{target}/`).  We strip it so the contents land
/// directly in `dest`:
///   archive: `postgresql-.../bin/initdb`  → dest: `dest/bin/initdb`
///   archive: `postgresql-.../lib/libpq.so.5`  → dest: `dest/lib/libpq.so.5`
///
/// Symlinks (common for shared-library versioned names on Linux) are created
/// as real filesystem symlinks so the dynamic linker resolves them correctly.
fn extract_tarball(bytes: &[u8], dest: &Path) -> Result<(), EmbeddedPostgresError> {
    let gz = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    // Preserve symlinks — required for libpq.so.5 → libpq.so etc.
    archive.set_preserve_permissions(true);
    archive.set_unpack_xattrs(false);

    for entry in archive
        .entries()
        .map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?;
        let entry_path = entry
            .path()
            .map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?
            .into_owned();

        // Strip the single top-level directory (`postgresql-{ver}-{target}/`).
        let mut components = entry_path.components();
        components.next(); // discard top-level dir
        let relative: std::path::PathBuf = components.collect();
        if relative.as_os_str().is_empty() {
            continue; // skip the top-level dir entry itself
        }

        let out_path = dest.join(&relative);
        let entry_type = entry.header().entry_type();

        if entry_type.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| EmbeddedPostgresError::Io {
                path: out_path.display().to_string(),
                reason: e.to_string(),
            })?;
            continue;
        }

        // Ensure parent directory exists.
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| EmbeddedPostgresError::Io {
                path: parent.display().to_string(),
                reason: e.to_string(),
            })?;
        }

        // Handle symlinks — critical for versioned .so files on Linux.
        // Without this, libpq.so.5 would be extracted as a file containing
        // only the symlink target text, causing "file too short" errors.
        if entry_type.is_symlink() || entry_type == tar::EntryType::Link {
            #[cfg(unix)]
            {
                use std::os::unix::fs::symlink;
                // link_name() gives the symlink target.
                if let Some(link_target) = entry.link_name().ok().flatten().map(|p| p.into_owned())
                {
                    // Remove stale entry if it already exists.
                    let _ = std::fs::remove_file(&out_path);
                    symlink(&link_target, &out_path).map_err(|e| EmbeddedPostgresError::Io {
                        path: out_path.display().to_string(),
                        reason: format!(
                            "symlink {} -> {}: {e}",
                            out_path.display(),
                            link_target.display()
                        ),
                    })?;
                    continue;
                }
                // Fall through to regular file extraction if link_name is absent.
            }
            #[cfg(not(unix))]
            {
                // On Windows symlinks require elevated privileges; skip them.
                // The PG Windows distribution uses hard links or DLL copies.
                continue;
            }
        }

        // Hard links: treat as regular file copy (simpler, always correct).
        // The tar::unpack path handles hard links natively but we replicate it
        // here to stay in control of error reporting.

        // Regular file.
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

        // Preserve executable bit on Unix (required for initdb, pg_ctl, etc.)
        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(mode);
            let _ = std::fs::set_permissions(&out_path, perms);
        }
    }

    debug!(dest = %dest.display(), "PostgreSQL archive extracted successfully");
    Ok(())
}

/// Extract a flat tar.gz (no top-level directory to strip) into `dest`.
///
/// Used for the pgvector archive whose entries are directly `lib/vector.so`,
/// `share/extension/vector.control`, etc.  No path component stripping.
fn extract_flat_tarball(bytes: &[u8], dest: &Path) -> Result<(), EmbeddedPostgresError> {
    let gz = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);

    for entry in archive.entries().map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))? {
        let mut entry = entry.map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?;
        let entry_path = entry
            .path()
            .map_err(|e| EmbeddedPostgresError::InitDb(e.to_string()))?
            .into_owned();

        let out_path = dest.join(&entry_path);

        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| EmbeddedPostgresError::Io {
                path: out_path.display().to_string(),
                reason: e.to_string(),
            })?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| EmbeddedPostgresError::Io {
                path: parent.display().to_string(),
                reason: e.to_string(),
            })?;
        }

        #[cfg(unix)]
        let mode = entry.header().mode().ok();

        let mut out_file = std::fs::File::create(&out_path).map_err(|e| {
            EmbeddedPostgresError::Io {
                path: out_path.display().to_string(),
                reason: e.to_string(),
            }
        })?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| EmbeddedPostgresError::Io {
            path: out_path.display().to_string(),
            reason: e.to_string(),
        })?;

        #[cfg(unix)]
        if let Some(mode) = mode {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(mode);
            let _ = std::fs::set_permissions(&out_path, perms);
        }
    }
    Ok(())
}
