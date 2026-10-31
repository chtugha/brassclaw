//! LLM configuration port for the WebChat v2 settings surface.
//!
//! This is the product-facing contract the webui2 Inference tab consumes to
//! list providers, add/edit/remove custom providers (including an API key),
//! pick the active provider+model, and probe a provider (test connection /
//! list models). The concrete implementation lives in the composition root
//! (`brassclaw_reborn_composition`), which owns the revisioned PostgreSQL catalogue,
//! the operator-scoped encrypted secret store, atomic role writer, and the live
//! provider-reload handle. Keeping the port here lets the facade stay the
//! single stable surface the route handlers depend on.
//!
//! Wire-safety: inbound API-key values are typed as [`SecretString`] so they
//! never land in `Debug`/logs and are deserialize-only (a request carrying a
//! key can't be serialized back out). Response snapshots never carry a key
//! value — only a boolean `api_key_set`.

use async_trait::async_trait;
use brassclaw_llm::ProviderRole;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use super::error::{RebornServicesError, RebornServicesErrorCode, RebornServicesErrorKind};
use crate::WebUiAuthenticatedCaller;

/// Wire-format token budget view sent to the settings UI.
/// Identical shape to `TokenSettingsResponse` so the frontend can reuse
/// the same `TokensTab` component verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTokenBudgetView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub conversation_history: Option<usize>,
    pub skills: Option<usize>,
    pub identity: Option<usize>,
    pub inline_control: Option<usize>,
    pub memory: Option<usize>,
    pub safety: Option<usize>,
    pub capability_surface: Option<usize>,
    pub total_input: Option<usize>,
    pub max_output: Option<usize>,
}

/// Operator-wide LLM configuration management.
#[async_trait]
pub trait LlmConfigService: Send + Sync {
    /// Current merged catalog + active selection, keys masked.
    async fn snapshot(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError>;

    /// Add or update a custom provider (and optionally its key / active state).
    async fn upsert_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: UpsertLlmProviderRequest,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError>;

    /// Deactivate a provider while retaining its definition and encrypted key.
    async fn delete_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: String,
        expected_revision: u64,
        mutation_id: uuid::Uuid,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError>;

    /// Read the original committed change, without resubmitting any data.
    async fn provider_mutation(
        &self,
        caller: WebUiAuthenticatedCaller,
        mutation_id: uuid::Uuid,
    ) -> Result<ProviderMutationReceipt, LlmConfigServiceError>;

    async fn reconcile_provider_mutation(
        &self,
        caller: WebUiAuthenticatedCaller,
        mutation_id: uuid::Uuid,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError>;

    /// Select the active provider + model.
    async fn set_active(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<LlmConfigSnapshot, LlmConfigServiceError>;

    /// Probe a provider's credentials/endpoint without persisting anything.
    async fn test_connection(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmProbeResult, LlmConfigServiceError>;

    /// List the models a provider exposes, without persisting anything.
    async fn list_models(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmModelsResult, LlmConfigServiceError>;

    /// Begin a NEAR AI browser login (GitHub/Google SSO). Returns the provider
    /// authorization URL for the frontend to open; NEAR AI redirects the browser
    /// back to the state-guarded callback. Credentials, selection and original
    /// receipt commit together; callers inspect that exact receipt and runtime
    /// reconciliation rather than polling an already active provider name.
    async fn start_nearai_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiLoginRequest,
    ) -> Result<NearAiLoginStart, LlmConfigServiceError>;

    /// Capture a one-use server nonce and original settings context before signing.
    async fn start_nearai_wallet_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiWalletLoginStartRequest,
    ) -> Result<NearAiWalletLoginStart, LlmConfigServiceError>;

    /// Complete a NEAR AI wallet (NEP-413) login. The frontend connects a NEAR
    /// wallet, signs the exact start response and posts it with its original ID.
    /// One consumed challenge permits one exchange; retries read the original
    /// receipt. Encrypted credentials/selection commit together before uptake.
    /// Wallet signing happens externally; there is no server-built auth URL.
    async fn complete_nearai_wallet_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiWalletLoginRequest,
    ) -> Result<NearAiWalletLoginResult, LlmConfigServiceError>;

    /// Start or recover this exact provider/revision/role/model intent. The
    /// mutation UUID is also its native flow identity; retries retain all bytes.
    /// Embedding is rejected before effects. The native owner, not this facade,
    /// authenticates the selected protocol and owns issuer work/publication.
    async fn start_codex_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<CodexLoginStart, LlmConfigServiceError>;

    /// Read only this original login. Never starts, refreshes, replaces an
    /// original, or adopts the current role/provider selection.
    async fn codex_login_status(
        &self,
        caller: WebUiAuthenticatedCaller,
        id: uuid::Uuid,
    ) -> Result<CodexLoginStatus, LlmConfigServiceError>;
}

/// OAuth identity provider for NEAR AI session login.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NearAiAuthProvider {
    Github,
    Google,
}

impl NearAiAuthProvider {
    /// Path segment used in the NEAR AI auth URL (`/v1/auth/<segment>`).
    pub fn as_path(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Google => "google",
        }
    }
}

