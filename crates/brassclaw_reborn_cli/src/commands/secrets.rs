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
    /// Updates `brassclaw_secrets_master` in-place (retaining its current version) — does NOT
    /// generate a new key or re-encrypt existing secret rows.
    ///
    /// KEY-SOURCE RULE: the current database ceremony is authoritative. Raw-key
    /// ceremonies read existing installation key files; a wrapped ceremony
    /// requires its passphrase, even if stale raw files remain. It never
    /// generates a new key when encrypted rows already exist — that would orphan every stored credential.
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
    ///   2. identity.tenant in brassclaw_config DB
    ///   3. "default"
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
        crate::commands::config::pg_lifecycle::with_postgres(&context, |pool| async move {
            brassclaw_pg::migrations::run_migrations(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("schema migrations failed: {e}"))?;

            // Step 2: Resolve the database-owned tenant identity.
            let tenant_id = resolve_tenant_id(self.tenant.as_deref(), &pool).await?;

            // Serialize ceremony changes, including a missing initial row. This
            // lock is transactional and released on every success/error path.
            let mut client = pool.get().await.context("failed to get DB connection")?;
            let transaction = client.transaction().await?;
            transaction
                .query_one(
                    "SELECT pg_advisory_xact_lock(hashtextextended($1, 602))",
                    &[&tenant_id],
                )
                .await
                .context("failed to lock master-key ceremony")?;

            // Step 3: Read the existing master key (key-source rule).
            let (master_key_bytes, version) = resolve_master_key(
                &tenant_id,
                &reborn_home,
                self.old_passphrase_file.as_deref(),
                &transaction,
            )
            .await?;

            // Finish passphrase/crypto validation before changing any file.
            let (wrapped_key, algorithm) = wrap_master_key(&master_key_bytes, &strategy)?;

            // Step 4a: For raw-key strategy, write the hex key to the canonical
            // on-disk file BEFORE the DB upsert.  If the upsert fails, the key
            // file is still present and the operator can retry.
            if matches!(strategy, RewrapStrategy::RawKey) {
                let key_path = reborn_home.join(".secrets-master-key");
                write_raw_master_key(&key_path, &master_key_bytes)?;
            }

            // Step 5: Commit the exact ceremony while holding the lock.
            upsert_secrets_master(&transaction, &tenant_id, version, &wrapped_key, &algorithm).await?;
            transaction.commit().await?;

            // Step 6: After a successful passphrase-wrap upsert, zero and remove any
            // raw key files that were read in step 3 (they are superseded by the DB
            // row).  For raw-key strategy we just wrote the canonical file, so there
            // is nothing to remove.  Only zero files that existed before this run.
            if algorithm != "raw-key-on-disk" {
                // Commit must precede deletion, but another ceremony may have
                // acquired the lock after commit. Reacquire it and check the
                // exact current row before removing material another command
                // could now require. Cleanup failures leave the committed wrap.
                let cleanup = client.transaction().await?;
                cleanup
                    .query_one(
                        "SELECT pg_advisory_xact_lock(hashtextextended($1, 602))",
                        &[&tenant_id],
                    )
                    .await?;
                let current = cleanup
                    .query_one(
                        "SELECT wrapped_key, algorithm FROM brassclaw_secrets_master \
                         WHERE tenant_id = $1 AND version = $2 FOR UPDATE",
                        &[&tenant_id, &version],
                    )
                    .await?;
                if current.try_get::<_, String>(0)? != wrapped_key
                    || current.try_get::<_, String>(1)? != algorithm
                {
                    anyhow::bail!("master-key ceremony changed after commit; raw files were retained, inspect the current ceremony before retrying");
                }
                let legacy = reborn_home.join(".reborn-local-dev-secrets-master-key");
                let canonical = reborn_home.join(".secrets-master-key");
                for path in &[&legacy, &canonical] {
                    if let Some(key) = brassclaw_reborn_composition::secrets_master::read_raw_master_key(path)? {
                        if key != master_key_bytes {
                            anyhow::bail!("wrapped ceremony committed; raw key file {} differs and was retained for operator reconciliation", path.display());
                        }
                        zero_and_remove(path).with_context(|| {
                            format!("failed to zero raw key file {}", path.display())
                        })?;
                        println!("Zeroed and removed: {}", path.display());
                    }
                }
                cleanup.commit().await?;
            }

            println!("brassclaw secrets rewrap: tenant={tenant_id} strategy={algorithm} — done.");
            Ok(())
        })
        .await
    }

    #[cfg(not(feature = "postgres"))]
    async fn run_async(self, _context: RebornCliContext) -> anyhow::Result<()> {
        anyhow::bail!("`brassclaw secrets rewrap` requires the `postgres` feature")
    }
}

