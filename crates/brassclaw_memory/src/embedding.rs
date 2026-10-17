//! Embedding provider trait and embedding-vector helpers.

use async_trait::async_trait;
use brassclaw_filesystem::{FilesystemError, FilesystemOperation};
use brassclaw_host_api::VirtualPath;

use crate::path::{MemoryDocumentScope, memory_error, valid_memory_path};

/// Exact host-selected embedding space, independent of vector dimensionality.
/// This identifies data compatibility; it grants no filesystem authority.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmbeddingSpaceId(String);

impl EmbeddingSpaceId {
    pub fn from_digest(digest: [u8; 32]) -> Self {
        let id = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn index_key(&self) -> brassclaw_filesystem::IndexKey {
        brassclaw_filesystem::IndexKey::new(format!("embedding_{}", self.0))
            .unwrap_or_else(|_| unreachable!("a fixed hexadecimal digest is a valid index key"))
    }
}

/// Error returned by memory embedding providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingError {
    ProviderUnavailable { reason: String },
    InvalidVector { expected: usize, actual: usize },
    TextTooLong { length: usize, max: usize },
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingError::ProviderUnavailable { reason } => {
                write!(formatter, "embedding provider unavailable: {reason}")
            }
            EmbeddingError::InvalidVector { expected, actual } => {
                write!(
                    formatter,
                    "embedding vector dimension mismatch: expected {expected}, got {actual}"
                )
            }
            EmbeddingError::TextTooLong { length, max } => {
                write!(formatter, "embedding input too long: {length} > {max}")
            }
        }
    }
}

impl std::error::Error for EmbeddingError {}

/// Memory-owned embedding-provider seam.
///
/// Concrete HTTP/provider integrations belong outside this core crate and can
/// implement this trait after resolving credentials/network policy at the host
/// boundary.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn space_id(&self) -> &EmbeddingSpaceId;
    fn dimension(&self) -> usize;

    fn model_name(&self) -> &str;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.embed(text).await?);
        }
        Ok(embeddings)
    }
}

/// Host-selected provider snapshot for one memory operation. Only absence of
/// configuration returns None; resolution failures must remain errors.
#[async_trait]
pub trait EmbeddingProviderSource: Send + Sync {
    async fn snapshot(
        &self,
    ) -> Result<Option<std::sync::Arc<dyn EmbeddingProvider>>, EmbeddingError>;
}

/// Installation attaches the source once before ingress. The source owns live
/// configuration; the slot never holds its lock during database/network awaits.
#[derive(Default)]
pub struct EmbeddingProviderSourceSlot {
    source: std::sync::RwLock<Option<std::sync::Arc<dyn EmbeddingProviderSource>>>,
}

impl EmbeddingProviderSourceSlot {
    pub fn attach(
        &self,
        source: std::sync::Arc<dyn EmbeddingProviderSource>,
    ) -> Result<(), EmbeddingError> {
        let mut installed =
            self.source
                .write()
                .map_err(|_| EmbeddingError::ProviderUnavailable {
                    reason: "embedding source state unavailable".into(),
                })?;
        if installed.is_some() {
            return Err(EmbeddingError::ProviderUnavailable {
                reason: "embedding source already attached".into(),
            });
        }
        *installed = Some(source);
        Ok(())
    }
}

#[async_trait]
impl EmbeddingProviderSource for EmbeddingProviderSourceSlot {
    async fn snapshot(
        &self,
    ) -> Result<Option<std::sync::Arc<dyn EmbeddingProvider>>, EmbeddingError> {
        let source = self
            .source
            .read()
            .map_err(|_| EmbeddingError::ProviderUnavailable {
                reason: "embedding source state unavailable".into(),
            })?
            .clone();
        match source {
            Some(source) => source.snapshot().await,
            None => Err(EmbeddingError::ProviderUnavailable {
                reason: "embedding source not attached before dispatch".into(),
            }),
        }
    }
}

pub(crate) fn validate_embedding_dimension(
    expected: usize,
    actual: usize,
) -> Result<(), EmbeddingError> {
    if expected == 0 || actual != expected {
        Err(EmbeddingError::InvalidVector { expected, actual })
    } else {
        Ok(())
    }
}

pub(crate) fn embedding_filesystem_error(
    path: VirtualPath,
    operation: FilesystemOperation,
    error: EmbeddingError,
) -> FilesystemError {
    let reason = match error {
        EmbeddingError::ProviderUnavailable { .. } => "embedding provider unavailable".to_string(),
        EmbeddingError::InvalidVector { expected, actual } => {
            format!("embedding vector dimension mismatch: expected {expected}, got {actual}")
        }
        EmbeddingError::TextTooLong { length, max } => {
            format!("embedding input too long: {length} > {max}")
        }
    };
    memory_error(path, operation, reason)
}

pub(crate) async fn embed_text(
    provider: &dyn EmbeddingProvider,
    scope: &MemoryDocumentScope,
    text: &str,
) -> Result<Vec<f32>, FilesystemError> {
    let embedding = provider.embed(text).await.map_err(|error| {
        embedding_filesystem_error(
            scope
                .virtual_prefix()
                .unwrap_or_else(|_| valid_memory_path()),
            FilesystemOperation::ReadFile,
            error,
        )
    })?;
    validate_embedding_dimension(provider.dimension(), embedding.len()).map_err(|error| {
        embedding_filesystem_error(
            scope
                .virtual_prefix()
                .unwrap_or_else(|_| valid_memory_path()),
            FilesystemOperation::ReadFile,
            error,
        )
    })?;
    Ok(embedding)
}
