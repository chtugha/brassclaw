//! Per-boot master-key resolution from `brassclaw_secrets_master` (§4.4).
//!
//! This module owns the ceremony-selector logic:
//!
//! - `algorithm = 'raw-key-on-disk'`   → read key from `$REBORN_HOME/.secrets-master-key`
//! - `algorithm = 'aes256gcm-argon2id'` → unwrap `wrapped_key` using the passphrase
//!   read from (in priority order):
//!     1. `$CREDENTIALS_DIRECTORY/secrets-passphrase` (systemd `LoadCredential`)
//!     2. `BRASSCLAW_SECRETS_PASSPHRASE_FILE` env var (path to a file)
//!
//! On a fresh install where no row exists yet (`boot.initialized` absent), this
//! returns `Ok(None)` so the first-run wizard can take over. Once a row exists,
//! missing ceremony inputs are a hard error (fail-closed).

use std::path::Path;

use base64::Engine as _;
use brassclaw_pg::PgPool;
use secrecy::SecretString;
use thiserror::Error;

/// Env var pointing to a file holding the passphrase for the production
/// `aes256gcm-argon2id` ceremony (systemd `EnvironmentFile` or operator-set).
pub const SECRETS_PASSPHRASE_FILE_ENV: &str = "BRASSCLAW_SECRETS_PASSPHRASE_FILE";

/// Result of resolving the master key at boot time.
pub enum ResolvedMasterKey {
    /// Key was resolved — proceed normally.
    Key(SecretString),
    /// No `brassclaw_secrets_master` row exists for this tenant yet
    /// (fresh install / first-run wizard not yet run). Boot should
    /// continue to the first-run wizard.
    NotYetInitialized,
}

#[derive(Debug, Error)]
pub enum MasterKeyResolveError {
    #[error("database error resolving master key: {reason}")]
    Db { reason: String },

    #[error(
        "master key is passphrase-wrapped but BRASSCLAW_SECRETS_PASSPHRASE_FILE is not set. \
         Set the env var or run 'brassclaw secrets rewrap --strategy raw-key' to revert."
    )]
    PassphraseFileNotSet,

    #[error("failed to read passphrase file '{path}': {reason}")]
    PassphraseFileRead { path: String, reason: String },

    #[error("failed to read raw key file '{path}': {reason}")]
    RawKeyFileRead { path: String, reason: String },

    #[error(
        "master key uses algorithm '{algorithm}' which is not supported by this version; \
         run 'brassclaw secrets rewrap --strategy passphrase-file=<path>' to re-wrap."
    )]
    UnknownAlgorithm { algorithm: String },

    #[error("AES-256-GCM key unwrap failed: {reason}")]
    Unwrap { reason: String },

    #[error("base64 decode error for wrapped_key: {reason}")]
    Base64 { reason: String },

    #[error("master key material is invalid: {reason}")]
    InvalidKey { reason: String },
}

impl From<deadpool_postgres::PoolError> for MasterKeyResolveError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        Self::Db { reason: e.to_string() }
    }
}

impl From<tokio_postgres::Error> for MasterKeyResolveError {
    fn from(e: tokio_postgres::Error) -> Self {
        Self::Db { reason: e.to_string() }
    }
}

