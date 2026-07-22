//! `brassclaw secrets` subcommands (§4.4).
//!
//! Currently implements:
//!   `brassclaw secrets rewrap --strategy <strategy> [--tenant <id>] [--old-passphrase-file <path>]`
//!
//! Wraps the *existing* AES-256 master key with a new strategy/passphrase.
//! Updates `wrapped_key` and `algorithm` on the `brassclaw_secrets_master`
//! row; does NOT generate a new key or re-encrypt existing `brassclaw_secrets`
//! rows (that is `rotate`, a separate operation).

use std::path::PathBuf;

use anyhow::Context as _;
use clap::{Args, Subcommand};

use crate::context::RebornCliContext;

#[derive(Debug, Args)]
pub(crate) struct SecretsCommand {
    #[command(subcommand)]
    subcommand: SecretsSubcommand,
}

#[derive(Debug, Subcommand)]
enum SecretsSubcommand {
    /// Wrap the existing master key with a new strategy (passphrase or raw-key).
    ///
    /// Updates `brassclaw_secrets_master` in-place (version = 1) — does NOT
    /// generate a new key or re-encrypt existing secret rows.
    ///
    /// KEY-SOURCE RULE: rewrap always reads an existing raw-key file if one is
    /// present (`$REBORN_HOME/.reborn-local-dev-secrets-master-key` then
    /// `.secrets-master-key`).  It never generates a new key when encrypted
    /// rows already exist — that would orphan every stored credential.
    ///
    /// TENANT: must match the `boot_tenant` that `brassclaw serve` will check
    /// at next boot (see tenant resolution below).  Use `--tenant` explicitly
    /// in upgrade runbooks to avoid ambiguity.
    Rewrap(RewrapCommand),
}

impl SecretsCommand {
    pub(crate) fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        match self.subcommand {
            SecretsSubcommand::Rewrap(cmd) => cmd.execute(context),
        }
    }
}

// ---------------------------------------------------------------------------
// rewrap
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
struct RewrapCommand {
    /// Wrap strategy. One of:
    ///   `passphrase-file=<path>` — read passphrase from a file (recommended for production)
    ///   `passphrase`             — prompt interactively (for ad-hoc / local-dev use)
    ///   `raw-key`                — revert to raw-key-on-disk (no passphrase wrapping)
    #[arg(long)]
    strategy: String,

    /// Tenant ID to write (must match identity.tenant in config.toml / DB).
    ///
    /// Priority order (highest → lowest):
    ///   1. this flag
    ///   2. identity.tenant in $REBORN_HOME/config.toml
    ///   3. identity.tenant in brassclaw_config DB
    ///   4. "default"
    #[arg(long)]
    tenant: Option<String>,

    /// Path to a file holding the *current* passphrase when changing the
    /// passphrase interactively from a shell (where BRASSCLAW_SECRETS_PASSPHRASE_FILE
    /// and $CREDENTIALS_DIRECTORY are absent).
    ///
    /// Ignored when the existing algorithm is `raw-key-on-disk`.
    #[arg(long)]
    old_passphrase_file: Option<PathBuf>,
}

/// Parsed strategy variant.
#[derive(Debug)]
enum RewrapStrategy {
    PassphraseFile(PathBuf),
    PassphraseInteractive,
    RawKey,
}

impl RewrapStrategy {
    fn parse(s: &str) -> anyhow::Result<Self> {
        if let Some(path) = s.strip_prefix("passphrase-file=") {
            if path.is_empty() {
                anyhow::bail!("--strategy passphrase-file=<path> requires a non-empty path");
            }
            return Ok(Self::PassphraseFile(PathBuf::from(path)));
        }
        match s {
            "passphrase" => Ok(Self::PassphraseInteractive),
            "raw-key" => Ok(Self::RawKey),
            "keychain" => anyhow::bail!(
                "--strategy keychain is not yet supported in the Linux/headless deployment \
                 path. Use passphrase-file=<path> for production or passphrase for \
                 interactive local use."
            ),
            other => anyhow::bail!(
                "unknown --strategy '{other}'; expected one of: \
                 passphrase-file=<path>, passphrase, raw-key"
            ),
        }
    }
}

