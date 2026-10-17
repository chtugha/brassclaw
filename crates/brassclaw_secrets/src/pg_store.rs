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
//! New secret and lease envelopes bind the physical store tenant, row identity
//! and logical owner through length-prefixed AAD. Owner-authenticated legacy
//! secret rows upgrade atomically on access; old lease envelopes remain readable
//! until consumed/revoked. Historical ciphertext cannot retroactively gain a
//! physical-owner binding before that upgrade.
//!
//! # Schema
//!
//! Rows live in `brassclaw_secrets` (V003__secrets.sql):
//! `(tenant_id, scope, name, ciphertext, key_version)`
//!
//! Credential accounts and sessions share the same table using structured
//! `scope` discriminators:
//! - secrets:    `scope = "secret-v2:<owner-and-handle digest>"`
//! - leases:     `scope = "lease-v2:<lease_id>"`
//! - accounts:   `scope = "credential-account:<account_id>"`
//! - sessions:   `scope = "credential-session:<session_id>"`

use std::sync::Arc;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use brassclaw_host_api::{ResourceScope, SecretHandle, Timestamp};
use brassclaw_pg::PgPool;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    #[serde(deserialize_with = "deserialize_stored_scope")]
    scope: ResourceScope,
}

// Only authenticated storage payloads may contain the reserved system identity.
// Public TenantId/UserId deserialization must continue rejecting that sentinel.
fn deserialize_stored_scope<'de, D>(deserializer: D) -> Result<ResourceScope, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct StoredScope {
        tenant_id: String,
        user_id: String,
        agent_id: Option<brassclaw_host_api::AgentId>,
        project_id: Option<brassclaw_host_api::ProjectId>,
        thread_id: Option<brassclaw_host_api::ThreadId>,
        invocation_id: brassclaw_host_api::InvocationId,
    }
    let stored = StoredScope::deserialize(deserializer)?;
    if stored.tenant_id == brassclaw_host_api::SYSTEM_RESERVED_ID
        && stored.user_id == brassclaw_host_api::SYSTEM_RESERVED_ID
        && stored.agent_id.is_none()
        && stored.project_id.is_none()
        && stored.thread_id.is_none()
    {
        return Ok(ResourceScope {
            invocation_id: stored.invocation_id,
            ..ResourceScope::system()
        });
    }
    Ok(ResourceScope {
        tenant_id: brassclaw_host_api::TenantId::new(stored.tenant_id)
            .map_err(serde::de::Error::custom)?,
        user_id: brassclaw_host_api::UserId::new(stored.user_id)
            .map_err(serde::de::Error::custom)?,
        agent_id: stored.agent_id,
        project_id: stored.project_id,
        thread_id: stored.thread_id,
        invocation_id: stored.invocation_id,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccountPayload {
    #[serde(deserialize_with = "deserialize_stored_account")]
    account: crate::CredentialAccount,
}

fn deserialize_stored_account<'de, D>(deserializer: D) -> Result<CredentialAccount, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct StoredAccount {
        #[serde(deserialize_with = "deserialize_stored_scope")]
        scope: ResourceScope,
        id: CredentialAccountId,
        provider_or_extension_id: brassclaw_host_api::ExtensionId,
        label: String,
        status: crate::CredentialAccountStatus,
        secret_handles: Vec<SecretHandle>,
        allowed_targets: Vec<crate::CredentialTargetPolicy>,
        redacted_metadata: crate::RedactedJson,
        updated_at: Timestamp,
    }
    let stored = StoredAccount::deserialize(deserializer)?;
    Ok(CredentialAccount {
        scope: stored.scope,
        id: stored.id,
        provider_or_extension_id: stored.provider_or_extension_id,
        label: stored.label,
        status: stored.status,
        secret_handles: stored.secret_handles,
        allowed_targets: stored.allowed_targets,
        redacted_metadata: stored.redacted_metadata,
        updated_at: stored.updated_at,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSessionPayload {
    #[serde(deserialize_with = "deserialize_stored_scope")]
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
fn legacy_secret_scope_str(scope: &ResourceScope, handle: &SecretHandle) -> String {
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

/// Owner-complete, unambiguous key. Invocation/thread are deliberately absent:
/// secret material belongs to tenant/user/agent/project, leases to an invocation.
fn secret_scope_str(scope: &ResourceScope, handle: &SecretHandle) -> String {
    format!(
        "secret-v2:{:x}",
        Sha256::digest(filesystem_secret_aad(scope, handle))
    )
}

fn pg_record_aad(tenant: &str, key: &str, name: &str, inner: &[u8]) -> Vec<u8> {
    crate::crypto::build_aad(
        b"reborn/v2/pg_secret_row",
        &[tenant.as_bytes(), key.as_bytes(), name.as_bytes(), inner],
    )
}

/// `scope` column discriminator for a lease row.
fn lease_scope_str(lease_id: SecretLeaseId) -> String {
    format!("lease-v2:{lease_id}")
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

    /// Trusted host setup read participating in the caller's database snapshot.
    /// These ports preserve authenticated owner/one-shot semantics; they are not
    /// plugin APIs or permission grants. The caller owns commit/rollback and must
    /// commit consumption before using material for external effects.
    pub async fn lease_once_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<SecretLease, SecretStoreError> {
        // Verify the secret exists.
        if self
            .load_secret(transaction, scope, handle)
            .await?
            .is_none()
        {
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
        // The authenticated payload carries the invocation. Bind the outer
        // envelope to its physical store and exact lease row as well.
        let legacy_aad = format!("pg-secret-lease:{lease_id}");
        let aad = pg_record_aad(
            &self.tenant_id,
            &lease_scope,
            "payload",
            legacy_aad.as_bytes(),
        );
        let (nonce_and_ct, salt) = self.crypto.encrypt(&payload_bytes, &aad).map_err(|e| {
            SecretStoreError::BackendMisconfigured {
                reason: format!("encrypt lease: {e:?}"),
            }
        })?;
        let ciphertext = encode_ciphertext(&salt, &nonce_and_ct);

        transaction
            .execute(
                "INSERT INTO brassclaw_secrets \
                 (tenant_id, scope, name, ciphertext, key_version)
                 VALUES ($1, $2, 'payload', $3, 1)",
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

    /// Consume exactly this authenticated lease inside the caller transaction.
    pub async fn consume_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretMaterial, SecretStoreError> {
        let (lease_scope, payload) =
            self.load_lease(transaction, lease_id)
                .await?
                .ok_or_else(|| SecretStoreError::UnknownLease {
                    scope: Box::new(scope.clone()),
                    lease_id,
                })?;
        if payload.scope != *scope {
            return Err(SecretStoreError::UnknownLease {
                scope: Box::new(scope.clone()),
                lease_id,
            });
        }
        let material = self
            .load_secret(transaction, &payload.scope, &payload.handle)
            .await?
            .ok_or_else(|| SecretStoreError::UnknownSecret {
                scope: Box::new(payload.scope.clone()),
                handle: payload.handle.clone(),
            })?;

        let deleted = transaction
            .execute(
                "DELETE FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $2 AND name = 'payload'",
                &[&self.tenant_id, &lease_scope],
            )
            .await
            .map_err(map_pg_error)?;
        if deleted != 1 {
            return Err(SecretStoreError::UnknownLease {
                scope: Box::new(scope.clone()),
                lease_id,
            });
        }
        Ok(material)
    }

    /// Trusted setup/composition write participating in the caller's PostgreSQL
    /// transaction. The caller owns commit/rollback; encryption, owner binding
    /// and authentication of preceding material remain owned by this store.
    /// This is not a runtime/plugin API or an authorization grant.
    pub async fn put_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
    ) -> Result<SecretMetadata, SecretStoreError> {
        let scope_key = secret_scope_str(&scope, &handle);
        let aad = pg_record_aad(
            &self.tenant_id,
            &scope_key,
            handle.as_str(),
            &filesystem_secret_aad(&scope, &handle),
        );
        // Row locks cannot protect an absent row. Serialize trusted writers by
        // physical owner and secret identity before authenticating or inserting.
        // A hash collision only serializes unrelated writes; it grants nothing.
        let digest = Sha256::digest(&aad);
        let mut lock_bytes = [0u8; 8];
        lock_bytes.copy_from_slice(&digest[..8]);
        let lock_id = i64::from_be_bytes(lock_bytes);
        transaction
            .query_one("SELECT pg_advisory_xact_lock($1)", &[&lock_id])
            .await
            .map_err(map_pg_error)?;
        drop(self.load_secret(transaction, &scope, &handle).await?);
        let (nonce_and_ct, salt) = self
            .crypto
            .encrypt(material.expose_secret().as_bytes(), &aad)
            .map_err(|_| SecretStoreError::BackendMisconfigured {
                reason: "secret record encryption failed".into(),
            })?;
        let ciphertext = encode_ciphertext(&salt, &nonce_and_ct);
        transaction.execute(
            "INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) VALUES ($1, $2, $3, $4, 1) ON CONFLICT (tenant_id, scope, name) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = now()",
            &[&self.tenant_id, &scope_key, &handle.as_str(), &ciphertext],
        ).await.map_err(map_pg_error)?;
        Ok(SecretMetadata { scope, handle })
    }
    fn decode_lease_payload(
        &self,
        ciphertext: &str,
        lease_id: SecretLeaseId,
        stored_scope: &str,
    ) -> Result<StoredLeasePayload, SecretStoreError> {
        let (salt, nonce_and_ct) = decode_ciphertext(ciphertext)?;
        let legacy_aad = format!("pg-secret-lease:{lease_id}");
        let aad = if stored_scope == lease_scope_str(lease_id) {
            pg_record_aad(
                &self.tenant_id,
                stored_scope,
                "payload",
                legacy_aad.as_bytes(),
            )
        } else if stored_scope == format!("lease:{lease_id}") {
            legacy_aad.into_bytes()
        } else {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "stored secret lease identity is invalid".into(),
            });
        };
        let plaintext = self
            .crypto
            .decrypt(&nonce_and_ct, &salt, &aad)
            .map_err(|_| SecretStoreError::StoreUnavailable {
                reason: "secret lease authentication failed".into(),
            })?;
        serde_json::from_str(plaintext.expose()).map_err(|_| SecretStoreError::StoreUnavailable {
            reason: "stored secret lease is invalid".into(),
        })
    }

    /// Authenticate before exposing metadata or deleting. Legacy owner-incomplete
    /// rows are upgraded only after their owner AAD verifies, inside the caller's
    /// transaction. Conflicts fail closed rather than overwriting another record.
    async fn load_secret(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<SecretMaterial>, SecretStoreError> {
        let key = secret_scope_str(scope, handle);
        let legacy = legacy_secret_scope_str(scope, handle);
        let mut rows = transaction.query(
            "SELECT scope, ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = $1 AND (scope = $2 OR scope = $3) AND name = $4 ORDER BY scope FOR UPDATE",
            &[&self.tenant_id, &key, &legacy, &handle.as_str()],
        ).await.map_err(map_pg_error)?;
        // READ COMMITTED can wait on a legacy row which another transaction
        // moves. Re-read the new key after that wait, instead of reporting a
        // missing secret that was continuously present during the upgrade.
        if rows.is_empty() {
            rows = transaction.query(
                "SELECT scope, ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $2 AND name = $3 FOR UPDATE",
                &[&self.tenant_id, &key, &handle.as_str()],
            ).await.map_err(map_pg_error)?;
        }
        if rows.len() > 1 {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "conflicting current and legacy secret records require reconciliation"
                    .into(),
            });
        }
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        if row.get::<_, i32>(2) != 1 {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "unsupported secret master-key version".into(),
            });
        }
        let stored_key: String = row.get(0);
        let ciphertext: String = row.get(1);
        let (salt, nonce_and_ct) = decode_ciphertext(&ciphertext)?;
        let inner = filesystem_secret_aad(scope, handle);
        let aad = if stored_key == key {
            pg_record_aad(&self.tenant_id, &key, handle.as_str(), &inner)
        } else {
            inner
        };
        let plaintext = self
            .crypto
            .decrypt(&nonce_and_ct, &salt, &aad)
            .map_err(|_| SecretStoreError::StoreUnavailable {
                reason: "secret record ownership or integrity verification failed".into(),
            })?;
        if stored_key == legacy {
            let new_aad = pg_record_aad(
                &self.tenant_id,
                &key,
                handle.as_str(),
                &filesystem_secret_aad(scope, handle),
            );
            let (nonce_and_ct, salt) = self
                .crypto
                .encrypt(plaintext.expose().as_bytes(), &new_aad)
                .map_err(|_| SecretStoreError::StoreUnavailable {
                    reason: "legacy secret upgrade encryption failed".into(),
                })?;
            let ciphertext = encode_ciphertext(&salt, &nonce_and_ct);
            let inserted = transaction.execute(
                "INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version, created_at) SELECT tenant_id, $2, name, $4, key_version, created_at FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $5 AND name = $3 ON CONFLICT (tenant_id, scope, name) DO NOTHING",
                &[&self.tenant_id, &key, &handle.as_str(), &ciphertext, &legacy],
            ).await.map_err(map_pg_error)?;
            if inserted != 1 {
                return Err(SecretStoreError::StoreUnavailable {
                    reason: "concurrent legacy secret upgrade requires retry".into(),
                });
            }
            transaction.execute(
                "DELETE FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $2 AND name = $3",
                &[&self.tenant_id, &legacy, &handle.as_str()],
            ).await.map_err(map_pg_error)?;
        }
        Ok(Some(SecretMaterial::from(plaintext.expose().to_owned())))
    }

    async fn load_lease(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        lease_id: SecretLeaseId,
    ) -> Result<Option<(String, StoredLeasePayload)>, SecretStoreError> {
        let key = lease_scope_str(lease_id);
        let legacy = format!("lease:{lease_id}");
        let rows = transaction.query(
            "SELECT scope, ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = $1 AND (scope = $2 OR scope = $3) AND name = 'payload' ORDER BY scope FOR UPDATE",
            &[&self.tenant_id, &key, &legacy],
        ).await.map_err(map_pg_error)?;
        if rows.len() > 1 {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "conflicting secret lease records require reconciliation".into(),
            });
        }
        rows.first()
            .map(|row| {
                if row.get::<_, i32>(2) != 1 {
                    return Err(SecretStoreError::StoreUnavailable {
                        reason: "unsupported secret lease master-key version".into(),
                    });
                }
                let stored_scope: String = row.get(0);
                self.decode_lease_payload(&row.get::<_, String>(1), lease_id, &stored_scope)
                    .map(|payload| (stored_scope, payload))
            })
            .transpose()
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
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let metadata = self
            .put_in_transaction(&transaction, scope, handle, material)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(metadata)
    }

    async fn metadata(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let found = self
            .load_secret(&transaction, scope, handle)
            .await?
            .is_some();
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(found.then(|| SecretMetadata {
            scope: scope.clone(),
            handle: handle.clone(),
        }))
    }

    async fn delete(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<bool, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        if self
            .load_secret(&transaction, scope, handle)
            .await?
            .is_none()
        {
            return Ok(false);
        }
        let scope_key = secret_scope_str(scope, handle);
        let rows = transaction
            .execute(
                "DELETE FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $2 AND name = $3",
                &[&self.tenant_id, &scope_key, &handle.as_str()],
            )
            .await
            .map_err(map_pg_error)?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(rows == 1)
    }

    async fn lease_once(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<SecretLease, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let lease = self
            .lease_once_in_transaction(&transaction, scope, handle)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(lease)
    }

    async fn consume(
        &self,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretMaterial, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let material = self
            .consume_in_transaction(&transaction, scope, lease_id)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(material)
    }

    async fn revoke(
        &self,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretLease, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let (lease_scope, payload) =
            self.load_lease(&transaction, lease_id)
                .await?
                .ok_or_else(|| SecretStoreError::UnknownLease {
                    scope: Box::new(scope.clone()),
                    lease_id,
                })?;
        if payload.scope != *scope {
            return Err(SecretStoreError::UnknownLease {
                scope: Box::new(scope.clone()),
                lease_id,
            });
        }
        transaction
            .execute(
                "DELETE FROM brassclaw_secrets \
                 WHERE tenant_id = $1 AND scope = $2",
                &[&self.tenant_id, &lease_scope],
            )
            .await
            .map_err(map_pg_error)?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(SecretLease {
            id: lease_id,
            scope: scope.clone(),
            handle: payload.handle,
            status: SecretLeaseStatus::Revoked,
        })
    }

    async fn leases_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<SecretLease>, SecretStoreError> {
        let client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let rows = client.query(
            "SELECT scope, ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = $1 AND (scope LIKE 'lease:%' OR scope LIKE 'lease-v2:%') AND name = 'payload' ORDER BY scope",
            &[&self.tenant_id],
        ).await.map_err(map_pg_error)?;
        let mut leases = Vec::new();
        for row in rows {
            if row.get::<_, i32>(2) != 1 {
                return Err(SecretStoreError::StoreUnavailable {
                    reason: "unsupported secret lease master-key version".into(),
                });
            }
            let lease_scope: String = row.get(0);
            let lease_id: SecretLeaseId = serde_json::from_value(serde_json::Value::String(
                lease_scope
                    .strip_prefix("lease-v2:")
                    .or_else(|| lease_scope.strip_prefix("lease:"))
                    .unwrap_or_default()
                    .to_string(),
            ))
            .map_err(|_| SecretStoreError::StoreUnavailable {
                reason: "stored secret lease identity is invalid".into(),
            })?;
            let payload =
                self.decode_lease_payload(&row.get::<_, String>(1), lease_id, &lease_scope)?;
            if payload.scope == *scope {
                leases.push(SecretLease {
                    id: lease_id,
                    scope: payload.scope,
                    handle: payload.handle,
                    status: SecretLeaseStatus::Active,
                });
            }
        }
        Ok(leases)
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

    fn encode_payload(
        &self,
        key: &str,
        name: &str,
        bytes: &[u8],
        inner_aad: &[u8],
    ) -> Result<String, CredentialBrokerError> {
        let aad = pg_record_aad(&self.tenant_id, key, name, inner_aad);
        let (nonce_and_ct, salt) = self.crypto.encrypt(bytes, &aad).map_err(|_| {
            CredentialBrokerError::BrokerUnavailable {
                reason: "credential encryption failed".into(),
            }
        })?;
        // Envelope version is separate from the master-key version. Existing
        // unprefixed records remain readable under their original logical AAD.
        Ok(format!("v2:{}", encode_ciphertext(&salt, &nonce_and_ct)))
    }

    fn decode_payload(
        &self,
        key: &str,
        name: &str,
        stored: &str,
        inner_aad: &[u8],
        key_version: i32,
    ) -> Result<crate::legacy_store::DecryptedSecret, CredentialBrokerError> {
        if key_version != 1 {
            return Err(CredentialBrokerError::BrokerUnavailable {
                reason: "unsupported credential master-key version".into(),
            });
        }
        let (ciphertext, aad) = if let Some(ciphertext) = stored.strip_prefix("v2:") {
            (
                ciphertext,
                pg_record_aad(&self.tenant_id, key, name, inner_aad),
            )
        } else {
            (stored, inner_aad.to_vec())
        };
        let (salt, nonce_and_ct) = decode_ciphertext_broker(ciphertext)?;
        self.crypto
            .decrypt(&nonce_and_ct, &salt, &aad)
            .map_err(|_| CredentialBrokerError::BrokerUnavailable {
                reason: "credential ownership or integrity verification failed".into(),
            })
    }

    /// Authenticate old material before replacing it. Wrong keys and copied
    /// envelopes must not turn an update into destructive credential loss.
    async fn write_encrypted_row(
        &self,
        key: &str,
        name: &str,
        bytes: &[u8],
        aad: &[u8],
    ) -> Result<(), CredentialBrokerError> {
        let ciphertext = self.encode_payload(key, name, bytes, aad)?;
        let mut client = self.pool.get().await.map_err(map_pg_pool_err_broker)?;
        let transaction = client.transaction().await.map_err(map_pg_err_broker)?;
        if let Some(row) = transaction.query_opt(
            "SELECT ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $2 AND name = $3 FOR UPDATE",
            &[&self.tenant_id, &key, &name],
        ).await.map_err(map_pg_err_broker)? {
            drop(self.decode_payload(key, name, &row.get::<_, String>(0), aad, row.get(1))?);
        }
        transaction.execute(
            "INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) VALUES ($1, $2, $3, $4, 1) ON CONFLICT (tenant_id, scope, name) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = now()",
            &[&self.tenant_id, &key, &name, &ciphertext],
        ).await.map_err(map_pg_err_broker)?;
        transaction.commit().await.map_err(map_pg_err_broker)?;
        Ok(())
    }

    async fn read_encrypted_row(
        &self,
        key: &str,
        name: &str,
        aad: &[u8],
    ) -> Result<Option<crate::legacy_store::DecryptedSecret>, CredentialBrokerError> {
        let client = self.pool.get().await.map_err(map_pg_pool_err_broker)?;
        let row = client.query_opt(
            "SELECT ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $2 AND name = $3",
            &[&self.tenant_id, &key, &name],
        ).await.map_err(map_pg_err_broker)?;
        row.map(|row| self.decode_payload(key, name, &row.get::<_, String>(0), aad, row.get(1)))
            .transpose()
    }

    async fn read_session_payload(
        &self,
        scope: &ResourceScope,
        session_id: CredentialSessionId,
    ) -> Result<Option<StoredSessionPayload>, CredentialBrokerError> {
        let key = session_scope_str(session_id);
        let aad = credential_session_aad(scope, session_id);
        self.read_encrypted_row(&key, "session", &aad)
            .await?
            .map(|plaintext| {
                decode_session_payload(plaintext.expose().as_bytes(), scope, session_id)
            })
            .transpose()
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
                    serde_json::from_slice(bytes.expose().as_bytes()).map_err(|e| {
                        CredentialBrokerError::BrokerUnavailable {
                            reason: format!("deserialize account: {e}"),
                        }
                    })?;
                if payload.account.id != *account_id
                    || resource_scope_key(&payload.account.scope) != resource_scope_key(scope)
                {
                    return Err(CredentialBrokerError::BrokerUnavailable {
                        reason: "stored credential account identity is invalid".into(),
                    });
                }
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
                "SELECT scope, ciphertext, key_version FROM brassclaw_secrets \
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
            let plaintext = self.decode_payload(&scope_str, &name, &stored_ct, &aad, row.get(2))?;
            let payload: StoredAccountPayload =
                serde_json::from_slice(plaintext.expose().as_bytes()).map_err(|e| {
                    CredentialBrokerError::BrokerUnavailable {
                        reason: format!("deserialize account: {e}"),
                    }
                })?;
            if payload.account.id != account_id
                || resource_scope_key(&payload.account.scope) != resource_scope_key(scope)
            {
                return Err(CredentialBrokerError::BrokerUnavailable {
                    reason: "stored credential account identity is invalid".into(),
                });
            }
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

        let ciphertext = self.encode_payload(&scope_str, "session", &payload_bytes, &aad)?;
        let client = self.pool.get().await.map_err(map_pg_pool_err_broker)?;
        let inserted = client.execute(
            "INSERT INTO brassclaw_secrets (tenant_id, scope, name, ciphertext, key_version) VALUES ($1, $2, 'session', $3, 1) ON CONFLICT (tenant_id, scope, name) DO NOTHING",
            &[&self.tenant_id, &scope_str, &ciphertext],
        ).await.map_err(map_pg_err_broker)?;
        drop(client);
        if inserted == 0 {
            // Idempotent issuance may retain the contract, never reset spent
            // uses or change an existing bearer session's identity/lifetime.
            let retained = self
                .get_session(session.scope(), session_id)
                .await?
                .ok_or(CredentialBrokerError::UnknownSession { session_id })?;
            if retained != session {
                return Err(CredentialBrokerError::BrokerUnavailable {
                    reason: "conflicting credential session issuance".into(),
                });
            }
        }
        Ok(session)
    }

    async fn get_session(
        &self,
        scope: &ResourceScope,
        session_id: CredentialSessionId,
    ) -> Result<Option<CredentialSession>, CredentialBrokerError> {
        self.read_session_payload(scope, session_id)
            .await?
            .map(session_payload_to_session)
            .transpose()
    }

    async fn validate_session(
        &self,
        scope: &ResourceScope,
        session_id: CredentialSessionId,
        now: Timestamp,
    ) -> Result<CredentialSession, CredentialBrokerError> {
        let payload = self
            .read_session_payload(scope, session_id)
            .await?
            .ok_or(CredentialBrokerError::UnknownSession { session_id })?;
        ensure_session_payload_usable(&payload, session_id, now)?;
        session_payload_to_session(payload)
    }

    async fn consume_session_use(
        &self,
        scope: &ResourceScope,
        session_id: CredentialSessionId,
        now: Timestamp,
    ) -> Result<CredentialSession, CredentialBrokerError> {
        let key = session_scope_str(session_id);
        let aad = credential_session_aad(scope, session_id);
        let mut client = self.pool.get().await.map_err(map_pg_pool_err_broker)?;
        let transaction = client.transaction().await.map_err(map_pg_err_broker)?;
        let row = transaction.query_opt(
            "SELECT ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id = $1 AND scope = $2 AND name = 'session' FOR UPDATE",
            &[&self.tenant_id, &key],
        ).await.map_err(map_pg_err_broker)?.ok_or(CredentialBrokerError::UnknownSession { session_id })?;
        let plaintext =
            self.decode_payload(&key, "session", &row.get::<_, String>(0), &aad, row.get(1))?;
        let mut payload = decode_session_payload(plaintext.expose().as_bytes(), scope, session_id)?;
        ensure_session_payload_usable(&payload, session_id, now)?;
        payload.uses = payload.uses.checked_add(1).ok_or_else(|| {
            CredentialBrokerError::BrokerUnavailable {
                reason: "credential session use counter exhausted".into(),
            }
        })?;
        let bytes =
            serde_json::to_vec(&payload).map_err(|_| CredentialBrokerError::BrokerUnavailable {
                reason: "credential session serialization failed".into(),
            })?;
        let ciphertext = self.encode_payload(&key, "session", &bytes, &aad)?;
        let session = session_payload_to_session(payload)?;
        let updated = transaction.execute(
            "UPDATE brassclaw_secrets SET ciphertext = $3, updated_at = now() WHERE tenant_id = $1 AND scope = $2 AND name = 'session'",
            &[&self.tenant_id, &key, &ciphertext],
        ).await.map_err(map_pg_err_broker)?;
        if updated != 1 {
            return Err(CredentialBrokerError::BrokerUnavailable {
                reason: "credential session changed during consumption".into(),
            });
        }
        transaction.commit().await.map_err(map_pg_err_broker)?;
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

fn decode_session_payload(
    bytes: &[u8],
    scope: &ResourceScope,
    session_id: CredentialSessionId,
) -> Result<StoredSessionPayload, CredentialBrokerError> {
    let payload: StoredSessionPayload =
        serde_json::from_slice(bytes).map_err(|_| CredentialBrokerError::BrokerUnavailable {
            reason: "stored credential session is invalid".into(),
        })?;
    if payload.scope != *scope
        || payload.invocation_id != scope.invocation_id
        || payload.correlation_id != session_id.to_private_storage_string()
    {
        return Err(CredentialBrokerError::BrokerUnavailable {
            reason: "stored credential session identity is invalid".into(),
        });
    }
    Ok(payload)
}

fn ensure_session_payload_usable(
    payload: &StoredSessionPayload,
    session_id: CredentialSessionId,
    now: Timestamp,
) -> Result<(), CredentialBrokerError> {
    if payload.expires_at.is_some_and(|expiry| expiry <= now) {
        return Err(CredentialBrokerError::SessionExpired { session_id });
    }
    if payload.max_uses.is_some_and(|limit| payload.uses >= limit) {
        return Err(CredentialBrokerError::SessionUseLimitExceeded { session_id });
    }
    Ok(())
}

fn session_payload_to_session(
    payload: StoredSessionPayload,
) -> Result<CredentialSession, CredentialBrokerError> {
    let cid = CredentialSessionId::parse(&payload.correlation_id).map_err(|e| {
        CredentialBrokerError::BrokerUnavailable {
            reason: format!("invalid session id: {e}"),
        }
    })?;
    Ok(__internal_session_for_filesystem_store(
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
    ))
}
