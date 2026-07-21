//! Interceptor configuration port for the WebChat v2 settings surface.
//!
//! Exposes the Sempai–Kohai interceptor's configuration and control surface to
//! the WebUI settings tab.  The concrete implementation lives in
//! `brassclaw_reborn_composition::interceptor_config_service`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::WebUiAuthenticatedCaller;

/// Snapshot of the current interceptor configuration — sent to the WebUI
/// settings tab on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterceptorConfigSnapshot {
    /// Whether a Sempai provider is currently connected (mode == Rerouting).
    pub sempai_connected: bool,
    /// Current interceptor mode string: `"routing"` or `"rerouting"`.
    pub mode: String,
    /// ISO-8601 timestamp when the base prompt was last assembled, or `None`
    /// if it has never been assembled.
    pub base_prompt_assembled_at: Option<String>,
    /// Number of characters in the current assembled base prompt (Part A).
    /// `None` when no base prompt has been assembled yet.
    pub base_prompt_size_chars: Option<usize>,
    /// Current Sempai persona text (Part B).
    pub persona: String,
    /// ISO-8601 timestamp when `POST /api/interceptor/prewarm` last succeeded.
    /// `None` if never pre-warmed.
    pub prewarm_last_at: Option<String>,
    /// Number of components that have been validated since the base prompt was
    /// last assembled.  A non-zero value is a passive nudge to re-assemble.
    /// `None` when the count is unavailable.
    pub components_since_rebuild: Option<u32>,
}

/// Request body for `POST /api/interceptor/config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInterceptorConfigRequest {
    /// New Sempai persona text (Part B).  `None` means no change.
    pub persona: Option<String>,
}

/// Interceptor configuration + control service (product-layer port).
#[async_trait]
pub trait InterceptorConfigService: Send + Sync {
    /// Return the current interceptor configuration snapshot.
    async fn snapshot(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;

    /// Update editable configuration fields (persona).
    async fn update(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: UpdateInterceptorConfigRequest,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;

    /// Reassemble the static base prompt (Part A) from validated components
    /// via direct SQL to individual component tables (Q20).  Synchronous with
    /// a 120-second server-side timeout; returns the updated snapshot.
    ///
    /// Rate-limited to 1 request per minute per caller.
    async fn reassemble_base_prompt(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;

    /// Send the current base prompt to the Sempai provider as a single system
    /// message to warm its KV cache.  Returns the updated snapshot.
    ///
    /// Rate-limited to 1 request per minute per caller.
    async fn prewarm(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<InterceptorConfigSnapshot, InterceptorConfigServiceError>;
}

/// Errors returned by [`InterceptorConfigService`] methods.
#[derive(Debug, thiserror::Error)]
pub enum InterceptorConfigServiceError {
    #[error("interceptor config service is unavailable")]
    Unavailable,
    #[error("interceptor: {reason}")]
    InvalidRequest { reason: String },
    #[error("interceptor: rate limit exceeded — retry after {retry_after_seconds}s")]
    RateLimitExceeded { retry_after_seconds: u64 },
    #[error("interceptor: base prompt has not been assembled yet")]
    BasePromptNotAssembled,
}

/// Returns a stable [`RebornServicesError`] for an unavailable interceptor
/// config service.  Callers use this in the `Option::ok_or_else` path.
pub fn interceptor_config_unavailable() -> crate::RebornServicesError {
    crate::RebornServicesError::from_status(
        crate::RebornServicesErrorCode::Unavailable,
        503,
        false,
    )
}

/// Map an [`InterceptorConfigServiceError`] to a [`RebornServicesError`].
pub fn map_interceptor_config_error(
    error: InterceptorConfigServiceError,
) -> crate::RebornServicesError {
    match error {
        InterceptorConfigServiceError::Unavailable => crate::RebornServicesError::from_status(
            crate::RebornServicesErrorCode::Unavailable,
            503,
            false,
        ),
        InterceptorConfigServiceError::InvalidRequest { .. }
        | InterceptorConfigServiceError::BasePromptNotAssembled => {
            crate::RebornServicesError::from_status(
                crate::RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            )
        }
        InterceptorConfigServiceError::RateLimitExceeded { .. } => {
            crate::RebornServicesError::from_status(
                crate::RebornServicesErrorCode::RateLimited,
                429,
                true,
            )
        }
    }
}
