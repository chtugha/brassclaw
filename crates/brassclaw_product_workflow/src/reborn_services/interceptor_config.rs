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
    #[serde(deserialize_with = "crate::instance_tools::revision_wire::deserialize_compatible")]
    pub expected_provider_revision: u64,
    pub model: String,
}
/// Draft counts use the same exact JSON/browser representation as model limits.
/// This is not a configured model budget or a generated Prefix qualification.
pub const PREFIX_SETUP_MAX_EXACT_INTEGER: u64 = (1_u64 << 53) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixModelSetupField {
    MaxPrefixTokens,
    MaxPromptTokens,
    ReservedOutputTokens,
    AdditionalFramingTokens,
}
impl PrefixModelSetupField {
    pub const ALL: [Self; 4] = [
        Self::MaxPrefixTokens,
        Self::MaxPromptTokens,
        Self::ReservedOutputTokens,
        Self::AdditionalFramingTokens,
    ];
    pub fn valid_value(self, value: Option<u64>) -> bool {
        value.is_none_or(|value| {
            value <= PREFIX_SETUP_MAX_EXACT_INTEGER
                && (value > 0
                    || matches!(self, Self::MaxPrefixTokens | Self::AdditionalFramingTokens))
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixModelSetupOrigin {
    Unset,
    ProviderConfiguration,
    Manual,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixModelSetupValue {
    #[serde(deserialize_with = "required_setup_value")]
    pub value: Option<u64>,
    pub origin: PrefixModelSetupOrigin,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixModelSetupDraft {
    pub specification: String,
    /// Zero is the untouched initial view, not a stored/approved revision.
    pub revision: u64,
    /// Present only for a stored manual operation; the initial view has null.
    #[serde(deserialize_with = "required_setup_value")]
    pub operation_id: Option<uuid::Uuid>,
    pub fields: std::collections::BTreeMap<PrefixModelSetupField, PrefixModelSetupValue>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixModelSetupEdit {
    pub field: PrefixModelSetupField,
    /// Explicit null is an operator clear. Missing is invalid, not a default.
    #[serde(deserialize_with = "required_setup_value")]
    pub value: Option<u64>,
}
fn required_setup_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePrefixModelSetupDraftRequest {
    /// Caller retains this original intent across retries; never generated by
    /// the writer. It identifies a data edit, not a task or permission grant.
    pub operation_id: uuid::Uuid,
    pub expected_revision: u64,
    pub edits: Vec<PrefixModelSetupEdit>,
}
impl UpdatePrefixModelSetupDraftRequest {
    pub fn valid(&self) -> bool {
        !self.operation_id.is_nil()
            && self.expected_revision < PREFIX_SETUP_MAX_EXACT_INTEGER
            && !self.edits.is_empty()
            && self
                .edits
                .iter()
                .all(|edit| edit.field.valid_value(edit.value))
            && self
                .edits
                .iter()
                .map(|edit| edit.field)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.edits.len()
    }
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
#[serde(deny_unknown_fields)]
pub struct PrefixAcceptanceSelection {
    pub profile: String,
    pub generation: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixAcceptanceDecision {
    pub selection: PrefixAcceptanceSelection,
    pub acceptance_id: uuid::Uuid,
    pub review_checksum: String,
    pub semantic_review: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixAcceptanceView {
    pub review_checksum: String,
    pub review: serde_json::Value,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixAcceptanceReceipt {
    pub generation_qualification: uuid::Uuid,
    pub serving_activated: bool,
}
/// An exact retained proposal reference, never a role or latest-design lookup.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixPairAcceptanceReference {
    pub specification: String,
    pub pair_proposal: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixPairAcceptanceSelection {
    pub pair: PrefixPairAcceptanceReference,
    /// Real quarantined evaluation, not the production generation of each profile.
    pub evaluation: PrefixAcceptanceSelection,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixPairAcceptanceDecision {
    pub selection: PrefixPairAcceptanceSelection,
    pub review_checksum: String,
    pub semantic_review: String,
}
/// Existing native review is not the future complete design-workflow seal.
/// A legacy Pair original must remain readable without being upgraded by labels.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixPairAcceptanceEvidence {
    LegacyNativeReview,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixPairAcceptanceReceipt {
    pub pair_acceptance: uuid::Uuid,
    pub evidence_checksum: String,
    pub selection: PrefixPairAcceptanceSelection,
    pub review: PrefixAcceptanceView,
    pub actor: String,
    pub semantic_review: String,
    pub evidence: PrefixPairAcceptanceEvidence,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixPairAcceptanceView {
    pub selection: PrefixPairAcceptanceSelection,
    pub review: PrefixAcceptanceView,
    /// No public release method exists until the real qualified producer is joined.
    pub release_blocked_reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivatePrefixAcceptanceRequest {
    pub generation_qualification: uuid::Uuid,
    pub expected_revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixTargetBindingReceipt {
    pub generation_qualification: uuid::Uuid,
    pub revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixDesignProposal {
    pub id: String,
    pub profile: String,
    pub specification: String,
    pub proposal_bytes: String,
    pub qualification: String,
}
/// Canonical complete case data from one unaccepted retained pair proposal.
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixPairCasePacket {
    pub proposal: String,
    pub specification: String,
    pub artifact_set: String,
    pub case_packet_bytes: String,
    pub qualification: String,
}

/// Role-independent unaccepted construction data, not an accepted formatter.
#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixPairConstructionProposal {
    pub id: String,
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
    /// Exact retained tokenizer/template source declarations; never a serving verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_observation: Option<String>,
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
    pub pair_proposals: Vec<PrefixPairConstructionProposal>,
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
    #[serde(deserialize_with = "crate::instance_tools::revision_wire::deserialize_compatible")]
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
    #[serde(deserialize_with = "crate::instance_tools::revision_wire::deserialize_compatible")]
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

/// Exact immutable target; Pair jobs have no synthetic profile or role revision.
/// Flattening preserves the existing profile status JSON fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target_kind", rename_all = "snake_case")]
pub enum PrefixJobTarget {
    Profile {
        profile: String,
        profile_revision: u64,
    },
    Pair {
        specification: String,
        proposal: String,
    },
    ModelSetup {
        specification: String,
        setup_revision: u64,
    },
}

/// Durable job projection. Counts describe recorded operations, not effect
/// reconciliation or permission to retry. Source/compiler bodies remain in
/// their immutable owner instead of being copied into each status poll.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "format")]
pub enum PrefixJobDetail {
    #[serde(rename = "prefix-job-detail/1")]
    Chat {
        thread_id: String,
        run_status: Option<brassclaw_turns::TurnStatus>,
        #[serde(flatten)]
        metadata: PrefixJobMetadata,
    },
    #[serde(rename = "prefix-job-detail/2")]
    PrivateWork {
        work_id: String,
        work_status: Option<brassclaw_turns::private_work::PrivateWorkStatus>,
        #[serde(flatten)]
        metadata: PrefixJobMetadata,
    },
}
impl std::ops::Deref for PrefixJobDetail {
    type Target = PrefixJobMetadata;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Chat { metadata, .. } | Self::PrivateWork { metadata, .. } => metadata,
        }
    }
}

impl std::ops::DerefMut for PrefixJobDetail {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Chat { metadata, .. } | Self::PrivateWork { metadata, .. } => metadata,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PrefixJobMetadata {
    pub job_id: String,
    #[serde(flatten)]
    pub target: PrefixJobTarget,
    pub operation: String,
    pub mode: String,
    pub created_at: String,
    pub request_sha256: String,
    pub program_sha256: String,
    pub input_sha256: String,
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
    async fn check_pair_cases(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _specification: &str,
        _proposal: &str,
        _job: &str,
    ) -> Result<PrefixJobDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn get_model_setup_job(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _specification: &str,
        _setup_revision: u64,
        _job: &str,
    ) -> Result<PrefixJobDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn get_pair_job(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _specification: &str,
        _proposal: &str,
        _job: &str,
    ) -> Result<PrefixJobDetail, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
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
    async fn get_pair_acceptance(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _pair: PrefixPairAcceptanceReference,
    ) -> Result<Option<PrefixPairAcceptanceReceipt>, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn reconcile_pair_acceptance(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _decision: PrefixPairAcceptanceDecision,
    ) -> Result<Option<PrefixPairAcceptanceReceipt>, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn prepare_pair_acceptance(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _selection: PrefixPairAcceptanceSelection,
    ) -> Result<PrefixPairAcceptanceView, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn prepare_acceptance(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _selection: PrefixAcceptanceSelection,
    ) -> Result<PrefixAcceptanceView, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn activate_acceptance(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: ActivatePrefixAcceptanceRequest,
    ) -> Result<PrefixTargetBindingReceipt, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn approve_acceptance(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _decision: PrefixAcceptanceDecision,
    ) -> Result<PrefixAcceptanceReceipt, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    /// Independent pair-only acceptance. Does not write profile or generation
    /// rows; the returned receipt uses `pair_acceptance`, never
    /// `generation_qualification`. Uses the existing prepare/encode path.
    async fn approve_pair_acceptance(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _decision: PrefixPairAcceptanceDecision,
    ) -> Result<PrefixPairAcceptanceReceipt, InterceptorConfigServiceError> {
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
    async fn model_setup_draft(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _specification: &str,
        _revision: Option<u64>,
    ) -> Result<PrefixModelSetupDraft, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn update_model_setup_draft(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _specification: &str,
        _request: UpdatePrefixModelSetupDraftRequest,
    ) -> Result<PrefixModelSetupDraft, InterceptorConfigServiceError> {
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
    async fn get_pair_case_packet(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _specification: &str,
        _proposal: &str,
    ) -> Result<PrefixPairCasePacket, InterceptorConfigServiceError> {
        Err(InterceptorConfigServiceError::Unavailable)
    }
    async fn retain_pair_construction_proposal(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: RetainPrefixDesignProposalRequest,
    ) -> Result<PrefixPairConstructionProposal, InterceptorConfigServiceError> {
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

#[cfg(test)]
mod revision_wire_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inbound_prefix_revisions_accept_legacy_numbers_without_changing_native_serialize() {
        for revision in [
            0,
            1,
            (1_u64 << 53) - 1,
            (1_u64 << 53) + 1,
            i64::MAX as u64,
            u64::MAX,
        ] {
            for input in [json!(revision), json!(revision.to_string())] {
                let capture: CapturePrefixModelRequest = serde_json::from_value(json!({
                    "provider_id":"p","expected_provider_revision":input,"model":"m"
                }))
                .unwrap();
                assert_eq!(capture.expected_provider_revision, revision);
                assert_eq!(
                    serde_json::to_value(&capture).unwrap()["expected_provider_revision"],
                    json!(revision)
                );
                let import: ImportPrefixGenerationRequest = serde_json::from_value(json!({
                    "expected_profile_revision":input,"generation_bytes":" exact original\r\n "
                }))
                .unwrap();
                assert_eq!(import.expected_profile_revision, revision);
                let serialized = serde_json::to_value(&import).unwrap();
                assert_eq!(serialized["expected_profile_revision"], json!(revision));
                assert_eq!(serialized["generation_bytes"], " exact original\r\n ");
                let measure: MeasurePrefixRequest = serde_json::from_value(json!({
                    "expected_profile_revision":input,"generation":"g","proposal":"p","artifact_set":"a",
                    "probes_json":" exact probes ","context_window_tokens":1,"reserved_additional_input_tokens":0
                })).unwrap();
                assert_eq!(measure.expected_profile_revision, revision);
                assert_eq!(
                    serde_json::to_value(&measure).unwrap()["expected_profile_revision"],
                    json!(revision)
                );
            }
        }
        for invalid in [
            json!(""),
            json!("01"),
            json!("+1"),
            json!("-1"),
            json!(" 1"),
            json!("1 "),
            json!("1.0"),
            json!("1e2"),
            json!("18446744073709551616"),
            json!(-1),
            json!(1.0),
            json!(true),
            json!(null),
            json!([]),
        ] {
            assert!(
                serde_json::from_value::<CapturePrefixModelRequest>(
                    json!({"provider_id":"p","expected_provider_revision":invalid,"model":"m"})
                )
                .is_err()
            );
            assert!(
                serde_json::from_value::<ImportPrefixGenerationRequest>(
                    json!({"expected_profile_revision":invalid,"generation_bytes":"original"})
                )
                .is_err()
            );
            assert!(serde_json::from_value::<MeasurePrefixRequest>(json!({"expected_profile_revision":invalid,"generation":"g","proposal":"p","artifact_set":"a","probes_json":"[]","context_window_tokens":1,"reserved_additional_input_tokens":0})).is_err());
        }
    }
}
