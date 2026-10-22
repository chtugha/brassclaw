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
/// Registry metadata is instance-wide; existing base bundle details remain
/// compatibility projections. Registration does not qualify a compiler or cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixEntry {
    /// Registry metadata, not executable authority or model-design qualification.
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub profile_revision: u64,
    #[serde(default)]
    pub generation_available: bool,
    #[serde(default)]
    pub generation_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
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
    /// Wall-clock milliseconds the last assembly took (V083).
    /// `None` for entries written before V083 was applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_ms: Option<i64>,
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
    /// Wall-clock milliseconds this assembly took (V083).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_ms: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturePrefixModelRequest {
    pub provider_id: String,
    pub expected_provider_revision: u64,
    pub model: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixModelTarget {
    pub provider_id: String,
    pub provider_revision: u64,
    pub model: String,
    pub consumer: String,
    pub configured_context_window: Option<u32>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixModelSpecification {
    pub id: String,
    pub provider_id: String,
    pub provider_revision: u64,
    pub model: String,
    pub specification_bytes: String,
    pub qualification: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixDesignProposal {
    pub id: String,
    pub profile: String,
    pub specification: String,
    pub proposal_bytes: String,
    pub qualification: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixModelArtifactFile {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixModelArtifactSet {
    pub id: String,
    pub specification: String,
    pub files: Vec<PrefixModelArtifactFile>,
    pub binding_bytes: String,
    pub qualification: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixModelArtifactDetail {
    pub summary: PrefixModelArtifactSet,
    pub artifact_set_bytes: String,
    /// Read-only exact-config projection, never effective serving/cache evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_observation: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainPrefixModelArtifactsRequest {
    pub specification: String,
    pub artifact_set_bytes: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixModelDesignCatalogue {
    pub targets: Vec<PrefixModelTarget>,
    pub specifications: Vec<PrefixModelSpecification>,
    pub proposals: Vec<PrefixDesignProposal>,
    #[serde(default)]
    pub artifact_sets: Vec<PrefixModelArtifactSet>,
    pub design_generation_available: bool,
    pub blocked_reason: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainPrefixDesignProposalRequest {
    pub specification: String,
    pub proposal_bytes: String,
}

/// Offline evaluation of exact retained inputs. These explicit probe/limit
/// assumptions are not verified provider capabilities or serving acceptance.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurePrefixRequest {
    pub expected_profile_revision: u64,
    pub generation: String,
    pub proposal: String,
    pub artifact_set: String,
    pub probes_json: String,
    pub context_window_tokens: u64,
    pub reserved_additional_input_tokens: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterPrefixSourceRequest {
    pub reference: String,
    pub title: String,
    pub applicability: String,
    pub required: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixSourceRegistration {
    pub id: String,
    pub revision: u64,
    pub kind: String,
    pub reference: String,
    pub title: String,
    pub applicability: String,
    pub required: bool,
    pub review_status: String,
    pub availability: String,
    pub checksum: String,
    pub registration_origin: Option<String>,
    pub registered_by: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixUsedSource {
    pub reference: String,
    pub checksum: String,
    pub disposition: String,
    pub reason: String,
    pub archived: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixSourceInventory {
    pub profile: String,
    pub generation: Option<String>,
    pub used: Vec<PrefixUsedSource>,
    pub registered: Vec<PrefixSourceRegistration>,
    pub checks_available: bool,
    pub check_blocked_reason: Option<String>,
}

/// Immutable retention is distinct from qualification and serving selection.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportPrefixGenerationRequest {
    pub expected_profile_revision: u64,
    /// Exact self-contained compiler package bytes, never a filesystem path.
    pub generation_bytes: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixGenerationSummary {
    pub profile: String,
    pub generation: String,
    pub profile_revision: u64,
    pub kind: String,
    pub qualification: String,
    pub created_at: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixGenerationDetail {
    pub summary: PrefixGenerationSummary,
    pub generation_bytes: String,
}

/// Durable job projection. Counts describe recorded native operations, not
/// effect reconciliation or permission to retry. Source/compiler bodies stay
/// in their immutable owner rather than being copied into every status poll.
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixJobDetail {
    pub job_id: String,
    /// Opaque conversation reference for the existing run/event/cancel APIs.
    pub thread_id: String,
    pub profile: String,
    pub profile_revision: u64,
    pub operation: String,
    pub mode: String,
    pub created_at: String,
    pub request_sha256: String,
    pub program_sha256: String,
    pub input_sha256: String,
    pub run_status: Option<brassclaw_turns::TurnStatus>,
    pub admission_phase: Option<String>,
    pub answered_tool_calls: u64,
    pub pending_tool_calls: u64,
    pub failure_kind: Option<String>,
    pub internal_receipt: Option<String>,
    pub result_ref: Option<String>,
}

/// Interceptor configuration + control service (product-layer port).
#[async_trait]
pub trait InterceptorConfigService: Send + Sync {
    async fn measure(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
        _job: &str,
        _request: MeasurePrefixRequest,
    ) -> Result<PrefixJobDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn get_job(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
        _job: &str,
    ) -> Result<PrefixJobDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn model_design_catalogue(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _targets: Vec<PrefixModelTarget>,
    ) -> Result<PrefixModelDesignCatalogue, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn capture_model(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: CapturePrefixModelRequest,
        _targets: Vec<PrefixModelTarget>,
    ) -> Result<PrefixModelSpecification, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn get_model_artifacts(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _specification: &str,
        _artifact_set: &str,
    ) -> Result<PrefixModelArtifactDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn retain_model_artifacts(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: RetainPrefixModelArtifactsRequest,
    ) -> Result<PrefixModelArtifactSet, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn retain_design_proposal(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
        _request: RetainPrefixDesignProposalRequest,
    ) -> Result<PrefixDesignProposal, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }

    async fn list_sources(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
        _generation: Option<&str>,
    ) -> Result<PrefixSourceInventory, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn register_source(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
        _request: RegisterPrefixSourceRequest,
    ) -> Result<PrefixSourceRegistration, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }

    async fn list_generations(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
    ) -> Result<Vec<PrefixGenerationSummary>, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn get_generation(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
        _generation: &str,
    ) -> Result<PrefixGenerationDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn import_generation(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _profile: &str,
        _request: ImportPrefixGenerationRequest,
    ) -> Result<PrefixGenerationDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }

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
    /// Returns every registered profile, including profiles awaiting compiler
    /// qualification. Bundle compatibility data still belongs to the caller.
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
    /// `name` is not registered. Registered profiles without the qualified
    /// generation path return Unavailable rather than pretending to build.
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
    #[error("prefix revision conflict")]
    Conflict,
    #[error("prefix job not found")]
    JobNotFound,
    #[error("prefix generation not found")]
    GenerationNotFound,
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
        InterceptorConfigServiceError::Conflict => crate::RebornServicesError::from_status(
            crate::RebornServicesErrorCode::InvalidRequest,
            409,
            false,
        ),
        InterceptorConfigServiceError::JobNotFound
        | InterceptorConfigServiceError::GenerationNotFound => {
            crate::RebornServicesError::from_status(
                crate::RebornServicesErrorCode::NotFound,
                404,
                false,
            )
        }
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