/// Start a NEAR AI login with the chosen identity provider.
#[derive(Debug, Clone, Deserialize)]
pub struct NearAiLoginRequest {
    pub provider: NearAiAuthProvider,
    /// The browser's own origin (`window.location.origin`), used to build the
    /// NEAR AI `frontend_callback` back to this server's public callback route.
    /// Validated server-side to a bare `scheme://host[:port]`.
    pub origin: String,
}

/// The authorization URL the frontend opens to complete NEAR AI login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearAiLoginStart {
    /// Original publication receipt to inspect; an active provider name is insufficient.
    pub mutation_id: uuid::Uuid,
    pub auth_url: String,
}

/// Optional redirect value that the external wallet includes in its signature.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NearAiWalletLoginStartRequest {
    #[serde(default)]
    pub callback_url: Option<String>,
}

/// Sign these exact values with the external wallet, then complete this ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearAiWalletLoginStart {
    pub mutation_id: uuid::Uuid,
    pub message: String,
    pub recipient: String,
    pub nonce: Vec<u8>,
    pub callback_url: Option<String>,
}

/// A NEP-413 wallet signature plus the payload it covers, posted by the browser
/// after it connects a NEAR wallet and signs the fixed login message. The server
/// relays this to NEAR AI's `/v1/auth/near` to obtain a session token.
#[derive(Clone, Deserialize)]
pub struct NearAiWalletLoginRequest {
    /// Original server challenge, never generated again for a transport retry.
    pub mutation_id: uuid::Uuid,
    pub account_id: String,
    pub public_key: String,
    /// base64-standard encoding of the 64 raw ed25519 signature bytes.
    pub signature: String,
    /// The exact message string the wallet signed.
    pub message: String,
    /// The NEP-413 recipient the wallet signed.
    pub recipient: String,
    /// The 32-byte nonce the wallet signed (first 8 bytes are big-endian epoch
    /// millis).
    pub nonce: Vec<u8>,
    #[serde(default)]
    pub callback_url: Option<String>,
}

impl std::fmt::Debug for NearAiWalletLoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NearAiWalletLoginRequest")
            .field("mutation_id", &self.mutation_id)
            .finish_non_exhaustive()
    }
}

/// Result of a completed NEAR AI wallet login. `active` is true once NEAR AI is
/// the live provider; the frontend can then proceed to chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearAiWalletLoginResult {
    pub active: bool,
    pub mutation_receipt: ProviderMutationReceipt,
}

/// The device code + verification URL the frontend displays for Codex login.
/// The user enters `user_code` at `verification_uri`; the backend polls for
/// completion in the background.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexLoginStart {
    pub mutation_id: uuid::Uuid,
    pub expires_at: brassclaw_auth::Timestamp,
    pub poll_interval_millis: u32,
    pub user_code: Option<String>,
    pub verification_uri: Option<String>,
}

impl std::fmt::Debug for CodexLoginStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexLoginStart")
            .field("mutation_id", &self.mutation_id)
            .field("expires_at", &self.expires_at)
            .field("poll_interval_millis", &self.poll_interval_millis)
            .finish_non_exhaustive()
    }
}

/// Original-only native projection. Polling is read-only; a completed result
/// binds its own immutable provider mutation receipt. Completing may remain
/// observable after the setup deadline when the issuer return is known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexLoginStatus {
    pub mutation_id: uuid::Uuid,
    pub status: brassclaw_auth::AuthFlowStatus,
    pub expires_at: brassclaw_auth::Timestamp,
    pub error: Option<brassclaw_auth::AuthErrorCode>,
    pub mutation_receipt: Option<ProviderMutationReceipt>,
    pub poll_interval_millis: u32,
}

