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
//! secret rows upgrade atomically on access; old unproven leases remain readable
//! for explicit revocation, but cannot consume a generation they never pinned.
//! Historical ciphertext cannot retroactively gain a
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
use chrono::Utc;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::custody::{
    CUSTODY_FORMAT_V1, CustodyBody, DecodedSecretValue, OrdinarySecretValue,
    TIMED_ORDINARY_FORMAT_V1, decrypt_stored_value,
};
use crate::stored_scope::deserialize_stored_scope;

use crate::{
    __internal_session_for_filesystem_store, CredentialAccount, CredentialAccountId,
    CredentialAccountStore, CredentialBrokerError, CredentialSession, CredentialSessionId,
    CredentialSessionStore, SecretCustodyMetadata, SecretCustodySource, SecretLease, SecretLeaseId,
    SecretLeaseStatus, SecretMaterial, SecretMetadata, SecretStore, SecretStoreError,
    SecretsCrypto, credential_account_aad, credential_session_aad, filesystem_secret_aad,
};

/// Size of the HKDF salt in bytes, matching `SecretsCrypto::SALT_SIZE = 32`.
const SALT_SIZE: usize = 32;

// Colon is outside ordinary STANDARD base64; no raw-material heuristic.
const CUSTODY_CIPHERTEXT_PREFIX: &str = "custody-v1:";
const TIMED_ORDINARY_CIPHERTEXT_PREFIX: &str = "timed-v1:";

// ---------------------------------------------------------------------------
// Wire DTOs (private, encrypted payloads)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredLeasePayload {
    handle: SecretHandle,
    #[serde(deserialize_with = "deserialize_stored_scope")]
    scope: ResourceScope,
    // Older leases cannot prove which generation was selected. They remain
    // readable for explicit revocation but never authorize reading latest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    record_reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secret_expires_at: Option<Timestamp>,
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