impl RewrapCommand {
    fn execute(self, context: RebornCliContext) -> anyhow::Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(self.run_async(context))
    }

    #[cfg(feature = "postgres")]
    async fn run_async(self, context: RebornCliContext) -> anyhow::Result<()> {
        let strategy = RewrapStrategy::parse(&self.strategy)?;
        let reborn_home = context.boot_config().home().path().to_path_buf();

        // Step 1: Build PG pool and run schema migrations (§6.4).
        let pool = build_pg_pool().await?;
        brassclaw_pg::migrations::run_migrations(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("schema migrations failed: {e}"))?;

        // Step 2: Resolve tenant_id (§4.4 4-step resolution).
        let tenant_id = resolve_tenant_id(self.tenant.as_deref(), &reborn_home, &pool).await?;

        // Step 3: Read the existing master key (key-source rule).
        let master_key_bytes = resolve_master_key(
            &tenant_id,
            &reborn_home,
            self.old_passphrase_file.as_deref(),
            &pool,
        )
        .await?;

        // Step 4a: For raw-key strategy, write the hex key to the canonical
        // on-disk file BEFORE the DB upsert.  If the upsert fails, the key
        // file is still present and the operator can retry.
        if matches!(strategy, RewrapStrategy::RawKey) {
            let key_path = reborn_home.join(".secrets-master-key");
            let hex_key = hex::encode(&master_key_bytes);
            std::fs::write(&key_path, &hex_key)
                .with_context(|| format!("failed to write key file {}", key_path.display()))?;
            // Restrict permissions: owner read-only.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                    .with_context(|| format!("failed to chmod 0600 {}", key_path.display()))?;
            }
        }

        // Step 4b: Produce wrapped_key / algorithm for the chosen strategy.
        let (wrapped_key, algorithm) = wrap_master_key(&master_key_bytes, &strategy)?;

        // Step 5: Upsert brassclaw_secrets_master.
        upsert_secrets_master(&pool, &tenant_id, &wrapped_key, &algorithm).await?;

        // Step 6: After a successful passphrase-wrap upsert, zero and remove any
        // raw key files that were read in step 3 (they are superseded by the DB
        // row).  For raw-key strategy we just wrote the canonical file, so there
        // is nothing to remove.  Only zero files that existed before this run.
        if algorithm != "raw-key-on-disk" {
            let legacy = reborn_home.join(".reborn-local-dev-secrets-master-key");
            let canonical = reborn_home.join(".secrets-master-key");
            for path in &[&legacy, &canonical] {
                if path.exists() {
                    zero_and_remove(path).with_context(|| {
                        format!("failed to zero raw key file {}", path.display())
                    })?;
                    println!("Zeroed and removed: {}", path.display());
                }
            }
        }

        println!("brassclaw secrets rewrap: tenant={tenant_id} strategy={algorithm} — done.");
        Ok(())
    }

    #[cfg(not(feature = "postgres"))]
    async fn run_async(self, _context: RebornCliContext) -> anyhow::Result<()> {
        anyhow::bail!("`brassclaw secrets rewrap` requires the `postgres` feature")
    }
}

// ---------------------------------------------------------------------------
// Tenant resolution (§4.4 — 4-step priority)
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
async fn resolve_tenant_id(
    flag: Option<&str>,
    reborn_home: &std::path::Path,
    pool: &deadpool_postgres::Pool,
) -> anyhow::Result<String> {
    // 1. --tenant flag
    if let Some(t) = flag {
        return Ok(t.to_string());
    }

    // 2. identity.tenant from $REBORN_HOME/config.toml
    let config_path = reborn_home.join("config.toml");
    if let Ok(Some(cfg)) = brassclaw_reborn_config::RebornConfigFile::load(&config_path)
        && let Some(tenant) = cfg.identity.and_then(|i| i.tenant)
        && !tenant.trim().is_empty()
    {
        return Ok(tenant);
    }

    // 3. brassclaw_config DB key identity.tenant
    let snapshot = brassclaw_reborn_composition::db_config::load_config_snapshot(pool, "default")
        .await
        .unwrap_or_default();
    if let Some(tenant) = snapshot.identity.and_then(|i| i.tenant)
        && !tenant.trim().is_empty()
    {
        return Ok(tenant);
    }

    // 4. Fallback
    Ok("default".to_string())
}

// ---------------------------------------------------------------------------
// Master key source resolution (key-source rule)
// ---------------------------------------------------------------------------

