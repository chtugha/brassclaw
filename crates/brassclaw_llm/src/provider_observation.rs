//! Call-local custody of complete OpenAI Chat Completions HTTP responses before
//! SDK parsing. No observer is shared across tasks, retries or model operations.
use std::{future::Future, sync::Arc};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};

/// Sanitized persistence error; upstream data never enters Debug or Display.
#[derive(Debug, thiserror::Error)]
#[error("provider response evidence unavailable")]
pub struct ProviderObservationError;

#[async_trait]
pub trait ProviderWireObserver: Send + Sync {
    /// Own the original bytes synchronously, before a cancellable write await.
    fn receive_response(&self, status: u16, body: &[u8]) -> Result<(), ProviderObservationError>;
    async fn persist_response(&self) -> Result<(), ProviderObservationError>;
}

tokio::task_local! {
    static OBSERVER: Arc<dyn ProviderWireObserver>;
}

/// Scope the observer to one already selected provider operation. The transport
/// captures its Arc when constructing the HTTP future; dropped callers cannot
/// redirect a late response into another task's evidence.
pub async fn observe_chat_completion<T>(
    observer: Arc<dyn ProviderWireObserver>,
    operation: impl Future<Output = T>,
) -> T {
    OBSERVER.scope(observer, operation).await
}

pub(crate) fn current() -> Option<Arc<dyn ProviderWireObserver>> {
    OBSERVER.try_with(Arc::clone).ok()
}

/// Lossless host-private envelope. Identity comes from the selected host route,
/// never from an upstream response, header or model-produced instruction.
pub fn wire_envelope(
    status: u16,
    body: &[u8],
    provider_id: &str,
    model_id: &str,
) -> serde_json::Value {
    serde_json::json!({"format":"provider-wire-observation/1", "provider_id":provider_id,
        "model_id":model_id,"status":status,"body_encoding":"base64","body":STANDARD.encode(body)})
}
