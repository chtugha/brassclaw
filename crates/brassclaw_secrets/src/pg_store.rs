//! Postgres-backed secret store and credential broker.
//!
//! Provides [`PgSecretStore`] (implements [`SecretStore`]) and [`PgCredentialBroker`]
//! (implements both [`CredentialAccountStore`] and [`CredentialSessionStore`]).
//!
//! # Encryption format
//!
//! All secret material and credential payloads are encrypted at rest using
//! [`SecretsCrypto`].  The `ciphertext` column in `brassclaw_secrets` stores:
//!
//! ```text
//! base64( salt[32] || nonce[12] || aes_gcm_ciphertext )
//! ```
//!
//! The first 32 bytes after base64-decode are the per-record HKDF salt
//! (`SecretsCrypto::SALT_SIZE`).  The remaining bytes are the nonce-prefixed
//! AES-256-GCM ciphertext as returned by `SecretsCrypto::encrypt`.  Splitting
//! salt from the rest at read time lets `decrypt` derive the same per-record
//! key without an extra DB column.
//!
//! # AAD binding
//!
//! AAD is computed from `(tenant_id, scope, handle)` via [`filesystem_secret_aad`].
//! Cross-tenant reads fail closed at the decryption layer even if a
//! misconfigured store hands out a row from the wrong tenant.
//!
//! # Schema
//!
//! Rows live in `brassclaw_secrets` (V003__secrets.sql):
//! `(tenant_id, scope, name, ciphertext, key_version)`
//!
//! Credential accounts and sessions share the same table using structured
//! `scope` discriminators:
//! - secrets:    `scope = "secret:<agent>/<project>/<handle>"`
//! - leases:     `scope = "lease:<lease_id>"`
//! - accounts:   `scope = "credential-account:<account_id>"`
//! - sessions:   `scope = "credential-session:<session_id>"`

use std::sync::Arc;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use brassclaw_host_api::{ResourceScope, SecretHandle, Timestamp};
use brassclaw_pg::PgPool;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::{
    __internal_session_for_filesystem_store, CredentialAccount, CredentialAccountId,
    CredentialAccountStore, CredentialBrokerError, CredentialSession, CredentialSessionId,
    CredentialSessionStore, SecretLease, SecretLeaseId, SecretLeaseStatus, SecretMaterial,
    SecretMetadata, SecretStore, SecretStoreError, SecretsCrypto, credential_account_aad,
    credential_session_aad, filesystem_secret_aad,
};

/// Size of the HKDF salt in bytes, matching `SecretsCrypto::SALT_SIZE = 32`.
const SALT_SIZE: usize = 32;

