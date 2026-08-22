use std::path::Path;

use tokio::process::Command;
use tracing::{debug, warn};

use crate::config::{DEFAULT_EMBEDDED_PG_LISTEN_ADDRESSES, EmbeddedPostgresConfig};
use crate::error::EmbeddedPostgresError;

/// Log queries that take longer than this many milliseconds (1 second).
/// Matches the `log_min_duration_statement` value in the conf tuning template.
#[allow(dead_code)]
pub(crate) const LOG_MIN_DURATION_STATEMENT_MS: u32 = 1000;

/// The tuned `postgresql.conf` appended after `initdb` generates the default.
///
/// `jit = off` is required when `MemoryDenyWriteExecute=yes` is set in the
/// systemd unit. The two settings must be changed in tandem.
///
/// `log_min_duration_statement` is set to `LOG_MIN_DURATION_STATEMENT_MS` (1 s).
///
/// `{listen_addresses}` is substituted at runtime from
/// `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES` (default `127.0.0.1`). Set to
/// `0.0.0.0` to allow network-wide connections (requires appropriate
/// `pg_hba.conf` rules and network firewall policy).
const POSTGRESQL_CONF_TUNING_TEMPLATE: &str = r#"# Force TCP listener on the correct port.
# 'localhost' resolves to a Unix socket on Linux; unavailable under PrivateTmp=true.
# Port must match BRASSCLAW_EMBEDDED_PG_PORT (default 5434).
# listen_addresses is controlled by BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES.
listen_addresses = '{listen_addresses}'
port = {port}
max_connections = 20
shared_buffers = 32MB
work_mem = 4MB
max_wal_size = 1GB
autovacuum = on
# JIT disabled: pays off for OLAP scans, not the small OLTP queries here.
# Also required for MemoryDenyWriteExecute=yes in the systemd unit — PG JIT
# compiles into executable memory at runtime, which MDWE forbids.
# Do not enable JIT without also removing MemoryDenyWriteExecute=yes.
jit = off
log_destination = 'stderr'
logging_collector = on
log_directory = 'log'
log_filename = 'postgresql-%Y-%m-%d.log'
log_rotation_age = 1d
log_rotation_size = 50MB
log_truncate_on_rotation = on
# LOG_MIN_DURATION_STATEMENT_MS: log queries taking longer than 1000 ms (1 second).
log_min_duration_statement = 1000
"#;

/// Render the `postgresql.conf` tuning block for the given port and
/// listen-addresses string.
fn postgresql_conf_tuning(port: u16, listen_addresses: &str) -> String {
    POSTGRESQL_CONF_TUNING_TEMPLATE
        .replace("{listen_addresses}", listen_addresses)
        .replace("{port}", &port.to_string())
}

/// Init SQL run once after `initdb` to create the role and database,
/// and to write the loopback-only trust auth entry.
#[allow(dead_code)]
pub(crate) fn init_sql(config: &EmbeddedPostgresConfig) -> String {
    format!(
        r#"
CREATE ROLE {db} LOGIN;
CREATE DATABASE {db} OWNER {db};
"#,
        db = config.database
    )
}

/// Build the `pg_hba.conf` entry (or entries) for the given listen configuration.
///
/// Always includes a loopback trust entry. When `BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES`
/// is set to a non-loopback value (e.g. `0.0.0.0`), also adds a `md5`-auth entry
/// for the `brassclaw` database role from any address. The brassclaw binary
/// always connects via loopback (127.0.0.1) so the loopback trust entry
/// is always present; the wider entry enables external tooling access.
fn pg_hba_entries(config: &EmbeddedPostgresConfig) -> String {
    let db = &config.database;
    let listen_addresses = std::env::var("BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES")
        .unwrap_or_else(|_| DEFAULT_EMBEDDED_PG_LISTEN_ADDRESSES.to_string());

    // Loopback trust entry — always present.
    let mut entries = format!("host  {db}  {db}  127.0.0.1/32  trust\n");

    // When PG listens on a non-loopback address, add a trust entry for the
    // entire IPv4 range so external psql/tooling can connect without a
    // password. The embedded PG is only accessible from the local network
    // segment; production deployments should use BRASSCLAW_PG_URL instead.
    if listen_addresses != "127.0.0.1" && listen_addresses != "localhost" {
        entries.push_str(&format!("host  {db}  {db}  0.0.0.0/0  trust\n"));
    }
    entries
}

