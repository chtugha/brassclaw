//! Ported legacy secret storage contracts used by Reborn adapters.
//!
//! These types mirror the existing `src/secrets/{types,store}.rs` behavior:
//! encrypted records, redacted Debug output, decrypted material exposed only via
//! an explicit host-boundary method, and an encrypted in-memory store for tests.

use std::{collections::HashMap, fmt, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::crypto::{secret_record_aad, secret_values_match};
use crate::custody::{
    CUSTODY_FORMAT_V1, CustodyBody, DecodedSecretValue, OrdinarySecretValue,
    TIMED_ORDINARY_FORMAT_V1, decrypt_stored_value, encrypt_body,
};
use crate::{SecretCustodyMetadata, SecretCustodySource, SecretsCrypto};
use brassclaw_host_api::{ResourceScope, SecretHandle};

#[derive(Clone)]
pub struct Secret {
    pub id: Uuid,
    pub user_id: String,
    pub name: String,
    pub encrypted_value: Vec<u8>,
    pub key_salt: Vec<u8>,
    pub(crate) custody_format: Option<u8>,
    pub provider: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub usage_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Secret")
            .field("id", &self.id)
            .field("user_id", &self.user_id)
            .field("name", &self.name)
            .field("encrypted_value", &"[REDACTED]")
            .field("key_salt", &"[REDACTED]")
            .field("provider", &self.provider)
            .field("expires_at", &self.expires_at)
            .field("last_used_at", &self.last_used_at)
            .field("usage_count", &self.usage_count)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    pub name: String,
    pub provider: Option<String>,
}

pub struct DecryptedSecret {
    value: SecretString,
}

impl DecryptedSecret {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, SecretError> {
        let value = String::from_utf8(bytes).map_err(|_| SecretError::InvalidUtf8)?;
        Ok(Self {
            value: SecretString::from(value),
        })
    }

    pub fn expose(&self) -> &str {
        self.value.expose_secret()
    }

    /// Preserve zeroizing ownership when another storage-owner decoder takes
    /// the value. This is private infrastructure, not a new raw-value port.
    pub(crate) fn into_material(self) -> SecretString {
        self.value
    }

    pub fn len(&self) -> usize {
        self.value.expose_secret().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Debug for DecryptedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "DecryptedSecret([REDACTED, {} bytes])",
            self.len()
        )
    }
}

