//! Operator-scoped storage for LLM provider API-key **values**.
//!
//! The PostgreSQL provider catalogue only ever references an
//! `api_key_env` *name* and the config selection only carries names — inline
//! secret values are rejected. When the webui2 settings surface lets an
//! operator paste an actual key, the value lands here instead: encrypted in the
//! scoped [`SecretStore`] under a fixed per-provider handle, and injected into
//! the resolved `LlmConfig` at provider-build / reload time.
//!
//! LLM configuration is operator-wide (a single instance config, not per-user),
//! so every key is stored under the synthetic system scope
//! ([`ResourceScope::system`]). The handle is derived from the provider id:
//! `llm_provider_<id>_api_key`.

use std::sync::Arc;

use brassclaw_host_api::{ResourceScope, SecretHandle};
use brassclaw_secrets::{SecretMaterial, SecretStore, SecretStoreError};
use thiserror::Error;

/// Thin, operator-scoped wrapper over the shared [`SecretStore`] for LLM keys.
#[derive(Clone)]
pub struct LlmKeyStore {
    store: Arc<dyn SecretStore>,
    #[cfg(feature = "postgres")]
    postgres: Option<Arc<brassclaw_secrets::PgSecretStore>>,
}

impl LlmKeyStore {
    /// Recheck the prepared usage under the existing authenticated secret-row
    /// lock before committing its selection. Codex selects its OAuth head;
    /// API-key protocols keep the original transactional reader unchanged.
    /// This does not acquire tokens, accounts or leases, or refresh a session.
    #[cfg(feature = "postgres")]
    pub(crate) async fn record_usage_credential_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        definition: &brassclaw_llm::ProviderDefinition,
    ) -> Result<Option<String>, LlmKeyStoreError> {
        if definition.protocol != brassclaw_llm::ProviderProtocol::OpenAiCodex {
            return self
                .record_reference_in_transaction(transaction, &definition.id)
                .await;
        }
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "native credential metadata requires its PostgreSQL secret owner".into(),
            })
        })?;
        store
            .authenticated_record_reference_in_transaction(
                transaction,
                &scope(),
                &oauth_handle_for(&definition.id)?,
            )
            .await
            .map_err(LlmKeyStoreError::Store)
    }

    /// Identity of the credential selected by the actual retained protocol.
    /// Codex selects its authenticated native login head, never an API-key row
    /// or decrypted session JSON. This read has no lease or refresh effect.
    #[cfg(feature = "postgres")]
    pub(crate) async fn inspect_usage_credential_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        definition: &brassclaw_llm::ProviderDefinition,
    ) -> Result<Option<String>, LlmKeyStoreError> {
        if definition.protocol != brassclaw_llm::ProviderProtocol::OpenAiCodex {
            return self
                .inspect_record_reference_in_transaction(transaction, &definition.id)
                .await;
        }
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "native credential metadata requires its PostgreSQL secret owner".into(),
            })
        })?;
        store
            .inspect_record_reference_in_transaction(
                transaction,
                &scope(),
                &oauth_handle_for(&definition.id)?,
            )
            .await
            .map_err(LlmKeyStoreError::Store)
    }

    /// Only API-key protocols resolve raw key material while preparing a
    /// selection. A native Codex request obtains its exact token later through
    /// the existing Auth owner and its immutable per-token dispatch guard.
    #[cfg(feature = "postgres")]
    pub(crate) async fn read_usage_credential_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        definition: &brassclaw_llm::ProviderDefinition,
    ) -> Result<Option<SecretMaterial>, LlmKeyStoreError> {
        if definition.protocol == brassclaw_llm::ProviderProtocol::OpenAiCodex {
            return Ok(None);
        }
        self.read_in_transaction(transaction, &definition.id).await
    }

    /// Read authenticated version metadata without consuming a lease, locking
    /// rows or migrating legacy records in an administration GET snapshot.
    #[cfg(feature = "postgres")]
    pub(crate) async fn inspect_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        provider_id: &str,
    ) -> Result<Option<String>, LlmKeyStoreError> {
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "coordinated key storage is unavailable".into(),
            })
        })?;
        store
            .inspect_record_reference_in_transaction(
                transaction,
                &scope(),
                &handle_for(provider_id)?,
            )
            .await
            .map_err(LlmKeyStoreError::Store)
    }

    /// Opaque identity of the actual authenticated encrypted provider key row.
    #[cfg(feature = "postgres")]
    pub(crate) async fn record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        provider_id: &str,
    ) -> Result<Option<String>, LlmKeyStoreError> {
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "coordinated key storage is unavailable".into(),
            })
        })?;
        store
            .authenticated_record_reference_in_transaction(
                transaction,
                &scope(),
                &handle_for(provider_id)?,
            )
            .await
            .map_err(LlmKeyStoreError::Store)
    }

    /// Trusted login publication compares the original credential under the
    /// same encrypted-store lock as its write; no file or live token is mutated.
    #[cfg(feature = "postgres")]
    pub(crate) async fn put_if_record_reference_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        provider_id: &str,
        material: SecretMaterial,
        expected: Option<&str>,
    ) -> Result<bool, LlmKeyStoreError> {
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "coordinated key storage is unavailable".into(),
            })
        })?;
        store
            .put_if_record_reference_in_transaction(
                transaction,
                scope(),
                handle_for(provider_id)?,
                material,
                expected,
            )
            .await
            .map(|metadata| metadata.is_some())
            .map_err(LlmKeyStoreError::Store)
    }

    /// Original mutation proof. It is deliberately stored as encrypted secret
    /// material, never a plaintext fingerprint in the provider history.
    #[cfg(feature = "postgres")]
    pub(crate) async fn record_mutation_proof(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        mutation_id: uuid::Uuid,
        proof: SecretMaterial,
    ) -> Result<(), LlmKeyStoreError> {
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "coordinated key storage is unavailable".into(),
            })
        })?;
        store
            .put_in_transaction(transaction, scope(), mutation_handle(mutation_id)?, proof)
            .await
            .map_err(LlmKeyStoreError::Store)?;
        Ok(())
    }

    #[cfg(feature = "postgres")]
    pub(crate) async fn matches_mutation_proof(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        mutation_id: uuid::Uuid,
        proof: &SecretMaterial,
    ) -> Result<bool, LlmKeyStoreError> {
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "coordinated key storage is unavailable".into(),
            })
        })?;
        store
            .matches_in_transaction(transaction, &scope(), &mutation_handle(mutation_id)?, proof)
            .await
            .map_err(LlmKeyStoreError::Store)
    }

    /// Wrap the instance's shared secret store.
    pub fn new(store: Arc<dyn SecretStore>) -> Self {
        Self {
            store,
            #[cfg(feature = "postgres")]
            postgres: None,
        }
    }

    /// Reuse the runtime's exact encrypted store for coordinated setup writes.
    #[cfg(feature = "postgres")]
    pub(crate) fn new_postgres(store: Arc<brassclaw_secrets::PgSecretStore>) -> Self {
        Self {
            store: store.clone(),
            postgres: Some(store),
        }
    }

    #[cfg(feature = "postgres")]
    pub(crate) async fn put_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        provider_id: &str,
        material: SecretMaterial,
    ) -> Result<(), LlmKeyStoreError> {
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "coordinated key storage is unavailable".into(),
            })
        })?;
        store
            .put_in_transaction(transaction, scope(), handle_for(provider_id)?, material)
            .await
            .map_err(LlmKeyStoreError::Store)?;
        Ok(())
    }

    /// Capture one authenticated credential in the same snapshot as its
    /// provider definition. Consumption must commit before an external call.
    #[cfg(feature = "postgres")]
    pub(crate) async fn read_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        provider_id: &str,
    ) -> Result<Option<SecretMaterial>, LlmKeyStoreError> {
        let store = self.postgres.as_ref().ok_or_else(|| {
            LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
                reason: "coordinated key storage is unavailable".into(),
            })
        })?;
        let owner = scope();
        let lease = match store
            .lease_once_in_transaction(transaction, &owner, &handle_for(provider_id)?)
            .await
        {
            Ok(lease) => lease,
            Err(error) if error.is_unknown_secret() => return Ok(None),
            Err(error) => return Err(LlmKeyStoreError::Store(error)),
        };
        store
            .consume_in_transaction(transaction, &owner, lease.id)
            .await
            .map(Some)
            .map_err(LlmKeyStoreError::Store)
    }

    /// Store (or replace) the API-key value for `provider_id`.
    pub async fn put(
        &self,
        provider_id: &str,
        value: SecretMaterial,
    ) -> Result<(), LlmKeyStoreError> {
        let handle = handle_for(provider_id)?;
        self.store
            .put(scope(), handle, value)
            .await
            .map_err(LlmKeyStoreError::Store)?;
        Ok(())
    }

    /// Whether a stored key exists for `provider_id` (without revealing it).
    pub async fn exists(&self, provider_id: &str) -> Result<bool, LlmKeyStoreError> {
        let handle = handle_for(provider_id)?;
        Ok(self
            .store
            .metadata(&scope(), &handle)
            .await
            .map_err(LlmKeyStoreError::Store)?
            .is_some())
    }

    /// Read back the stored key value for `provider_id`, if any.
    ///
    /// Returns `Ok(None)` when no key is stored. Uses a one-shot lease +
    /// consume; the underlying secret persists, so this is repeatable across
    /// reloads.
    pub async fn read(
        &self,
        provider_id: &str,
    ) -> Result<Option<SecretMaterial>, LlmKeyStoreError> {
        let handle = handle_for(provider_id)?;
        let scope = scope();
        let lease = match self.store.lease_once(&scope, &handle).await {
            Ok(lease) => lease,
            Err(error) if error.is_unknown_secret() => return Ok(None),
            Err(error) => return Err(LlmKeyStoreError::Store(error)),
        };
        let material = self
            .store
            .consume(&scope, lease.id)
            .await
            .map_err(LlmKeyStoreError::Store)?;
        Ok(Some(material))
    }

    /// Delete the stored key for `provider_id`. Returns whether one existed.
    pub async fn delete(&self, provider_id: &str) -> Result<bool, LlmKeyStoreError> {
        let handle = handle_for(provider_id)?;
        self.store
            .delete(&scope(), &handle)
            .await
            .map_err(LlmKeyStoreError::Store)
    }
}