/// Run `initdb` and set up `postgresql.conf` + `pg_hba.conf`.
/// Skips silently if the data directory already exists and is non-empty.
pub async fn run_initdb(
    config: &EmbeddedPostgresConfig,
    pg_bin_dir: &Path,
) -> Result<(), EmbeddedPostgresError> {
    let data_dir = &config.data_dir;

    // Skip if the cluster already exists.
    if data_dir.exists() {
        let mut entries =
            tokio::fs::read_dir(data_dir)
                .await
                .map_err(|e| EmbeddedPostgresError::Io {
                    path: data_dir.display().to_string(),
                    reason: e.to_string(),
                })?;
        if entries.next_entry().await.ok().flatten().is_some() {
            debug!(
                data_dir = %data_dir.display(),
                "initdb skipped: data directory already exists and is non-empty"
            );
            return Ok(());
        }
    }

    tokio::fs::create_dir_all(data_dir)
        .await
        .map_err(|e| EmbeddedPostgresError::Io {
            path: data_dir.display().to_string(),
            reason: e.to_string(),
        })?;

    let initdb = pg_bin_dir.join("initdb");
    let output = Command::new(&initdb)
        .args([
            "-D",
            &data_dir.display().to_string(),
            "-U",
            &config.superuser,
            "--auth=trust",
            "--encoding=UTF8",
        ])
        .output()
        .await
        .map_err(|e| EmbeddedPostgresError::Spawn(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EmbeddedPostgresError::InitDb(stderr.into_owned()));
    }

    // Append tuning to postgresql.conf (the file was created by initdb above).
    let conf_path = data_dir.join("postgresql.conf");
    let existing_conf =
        tokio::fs::read_to_string(&conf_path)
            .await
            .map_err(|e| EmbeddedPostgresError::Io {
                path: conf_path.display().to_string(),
                reason: e.to_string(),
            })?;
    // Only append if the tuning block is not already present (idempotency guard).
    if !existing_conf.contains("brassclaw tuning") {
        let listen_addresses = std::env::var("BRASSCLAW_EMBEDDED_PG_LISTEN_ADDRESSES")
            .unwrap_or_else(|_| DEFAULT_EMBEDDED_PG_LISTEN_ADDRESSES.to_string());
        let tuning = postgresql_conf_tuning(config.port, &listen_addresses);
        let appended = format!(
            "{existing_conf}\n# brassclaw tuning — conservative settings for a single-user agent workload\n{tuning}"
        );
        tokio::fs::write(&conf_path, appended)
            .await
            .map_err(|e| EmbeddedPostgresError::Io {
                path: conf_path.display().to_string(),
                reason: e.to_string(),
            })?;
    }

    // Append trust auth entries to pg_hba.conf.
    let hba_path = data_dir.join("pg_hba.conf");
    let existing = tokio::fs::read_to_string(&hba_path)
        .await
        .unwrap_or_default();
    let entries = pg_hba_entries(config);
    // Only append if the first (loopback) entry is not already there (idempotency guard).
    let loopback_entry = format!(
        "host  {}  {}  127.0.0.1/32  trust\n",
        config.database, config.database
    );
    if !existing.contains(&loopback_entry) {
        let mut updated = existing;
        updated.push_str(&entries);
        tokio::fs::write(&hba_path, updated)
            .await
            .map_err(|e| EmbeddedPostgresError::Io {
                path: hba_path.display().to_string(),
                reason: e.to_string(),
            })?;
    }

    // Install pgvector shared library.
    install_pgvector(config, pg_bin_dir).await?;

    debug!(
        data_dir = %data_dir.display(),
        "initdb completed"
    );
    Ok(())
}