impl Clone for DecryptedSecret {
    fn clone(&self) -> Self {
        Self {
            value: SecretString::from(self.value.expose_secret().to_string()),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SecretError {
    #[error("Secret not found: {0}")]
    NotFound(String),
    #[error("Secret has expired")]
    Expired,
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Invalid master key")]
    InvalidMasterKey,
    #[error("Secret value is not valid UTF-8")]
    InvalidUtf8,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Secret access denied for tool")]
    AccessDenied,
    #[error("Keychain error: {0}")]
    KeychainError(String),
}

#[derive(Debug)]
pub struct CreateSecretParams {
    pub name: String,
    pub value: SecretString,
    pub provider: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CreateSecretParams {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into().to_lowercase(),
            value: SecretString::from(value.into()),
            provider: None,
            expires_at: None,
        }
    }

    pub fn from_secret(name: impl Into<String>, value: SecretString) -> Self {
        Self {
            name: name.into().to_lowercase(),
            value,
            provider: None,
            expires_at: None,
        }
    }

    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretConsumeResult {
    Matched,
    Mismatched,
    NotFound,
}

#[async_trait]
pub trait SecretsStore: Send + Sync {
    async fn create(
        &self,
        user_id: &str,
        params: CreateSecretParams,
    ) -> Result<Secret, SecretError>;
    /// Trusted authenticated generation projection; not access or eligibility.
    async fn record_reference(
        &self,
        user_id: &str,
        name: &str,
    ) -> Result<Option<String>, SecretError>;
    /// Exact encrypted generation CAS. None protects absence, not unconditional write.
    async fn create_if_record_reference(
        &self,
        user_id: &str,
        params: CreateSecretParams,
        expected: Option<&str>,
    ) -> Result<Option<Secret>, SecretError>;
    /// Compare inside the owner without returning material or consuming a secret.
    async fn matches_material(
        &self,
        user_id: &str,
        name: &str,
        candidate: &SecretString,
    ) -> Result<bool, SecretError>;
    async fn close_custody_identity(
        &self,
        user_id: &str,
        scope: ResourceScope,
        handle: SecretHandle,
        expected: crate::SecretIdentityClosure,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError>;
    async fn create_original(
        &self,
        user_id: &str,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretString,
        context: serde_json::Value,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError>;
    async fn custody(
        &self,
        user_id: &str,
        handle: &SecretHandle,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError>;
    async fn create_retained_from_original(
        &self,
        user_id: &str,
        scope: ResourceScope,
        handle: SecretHandle,
        source: SecretCustodySource,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError>;
    async fn get(&self, user_id: &str, name: &str) -> Result<Secret, SecretError>;
    /// Authenticate and select this exact encrypted record under the owner's
    /// read lock. This returns encrypted metadata, not a lease or raw material.
    async fn get_if_record_reference(
        &self,
        user_id: &str,
        name: &str,
        expected: &str,
    ) -> Result<Option<Secret>, SecretError>;
    async fn get_decrypted(
        &self,
        user_id: &str,
        name: &str,
    ) -> Result<DecryptedSecret, SecretError>;

    /// Must compare and remove/retire inside the actual owner atomically.
    async fn consume_if_matches(
        &self,
        user_id: &str,
        name: &str,
        expected_value: &str,
    ) -> Result<SecretConsumeResult, SecretError>;

    async fn exists(&self, user_id: &str, name: &str) -> Result<bool, SecretError>;
    async fn any_exist(&self) -> Result<bool, SecretError>;
    async fn list(&self, user_id: &str) -> Result<Vec<SecretRef>, SecretError>;
    async fn delete(&self, user_id: &str, name: &str) -> Result<bool, SecretError>;
    async fn record_usage(&self, secret_id: Uuid) -> Result<(), SecretError>;
    async fn is_accessible(
        &self,
        user_id: &str,
        secret_name: &str,
        allowed_secrets: &[String],
    ) -> Result<bool, SecretError>;
}

#[derive(Debug)]
pub(crate) struct InMemorySecretsStore {
    secrets: RwLock<HashMap<(String, String), Secret>>,
    crypto: Arc<SecretsCrypto>,
}

impl InMemorySecretsStore {
    pub(crate) fn new(crypto: Arc<SecretsCrypto>) -> Self {
        Self {
            secrets: RwLock::new(HashMap::new()),
            crypto,
        }
    }

    fn decode(
        &self,
        user_id: &str,
        name: &str,
        secret: &Secret,
    ) -> Result<DecodedSecretValue, SecretError> {
        if secret.user_id != user_id || secret.name != name.to_lowercase() {
            return Err(SecretError::AccessDenied);
        }
        let value = decrypt_stored_value(
            &self.crypto,
            &secret_record_aad(user_id, &secret.name),
            &secret.encrypted_value,
            &secret.key_salt,
            secret.custody_format,
        )?;
        if secret.custody_format == Some(TIMED_ORDINARY_FORMAT_V1)
            && secret.expires_at != value.secret_expires_at()
        {
            return Err(SecretError::AccessDenied);
        }
        if let Some(body) = value.custody()
            && (secret.expires_at.is_some()
                || crate::scoped_legacy_user_id(body.scope()) != user_id)
        {
            return Err(SecretError::AccessDenied);
        }
        Ok(value)
    }

    fn custody_record(
        &self,
        user_id: &str,
        name: &str,
        body: &CustodyBody,
    ) -> Result<Secret, SecretError> {
        if crate::scoped_legacy_user_id(body.scope()) != user_id {
            return Err(SecretError::AccessDenied);
        }
        let (encrypted_value, key_salt) =
            encrypt_body(&self.crypto, &secret_record_aad(user_id, name), body)?;
        let now = Utc::now();
        Ok(Secret {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            name: name.into(),
            encrypted_value,
            key_salt,
            custody_format: Some(CUSTODY_FORMAT_V1),
            provider: None,
            expires_at: None,
            last_used_at: None,
            usage_count: 0,
            created_at: now,
            updated_at: now,
        })
    }

    fn metadata_for(
        secret: &Secret,
        body: &CustodyBody,
    ) -> Result<SecretCustodyMetadata, SecretError> {
        let handle = SecretHandle::new(secret.name.clone())
            .map_err(|_| SecretError::Database("invalid stored custody handle".into()))?;
        Ok(body.metadata(handle, encrypted_legacy_reference(secret)))
    }

    fn retire_record(&self, secret: &Secret, body: CustodyBody) -> Result<Secret, SecretError> {
        let retired = body.retire(encrypted_legacy_reference(secret));
        let (encrypted_value, key_salt) = encrypt_body(
            &self.crypto,
            &secret_record_aad(&secret.user_id, &secret.name),
            &retired,
        )?;
        let mut record = secret.clone();
        record.encrypted_value = encrypted_value;
        record.key_salt = key_salt;
        record.updated_at = Utc::now();
        Ok(record)
    }
}

fn encrypted_legacy_reference(secret: &Secret) -> String {
    let mut digest = Sha256::new();
    digest.update(b"brassclaw-legacy-encrypted-secret-reference/1\0");
    let expiry = secret
        .expires_at
        .map(|value| value.to_rfc3339())
        .unwrap_or_default();
    for value in [
        secret.id.as_bytes().as_slice(),
        secret.user_id.as_bytes(),
        secret.name.as_bytes(),
        secret.provider.as_deref().unwrap_or_default().as_bytes(),
        secret.key_salt.as_slice(),
        secret.encrypted_value.as_slice(),
        expiry.as_bytes(),
    ] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    }
    format!("{:x}", digest.finalize())
}

#[async_trait]
impl SecretsStore for InMemorySecretsStore {
    async fn close_custody_identity(
        &self,
        user_id: &str,
        scope: ResourceScope,
        handle: SecretHandle,
        expected: crate::SecretIdentityClosure,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError> {
        if crate::scoped_legacy_user_id(&scope) != user_id
            || expected
                .source()
                .is_some_and(|source| crate::scoped_legacy_user_id(&source.scope) != user_id)
        {
            return Err(SecretError::AccessDenied);
        }
        let name = handle.as_str().to_lowercase();
        SecretHandle::new(name.clone()).map_err(|_| SecretError::AccessDenied)?;
        let key = (user_id.to_string(), name.clone());
        let mut secrets = self.secrets.write().await;
        let prior = secrets.get(&key);
        let decoded = prior
            .map(|record| self.decode(user_id, &name, record))
            .transpose()?;
        let reference = prior.map(encrypted_legacy_reference);
        let current = decoded.as_ref().zip(reference.as_deref());
        match expected.decide(&scope, current) {
            crate::identity_closure::ClosureDecision::Reject => Ok(None),
            crate::identity_closure::ClosureDecision::Keep => {
                let record = prior.ok_or(SecretError::AccessDenied)?;
                let body = decoded
                    .as_ref()
                    .and_then(|value| value.custody())
                    .ok_or(SecretError::AccessDenied)?;
                Self::metadata_for(record, body).map(Some)
            }
            crate::identity_closure::ClosureDecision::Write(body) => {
                let mut record = self.custody_record(user_id, &name, &body)?;
                if let Some(old) = prior {
                    let mut preserved = old.clone();
                    preserved.encrypted_value = record.encrypted_value;
                    preserved.key_salt = record.key_salt;
                    preserved.custody_format = record.custody_format;
                    preserved.updated_at = record.updated_at;
                    record = preserved;
                }
                let metadata = Self::metadata_for(&record, &body)?;
                secrets.insert(key, record);
                Ok(Some(metadata))
            }
        }
    }

    async fn create(
        &self,
        user_id: &str,
        mut params: CreateSecretParams,
    ) -> Result<Secret, SecretError> {
        params.name = params.name.to_lowercase();
        let key = (user_id.to_string(), params.name.clone());
        let mut secrets = self.secrets.write().await;
        if let Some(current) = secrets.get(&key)
            && self
                .decode(user_id, &params.name, current)?
                .custody()
                .is_some()
        {
            return Err(SecretError::AccessDenied);
        }
        let value = OrdinarySecretValue {
            material: params.value,
            expires_at: params.expires_at,
        };
        let encrypted = value.encrypt(&self.crypto, &secret_record_aad(user_id, &params.name))?;
        let now = Utc::now();
        let record = Secret {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            name: params.name,
            encrypted_value: encrypted.ciphertext,
            key_salt: encrypted.salt,
            custody_format: encrypted.format,
            provider: params.provider,
            expires_at: params.expires_at,
            last_used_at: None,
            usage_count: 0,
            created_at: now,
            updated_at: now,
        };
        secrets.insert(key, record.clone());
        Ok(record)
    }

    async fn record_reference(
        &self,
        user_id: &str,
        name: &str,
    ) -> Result<Option<String>, SecretError> {
        let secrets = self.secrets.read().await;
        let Some(secret) = secrets.get(&(user_id.to_string(), name.to_lowercase())) else {
            return Ok(None);
        };
        drop(self.decode(user_id, name, secret)?);
        Ok(Some(encrypted_legacy_reference(secret)))
    }

    async fn create_if_record_reference(
        &self,
        user_id: &str,
        mut params: CreateSecretParams,
        expected: Option<&str>,
    ) -> Result<Option<Secret>, SecretError> {
        params.name = params.name.to_lowercase();
        let key = (user_id.to_string(), params.name.clone());
        let mut secrets = self.secrets.write().await;
        let current = secrets.get(&key);
        let decoded = current
            .map(|secret| self.decode(user_id, &params.name, secret))
            .transpose()?;
        if current.map(encrypted_legacy_reference).as_deref() != expected {
            return Ok(None);
        }
        if decoded
            .as_ref()
            .and_then(DecodedSecretValue::custody)
            .is_some()
        {
            return Ok(None);
        }
        let value = OrdinarySecretValue {
            material: params.value,
            expires_at: params.expires_at,
        };
        let encrypted = value.encrypt(&self.crypto, &secret_record_aad(user_id, &params.name))?;
        let now = Utc::now();
        let record = Secret {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            name: params.name,
            encrypted_value: encrypted.ciphertext,
            key_salt: encrypted.salt,
            custody_format: encrypted.format,
            provider: params.provider,
            expires_at: params.expires_at,
            last_used_at: None,
            usage_count: 0,
            created_at: now,
            updated_at: now,
        };
        secrets.insert(key, record.clone());
        Ok(Some(record))
    }

    async fn matches_material(
        &self,
        user_id: &str,
        name: &str,
        candidate: &SecretString,
    ) -> Result<bool, SecretError> {
        let secrets = self.secrets.read().await;
        let secret = secrets
            .get(&(user_id.to_string(), name.to_lowercase()))
            .ok_or_else(|| SecretError::NotFound(name.into()))?;
        let decoded = self.decode(user_id, name, secret)?;
        let material = decoded
            .material_for_compare()
            .ok_or_else(|| SecretError::NotFound(name.into()))?;
        Ok(secret_values_match(
            material.expose_secret(),
            candidate.expose_secret(),
        ))
    }

    async fn create_original(
        &self,
        user_id: &str,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretString,
        context: serde_json::Value,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError> {
        if crate::scoped_legacy_user_id(&scope) != user_id {
            return Err(SecretError::AccessDenied);
        }
        let name = handle.as_str().to_lowercase();
        SecretHandle::new(name.clone()).map_err(|_| SecretError::AccessDenied)?;
        let key = (user_id.to_string(), name.clone());
        let mut secrets = self.secrets.write().await;
        if let Some(record) = secrets.get(&key) {
            let decoded = self.decode(user_id, &name, record)?;
            let Some(body) = decoded.custody() else {
                return Ok(None);
            };
            let CustodyBody::Original {
                scope: original_scope,
                context: original_context,
                material: original_material,
            } = body
            else {
                return Ok(None);
            };
            return if original_scope == &scope
                && original_context == &context
                && secret_values_match(original_material.expose_secret(), material.expose_secret())
            {
                Self::metadata_for(record, body).map(Some)
            } else {
                Ok(None)
            };
        }
        let body = CustodyBody::Original {
            scope,
            context,
            material,
        };
        let record = self.custody_record(user_id, &name, &body)?;
        let metadata = Self::metadata_for(&record, &body)?;
        secrets.insert(key, record);
        Ok(Some(metadata))
    }

    async fn custody(
        &self,
        user_id: &str,
        handle: &SecretHandle,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError> {
        let name = handle.as_str().to_lowercase();
        let secrets = self.secrets.read().await;
        let Some(record) = secrets.get(&(user_id.to_string(), name.clone())) else {
            return Ok(None);
        };
        let decoded = self.decode(user_id, &name, record)?;
        decoded
            .custody()
            .map(|body| Self::metadata_for(record, body))
            .transpose()
    }

    async fn create_retained_from_original(
        &self,
        user_id: &str,
        scope: ResourceScope,
        handle: SecretHandle,
        source: SecretCustodySource,
    ) -> Result<Option<SecretCustodyMetadata>, SecretError> {
        if crate::scoped_legacy_user_id(&scope) != user_id
            || crate::scoped_legacy_user_id(&source.scope) != user_id
        {
            return Err(SecretError::AccessDenied);
        }
        let name = handle.as_str().to_lowercase();
        SecretHandle::new(name.clone()).map_err(|_| SecretError::AccessDenied)?;
        let original_name = source.handle.as_str().to_lowercase();
        if original_name == name {
            return Ok(None);
        }
        let mut secrets = self.secrets.write().await;
        let Some(original) = secrets.get(&(user_id.to_string(), original_name.clone())) else {
            return Ok(None);
        };
        let decoded = self.decode(user_id, &original_name, original)?;
        let DecodedSecretValue::Custody(body) = decoded else {
            return Ok(None);
        };
        let CustodyBody::Original {
            scope: origin_scope,
            material,
            ..
        } = *body
        else {
            return Ok(None);
        };
        if origin_scope != source.scope || encrypted_legacy_reference(original) != source.reference
        {
            return Ok(None);
        }
        let key = (user_id.to_string(), name.clone());
        if let Some(record) = secrets.get(&key) {
            let decoded = self.decode(user_id, &name, record)?;
            let Some(body) = decoded.custody() else {
                return Ok(None);
            };
            let CustodyBody::Retained {
                scope: target_scope,
                source: existing_source,
                material: existing_material,
            } = body
            else {
                return Ok(None);
            };
            return if target_scope == &scope
                && existing_source == &source
                && secret_values_match(existing_material.expose_secret(), material.expose_secret())
            {
                Self::metadata_for(record, body).map(Some)
            } else {
                Ok(None)
            };
        }
        let body = CustodyBody::Retained {
            scope,
            source,
            material,
        };
        let record = self.custody_record(user_id, &name, &body)?;
        let metadata = Self::metadata_for(&record, &body)?;
        secrets.insert(key, record);
        Ok(Some(metadata))
    }

    async fn get(&self, user_id: &str, name: &str) -> Result<Secret, SecretError> {
        let name = name.to_lowercase();
        let secret = self
            .secrets
            .read()
            .await
            .get(&(user_id.to_string(), name.clone()))
            .cloned()
            .ok_or_else(|| SecretError::NotFound(name.clone()))?;
        if secret.custody_format.is_some()
            && !self.decode(user_id, &name, &secret)?.runtime_eligible()
        {
            return Err(SecretError::NotFound(name));
        }
        if secret.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Err(SecretError::Expired);
        }
        Ok(secret)
    }

    async fn get_if_record_reference(
        &self,
        user_id: &str,
        name: &str,
        expected: &str,
    ) -> Result<Option<Secret>, SecretError> {
        let name = name.to_lowercase();
        let secrets = self.secrets.read().await;
        let Some(secret) = secrets.get(&(user_id.to_string(), name.clone())) else {
            return Ok(None);
        };
        let decoded = self.decode(user_id, &name, secret)?;
        if encrypted_legacy_reference(secret) != expected || !decoded.runtime_eligible() {
            return Ok(None);
        }
        if secret.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Err(SecretError::Expired);
        }
        Ok(Some(secret.clone()))
    }

    async fn get_decrypted(
        &self,
        user_id: &str,
        name: &str,
    ) -> Result<DecryptedSecret, SecretError> {
        let name = name.to_lowercase();
        let secret = self
            .secrets
            .read()
            .await
            .get(&(user_id.to_string(), name.clone()))
            .cloned()
            .ok_or_else(|| SecretError::NotFound(name.clone()))?;
        if secret.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Err(SecretError::Expired);
        }
        let decoded = self.decode(user_id, &name, &secret)?;
        if secret.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Err(SecretError::Expired);
        }
        decoded
            .into_runtime_material()
            .map(|value| DecryptedSecret { value })
            .ok_or(SecretError::NotFound(name))
    }

    async fn consume_if_matches(
        &self,
        user_id: &str,
        name: &str,
        expected_value: &str,
    ) -> Result<SecretConsumeResult, SecretError> {
        let name = name.to_lowercase();
        let key = (user_id.to_string(), name.clone());
        let mut secrets = self.secrets.write().await;
        let Some(secret) = secrets.get(&key).cloned() else {
            return Ok(SecretConsumeResult::NotFound);
        };
        if secret.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Err(SecretError::Expired);
        }
        let decoded = self.decode(user_id, &name, &secret)?;
        if !decoded.runtime_eligible() {
            return Ok(SecretConsumeResult::NotFound);
        }
        let material = decoded
            .material_for_compare()
            .ok_or(SecretError::NotFound(name))?;
        if !secret_values_match(material.expose_secret(), expected_value) {
            return Ok(SecretConsumeResult::Mismatched);
        }
        if secret.expires_at.is_some_and(|expiry| expiry <= Utc::now()) {
            return Err(SecretError::Expired);
        }
        match decoded {
            DecodedSecretValue::Ordinary(_) => {
                secrets.remove(&key);
            }
            DecodedSecretValue::Custody(body) => {
                secrets.insert(key, self.retire_record(&secret, *body)?);
            }
        }
        Ok(SecretConsumeResult::Matched)
    }

    async fn exists(&self, user_id: &str, name: &str) -> Result<bool, SecretError> {
        let secrets = self.secrets.read().await;
        let Some(secret) = secrets.get(&(user_id.to_string(), name.to_lowercase())) else {
            return Ok(false);
        };
        if secret.custody_format.is_some() {
            return Ok(self.decode(user_id, name, secret)?.runtime_eligible());
        }
        Ok(true)
    }
    async fn any_exist(&self) -> Result<bool, SecretError> {
        // Internal/retired ciphertext still needs the key, even when no runtime
        // credential is advertised. Never fabricate an empty owner.
        Ok(!self.secrets.read().await.is_empty())
    }
    async fn list(&self, user_id: &str) -> Result<Vec<SecretRef>, SecretError> {
        let secrets = self.secrets.read().await;
        let mut visible = Vec::new();
        for ((owner, name), secret) in secrets.iter() {
            if owner != user_id {
                continue;
            }
            if secret.custody_format.is_some()
                && !self.decode(user_id, name, secret)?.runtime_eligible()
            {
                continue;
            }
            visible.push(SecretRef {
                name: secret.name.clone(),
                provider: secret.provider.clone(),
            });
        }
        Ok(visible)
    }
    async fn delete(&self, user_id: &str, name: &str) -> Result<bool, SecretError> {
        let name = name.to_lowercase();
        let key = (user_id.to_string(), name.clone());
        let mut secrets = self.secrets.write().await;
        let Some(secret) = secrets.get(&key).cloned() else {
            return Ok(false);
        };
        match self.decode(user_id, &name, &secret)? {
            DecodedSecretValue::Ordinary(_) => {
                secrets.remove(&key);
            }
            DecodedSecretValue::Custody(body) => {
                if body.is_retired() {
                    return Ok(false);
                }
                secrets.insert(key, self.retire_record(&secret, *body)?);
            }
        }
        Ok(true)
    }
    async fn record_usage(&self, secret_id: Uuid) -> Result<(), SecretError> {
        let mut secrets = self.secrets.write().await;
        let secret = secrets
            .values_mut()
            .find(|secret| secret.id == secret_id)
            .ok_or_else(|| SecretError::NotFound(secret_id.to_string()))?;
        if secret.custody_format.is_some()
            && !self
                .decode(&secret.user_id, &secret.name, secret)?
                .runtime_eligible()
        {
            return Err(SecretError::NotFound(secret_id.to_string()));
        }
        let usage_count = secret
            .usage_count
            .checked_add(1)
            .ok_or_else(|| SecretError::Database("secret usage counter is exhausted".into()))?;
        secret.last_used_at = Some(Utc::now());
        secret.usage_count = usage_count;
        Ok(())
    }
    async fn is_accessible(
        &self,
        user_id: &str,
        secret_name: &str,
        allowed_secrets: &[String],
    ) -> Result<bool, SecretError> {
        let name = secret_name.to_lowercase();
        if !self.exists(user_id, &name).await? {
            return Ok(false);
        }
        for pattern in allowed_secrets {
            let pattern = pattern.to_lowercase();
            if pattern == name
                || pattern
                    .strip_suffix('*')
                    .is_some_and(|prefix| name.starts_with(prefix))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod usage_counter_tests {
    use super::*;
    use crate::{ScopedSecretsStoreAdapter, SecretStore, SecretStoreError};
    use brassclaw_host_api::{InvocationId, UserId};

    #[tokio::test]
    async fn actual_consume_fails_before_material_when_usage_counter_is_exhausted() {
        let inner = Arc::new(InMemorySecretsStore::new(Arc::new(
            SecretsCrypto::new("actual-counter-owner-master-key-2026".into()).unwrap(),
        )));
        let store = ScopedSecretsStoreAdapter::new(inner.clone());
        let scope = ResourceScope::local_default(
            UserId::new("counter-owner").unwrap(),
            InvocationId::new(),
        )
        .unwrap();
        let handle = SecretHandle::new("finite-counter-source").unwrap();
        store
            .put(
                scope.clone(),
                handle.clone(),
                "protected counter material".into(),
            )
            .await
            .unwrap();
        let lease = store.lease_once(&scope, &handle).await.unwrap();
        let owner = crate::scoped_legacy_user_id(&scope);
        {
            let mut records = inner.secrets.write().await;
            records
                .get_mut(&(owner.clone(), handle.to_string()))
                .unwrap()
                .usage_count = i64::MAX;
        }
        let before = inner.get(&owner, handle.as_str()).await.unwrap();
        assert!(matches!(
            store.consume(&scope, lease.id).await,
            Err(SecretStoreError::StoreUnavailable { .. })
        ));
        let after = inner.get(&owner, handle.as_str()).await.unwrap();
        assert_eq!(after.usage_count, i64::MAX);
        assert_eq!(after.last_used_at, before.last_used_at);
    }
}