/// Merged catalog plus the active selection. Keys are masked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmConfigSnapshot {
    /// Desired identities from the same read-only database snapshot as the
    /// definitions, role selection and authenticated credential metadata. This
    /// is not client or prefix uptake; untracked denotes unresolved selection.
    #[serde(default)]
    pub desired_publications:
        std::collections::BTreeMap<String, brassclaw_llm::ProviderPublication>,
    /// Exact displayed role selections for optimistic edits. These content
    /// references grant no authority and are distinct from task/client pins.
    #[serde(default)]
    pub role_selection_refs: std::collections::BTreeMap<String, String>,
    /// Metadata published with the actual live client. This may precede the
    /// current desired selections; untracked is not an acknowledgement.
    #[serde(default)]
    pub runtime_publications:
        std::collections::BTreeMap<String, brassclaw_llm::ProviderPublication>,
    /// Failed target captured by the same actual publication owner. It may be
    /// older than desired and contains no backend error text or credentials.
    #[serde(default)]
    pub runtime_publication_failures:
        std::collections::BTreeMap<String, brassclaw_llm::ProviderPublication>,
    /// Original committed request. Current selections below may be newer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_receipt: Option<ProviderMutationReceipt>,
    pub providers: Vec<LlmProviderView>,
    /// Retained definitions available only for explicit operator reactivation.
    #[serde(default)]
    pub inactive_providers: Vec<LlmProviderView>,
    pub response_cache_defaults: brassclaw_llm::ResponseCachePolicy,
    pub model_discovery_defaults: brassclaw_llm::model_discovery::ModelDiscoveryPolicy,
    pub request_policy_defaults: brassclaw_llm::ProviderRequestPolicy,
    /// Active Kohai (primary inference) selection.
    /// This field is always kept in sync with `kohai_active` so that old
    /// clients reading only `active` observe the Kohai selection unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<LlmActiveSelection>,

    // ── Sempai–Kohai additive fields (all optional for back-compat) ─────────
    /// Kohai (primary inference) active selection.
    /// Identical to `active`; kept in sync so new clients can use the typed
    /// name while old clients keep reading `active`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kohai_active: Option<LlmActiveSelection>,
    /// Sempai (auditor) active selection.
    /// Absent means the interceptor runs in Passthrough — no Sempai is wired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sempai_active: Option<LlmActiveSelection>,
    /// Embedding provider selection.
    /// Absent means no embedding provider is configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_active: Option<LlmActiveSelection>,
}

/// Immutable publication acknowledgement, not a client/prefix uptake claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderMutationReceipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_job_id: Option<uuid::Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_target: Option<super::interceptor_config::PrefixJobTarget>,
    pub mutation_id: uuid::Uuid,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_role: Option<ProviderRole>,
    pub provider_id: Option<String>,
    pub provider_revision: Option<u64>,
    pub role_selections: std::collections::BTreeMap<String, String>,
}

/// One provider in the merged catalog, annotated for the settings UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmProviderView {
    #[serde(default)]
    pub embedding_models: brassclaw_llm::EmbeddingModelContracts,
    #[serde(default)]
    pub model_token_limits: brassclaw_llm::ModelTokenLimitOverrides,
    /// Exact definition revision for compare-and-replace edits.
    pub revision: u64,
    pub response_cache: brassclaw_llm::ResponseCachePolicy,
    pub model_discovery: brassclaw_llm::model_discovery::ModelDiscoveryPolicy,
    pub request_policy: brassclaw_llm::ProviderRequestPolicy,
    pub id: String,
    pub description: String,
    /// Protocol/adapter wire name (e.g. `open_ai_completions`, `anthropic`).
    pub adapter: String,
    pub default_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// `true` for compiled-in providers, `false` for operator-defined ones.
    pub builtin: bool,
    /// Whether this provider is the active selection.
    pub active: bool,
    /// The active model, present only when `active` is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_model: Option<String>,
    pub api_key_required: bool,
    /// Whether this provider supports API-key auth at all. This can be true
    /// even when `api_key_required` is false for dual-auth providers.
    pub accepts_api_key: bool,
    /// Whether an API-key value is stored for this provider (never the value).
    pub api_key_set: bool,
    pub can_list_models: bool,
    /// Token budget stored for this provider, if any.
    /// `None` means the global `[tokens]` section (or compiled defaults) apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<ProviderTokenBudgetView>,
    /// Registered context window size in tokens for this provider's default model.
    /// `None` means the provider definition does not specify a window size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u32>,

    // ── Sempai–Kohai additive fields (default false — skipped when absent) ──
    /// `true` when this provider is the active Kohai selection.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_kohai: bool,
    /// `true` when this provider is the active Sempai selection.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_sempai: bool,
    /// `true` when this provider is the active Embedding selection.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_embedding: bool,
}