/// Copy the bundled pgvector shared library into the PostgreSQL installation's
/// `lib/` and `share/extension/` subdirectories so `CREATE EXTENSION vector`
/// works. PostgreSQL resolves extension files from its installation tree, not
/// from the data directory.
///
/// The pgvector library files are expected to be pre-bundled alongside the PG
/// binaries in the `postgresql_embedded` distribution (in `lib/` and
/// `share/extension/` relative to the installation root).
pub async fn install_pgvector(
    _config: &EmbeddedPostgresConfig,
    pg_bin_dir: &Path,
) -> Result<(), EmbeddedPostgresError> {
    // The installation root is the parent of the bin/ directory.
    let pg_base = pg_bin_dir.parent().unwrap_or(pg_bin_dir);
    let lib_src = pg_base.join("lib");
    let ext_src = pg_base.join("share").join("extension");

    // Destination: same installation tree. If the files are already present
    // (bundled by postgresql_embedded), the copy is skipped (exists check below).
    let lib_dst = pg_base.join("lib");
    let ext_dst = pg_base.join("share").join("extension");

    for dir in [&lib_dst, &ext_dst] {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| EmbeddedPostgresError::Io {
                path: dir.display().to_string(),
                reason: e.to_string(),
            })?;
    }

    // Check that the pgvector control file is present in the source tree.
    let control_src = ext_src.join("vector.control");
    if !control_src.exists() {
        // The embedded pgvector archive was empty (compile failed on this host).
        // Try the runtime fallback chain (system/brew PG-16 ->
        // BRASSCLAW_PGVECTOR_URL) before degrading. This is a degraded-mode
        // escape hatch; the normal path is fully embedded (no runtime network).
        if procure_pgvector_fallback(pg_base).await {
            debug!(
                "pgvector procured via runtime fallback \
                 (system/brew or BRASSCLAW_PGVECTOR_URL)"
            );
            // The fallback places files directly into pg_base/lib +
            // pg_base/share/extension (== lib_dst + ext_dst). Re-check the
            // control file; if present, V000 `CREATE EXTENSION vector` can proceed.
            if ext_dst.join("vector.control").exists() {
                return Ok(());
            }
        }
        warn!(
            path = %control_src.display(),
            "pgvector control file not found in embedded distribution and runtime \
             fallback did not procure it; CREATE EXTENSION vector will fail unless \
             pgvector is installed globally"
        );
        return Ok(());
    }

    // Copy control file.
    let control_dst = ext_dst.join("vector.control");
    if !control_dst.exists() {
        tokio::fs::copy(&control_src, &control_dst)
            .await
            .map_err(|e| EmbeddedPostgresError::Io {
                path: control_dst.display().to_string(),
                reason: e.to_string(),
            })?;
    }

    // Copy SQL script files (vector--*.sql).
    let mut dir_entries =
        tokio::fs::read_dir(&ext_src)
            .await
            .map_err(|e| EmbeddedPostgresError::Io {
                path: ext_src.display().to_string(),
                reason: e.to_string(),
            })?;
    while let Ok(Some(entry)) = dir_entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("vector--") && name_str.ends_with(".sql") {
            let dst = ext_dst.join(&*name_str);
            if !dst.exists() {
                tokio::fs::copy(entry.path(), &dst).await.map_err(|e| {
                    EmbeddedPostgresError::Io {
                        path: dst.display().to_string(),
                        reason: e.to_string(),
                    }
                })?;
            }
        }
    }

    // Copy shared library (vector.so / vector.dylib).
    for lib_name in ["vector.so", "vector.dylib", "vector.dll"] {
        let lib_src_path = lib_src.join(lib_name);
        if lib_src_path.exists() {
            let lib_dst_path = lib_dst.join(lib_name);
            if !lib_dst_path.exists() {
                tokio::fs::copy(&lib_src_path, &lib_dst_path)
                    .await
                    .map_err(|e| EmbeddedPostgresError::Io {
                        path: lib_dst_path.display().to_string(),
                        reason: e.to_string(),
                    })?;
            }
        }
    }

    debug!("pgvector extension files installed in data directory");
    Ok(())
}

