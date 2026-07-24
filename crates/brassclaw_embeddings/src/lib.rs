//! Embedding-provider trait, caching decorator, and utility helpers.
//!
//! This crate is a **pure trait-and-utilities layer** (S10 refactor, revision 17).
//! Concrete provider implementations (`OpenAiEmbeddings`, `NearAiEmbeddings`,
//! `OllamaEmbeddings`, `BedrockEmbeddings`), the `EmbeddingsConfig` data shape,
//! the `create_provider` factory, and `ProviderDeps` have been moved to
//! `brassclaw_reborn_composition::embedding_providers` — they depend on
//! `brassclaw_llm` runtime objects and DB-backed config that belongs in the
//! composition layer, not here.
//!
//! ## Public surface
//!
//! | Symbol | Use |
//! |--------|-----|
//! | `EmbeddingProvider` trait | Trait object for all providers |
//! | `EmbeddingError` | Error type returned by every provider |
//! | `CachedEmbeddingProvider`, `EmbeddingCacheConfig` | LRU caching decorator |
//! | `url_check::check_base_url` | AlwaysBlocked URL floor check |
//! | `default_dimension_for_model` | Model → dimension helper |
//! | `DEFAULT_EMBEDDING_CACHE_SIZE` | Default LRU cap |
//! | `MockEmbeddings` | Deterministic test double (gated: `testing` feature) |

mod cache;
#[cfg(any(test, feature = "testing"))]
mod mock;
mod provider;
pub mod url_check;

pub use cache::{CachedEmbeddingProvider, EmbeddingCacheConfig};
#[cfg(any(test, feature = "testing"))]
pub use mock::MockEmbeddings;
pub use provider::{EmbeddingError, EmbeddingProvider};

/// Default maximum number of cached embeddings.
pub const DEFAULT_EMBEDDING_CACHE_SIZE: usize = 10_000;

// Embedding dimension constants for known models.
/// OpenAI text-embedding-3-small and text-embedding-ada-002 output dimension.
pub const DIM_OPENAI_SMALL: usize = 1536;
/// OpenAI text-embedding-3-large output dimension.
pub const DIM_OPENAI_LARGE: usize = 3072;
/// Amazon Titan text-embedding-v2 and mxbai-embed-large output dimension.
pub const DIM_1024: usize = 1024;
/// Nomic embed-text output dimension.
pub const DIM_NOMIC: usize = 768;
/// all-MiniLM sentence-transformers output dimension.
pub const DIM_MINILM: usize = 384;
/// Fallback dimension for unrecognised models (matches `DIM_OPENAI_SMALL`).
pub const DIM_DEFAULT_FALLBACK: usize = DIM_OPENAI_SMALL;

/// Infer the embedding dimension from a well-known model name.
///
/// Falls back to [`DIM_DEFAULT_FALLBACK`] (1536) for unknown models.
pub fn default_dimension_for_model(model: &str) -> usize {
    match model {
        "text-embedding-3-small" => DIM_OPENAI_SMALL,
        "text-embedding-3-large" => DIM_OPENAI_LARGE,
        "text-embedding-ada-002" => DIM_OPENAI_SMALL,
        "amazon.titan-embed-text-v2:0" => DIM_1024,
        "nomic-embed-text" => DIM_NOMIC,
        "mxbai-embed-large" => DIM_1024,
        "all-minilm" => DIM_MINILM,
        _ => DIM_DEFAULT_FALLBACK,
    }
}