/// Resolve the AES-256 master key for `tenant_id` from `brassclaw_secrets_master`.
///
/// Returns `Ok(ResolvedMasterKey::NotYetInitialized)` when no row exists yet
/// (fresh install); the caller should skip the ceremony check and let the
/// first-run wizard proceed.
///
/// Returns `Ok(ResolvedMasterKey::Key(k))` on success.
///
/// Returns `Err` on any ceremony mismatch, missing passphrase, or I/O failure.
pub async fn resolve_pg_master_key(
    pool: &PgPool,
    tenant_id: &str,
    reborn_home: &Path,
) -> Result<ResolvedMasterKey, MasterKeyResolveError> {
    let client = pool.get().await?;

    // Load the current row (version=1 — the active version row).
    let row = client
        .query_opt(
            "SELECT wrapped_key, algorithm FROM brassclaw_secrets_master \
             WHERE tenant_id = $1 ORDER BY version DESC LIMIT 1",
            &[&tenant_id],
        )
        .await?;

    let (wrapped_key, algorithm) = match row {
        None => return Ok(ResolvedMasterKey::NotYetInitialized),
        Some(r) => {
            let wk: String = r.get(0);
            let alg: String = r.get(1);
            (wk, alg)
        }
    };

    match algorithm.as_str() {
        "raw-key-on-disk" => {
            // Log (debug) if BRASSCLAW_SECRETS_PASSPHRASE_FILE is also set (stale env).
            if passphrase_file_path().is_some() {
                tracing::debug!(
                    "BRASSCLAW_SECRETS_PASSPHRASE_FILE is set but master key is not wrapped; \
                     the env var will be ignored. Run \
                     'brassclaw secrets rewrap --strategy passphrase-file=<path>' \
                     to switch to passphrase ceremony."
                );
            }
            // Read key from canonical raw-key file.
            let key_path = reborn_home.join(".secrets-master-key");
            let hex = std::fs::read_to_string(&key_path).map_err(|e| {
                MasterKeyResolveError::RawKeyFileRead {
                    path: key_path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
            Ok(ResolvedMasterKey::Key(SecretString::from(hex.trim().to_string())))
        }

        "aes256gcm-argon2id" => {
            // Passphrase-wrapped ceremony.
            let passphrase = read_passphrase()?;
            let plaintext_key = unwrap_key_argon2id(&wrapped_key, &passphrase)?;
            Ok(ResolvedMasterKey::Key(SecretString::from(
                hex::encode(plaintext_key),
            )))
        }

        other => Err(MasterKeyResolveError::UnknownAlgorithm {
            algorithm: other.to_string(),
        }),
    }
}

/// Read the Argon2id passphrase from the correct source, in priority order:
///
/// 1. `$CREDENTIALS_DIRECTORY/secrets-passphrase` (systemd `LoadCredential`)
/// 2. `BRASSCLAW_SECRETS_PASSPHRASE_FILE` env var (path to a file)
fn read_passphrase() -> Result<String, MasterKeyResolveError> {
    // 1. systemd LoadCredential: $CREDENTIALS_DIRECTORY/secrets-passphrase
    if let Ok(cred_dir) = std::env::var("CREDENTIALS_DIRECTORY") {
        let cred_path = std::path::Path::new(&cred_dir).join("secrets-passphrase");
        if cred_path.exists() {
            return std::fs::read_to_string(&cred_path)
                .map(|s| s.trim().to_string())
                .map_err(|e| MasterKeyResolveError::PassphraseFileRead {
                    path: cred_path.display().to_string(),
                    reason: e.to_string(),
                });
        }
    }

    // 2. BRASSCLAW_SECRETS_PASSPHRASE_FILE env var
    let path = passphrase_file_path()
        .ok_or(MasterKeyResolveError::PassphraseFileNotSet)?;
    std::fs::read_to_string(&path)
        .map(|s| s.trim().to_string())
        .map_err(|e| MasterKeyResolveError::PassphraseFileRead {
            path: path.to_string_lossy().to_string(),
            reason: e.to_string(),
        })
}

/// Returns the path from `BRASSCLAW_SECRETS_PASSPHRASE_FILE` if present and
/// non-empty, otherwise `None`.
fn passphrase_file_path() -> Option<std::path::PathBuf> {
    std::env::var(SECRETS_PASSPHRASE_FILE_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(std::path::PathBuf::from)
}

/// Public wrapper so the `secrets rewrap` CLI command can unwrap an existing
/// passphrase-wrapped key during a passphrase-change operation.
pub fn unwrap_master_key_argon2id(
    wrapped_key_b64: &str,
    passphrase: &str,
) -> Result<Vec<u8>, MasterKeyResolveError> {
    unwrap_key_argon2id(wrapped_key_b64, passphrase)
}

/// Wrap `master_key_bytes` with an Argon2id-derived AES-256-GCM key.
///
/// Returns `base64(salt[32] || nonce[12] || ciphertext)` suitable for storing
/// in `brassclaw_secrets_master.wrapped_key`.  Key derivation parameters match
/// [`unwrap_key_argon2id`] (m=65536, t=3, p=1).
pub fn wrap_master_key_argon2id(
    master_key_bytes: &[u8],
    passphrase: &str,
) -> Result<String, MasterKeyResolveError> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

    // Generate a random 32-byte salt and 12-byte nonce.
    let mut salt = [0u8; 32];
    let mut nonce_bytes = [0u8; 12];
    use std::io::Read as _;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| {
            f.read_exact(&mut salt)?;
            f.read_exact(&mut nonce_bytes)
        })
        .map_err(|e| MasterKeyResolveError::Unwrap {
            reason: format!("failed to read random bytes: {e}"),
        })?;

    let wrapping_key = derive_wrapping_key(passphrase.as_bytes(), &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&wrapping_key).map_err(|e| {
        MasterKeyResolveError::Unwrap { reason: e.to_string() }
    })?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, master_key_bytes).map_err(|e| {
        MasterKeyResolveError::Unwrap { reason: format!("AES-GCM encrypt: {e}") }
    })?;

    // Layout: salt[32] || nonce[12] || ciphertext
    let mut blob = Vec::with_capacity(32 + 12 + ciphertext.len());
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(B64.encode(&blob))
}

/// Unwrap an AES-256-GCM key from `base64(salt[32] || nonce[12] || ciphertext)` using an
/// Argon2id-derived wrapping key.
///
/// Key derivation parameters must match the `rewrap` command that wrote the row.
fn unwrap_key_argon2id(
    wrapped_key_b64: &str,
    passphrase: &str,
) -> Result<Vec<u8>, MasterKeyResolveError> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

    let blob = B64.decode(wrapped_key_b64).map_err(|e| {
        MasterKeyResolveError::Base64 { reason: e.to_string() }
    })?;

    // Layout: salt[32] || nonce[12] || ciphertext
    const SALT_LEN: usize = 32;
    const NONCE_LEN: usize = 12;
    if blob.len() < SALT_LEN + NONCE_LEN + 16 {
        return Err(MasterKeyResolveError::Unwrap {
            reason: format!("wrapped_key blob too short: {} bytes", blob.len()),
        });
    }

    let salt = &blob[..SALT_LEN];
    let nonce_bytes = &blob[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &blob[SALT_LEN + NONCE_LEN..];

    // Derive wrapping key: Argon2id, params matching rewrap.
    let wrapping_key = derive_wrapping_key(passphrase.as_bytes(), salt)?;

    let cipher = Aes256Gcm::new_from_slice(&wrapping_key).map_err(|e| {
        MasterKeyResolveError::Unwrap { reason: e.to_string() }
    })?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
        MasterKeyResolveError::Unwrap { reason: format!("AES-GCM decrypt: {e}") }
    })?;

    Ok(plaintext)
}