/// Procure pgvector at runtime when the embedded archive was empty (compile
/// failed on this host). A **degraded-mode escape hatch** — the normal path is
/// fully embedded (no runtime network; see the `build.rs` header). Fires only
/// inside `install_pgvector` when `pg_base/share/extension/vector.control` is
/// absent.
///
/// Tries, in order, and returns `true` only when a `vector.control` lands in
/// `pg_base/share/extension`:
///
/// (a) **System/brew location (no network):** probe candidate roots for a
///     PG-16 pgvector (`vector.control` + `vector.dylib|so` + `vector--*.sql`)
///     and copy the files into `pg_base/lib` + `pg_base/share/extension`. PG's
///     extension ABI is stable within a major version, so a system PG-16
///     `vector.dylib` loads in the embedded PG 16.4.0 — gated on major 16.
/// (b) **Env-URL download (network, opt-in):** if `BRASSCLAW_PGVECTOR_URL` is
///     set, download a prebuilt flat tarball (`lib/vector.*` +
///     `share/extension/*`) via `curl` (mirroring `build.rs::download_with_curl`)
///     and extract it into `pg_base`.
/// (c) Neither procures → `false`.
///
/// Errors are logged at `debug` and swallowed — this is best-effort, never a
/// hard failure (the caller warns + returns `Ok` so V000 reports the real error).
async fn procure_pgvector_fallback(pg_base: &Path) -> bool {
    let lib_dst = pg_base.join("lib");
    let ext_dst = pg_base.join("share").join("extension");
    for dir in [&lib_dst, &ext_dst] {
        if let Err(e) = tokio::fs::create_dir_all(dir).await {
            debug!(path = %dir.display(), error = %e, "pgvector fallback: create_dir failed");
        }
    }

    if procure_pgvector_from_system(&lib_dst, &ext_dst).await {
        return ext_dst.join("vector.control").exists();
    }

    if let Ok(url) = std::env::var("BRASSCLAW_PGVECTOR_URL") {
        let url = url.trim();
        if !url.is_empty() {
            debug!(url = %url, "pgvector fallback: downloading prebuilt tarball");
            if procure_pgvector_from_url(url, pg_base).await {
                return ext_dst.join("vector.control").exists();
            }
        }
    }

    false
}

/// System/brew pgvector probe (no network). Builds the per-platform candidate
/// `(extension_dir, lib_dir)` pairs plus a `pg_config --version`-gated PG-16
/// probe, then delegates the copy to `copy_pgvector_from_candidates`.
async fn procure_pgvector_from_system(lib_dst: &Path, ext_dst: &Path) -> bool {
    let mut candidates = pgvector_system_candidates();
    if let Some((ext_dir, lib_dir)) = pgvector_via_pgconfig().await {
        candidates.push((ext_dir, lib_dir));
    }
    copy_pgvector_from_candidates(&candidates, lib_dst, ext_dst).await
}

/// Copy the first **complete** pgvector set from `candidates` into the embedded
/// install tree `(lib_dst, ext_dst)`. A candidate is complete when its
/// `extension_dir` holds `vector.control` (and any `vector--*.sql`) AND its
/// `lib_dir` holds one of `vector.dylib` / `vector.so` / `vector.dll`.
///
/// Candidate-injected so the side-effecting copy path is unit-testable with
/// temp dirs (the production caller supplies the real system/brew + pg_config
/// candidate list; tests supply a synthetic one).
async fn copy_pgvector_from_candidates(
    candidates: &[(std::path::PathBuf, std::path::PathBuf)],
    lib_dst: &Path,
    ext_dst: &Path,
) -> bool {
    for (ext_dir, lib_dir) in candidates {
        if !ext_dir.join("vector.control").exists() {
            continue;
        }
        // Need at least one shared library (vector.dylib on macOS, vector.so on Linux).
        let mut lib_name: Option<&str> = None;
        for n in ["vector.dylib", "vector.so", "vector.dll"] {
            if lib_dir.join(n).exists() {
                lib_name = Some(n);
                break;
            }
        }
        let Some(lib_name) = lib_name else { continue };

        // Copy control + all vector--*.sql scripts.
        if tokio::fs::copy(
            ext_dir.join("vector.control"),
            ext_dst.join("vector.control"),
        )
        .await
        .is_err()
        {
            continue;
        }
        if let Ok(mut rd) = tokio::fs::read_dir(ext_dir).await {
            while let Ok(Some(entry)) = rd.next_entry().await {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("vector--") && name_str.ends_with(".sql") {
                    let _ = tokio::fs::copy(entry.path(), ext_dst.join(&*name_str)).await;
                }
            }
        }
        // Copy the shared library.
        let _ = tokio::fs::copy(lib_dir.join(lib_name), lib_dst.join(lib_name)).await;

        debug!(
            ext_dir = %ext_dir.display(),
            lib_dir = %lib_dir.display(),
            "pgvector system fallback: copied extension files"
        );
        return true;
    }
    false
}

/// Per-platform candidate `(extension_dir, lib_dir)` pairs for a system/brew
/// pgvector. Covers Homebrew `postgresql@16` / `libpgvector` / `pgvector` kegs
/// (Apple Silicon + Intel) and the PGDG/Debian apt layout. Returns empty on
/// targets with no known system layout (the `pg_config` probe may still find one).
fn pgvector_system_candidates() -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    let mut out: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    push_macos_pgvector_candidates(&mut out);
    push_linux_pgvector_candidates(&mut out);
    out
}

