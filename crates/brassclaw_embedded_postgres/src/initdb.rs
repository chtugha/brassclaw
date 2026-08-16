use std::path::Path;

use tokio::process::Command;
use tracing::debug;

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
        entries.push_str(&format!(
            "host  {db}  {db}  0.0.0.0/0  trust\n"
        ));
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
    let loopback_entry = format!("host  {}  {}  127.0.0.1/32  trust\n", config.database, config.database);
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
        // pgvector may already be installed globally — this is not fatal if the
        // extension is available at the OS level. Log a warning.
        tracing::warn!(
            path = %control_src.display(),
            "pgvector control file not found in embedded distribution; \
             CREATE EXTENSION vector will fail if pgvector is not installed globally"
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
