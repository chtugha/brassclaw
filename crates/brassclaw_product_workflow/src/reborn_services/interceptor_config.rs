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
    /// Current Sempai persona text (Part B).
    pub persona: String,
}

/// Request body for `POST /api/interceptor/config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInterceptorConfigRequest {
    /// New Sempai persona text (Part B).  `None` means no change.
    pub persona: Option<String>,
}

/// A single named prefix-cache entry.
///
/// Corresponds to one row in `reborn_basic_prompt_store`.  Phase K.1 only has
/// the `"base-prompt"` entry; additional named entries are additive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixEntry {
    /// Canonical name, e.g. `"base-prompt"`.
    pub name: String,
    /// SHA-256 hex over `(uuid_bytes || updated_at_micros_le)` for all
    /// component rows included in the last assembly.  `None` if no assembly
    /// has run yet.
    pub fingerprint: Option<String>,
    /// `true` when a Q2 graduation has occurred since the last assembly;
    /// a passive nudge to re-run `regenerate_prefix`.
    pub is_stale: bool,
    /// ISO-8601 timestamp of the last successful assembly, or `None`.
    pub assembled_at: Option<String>,
    /// ISO-8601 timestamp of the last pre-warm gateway call, or `None`.
    pub prewarm_last_at: Option<String>,
}

/// Response body for `GET /api/prefixes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixListResponse {
    pub prefixes: Vec<PrefixEntry>,
}

/// Response body for `POST /api/prefixes/{name}/regenerate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixRegenerateResponse {
    /// The name of the regenerated prefix.
    pub name: String,
    /// SHA-256 fingerprint of the assembled component set.
    pub fingerprint: String,
    /// ISO-8601 timestamp of this assembly.
    pub assembled_at: String,
    /// ISO-8601 timestamp of the pre-warm gateway call, or `None` if no
    /// Sempai gateway is configured.
    pub prewarm_last_at: Option<String>,
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

    /// List all named prefix-cache entries for a scope.
    ///
    /// Returns one entry per known prefix name.  Phase K.1 returns only the
    /// `"base-prompt"` entry.
    async fn list_prefix_entries(
        &self,
        caller: WebUiAuthenticatedCaller,
        user_id: &str,
        project_id: &str,
    ) -> Result<PrefixListResponse, InterceptorConfigServiceError>;

    /// Assemble the named prefix bundle from validated components, optionally
    /// pre-warm the Sempai gateway, and record the result.
    ///
    /// Rate-limited to 1 request per minute per caller.
    ///
    /// Errors with [`InterceptorConfigServiceError::PrefixNotFound`] when
    /// `name` is not a known prefix (currently only `"base-prompt"`).
    async fn regenerate_prefix(
        &self,
        caller: WebUiAuthenticatedCaller,
        name: &str,
        user_id: &str,
        project_id: &str,
    ) -> Result<PrefixRegenerateResponse, InterceptorConfigServiceError>;
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
    #[error("interceptor: prefix '{name}' not found")]
    PrefixNotFound { name: String },
}

/// Returns a stable [`RebornServicesError`] for an unavailable interceptor
/// config service.  Callers use this in the `Option::ok_or_else` path.
pub fn interceptor_config_unavailable() -> crate::RebornServicesError {
    crate::RebornServicesError::from_status(crate::RebornServicesErrorCode::Unavailable, 503, false)
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
        InterceptorConfigServiceError::InvalidRequest { .. } => {
            crate::RebornServicesError::from_status(
                crate::RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            )
        }
        InterceptorConfigServiceError::PrefixNotFound { .. } => {
            crate::RebornServicesError::from_status(
                crate::RebornServicesErrorCode::NotFound,
                404,
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