/// Argon2id key derivation for the master-key wrapping cipher.
///
/// Parameters must match those used in `brassclaw secrets rewrap`:
/// m=65536, t=3, p=1 (OWASP minimum for interactive).
fn derive_wrapping_key(
    passphrase: &[u8],
    salt: &[u8],
) -> Result<[u8; 32], MasterKeyResolveError> {
    use argon2::{Argon2, Params, Version, password_hash::SaltString};

    let params = Params::new(65536, 3, 1, Some(32)).map_err(|e| {
        MasterKeyResolveError::Unwrap { reason: format!("argon2 params: {e}") }
    })?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    let salt_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(salt);
    let salt_str = SaltString::from_b64(&salt_b64).map_err(|e| {
        MasterKeyResolveError::Unwrap { reason: format!("argon2 salt: {e}") }
    })?;
    let mut output = [0u8; 32];
    argon2
        .hash_password_into(passphrase, salt_str.as_str().as_bytes(), &mut output)
        .map_err(|e| MasterKeyResolveError::Unwrap { reason: format!("argon2 hash: {e}") })?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passphrase_file_path_absent_when_env_unset() {
        // Guard: unset the env var before testing.
        let _guard = EnvGuard::unset(SECRETS_PASSPHRASE_FILE_ENV);
        assert!(passphrase_file_path().is_none());
    }

    #[test]
    fn passphrase_file_path_absent_when_env_empty() {
        let _guard = EnvGuard::set(SECRETS_PASSPHRASE_FILE_ENV, "");
        assert!(passphrase_file_path().is_none());
    }

    #[test]
    fn passphrase_file_path_present_when_env_set() {
        let _guard = EnvGuard::set(SECRETS_PASSPHRASE_FILE_ENV, "/run/secrets/passphrase");
        let path = passphrase_file_path().expect("should be Some");
        assert_eq!(path.to_string_lossy(), "/run/secrets/passphrase");
    }

    /// Round-trip: wrap a known key with a passphrase, then unwrap it and
    /// verify the plaintext matches.  This exercises both the Argon2id KDF
    /// and the AES-256-GCM encrypt/decrypt path end-to-end.
    #[test]
    fn wrap_unwrap_round_trip() {
        let master_key = b"this-is-a-32-byte-test-master-ke"; // 32 bytes
        let passphrase = "correct-horse-battery-staple";

        let wrapped = wrap_master_key_argon2id(master_key, passphrase)
            .expect("wrap should succeed");

        let recovered = unwrap_master_key_argon2id(&wrapped, passphrase)
            .expect("unwrap should succeed");

        assert_eq!(recovered, master_key, "recovered key must equal original");
    }

    /// Wrong passphrase must fail decryption (AES-GCM authentication tag fails).
    #[test]
    fn unwrap_wrong_passphrase_fails() {
        let master_key = b"this-is-a-32-byte-test-master-ke";
        let wrapped = wrap_master_key_argon2id(master_key, "correct-passphrase")
            .expect("wrap should succeed");

        let result = unwrap_master_key_argon2id(&wrapped, "wrong-passphrase");
        assert!(result.is_err(), "decryption with wrong passphrase must fail");
    }

    /// Two calls to wrap produce different blobs (different random salt/nonce),
    /// but both unwrap to the same plaintext key.
    #[test]
    fn wrap_is_non_deterministic() {
        let master_key = b"this-is-a-32-byte-test-master-ke";
        let passphrase = "same-passphrase";

        let wrapped1 = wrap_master_key_argon2id(master_key, passphrase)
            .expect("first wrap should succeed");
        let wrapped2 = wrap_master_key_argon2id(master_key, passphrase)
            .expect("second wrap should succeed");

        assert_ne!(wrapped1, wrapped2, "each wrap must produce a unique ciphertext");

        let key1 = unwrap_master_key_argon2id(&wrapped1, passphrase).expect("unwrap 1");
        let key2 = unwrap_master_key_argon2id(&wrapped2, passphrase).expect("unwrap 2");
        assert_eq!(key1, master_key);
        assert_eq!(key2, master_key);
    }

    struct EnvGuard {
        key: &'static str,
        prev: Option<String>,
    }
    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prev = std::env::var(key).ok();
            // SAFETY: single-threaded test context (`cargo test` runs each
            // `#[test]` fn sequentially within the same thread group); no other
            // thread reads this env var concurrently.
            unsafe { std::env::set_var(key, value) };
            Self { key, prev }
        }
        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            // SAFETY: same single-threaded test context as `set`.
            unsafe { std::env::remove_var(key) };
            Self { key, prev }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: restoring the env var on drop; same single-threaded test context.
            unsafe {
                match &self.prev {
                    Some(v) => std::env::set_var(self.key, v),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }
}
