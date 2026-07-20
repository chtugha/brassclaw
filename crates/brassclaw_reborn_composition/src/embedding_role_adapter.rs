//! `EmbeddingRoleAdapter` — bridges `brassclaw_embeddings::EmbeddingProvider`
//! to the `brassclaw_memory::EmbeddingProvider` seam (§3, §4.30).
//!
//! The two traits have slightly different shapes:
//!
//! | Trait | Methods |
//! |-------|---------|
//! | `brassclaw_embeddings::EmbeddingProvider` | `dimension`, `model_name`, `max_input_length`, `embed`, `embed_batch` |
//! | `brassclaw_memory::EmbeddingProvider`     | `dimension`, `model_name`, `embed`, `embed_batch` |
//!
//! The adapter implements the memory seam by delegating the four overlapping
//! methods to the inner embeddings provider and discarding `max_input_length`
//! (not present on the memory seam).
//!
//! # Error mapping
//!
//! `brassclaw_embeddings::EmbeddingError` (6 variants) is mapped to
//! `brassclaw_memory::EmbeddingError` (3 variants):
//!
//! | Source | Target |
//! |--------|--------|
//! | `HttpError(s)` | `ProviderUnavailable { reason: s }` |
//! | `InvalidResponse(s)` | `ProviderUnavailable { reason: s }` |
//! | `RateLimited { .. }` | `ProviderUnavailable { reason: "rate limited" }` |
//! | `AuthFailed` | `ProviderUnavailable { reason: "authentication failed" }` |
//! | `TextTooLong { length, max }` | `TextTooLong { length, max }` (pass-through) |
//! | `InvalidUrl { url, reason }` | `ProviderUnavailable { reason: "invalid URL …" }` |

use std::sync::Arc;

use async_trait::async_trait;
#[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
use brassclaw_embeddings::{CachedEmbeddingProvider, EmbeddingCacheConfig};
use brassclaw_memory::EmbeddingError as MemoryEmbeddingError;

fn map_error(e: brassclaw_embeddings::EmbeddingError) -> MemoryEmbeddingError {
    use brassclaw_embeddings::EmbeddingError as E;
    match e {
        E::HttpError(s) => MemoryEmbeddingError::ProviderUnavailable { reason: s },
        E::InvalidResponse(s) => MemoryEmbeddingError::ProviderUnavailable { reason: s },
        E::RateLimited { .. } => MemoryEmbeddingError::ProviderUnavailable {
            reason: "rate limited".into(),
        },
        E::AuthFailed => MemoryEmbeddingError::ProviderUnavailable {
            reason: "authentication failed".into(),
        },
        E::TextTooLong { length, max } => MemoryEmbeddingError::TextTooLong { length, max },
        E::InvalidUrl { url, reason } => MemoryEmbeddingError::ProviderUnavailable {
            reason: format!("invalid URL {url}: {reason}"),
        },
    }
}

/// Adapts a `brassclaw_embeddings::EmbeddingProvider` to the
/// `brassclaw_memory::EmbeddingProvider` seam.
///
/// Wrap the inner provider in a `CachedEmbeddingProvider` before passing it
/// here to avoid redundant HTTP calls for repeated identical texts.
pub(crate) struct EmbeddingRoleAdapter {
    inner: Arc<dyn brassclaw_embeddings::EmbeddingProvider>,
}

impl EmbeddingRoleAdapter {
    /// Wrap an embeddings provider with the default LRU cache configuration.
    #[cfg(all(feature = "postgres", feature = "root-llm-provider"))]
    pub(crate) fn new_cached(
        provider: Arc<dyn brassclaw_embeddings::EmbeddingProvider>,
        cache_config: EmbeddingCacheConfig,
    ) -> Arc<dyn brassclaw_memory::EmbeddingProvider> {
        let cached: Arc<dyn brassclaw_embeddings::EmbeddingProvider> =
            Arc::new(CachedEmbeddingProvider::new(provider, cache_config));
        Arc::new(Self { inner: cached })
    }

    /// Wrap an embeddings provider without a cache (for tests / backfill worker).
    #[allow(dead_code)] // Used by backfill-embeddings CLI (S10 item 8)
    pub(crate) fn new_uncached(
        provider: Arc<dyn brassclaw_embeddings::EmbeddingProvider>,
    ) -> Arc<dyn brassclaw_memory::EmbeddingProvider> {
        Arc::new(Self { inner: provider })
    }
}

#[async_trait]
impl brassclaw_memory::EmbeddingProvider for EmbeddingRoleAdapter {
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>, MemoryEmbeddingError> {
        self.inner.embed(text).await.map_err(map_error)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, MemoryEmbeddingError> {
        self.inner.embed_batch(texts).await.map_err(map_error)
    }
}
