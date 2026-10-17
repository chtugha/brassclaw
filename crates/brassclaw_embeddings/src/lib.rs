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