/// The active provider + model selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmActiveSelection {
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Add or update a custom provider. Deserialize-only (carries a secret).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpsertLlmProviderRequest {
    /// Reuse this exact identity and payload after an uncertain response.
    pub mutation_id: uuid::Uuid,
    /// Required for a new set_active publication, from the displayed Kohai role.
    /// Kept optional on the wire so an old committed request can be reconciled.
    #[serde(default)]
    pub expected_selection_ref: Option<String>,
    /// Omitted preserves the previous model contracts; an empty object clears them.
    #[serde(default, deserialize_with = "deserialize_embedding_models_update")]
    pub embedding_models: Option<brassclaw_llm::EmbeddingModelContracts>,
    /// Omitted preserves all model ceilings; an empty map explicitly clears them.
    #[serde(default, deserialize_with = "deserialize_model_token_limits_update")]
    pub model_token_limits: Option<brassclaw_llm::ModelTokenLimitOverrides>,
    /// Required for replacing/reactivating a retained identity; omit only for a new ID.
    #[serde(default)]
    pub expected_revision: Option<u64>,
    /// Omitted preserves the existing definition policy.
    #[serde(default, deserialize_with = "deserialize_response_cache_update")]
    pub response_cache: Option<brassclaw_llm::ResponseCachePolicy>,
    #[serde(default, deserialize_with = "deserialize_model_discovery_update")]
    pub model_discovery: Option<brassclaw_llm::model_discovery::ModelDiscoveryPolicy>,
    #[serde(default, deserialize_with = "deserialize_request_policy_update")]
    pub request_policy: Option<brassclaw_llm::ProviderRequestPolicy>,
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Protocol/adapter wire name.
    pub adapter: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    /// New key value. Absent leaves any stored key untouched; the UI sends the
    /// `••••••••` sentinel for "unchanged" which the impl treats as absent.
    #[serde(default)]
    pub api_key: Option<SecretString>,
    /// When `true`, also make this the active provider.
    #[serde(default)]
    pub set_active: bool,
    /// Model to activate when `set_active` is `true`.
    #[serde(default)]
    pub model: Option<String>,
    /// Token budget for this provider.  `None` leaves any existing budget
    /// untouched.  Send an all-None `ProviderTokenBudgetView` to clear it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<ProviderTokenBudgetView>,
}

fn deserialize_request_policy_update<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<brassclaw_llm::ProviderRequestPolicy>, D::Error> {
    brassclaw_llm::ProviderRequestPolicy::deserialize(deserializer).map(Some)
}
fn deserialize_model_token_limits_update<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<brassclaw_llm::ModelTokenLimitOverrides>, D::Error> {
    brassclaw_llm::ModelTokenLimitOverrides::deserialize(deserializer).map(Some)
}

fn deserialize_embedding_models_update<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<brassclaw_llm::EmbeddingModelContracts>, D::Error> {
    brassclaw_llm::EmbeddingModelContracts::deserialize(deserializer).map(Some)
}

fn deserialize_response_cache_update<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<brassclaw_llm::ResponseCachePolicy>, D::Error> {
    // Omitted retains the old policy. Explicit null is not a reset instruction.
    brassclaw_llm::ResponseCachePolicy::deserialize(deserializer).map(Some)
}

fn deserialize_model_discovery_update<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<brassclaw_llm::model_discovery::ModelDiscoveryPolicy>, D::Error> {
    brassclaw_llm::model_discovery::ModelDiscoveryPolicy::deserialize(deserializer).map(Some)
}

