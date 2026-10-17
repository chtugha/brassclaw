//! Live DB-selected embedding infrastructure, not a workflow executor.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_memory::{EmbeddingError, EmbeddingProvider, EmbeddingProviderSource};
use secrecy::ExposeSecret as _;

struct CachedSelection {
    descriptor: String,
    credential: Option<secrecy::SecretString>,
    provider: Arc<dyn EmbeddingProvider>,
    cache: Arc<brassclaw_embeddings::CachedEmbeddingProvider>,
}

pub(crate) struct PgMemoryEmbeddingSource {
    admin: crate::RebornProviderAdmin,
    namespace: String,
    keys: crate::LlmKeyStore,
    session: Arc<brassclaw_llm::SessionManager>,
    cached: tokio::sync::Mutex<Option<CachedSelection>>,
    resolution: tokio::sync::Mutex<()>,
}

fn unavailable() -> EmbeddingError {
    EmbeddingError::ProviderUnavailable {
        reason: "configured memory embedding provider could not be resolved".into(),
    }
}

impl PgMemoryEmbeddingSource {
    pub(crate) fn new(
        pool: brassclaw_pg::PgPool,
        namespace: String,
        keys: crate::LlmKeyStore,
        session: Arc<brassclaw_llm::SessionManager>,
    ) -> Self {
        Self {
            admin: crate::RebornProviderAdmin::new(pool, namespace.clone()),
            namespace,
            keys,
            session,
            cached: tokio::sync::Mutex::new(None),
            resolution: tokio::sync::Mutex::new(()),
        }
    }
}

fn matches(
    old: &CachedSelection,
    descriptor: &str,
    credential: Option<&secrecy::SecretString>,
) -> bool {
    old.descriptor == descriptor
        && match (old.credential.as_ref(), credential) {
            (None, None) => true,
            (Some(old), Some(new)) => old.expose_secret() == new.expose_secret(),
            _ => false,
        }
}

impl PgMemoryEmbeddingSource {
    async fn resolve_snapshot(&self) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError> {
        let captured = self
            .admin
            .capture_selection(
                crate::provider_admin::ProviderSelectionSource::Role(
                    brassclaw_llm::ProviderRole::Embedding,
                ),
                &self.keys,
            )
            .await
            .map_err(|_| unavailable())?;
        let Some(mut captured) = captured else {
            // Drop retained caches when disabled; in-flight snapshots stay owned
            // by their requests. No failed resolution returns an old provider.
            if let Some(old) = self.cached.lock().await.take() {
                old.cache.set_max_entries(0);
            }
            return Ok(None);
        };
        // Named external credentials are frozen once, just like a DB key. They
        // are private cache inputs, never descriptor bytes or log fields.
        if captured.credential.is_none() {
            captured.credential = captured
                .definition
                .api_key_env
                .as_deref()
                .and_then(|name| std::env::var(name).ok())
                .map(secrecy::SecretString::from);
        }
        let model = captured
            .selection
            .model
            .as_deref()
            .unwrap_or(&captured.definition.default_model);
        let contract = captured
            .definition
            .embedding_models
            .0
            .get(model)
            .ok_or_else(unavailable)?;
        let capacity = contract.cache_max_entries as usize;
        // Only adapter-affecting inputs construct a new client. Cache sizing is
        // a live owner control; cosmetic edits and other model contracts do not
        // discard this model's reusable client and cache.
        let descriptor = serde_json::to_string(&(
            &captured.definition.id,
            captured.definition.protocol,
            &captured.definition.default_base_url,
            &captured.definition.api_key_env,
            captured.definition.api_key_required,
            captured.definition.request_policy,
            model,
            contract.dimension,
            contract.vector_revision,
        ))
        .map_err(|_| unavailable())?;
        {
            let cached = self.cached.lock().await;
            if let Some(old) = cached.as_ref()
                && matches(old, &descriptor, captured.credential.as_ref())
            {
                old.cache.set_max_entries(capacity);
                return Ok(Some(old.provider.clone()));
            }
        }
        let credential = captured.credential.clone();
        let prepared = crate::factory::build_captured_embedding_provider(
            &self.namespace,
            captured,
            self.session.clone(),
        )
        .await
        .map_err(|_| unavailable())?;
        let mut cached = self.cached.lock().await;
        // Retired clients may still complete their requests. Their cache is
        // emptied and fenced so those completions cannot retain old entries.
        if let Some(old) = cached.as_ref() {
            old.cache.set_max_entries(0);
        }
        *cached = Some(CachedSelection {
            descriptor,
            credential,
            provider: prepared.provider.clone(),
            cache: prepared.cache,
        });
        Ok(Some(prepared.provider))
    }
}

#[async_trait]
impl EmbeddingProviderSource for PgMemoryEmbeddingSource {
    async fn snapshot(&self) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError> {
        // This gate covers coherent capture/preparation, never embedding HTTP
        // or task execution. Earlier resolutions cannot publish after new ones.
        let _resolution = self.resolution.lock().await;
        let result = self.resolve_snapshot().await;
        if result.is_err()
            && let Some(old) = self.cached.lock().await.take()
        {
            old.cache.set_max_entries(0);
        }
        result
    }
}