#[cfg(target_os = "macos")]
fn push_macos_pgvector_candidates(out: &mut Vec<(std::path::PathBuf, std::path::PathBuf)>) {
    let brew = if cfg!(target_arch = "aarch64") {
        "/opt/homebrew"
    } else {
        "/usr/local"
    };
    // Homebrew `postgresql@16` keg.
    out.push((
        format!("{brew}/opt/postgresql@16/share/postgresql@16/extension").into(),
        format!("{brew}/opt/postgresql@16/lib").into(),
    ));
    // Homebrew shared tree (unlinked keg writes here).
    out.push((
        format!("{brew}/share/postgresql@16/extension").into(),
        format!("{brew}/lib").into(),
    ));
    // Homebrew `libpgvector` / `pgvector` standalone kegs.
    out.push((
        format!("{brew}/opt/libpgvector/share/extension").into(),
        format!("{brew}/opt/libpgvector/lib").into(),
    ));
    out.push((
        format!("{brew}/opt/pgvector/share/extension").into(),
        format!("{brew}/opt/pgvector/lib").into(),
    ));
}

#[cfg(not(target_os = "macos"))]
fn push_macos_pgvector_candidates(_out: &mut Vec<(std::path::PathBuf, std::path::PathBuf)>) {}

#[cfg(target_os = "linux")]
fn push_linux_pgvector_candidates(out: &mut Vec<(std::path::PathBuf, std::path::PathBuf)>) {
    // PGDG / Debian apt `postgresql-16` layout.
    out.push((
        "/usr/share/postgresql/16/extension".into(),
        "/usr/lib/postgresql/16/lib".into(),
    ));
}

#[cfg(not(target_os = "linux"))]
fn push_linux_pgvector_candidates(_out: &mut Vec<(std::path::PathBuf, std::path::PathBuf)>) {}

/// If `pg_config` is on `PATH` and reports PostgreSQL 16.x, return
/// `(<sharedir>/extension, <pkglibdir>)` as a candidate probe root. PG major 16
/// is required (ABI-stable within a major version → loads in embedded PG 16.4.0).
async fn pgvector_via_pgconfig() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let version_output = Command::new("pg_config")
        .arg("--version")
        .output()
        .await
        .ok()?;
    if !version_output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&version_output.stdout);
    let major = pg_major_from_version(&version)?;
    if major != 16 {
        debug!(
            version = %version.trim(),
            "pgvector fallback: pg_config is not PG-16, skipping system probe"
        );
        return None;
    }
    let pkglibdir = pg_config_dir("--pkglibdir").await?;
    let sharedir = pg_config_dir("--sharedir").await?;
    Some((sharedir.join("extension"), pkglibdir))
}

/// Run `pg_config <flag>` and return its trimmed stdout as a path, or `None` on
/// any failure.
async fn pg_config_dir(flag: &str) -> Option<std::path::PathBuf> {
    let out = Command::new("pg_config").arg(flag).output().await.ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        return None;
    }
    Some(s.into())
}

/// Parse the major version number from a `pg_config --version` string like
/// `"PostgreSQL 16.4"` or `"PostgreSQL 16.4.0"`. Returns `None` if no leading
/// integer follows the `PostgreSQL` token.
fn pg_major_from_version(version: &str) -> Option<u32> {
    let after = version.split("PostgreSQL").nth(1)?;
    let num = after
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()?;
    num.parse::<u32>().ok()
}