/// Resolve the raw plaintext master key bytes.
///
/// Priority:
///  1. If a raw key file exists (pre-migration name first, then canonical) →
///     read and decode hex.  This is the "normal first run" path.
///  2. If `brassclaw_secrets_master` already has a row with algorithm =
///     `aes256gcm-argon2id` → unwrap with the old passphrase (passphrase-change
///     path).
///  3. If neither file nor DB row exists AND no encrypted rows are present →
///     generate a fresh 32-byte random key (fresh-install path).
///  4. Fail-closed: if neither file nor DB row and encrypted rows DO exist.
#[cfg(feature = "postgres")]
async fn resolve_master_key(
    tenant_id: &str,
    reborn_home: &std::path::Path,
    old_passphrase_file: Option<&std::path::Path>,
    pool: &deadpool_postgres::Pool,
) -> anyhow::Result<Vec<u8>> {
    use secrecy::ExposeSecret as _;

    // Check for raw key files (pre-migration name first).
    let legacy = reborn_home.join(".reborn-local-dev-secrets-master-key");
    let canonical = reborn_home.join(".secrets-master-key");

    for path in &[&legacy, &canonical] {
        if path.exists() {
            let hex = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read raw key file {}", path.display()))?;
            let key = hex::decode(hex.trim())
                .with_context(|| format!("raw key file {} is not valid hex", path.display()))?;
            return Ok(key);
        }
    }

    // No raw key file — check the DB row.
    let client = pool.get().await.context("failed to get DB connection")?;
    let row = client
        .query_opt(
            "SELECT wrapped_key, algorithm FROM brassclaw_secrets_master \
             WHERE tenant_id = $1 ORDER BY version DESC LIMIT 1",
            &[&tenant_id],
        )
        .await
        .context("failed to query brassclaw_secrets_master")?;

    if let Some(r) = row {
        let wrapped_key: String = r.get(0);
        let algorithm: String = r.get(1);

        if algorithm == "aes256gcm-argon2id" {
            // Passphrase-change path: read old passphrase and unwrap.
            let old_passphrase = read_old_passphrase(old_passphrase_file)?;
            let key = brassclaw_reborn_composition::secrets_master::unwrap_master_key_argon2id(
                &wrapped_key,
                old_passphrase.expose_secret(),
            )
            .map_err(|e| anyhow::anyhow!("failed to unwrap current master key: {e}"))?;
            return Ok(key);
        }

        // algorithm = 'raw-key-on-disk' but no file on disk — this should not
        // happen in normal operation (the serve path also enforces this).
        anyhow::bail!(
            "brassclaw_secrets_master has algorithm='raw-key-on-disk' for tenant '{tenant_id}' \
             but no raw key file was found at {} or {}. \
             Restore the original key file and retry.",
            legacy.display(),
            canonical.display(),
        );
    }

    // No DB row and no raw key file.  If encrypted rows exist, fail closed.
    let encrypted_rows = count_encrypted_rows(pool, tenant_id).await?;
    if encrypted_rows > 0 {
        anyhow::bail!(
            "raw key file not found but encrypted rows exist — cannot generate new key; \
             restore the original key file first"
        );
    }

    // Truly fresh install: generate a new 32-byte key.
    let mut key = vec![0u8; 32];
    use std::io::Read as _;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut key))
        .context("failed to read random bytes from /dev/urandom")?;
    Ok(key)
}

/// Read the "old passphrase" for a passphrase-change rewrap.
///
/// Priority (§4.4 R6-L1):
///   1. `--old-passphrase-file <path>` CLI flag
///   2. `BRASSCLAW_SECRETS_PASSPHRASE_FILE` env var
///   3. `$CREDENTIALS_DIRECTORY/secrets-passphrase` (systemd `LoadCredential`)
#[cfg(feature = "postgres")]
fn read_old_passphrase(
    flag_path: Option<&std::path::Path>,
) -> anyhow::Result<secrecy::SecretString> {
    // 1. CLI flag
    if let Some(path) = flag_path {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read --old-passphrase-file {}", path.display()))?;
        return Ok(secrecy::SecretString::from(s.trim().to_string()));
    }

    // 2. BRASSCLAW_SECRETS_PASSPHRASE_FILE
    if let Ok(env_path) = std::env::var("BRASSCLAW_SECRETS_PASSPHRASE_FILE") {
        let env_path = env_path.trim();
        if !env_path.is_empty() {
            let s = std::fs::read_to_string(env_path).with_context(|| {
                format!("failed to read BRASSCLAW_SECRETS_PASSPHRASE_FILE={env_path}")
            })?;
            return Ok(secrecy::SecretString::from(s.trim().to_string()));
        }
    }

    // 3. $CREDENTIALS_DIRECTORY/secrets-passphrase
    if let Ok(cred_dir) = std::env::var("CREDENTIALS_DIRECTORY") {
        let cred_path = std::path::Path::new(&cred_dir).join("secrets-passphrase");
        if cred_path.exists() {
            let s = std::fs::read_to_string(&cred_path)
                .with_context(|| format!("failed to read {}", cred_path.display()))?;
            return Ok(secrecy::SecretString::from(s.trim().to_string()));
        }
    }

    anyhow::bail!(
        "the existing master key is passphrase-wrapped but no old passphrase source was found. \
         Supply --old-passphrase-file=<path>, set BRASSCLAW_SECRETS_PASSPHRASE_FILE, \
         or ensure $CREDENTIALS_DIRECTORY/secrets-passphrase is accessible."
    )
}

