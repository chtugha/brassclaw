//! Live DB-selected embedding infrastructure, not a workflow executor.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_memory::{EmbeddingError, EmbeddingProvider, EmbeddingProviderSource};

struct CachedSelection {
    descriptor: String,
    credential_ref: Option<String>,
    provider: Arc<dyn EmbeddingProvider>,
    cache: Arc<brassclaw_embeddings::CachedEmbeddingProvider>,
}

struct PublishedSource {
    cached: Option<CachedSelection>,
    publication: brassclaw_llm::ProviderPublication,
    failed: Option<brassclaw_llm::ProviderPublication>,
}

pub(crate) struct PgMemoryEmbeddingSource {
    admin: crate::RebornProviderAdmin,
    namespace: String,
    keys: crate::LlmKeyStore,
    session: Arc<brassclaw_llm::SessionManager>,
    published: tokio::sync::Mutex<PublishedSource>,
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
            published: tokio::sync::Mutex::new(PublishedSource {
                cached: None,
                publication: brassclaw_llm::ProviderPublication::Untracked,
                failed: None,
            }),
            resolution: tokio::sync::Mutex::new(()),
        }
    }

    /// Actual effective/failure metadata under the same publication lock.
    pub(crate) async fn publication_status(&self) -> brassclaw_llm::ProviderPublicationStatus {
        let state = self.published.lock().await;
        brassclaw_llm::ProviderPublicationStatus {
            effective: state.publication.clone(),
            failed: state.failed.clone(),
        }
    }
}

fn matches(old: &CachedSelection, descriptor: &str, credential_ref: Option<&str>) -> bool {
    old.descriptor == descriptor && old.credential_ref.as_deref() == credential_ref
}

impl PgMemoryEmbeddingSource {
    async fn resolve_snapshot(
        &self,
        attempted: &mut Option<brassclaw_llm::ProviderPublication>,
    ) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError> {
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
        let Some(captured) = captured else {
            // Drop retained caches when disabled; in-flight snapshots stay owned
            // by their requests. No failed resolution returns an old provider.
            let mut published = self.published.lock().await;
            if let Some(old) = published.cached.take() {
                old.cache.retire();
            }
            published.publication = brassclaw_llm::ProviderPublication::Unconfigured;
            published.failed = None;
            return Ok(None);
        };
        let publication = brassclaw_llm::ProviderPublication::Configured {
            selection: captured.revision.clone(),
        };
        *attempted = Some(publication.clone());
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
            let mut published = self.published.lock().await;
            if let Some(old) = published.cached.as_ref()
                && matches(old, &descriptor, captured.credential_ref.as_deref())
            {
                old.cache.set_max_entries(capacity);
                let provider = old.provider.clone();
                // Compatible definition/cache changes are applied to the
                // retained client before acknowledging their exact capture.
                published.publication = publication;
                published.failed = None;
                return Ok(Some(provider));
            }
        }
        let credential_ref = captured.credential_ref.clone();
        let guard = crate::provider_dispatch::PgProviderDispatchGuard::from_admin(
            self.admin.clone(),
            self.keys.clone(),
            &captured,
        )
        .map_err(|_| unavailable())?;
        let prepared = crate::factory::build_captured_embedding_provider(
            &self.namespace,
            captured,
            self.session.clone(),
            guard,
        )
        .await
        .map_err(|_| unavailable())?;
        let mut published = self.published.lock().await;
        // Retired clients may still complete their requests. Their cache is
        // emptied and fenced so those completions cannot retain old entries.
        if let Some(old) = published.cached.as_ref() {
            old.cache.retire();
        }
        published.cached = Some(CachedSelection {
            descriptor,
            credential_ref,
            provider: prepared.provider.clone(),
            cache: prepared.cache,
        });
        published.publication = publication;
        published.failed = None;
        Ok(Some(prepared.provider))
    }
}

#[async_trait]
impl EmbeddingProviderSource for PgMemoryEmbeddingSource {
    async fn retained(
        &self,
        configuration_ref: &str,
    ) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError> {
        let state = self.published.lock().await;
        Ok(match &state.publication {
            brassclaw_llm::ProviderPublication::Configured { selection }
                if selection.configuration_ref == configuration_ref =>
            {
                state.cached.as_ref().map(|cached| cached.provider.clone())
            }
            _ => None,
        })
    }

    async fn snapshot(&self) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError> {
        // This gate covers coherent capture/preparation, never embedding HTTP
        // or task execution. Earlier resolutions cannot publish after new ones.
        let _resolution = self.resolution.lock().await;
        let mut attempted = None;
        let result = self.resolve_snapshot(&mut attempted).await;
        if result.is_err() {
            let mut published = self.published.lock().await;
            if let Some(old) = published.cached.take() {
                old.cache.retire();
            }
            published.publication = brassclaw_llm::ProviderPublication::Untracked;
            published.failed = attempted;
        }
        result
    }
}
