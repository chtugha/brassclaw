//! Original task dependency carried through the existing native service seam.
//! Cached native backends retain only weak task/client references, so completed
//! task families can release private credentials, SDKs and embedding caches.
use std::sync::{Arc, OnceLock, Weak};

use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_host_runtime::RuntimeMemoryEmbeddingPort;
use brassclaw_memory::{EmbeddingError, EmbeddingProvider, EmbeddingSpaceId};
use brassclaw_reborn::monty_task_host::MontyTaskHost;

fn unavailable() -> EmbeddingError {
    EmbeddingError::ProviderUnavailable {
        reason: "original task embedding dependency is unavailable".into(),
    }
}

pub(crate) fn bind(
    host: &Arc<MontyTaskHost>,
) -> Result<(), brassclaw_turns::run_profile::AgentLoopHostError> {
    host.bind_memory_embeddings(Arc::new(TaskMemoryEmbeddings {
        host: Arc::downgrade(host),
        provider: OnceLock::new(),
    }))
}

struct TaskMemoryEmbeddings {
    host: Weak<MontyTaskHost>,
    provider: OnceLock<Arc<dyn EmbeddingProvider>>,
}

#[async_trait]
impl RuntimeMemoryEmbeddingPort for TaskMemoryEmbeddings {
    async fn snapshot(
        &self,
        scope: &ResourceScope,
    ) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError> {
        let host = self.host.upgrade().ok_or_else(unavailable)?;
        let context = host.run_context();
        if scope.tenant_id != context.scope.tenant_id
            || context
                .scope
                .explicit_owner_user_id()
                .or_else(|| context.actor.as_ref().map(|actor| &actor.user_id))
                != Some(&scope.user_id)
            || scope.agent_id != context.scope.agent_id
            || scope.project_id != context.scope.project_id
            || scope.thread_id.as_ref() != Some(&context.thread_id)
        {
            return Err(unavailable());
        }
        // Recheck the live guard even when the wrapper/backend is cached. This
        // uses the same captured family selection, never the latest role.
        let Some(provider) = crate::task_model_provider::task_embedding_provider(&host).await?
        else {
            return Ok(None);
        };
        Ok(Some(
            self.provider
                .get_or_init(|| {
                    Arc::new(TaskEmbeddingProvider {
                        host: self.host.clone(),
                        provider: Arc::downgrade(&provider),
                        space: provider.space_id().clone(),
                        dimension: provider.dimension(),
                        model: provider.model_name().to_owned(),
                    })
                })
                .clone(),
        ))
    }
}

struct TaskEmbeddingProvider {
    host: Weak<MontyTaskHost>,
    provider: Weak<dyn EmbeddingProvider>,
    space: EmbeddingSpaceId,
    dimension: usize,
    model: String,
}

impl TaskEmbeddingProvider {
    fn active(&self) -> Result<(Arc<MontyTaskHost>, Arc<dyn EmbeddingProvider>), EmbeddingError> {
        let host = self.host.upgrade().ok_or_else(unavailable)?;
        host.check_dispatch_open().map_err(|_| unavailable())?;
        let provider = self.provider.upgrade().ok_or_else(unavailable)?;
        Ok((host, provider))
    }
}

#[async_trait]
impl EmbeddingProvider for TaskEmbeddingProvider {
    fn space_id(&self) -> &EmbeddingSpaceId {
        &self.space
    }
    fn dimension(&self) -> usize {
        self.dimension
    }
    fn model_name(&self) -> &str {
        &self.model
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let (host, provider) = self.active()?;
        let result = provider.embed(text).await;
        host.check_dispatch_open().map_err(|_| unavailable())?;
        result
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let (host, provider) = self.active()?;
        let result = provider.embed_batch(texts).await;
        host.check_dispatch_open().map_err(|_| unavailable())?;
        result
    }
}

#[cfg(all(test, feature = "skills-db"))]
#[path = "task_memory_embeddings_tests.rs"]
mod tests;