/// Count rows in `brassclaw_secrets` and `brassclaw_root_filesystem` for `tenant_id`.
#[cfg(feature = "postgres")]
async fn count_encrypted_rows(
    pool: &deadpool_postgres::Pool,
    tenant_id: &str,
) -> anyhow::Result<i64> {
    let client = pool.get().await.context("failed to get DB connection")?;

    // brassclaw_secrets
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_secrets WHERE tenant_id = $1",
            &[&tenant_id],
        )
        .await
        .context("failed to count brassclaw_secrets rows")?;
    let secrets_count: i64 = row.get(0);

    // brassclaw_root_filesystem (may not exist on a truly fresh install before
    // any schema migrations — ignore if the table is absent).
    let fs_row = client
        .query_opt(
            "SELECT COUNT(*) FROM brassclaw_root_filesystem WHERE tenant_id = $1",
            &[&tenant_id],
        )
        .await;
    let fs_count: i64 = match fs_row {
        Ok(Some(r)) => r.get(0),
        Ok(None) | Err(_) => 0,
    };

    Ok(secrets_count + fs_count)
}

// ---------------------------------------------------------------------------
// Wrap / unwrap helpers
// ---------------------------------------------------------------------------

/// Wrap `master_key_bytes` using the chosen strategy.
///
/// Returns `(wrapped_key, algorithm)` where:
/// - `raw-key`:            `("", "raw-key-on-disk")`
/// - `passphrase*`:        `(base64(salt||nonce||ciphertext), "aes256gcm-argon2id")`
#[cfg(feature = "postgres")]
fn wrap_master_key(
    master_key_bytes: &[u8],
    strategy: &RewrapStrategy,
) -> anyhow::Result<(String, String)> {
    match strategy {
        RewrapStrategy::RawKey => {
            // Raw-key-on-disk: the key lives on disk at $REBORN_HOME/.secrets-master-key.
            // The DB row stores an empty wrapped_key with algorithm = 'raw-key-on-disk'.
            // The caller is responsible for writing the hex key to disk.
            Ok(("".to_string(), "raw-key-on-disk".to_string()))
        }

        RewrapStrategy::PassphraseFile(path) => {
            let passphrase = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read passphrase file {}", path.display()))?;
            let wrapped = brassclaw_reborn_composition::secrets_master::wrap_master_key_argon2id(
                master_key_bytes,
                passphrase.trim(),
            )
            .map_err(|e| anyhow::anyhow!("failed to wrap master key: {e}"))?;
            Ok((wrapped, "aes256gcm-argon2id".to_string()))
        }

        RewrapStrategy::PassphraseInteractive => {
            let passphrase = rpassword::prompt_password("Enter passphrase for master key: ")
                .context("failed to read passphrase from terminal")?;
            let wrapped = brassclaw_reborn_composition::secrets_master::wrap_master_key_argon2id(
                master_key_bytes,
                passphrase.trim(),
            )
            .map_err(|e| anyhow::anyhow!("failed to wrap master key: {e}"))?;
            Ok((wrapped, "aes256gcm-argon2id".to_string()))
        }
    }
}

// ---------------------------------------------------------------------------
// DB write
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
async fn upsert_secrets_master(
    pool: &deadpool_postgres::Pool,
    tenant_id: &str,
    wrapped_key: &str,
    algorithm: &str,
) -> anyhow::Result<()> {
    let client = pool.get().await.context("failed to get DB connection")?;
    client
        .execute(
            "INSERT INTO brassclaw_secrets_master \
             (tenant_id, version, wrapped_key, algorithm) \
             VALUES ($1, 1, $2, $3) \
             ON CONFLICT (tenant_id, version) DO UPDATE \
             SET wrapped_key = excluded.wrapped_key, \
                 algorithm   = excluded.algorithm, \
                 updated_at  = now()",
            &[&tenant_id, &wrapped_key, &algorithm],
        )
        .await
        .context("failed to upsert brassclaw_secrets_master")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Raw key file zeroing
// ---------------------------------------------------------------------------

/// Overwrite `path` with zero bytes then remove it.
///
/// This minimises the time the key material is on disk in residual form.
fn zero_and_remove(path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write as _;
    let len = std::fs::metadata(path)
        .with_context(|| format!("metadata for {}", path.display()))?
        .len();
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("open {} for zeroing", path.display()))?;
    let zeros = vec![0u8; len as usize];
    f.write_all(&zeros)
        .with_context(|| format!("zero-write to {}", path.display()))?;
    f.flush()
        .with_context(|| format!("flush zero-write to {}", path.display()))?;
    drop(f);
    std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared pool helper
// ---------------------------------------------------------------------------

async fn build_pg_pool() -> anyhow::Result<deadpool_postgres::Pool> {
    crate::commands::config::pg_lifecycle::build_pg_pool().await
}
