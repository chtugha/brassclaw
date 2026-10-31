//! Task-selected embedding dependency at the existing native service boundary.
//! The binding supplies neither Tool authority nor a memory workflow.
use std::{future::Future, sync::Arc};

use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_memory::{EmbeddingError, EmbeddingProvider};

/// Resolve this task's original dependency without selecting a live role again.
/// Only captured absence returns None; invalid or fenced selection is an error.
/// The returned provider must preserve live checks at actual embedding use.
#[async_trait]
pub trait RuntimeMemoryEmbeddingPort: Send + Sync {
    async fn snapshot(
        &self,
        scope: &ResourceScope,
    ) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError>;
}

tokio::task_local! {
    static TASK_MEMORY_EMBEDDINGS: Option<Arc<dyn RuntimeMemoryEmbeddingPort>>;
}

/// Trusted host transport around one existing invocation future. The native
/// adapter reads this binding only after ordinary planning/service resolution.
/// It is future-local, never a global run map or serialized Tool argument, and
/// is not inherited by spawned tasks. Both generic and retained Monty dispatch
/// use this same boundary; None clears any enclosing task binding. Other
/// callers retain their explicit service source.
pub async fn with_task_memory_embeddings<T>(
    embeddings: Option<Arc<dyn RuntimeMemoryEmbeddingPort>>,
    invocation: impl Future<Output = T>,
) -> T {
    TASK_MEMORY_EMBEDDINGS.scope(embeddings, invocation).await
}

pub(crate) fn selected() -> Option<Arc<dyn RuntimeMemoryEmbeddingPort>> {
    TASK_MEMORY_EMBEDDINGS.try_with(Clone::clone).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoEmbedding;
    #[async_trait]
    impl RuntimeMemoryEmbeddingPort for NoEmbedding {
        async fn snapshot(
            &self,
            _: &ResourceScope,
        ) -> Result<Option<Arc<dyn EmbeddingProvider>>, EmbeddingError> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn memory_binding_is_future_local_and_clears_nested_spawned_and_dropped_work() {
        let a: Arc<dyn RuntimeMemoryEmbeddingPort> = Arc::new(NoEmbedding);
        let b: Arc<dyn RuntimeMemoryEmbeddingPort> = Arc::new(NoEmbedding);
        let left = with_task_memory_embeddings(Some(a.clone()), async {
            tokio::task::yield_now().await;
            assert!(Arc::ptr_eq(&selected().unwrap(), &a));
            with_task_memory_embeddings(None, async {
                tokio::task::yield_now().await;
                assert!(
                    selected().is_none(),
                    "unbound nested host cannot borrow its caller's selection"
                );
            })
            .await;
            assert!(Arc::ptr_eq(&selected().unwrap(), &a));
            assert!(tokio::spawn(async { selected().is_none() }).await.unwrap());
        });
        let right = with_task_memory_embeddings(Some(b.clone()), async {
            tokio::task::yield_now().await;
            assert!(Arc::ptr_eq(&selected().unwrap(), &b));
        });
        tokio::join!(left, right);
        assert!(selected().is_none());
        let mut dropped = Box::pin(with_task_memory_embeddings(Some(a), async {
            assert!(selected().is_some());
            std::future::pending::<()>().await;
        }));
        assert!(futures_util::poll!(&mut dropped).is_pending());
        drop(dropped);
        assert!(
            selected().is_none(),
            "dropping a host future must not leave ambient task state"
        );
    }
}