#[cfg(feature = "postgres")]
fn mutation_handle(mutation_id: uuid::Uuid) -> Result<SecretHandle, LlmKeyStoreError> {
    SecretHandle::new(format!("llm_mutation_{mutation_id}_proof")).map_err(|_| {
        LlmKeyStoreError::Store(SecretStoreError::BackendMisconfigured {
            reason: "provider mutation proof identity is invalid".into(),
        })
    })
}

fn scope() -> ResourceScope {
    ResourceScope::system()
}

fn handle_for(provider_id: &str) -> Result<SecretHandle, LlmKeyStoreError> {
    provider_handle(provider_id, "api_key")
}

#[cfg(feature = "postgres")]
pub(crate) fn oauth_handle_for(provider_id: &str) -> Result<SecretHandle, LlmKeyStoreError> {
    provider_handle(provider_id, "oauth")
}

fn provider_handle(provider_id: &str, purpose: &str) -> Result<SecretHandle, LlmKeyStoreError> {
    SecretHandle::new(format!("llm_provider_{provider_id}_{purpose}")).map_err(|source| {
        LlmKeyStoreError::InvalidProviderId {
            provider_id: provider_id.to_string(),
            reason: source.to_string(),
        }
    })
}

/// Errors surfaced when storing or reading LLM key values.
#[derive(Debug, Error)]
pub enum LlmKeyStoreError {
    #[error("invalid provider id `{provider_id}` for secret handle: {reason}")]
    InvalidProviderId { provider_id: String, reason: String },
    #[error("secret store error: {0}")]
    Store(#[source] SecretStoreError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_secrets::InMemorySecretStore;

    fn store() -> LlmKeyStore {
        LlmKeyStore::new(Arc::new(InMemorySecretStore::new()))
    }

    #[tokio::test]
    async fn put_then_read_round_trips() {
        let keys = store();
        assert!(!keys.exists("acme").await.expect("exists"));
        assert!(keys.read("acme").await.expect("read").is_none());

        keys.put("acme", SecretMaterial::from("sk-test-value"))
            .await
            .expect("put");

        assert!(keys.exists("acme").await.expect("exists"));
        let value = keys.read("acme").await.expect("read").expect("some");
        assert_eq!(
            secrecy::ExposeSecret::expose_secret(&value),
            "sk-test-value"
        );
    }

    #[tokio::test]
    async fn read_is_repeatable_across_reloads() {
        let keys = store();
        keys.put("acme", SecretMaterial::from("sk-test-value"))
            .await
            .expect("put");
        // Two reads in a row must both succeed (lease+consume must not destroy
        // the underlying secret).
        assert!(keys.read("acme").await.expect("read 1").is_some());
        assert!(keys.read("acme").await.expect("read 2").is_some());
    }

    #[tokio::test]
    async fn delete_removes_key() {
        let keys = store();
        keys.put("acme", SecretMaterial::from("v"))
            .await
            .expect("put");
        assert!(keys.delete("acme").await.expect("delete"));
        assert!(!keys.exists("acme").await.expect("exists"));
        assert!(!keys.delete("acme").await.expect("delete again"));
    }

    #[tokio::test]
    async fn unknown_provider_id_is_rejected() {
        let keys = store();
        let err = keys
            .put("bad id!", SecretMaterial::from("v"))
            .await
            .expect_err("must reject");
        assert!(matches!(err, LlmKeyStoreError::InvalidProviderId { .. }));
    }
}