// ---------------------------------------------------------------------------
// Database-owned identity resolution
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
async fn resolve_tenant_id(
    flag: Option<&str>,
    pool: &deadpool_postgres::Pool,
) -> anyhow::Result<String> {
    let tenant = if let Some(tenant) = flag {
        tenant.to_owned()
    } else {
        let snapshot =
            brassclaw_reborn_composition::db_config::load_config_snapshot(pool, "default")
                .await
                .context("failed to resolve master-key identity from PostgreSQL")?;
        snapshot
            .identity
            .and_then(|identity| identity.tenant)
            .unwrap_or_else(|| "default".to_owned())
    };
    brassclaw_reborn_composition::host_api::TenantId::new(&tenant)
        .context("invalid master-key tenant identity")?;
    Ok(tenant)
}

// ---------------------------------------------------------------------------
// Master key source resolution (key-source rule)
// ---------------------------------------------------------------------------

/// Resolve the raw plaintext master key bytes.
///
/// Priority:
///  1. If `brassclaw_secrets_master` already has a row with algorithm =
///     `aes256gcm-argon2id` → unwrap with the old passphrase (passphrase-change
///     path).
///  2. A raw ceremony or a not-yet-initialized DB reads the existing key files.
///     Canonical and legacy files must agree; unknown ceremonies remain errors.
///  3. If neither file nor DB row exists AND no encrypted rows are present →
///     generate a fresh 32-byte random key (fresh-install path).
///  4. Fail-closed: if neither file nor DB row and encrypted rows DO exist.
#[cfg(feature = "postgres")]
async fn resolve_master_key(
    tenant_id: &str,
    reborn_home: &std::path::Path,
    old_passphrase_file: Option<&std::path::Path>,
    client: &deadpool_postgres::Transaction<'_>,
) -> anyhow::Result<(Vec<u8>, i32)> {
    use secrecy::ExposeSecret as _;

    let legacy = reborn_home.join(".reborn-local-dev-secrets-master-key");
    let canonical = reborn_home.join(".secrets-master-key");
    // A successful wrapped DB ceremony wins over stale disk material left by
    // interrupted cleanup. Never use such a file to bypass passphrase validation.
    let row = client
        .query_opt(
            "SELECT wrapped_key, algorithm, version FROM brassclaw_secrets_master \
             WHERE tenant_id = $1 ORDER BY version DESC LIMIT 1 FOR UPDATE",
            &[&tenant_id],
        )
        .await
        .context("failed to query brassclaw_secrets_master")?;
    let version = row
        .as_ref()
        .map(|row| row.try_get::<_, i32>(2))
        .transpose()?
        .unwrap_or(1);
    anyhow::ensure!(version > 0, "master-key version must be positive");
    let raw_ceremony = row.is_some();
    if let Some(r) = row {
        let wrapped_key: String = r.try_get(0)?;
        let algorithm: String = r.try_get(1)?;

        if algorithm == "aes256gcm-argon2id" {
            // Passphrase-change path: read old passphrase and unwrap.
            let old_passphrase = read_old_passphrase(old_passphrase_file)?;
            let key = brassclaw_reborn_composition::secrets_master::unwrap_master_key_argon2id(
                &wrapped_key,
                old_passphrase.expose_secret(),
            )
            .map_err(|e| anyhow::anyhow!("failed to unwrap current master key: {e}"))?;
            return Ok((key, version));
        }

        if algorithm != "raw-key-on-disk" {
            anyhow::bail!(
                "unsupported master-key ceremony '{algorithm}'; existing key was not changed"
            );
        }
    }

    let mut selected = None;
    for path in [&canonical, &legacy] {
        if let Some(key) = brassclaw_reborn_composition::secrets_master::read_raw_master_key(path)?
        {
            if selected.as_ref().is_some_and(|selected| selected != &key) {
                anyhow::bail!(
                    "canonical and legacy master-key files disagree; reconcile the original key before rewrapping"
                );
            }
            selected = Some(key);
        }
    }
    if let Some(key) = selected {
        return Ok((key, version));
    }
    if raw_ceremony {
        anyhow::bail!(
            "master-key ceremony requires an existing raw key file; restore the original file before retrying"
        );
    }

    // No DB row and no raw key file.  If encrypted rows exist, fail closed.
    let encrypted_rows = count_encrypted_rows(client, tenant_id).await?;
    if encrypted_rows > 0 {
        anyhow::bail!(
            "raw key file not found but encrypted rows exist — cannot generate new key; \
             restore the original key file first"
        );
    }

    // Truly fresh install: generate a new 32-byte key.
    brassclaw_reborn_composition::secrets_master::generate_master_key_bytes()
        .map(|key| (key, version))
        .map_err(Into::into)
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
    client: &deadpool_postgres::Transaction<'_>,
    tenant_id: &str,
) -> anyhow::Result<i64> {
    // Migrations have completed before this check. Any missing table or query
    // failure must abort; it is never evidence that encrypted data is absent.
    // brassclaw_secrets
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_secrets WHERE tenant_id = $1",
            &[&tenant_id],
        )
        .await
        .context("failed to count brassclaw_secrets rows")?;
    let secrets_count: i64 = row.try_get(0)?;

    let fs_row = client
        .query_one(
            "SELECT COUNT(*) FROM brassclaw_root_filesystem WHERE tenant_id = $1 \
             AND contents IS NOT NULL AND kind = 'encrypted'",
            &[&tenant_id],
        )
        .await
        .context("failed to count encrypted filesystem rows")?;
    let fs_count: i64 = fs_row.try_get(0)?;
    secrets_count
        .checked_add(fs_count)
        .context("encrypted row count overflow")
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
    client: &deadpool_postgres::Transaction<'_>,
    tenant_id: &str,
    version: i32,
    wrapped_key: &str,
    algorithm: &str,
) -> anyhow::Result<()> {
    client
        .execute(
            "INSERT INTO brassclaw_secrets_master \
             (tenant_id, version, wrapped_key, algorithm) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (tenant_id, version) DO UPDATE \
             SET wrapped_key = excluded.wrapped_key, \
                 algorithm   = excluded.algorithm, \
                 updated_at  = now()",
            &[&tenant_id, &version, &wrapped_key, &algorithm],
        )
        .await
        .context("failed to upsert brassclaw_secrets_master")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Raw key file zeroing
// ---------------------------------------------------------------------------

/// Persist new raw material without overwriting an existing key. Restrict the
/// empty temporary file before writing any secret and publish only after fsync.
#[cfg(feature = "postgres")]
fn write_raw_master_key(path: &std::path::Path, key: &[u8]) -> anyhow::Result<()> {
    use std::io::Write as _;
    if let Some(existing) = brassclaw_reborn_composition::secrets_master::read_raw_master_key(path)?
    {
        if existing != key {
            anyhow::bail!("existing canonical master key differs; it was not overwritten");
        }
        restrict_key_file(path)?;
        return Ok(());
    }
    let parent = path.parent().context("master key path requires a parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .context("failed to create private master-key temporary file")?;
    restrict_key_file(temporary.path())?;
    writeln!(temporary, "{}", hex::encode(key))?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path).map_err(|error| {
        anyhow::anyhow!(
            "failed to publish master key without replacing an existing file: {}",
            error.error
        )
    })?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(feature = "postgres")]
fn restrict_key_file(path: &std::path::Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    {
        // Restrict the empty file before writing. No shell interpolation and no
        // guessed domain/account: whoami supplies the current Windows identity.
        let identity = std::process::Command::new("whoami").output()?;
        anyhow::ensure!(
            identity.status.success(),
            "cannot resolve Windows key owner"
        );
        let account = std::str::from_utf8(&identity.stdout)?.trim();
        anyhow::ensure!(!account.is_empty(), "Windows key owner is empty");
        let permissions = std::process::Command::new("icacls")
            .arg(path)
            .arg("/inheritance:r")
            .arg("/grant:r")
            .arg(format!("{account}:F"))
            .output()?;
        anyhow::ensure!(
            permissions.status.success(),
            "cannot restrict Windows key permissions"
        );
    }
    Ok(())
}

/// Overwrite `path` with zero bytes then remove it.
///
/// This does not guarantee physical erasure on snapshots, SSDs or copy-on-write
/// filesystems. The wrapped DB ceremony has already committed before cleanup.
fn zero_and_remove(path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write as _;
    let len = std::fs::metadata(path)
        .with_context(|| format!("metadata for {}", path.display()))?
        .len();
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("open {} for zeroing", path.display()))?;
    let zeros = [0u8; 4096];
    let mut remaining = len;
    while remaining != 0 {
        let count = remaining.min(zeros.len() as u64) as usize;
        f.write_all(&zeros[..count])
            .with_context(|| format!("zero-write to {}", path.display()))?;
        remaining -= count as u64;
    }
    f.sync_all()
        .with_context(|| format!("flush zero-write to {}", path.display()))?;
    drop(f);
    std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
    Ok(())
}