/// Download a prebuilt pgvector tarball from `BRASSCLAW_PGVECTOR_URL` via `curl`
/// and extract it (flat layout `lib/vector.*` + `share/extension/*`) into
/// `pg_base`. Mirrors `build.rs::download_with_curl`. Uses `curl --output -` so
/// the bytes land in memory (the tarball is small) — no temp file needed.
/// Returns `true` on success.
async fn procure_pgvector_from_url(url: &str, pg_base: &Path) -> bool {
    let out = Command::new("curl")
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "--retry",
            "5",
            "--retry-connrefused",
            "--location",
            "--silent",
            "--show-error",
            "--fail",
            "--output",
            "-",
            url,
        ])
        .output()
        .await;
    let bytes = match out {
        Ok(o) if o.status.success() => o.stdout,
        _ => {
            debug!("pgvector fallback: curl download failed");
            return false;
        }
    };
    if bytes.is_empty() {
        debug!("pgvector fallback: downloaded tarball is empty");
        return false;
    }
    let dest = pg_base.to_path_buf();
    let result =
        tokio::task::spawn_blocking(move || crate::extract::extract_flat_tarball(&bytes, &dest))
            .await;
    matches!(result, Ok(Ok(())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_major_from_version_parses_postgres_versions() {
        assert_eq!(pg_major_from_version("PostgreSQL 16.4"), Some(16));
        assert_eq!(pg_major_from_version("PostgreSQL 16.4.0\n"), Some(16));
        assert_eq!(pg_major_from_version("PostgreSQL 17.0"), Some(17));
        assert_eq!(pg_major_from_version("PostgreSQL 16beta1"), Some(16));
        assert_eq!(pg_major_from_version("not a version"), None);
        assert_eq!(pg_major_from_version(""), None);
    }

    #[test]
    fn system_candidates_extension_dirs_end_in_extension() {
        let candidates = pgvector_system_candidates();
        // On macOS/Linux at least one candidate is registered; on other targets
        // the static list is empty (the pg_config probe may still find one at
        // runtime). Every registered extension dir must end in `extension`.
        for (ext_dir, _lib_dir) in &candidates {
            assert!(
                ext_dir
                    .file_name()
                    .map(|n| n == "extension")
                    .unwrap_or(false),
                "extension dir must end in `extension`: {}",
                ext_dir.display()
            );
        }
    }

    #[tokio::test]
    async fn copy_pgvector_from_candidates_copies_complete_set() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();

        // Synthetic "system" pgvector layout.
        let sys_ext = root.join("sys").join("share").join("extension");
        let sys_lib = root.join("sys").join("lib");
        tokio::fs::create_dir_all(&sys_ext).await.unwrap();
        tokio::fs::create_dir_all(&sys_lib).await.unwrap();
        tokio::fs::write(sys_ext.join("vector.control"), b"# control")
            .await
            .unwrap();
        tokio::fs::write(sys_ext.join("vector--0.0.0.sql"), b"-- sql")
            .await
            .unwrap();
        tokio::fs::write(sys_ext.join("not-vector.sql"), b"ignore me")
            .await
            .unwrap();
        tokio::fs::write(sys_lib.join("vector.dylib"), b"\x7fELF-fake")
            .await
            .unwrap();

        // Destination (embedded install tree).
        let dst_lib = root.join("dst").join("lib");
        let dst_ext = root.join("dst").join("share").join("extension");
        tokio::fs::create_dir_all(&dst_lib).await.unwrap();
        tokio::fs::create_dir_all(&dst_ext).await.unwrap();

        let candidates = vec![(sys_ext.clone(), sys_lib.clone())];
        let ok = copy_pgvector_from_candidates(&candidates, &dst_lib, &dst_ext).await;
        assert!(ok, "complete candidate must copy");

        // Control + matching sql + lib copied; non-matching sql ignored.
        assert!(dst_ext.join("vector.control").exists());
        assert!(dst_ext.join("vector--0.0.0.sql").exists());
        assert!(!dst_ext.join("not-vector.sql").exists());
        assert!(dst_lib.join("vector.dylib").exists());
    }

    #[tokio::test]
    async fn copy_pgvector_from_candidates_skips_incomplete_set() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();

        // Candidate with a control file but NO shared library -> incomplete.
        let sys_ext = root.join("sys_ext");
        let sys_lib = root.join("sys_lib");
        tokio::fs::create_dir_all(&sys_ext).await.unwrap();
        tokio::fs::create_dir_all(&sys_lib).await.unwrap();
        tokio::fs::write(sys_ext.join("vector.control"), b"# control")
            .await
            .unwrap();
        // No vector.dylib/so/dll in sys_lib.

        let dst_lib = root.join("dst_lib");
        let dst_ext = root.join("dst_ext");
        tokio::fs::create_dir_all(&dst_lib).await.unwrap();
        tokio::fs::create_dir_all(&dst_ext).await.unwrap();

        let candidates = vec![(sys_ext.clone(), sys_lib.clone())];
        let ok = copy_pgvector_from_candidates(&candidates, &dst_lib, &dst_ext).await;
        assert!(!ok, "candidate missing a shared library must be skipped");
        assert!(
            !dst_ext.join("vector.control").exists(),
            "incomplete candidate must not copy the control file"
        );
    }
}