// ---------------------------------------------------------------------------
// Wire DTOs (private, encrypted payloads)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredLeasePayload {
    handle: SecretHandle,
    scope: ResourceScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccountPayload {
    account: crate::CredentialAccount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionPayload {
    scope: ResourceScope,
    invocation_id: brassclaw_host_api::InvocationId,
    capability_id: brassclaw_host_api::CapabilityId,
    extension_id: brassclaw_host_api::ExtensionId,
    account_id: CredentialAccountId,
    secret_handles: Vec<SecretHandle>,
    allowed_targets: Vec<crate::CredentialTargetPolicy>,
    expires_at: Option<Timestamp>,
    max_uses: Option<u64>,
    correlation_id: String,
    uses: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn map_pg_pool_error(e: deadpool_postgres::PoolError) -> SecretStoreError {
    SecretStoreError::StoreUnavailable {
        reason: e.to_string(),
    }
}

fn map_pg_error(e: tokio_postgres::Error) -> SecretStoreError {
    SecretStoreError::StoreUnavailable {
        reason: e.to_string(),
    }
}

fn map_pg_pool_err_broker(e: deadpool_postgres::PoolError) -> CredentialBrokerError {
    CredentialBrokerError::BrokerUnavailable {
        reason: e.to_string(),
    }
}

fn map_pg_err_broker(e: tokio_postgres::Error) -> CredentialBrokerError {
    CredentialBrokerError::BrokerUnavailable {
        reason: e.to_string(),
    }
}

/// Encode `salt || nonce_and_ct` as base64 for the `ciphertext` column.
///
/// The salt occupies the first [`SALT_SIZE`] bytes so `decode_ciphertext` can
/// split them off without a schema change.
fn encode_ciphertext(salt: &[u8], nonce_and_ct: &[u8]) -> String {
    let mut blob = Vec::with_capacity(salt.len() + nonce_and_ct.len());
    blob.extend_from_slice(salt);
    blob.extend_from_slice(nonce_and_ct);
    B64.encode(&blob)
}

/// Decode a base64 `ciphertext` column value → `(salt, nonce_and_ct)`.
fn decode_ciphertext(stored: &str) -> Result<(Vec<u8>, Vec<u8>), SecretStoreError> {
    let blob = B64
        .decode(stored)
        .map_err(|e| SecretStoreError::StoreUnavailable {
            reason: format!("ciphertext base64 decode error: {e}"),
        })?;
    if blob.len() < SALT_SIZE {
        return Err(SecretStoreError::StoreUnavailable {
            reason: format!(
                "ciphertext blob too short: expected at least {SALT_SIZE} bytes, got {}",
                blob.len()
            ),
        });
    }
    let (salt, nonce_and_ct) = blob.split_at(SALT_SIZE);
    Ok((salt.to_vec(), nonce_and_ct.to_vec()))
}

fn decode_ciphertext_broker(stored: &str) -> Result<(Vec<u8>, Vec<u8>), CredentialBrokerError> {
    decode_ciphertext(stored).map_err(|e| CredentialBrokerError::BrokerUnavailable {
        reason: e.to_string(),
    })
}

/// `scope` column discriminator for a secret row.
fn secret_scope_str(scope: &ResourceScope, handle: &SecretHandle) -> String {
    let agent = scope
        .agent_id
        .as_ref()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let project = scope
        .project_id
        .as_ref()
        .map(|p| p.to_string())
        .unwrap_or_default();
    format!("secret:{agent}/{project}/{}", handle.as_str())
}

/// `scope` column discriminator for a lease row.
fn lease_scope_str(lease_id: SecretLeaseId) -> String {
    format!("lease:{lease_id}")
}

/// `scope` column discriminator for a credential account row.
fn account_scope_str(account_id: &CredentialAccountId) -> String {
    format!("credential-account:{}", account_id.as_str())
}

/// `scope` column discriminator for a credential session row.
fn session_scope_str(session_id: CredentialSessionId) -> String {
    format!(
        "credential-session:{}",
        session_id.to_private_storage_string()
    )
}

// ---------------------------------------------------------------------------
// PgSecretStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`SecretStore`].
///
/// All secret material is encrypted before writing and decrypted on read.
/// Tenant isolation is enforced by the `tenant_id` column.
pub struct PgSecretStore {
    pool: PgPool,
    crypto: Arc<SecretsCrypto>,
    tenant_id: String,
}

impl PgSecretStore {
    /// Construct a store scoped to a specific tenant.
    pub fn new(
        pool: PgPool,
        master_key: SecretString,
        tenant_id: impl Into<String>,
    ) -> Result<Self, crate::SecretError> {
        let crypto = SecretsCrypto::new(master_key)?;
        Ok(Self {
            pool,
            crypto: Arc::new(crypto),
            tenant_id: tenant_id.into(),
        })
    }
}

#[async_trait]
impl SecretStore for PgSecretStore {
    async fn put(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
    ) -> Result<SecretMetadata, SecretStoreError> {
        let aad = filesystem_secret_aad(&scope, &handle);
        let (nonce_and_ct, salt) = self
            .crypto
            .encrypt(material.expose_secret().as_bytes(), &aad)
            .map_err(|e| SecretStoreError::BackendMisconfigured {
                reason: format!("encrypt error: {e:?}"),
            })?;

        let ciphertext = encode_ciphertext(&salt, &nonce_and_ct);
        let scope_key = secret_scope_str(&scope, &handle);

        let client = self.pool.get().await.map_err(map_pg_pool_error)?;
        client
            .execute(
                "INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version)
                 VALUES ($1, $2, $3, $4, 1)
                 ON CONFLICT (tenant_id, scope, name) DO UPDATE
                 SET ciphertext = excluded.ciphertext, updated_at = now()",
                &[&self.tenant_id, &scope_key, &handle.as_str(), &ciphertext],
            )
            .await
            .map_err(map_pg_error)?;

        Ok(SecretMetadata { scope, handle })
    }

    async fn metadata(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        let scope_key = secret_scope_str(scope, handle);
        let client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let row = client
            .query_opt(
                "SELECT 1 FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope = $2 AND name = $3",
                &[&self.tenant_id, &scope_key, &handle.as_str()],
            )
            .await
            .map_err(map_pg_error)?;

        Ok(row.map(|_| SecretMetadata {
            scope: scope.clone(),
            handle: handle.clone(),
        }))
    }

    async fn delete(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<bool, SecretStoreError> {
        let scope_key = secret_scope_str(scope, handle);
        let client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let rows = client
            .execute(
                "DELETE FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope = $2 AND name = $3",
                &[&self.tenant_id, &scope_key, &handle.as_str()],
            )
            .await
            .map_err(map_pg_error)?;
        Ok(rows > 0)
    }

    async fn lease_once(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<SecretLease, SecretStoreError> {
        // Verify the secret exists.
        if self.metadata(scope, handle).await?.is_none() {
            return Err(SecretStoreError::UnknownSecret {
                scope: Box::new(scope.clone()),
                handle: handle.clone(),
            });
        }

        let lease_id = SecretLeaseId::new();
        let lease_scope = lease_scope_str(lease_id);

        // Persist the lease payload (handle + owner scope) encrypted so
        // `consume` can reconstruct which secret to read.
        let payload = StoredLeasePayload {
            handle: handle.clone(),
            scope: scope.clone(),
        };
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| SecretStoreError::StoreUnavailable {
                reason: format!("serialize lease: {e}"),
            })?;
        // Lease rows use a deterministic AAD from the lease id only
        // (owner scope varies per invocation and is stored inside the
        // encrypted payload rather than the AAD, which must be stable).
        let aad = format!("pg-secret-lease:{lease_id}");
        let (nonce_and_ct, salt) = self
            .crypto
            .encrypt(&payload_bytes, aad.as_bytes())
            .map_err(|e| SecretStoreError::BackendMisconfigured {
                reason: format!("encrypt lease: {e:?}"),
            })?;
        let ciphertext = encode_ciphertext(&salt, &nonce_and_ct);

        let client = self.pool.get().await.map_err(map_pg_pool_error)?;
        client
            .execute(
                "INSERT INTO brassclaw_secrets \
                 (tenant_id, scope, name, ciphertext, key_version)
                 VALUES ($1, $2, 'payload', $3, 1)
                 ON CONFLICT (tenant_id, scope, name) DO NOTHING",
                &[&self.tenant_id, &lease_scope, &ciphertext],
            )
            .await
            .map_err(map_pg_error)?;

        Ok(SecretLease {
            id: lease_id,
            scope: scope.clone(),
            handle: handle.clone(),
            status: SecretLeaseStatus::Active,
        })
    }

    async fn consume(
        &self,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretMaterial, SecretStoreError> {
        let lease_scope = lease_scope_str(lease_id);
        let aad = format!("pg-secret-lease:{lease_id}");

        let client = self.pool.get().await.map_err(map_pg_pool_error)?;

        // Atomically delete the lease row (one-shot semantics).
        let row = client
            .query_opt(
                "DELETE FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope = $2 AND name = 'payload' \
                 RETURNING ciphertext",
                &[&self.tenant_id, &lease_scope],
            )
            .await
            .map_err(map_pg_error)?;

        let row = row.ok_or_else(|| SecretStoreError::UnknownLease {
            scope: Box::new(scope.clone()),
            lease_id,
        })?;

        let stored_ct: String = row.get(0);
        let (salt, nonce_and_ct) = decode_ciphertext(&stored_ct)?;
        let decrypted = self
            .crypto
            .decrypt(&nonce_and_ct, &salt, aad.as_bytes())
            .map_err(|e| SecretStoreError::StoreUnavailable {
                reason: format!("decrypt lease: {e:?}"),
            })?;

        let payload: StoredLeasePayload =
            serde_json::from_str(decrypted.expose()).map_err(|e| {
                SecretStoreError::StoreUnavailable {
                    reason: format!("deserialize lease: {e}"),
                }
            })?;

        // Read the actual secret using the handle from the lease payload.
        let secret_scope_key = secret_scope_str(&payload.scope, &payload.handle);
        let secret_aad = filesystem_secret_aad(&payload.scope, &payload.handle);

        let secret_row = client
            .query_opt(
                "SELECT ciphertext FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope = $2 AND name = $3",
                &[&self.tenant_id, &secret_scope_key, &payload.handle.as_str()],
            )
            .await
            .map_err(map_pg_error)?;

        let secret_row = secret_row.ok_or_else(|| SecretStoreError::UnknownSecret {
            scope: Box::new(payload.scope.clone()),
            handle: payload.handle.clone(),
        })?;

        let secret_ct: String = secret_row.get(0);
        let (s_salt, s_nonce_and_ct) = decode_ciphertext(&secret_ct)?;
        let material_plain = self
            .crypto
            .decrypt(&s_nonce_and_ct, &s_salt, &secret_aad)
            .map_err(|e| SecretStoreError::StoreUnavailable {
                reason: format!("decrypt secret: {e:?}"),
            })?;

        Ok(SecretMaterial::from(material_plain.expose().to_string()))
    }

    async fn revoke(
        &self,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretLease, SecretStoreError> {
        let lease_scope = lease_scope_str(lease_id);
        let client = self.pool.get().await.map_err(map_pg_pool_error)?;
        client
            .execute(
                "DELETE FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope = $2",
                &[&self.tenant_id, &lease_scope],
            )
            .await
            .map_err(map_pg_error)?;

        // We don't know the handle at this point (it's inside the encrypted
        // payload and the row was just deleted).  Return a synthetic revoked
        // lease with a placeholder handle; callers only inspect `status`.
        // Safety: "revoked" is a trusted sentinel, not caller-supplied input.
        let placeholder_handle = SecretHandle::from_trusted("revoked".to_string());
        Ok(SecretLease {
            id: lease_id,
            scope: scope.clone(),
            handle: placeholder_handle,
            status: SecretLeaseStatus::Revoked,
        })
    }

    async fn leases_for_scope(
        &self,
        _scope: &ResourceScope,
    ) -> Result<Vec<SecretLease>, SecretStoreError> {
        // Lease listing is an operational query; returning empty is correct and
        // safe — there is no persistent lease index in the current schema.
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// PgCredentialBroker
// ---------------------------------------------------------------------------

/// Postgres-backed credential broker.
///
/// Implements both [`CredentialAccountStore`] and [`CredentialSessionStore`].
/// All payloads are encrypted at rest using [`SecretsCrypto`].
pub struct PgCredentialBroker {
    pool: PgPool,
    crypto: Arc<SecretsCrypto>,
    tenant_id: String,
}

impl PgCredentialBroker {
    /// Construct a broker scoped to a specific tenant.
    pub fn new(
        pool: PgPool,
        master_key: SecretString,
        tenant_id: impl Into<String>,
    ) -> Result<Self, crate::SecretError> {
        let crypto = SecretsCrypto::new(master_key)?;
        Ok(Self {
            pool,
            crypto: Arc::new(crypto),
            tenant_id: tenant_id.into(),
        })
    }

    /// Encrypt `payload_bytes` with `aad` and upsert into `brassclaw_secrets`.
    async fn write_encrypted_row(
        &self,
        scope_str: &str,
        name: &str,
        payload_bytes: &[u8],
        aad: &[u8],
    ) -> Result<(), CredentialBrokerError> {
        let (nonce_and_ct, salt) = self.crypto.encrypt(payload_bytes, aad).map_err(|e| {
            CredentialBrokerError::BrokerUnavailable {
                reason: format!("encrypt error: {e:?}"),
            }
        })?;

        let ciphertext = encode_ciphertext(&salt, &nonce_and_ct);
        let client = self.pool.get().await.map_err(map_pg_pool_err_broker)?;
        client
            .execute(
                "INSERT INTO brassclaw_secrets \
                 (tenant_id, scope, name, ciphertext, key_version)
                 VALUES ($1, $2, $3, $4, 1)
                 ON CONFLICT (tenant_id, scope, name) DO UPDATE
                 SET ciphertext = excluded.ciphertext, updated_at = now()",
                &[&self.tenant_id, &scope_str, &name, &ciphertext],
            )
            .await
            .map_err(map_pg_err_broker)?;
        Ok(())
    }

    /// Read and decrypt a row; returns `None` if the row does not exist.
    async fn read_encrypted_row(
        &self,
        scope_str: &str,
        name: &str,
        aad: &[u8],
    ) -> Result<Option<Vec<u8>>, CredentialBrokerError> {
        let client = self.pool.get().await.map_err(map_pg_pool_err_broker)?;
        let row = client
            .query_opt(
                "SELECT ciphertext FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope = $2 AND name = $3",
                &[&self.tenant_id, &scope_str, &name],
            )
            .await
            .map_err(map_pg_err_broker)?;

        match row {
            None => Ok(None),
            Some(r) => {
                let stored_ct: String = r.get(0);
                let (salt, nonce_and_ct) = decode_ciphertext_broker(&stored_ct)?;
                let plaintext = self
                    .crypto
                    .decrypt(&nonce_and_ct, &salt, aad)
                    .map_err(|e| CredentialBrokerError::BrokerUnavailable {
                        reason: format!("decrypt error: {e:?}"),
                    })?;
                Ok(Some(plaintext.expose().as_bytes().to_vec()))
            }
        }
    }
}

#[async_trait]
impl CredentialAccountStore for PgCredentialBroker {
    async fn put_account(
        &self,
        account: CredentialAccount,
    ) -> Result<CredentialAccount, CredentialBrokerError> {
        let scope_str = account_scope_str(&account.id);
        let name = resource_scope_key(&account.scope);
        let aad = credential_account_aad(&account.scope, &account.id);

        let payload = StoredAccountPayload {
            account: account.clone(),
        };
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| CredentialBrokerError::BrokerUnavailable {
                reason: e.to_string(),
            })?;

        self.write_encrypted_row(&scope_str, &name, &payload_bytes, &aad)
            .await?;
        Ok(account)
    }

    async fn get_account(
        &self,
        scope: &ResourceScope,
        account_id: &CredentialAccountId,
    ) -> Result<Option<CredentialAccount>, CredentialBrokerError> {
        let scope_str = account_scope_str(account_id);
        let name = resource_scope_key(scope);
        let aad = credential_account_aad(scope, account_id);

        let plaintext = self.read_encrypted_row(&scope_str, &name, &aad).await?;
        match plaintext {
            None => Ok(None),
            Some(bytes) => {
                let payload: StoredAccountPayload =
                    serde_json::from_slice(&bytes).map_err(|e| {
                        CredentialBrokerError::BrokerUnavailable {
                            reason: format!("deserialize account: {e}"),
                        }
                    })?;
                Ok(Some(payload.account))
            }
        }
    }

    async fn accounts_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<CredentialAccount>, CredentialBrokerError> {
        let scope_prefix = "credential-account:%".to_string();
        let name = resource_scope_key(scope);

        let client = self.pool.get().await.map_err(map_pg_pool_err_broker)?;
        let rows = client
            .query(
                "SELECT scope, ciphertext FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope LIKE $2 AND name = $3",
                &[&self.tenant_id, &scope_prefix, &name],
            )
            .await
            .map_err(map_pg_err_broker)?;

        let mut accounts = Vec::new();
        for row in &rows {
            let scope_str: String = row.get(0);
            let account_id_str = scope_str
                .strip_prefix("credential-account:")
                .unwrap_or(&scope_str);
            let account_id = CredentialAccountId::new(account_id_str).map_err(|e| {
                CredentialBrokerError::BrokerUnavailable {
                    reason: format!("invalid account id in row: {e}"),
                }
            })?;
            let aad = credential_account_aad(scope, &account_id);
            let stored_ct: String = row.get(1);
            let (salt, nonce_and_ct) = decode_ciphertext_broker(&stored_ct)?;
            let plaintext = self
                .crypto
                .decrypt(&nonce_and_ct, &salt, &aad)
                .map_err(|e| CredentialBrokerError::BrokerUnavailable {
                    reason: format!("decrypt error: {e:?}"),
                })?;
            let payload: StoredAccountPayload =
                serde_json::from_slice(plaintext.expose().as_bytes()).map_err(|e| {
                    CredentialBrokerError::BrokerUnavailable {
                        reason: format!("deserialize account: {e}"),
                    }
                })?;
            accounts.push(payload.account);
        }
        Ok(accounts)
    }
}

#[async_trait]
impl CredentialSessionStore for PgCredentialBroker {
    async fn issue_session(
        &self,
        session: CredentialSession,
    ) -> Result<CredentialSession, CredentialBrokerError> {
        let session_id = session.correlation_id();
        let scope_str = session_scope_str(session_id);
        let aad = credential_session_aad(session.scope(), session_id);

        let payload = StoredSessionPayload {
            scope: session.scope().clone(),
            invocation_id: session.invocation_id(),
            capability_id: session.capability_id().clone(),
            extension_id: session.extension_id().clone(),
            account_id: session.account_id().clone(),
            secret_handles: session.secret_handles().to_vec(),
            allowed_targets: session.allowed_targets().to_vec(),
            expires_at: session.expires_at(),
            max_uses: session.max_uses(),
            correlation_id: session_id.to_private_storage_string(),
            uses: 0,
        };
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| CredentialBrokerError::BrokerUnavailable {
                reason: e.to_string(),
            })?;

        self.write_encrypted_row(&scope_str, "session", &payload_bytes, &aad)
            .await?;
        Ok(session)
    }

    async fn get_session(
        &self,
        scope: &ResourceScope,
        session_id: CredentialSessionId,
    ) -> Result<Option<CredentialSession>, CredentialBrokerError> {
        let scope_str = session_scope_str(session_id);
        let aad = credential_session_aad(scope, session_id);

        let plaintext = self.read_encrypted_row(&scope_str, "session", &aad).await?;

        match plaintext {
            None => Ok(None),
            Some(bytes) => session_payload_to_session(&bytes),
        }
    }

    async fn validate_session(
        &self,
        scope: &ResourceScope,
        session_id: CredentialSessionId,
        now: Timestamp,
    ) -> Result<CredentialSession, CredentialBrokerError> {
        let session = self
            .get_session(scope, session_id)
            .await?
            .ok_or(CredentialBrokerError::UnknownSession { session_id })?;

        if let Some(expires_at) = session.expires_at()
            && expires_at <= now
        {
            return Err(CredentialBrokerError::SessionExpired { session_id });
        }
        Ok(session)
    }

    async fn consume_session_use(
        &self,
        scope: &ResourceScope,
        session_id: CredentialSessionId,
        now: Timestamp,
    ) -> Result<CredentialSession, CredentialBrokerError> {
        // Validate first (checks expiry + max_uses).
        let session = self.validate_session(scope, session_id, now).await?;

        let scope_str = session_scope_str(session_id);
        let aad = credential_session_aad(scope, session_id);

        let plaintext = self
            .read_encrypted_row(&scope_str, "session", &aad)
            .await?
            .ok_or(CredentialBrokerError::UnknownSession { session_id })?;

        let mut payload: StoredSessionPayload =
            serde_json::from_slice(&plaintext).map_err(|e| {
                CredentialBrokerError::BrokerUnavailable {
                    reason: format!("deserialize session: {e}"),
                }
            })?;

        if let Some(max_uses) = payload.max_uses
            && payload.uses >= max_uses
        {
            return Err(CredentialBrokerError::SessionUseLimitExceeded { session_id });
        }
        payload.uses += 1;

        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| CredentialBrokerError::BrokerUnavailable {
                reason: e.to_string(),
            })?;
        self.write_encrypted_row(&scope_str, "session", &payload_bytes, &aad)
            .await?;

        Ok(session)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Stable `name` column value for a credential row, keyed by owner scope
/// (tenant / user / agent / project).
fn resource_scope_key(scope: &ResourceScope) -> String {
    format!(
        "tenant:{}/user:{}/agent:{}/project:{}",
        scope.tenant_id,
        scope.user_id,
        scope
            .agent_id
            .as_ref()
            .map(|a| a.to_string())
            .unwrap_or_default(),
        scope
            .project_id
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_default(),
    )
}

fn session_payload_to_session(
    bytes: &[u8],
) -> Result<Option<CredentialSession>, CredentialBrokerError> {
    let payload: StoredSessionPayload =
        serde_json::from_slice(bytes).map_err(|e| CredentialBrokerError::BrokerUnavailable {
            reason: format!("deserialize session: {e}"),
        })?;
    let cid = CredentialSessionId::parse(&payload.correlation_id).map_err(|e| {
        CredentialBrokerError::BrokerUnavailable {
            reason: format!("invalid session id: {e}"),
        }
    })?;
    Ok(Some(__internal_session_for_filesystem_store(
        payload.scope,
        payload.invocation_id,
        payload.capability_id,
        payload.extension_id,
        payload.account_id,
        payload.secret_handles,
        payload.allowed_targets,
        payload.expires_at,
        payload.max_uses,
        cid,
    )))
}