// Authenticated bytes stay private to this storage owner. Never derive Debug.
struct AuthenticatedSecretRecord {
    key: String,
    ciphertext: String,
    version: i32,
    value: DecodedSecretValue,
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
        self.lease_selected_in_transaction(transaction, scope, handle, None)
            .await?
            .ok_or_else(|| SecretStoreError::UnknownSecret {
                scope: Box::new(scope.clone()),
                handle: handle.clone(),
            })
    }

    /// Check the caller-selected pre-upgrade generation under the actual row
    /// lock. The caller owns commit/rollback; no advisory lock is taken after
    /// acquiring the row. Missing/mismatched selection creates no lease.
    pub async fn lease_once_if_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected: &str,
    ) -> Result<Option<SecretLease>, SecretStoreError> {
        self.lease_selected_in_transaction(transaction, scope, handle, Some(expected))
            .await
    }

    async fn lease_selected_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected: Option<&str>,
    ) -> Result<Option<SecretLease>, SecretStoreError> {
        // Verify the secret exists.
        if self
            .load_secret_selected(transaction, scope, handle, expected)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        let lease_id = SecretLeaseId::new();
        let lease_scope = lease_scope_str(lease_id);

        // load_secret holds the actual row lock and upgrades legacy storage
        // before this selection. Capture those exact authenticated bytes while
        // the same transaction still owns the row; no new advisory lock here.
        let record = self
            .inspect_secret_in_transaction(transaction, scope, handle)
            .await?
            .ok_or_else(|| SecretStoreError::UnknownSecret {
                scope: Box::new(scope.clone()),
                handle: handle.clone(),
            })?;
        let secret_expires_at = record.value.secret_expires_at();
        if secret_expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Err(SecretStoreError::SecretExpired);
        }
        let record_reference = self.encrypted_record_reference(
            &record.key,
            handle,
            &record.ciphertext,
            record.version,
        );
        // The selected encrypted generation belongs inside the authenticated
        // lease payload, not a later handle-only lookup or external assertion.
        let payload = StoredLeasePayload {
            handle: handle.clone(),
            scope: scope.clone(),
            record_reference: Some(record_reference),
            secret_expires_at,
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

        Ok(Some(SecretLease {
            id: lease_id,
            scope: scope.clone(),
            handle: handle.clone(),
            status: SecretLeaseStatus::Active,
            secret_expires_at,
        }))
    }

    async fn issue_selected_lease(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected: Option<&str>,
    ) -> Result<Option<SecretLease>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let lease = self
            .lease_selected_in_transaction(&transaction, scope, handle, expected)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        if lease
            .as_ref()
            .and_then(|lease| lease.secret_expires_at)
            .is_some_and(|expiry| expiry <= Utc::now())
        {
            return Err(SecretStoreError::SecretExpired);
        }
        Ok(lease)
    }

    /// Consume exactly this authenticated lease inside the caller transaction.
    pub async fn consume_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretMaterial, SecretStoreError> {
        let selected = self
            .consume_selected_in_transaction(transaction, scope, lease_id)
            .await?;
        if selected
            .expires_at
            .is_some_and(|expiry| expiry <= Utc::now())
        {
            return Err(SecretStoreError::SecretExpired);
        }
        Ok(selected.material)
    }

    async fn consume_selected_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<OrdinarySecretValue, SecretStoreError> {
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
        let expected = payload
            .record_reference
            .as_deref()
            .filter(|reference| !reference.is_empty())
            .ok_or(SecretStoreError::LeaseRevoked { lease_id })?;
        // Keep the selected secret row locked through lease deletion/commit.
        // Do not acquire a writer advisory lock after the lease/secret row, or
        // migrate a replacement before establishing that it was selected.
        self.lock_secret_rows(transaction, &[(&payload.scope, &payload.handle)])
            .await?;
        let record = self
            .inspect_secret_in_transaction(transaction, &payload.scope, &payload.handle)
            .await?
            .ok_or_else(|| SecretStoreError::UnknownSecret {
                scope: Box::new(payload.scope.clone()),
                handle: payload.handle.clone(),
            })?;
        if self.encrypted_record_reference(
            &record.key,
            &payload.handle,
            &record.ciphertext,
            record.version,
        ) != expected
        {
            return Err(SecretStoreError::LeaseRevoked { lease_id });
        }
        if record.value.secret_expires_at() != payload.secret_expires_at {
            return Err(SecretStoreError::LeaseRevoked { lease_id });
        }
        if payload
            .secret_expires_at
            .is_some_and(|expiry| expiry <= Utc::now())
        {
            return Err(SecretStoreError::SecretExpired);
        }
        let material = record.value.into_runtime_material().ok_or_else(|| {
            SecretStoreError::UnknownSecret {
                scope: Box::new(payload.scope.clone()),
                handle: payload.handle.clone(),
            }
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
        Ok(OrdinarySecretValue {
            material,
            expires_at: payload.secret_expires_at,
        })
    }

    /// Trusted setup/composition write participating in the caller's PostgreSQL
    /// transaction. The caller owns commit/rollback; encryption, owner binding
    /// and authentication of preceding material remain owned by this store.
    /// This is not a runtime/plugin API or an authorization grant.
    async fn lock_secret_records(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        identities: &[(&ResourceScope, &SecretHandle)],
    ) -> Result<(), SecretStoreError> {
        let mut locks = Vec::new();
        for (scope, handle) in identities {
            let key = secret_scope_str(scope, handle);
            let aad = pg_record_aad(
                &self.tenant_id,
                &key,
                handle.as_str(),
                &filesystem_secret_aad(scope, handle),
            );
            let digest = Sha256::digest(&aad);
            let mut lock_bytes = [0u8; 8];
            lock_bytes.copy_from_slice(&digest[..8]);
            locks.push(i64::from_be_bytes(lock_bytes));
        }
        locks.sort_unstable();
        locks.dedup();
        for lock_id in locks {
            transaction
                .query_one("SELECT pg_advisory_xact_lock($1)", &[&lock_id])
                .await
                .map_err(map_pg_error)?;
        }
        self.lock_secret_rows(transaction, identities).await
    }

    // Reads that already own a row through lease/consume must never acquire a
    // new advisory write lock AFTER that row. provider_admin's real capture
    // reads a key and then its reference in one transaction. A writer can own
    // advisory before waiting on that row; acquiring it here would form a cycle.
    async fn lock_secret_rows(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        identities: &[(&ResourceScope, &SecretHandle)],
    ) -> Result<(), SecretStoreError> {
        let mut rows: Vec<_> = identities
            .iter()
            .map(|(scope, handle)| {
                (
                    secret_scope_str(scope, handle),
                    legacy_secret_scope_str(scope, handle),
                    handle.as_str().to_owned(),
                )
            })
            .collect();
        rows.sort();
        rows.dedup();
        for (key, legacy, name) in rows {
            transaction.query("SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND (scope=$2 OR scope=$3) AND name=$4 ORDER BY scope FOR UPDATE", &[&self.tenant_id, &key, &legacy, &name]).await.map_err(map_pg_error)?;
            // A legacy reader may have moved the row while the first query
            // waited. Lock its actual current key under the same transaction.
            transaction.query("SELECT scope FROM brassclaw_secrets WHERE tenant_id=$1 AND scope=$2 AND name=$3 FOR UPDATE", &[&self.tenant_id, &key, &name]).await.map_err(map_pg_error)?;
        }
        Ok(())
    }

    fn custody_metadata_for(
        &self,
        record: &AuthenticatedSecretRecord,
        handle: &SecretHandle,
    ) -> Option<SecretCustodyMetadata> {
        record.value.custody().map(|body| {
            body.metadata(
                handle.clone(),
                self.encrypted_record_reference(
                    &record.key,
                    handle,
                    &record.ciphertext,
                    record.version,
                ),
            )
        })
    }

    fn encode_custody(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        body: &CustodyBody,
    ) -> Result<String, SecretStoreError> {
        if crate::scoped_legacy_user_id(body.scope()) != crate::scoped_legacy_user_id(scope) {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "custody encryption owner mismatch".into(),
            });
        }
        let key = secret_scope_str(scope, handle);
        let aad = pg_record_aad(
            &self.tenant_id,
            &key,
            handle.as_str(),
            &filesystem_secret_aad(scope, handle),
        );
        let (ciphertext, salt) =
            crate::custody::encrypt_body(&self.crypto, &aad, body).map_err(|_| {
                SecretStoreError::BackendMisconfigured {
                    reason: "custody record encryption failed".into(),
                }
            })?;
        Ok(format!(
            "{CUSTODY_CIPHERTEXT_PREFIX}{}",
            encode_ciphertext(&salt, &ciphertext)
        ))
    }

    async fn insert_custody(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
        body: &CustodyBody,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        let key = secret_scope_str(scope, handle);
        let ciphertext = self.encode_custody(scope, handle, body)?;
        let affected = transaction.execute("INSERT INTO brassclaw_secrets (tenant_id,scope,name,ciphertext,key_version) VALUES ($1,$2,$3,$4,1) ON CONFLICT (tenant_id,scope,name) DO NOTHING", &[&self.tenant_id, &key, &handle.as_str(), &ciphertext]).await.map_err(map_pg_error)?;
        Ok((affected == 1).then(|| {
            body.metadata(
                handle.clone(),
                self.encrypted_record_reference(&key, handle, &ciphertext, 1),
            )
        }))
    }

    /// Prepare an original and its immutable output under one sorted lock set.
    /// This joins the caller's transaction; a conflict (`None`) or error requires
    /// rollback. It does not commit, authorize publication or expose raw material.
    pub async fn put_original_and_retained_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        source: (ResourceScope, SecretHandle),
        material: SecretMaterial,
        context: serde_json::Value,
        destination: (ResourceScope, SecretHandle),
    ) -> Result<Option<(SecretCustodyMetadata, SecretCustodyMetadata)>, SecretStoreError> {
        if source.1 == destination.1 {
            return Ok(None);
        }
        if crate::scoped_legacy_user_id(&source.0) != crate::scoped_legacy_user_id(&destination.0) {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "custody source owner mismatch".into(),
            });
        }
        self.lock_secret_records(
            transaction,
            &[(&source.0, &source.1), (&destination.0, &destination.1)],
        )
        .await?;
        let Some(original) = self
            .put_original_locked(transaction, source.0, source.1, material, context)
            .await?
        else {
            return Ok(None);
        };
        let reference = SecretCustodySource {
            scope: original.scope.clone(),
            handle: original.handle.clone(),
            reference: original.reference.clone(),
        };
        let Some(retained) = self
            .put_retained_from_original_locked(transaction, destination.0, destination.1, reference)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some((original, retained)))
    }

    /// Retain the first authenticated input and context in the caller's native
    /// transaction. Exact repeats read it without re-encryption or replacement.
    /// Close one exact custody identity under the same actual target writer lock.
    /// The caller owns its transaction and any product-level publication receipt.
    pub async fn close_identity_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        expected: crate::SecretIdentityClosure,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        if expected.source().is_some_and(|source| {
            crate::scoped_legacy_user_id(&source.scope) != crate::scoped_legacy_user_id(&scope)
        }) {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "custody source owner mismatch".into(),
            });
        }
        self.lock_secret_records(transaction, &[(&scope, &handle)])
            .await?;
        let record = self
            .inspect_secret_in_transaction(transaction, &scope, &handle)
            .await?;
        let reference = record
            .as_ref()
            .map(|r| self.encrypted_record_reference(&r.key, &handle, &r.ciphertext, r.version));
        let current = record
            .as_ref()
            .zip(reference.as_deref())
            .map(|(r, reference)| (&r.value, reference));
        match expected.decide(&scope, current) {
            crate::identity_closure::ClosureDecision::Reject => Ok(None),
            crate::identity_closure::ClosureDecision::Keep => Ok(record
                .as_ref()
                .and_then(|r| self.custody_metadata_for(r, &handle))),
            crate::identity_closure::ClosureDecision::Write(body) => {
                if let Some(record) = record {
                    let ciphertext = self.encode_custody(&scope, &handle, &body)?;
                    transaction.execute("UPDATE brassclaw_secrets SET ciphertext=$4,scope=$5,updated_at=now() WHERE tenant_id=$1 AND scope=$2 AND name=$3",
                        &[&self.tenant_id,&record.key,&handle.as_str(),&ciphertext,&secret_scope_str(&scope,&handle)]).await.map_err(map_pg_error)?;
                    self.custody_in_transaction(transaction, &scope, &handle)
                        .await
                } else {
                    self.insert_custody(transaction, &scope, &handle, &body)
                        .await
                }
            }
        }
    }

    pub async fn put_original_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        context: serde_json::Value,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        self.lock_secret_records(transaction, &[(&scope, &handle)])
            .await?;
        self.put_original_locked(transaction, scope, handle, material, context)
            .await
    }

    async fn put_original_locked(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        context: serde_json::Value,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        if let Some(record) = self
            .inspect_secret_in_transaction(transaction, &scope, &handle)
            .await?
        {
            let Some(CustodyBody::Original {
                scope: original_scope,
                context: original_context,
                material: original_material,
            }) = record.value.custody()
            else {
                return Ok(None);
            };
            return if original_scope == &scope
                && original_context == &context
                && crate::crypto::secret_values_match(
                    original_material.expose_secret(),
                    material.expose_secret(),
                ) {
                Ok(self.custody_metadata_for(&record, &handle))
            } else {
                Ok(None)
            };
        }
        let body = CustodyBody::Original {
            scope: scope.clone(),
            context,
            material,
        };
        self.insert_custody(transaction, &scope, &handle, &body)
            .await
    }

    /// Read actual authenticated custody metadata. Never locks, migrates or
    /// returns raw values; caller may use a read-only transaction.
    pub async fn custody_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        Ok(self
            .inspect_secret_in_transaction(transaction, scope, handle)
            .await?
            .and_then(|record| self.custody_metadata_for(&record, handle)))
    }

    /// Copy one exact live original into an immutable retained output under
    /// the same actual transaction/locks. Metadata alone cannot create material.
    pub async fn put_retained_from_original_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        source: SecretCustodySource,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        if crate::scoped_legacy_user_id(&scope) != crate::scoped_legacy_user_id(&source.scope) {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "custody source owner mismatch".into(),
            });
        }
        self.lock_secret_records(
            transaction,
            &[(&scope, &handle), (&source.scope, &source.handle)],
        )
        .await?;
        self.put_retained_from_original_locked(transaction, scope, handle, source)
            .await
    }

    async fn put_retained_from_original_locked(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        source: SecretCustodySource,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        if crate::scoped_legacy_user_id(&scope) != crate::scoped_legacy_user_id(&source.scope) {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "custody source owner mismatch".into(),
            });
        }
        if handle == source.handle {
            return Ok(None);
        }
        let Some(original) = self
            .inspect_secret_in_transaction(transaction, &source.scope, &source.handle)
            .await?
        else {
            return Ok(None);
        };
        if self.encrypted_record_reference(
            &original.key,
            &source.handle,
            &original.ciphertext,
            original.version,
        ) != source.reference
        {
            return Ok(None);
        }
        let DecodedSecretValue::Custody(body) = original.value else {
            return Ok(None);
        };
        let CustodyBody::Original {
            scope: original_scope,
            material,
            ..
        } = *body
        else {
            return Ok(None);
        };
        if original_scope != source.scope {
            return Ok(None);
        }
        if let Some(record) = self
            .inspect_secret_in_transaction(transaction, &scope, &handle)
            .await?
        {
            let Some(CustodyBody::Retained {
                scope: original_target_scope,
                source: original_source,
                material: original_material,
            }) = record.value.custody()
            else {
                return Ok(None);
            };
            return if original_target_scope == &scope
                && original_source == &source
                && crate::crypto::secret_values_match(
                    original_material.expose_secret(),
                    material.expose_secret(),
                ) {
                Ok(self.custody_metadata_for(&record, &handle))
            } else {
                Ok(None)
            };
        }
        let body = CustodyBody::Retained {
            scope: scope.clone(),
            source,
            material,
        };
        self.insert_custody(transaction, &scope, &handle, &body)
            .await
    }

    pub async fn put_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
    ) -> Result<SecretMetadata, SecretStoreError> {
        self.put_with_expiry_in_transaction(transaction, scope, handle, material, None)
            .await
    }

    /// Write a deadline authenticated with this generation within the caller's
    /// transaction; never derive a lifetime from account labels or lease TTL.
    pub async fn put_with_expiry_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        expires_at: Option<Timestamp>,
    ) -> Result<SecretMetadata, SecretStoreError> {
        self.put_checked_in_transaction(
            transaction,
            scope,
            handle,
            OrdinarySecretValue {
                material,
                expires_at,
            },
            None,
        )
        .await?
        .ok_or_else(|| SecretStoreError::StoreUnavailable {
            reason: "unconditional secret write was not applied".into(),
        })
    }

    /// Compare authenticated ciphertext identity and write inside the caller's
    /// transaction. None means the original reference changed; nothing is written.
    /// An absent expected reference also protects against a concurrent INSERT.
    pub async fn put_if_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        expected: Option<&str>,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        self.put_checked_in_transaction(
            transaction,
            scope,
            handle,
            OrdinarySecretValue {
                material,
                expires_at: None,
            },
            Some(expected),
        )
        .await
    }

    /// Conditional authenticated generation and lifetime in the caller's
    /// transaction. None expected means create-only, including concurrent
    /// insertions. Reuses the same row/advisory checks as ordinary secret CAS.
    pub async fn put_with_expiry_if_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        expires_at: Option<Timestamp>,
        expected: Option<&str>,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        self.put_checked_in_transaction(
            transaction,
            scope,
            handle,
            OrdinarySecretValue {
                material,
                expires_at,
            },
            Some(expected),
        )
        .await
    }

    async fn put_checked_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: ResourceScope,
        handle: SecretHandle,
        material: OrdinarySecretValue,
        expected: Option<Option<&str>>,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        let scope_key = secret_scope_str(&scope, &handle);
        let aad = pg_record_aad(
            &self.tenant_id,
            &scope_key,
            handle.as_str(),
            &filesystem_secret_aad(&scope, &handle),
        );
        self.lock_secret_records(transaction, &[(&scope, &handle)])
            .await?;
        let current = self
            .inspect_secret_in_transaction(transaction, &scope, &handle)
            .await?;
        let reference = current.as_ref().map(|record| {
            self.encrypted_record_reference(
                &record.key,
                &handle,
                &record.ciphertext,
                record.version,
            )
        });
        if let Some(expected) = expected
            && reference.as_deref() != expected
        {
            return Ok(None);
        }
        if current
            .as_ref()
            .and_then(|record| record.value.custody())
            .is_some()
        {
            return Ok(None);
        }
        let migrate_legacy = current
            .as_ref()
            .is_some_and(|record| record.key != scope_key);
        drop(current);
        if migrate_legacy {
            drop(self.load_secret(transaction, &scope, &handle).await?);
        }
        let encrypted = material.encrypt(&self.crypto, &aad).map_err(|_| {
            SecretStoreError::BackendMisconfigured {
                reason: "secret record encryption failed".into(),
            }
        })?;
        let encoded = encode_ciphertext(&encrypted.salt, &encrypted.ciphertext);
        let ciphertext = match encrypted.format {
            None => encoded,
            Some(TIMED_ORDINARY_FORMAT_V1) => {
                format!("{TIMED_ORDINARY_CIPHERTEXT_PREFIX}{encoded}")
            }
            Some(_) => {
                return Err(SecretStoreError::BackendMisconfigured {
                    reason: "unsupported ordinary secret encryption format".into(),
                });
            }
        };
        let affected = match expected {
            Some(None) => transaction.execute(
                "INSERT INTO brassclaw_secrets (tenant_id,scope,name,ciphertext,key_version) VALUES ($1,$2,$3,$4,1) ON CONFLICT (tenant_id,scope,name) DO NOTHING",
                &[&self.tenant_id,&scope_key,&handle.as_str(),&ciphertext],
            ).await,
            Some(Some(_)) => transaction.execute(
                "UPDATE brassclaw_secrets SET ciphertext=$4,updated_at=now() WHERE tenant_id=$1 AND scope=$2 AND name=$3",
                &[&self.tenant_id,&scope_key,&handle.as_str(),&ciphertext],
            ).await,
            None => transaction.execute(
                "INSERT INTO brassclaw_secrets (tenant_id,scope,name,ciphertext,key_version) VALUES ($1,$2,$3,$4,1) ON CONFLICT (tenant_id,scope,name) DO UPDATE SET ciphertext=excluded.ciphertext,updated_at=now()",
                &[&self.tenant_id,&scope_key,&handle.as_str(),&ciphertext],
            ).await,
        }.map_err(map_pg_error)?;
        Ok((affected == 1).then_some(SecretMetadata { scope, handle }))
    }

    /// Identify the authenticated encrypted record without exposing its value.
    /// This is a version reference, never a credential or an authorization grant.
    /// Randomized ciphertext (not plaintext) is hashed; legacy rows are first
    /// authenticated and upgraded under the existing row lock.
    pub async fn authenticated_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<String>, SecretStoreError> {
        self.lock_secret_rows(transaction, &[(scope, handle)])
            .await?;
        let Some(record) = self
            .inspect_secret_in_transaction(transaction, scope, handle)
            .await?
        else {
            return Ok(None);
        };
        if record.key == secret_scope_str(scope, handle) {
            return Ok(Some(self.encrypted_record_reference(
                &record.key,
                handle,
                &record.ciphertext,
                record.version,
            )));
        }
        drop(record);
        if self
            .load_secret(transaction, scope, handle)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let key = secret_scope_str(scope, handle);
        let row = transaction.query_one(
            "SELECT ciphertext,key_version FROM brassclaw_secrets WHERE tenant_id=$1 AND scope=$2 AND name=$3",
            &[&self.tenant_id, &key, &handle.as_str()],
        ).await.map_err(map_pg_error)?;
        let ciphertext: String = row.try_get(0).map_err(map_pg_error)?;
        let version: i32 = row.try_get(1).map_err(map_pg_error)?;
        Ok(Some(self.encrypted_record_reference(
            &key,
            handle,
            &ciphertext,
            version,
        )))
    }

    /// Authenticate metadata in the caller's read-only snapshot. Unlike a
    /// consuming read, this never locks/upgrades a legacy row or issues a lease.
    /// A legacy reference identifies those exact legacy bytes until migration.
    pub async fn inspect_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<String>, SecretStoreError> {
        let Some(record) = self
            .inspect_secret_in_transaction(transaction, scope, handle)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(self.encrypted_record_reference(
            &record.key,
            handle,
            &record.ciphertext,
            record.version,
        )))
    }

    /// Read/authenticate the selected bytes without locking or legacy migration.
    /// Material remains inside this storage owner; callers project only reference
    /// or boolean comparison. A conflicting legacy/current pair stays an error.
    async fn inspect_secret_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<AuthenticatedSecretRecord>, SecretStoreError> {
        let key = secret_scope_str(scope, handle);
        let legacy = legacy_secret_scope_str(scope, handle);
        let rows = transaction.query(
            "SELECT scope, ciphertext, key_version FROM brassclaw_secrets WHERE tenant_id=$1 AND (scope=$2 OR scope=$3) AND name=$4",
            &[&self.tenant_id, &key, &legacy, &handle.as_str()],
        ).await.map_err(map_pg_error)?;
        if rows.len() > 1 {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "conflicting current and legacy secret records require reconciliation"
                    .into(),
            });
        }
        let Some(row) = rows.first() else {
            return Ok(None);
        };
        let stored_key: String = row.get(0);
        let ciphertext: String = row.get(1);
        let version: i32 = row.get(2);
        let value =
            self.authenticate_secret_record(scope, handle, &stored_key, &ciphertext, version)?;
        Ok(Some(AuthenticatedSecretRecord {
            key: stored_key,
            ciphertext,
            version,
            value,
        }))
    }

    fn encrypted_record_reference(
        &self,
        key: &str,
        handle: &SecretHandle,
        ciphertext: &str,
        version: i32,
    ) -> String {
        let mut digest = Sha256::new();
        digest.update(b"brassclaw-encrypted-secret-reference/1\0");
        for value in [
            self.tenant_id.as_bytes(),
            key.as_bytes(),
            handle.as_str().as_bytes(),
            ciphertext.as_bytes(),
        ] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value);
        }
        digest.update(version.to_be_bytes());
        format!("{:x}", digest.finalize())
    }

    /// Compare an exact authenticated record in the caller's existing read
    /// snapshot. No raw value, lease, lock, migration or write crosses this port.
    /// A missing, changed or non-material record cannot match the selection.
    pub async fn inspect_matches_if_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected_reference: &str,
        candidate: &SecretMaterial,
    ) -> Result<bool, SecretStoreError> {
        let Some(record) = self
            .inspect_secret_in_transaction(transaction, scope, handle)
            .await?
        else {
            return Ok(false);
        };
        let reference = self.encrypted_record_reference(
            &record.key,
            handle,
            &record.ciphertext,
            record.version,
        );
        if reference != expected_reference {
            return Ok(false);
        }
        let Some(expected) = record.value.material_for_compare() else {
            return Ok(false);
        };
        Ok(crate::crypto::secret_values_match(
            expected.expose_secret(),
            candidate.expose_secret(),
        ))
    }

    /// Trusted publication-owner comparison inside its existing transaction.
    /// The expected fingerprint stays encrypted at rest and material never
    /// crosses this port. This neither issues a lease nor replaces a secret.
    /// As with other authenticated reads, legacy envelopes may be upgraded.
    pub async fn matches_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
        candidate: &SecretMaterial,
    ) -> Result<bool, SecretStoreError> {
        self.lock_secret_rows(transaction, &[(scope, handle)])
            .await?;
        let Some(record) = self
            .inspect_secret_in_transaction(transaction, scope, handle)
            .await?
        else {
            return Err(SecretStoreError::UnknownSecret {
                scope: Box::new(scope.clone()),
                handle: handle.clone(),
            });
        };
        if record.key == secret_scope_str(scope, handle) {
            let expected = record.value.material_for_compare().ok_or_else(|| {
                SecretStoreError::UnknownSecret {
                    scope: Box::new(scope.clone()),
                    handle: handle.clone(),
                }
            })?;
            return Ok(crate::crypto::secret_values_match(
                expected.expose_secret(),
                candidate.expose_secret(),
            ));
        }
        drop(record);
        let expected = self
            .load_secret(transaction, scope, handle)
            .await?
            .ok_or_else(|| SecretStoreError::UnknownSecret {
                scope: Box::new(scope.clone()),
                handle: handle.clone(),
            })?;
        Ok(crate::crypto::secret_values_match(
            expected.expose_secret(),
            candidate.expose_secret(),
        ))
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
        self.load_secret_selected(transaction, scope, handle, None)
            .await
    }

    async fn load_secret_selected(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected: Option<&str>,
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
        let stored_key: String = row.get(0);
        let ciphertext: String = row.get(1);
        let decoded =
            self.authenticate_secret_record(scope, handle, &stored_key, &ciphertext, row.get(2))?;
        // Compare before migrating legacy bytes. This exact row is locked
        // until the owner transaction ends, so a replacement cannot be selected
        // between the precondition and creation of the authenticated lease.
        if let Some(expected) = expected
            && self.encrypted_record_reference(&stored_key, handle, &ciphertext, row.get(2))
                != expected
        {
            return Ok(None);
        }
        let Some(plaintext) = decoded.into_runtime_material() else {
            // Immutable input and retired values never become runtime credentials.
            return Ok(None);
        };
        if stored_key == legacy {
            let new_aad = pg_record_aad(
                &self.tenant_id,
                &key,
                handle.as_str(),
                &filesystem_secret_aad(scope, handle),
            );
            let (nonce_and_ct, salt) = self
                .crypto
                .encrypt(plaintext.expose_secret().as_bytes(), &new_aad)
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
        Ok(Some(plaintext))
    }

    fn authenticate_secret_record(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        stored_key: &str,
        ciphertext: &str,
        version: i32,
    ) -> Result<DecodedSecretValue, SecretStoreError> {
        if version != 1 {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "unsupported secret master-key version".into(),
            });
        }
        let key = secret_scope_str(scope, handle);
        let (encoded, format) = match ciphertext.strip_prefix(CUSTODY_CIPHERTEXT_PREFIX) {
            Some(encoded) => {
                if stored_key != key {
                    return Err(SecretStoreError::StoreUnavailable {
                        reason: "custody record has invalid native identity".into(),
                    });
                }
                (encoded, Some(CUSTODY_FORMAT_V1))
            }
            None => match ciphertext.strip_prefix(TIMED_ORDINARY_CIPHERTEXT_PREFIX) {
                Some(encoded) if stored_key == key => (encoded, Some(TIMED_ORDINARY_FORMAT_V1)),
                Some(_) => {
                    return Err(SecretStoreError::StoreUnavailable {
                        reason: "expiring secret record has invalid native identity".into(),
                    });
                }
                None => (ciphertext, None),
            },
        };
        // Unknown purpose prefixes are not base64 and fail closed. Raw ordinary
        // material is never inspected or reinterpreted as structured JSON.
        let (salt, nonce_and_ct) = decode_ciphertext(encoded)?;
        let inner = filesystem_secret_aad(scope, handle);
        let aad = if stored_key == key {
            pg_record_aad(&self.tenant_id, &key, handle.as_str(), &inner)
        } else {
            inner
        };
        let value = decrypt_stored_value(&self.crypto, &aad, &nonce_and_ct, &salt, format)
            .map_err(|_| SecretStoreError::StoreUnavailable {
                reason: "secret record ownership or integrity verification failed".into(),
            })?;
        if let Some(body) = value.custody()
            && crate::scoped_legacy_user_id(body.scope()) != crate::scoped_legacy_user_id(scope)
        {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "custody record has invalid stored owner".into(),
            });
        }
        Ok(value)
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
    async fn close_identity(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        expected: crate::SecretIdentityClosure,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let metadata = self
            .close_identity_in_transaction(&transaction, scope, handle, expected)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(metadata)
    }

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

    async fn put_with_expiry(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        expires_at: Option<Timestamp>,
    ) -> Result<SecretMetadata, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let metadata = self
            .put_with_expiry_in_transaction(&transaction, scope, handle, material, expires_at)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(metadata)
    }

    async fn record_reference(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<String>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client
            .build_transaction()
            .read_only(true)
            .start()
            .await
            .map_err(map_pg_error)?;
        let reference = self
            .inspect_record_reference_in_transaction(&transaction, scope, handle)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(reference)
    }

    async fn put_if_record_reference(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        expected: Option<&str>,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let written = self
            .put_if_record_reference_in_transaction(&transaction, scope, handle, material, expected)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(written)
    }

    async fn matches(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        candidate: &SecretMaterial,
    ) -> Result<bool, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client
            .build_transaction()
            .read_only(true)
            .start()
            .await
            .map_err(map_pg_error)?;
        let Some(record) = self
            .inspect_secret_in_transaction(&transaction, scope, handle)
            .await?
        else {
            return Err(SecretStoreError::UnknownSecret {
                scope: Box::new(scope.clone()),
                handle: handle.clone(),
            });
        };
        let matches = crate::crypto::secret_values_match(
            record
                .value
                .material_for_compare()
                .ok_or_else(|| SecretStoreError::UnknownSecret {
                    scope: Box::new(scope.clone()),
                    handle: handle.clone(),
                })?
                .expose_secret(),
            candidate.expose_secret(),
        );
        drop(record);
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(matches)
    }

    async fn put_original(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        context: serde_json::Value,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let metadata = self
            .put_original_in_transaction(&transaction, scope, handle, material, context)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(metadata)
    }
    async fn custody(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client
            .build_transaction()
            .read_only(true)
            .start()
            .await
            .map_err(map_pg_error)?;
        let metadata = self
            .custody_in_transaction(&transaction, scope, handle)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(metadata)
    }
    async fn put_retained_from_original(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        source: SecretCustodySource,
    ) -> Result<Option<SecretCustodyMetadata>, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let metadata = self
            .put_retained_from_original_in_transaction(&transaction, scope, handle, source)
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
        self.lock_secret_records(&transaction, &[(scope, handle)])
            .await?;
        let Some(record) = self
            .inspect_secret_in_transaction(&transaction, scope, handle)
            .await?
        else {
            return Ok(false);
        };
        let reference = self.encrypted_record_reference(
            &record.key,
            handle,
            &record.ciphertext,
            record.version,
        );
        let changed = match record.value {
            DecodedSecretValue::Custody(body) => {
                if body.is_retired() {
                    return Ok(false);
                }
                let retired = (*body).retire(reference);
                let ciphertext = self.encode_custody(scope, handle, &retired)?;
                transaction.execute("UPDATE brassclaw_secrets SET ciphertext=$4,updated_at=now() WHERE tenant_id=$1 AND scope=$2 AND name=$3", &[&self.tenant_id, &record.key, &handle.as_str(), &ciphertext]).await.map_err(map_pg_error)?
            }
            DecodedSecretValue::Ordinary(material) => {
                drop(material);
                // Delete the authenticated actual key, including an old legacy
                // key, without encrypting a replacement merely to delete it.
                transaction
                    .execute(
                        "DELETE FROM brassclaw_secrets WHERE tenant_id=$1 AND scope=$2 AND name=$3",
                        &[&self.tenant_id, &record.key, &handle.as_str()],
                    )
                    .await
                    .map_err(map_pg_error)?
            }
        };
        transaction.commit().await.map_err(map_pg_error)?;
        Ok(changed == 1)
    }

    async fn lease_once(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<SecretLease, SecretStoreError> {
        self.issue_selected_lease(scope, handle, None)
            .await?
            .ok_or_else(|| SecretStoreError::UnknownSecret {
                scope: Box::new(scope.clone()),
                handle: handle.clone(),
            })
    }

    async fn lease_once_if_record_reference(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected: &str,
    ) -> Result<Option<SecretLease>, SecretStoreError> {
        self.issue_selected_lease(scope, handle, Some(expected))
            .await
    }

    async fn consume(
        &self,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretMaterial, SecretStoreError> {
        let mut client = self.pool.get().await.map_err(map_pg_pool_error)?;
        let transaction = client.transaction().await.map_err(map_pg_error)?;
        let material = self
            .consume_selected_in_transaction(&transaction, scope, lease_id)
            .await?;
        transaction.commit().await.map_err(map_pg_error)?;
        if material
            .expires_at
            .is_some_and(|expiry| expiry <= Utc::now())
        {
            return Err(SecretStoreError::SecretExpired);
        }
        Ok(material.material)
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
            secret_expires_at: payload.secret_expires_at,
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
                    status: if payload
                        .secret_expires_at
                        .is_some_and(|expiry| expiry <= Utc::now())
                    {
                        SecretLeaseStatus::Expired
                    } else {
                        SecretLeaseStatus::Active
                    },
                    secret_expires_at: payload.secret_expires_at,
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