/// Select the active provider + model.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetActiveLlmRequest {
    pub mutation_id: uuid::Uuid,
    /// Required for every new edit; original committed retries are read first.
    #[serde(default)]
    pub expected_selection_ref: Option<String>,
    /// Compare the selected provider definition when the caller has a snapshot.
    #[serde(default)]
    pub expected_revision: Option<u64>,
    pub provider_id: String,
    #[serde(default)]
    pub model: Option<String>,
    /// Which role to assign this provider to.
    /// Defaults to [`ProviderRole::Kohai`] when absent for back-compat with
    /// existing clients that do not send a role field.
    #[serde(default)]
    pub role: Option<ProviderRole>,
}

/// Probe a provider. Deserialize-only (may carry a secret).
#[derive(Deserialize)]
pub struct LlmProbeRequest {
    /// Explicit unsaved operator discovery settings. Omission uses the captured
    /// persisted provider policy, or the API's default for a new definition.
    #[serde(default, deserialize_with = "deserialize_model_discovery_update")]
    pub model_discovery: Option<brassclaw_llm::model_discovery::ModelDiscoveryPolicy>,
    pub adapter: String,
    #[serde(default)]
    pub base_url: Option<String>,
    pub provider_id: String,
    #[serde(default)]
    pub model: Option<String>,
    /// Optional override key for the probe; when absent the impl falls back to
    /// the provider's encrypted stored key when the endpoint/adapter match.
    #[serde(default)]
    pub api_key: Option<SecretString>,
}

/// Result of a connection probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmProbeResult {
    pub ok: bool,
    pub message: String,
}

/// Result of a model-listing probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmModelsResult {
    pub ok: bool,
    #[serde(default)]
    pub models: Vec<String>,
    pub message: String,
}

/// Port-level error surface. The facade maps this to the sanitized
/// `RebornServicesError` taxonomy; no backend strings, paths, or secrets cross
/// the boundary beyond the user-safe `reason` on `InvalidRequest`.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LlmConfigServiceError {
    /// Caller-supplied input was invalid. `reason` is user-safe.
    #[error("invalid request: {reason}")]
    InvalidRequest {
        field: Option<String>,
        reason: String,
    },
    /// The named provider does not exist in the merged catalog.
    #[error("provider not found")]
    NotFound,
    /// A revision or configuration conflict prevents publication.
    #[error("provider role conflict: {reason}")]
    Conflict { reason: String },
    /// The configuration backend (filesystem / secret store / reload) failed
    /// transiently or is not wired.
    #[error("llm config backend unavailable")]
    Unavailable,
    /// An internal invariant was violated.
    #[error("llm config internal error")]
    Internal,
}

pub(super) fn map_llm_config_error(error: LlmConfigServiceError) -> RebornServicesError {
    match error {
        LlmConfigServiceError::InvalidRequest { .. } => {
            RebornServicesError::from_status(RebornServicesErrorCode::InvalidRequest, 400, false)
        }
        LlmConfigServiceError::NotFound => {
            RebornServicesError::from_status(RebornServicesErrorCode::NotFound, 404, false)
        }
        LlmConfigServiceError::Conflict { .. } => {
            RebornServicesError::from_status(RebornServicesErrorCode::InvalidRequest, 409, false)
        }
        LlmConfigServiceError::Unavailable => RebornServicesError::service_unavailable(true),
        LlmConfigServiceError::Internal => RebornServicesError::internal_invariant(),
    }
}

/// Error returned when an LLM-config method is invoked but no service is wired.
pub(super) fn llm_config_unavailable() -> RebornServicesError {
    RebornServicesError::from_status_kind(
        RebornServicesErrorCode::Unavailable,
        RebornServicesErrorKind::ServiceUnavailable,
        503,
        false,
    )
}

#[cfg(test)]
mod codex_login_debug_tests {
    #[test]
    fn codex_login_start_debug_never_discloses_user_code_or_verification_uri() {
        let start = super::CodexLoginStart {
            mutation_id: uuid::Uuid::new_v4(),
            expires_at: "2026-10-31T12:00:00Z".parse().unwrap(),
            poll_interval_millis: 713,
            user_code: Some("PRIVATE_USER_CODE".into()),
            verification_uri: Some("https://private-issuer.example/device".into()),
        };
        let debug = format!("{start:?}");
        assert!(!debug.contains("PRIVATE_USER_CODE"));
        assert!(!debug.contains("private-issuer"));
        assert_eq!(
            serde_json::to_value(&start).unwrap()["user_code"],
            "PRIVATE_USER_CODE"
        );
    }
}
