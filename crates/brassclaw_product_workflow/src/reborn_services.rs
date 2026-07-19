//! WebUI-facing Reborn service facade.
//!
//! This module is the stable high-level API beta WebUI route handlers use
//! instead of reaching into turn coordination, thread stores, runtime lanes, DB
//! stores, dispatchers, or capability hosts directly.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex as StdMutex, Weak},
};

use async_trait::async_trait;
use brassclaw_auth::{
    AuthProductScope, AuthProviderId, CredentialAccountId, CredentialAccountProjection,
    CredentialAccountUpdateBinding, ProviderScope,
};
use brassclaw_host_api::{
    AgentId, ExtensionId, PermissionMode, ProjectId, TenantId, ThreadId, UserId,
};
use brassclaw_product_adapters::{
    ProductAdapterError, ProductWorkflowRejectionKind, ProjectionStream,
    ProjectionSubscriptionRequest,
};
use brassclaw_threads::{
    AcceptInboundMessageRequest, AcceptedInboundMessageReplay, EnsureThreadRequest, MessageContent,
    MessageStatus, ReplayAcceptedInboundMessageRequest, SessionThreadError, SessionThreadService,
    ThreadHistoryRequest, ThreadMessageId, ThreadScope,
};
use brassclaw_turns::{
    AcceptedMessageRef, GateRef, GetRunStateRequest, IdempotencyKey, ResumeTurnPrecondition,
    ResumeTurnRequest, SanitizedCancelReason, SubmitTurnRequest, SubmitTurnResponse, TurnActor,
    TurnCoordinator, TurnError, TurnRunId, TurnScope, TurnStatus,
};
use chrono::Utc;
use secrecy::SecretString;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use uuid::Uuid;

use crate::{
    ApprovalInteractionDecision, ApprovalInteractionService, AuthInteractionDecision,
    AuthInteractionRejectionKind, AuthInteractionService, LifecyclePackageRef,
    LifecycleProductFacade, ProductWorkflowError, ResolveApprovalInteractionRequest,
    ResolveApprovalInteractionResponse, ResolveAuthInteractionRequest,
    ResolveAuthInteractionResponse, UnsupportedLifecycleProductFacade, WebUiAuthenticatedCaller,
    WebUiCancelRunRequest, WebUiCreateThreadRequest, WebUiGateResolution, WebUiInboundCommand,
    WebUiInboundValidationCode, WebUiInboundValidationError, WebUiListAutomationsRequest,
    WebUiListThreadsRequest, WebUiResolveGateRequest, WebUiSendMessageRequest,
    WebUiSetupExtensionRequest,
    approval_interaction::RejectingApprovalInteractionService,
    auth_interaction::RejectingAuthInteractionService,
    binding_ref::{
        DEFAULT_BINDING_REF_RAW_MAX_BYTES, bounded_reply_target_binding_ref,
        bounded_source_binding_ref,
    },
    is_approval_gate_ref, is_auth_gate_ref,
};

mod error;
mod extension_onboarding;
mod extension_setup_credentials;
mod extensions;
mod lifecycle_setup;

type CacheInvalidatorFn = Arc<dyn Fn(&str, &str) + Send + Sync>;
mod llm_config;
mod types;

/// Trait for storing and retrieving capability permission overrides.
///
/// This trait abstracts the database layer for V2 capability permissions,
/// allowing different storage backends (LibSQL, Postgres, etc.) to be used.
#[async_trait]
pub trait CapabilityPermissionStore: Send + Sync {
    /// Get the permission override for a specific capability.
    async fn get_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<Option<PermissionMode>, Box<dyn std::error::Error + Send + Sync>>;

    /// Set a permission override for a specific capability.
    async fn set_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
        mode: PermissionMode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Delete a permission override for a specific capability.
    async fn delete_capability_permission(
        &self,
        tenant_id: &str,
        capability_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// List all permission overrides for a tenant.
    async fn list_capability_overrides(
        &self,
        tenant_id: &str,
    ) -> Result<HashMap<String, PermissionMode>, Box<dyn std::error::Error + Send + Sync>>;
}

pub use error::{RebornServicesError, RebornServicesErrorCode, RebornServicesErrorKind};
pub use llm_config::{
    CodexLoginStart, LlmActiveSelection, LlmConfigService, LlmConfigServiceError,
    LlmConfigSnapshot, LlmModelsResult, LlmProbeRequest, LlmProbeResult, LlmProviderView,
    NearAiAuthProvider, NearAiLoginRequest, NearAiLoginStart, NearAiWalletLoginRequest,
    NearAiWalletLoginResult, ProviderTokenBudgetView, SetActiveLlmRequest,
    UpsertLlmProviderRequest,
};
pub use types::{
    RebornAutomationInfo, RebornAutomationRunStatus, RebornAutomationSource, RebornAutomationState,
    RebornCancelRunResponse, RebornCapabilityInfo, RebornChannelConnectAction,
    RebornChannelConnectStrategy, RebornConnectableChannelInfo,
    RebornConnectableChannelListResponse, RebornCreateThreadResponse, RebornDeleteThreadRequest,
    RebornDeleteThreadResponse, RebornExtensionActionResponse, RebornExtensionCredentialSetup,
    RebornExtensionInfo, RebornExtensionListResponse, RebornExtensionOnboardingPayload,
    RebornExtensionOnboardingState, RebornExtensionRegistryEntry, RebornExtensionRegistryResponse,
    RebornExtensionSetupField, RebornExtensionSetupSecret, RebornGetRunStateRequest,
    RebornGetRunStateResponse, RebornListAutomationsResponse, RebornListCapabilitiesResponse,
    RebornListThreadsResponse, RebornOutboundDeliveryModality,
    RebornOutboundDeliveryTargetCapabilities, RebornOutboundDeliveryTargetChannel,
    RebornOutboundDeliveryTargetDescription, RebornOutboundDeliveryTargetDisplayName,
    RebornOutboundDeliveryTargetId, RebornOutboundDeliveryTargetListResponse,
    RebornOutboundDeliveryTargetOption, RebornOutboundDeliveryTargetSummary,
    RebornOutboundPreferencesResponse, RebornResolveGateResponse, RebornResumeGateResponse,
    RebornSetOutboundPreferencesRequest, RebornSetupExtensionResponse, RebornStreamEventsRequest,
    RebornStreamEventsResponse, RebornSubmitTurnResponse, RebornTimelineRequest,
    RebornTimelineResponse, RebornUpdateCapabilityPermissionRequest,
    RebornUpdateCapabilityPermissionResponse,
};

type SkillActivationRecorder =
    dyn Fn(&TurnScope, &AcceptedMessageRef, &str) -> Result<(), RebornServicesError> + Send + Sync;
type SkillActivationClearer =
    dyn Fn(&TurnScope, &AcceptedMessageRef) -> Result<(), RebornServicesError> + Send + Sync;
type ThreadOperationLocks = StdMutex<HashMap<String, Weak<AsyncMutex<()>>>>;

#[async_trait]
pub trait ConnectableChannelsProductFacade: Send + Sync {
    async fn list_connectable_channels(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornConnectableChannelListResponse, RebornServicesError>;
}

#[derive(Debug, Clone, Default)]
pub struct StaticConnectableChannelsProductFacade {
    channels: Arc<[RebornConnectableChannelInfo]>,
}

impl StaticConnectableChannelsProductFacade {
    pub fn new(channels: impl Into<Vec<RebornConnectableChannelInfo>>) -> Self {
        Self {
            channels: Arc::from(channels.into()),
        }
    }
}

#[async_trait]
impl ConnectableChannelsProductFacade for StaticConnectableChannelsProductFacade {
    async fn list_connectable_channels(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornConnectableChannelListResponse, RebornServicesError> {
        Ok(RebornConnectableChannelListResponse {
            channels: self.channels.iter().cloned().collect(),
        })
    }
}

// ── Skills facade ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebornSkillInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String, // "system" | "user" | "installed"
    pub keywords: Vec<String>,
    pub tags: Vec<String>,
    pub requires_skills: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebornListSkillsResponse {
    pub skills: Vec<RebornSkillInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebornInstallSkillRequest {
    pub content: String,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebornSkillInstallResult {
    pub name: String,
    pub source: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebornSkillRemoveResult {
    pub name: String,
    pub success: bool,
    pub message: String,
}

#[async_trait]
pub trait SkillsProductFacade: Send + Sync {
    async fn list_skills(
        &self,
        caller: &WebUiAuthenticatedCaller,
    ) -> Result<RebornListSkillsResponse, RebornServicesError>;

    async fn install_skill(
        &self,
        caller: &WebUiAuthenticatedCaller,
        content: String,
        source_url: Option<String>,
    ) -> Result<RebornSkillInstallResult, RebornServicesError>;

    async fn remove_skill(
        &self,
        caller: &WebUiAuthenticatedCaller,
        name: &str,
    ) -> Result<RebornSkillRemoveResult, RebornServicesError>;
}

#[derive(Debug)]
pub struct UnsupportedSkillsProductFacade;

#[async_trait]
impl SkillsProductFacade for UnsupportedSkillsProductFacade {
    async fn list_skills(
        &self,
        _caller: &WebUiAuthenticatedCaller,
    ) -> Result<RebornListSkillsResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn install_skill(
        &self,
        _caller: &WebUiAuthenticatedCaller,
        _content: String,
        _source_url: Option<String>,
    ) -> Result<RebornSkillInstallResult, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn remove_skill(
        &self,
        _caller: &WebUiAuthenticatedCaller,
        _name: &str,
    ) -> Result<RebornSkillRemoveResult, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }
}

#[async_trait]
pub trait OutboundPreferencesProductFacade: Send + Sync {
    /// Return the authenticated caller's scoped outbound preferences.
    ///
    /// Real implementations must scope stored preferences by the caller's
    /// tenant/user identity. The Phase 1 unsupported implementation returns an
    /// empty projection so read callers can treat "not configured yet" as a
    /// stable state while mutation and target inventory remain fail-closed.
    async fn get_outbound_preferences(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError>;

    /// Persist the caller's scoped outbound delivery preferences.
    ///
    /// Implementations must scope writes by the caller's tenant/user identity.
    /// `RebornServices` installs `UnsupportedOutboundPreferencesProductFacade`
    /// by default, which keeps Phase 1 mutation attempts fail-closed with a
    /// non-retryable service-unavailable response until a real facade is wired.
    async fn set_outbound_preferences(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornSetOutboundPreferencesRequest,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError>;

    /// List delivery targets available to the authenticated caller.
    ///
    /// Implementations must scope target inventory by the caller's tenant/user
    /// identity. `RebornServices` installs
    /// `UnsupportedOutboundPreferencesProductFacade` by default, which keeps
    /// Phase 1 target discovery fail-closed with a non-retryable
    /// service-unavailable response until a real facade is wired.
    async fn list_outbound_delivery_targets(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundDeliveryTargetListResponse, RebornServicesError>;
}

#[derive(Debug)]
pub struct UnsupportedOutboundPreferencesProductFacade;

impl UnsupportedOutboundPreferencesProductFacade {
    pub fn new_static() -> Self {
        Self
    }
}

#[async_trait]
impl OutboundPreferencesProductFacade for UnsupportedOutboundPreferencesProductFacade {
    async fn get_outbound_preferences(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError> {
        Ok(RebornOutboundPreferencesResponse::default())
    }

    async fn set_outbound_preferences(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: RebornSetOutboundPreferencesRequest,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError> {
        Err(outbound_preferences_unavailable())
    }

    async fn list_outbound_delivery_targets(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundDeliveryTargetListResponse, RebornServicesError> {
        Err(outbound_preferences_unavailable())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionCredentialStatusRequest {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub provider_scopes: Vec<ProviderScope>,
    pub requester_extension: ExtensionId,
}

#[derive(Debug)]
pub struct ExtensionCredentialSubmitRequest {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub label: String,
    pub requester_extension: ExtensionId,
    pub existing_account: Option<CredentialAccountUpdateBinding>,
    pub secret: SecretString,
}

#[async_trait]
pub trait ExtensionCredentialSetupService: Send + Sync {
    async fn credential_status(
        &self,
        request: ExtensionCredentialStatusRequest,
    ) -> Result<Option<CredentialAccountProjection>, RebornServicesError>;

    async fn submit_manual_token(
        &self,
        request: ExtensionCredentialSubmitRequest,
    ) -> Result<CredentialAccountId, RebornServicesError>;
}

/// Product caller scope for actions that must run against a concrete agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductAgentBoundCaller {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub agent_id: AgentId,
    pub project_id: Option<ProjectId>,
}

impl ProductAgentBoundCaller {
    pub fn new(
        tenant_id: TenantId,
        user_id: UserId,
        agent_id: AgentId,
        project_id: Option<ProjectId>,
    ) -> Self {
        Self {
            tenant_id,
            user_id,
            agent_id,
            project_id,
        }
    }
}

#[async_trait]
pub trait AutomationProductFacade: Send + Sync {
    async fn list_automations(
        &self,
        caller: ProductAgentBoundCaller,
        limit: usize,
    ) -> Result<Vec<RebornAutomationInfo>, RebornServicesError>;
}

#[derive(Debug)]
pub struct UnsupportedAutomationProductFacade;

impl UnsupportedAutomationProductFacade {
    pub fn new_static() -> Self {
        Self
    }
}

#[async_trait]
impl AutomationProductFacade for UnsupportedAutomationProductFacade {
    async fn list_automations(
        &self,
        _caller: ProductAgentBoundCaller,
        _limit: usize,
    ) -> Result<Vec<RebornAutomationInfo>, RebornServicesError> {
        Err(automation_unavailable())
    }
}

#[derive(Clone, Copy)]
enum GateResolutionRoute {
    Approval,
    Auth,
    Generic,
}

impl GateResolutionRoute {
    fn from_run_state(
        status: TurnStatus,
        parked_gate_ref: Option<&GateRef>,
        requested_gate_ref: &GateRef,
        resolution: &WebUiGateResolution,
    ) -> Result<Self, RebornServicesError> {
        match status {
            TurnStatus::BlockedApproval => {
                validate_current_gate_ref(
                    parked_gate_ref,
                    requested_gate_ref,
                    RebornServicesErrorKind::BlockedApproval,
                )?;
                Ok(Self::Approval)
            }
            TurnStatus::BlockedAuth => {
                validate_current_gate_ref(
                    parked_gate_ref,
                    requested_gate_ref,
                    RebornServicesErrorKind::BlockedAuthentication,
                )?;
                Ok(Self::Auth)
            }
            status if status.is_terminal() => Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Conflict,
                RebornServicesErrorKind::Conflict,
                409,
                false,
            )),
            _ => Ok(Self::from_gate_shape(requested_gate_ref, resolution)),
        }
    }

    fn from_gate_shape(gate_ref: &GateRef, resolution: &WebUiGateResolution) -> Self {
        match (
            is_approval_gate_ref(gate_ref),
            is_auth_gate_ref(gate_ref),
            matches!(resolution, WebUiGateResolution::CredentialProvided { .. }),
        ) {
            (true, _, _) => Self::Approval,
            (_, true, _) | (_, _, true) => Self::Auth,
            _ => Self::Generic,
        }
    }
}

/// Stable WebUI-facing facade surface for beta Reborn routes.
#[async_trait]
pub trait RebornServicesApi: Send + Sync {
    async fn create_thread(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiCreateThreadRequest,
    ) -> Result<RebornCreateThreadResponse, RebornServicesError>;

    async fn submit_turn(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiSendMessageRequest,
    ) -> Result<RebornSubmitTurnResponse, RebornServicesError>;

    async fn delete_thread(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornDeleteThreadRequest,
    ) -> Result<RebornDeleteThreadResponse, RebornServicesError>;

    async fn get_timeline(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornTimelineRequest,
    ) -> Result<RebornTimelineResponse, RebornServicesError>;

    async fn stream_events(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornStreamEventsRequest,
    ) -> Result<RebornStreamEventsResponse, RebornServicesError>;

    async fn cancel_run(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiCancelRunRequest,
    ) -> Result<RebornCancelRunResponse, RebornServicesError>;

    async fn resolve_gate(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiResolveGateRequest,
    ) -> Result<RebornResolveGateResponse, RebornServicesError>;

    async fn get_run_state(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornGetRunStateRequest,
    ) -> Result<RebornGetRunStateResponse, RebornServicesError>;

    /// List the caller-scoped threads. Pagination is opaque: callers
    /// echo back the `next_cursor` from a prior response to retrieve
    /// the next page; the cursor encoding is implementation-defined.
    ///
    /// Returns an empty list + `next_cursor: None` when no threads
    /// exist for the caller's scope.
    async fn list_threads(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiListThreadsRequest,
    ) -> Result<RebornListThreadsResponse, RebornServicesError>;

    async fn list_automations(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiListAutomationsRequest,
    ) -> Result<RebornListAutomationsResponse, RebornServicesError>;

    async fn list_connectable_channels(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornConnectableChannelListResponse, RebornServicesError> {
        Ok(RebornConnectableChannelListResponse {
            channels: Vec::new(),
        })
    }

    /// Return the authenticated caller's scoped outbound preferences.
    ///
    /// Implementations must scope stored preferences by the caller's
    /// tenant/user identity. Unsupported behavior belongs in
    /// `UnsupportedOutboundPreferencesProductFacade`, not in trait defaults.
    async fn get_outbound_preferences(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError>;

    /// Persist the authenticated caller's outbound delivery preference.
    ///
    /// Implementations must scope mutations by the caller's tenant/user
    /// identity and fail closed when no writable outbound-preferences facade is
    /// wired.
    async fn set_outbound_preferences(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornSetOutboundPreferencesRequest,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError>;

    /// List delivery targets available to the authenticated caller.
    ///
    /// Implementations must scope target inventory by the caller's tenant/user
    /// identity and fail closed when no outbound target inventory facade is
    /// wired.
    async fn list_outbound_delivery_targets(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundDeliveryTargetListResponse, RebornServicesError>;

    async fn list_extensions(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornExtensionListResponse, RebornServicesError>;

    async fn list_extension_registry(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornExtensionRegistryResponse, RebornServicesError>;

    async fn install_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
    ) -> Result<RebornExtensionActionResponse, RebornServicesError>;

    async fn activate_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
    ) -> Result<RebornExtensionActionResponse, RebornServicesError>;

    async fn remove_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
    ) -> Result<RebornExtensionActionResponse, RebornServicesError>;

    /// Run a step in a v2-native extension onboarding flow. Today the
    /// facade returns
    /// [`RebornSetupExtensionStatus::NotImplemented`](types::RebornSetupExtensionStatus::NotImplemented)
    /// because the underlying extension lifecycle is still v1-only.
    /// The route exists so the WebUI v2 entrypoint inventory is
    /// complete and so future onboarding port work has a fixed surface
    /// to fill in.
    ///
    /// `package_ref` is the validated lifecycle package identity from
    /// the route path or request body. The browser can still render
    /// display names from registry metadata, but lifecycle side effects
    /// use package refs end to end.
    async fn setup_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
        request: WebUiSetupExtensionRequest,
    ) -> Result<RebornSetupExtensionResponse, RebornServicesError>;

    /// LLM provider configuration: merged catalog + active selection.
    ///
    /// The six LLM-config methods default to "service unavailable" so facade
    /// impls (and test fakes) that don't wire an [`LlmConfigService`] inherit a
    /// safe surface; the default `RebornServices` overrides them all.
    async fn get_llm_config(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let _ = caller;
        Err(llm_config::llm_config_unavailable())
    }

    /// Add or update a custom LLM provider (and optionally its key / active state).
    async fn upsert_llm_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: UpsertLlmProviderRequest,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let _ = (caller, request);
        Err(llm_config::llm_config_unavailable())
    }

    /// Remove a custom LLM provider and any stored key for it.
    async fn delete_llm_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: String,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let _ = (caller, provider_id);
        Err(llm_config::llm_config_unavailable())
    }

    /// Select the active LLM provider + model.
    async fn set_active_llm(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let _ = (caller, request);
        Err(llm_config::llm_config_unavailable())
    }

    /// Probe an LLM provider's credentials/endpoint without persisting.
    async fn test_llm_connection(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmProbeResult, RebornServicesError> {
        let _ = (caller, request);
        Err(llm_config::llm_config_unavailable())
    }

    /// List the models an LLM provider exposes, without persisting.
    async fn list_llm_models(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmModelsResult, RebornServicesError> {
        let _ = (caller, request);
        Err(llm_config::llm_config_unavailable())
    }

    /// Begin a NEAR AI browser login; returns the authorization URL to open.
    async fn start_nearai_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiLoginRequest,
    ) -> Result<NearAiLoginStart, RebornServicesError> {
        let _ = (caller, request);
        Err(llm_config::llm_config_unavailable())
    }

    /// Complete a NEAR AI wallet (NEP-413) login from a browser-signed message.
    async fn complete_nearai_wallet_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiWalletLoginRequest,
    ) -> Result<NearAiWalletLoginResult, RebornServicesError> {
        let _ = (caller, request);
        Err(llm_config::llm_config_unavailable())
    }

    /// Begin an OpenAI Codex device-code login; returns the user code + URL.
    async fn start_codex_login(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<CodexLoginStart, RebornServicesError> {
        let _ = caller;
        Err(llm_config::llm_config_unavailable())
    }

    /// List all available capabilities (built-in and extension-provided) with their
    /// current permission modes and default permissions.
    async fn list_capabilities(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornListCapabilitiesResponse, RebornServicesError>;

    /// Update the permission mode for a specific capability. The permission override
    /// is scoped to the caller's tenant and persisted in the database.
    async fn update_capability_permission(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornUpdateCapabilityPermissionRequest,
    ) -> Result<RebornUpdateCapabilityPermissionResponse, RebornServicesError>;

    /// List all skills available to the authenticated caller.
    async fn list_skills(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornListSkillsResponse, RebornServicesError>;

    /// Install a skill from its SKILL.md content.
    async fn install_skill(
        &self,
        caller: WebUiAuthenticatedCaller,
        content: String,
        source_url: Option<String>,
    ) -> Result<RebornSkillInstallResult, RebornServicesError>;

    /// Remove a skill by name.
    async fn remove_skill(
        &self,
        caller: WebUiAuthenticatedCaller,
        name: String,
    ) -> Result<RebornSkillRemoveResult, RebornServicesError>;

    /// Safety configuration methods - default to "not implemented" so facades that
    /// don't wire safety config inherit a safe surface.
    async fn get_safety_sensitive_paths(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn update_safety_sensitive_paths(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: crate::safety_config::UpdateSafetyConfigRequest,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn get_safety_workspace_rules(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn update_safety_workspace_rules(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: crate::safety_config::UpdateSafetyConfigRequest,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn get_safety_blocked_paths(
        &self,
        _caller: WebUiAuthenticatedCaller,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn update_safety_blocked_paths(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: crate::safety_config::UpdateSafetyConfigRequest,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Per-provider token settings methods — default to "not implemented" so
    /// facades that don't wire the token store inherit a safe surface.
    async fn get_provider_token_settings(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _provider_id: &str,
    ) -> Result<crate::token_settings::TokenSettingsResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn update_provider_token_settings(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _provider_id: &str,
        _request: crate::token_settings::UpdateTokenSettingsRequest,
    ) -> Result<crate::token_settings::TokenSettingsResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Reduction rules — default to "not implemented" so facades that don't
    /// wire the rules store inherit a safe surface. The composer in
    /// `brassclaw_reborn_composition` overrides these via `.with_...` so the
    /// WebUI v2 operators can author, list, and replace rules without
    /// restarting the runtime.
    async fn list_reduction_rules(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
    ) -> Result<crate::reduction_rules::ReductionRulesResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn replace_reduction_rules(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _request: crate::reduction_rules::ReductionRulesRequest,
    ) -> Result<crate::reduction_rules::ReductionRulesResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn author_reduction_rule(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _request: crate::reduction_rules::AuthorReductionRuleRequest,
    ) -> Result<crate::reduction_rules::AuthorReductionRuleResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    // ── Recipe-Skill-Tool library ────────────────────────────
    //
    // Phase 7 surface. Defaults return `501` so facades that don't
    // wire a [`RecipeStore`] inherit a safe surface; the default
    // `RebornServices` overrides these via `with_recipe_store`.

    /// List the caller's Recipe library.
    async fn list_recipes(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
    ) -> Result<crate::recipes::RecipeListResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// List the caller's ToolSkill library.
    async fn list_tool_skills(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
    ) -> Result<crate::recipes::ToolSkillListResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Fetch one Recipe by id (full payload).
    async fn get_recipe(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _recipe_id: &str,
    ) -> Result<crate::recipes::RecipeDetail, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Fetch one ToolSkill by id (full payload).
    async fn get_tool_skill(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _skill_id: &str,
    ) -> Result<crate::recipes::ToolSkillDetail, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// List validation-queue rows (post-extraction review).
    async fn list_validation_queue(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
    ) -> Result<crate::recipes::ValidationQueueListResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Count validation-queue rows by `validation_status`.
    async fn count_validation_queue(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _status: &str,
    ) -> Result<crate::recipes::ValidationQueueCountResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Promote a Recipe to `validated`.
    async fn validate_recipe(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _recipe_id: &str,
        _request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Reject a Recipe (soft delete → moves to garbage after 30 days).
    async fn reject_recipe(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _recipe_id: &str,
        _request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Send a Recipe back into the review queue for an LLM
    /// review-and-fix cycle. Requires `feedback` in the request body
    /// so the review mission has context to work with.
    async fn request_recipe_review(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _recipe_id: &str,
        _request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Promote a ToolSkill to `validated`.
    async fn validate_tool_skill(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _skill_id: &str,
        _request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn reject_tool_skill(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _skill_id: &str,
        _request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    async fn request_tool_skill_review(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _skill_id: &str,
        _request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }

    /// Record one execution outcome (success/failure) for a Recipe or
    /// ToolSkill. Drives the Wilson/tier counters via the engine
    /// `MetricRecorder`.
    async fn record_recipe_outcome(
        &self,
        _caller: WebUiAuthenticatedCaller,
        _project_id: &str,
        _request: crate::recipes::RecordOutcomeRequest,
    ) -> Result<crate::recipes::RecordOutcomeResponse, RebornServicesError> {
        Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            501,
            false,
        ))
    }
}

/// Default facade implementation composed at the WebUI boundary.
#[derive(Clone)]
pub struct RebornServices {
    thread_service: Arc<dyn SessionThreadService>,
    turn_coordinator: Arc<dyn TurnCoordinator>,
    event_stream: Option<Arc<dyn ProjectionStream>>,
    lifecycle_facade: Arc<dyn LifecycleProductFacade>,
    automation_facade: Arc<dyn AutomationProductFacade>,
    connectable_channels_facade: Arc<dyn ConnectableChannelsProductFacade>,
    outbound_preferences_facade: Arc<dyn OutboundPreferencesProductFacade>,
    skills_facade: Arc<dyn SkillsProductFacade>,
    approval_interactions: Arc<dyn ApprovalInteractionService>,
    auth_interactions: Arc<dyn AuthInteractionService>,
    extension_credentials: Option<Arc<dyn ExtensionCredentialSetupService>>,
    skill_activation_recorder: Option<Arc<SkillActivationRecorder>>,
    skill_activation_clearer: Option<Arc<SkillActivationClearer>>,
    llm_config: Option<Arc<dyn LlmConfigService>>,
    thread_operation_locks: Arc<ThreadOperationLocks>,
    extension_registry: Option<Arc<dyn brassclaw_host_api::CapabilityRegistry>>,
    capability_permission_store: Option<Arc<dyn CapabilityPermissionStore>>,
    safety_config_store: Option<Arc<dyn crate::safety_config_store::SafetyConfigStore>>,
    token_settings_store: Option<Arc<dyn crate::token_settings_store::TokenSettingsStore>>,
    /// Reduction-rule persistence port backing `/tokens/reduction-rules/*`.
    /// When unwired, the default `501` responses keep the WebUI surface
    /// fail-safe: there's no cache to invalidate, no partial write to roll
    /// back, and no orchestrator path engaged.
    reduction_rule_store: Option<Arc<dyn crate::reduction_rules::ReductionRuleStore>>,
    /// Hook composition installs to flush the engine-side
    /// `(project_id, user_id) → rules` cache after a successful PUT. The
    /// orchestrator Python picks up the change on the very next over-budget
    /// turn — no restart required.
    reduction_rules_cache_invalidator: Option<CacheInvalidatorFn>,
    /// Callback that updates the live context-token budget slot in the running
    /// `DefaultContextStrategy`. When wired, calling this updates the token
    /// cap on the very next turn — no restart required.
    live_context_budget_setter: Option<Arc<dyn Fn(Option<usize>) + Send + Sync>>,
    /// Live-setter for max-output-tokens. Mirrors `live_context_budget_setter`
    /// for the `max_output` slot on `LiveTokenBudget`.
    live_max_output_setter: Option<Arc<dyn Fn(Option<usize>) + Send + Sync>>,
    /// Live-setter for total-input-tokens.
    live_total_input_setter: Option<Arc<dyn Fn(Option<usize>) + Send + Sync>>,
    /// Live-setter for inline-control-tokens.
    live_inline_control_setter: Option<Arc<dyn Fn(Option<usize>) + Send + Sync>>,
    /// Recipe-Skill-Tool library port backing `/recipes/*` and
    /// `/tool-skills/*` plus the validation queue. When unwired,
    /// the trait defaults above return `501`.
    recipe_store: Option<Arc<dyn crate::recipes::RecipeStore>>,
}

impl RebornServices {
    pub fn new(
        thread_service: Arc<dyn SessionThreadService>,
        turn_coordinator: Arc<dyn TurnCoordinator>,
    ) -> Self {
        Self {
            thread_service,
            turn_coordinator,
            event_stream: None,
            lifecycle_facade: Arc::new(UnsupportedLifecycleProductFacade::new_static(
                "reborn_lifecycle_facade_unwired",
            )),
            automation_facade: Arc::new(UnsupportedAutomationProductFacade::new_static()),
            connectable_channels_facade: Arc::new(StaticConnectableChannelsProductFacade::default()),
            outbound_preferences_facade: Arc::new(
                UnsupportedOutboundPreferencesProductFacade::new_static(),
            ),
            skills_facade: Arc::new(UnsupportedSkillsProductFacade),
            approval_interactions: Arc::new(RejectingApprovalInteractionService),
            auth_interactions: Arc::new(RejectingAuthInteractionService),
            extension_credentials: None,
            skill_activation_recorder: None,
            skill_activation_clearer: None,
            llm_config: None,
            thread_operation_locks: Arc::new(StdMutex::new(HashMap::new())),
            extension_registry: None,
            capability_permission_store: None,
            safety_config_store: None,
            token_settings_store: None,
            reduction_rule_store: None,
            reduction_rules_cache_invalidator: None,
            live_context_budget_setter: None,
            live_max_output_setter: None,
            live_total_input_setter: None,
            live_inline_control_setter: None,
            recipe_store: None,
        }
    }

    pub fn with_event_stream(mut self, event_stream: Arc<dyn ProjectionStream>) -> Self {
        self.event_stream = Some(event_stream);
        self
    }

    pub fn with_llm_config_service(mut self, llm_config: Arc<dyn LlmConfigService>) -> Self {
        self.llm_config = Some(llm_config);
        self
    }

    pub fn with_lifecycle_product_facade(
        mut self,
        lifecycle_facade: Arc<dyn LifecycleProductFacade>,
    ) -> Self {
        self.lifecycle_facade = lifecycle_facade;
        self
    }

    pub fn with_automation_product_facade(
        mut self,
        automation_facade: Arc<dyn AutomationProductFacade>,
    ) -> Self {
        self.automation_facade = automation_facade;
        self
    }

    pub fn with_connectable_channels_facade(
        mut self,
        connectable_channels_facade: Arc<dyn ConnectableChannelsProductFacade>,
    ) -> Self {
        self.connectable_channels_facade = connectable_channels_facade;
        self
    }

    pub fn with_outbound_preferences_facade(
        mut self,
        outbound_preferences_facade: Arc<dyn OutboundPreferencesProductFacade>,
    ) -> Self {
        self.outbound_preferences_facade = outbound_preferences_facade;
        self
    }

    pub fn with_skills_facade(mut self, skills_facade: Arc<dyn SkillsProductFacade>) -> Self {
        self.skills_facade = skills_facade;
        self
    }

    pub fn with_approval_interactions(
        mut self,
        approval_interactions: Arc<dyn ApprovalInteractionService>,
    ) -> Self {
        self.approval_interactions = approval_interactions;
        self
    }

    pub fn with_auth_interactions(
        mut self,
        auth_interactions: Arc<dyn AuthInteractionService>,
    ) -> Self {
        self.auth_interactions = auth_interactions;
        self
    }

    pub fn with_extension_credentials(
        mut self,
        extension_credentials: Arc<dyn ExtensionCredentialSetupService>,
    ) -> Self {
        self.extension_credentials = Some(extension_credentials);
        self
    }

    pub fn with_skill_activation_recorder<F>(mut self, recorder: F) -> Self
    where
        F: Fn(&TurnScope, &AcceptedMessageRef, &str) -> Result<(), RebornServicesError>
            + Send
            + Sync
            + 'static,
    {
        self.skill_activation_recorder = Some(Arc::new(recorder));
        self
    }

    pub fn with_skill_activation_hooks<R, C>(mut self, recorder: R, clearer: C) -> Self
    where
        R: Fn(&TurnScope, &AcceptedMessageRef, &str) -> Result<(), RebornServicesError>
            + Send
            + Sync
            + 'static,
        C: Fn(&TurnScope, &AcceptedMessageRef) -> Result<(), RebornServicesError>
            + Send
            + Sync
            + 'static,
    {
        self.skill_activation_recorder = Some(Arc::new(recorder));
        self.skill_activation_clearer = Some(Arc::new(clearer));
        self
    }

    /// Attach the extension registry for listing available capabilities.
    pub fn with_extension_registry(
        mut self,
        extension_registry: Arc<dyn brassclaw_host_api::CapabilityRegistry>,
    ) -> Self {
        self.extension_registry = Some(extension_registry);
        self
    }

    /// Attach the capability permission store for managing permission overrides.
    pub fn with_capability_permission_store(
        mut self,
        capability_permission_store: Arc<dyn CapabilityPermissionStore>,
    ) -> Self {
        self.capability_permission_store = Some(capability_permission_store);
        self
    }

    /// Attach the safety configuration store for managing safety rules.
    pub fn with_safety_config_store(
        mut self,
        safety_config_store: Arc<dyn crate::safety_config_store::SafetyConfigStore>,
    ) -> Self {
        self.safety_config_store = Some(safety_config_store);
        self
    }

    /// Attach the token settings store for reading/writing token limits.
    pub fn with_token_settings_store(
        mut self,
        token_settings_store: Arc<dyn crate::token_settings_store::TokenSettingsStore>,
    ) -> Self {
        self.token_settings_store = Some(token_settings_store);
        self
    }

    /// Attach a callback that updates the live context-token budget in the
    /// running `DefaultContextStrategy`.  Call this with the new
    /// `conversation_history` value (or `None` to revert to compiled default)
    /// after any successful `update_provider_token_settings`.
    pub fn with_live_context_budget_setter(
        mut self,
        setter: Arc<dyn Fn(Option<usize>) + Send + Sync>,
    ) -> Self {
        self.live_context_budget_setter = Some(setter);
        self
    }

    /// Attach a callback that updates the live max-output-tokens slot in the
    /// running strategy.  Fired after a successful `update_provider_token_settings`.
    pub fn with_live_max_output_setter(
        mut self,
        setter: Arc<dyn Fn(Option<usize>) + Send + Sync>,
    ) -> Self {
        self.live_max_output_setter = Some(setter);
        self
    }

    /// Attach a callback that updates the live total-input-tokens slot in the
    /// running strategy.  Fired after a successful `update_provider_token_settings`.
    pub fn with_live_total_input_setter(
        mut self,
        setter: Arc<dyn Fn(Option<usize>) + Send + Sync>,
    ) -> Self {
        self.live_total_input_setter = Some(setter);
        self
    }

    /// Attach a callback that updates the live inline-control-tokens slot in
    /// the running strategy.  Fired after a successful `update_provider_token_settings`.
    pub fn with_live_inline_control_setter(
        mut self,
        setter: Arc<dyn Fn(Option<usize>) + Send + Sync>,
    ) -> Self {
        self.live_inline_control_setter = Some(setter);
        self
    }

    /// Wire the persistence port for `GET/PUT /api/webchat/v2/tokens/reduction-rules`.
    /// Composition builds this from the libSQL settings table; the trait
    /// method itself is keyed by `(user_id, project_id)` so per-project
    /// isolation matches the engine's `(project_id, user_id)` cache key.
    /// Without this setter the facade returns `501` to every reduction-rule
    /// request, so misconfigured deployments fail loud rather than quietly
    /// serving the empty default.
    pub fn with_reduction_rule_store(
        mut self,
        store: Arc<dyn crate::reduction_rules::ReductionRuleStore>,
    ) -> Self {
        self.reduction_rule_store = Some(store);
        self
    }

    /// Wire a cache-invalidation hook fired after every successful
    /// `replace_reduction_rules`. Composition points this at the engine's
    /// `invalidate_reduction_rules_cache(project_id, user_id)` so the
    /// orchestrator Python picks up the new rules on the very next
    /// over-budget turn without restarting the runtime.
    pub fn with_reduction_rules_cache_invalidator(
        mut self,
        invalidator: CacheInvalidatorFn,
    ) -> Self {
        self.reduction_rules_cache_invalidator = Some(invalidator);
        self
    }

    /// Wire the Recipe-Skill-Tool persistence port. Composition
    /// builds this from the libSQL `MemoryDoc` store; the trait
    /// methods (list/validate/reject/record-outcome) all default to
    /// `501` when this setter has not been called, so misconfigured
    /// deployments fail loud rather than silently serving the empty
    /// stub responses.
    pub fn with_recipe_store(
        mut self,
        store: Arc<dyn crate::recipes::RecipeStore>,
    ) -> Self {
        self.recipe_store = Some(store);
        self
    }

    fn record_skill_activation_message(
        &self,
        scope: &TurnScope,
        accepted_message_ref: &AcceptedMessageRef,
        content: &str,
    ) -> Result<(), RebornServicesError> {
        if let Some(recorder) = &self.skill_activation_recorder {
            recorder(scope, accepted_message_ref, content)?;
        }
        Ok(())
    }

    fn clear_skill_activation_message(
        &self,
        scope: &TurnScope,
        accepted_message_ref: &AcceptedMessageRef,
    ) -> Result<(), RebornServicesError> {
        if let Some(clearer) = &self.skill_activation_clearer {
            clearer(scope, accepted_message_ref)?;
        }
        Ok(())
    }
}

#[async_trait]
impl RebornServicesApi for RebornServices {
    /// `requested_thread_id` makes the caller's choice authoritative.
    /// Without it, `client_action_id` deterministically derives the thread id
    /// so a retry of the same create maps back to the same thread.
    ///
    /// When the caller supplies an explicit `requested_thread_id`, an
    /// `ensure_thread` collision with a thread owned by another user is
    /// remapped to `NotFound` rather than the underlying `409 Conflict`.
    /// Otherwise the 400/409 distinction would be an existence oracle:
    /// callers sharing the same (tenant, agent, project) scope could probe
    /// for thread ids they did not create.
    async fn create_thread(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiCreateThreadRequest,
    ) -> Result<RebornCreateThreadResponse, RebornServicesError> {
        let command = request.into_command(caller)?;
        let WebUiInboundCommand::CreateThread {
            caller,
            client_action_id,
            requested_thread_id,
        } = command
        else {
            return Err(RebornServicesError::internal_invariant());
        };
        let caller_supplied_id = requested_thread_id.is_some();
        let thread_id =
            requested_thread_id.unwrap_or_else(|| generated_thread_id(&caller, &client_action_id));
        let scope = caller.turn_scope(thread_id.clone());
        let thread_scope = thread_scope_from_turn_scope(&scope, Some(caller.user_id.clone()))?;
        let thread = self
            .thread_service
            .ensure_thread(EnsureThreadRequest {
                scope: thread_scope,
                thread_id: Some(thread_id),
                created_by_actor_id: caller.user_id.as_str().to_string(),
                title: None,
                metadata_json: Some(create_thread_metadata_json(&client_action_id)?),
            })
            .await
            .map_err(|error| {
                if caller_supplied_id {
                    map_ownership_probe_error(error)
                } else {
                    // Deterministic generated ids derive from caller scope so
                    // a cross-user collision implies a UUIDv5 hash collision,
                    // which is not an oracle the caller can usefully probe.
                    // Preserve the underlying mapping for diagnosability.
                    map_thread_error(error)
                }
            })?;
        Ok(RebornCreateThreadResponse { thread })
    }

    async fn submit_turn(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiSendMessageRequest,
    ) -> Result<RebornSubmitTurnResponse, RebornServicesError> {
        let command = request.into_command(caller)?;
        let WebUiInboundCommand::SendMessage {
            scope,
            actor,
            client_action_id,
            content,
        } = command
        else {
            return Err(RebornServicesError::internal_invariant());
        };

        let (scope, thread_scope) = self.resolve_webui_thread_metadata(scope, &actor).await?;
        let _thread_operation_guard = self.lock_thread_operation(&scope).await;
        let source_binding_id = webui_source_binding_id(&scope, &actor);
        let external_event_id = client_action_id.as_str().to_string();

        let handoff = if let Some((replay, replay_source_binding_id)) = replay_webui_send_message(
            &*self.thread_service,
            &thread_scope,
            &scope,
            &actor,
            &external_event_id,
        )
        .await?
        {
            if replay.thread_id != scope.thread_id {
                return Err(RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Conflict,
                    RebornServicesErrorKind::Duplicate,
                    409,
                    false,
                ));
            }
            match replay.status {
                MessageStatus::Submitted => {
                    let run_id = parse_replay_run_id(replay.turn_run_id)?;
                    let state = self
                        .turn_coordinator
                        .get_run_state(GetRunStateRequest {
                            scope: scope.clone(),
                            run_id,
                        })
                        .await
                        .map_err(map_turn_error)?;
                    return Ok(RebornSubmitTurnResponse::AlreadySubmitted {
                        thread_id: replay.thread_id,
                        accepted_message_ref: accepted_message_ref(replay.message_id.to_string())?,
                        run_id,
                        status: state.status,
                        event_cursor: state.event_cursor,
                    });
                }
                MessageStatus::Accepted | MessageStatus::DeferredBusy => AcceptedWebUiMessage {
                    thread_id: replay.thread_id,
                    message_id: replay.message_id,
                    actor_id: actor.user_id.as_str().to_string(),
                    source_binding_id: replay
                        .source_binding_id
                        .unwrap_or_else(|| replay_source_binding_id.clone()),
                    reply_target_binding_id: replay
                        .reply_target_binding_id
                        .unwrap_or(replay_source_binding_id),
                },
                _ => {
                    return Err(RebornServicesError::from_status(
                        RebornServicesErrorCode::Conflict,
                        409,
                        false,
                    ));
                }
            }
        } else {
            let accepted = self
                .thread_service
                .accept_inbound_message(AcceptInboundMessageRequest {
                    scope: thread_scope.clone(),
                    thread_id: scope.thread_id.clone(),
                    actor_id: actor.user_id.as_str().to_string(),
                    source_binding_id: Some(source_binding_id.clone()),
                    reply_target_binding_id: Some(source_binding_id.clone()),
                    external_event_id: Some(external_event_id),
                    content: MessageContent::text(content.clone()),
                })
                .await
                .map_err(map_thread_error)?;
            AcceptedWebUiMessage {
                thread_id: accepted.thread_id,
                message_id: accepted.message_id,
                actor_id: actor.user_id.as_str().to_string(),
                source_binding_id: source_binding_id.clone(),
                reply_target_binding_id: source_binding_id.clone(),
            }
        };

        let accepted_message_ref = accepted_message_ref(handoff.message_id.to_string())?;
        let source_binding_ref =
            webui_source_binding_ref_from_raw("webui-src", &handoff.source_binding_id)?;
        let reply_target_binding_ref = webui_reply_target_binding_ref_from_raw(
            "webui-reply",
            &handoff.reply_target_binding_id,
        )?;
        let submit = SubmitTurnRequest {
            scope: scope.clone(),
            actor,
            accepted_message_ref: accepted_message_ref.clone(),
            source_binding_ref,
            reply_target_binding_ref,
            requested_run_profile: None,
            idempotency_key: client_action_id.clone(),
            received_at: Utc::now(),
            requested_run_id: None,
            parent_run_id: None,
            subagent_depth: 0,
            spawn_tree_root_run_id: None,
        };

        self.record_skill_activation_message(&scope, &accepted_message_ref, &content)?;
        match self.turn_coordinator.submit_turn(submit).await {
            Ok(SubmitTurnResponse::Accepted {
                turn_id,
                run_id,
                status,
                resolved_run_profile_id,
                resolved_run_profile_version,
                event_cursor,
                ..
            }) => {
                mark_message_submitted_or_replay(
                    &*self.thread_service,
                    &thread_scope,
                    &handoff,
                    &client_action_id,
                    turn_id.to_string(),
                    run_id.to_string(),
                )
                .await?;

                Ok(RebornSubmitTurnResponse::Submitted {
                    thread_id: handoff.thread_id,
                    accepted_message_ref,
                    turn_id: turn_id.to_string(),
                    run_id,
                    status,
                    resolved_run_profile_id: resolved_run_profile_id.as_str().to_string(),
                    resolved_run_profile_version: resolved_run_profile_version.as_u64(),
                    event_cursor,
                })
            }
            Err(TurnError::ThreadBusy(busy)) => {
                self.clear_skill_activation_message(&scope, &accepted_message_ref)?;
                mark_message_deferred_busy_or_replay(
                    &*self.thread_service,
                    &thread_scope,
                    &handoff,
                    &client_action_id,
                )
                .await?;

                Ok(RebornSubmitTurnResponse::DeferredBusy {
                    thread_id: handoff.thread_id,
                    accepted_message_ref,
                    active_run_id: busy.active_run_id,
                    status: busy.status,
                    event_cursor: busy.event_cursor,
                })
            }
            Err(error) => {
                self.clear_skill_activation_message(&scope, &accepted_message_ref)?;
                Err(map_turn_error(error))
            }
        }
    }

    async fn delete_thread(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornDeleteThreadRequest,
    ) -> Result<RebornDeleteThreadResponse, RebornServicesError> {
        let thread_id = parse_thread_id_field("thread_id", request.thread_id)?;
        let scope = caller.turn_scope(thread_id.clone());
        let thread_scope = thread_scope_from_turn_scope(&scope, Some(caller.user_id.clone()))?;
        let _thread_operation_guard = self.lock_thread_operation(&scope).await;
        self.reject_delete_with_active_run(&scope, &thread_scope, &thread_id)
            .await?;
        self.thread_service
            .delete_thread(&thread_scope, &thread_id)
            .await
            .map_err(map_ownership_probe_error)?;
        Ok(RebornDeleteThreadResponse {
            thread_id,
            deleted: true,
        })
    }

    async fn get_timeline(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornTimelineRequest,
    ) -> Result<RebornTimelineResponse, RebornServicesError> {
        let thread_id = parse_thread_id_field("thread_id", request.thread_id)?;
        let actor = caller.actor();
        let limit = clamp_timeline_limit(request.limit);
        let cursor = parse_timeline_cursor(request.cursor.as_deref())?;
        let scope = caller.turn_scope(thread_id);
        let thread_scope = thread_scope_from_turn_scope(&scope, Some(actor.user_id.clone()))?;
        let history = self
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: thread_scope,
                thread_id: scope.thread_id.clone(),
            })
            .await
            .map_err(map_timeline_probe_error)?;

        let (messages, next_cursor) = paginate_timeline_messages(history.messages, limit, cursor);
        let summary_artifacts = cap_summary_artifacts(history.summary_artifacts);

        Ok(RebornTimelineResponse {
            thread: history.thread,
            messages,
            summary_artifacts,
            next_cursor,
        })
    }

    async fn stream_events(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornStreamEventsRequest,
    ) -> Result<RebornStreamEventsResponse, RebornServicesError> {
        let thread_id = parse_thread_id_field("thread_id", request.thread_id)?;
        let actor = caller.actor();
        // Metadata-only ownership probe: the SSE handler calls
        // stream_events once per poll, and using list_thread_history here
        // would load the full message transcript + summary artifacts per
        // call — for an active stream that is hundreds of rows per second
        // per caller. resolve_webui_thread_metadata uses the cheap
        // read_thread probe; without it a caller sharing
        // (tenant, agent, project) could still read another user's
        // projection feed by guessing thread_id, so the probe itself
        // stays.
        let (scope, _thread_scope) = self
            .resolve_webui_thread_metadata(caller.turn_scope(thread_id), &actor)
            .await?;
        let Some(event_stream) = &self.event_stream else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ReplayUnavailable,
                503,
                false,
            ));
        };
        let events = event_stream
            .drain(ProjectionSubscriptionRequest {
                actor,
                scope,
                after_cursor: request.after_cursor,
            })
            .await
            .map_err(map_projection_error)?;
        Ok(RebornStreamEventsResponse { events })
    }

    async fn cancel_run(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiCancelRunRequest,
    ) -> Result<RebornCancelRunResponse, RebornServicesError> {
        let command = request.into_command(caller)?;
        let WebUiInboundCommand::CancelRun { request } = command else {
            return Err(RebornServicesError::internal_invariant());
        };
        // Metadata-only ownership probe — cancel_run has no use for the
        // message transcript and the load would be wasted work.
        self.resolve_webui_thread_metadata(request.scope.clone(), &request.actor)
            .await?;
        let response = self
            .turn_coordinator
            .cancel_run(request)
            .await
            .map_err(map_turn_error)?;
        Ok(response.into())
    }

    async fn resolve_gate(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiResolveGateRequest,
    ) -> Result<RebornResolveGateResponse, RebornServicesError> {
        let command = request.into_command(caller)?;
        let WebUiInboundCommand::ResolveGate {
            scope,
            actor,
            run_id,
            gate_ref,
            client_action_id,
            resolution,
        } = command
        else {
            return Err(RebornServicesError::internal_invariant());
        };

        // Metadata-only ownership probe — resolve_gate has no use for
        // the message transcript and the load would be wasted work.
        self.resolve_webui_thread_metadata(scope.clone(), &actor)
            .await?;
        match self
            .gate_resolution_route(&scope, &actor, run_id, &gate_ref, &resolution)
            .await?
        {
            GateResolutionRoute::Approval => {
                self.resolve_approval_gate(
                    scope,
                    actor,
                    run_id,
                    gate_ref,
                    client_action_id,
                    resolution,
                )
                .await
            }
            GateResolutionRoute::Auth => {
                self.resolve_auth_gate(scope, actor, run_id, gate_ref, client_action_id, resolution)
                    .await
            }
            GateResolutionRoute::Generic => {
                self.resolve_generic_gate(
                    scope,
                    actor,
                    run_id,
                    gate_ref,
                    client_action_id,
                    resolution,
                )
                .await
            }
        }
    }

    async fn get_run_state(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornGetRunStateRequest,
    ) -> Result<RebornGetRunStateResponse, RebornServicesError> {
        let thread_id = parse_thread_id_field("thread_id", request.thread_id)?;
        let run_id = parse_run_id_field("run_id", request.run_id)?;
        let scope = caller.turn_scope(thread_id);
        let actor = caller.actor();
        // TurnScope has no owner_user_id, so without this gate any caller
        // sharing the (tenant, agent, project) scope could read another user's
        // run state by guessing thread_id and run_id. Mirrors the ownership
        // probe `cancel_run` and `resolve_gate` already perform.
        // Metadata-only — get_run_state has no use for the transcript.
        self.resolve_webui_thread_metadata(scope.clone(), &actor)
            .await?;
        let state = self
            .turn_coordinator
            .get_run_state(GetRunStateRequest { scope, run_id })
            .await
            .map_err(map_turn_error)?;
        Ok(state.into())
    }

    async fn list_threads(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiListThreadsRequest,
    ) -> Result<RebornListThreadsResponse, RebornServicesError> {
        // Reuse the same scope-construction shape the other v2 facade
        // methods use: fail-closed when the caller has no agent
        // binding, owner-scope to the caller's user_id so the listing
        // is per-caller.
        let Some(agent_id) = caller.agent_id.clone() else {
            return Err(RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            ));
        };
        let scope = ThreadScope {
            tenant_id: caller.tenant_id.clone(),
            agent_id,
            project_id: caller.project_id.clone(),
            owner_user_id: Some(caller.user_id.clone()),
            mission_id: None,
        };
        let response = self
            .thread_service
            .list_threads_for_scope(brassclaw_threads::ListThreadsForScopeRequest {
                scope,
                limit: request.limit,
                cursor: request.cursor,
            })
            .await
            .map_err(map_thread_error)?;
        Ok(RebornListThreadsResponse {
            threads: response.threads,
            next_cursor: response.next_cursor,
        })
    }

    async fn list_automations(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: WebUiListAutomationsRequest,
    ) -> Result<RebornListAutomationsResponse, RebornServicesError> {
        let Some(caller) = product_agent_bound_caller_from_webui(caller) else {
            return Err(RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            ));
        };
        let limit = clamp_automation_list_limit(request.limit);
        let automations = self
            .automation_facade
            .list_automations(caller, limit)
            .await?;
        Ok(RebornListAutomationsResponse { automations })
    }

    async fn list_connectable_channels(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornConnectableChannelListResponse, RebornServicesError> {
        self.connectable_channels_facade
            .list_connectable_channels(caller)
            .await
    }

    async fn get_outbound_preferences(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError> {
        self.outbound_preferences_facade
            .get_outbound_preferences(caller)
            .await
    }

    async fn set_outbound_preferences(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornSetOutboundPreferencesRequest,
    ) -> Result<RebornOutboundPreferencesResponse, RebornServicesError> {
        self.outbound_preferences_facade
            .set_outbound_preferences(caller, request)
            .await
    }

    async fn list_outbound_delivery_targets(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornOutboundDeliveryTargetListResponse, RebornServicesError> {
        self.outbound_preferences_facade
            .list_outbound_delivery_targets(caller)
            .await
    }

    async fn list_extensions(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornExtensionListResponse, RebornServicesError> {
        extensions::list_extensions(self.lifecycle_facade.as_ref(), caller).await
    }

    async fn list_extension_registry(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornExtensionRegistryResponse, RebornServicesError> {
        extensions::list_extension_registry(self.lifecycle_facade.as_ref(), caller).await
    }

    async fn install_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
    ) -> Result<RebornExtensionActionResponse, RebornServicesError> {
        extensions::install_extension(self.lifecycle_facade.as_ref(), caller, package_ref).await
    }

    async fn activate_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
    ) -> Result<RebornExtensionActionResponse, RebornServicesError> {
        extensions::activate_extension(self.lifecycle_facade.as_ref(), caller, package_ref).await
    }

    async fn remove_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
    ) -> Result<RebornExtensionActionResponse, RebornServicesError> {
        extensions::remove_extension(self.lifecycle_facade.as_ref(), caller, package_ref).await
    }

    async fn setup_extension(
        &self,
        caller: WebUiAuthenticatedCaller,
        package_ref: LifecyclePackageRef,
        request: WebUiSetupExtensionRequest,
    ) -> Result<RebornSetupExtensionResponse, RebornServicesError> {
        lifecycle_setup::setup_extension(
            self.lifecycle_facade.as_ref(),
            self.extension_credentials.as_deref(),
            caller,
            package_ref,
            request,
        )
        .await
    }

    async fn get_llm_config(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .snapshot(caller)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn upsert_llm_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: UpsertLlmProviderRequest,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .upsert_provider(caller, request)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn delete_llm_provider(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: String,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .delete_provider(caller, provider_id)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn set_active_llm(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: SetActiveLlmRequest,
    ) -> Result<LlmConfigSnapshot, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .set_active(caller, request)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn test_llm_connection(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmProbeResult, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .test_connection(caller, request)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn list_llm_models(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: LlmProbeRequest,
    ) -> Result<LlmModelsResult, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .list_models(caller, request)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn start_nearai_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiLoginRequest,
    ) -> Result<NearAiLoginStart, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .start_nearai_login(caller, request)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn start_codex_login(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<CodexLoginStart, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .start_codex_login(caller)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn complete_nearai_wallet_login(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: NearAiWalletLoginRequest,
    ) -> Result<NearAiWalletLoginResult, RebornServicesError> {
        let service = self
            .llm_config
            .as_ref()
            .ok_or_else(llm_config::llm_config_unavailable)?;
        service
            .complete_nearai_wallet_login(caller, request)
            .await
            .map_err(llm_config::map_llm_config_error)
    }

    async fn list_capabilities(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornListCapabilitiesResponse, RebornServicesError> {
        use brassclaw_host_api::PermissionMode;

        // If extension registry is not wired, return empty list
        let Some(registry) = &self.extension_registry else {
            return Ok(RebornListCapabilitiesResponse {
                capabilities: Vec::new(),
            });
        };

        let tenant_id = caller.tenant_id.to_string();

        // Load permission overrides from the database if available
        let permission_overrides = if let Some(store) = &self.capability_permission_store {
            store
                .list_capability_overrides(&tenant_id)
                .await
                .map_err(|_e| {
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Internal,
                        RebornServicesErrorKind::Internal,
                        500,
                        false,
                    )
                })?
        } else {
            std::collections::HashMap::new()
        };

        // Build capability info list from registry
        let mut capabilities = Vec::new();
        for descriptor in registry.capabilities() {
            let default_permission = match descriptor.default_permission {
                PermissionMode::Allow => "allow",
                PermissionMode::Ask => "ask",
                PermissionMode::Deny => "deny",
            };

            let permission_mode = permission_overrides
                .get(&descriptor.id.to_string())
                .map(|mode| match mode {
                    PermissionMode::Allow => "allow",
                    PermissionMode::Ask => "ask",
                    PermissionMode::Deny => "deny",
                })
                .unwrap_or(default_permission);

            capabilities.push(RebornCapabilityInfo {
                id: descriptor.id.to_string(),
                description: descriptor.description.clone(),
                provider: descriptor.provider.to_string(),
                effects: descriptor
                    .effects
                    .iter()
                    .map(|e| format!("{:?}", e))
                    .collect(),
                permission_mode: permission_mode.to_string(),
                default_permission: default_permission.to_string(),
            });
        }

        Ok(RebornListCapabilitiesResponse { capabilities })
    }

    async fn update_capability_permission(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: RebornUpdateCapabilityPermissionRequest,
    ) -> Result<RebornUpdateCapabilityPermissionResponse, RebornServicesError> {
        use brassclaw_host_api::PermissionMode;

        // Require capability permission store to be wired
        let Some(store) = &self.capability_permission_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };

        // Validate and parse permission mode
        let permission_mode = match request.permission_mode.as_str() {
            "allow" => PermissionMode::Allow,
            "ask" => PermissionMode::Ask,
            "deny" => PermissionMode::Deny,
            _ => {
                return Err(RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::InvalidRequest,
                    RebornServicesErrorKind::Validation,
                    400,
                    false,
                ));
            }
        };

        let tenant_id = caller.tenant_id.to_string();
        let capability_id = request.capability_id.clone();

        // Verify capability exists in registry if available
        if let Some(registry) = &self.extension_registry {
            let cap_id = brassclaw_host_api::CapabilityId::new(&capability_id).map_err(|_| {
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::InvalidRequest,
                    RebornServicesErrorKind::Validation,
                    400,
                    false,
                )
            })?;

            if registry.get_capability(&cap_id).is_none() {
                return Err(RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::NotFound,
                    RebornServicesErrorKind::NotFound,
                    404,
                    false,
                ));
            }
        }

        // Store the permission override
        store
            .set_capability_permission(&tenant_id, &capability_id, permission_mode)
            .await
            .map_err(|_e| {
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })?;

        Ok(RebornUpdateCapabilityPermissionResponse {
            capability_id,
            permission_mode: request.permission_mode,
            updated: true,
        })
    }

    async fn list_skills(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<RebornListSkillsResponse, RebornServicesError> {
        self.skills_facade.list_skills(&caller).await
    }

    async fn install_skill(
        &self,
        caller: WebUiAuthenticatedCaller,
        content: String,
        source_url: Option<String>,
    ) -> Result<RebornSkillInstallResult, RebornServicesError> {
        self.skills_facade
            .install_skill(&caller, content, source_url)
            .await
    }

    async fn remove_skill(
        &self,
        caller: WebUiAuthenticatedCaller,
        name: String,
    ) -> Result<RebornSkillRemoveResult, RebornServicesError> {
        self.skills_facade.remove_skill(&caller, &name).await
    }

    async fn get_safety_sensitive_paths(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        let Some(store) = &self.safety_config_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };

        let user_id = caller.user_id.to_string();
        store
            .get_config(
                &user_id,
                crate::safety_config_store::SafetyCategory::SensitivePaths,
            )
            .await
            .map_err(|e| {
                tracing::error!("❌ Failed to get safety config: {:?}", e);
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })
    }

    async fn update_safety_sensitive_paths(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: crate::safety_config::UpdateSafetyConfigRequest,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        let Some(store) = &self.safety_config_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };

        let user_id = caller.user_id.to_string();
        store
            .update_config(
                &user_id,
                crate::safety_config_store::SafetyCategory::SensitivePaths,
                request.entries,
            )
            .await
            .map_err(|_e| {
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })
    }

    async fn get_safety_workspace_rules(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        let Some(store) = &self.safety_config_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };

        let user_id = caller.user_id.to_string();
        store
            .get_config(
                &user_id,
                crate::safety_config_store::SafetyCategory::WorkspaceRules,
            )
            .await
            .map_err(|_e| {
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })
    }

    async fn update_safety_workspace_rules(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: crate::safety_config::UpdateSafetyConfigRequest,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        let Some(store) = &self.safety_config_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };

        let user_id = caller.user_id.to_string();
        store
            .update_config(
                &user_id,
                crate::safety_config_store::SafetyCategory::WorkspaceRules,
                request.entries,
            )
            .await
            .map_err(|_e| {
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })
    }

    async fn get_safety_blocked_paths(
        &self,
        caller: WebUiAuthenticatedCaller,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        let Some(store) = &self.safety_config_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };

        let user_id = caller.user_id.to_string();
        store
            .get_config(
                &user_id,
                crate::safety_config_store::SafetyCategory::BlockedPaths,
            )
            .await
            .map_err(|_e| {
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })
    }

    async fn update_safety_blocked_paths(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: crate::safety_config::UpdateSafetyConfigRequest,
    ) -> Result<crate::safety_config::SafetyConfigResponse, RebornServicesError> {
        let Some(store) = &self.safety_config_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };

        let user_id = caller.user_id.to_string();
        store
            .update_config(
                &user_id,
                crate::safety_config_store::SafetyCategory::BlockedPaths,
                request.entries,
            )
            .await
            .map_err(|_e| {
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })
    }

    async fn get_provider_token_settings(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: &str,
    ) -> Result<crate::token_settings::TokenSettingsResponse, RebornServicesError> {
        let Some(store) = &self.token_settings_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };
        // Validate provider_id at the service boundary (same rule as UpsertLlmProvider).
        if !provider_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
            || provider_id.is_empty()
            || provider_id.len() > 64
        {
            return Err(RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            ));
        }
        let user_id = caller.user_id.to_string();
        store
            .get_provider_token_settings(&user_id, provider_id)
            .await
            .map_err(|e| {
                tracing::error!("❌ Failed to get provider token settings: {:?}", e);
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })
    }

    async fn update_provider_token_settings(
        &self,
        caller: WebUiAuthenticatedCaller,
        provider_id: &str,
        request: crate::token_settings::UpdateTokenSettingsRequest,
    ) -> Result<crate::token_settings::TokenSettingsResponse, RebornServicesError> {
        let Some(store) = &self.token_settings_store else {
            return Err(RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            ));
        };
        // Validate provider_id at the service boundary (same rule as UpsertLlmProvider).
        if !provider_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
            || provider_id.is_empty()
            || provider_id.len() > 64
        {
            return Err(RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            ));
        }
        let user_id = caller.user_id.to_string();
        let response = store
            .update_provider_token_settings(&user_id, provider_id, request)
            .await
            .map_err(|e| {
                tracing::error!("❌ Failed to update provider token settings: {:?}", e);
                RebornServicesError::from_status_kind(
                    RebornServicesErrorCode::Internal,
                    RebornServicesErrorKind::Internal,
                    500,
                    false,
                )
            })?;
        // Propagate all updated budget values to live strategy slots so changes
        // take effect on the very next turn — no restart required.
        if let Some(setter) = &self.live_context_budget_setter {
            setter(response.conversation_history);
        }
        if let Some(setter) = &self.live_max_output_setter {
            setter(response.max_output);
        }
        if let Some(setter) = &self.live_total_input_setter {
            setter(response.total_input);
        }
        if let Some(setter) = &self.live_inline_control_setter {
            setter(response.inline_control);
        }
        Ok(response)
    }

    async fn list_reduction_rules(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
    ) -> Result<crate::reduction_rules::ReductionRulesResponse, RebornServicesError> {
        let store = self.reduction_rule_store.as_ref().ok_or_else(|| {
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            )
        })?;
        let user_id = caller.user_id.to_string();
        let mut rules = store
            .list(&user_id, project_id)
            .await
            .map_err(|error| match error {
                crate::reduction_rules::ReductionRuleStoreError::Invalid(_) => {
                    RebornServicesError::from_status(
                        RebornServicesErrorCode::InvalidRequest,
                        400,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Unavailable(_) => {
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Unavailable,
                        RebornServicesErrorKind::ServiceUnavailable,
                        503,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Internal(_) => {
                    tracing::error!("❌ Failed to list reduction rules: {:?}", error);
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Internal,
                        RebornServicesErrorKind::Internal,
                        500,
                        false,
                    )
                }
            })?;
        // Read-side guarantee: serve sorted so the WebUI never observes a
        // list out of order with respect to its own subsequent PUT.
        crate::reduction_rules::sort_for_storage(&mut rules);
        Ok(crate::reduction_rules::ReductionRulesResponse {
            project_id: project_id.to_string(),
            rules,
        })
    }

    async fn replace_reduction_rules(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        request: crate::reduction_rules::ReductionRulesRequest,
    ) -> Result<crate::reduction_rules::ReductionRulesResponse, RebornServicesError> {
        let store = self.reduction_rule_store.as_ref().ok_or_else(|| {
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            )
        })?;
        let user_id = caller.user_id.to_string();
        // Validate every rule before any write happens so a single broken
        // entry cannot leave the store with a partial replacement. The
        // typed-view shape makes `field`, `max_chars`, etc. type-checked
        // rather than relying on the storage layer to catch typos.
        if request.rules.len() > crate::reduction_rules::REDUCTION_RULES_MAX_PER_USER {
            return Err(RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            ));
        }
        let mut validated = Vec::with_capacity(request.rules.len());
        for rule in request.rules {
            rule.validate().map_err(|source| {
                tracing::warn!(
                    "reduction rule payload rejected: id={:?} code={:?} message={}",
                    rule.id,
                    source,
                    source
                );
                RebornServicesError::from_status(
                    RebornServicesErrorCode::InvalidRequest,
                    400,
                    false,
                )
            })?;
            validated.push(rule);
        }
        crate::reduction_rules::sort_for_storage(&mut validated);
        // Detect duplicate ids — the orchestrator Python would otherwise
        // apply whichever copy comes later in the ordered list, masking
        // the operator's intent; reject at write time.
        {
            let mut i = 0;
            while i < validated.len() {
                let id = validated[i].id.clone();
                let mut j = i + 1;
                while j < validated.len() {
                    if validated[j].id == id {
                        return Err(RebornServicesError::from_status(
                            RebornServicesErrorCode::InvalidRequest,
                            400,
                            false,
                        ));
                    }
                    j += 1;
                }
                i += 1;
            }
        }
        let rules = store
            .replace(&user_id, project_id, validated)
            .await
            .map_err(|error| match error {
                crate::reduction_rules::ReductionRuleStoreError::Invalid(_) => {
                    RebornServicesError::from_status(
                        RebornServicesErrorCode::InvalidRequest,
                        400,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Unavailable(_) => {
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Unavailable,
                        RebornServicesErrorKind::ServiceUnavailable,
                        503,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Internal(_) => {
                    tracing::error!("❌ Failed to replace reduction rules: {:?}", error);
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Internal,
                        RebornServicesErrorKind::Internal,
                        500,
                        false,
                    )
                }
            })?;
        // Cache invalidation runs AFTER the storage write succeeds. If the
        // invalidator panics or the channel is closed (e.g. composition
        // unwired the hook), the storage row is still authoritative and
        // will be reread on the next cache miss.
        if let Some(invalidator) = &self.reduction_rules_cache_invalidator {
            invalidator(project_id, &user_id);
        }
        Ok(crate::reduction_rules::ReductionRulesResponse {
            project_id: project_id.to_string(),
            rules,
        })
    }

    async fn author_reduction_rule(
        &self,
        caller: WebUiAuthenticatedCaller,
        request: crate::reduction_rules::AuthorReductionRuleRequest,
    ) -> Result<crate::reduction_rules::AuthorReductionRuleResponse, RebornServicesError> {
        // Author a new reduction rule from a structured request.
        //
        // No live LLM is invoked here today. A generic WebUI-side LLM-call
        // port is intentionally out of scope for v2 (the composition
        // facade does not yet expose one) — but the orchestrator Python
        // already runs the same Monty-safe reducer end-to-end on the very
        // next over-budget turn, so a misconfigured rule is *detected*
        // immediately by `_reduce_prompt` rather than by an LLM pre-flight
        // check. Authoring the rule and verifying it in production is
        // therefore the right division of labor here: validate the shape
        // now, run it through Monty later. The validation is the same one
        // that gates the bulk-replace path, so a rule that passes here is
        // indistinguishable from one the WebUI handed us via PUT.
        //
        // The returned rule is wired with a deterministic id of the form
        // `auto-{rule_type}-{nanos}` so a future audit pass can tell it
        // apart from operator-authored rules. Using monotonic nanoseconds
        // (not random IDs) keeps id uniqueness without introducing a UUID
        // dependency on a struct with a strict ASCII-format validator.
        //
        // The `project_id` is taken from the caller's authenticated
        // scope, NOT a request body field — that's a deliberate choice
        // to keep the author surface minimal and aligned with how the
        // list/replace endpoints already work. A request with no
        // caller-side project_id returns `400` rather than silently
        // writing into the local-default bucket.
        let store = self.reduction_rule_store.as_ref().ok_or_else(|| {
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            )
        })?;
        let project_id = caller.project_id.as_ref().ok_or_else(|| {
            tracing::warn!(
                "author_reduction_rule called without caller project_id: user={:?}",
                caller.user_id
            );
            RebornServicesError::from_status(RebornServicesErrorCode::InvalidRequest, 400, false)
        })?;
        let project_id_str = project_id.as_str();
        let user_id = caller.user_id.to_string();
        let id = format!(
            "auto-{}-{}",
            request.rule_type,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let rule = crate::reduction_rules::ReductionRuleConfigView {
            id,
            rule_type: request.rule_type,
            params: request.params,
            priority: crate::reduction_rules::sort_for_storage_default_priority(),
        };
        // The same validation path as the bulk `replace` endpoint runs
        // here; it's exhaustive on `RuleType`/params. If the rule fails
        // validation, no write occurs.
        rule.validate().map_err(|source| {
            tracing::warn!(
                "authored reduction rule rejected: id={:?} code={:?} message={}",
                rule.id,
                source,
                source
            );
            RebornServicesError::from_status(RebornServicesErrorCode::InvalidRequest, 400, false)
        })?;
        // Persist by re-reading the existing list, appending the new rule,
        // and writing back atomically. We deliberately don't try to peek
        // the store's internal ordering — the storage layer is responsible
        // for sort/idempotency under replace.
        let existing = store
            .list(&user_id, project_id_str)
            .await
            .map_err(|error| match error {
                crate::reduction_rules::ReductionRuleStoreError::Invalid(_) => {
                    RebornServicesError::from_status(
                        RebornServicesErrorCode::InvalidRequest,
                        400,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Unavailable(_) => {
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Unavailable,
                        RebornServicesErrorKind::ServiceUnavailable,
                        503,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Internal(_) => {
                    tracing::error!(
                        "❌ Failed to list reduction rules during author: {:?}",
                        error
                    );
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Internal,
                        RebornServicesErrorKind::Internal,
                        500,
                        false,
                    )
                }
            })?;
        let mut merged = existing;
        merged.push(rule.clone());
        if merged.len() > crate::reduction_rules::REDUCTION_RULES_MAX_PER_USER {
            return Err(RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            ));
        }
        let _stored = store
            .replace(&user_id, project_id_str, merged)
            .await
            .map_err(|error| match error {
                crate::reduction_rules::ReductionRuleStoreError::Invalid(_) => {
                    RebornServicesError::from_status(
                        RebornServicesErrorCode::InvalidRequest,
                        400,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Unavailable(_) => {
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Unavailable,
                        RebornServicesErrorKind::ServiceUnavailable,
                        503,
                        false,
                    )
                }
                crate::reduction_rules::ReductionRuleStoreError::Internal(_) => {
                    tracing::error!("❌ Failed to persist authored reduction rule: {:?}", error);
                    RebornServicesError::from_status_kind(
                        RebornServicesErrorCode::Internal,
                        RebornServicesErrorKind::Internal,
                        500,
                        false,
                    )
                }
            })?;
        if let Some(invalidator) = &self.reduction_rules_cache_invalidator {
            invalidator(project_id_str, &user_id);
        }
        Ok(crate::reduction_rules::AuthorReductionRuleResponse {
            rule,
            description: request.description,
        })
    }

    // ── Recipe-Skill-Tool library methods ────────────────────────
    //
    // Each method checks `recipe_store` first; with the setter
    // unwired, the trait defaults raise `501` so callers see the same
    // shape regardless of whether composition wired the
    // [libsql] MemoryDocStore. Validation status transitions route
    // through the same store; on `request_*_review` we accept the
    // feedback string the operator pastes so the review mission has
    // context for the LLM fix.

    async fn list_recipes(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
    ) -> Result<crate::recipes::RecipeListResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        let recipes = store
            .list_recipes(&user_id, project_id)
            .await
            .map_err(map_recipe_store_error)?;
        Ok(crate::recipes::RecipeListResponse { recipes })
    }

    async fn list_tool_skills(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
    ) -> Result<crate::recipes::ToolSkillListResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        let tool_skills = store
            .list_tool_skills(&user_id, project_id)
            .await
            .map_err(map_recipe_store_error)?;
        Ok(crate::recipes::ToolSkillListResponse { tool_skills })
    }

    async fn get_recipe(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        recipe_id: &str,
    ) -> Result<crate::recipes::RecipeDetail, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .get_recipe(&user_id, project_id, recipe_id)
            .await
            .map_err(map_recipe_store_error)?
            .ok_or_else(|| recipe_not_found("recipe", recipe_id))
    }

    async fn get_tool_skill(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        skill_id: &str,
    ) -> Result<crate::recipes::ToolSkillDetail, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .get_tool_skill(&user_id, project_id, skill_id)
            .await
            .map_err(map_recipe_store_error)?
            .ok_or_else(|| recipe_not_found("tool_skill", skill_id))
    }

    async fn list_validation_queue(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
    ) -> Result<crate::recipes::ValidationQueueListResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        let items = store
            .list_validation_queue(&user_id, project_id)
            .await
            .map_err(map_recipe_store_error)?;
        Ok(crate::recipes::ValidationQueueListResponse { items })
    }

    async fn count_validation_queue(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        status: &str,
    ) -> Result<crate::recipes::ValidationQueueCountResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        let count = store
            .count_by_status(&user_id, project_id, status)
            .await
            .map_err(map_recipe_store_error)?;
        Ok(crate::recipes::ValidationQueueCountResponse {
            count,
            status: status.to_string(),
        })
    }

    async fn validate_recipe(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        recipe_id: &str,
        request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .update_recipe_validation_status(
                &user_id,
                project_id,
                recipe_id,
                "validated",
                request.feedback.as_deref(),
            )
            .await
            .map_err(map_recipe_store_error)
    }

    async fn reject_recipe(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        recipe_id: &str,
        request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .update_recipe_validation_status(
                &user_id,
                project_id,
                recipe_id,
                "rejected",
                request.feedback.as_deref(),
            )
            .await
            .map_err(map_recipe_store_error)
    }

    async fn request_recipe_review(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        recipe_id: &str,
        request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        // Reviewer must attach feedback — the LLM review mission
        // uses it as the "why are you fixing this?" context prompt.
        let feedback = request.feedback.as_deref().ok_or_else(|| {
            RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            )
        })?;
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .update_recipe_validation_status(
                &user_id,
                project_id,
                recipe_id,
                "review_requested",
                Some(feedback),
            )
            .await
            .map_err(map_recipe_store_error)
    }

    async fn validate_tool_skill(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        skill_id: &str,
        request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .update_skill_validation_status(
                &user_id,
                project_id,
                skill_id,
                "validated",
                request.feedback.as_deref(),
            )
            .await
            .map_err(map_recipe_store_error)
    }

    async fn reject_tool_skill(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        skill_id: &str,
        request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .update_skill_validation_status(
                &user_id,
                project_id,
                skill_id,
                "rejected",
                request.feedback.as_deref(),
            )
            .await
            .map_err(map_recipe_store_error)
    }

    async fn request_tool_skill_review(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        skill_id: &str,
        request: crate::recipes::UpdateValidationStatusRequest,
    ) -> Result<crate::recipes::UpdateValidationStatusResponse, RebornServicesError> {
        let feedback = request.feedback.as_deref().ok_or_else(|| {
            RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            )
        })?;
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .update_skill_validation_status(
                &user_id,
                project_id,
                skill_id,
                "review_requested",
                Some(feedback),
            )
            .await
            .map_err(map_recipe_store_error)
    }

    async fn record_recipe_outcome(
        &self,
        caller: WebUiAuthenticatedCaller,
        project_id: &str,
        request: crate::recipes::RecordOutcomeRequest,
    ) -> Result<crate::recipes::RecordOutcomeResponse, RebornServicesError> {
        let store = self.recipe_store.as_ref().ok_or_else(recipe_store_unavailable)?;
        let user_id = caller.user_id.to_string();
        store
            .record_outcome(&user_id, project_id, request)
            .await
            .map_err(map_recipe_store_error)
    }
}

/// Default error mapping for [`crate::recipes::RecipeStoreError`] →
/// `RebornServicesError`. Mirrors the reduction-rule mapping at the top
/// of the file: `Invalid` / `NotFound` → 400, `Unavailable` → 503,
/// `Internal` → 500.
fn map_recipe_store_error(
    error: crate::recipes::RecipeStoreError,
) -> RebornServicesError {
    match error {
        crate::recipes::RecipeStoreError::Invalid(_) => {
            RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                400,
                false,
            )
        }
        crate::recipes::RecipeStoreError::NotFound(_) => {
            RebornServicesError::from_status(
                RebornServicesErrorCode::InvalidRequest,
                404,
                false,
            )
        }
        crate::recipes::RecipeStoreError::Unavailable(_) => {
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Unavailable,
                RebornServicesErrorKind::ServiceUnavailable,
                503,
                false,
            )
        }
        crate::recipes::RecipeStoreError::Internal(reason) => {
            tracing::error!("❌ Recipe store internal error: {reason}");
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Internal,
                RebornServicesErrorKind::Internal,
                500,
                false,
            )
        }
    }
}

fn recipe_store_unavailable() -> RebornServicesError {
    RebornServicesError::from_status_kind(
        RebornServicesErrorCode::Unavailable,
        RebornServicesErrorKind::ServiceUnavailable,
        503,
        false,
    )
}

fn recipe_not_found(kind: &str, id: &str) -> RebornServicesError {
    tracing::debug!("recipe/skill lookup miss: {kind} '{id}'");
    RebornServicesError::from_status(RebornServicesErrorCode::NotFound, 404, false)
}

impl RebornServices {
    fn thread_operation_lock(&self, scope: &TurnScope) -> Arc<AsyncMutex<()>> {
        let key = thread_operation_key(scope);
        let mut locks = match self.thread_operation_locks.lock() {
            Ok(locks) => locks,
            Err(poisoned) => poisoned.into_inner(),
        };
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(AsyncMutex::new(()));
        locks.insert(key, Arc::downgrade(&lock));
        lock
    }

    async fn lock_thread_operation(&self, scope: &TurnScope) -> OwnedMutexGuard<()> {
        self.thread_operation_lock(scope).lock_owned().await
    }

    async fn reject_delete_with_active_run(
        &self,
        scope: &TurnScope,
        thread_scope: &ThreadScope,
        thread_id: &ThreadId,
    ) -> Result<(), RebornServicesError> {
        let history = self
            .thread_service
            .list_thread_history(ThreadHistoryRequest {
                scope: thread_scope.clone(),
                thread_id: thread_id.clone(),
            })
            .await
            .map_err(map_timeline_probe_error)?;
        let mut seen = HashSet::new();
        for run_id in history
            .messages
            .iter()
            .filter_map(|message| message.turn_run_id.as_deref())
            .map(parse_persisted_turn_run_id)
        {
            let run_id = run_id?;
            if !seen.insert(run_id) {
                continue;
            }
            match self
                .turn_coordinator
                .get_run_state(GetRunStateRequest {
                    scope: scope.clone(),
                    run_id,
                })
                .await
            {
                Ok(state) if state.status.keeps_active_lock() => {
                    return Err(delete_thread_busy());
                }
                Ok(_) | Err(TurnError::ScopeNotFound) => {}
                Err(error) => return Err(map_turn_error(error)),
            }
        }
        Ok(())
    }
}

fn automation_unavailable() -> RebornServicesError {
    RebornServicesError::service_unavailable(true)
}

fn outbound_preferences_unavailable() -> RebornServicesError {
    RebornServicesError::service_unavailable(false)
}

struct AcceptedWebUiMessage {
    thread_id: ThreadId,
    message_id: ThreadMessageId,
    actor_id: String,
    source_binding_id: String,
    reply_target_binding_id: String,
}

async fn mark_message_submitted_or_replay(
    thread_service: &dyn SessionThreadService,
    thread_scope: &ThreadScope,
    handoff: &AcceptedWebUiMessage,
    client_action_id: &IdempotencyKey,
    turn_id: String,
    run_id: String,
) -> Result<(), RebornServicesError> {
    match thread_service
        .mark_message_submitted(
            thread_scope,
            &handoff.thread_id,
            handoff.message_id,
            turn_id,
            run_id.clone(),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(error) => {
            reconcile_terminal_duplicate(
                thread_service,
                thread_scope,
                handoff,
                client_action_id,
                |replay| {
                    replay.status == MessageStatus::Submitted && replay.turn_run_id == Some(run_id)
                },
                error,
            )
            .await
        }
    }
}

async fn mark_message_deferred_busy_or_replay(
    thread_service: &dyn SessionThreadService,
    thread_scope: &ThreadScope,
    handoff: &AcceptedWebUiMessage,
    client_action_id: &IdempotencyKey,
) -> Result<(), RebornServicesError> {
    match thread_service
        .mark_message_deferred_busy(thread_scope, &handoff.thread_id, handoff.message_id)
        .await
    {
        Ok(_) => Ok(()),
        Err(error) => {
            reconcile_terminal_duplicate(
                thread_service,
                thread_scope,
                handoff,
                client_action_id,
                |replay| replay.status == MessageStatus::DeferredBusy,
                error,
            )
            .await
        }
    }
}

async fn reconcile_terminal_duplicate(
    thread_service: &dyn SessionThreadService,
    thread_scope: &ThreadScope,
    handoff: &AcceptedWebUiMessage,
    client_action_id: &IdempotencyKey,
    matches_replay: impl FnOnce(&AcceptedInboundMessageReplay) -> bool,
    original_error: SessionThreadError,
) -> Result<(), RebornServicesError> {
    let replay = thread_service
        .replay_accepted_inbound_message(ReplayAcceptedInboundMessageRequest {
            scope: thread_scope.clone(),
            actor_id: handoff.actor_id.clone(),
            source_binding_id: handoff.source_binding_id.clone(),
            external_event_id: client_action_id.as_str().to_string(),
        })
        .await
        .map_err(map_thread_error)?;
    match replay {
        Some(replay)
            if replay.thread_id == handoff.thread_id
                && replay.message_id == handoff.message_id
                && matches_replay(&replay) =>
        {
            Ok(())
        }
        _ => Err(map_thread_error(original_error)),
    }
}

async fn replay_webui_send_message(
    thread_service: &dyn SessionThreadService,
    thread_scope: &ThreadScope,
    scope: &TurnScope,
    actor: &TurnActor,
    external_event_id: &str,
) -> Result<Option<(AcceptedInboundMessageReplay, String)>, RebornServicesError> {
    let source_binding_id = webui_source_binding_id(scope, actor);
    if let Some(replay) = replay_accepted_message(
        thread_service,
        thread_scope,
        actor,
        &source_binding_id,
        external_event_id,
    )
    .await?
    {
        return Ok(Some((replay, source_binding_id)));
    }

    let legacy_source_binding_id = legacy_webui_source_binding_id(scope, actor);
    replay_accepted_message(
        thread_service,
        thread_scope,
        actor,
        &legacy_source_binding_id,
        external_event_id,
    )
    .await
    .map(|replay| replay.map(|replay| (replay, legacy_source_binding_id)))
}

async fn replay_accepted_message(
    thread_service: &dyn SessionThreadService,
    thread_scope: &ThreadScope,
    actor: &TurnActor,
    source_binding_id: &str,
    external_event_id: &str,
) -> Result<Option<AcceptedInboundMessageReplay>, RebornServicesError> {
    thread_service
        .replay_accepted_inbound_message(ReplayAcceptedInboundMessageRequest {
            scope: thread_scope.clone(),
            actor_id: actor.user_id.as_str().to_string(),
            source_binding_id: source_binding_id.to_string(),
            external_event_id: external_event_id.to_string(),
        })
        .await
        .map_err(map_thread_error)
}

// Owner-bound thread resolution shared by the WebUI-facing methods that
// only need to prove a browser thread id belongs to the authenticated actor.
// The actor is pinned as `owner_user_id` so a caller sharing (tenant, agent,
// project) cannot act on a thread it does not own; `map_ownership_probe_error`
// collapses both UnknownThread and ThreadScopeMismatch into NotFound so the
// response cannot be used as an existence oracle.
impl RebornServices {
    async fn resolve_webui_thread_metadata(
        &self,
        scope: TurnScope,
        actor: &TurnActor,
    ) -> Result<(TurnScope, ThreadScope), RebornServicesError> {
        let thread_scope = thread_scope_from_turn_scope(&scope, Some(actor.user_id.clone()))?;
        // `read_thread` is the metadata-only probe; production backends
        // override it to skip the message/summary load entirely. The
        // ownership semantics (UnknownThread / ThreadScopeMismatch
        // collapse to NotFound) must match `list_thread_history`'s
        // path, which `map_ownership_probe_error` guarantees.
        self.thread_service
            .read_thread(ThreadHistoryRequest {
                scope: thread_scope.clone(),
                thread_id: scope.thread_id.clone(),
            })
            .await
            .map_err(map_ownership_probe_error)?;
        Ok((scope, thread_scope))
    }

    async fn resolve_approval_gate(
        &self,
        scope: TurnScope,
        actor: TurnActor,
        run_id: TurnRunId,
        gate_ref: GateRef,
        client_action_id: IdempotencyKey,
        resolution: WebUiGateResolution,
    ) -> Result<RebornResolveGateResponse, RebornServicesError> {
        let decision = match resolution {
            WebUiGateResolution::Approved { always } => {
                // `always: true` requests a *persistent* approval but this
                // facade has only one-shot approval interaction routing and no
                // approval-policy port. Fail loud rather than silently downgrade.
                if always {
                    return Err(persistent_approval_unavailable());
                }
                ApprovalInteractionDecision::ApproveOnce
            }
            WebUiGateResolution::Denied | WebUiGateResolution::Cancelled => {
                ApprovalInteractionDecision::Deny
            }
            WebUiGateResolution::CredentialProvided { .. } => {
                return Err(blocked_authentication_unavailable());
            }
        };
        let response = self
            .approval_interactions
            .resolve(ResolveApprovalInteractionRequest {
                scope,
                actor,
                run_id_hint: Some(run_id),
                gate_ref,
                decision,
                idempotency_key: client_action_id,
            })
            .await
            .map_err(|error| map_adapter_error(error.into()))?;
        match response {
            ResolveApprovalInteractionResponse::Approved(response) => {
                Ok(RebornResolveGateResponse::Resumed(response.into()))
            }
            ResolveApprovalInteractionResponse::Denied(response) => {
                Ok(RebornResolveGateResponse::Cancelled(response.into()))
            }
        }
    }

    async fn gate_resolution_route(
        &self,
        scope: &TurnScope,
        actor: &TurnActor,
        run_id: TurnRunId,
        gate_ref: &GateRef,
        resolution: &WebUiGateResolution,
    ) -> Result<GateResolutionRoute, RebornServicesError> {
        let state = match self
            .turn_coordinator
            .get_run_state(GetRunStateRequest {
                scope: scope.clone(),
                run_id,
            })
            .await
        {
            Ok(state) => state,
            Err(error) if error.category() == brassclaw_turns::TurnErrorCategory::ScopeNotFound => {
                return Ok(GateResolutionRoute::from_gate_shape(gate_ref, resolution));
            }
            Err(error) => return Err(map_turn_error(error)),
        };
        if state.actor.as_ref() != Some(actor) {
            return Err(participant_denied());
        }
        // This read only selects the WebUI route. The typed auth/approval
        // services intentionally re-read run-state through `blocked_gate_state`
        // before mutating auth/approval records or resuming/cancelling a run,
        // so stale facade classification cannot authorize a side effect.
        GateResolutionRoute::from_run_state(
            state.status,
            state.gate_ref.as_ref(),
            gate_ref,
            resolution,
        )
    }

    async fn resolve_auth_gate(
        &self,
        scope: TurnScope,
        actor: TurnActor,
        run_id: TurnRunId,
        gate_ref: GateRef,
        client_action_id: IdempotencyKey,
        resolution: WebUiGateResolution,
    ) -> Result<RebornResolveGateResponse, RebornServicesError> {
        let decision = match resolution {
            WebUiGateResolution::CredentialProvided { credential_ref } => {
                AuthInteractionDecision::CredentialProvided {
                    credential_ref: parse_credential_account_id(&credential_ref)
                        .map_err(map_auth_interaction_error)?,
                }
            }
            WebUiGateResolution::Denied | WebUiGateResolution::Cancelled => {
                AuthInteractionDecision::Deny
            }
            WebUiGateResolution::Approved { .. } => {
                return Err(blocked_authentication_unavailable());
            }
        };
        let response = self
            .auth_interactions
            .resolve(ResolveAuthInteractionRequest {
                scope,
                actor,
                run_id_hint: Some(run_id),
                gate_ref,
                decision,
                idempotency_key: client_action_id,
            })
            .await
            .map_err(map_auth_interaction_error)?;
        match response {
            ResolveAuthInteractionResponse::Resumed(response) => {
                Ok(RebornResolveGateResponse::Resumed(response.into()))
            }
            ResolveAuthInteractionResponse::Canceled(response) => {
                Ok(RebornResolveGateResponse::Cancelled(response.into()))
            }
        }
    }

    async fn resolve_generic_gate(
        &self,
        scope: TurnScope,
        actor: TurnActor,
        run_id: TurnRunId,
        gate_ref: GateRef,
        client_action_id: IdempotencyKey,
        resolution: WebUiGateResolution,
    ) -> Result<RebornResolveGateResponse, RebornServicesError> {
        match resolution {
            WebUiGateResolution::Approved { always } => {
                reject_generic_auth_gate_resolution(self.turn_coordinator.as_ref(), &scope, run_id)
                    .await?;
                // `always: true` requests a *persistent* approval but this
                // facade has only one-shot `resume_turn` and no approval-policy
                // port. Fail loud rather than silently downgrade.
                if always {
                    return Err(persistent_approval_unavailable());
                }
                let binding_id = webui_gate_binding_id(&scope, &gate_ref_string(&gate_ref));
                let response = self
                    .turn_coordinator
                    .resume_turn(ResumeTurnRequest {
                        scope,
                        actor,
                        run_id,
                        gate_resolution_ref: gate_ref,
                        precondition: ResumeTurnPrecondition::AnyBlockedGate,
                        source_binding_ref: webui_source_binding_ref_from_raw(
                            "webui-gate-src",
                            &binding_id,
                        )?,
                        reply_target_binding_ref: webui_reply_target_binding_ref_from_raw(
                            "webui-gate-reply",
                            &binding_id,
                        )?,
                        idempotency_key: client_action_id,
                    })
                    .await
                    .map_err(map_turn_error)?;
                Ok(RebornResolveGateResponse::Resumed(response.into()))
            }
            WebUiGateResolution::CredentialProvided { .. } => {
                Err(blocked_authentication_unavailable())
            }
            WebUiGateResolution::Denied | WebUiGateResolution::Cancelled => {
                assert_generic_run_parked_on_gate(
                    self.turn_coordinator.as_ref(),
                    &scope,
                    run_id,
                    &gate_ref,
                )
                .await?;
                // `cancel_run` is not gate-aware, so without this check a
                // denied/cancelled resolution for a stale or attacker-supplied
                // gate_ref would terminate any non-terminal run sharing run_id.
                let response = self
                    .turn_coordinator
                    .cancel_run(brassclaw_turns::CancelRunRequest {
                        scope,
                        actor,
                        run_id,
                        reason: SanitizedCancelReason::UserRequested,
                        idempotency_key: client_action_id,
                    })
                    .await
                    .map_err(map_turn_error)?;
                Ok(RebornResolveGateResponse::Cancelled(response.into()))
            }
        }
    }
}

/// Ownership probes must collapse "thread does not exist" and "thread exists
/// but is owned by another caller" into NotFound so that a caller sharing the
/// (tenant, agent, project) scope cannot tell whether the supplied `thread_id`
/// matches a real thread under a different owner. The current backends return
/// `UnknownThread` for both cases on `list_thread_history`, but the contract
/// also permits `ThreadScopeMismatch`; remap it explicitly so a future backend
/// change cannot silently reintroduce an existence-leak.
fn map_ownership_probe_error(error: SessionThreadError) -> RebornServicesError {
    match &error {
        SessionThreadError::ThreadScopeMismatch { .. } => {
            RebornServicesError::from_status(RebornServicesErrorCode::NotFound, 404, false)
        }
        _ => map_thread_error(error),
    }
}

fn validate_current_gate_ref(
    parked_gate_ref: Option<&GateRef>,
    requested_gate_ref: &GateRef,
    kind: RebornServicesErrorKind,
) -> Result<(), RebornServicesError> {
    match parked_gate_ref {
        Some(parked) if parked == requested_gate_ref => Ok(()),
        _ => Err(RebornServicesError::from_status_kind(
            RebornServicesErrorCode::Conflict,
            kind,
            409,
            false,
        )),
    }
}

fn participant_denied() -> RebornServicesError {
    RebornServicesError::from_status_kind(
        RebornServicesErrorCode::Forbidden,
        RebornServicesErrorKind::ParticipantDenied,
        403,
        false,
    )
}

/// Reject denied/cancelled generic gate resolutions whose `gate_ref` does not
/// match the gate the run is actually parked on. `cancel_run` is not gate-aware,
/// so without this guard a stale or attacker-supplied `gate_ref` would cancel
/// any non-terminal run sharing the same `run_id`.
async fn assert_generic_run_parked_on_gate(
    turn_coordinator: &dyn TurnCoordinator,
    scope: &TurnScope,
    run_id: TurnRunId,
    expected_gate_ref: &GateRef,
) -> Result<(), RebornServicesError> {
    let state = turn_coordinator
        .get_run_state(GetRunStateRequest {
            scope: scope.clone(),
            run_id,
        })
        .await
        .map_err(map_turn_error)?;
    if state.status == TurnStatus::BlockedAuth {
        return Err(blocked_authentication_unavailable());
    }
    if state.status == TurnStatus::BlockedApproval {
        return Err(blocked_approval_unavailable());
    }
    match state.gate_ref.as_ref() {
        Some(parked) if parked == expected_gate_ref => Ok(()),
        _ => Err(RebornServicesError::from_status_kind(
            RebornServicesErrorCode::Conflict,
            RebornServicesErrorKind::BlockedApproval,
            409,
            false,
        )),
    }
}

/// Generic WebUI gate handling is intentionally not allowed to resume or
/// cancel auth-blocked runs. Auth gates must pass through
/// AuthInteractionService so completed-flow/credential validation and
/// BlockedAuthGate preconditions are enforced.
async fn reject_generic_auth_gate_resolution(
    turn_coordinator: &dyn TurnCoordinator,
    scope: &TurnScope,
    run_id: TurnRunId,
) -> Result<(), RebornServicesError> {
    let state = turn_coordinator
        .get_run_state(GetRunStateRequest {
            scope: scope.clone(),
            run_id,
        })
        .await
        .map_err(map_turn_error)?;
    if state.status == TurnStatus::BlockedAuth {
        return Err(blocked_authentication_unavailable());
    }
    if state.status == TurnStatus::BlockedApproval {
        return Err(blocked_approval_unavailable());
    }
    Ok(())
}

fn parse_credential_account_id(value: &str) -> Result<CredentialAccountId, ProductWorkflowError> {
    uuid::Uuid::parse_str(value)
        .map(CredentialAccountId::from_uuid)
        .map_err(|_| ProductWorkflowError::AuthInteractionRejected {
            kind: AuthInteractionRejectionKind::InvalidCredentialRef,
        })
}

fn thread_scope_from_turn_scope(
    scope: &TurnScope,
    owner_user_id: Option<brassclaw_host_api::UserId>,
) -> Result<ThreadScope, RebornServicesError> {
    let Some(agent_id) = scope.agent_id.clone() else {
        return Err(RebornServicesError::from_status(
            RebornServicesErrorCode::InvalidRequest,
            400,
            false,
        ));
    };
    Ok(ThreadScope {
        tenant_id: scope.tenant_id.clone(),
        agent_id,
        project_id: scope.project_id.clone(),
        owner_user_id,
        mission_id: None,
    })
}

fn parse_thread_id_field(
    field: &'static str,
    value: String,
) -> Result<ThreadId, RebornServicesError> {
    ThreadId::new(value).map_err(|_| {
        RebornServicesError::validation(WebUiInboundValidationError::new(
            field,
            WebUiInboundValidationCode::InvalidId,
        ))
    })
}

fn parse_run_id_field(
    field: &'static str,
    value: String,
) -> Result<TurnRunId, RebornServicesError> {
    Uuid::parse_str(&value)
        .map(TurnRunId::from_uuid)
        .map_err(|_| {
            RebornServicesError::validation(WebUiInboundValidationError::new(
                field,
                WebUiInboundValidationCode::InvalidId,
            ))
        })
}

fn parse_persisted_turn_run_id(value: &str) -> Result<TurnRunId, RebornServicesError> {
    TurnRunId::parse(value).map_err(|_| RebornServicesError::internal_invariant())
}

fn accepted_message_ref(message_id: String) -> Result<AcceptedMessageRef, RebornServicesError> {
    AcceptedMessageRef::new(format!("msg:{message_id}")).map_err(|_| {
        RebornServicesError::from_status(RebornServicesErrorCode::Internal, 500, false)
    })
}

fn parse_replay_run_id(value: Option<String>) -> Result<TurnRunId, RebornServicesError> {
    let Some(value) = value else {
        return Err(RebornServicesError::from_status_kind(
            RebornServicesErrorCode::Conflict,
            RebornServicesErrorKind::ReplayUnavailable,
            409,
            false,
        ));
    };
    Uuid::parse_str(&value)
        .map(TurnRunId::from_uuid)
        .map_err(|_| {
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Conflict,
                RebornServicesErrorKind::ReplayUnavailable,
                409,
                false,
            )
        })
}

fn webui_source_binding_ref_from_raw(
    prefix: &str,
    raw: &str,
) -> Result<brassclaw_turns::SourceBindingRef, RebornServicesError> {
    bounded_source_binding_ref(prefix, raw, DEFAULT_BINDING_REF_RAW_MAX_BYTES).map_err(|_| {
        RebornServicesError::from_status(RebornServicesErrorCode::Internal, 500, false)
    })
}

fn webui_reply_target_binding_ref_from_raw(
    prefix: &str,
    raw: &str,
) -> Result<brassclaw_turns::ReplyTargetBindingRef, RebornServicesError> {
    bounded_reply_target_binding_ref(prefix, raw, DEFAULT_BINDING_REF_RAW_MAX_BYTES).map_err(|_| {
        RebornServicesError::from_status(RebornServicesErrorCode::Internal, 500, false)
    })
}

fn webui_source_binding_id(scope: &TurnScope, actor: &TurnActor) -> String {
    // WebUI retries are scoped to the authenticated caller context, not the
    // thread id. When the caller is not project-bound, we encode that
    // explicitly rather than collapsing onto an empty string.
    format!(
        "{}{}{}{}{}{}",
        segment("surface", "webui"),
        segment("tenant", scope.tenant_id.as_str()),
        segment(
            "agent",
            scope.agent_id.as_ref().map(AgentId::as_str).unwrap_or("")
        ),
        segment(
            "project_scope",
            if scope.project_id.is_some() {
                "bound"
            } else {
                "none"
            }
        ),
        scope
            .project_id
            .as_ref()
            .map(|project_id| segment("project", project_id.as_str()))
            .unwrap_or_default(),
        segment("actor", actor.user_id.as_str())
    )
}

fn legacy_webui_source_binding_id(scope: &TurnScope, actor: &TurnActor) -> String {
    format!(
        "{}{}{}{}{}",
        segment("surface", "webui"),
        segment("tenant", scope.tenant_id.as_str()),
        segment(
            "agent",
            scope.agent_id.as_ref().map(AgentId::as_str).unwrap_or("")
        ),
        segment("thread", scope.thread_id.as_str()),
        segment("actor", actor.user_id.as_str())
    )
}

fn thread_operation_key(scope: &TurnScope) -> String {
    format!(
        "{}{}{}{}{}",
        segment("tenant", scope.tenant_id.as_str()),
        segment(
            "agent",
            scope.agent_id.as_ref().map(AgentId::as_str).unwrap_or("")
        ),
        segment(
            "project",
            scope
                .project_id
                .as_ref()
                .map(ProjectId::as_str)
                .unwrap_or("")
        ),
        segment("thread", scope.thread_id.as_str()),
        segment(
            "owner",
            scope
                .explicit_owner_user_id()
                .map(UserId::as_str)
                .unwrap_or("")
        )
    )
}

/// Default page size for [`RebornServicesApi::get_timeline`] when the
/// caller does not supply one. Sized to cover a typical chat history
/// without forcing a multi-megabyte JSON response on first load.
pub(crate) const TIMELINE_DEFAULT_PAGE_SIZE: u32 = 100;

/// Hard ceiling on the number of messages a single timeline response can
/// carry. Callers asking for more get the cap. Without this, a large
/// thread would let the per-route rate limit be the only thing bounding
/// per-request response size, which was the original Medium review
/// issue.
pub(crate) const TIMELINE_MAX_PAGE_SIZE: u32 = 200;

/// Default number of automation rows returned when the browser does not
/// request a smaller page.
pub const AUTOMATION_LIST_DEFAULT_PAGE_SIZE: u32 = 50;

/// Hard ceiling for the beta automation management list response. This keeps
/// the user-facing endpoint bounded until the trigger capability exposes an
/// opaque cursor contract.
pub const AUTOMATION_LIST_MAX_PAGE_SIZE: u32 = 100;

/// Hard ceiling on summary artifacts returned per response. Summary
/// artifacts are typically much smaller than the message transcript so
/// this cap is generous; it exists to bound the worst case where a
/// thread accumulates an unusual number of summaries.
const TIMELINE_MAX_SUMMARY_ARTIFACTS: usize = 200;

fn clamp_timeline_limit(requested: Option<u32>) -> usize {
    let raw = requested.unwrap_or(TIMELINE_DEFAULT_PAGE_SIZE);
    let clamped = raw.clamp(1, TIMELINE_MAX_PAGE_SIZE);
    clamped as usize
}

fn clamp_automation_list_limit(requested: Option<u32>) -> usize {
    let raw = requested.unwrap_or(AUTOMATION_LIST_DEFAULT_PAGE_SIZE);
    let clamped = raw.clamp(1, AUTOMATION_LIST_MAX_PAGE_SIZE);
    clamped as usize
}

/// Wire shape of the opaque timeline cursor. The browser does not need
/// to interpret this; it just echoes the previous response's
/// `next_cursor` back as the next request's `cursor`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TimelineCursor {
    /// Only return messages whose `sequence` is strictly less than this
    /// value. Naming is deliberate: `before_*` makes the directional
    /// semantics (page backward through history) obvious at call sites.
    before_message_sequence: u64,
}

fn parse_timeline_cursor(raw: Option<&str>) -> Result<Option<TimelineCursor>, RebornServicesError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    let cursor: TimelineCursor = serde_json::from_str(raw).map_err(|_| {
        RebornServicesError::validation(WebUiInboundValidationError::new(
            "cursor",
            WebUiInboundValidationCode::InvalidValue,
        ))
    })?;
    Ok(Some(cursor))
}

fn serialize_timeline_cursor(cursor: &TimelineCursor) -> Option<String> {
    // Serialization of a tiny tagged struct is total in practice, but
    // returning Option keeps the call site honest without an unwrap.
    serde_json::to_string(cursor).ok()
}

/// Slice the message transcript to the most recent `limit` messages
/// strictly older than `cursor.before_message_sequence` (or the most
/// recent `limit` overall when no cursor is supplied), returning the
/// page plus the cursor the caller should pass back to load the page
/// preceding this one. `None` for `next_cursor` means there is nothing
/// older — the caller has reached the start of the thread.
///
/// Messages are sorted by `sequence` ascending before slicing so the
/// returned page is in monotonic order regardless of the input order
/// the underlying store happens to produce.
fn paginate_timeline_messages(
    mut messages: Vec<brassclaw_threads::ThreadMessageRecord>,
    limit: usize,
    cursor: Option<TimelineCursor>,
) -> (Vec<brassclaw_threads::ThreadMessageRecord>, Option<String>) {
    messages.sort_by_key(|message| message.sequence);
    if let Some(cursor) = cursor.as_ref() {
        messages.retain(|message| message.sequence < cursor.before_message_sequence);
    }
    let total = messages.len();
    let start = total.saturating_sub(limit);
    let next_cursor = if start > 0 {
        // The next page is older than the oldest message in *this* page.
        // We take the sequence of the page's first (oldest) message and
        // use it as `before_message_sequence` for the follow-up: that
        // request returns messages with sequence < this one, i.e. the
        // page strictly preceding the current one.
        messages.get(start).and_then(|message| {
            serialize_timeline_cursor(&TimelineCursor {
                before_message_sequence: message.sequence,
            })
        })
    } else {
        None
    };
    let page: Vec<_> = messages.into_iter().skip(start).collect();
    (page, next_cursor)
}

fn cap_summary_artifacts(
    mut artifacts: Vec<brassclaw_threads::SummaryArtifact>,
) -> Vec<brassclaw_threads::SummaryArtifact> {
    if artifacts.len() > TIMELINE_MAX_SUMMARY_ARTIFACTS {
        artifacts.truncate(TIMELINE_MAX_SUMMARY_ARTIFACTS);
    }
    artifacts
}

fn webui_gate_binding_id(scope: &TurnScope, gate_ref: &str) -> String {
    format!(
        "{}{}{}{}",
        segment("surface", "webui"),
        segment("tenant", scope.tenant_id.as_str()),
        segment("thread", scope.thread_id.as_str()),
        segment("gate", gate_ref)
    )
}

fn gate_ref_string(gate_ref: &brassclaw_turns::GateRef) -> String {
    gate_ref.as_str().to_string()
}

fn persistent_approval_unavailable() -> RebornServicesError {
    RebornServicesError::from_status_kind(
        RebornServicesErrorCode::Unavailable,
        RebornServicesErrorKind::BlockedApproval,
        503,
        false,
    )
}

fn blocked_approval_unavailable() -> RebornServicesError {
    persistent_approval_unavailable()
}

fn blocked_authentication_unavailable() -> RebornServicesError {
    RebornServicesError::from_status_kind(
        RebornServicesErrorCode::Unavailable,
        RebornServicesErrorKind::BlockedAuthentication,
        503,
        false,
    )
}

fn segment(name: &str, value: &str) -> String {
    format!("{name}:{}:{value};", value.len())
}

fn map_timeline_probe_error(error: SessionThreadError) -> RebornServicesError {
    match error {
        SessionThreadError::Serialization(_)
        | SessionThreadError::Deserialization(_)
        | SessionThreadError::Backend(_) => RebornServicesError::from_status_kind(
            RebornServicesErrorCode::Unavailable,
            RebornServicesErrorKind::TimelineUnavailable,
            503,
            true,
        ),
        _ => map_ownership_probe_error(error),
    }
}

fn map_thread_error(error: SessionThreadError) -> RebornServicesError {
    match error {
        SessionThreadError::UnknownThread { .. } | SessionThreadError::UnknownMessage { .. } => {
            RebornServicesError::from_status(RebornServicesErrorCode::NotFound, 404, false)
        }
        SessionThreadError::IdempotentReplayThreadMismatch { .. } => {
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Conflict,
                RebornServicesErrorKind::Duplicate,
                409,
                false,
            )
        }
        SessionThreadError::ThreadScopeMismatch { .. }
        | SessionThreadError::IdempotentReplayActorMismatch { .. }
        | SessionThreadError::InvalidMessageTransition { .. }
        | SessionThreadError::MessageNotDraft { .. }
        | SessionThreadError::InvalidSummaryRange { .. }
        | SessionThreadError::OverlappingSummaryRange { .. } => {
            RebornServicesError::from_status(RebornServicesErrorCode::Conflict, 409, false)
        }
        SessionThreadError::GeneratedThreadId(_)
        | SessionThreadError::Serialization(_)
        | SessionThreadError::Deserialization(_)
        | SessionThreadError::Backend(_) => RebornServicesError::service_unavailable(true),
    }
}

fn delete_thread_busy() -> RebornServicesError {
    RebornServicesError::from_status_kind(
        RebornServicesErrorCode::Conflict,
        RebornServicesErrorKind::Busy,
        409,
        false,
    )
}

fn map_turn_error(error: TurnError) -> RebornServicesError {
    let (code, kind, status_code, retryable) = match error.category() {
        brassclaw_turns::TurnErrorCategory::ThreadBusy => (
            RebornServicesErrorCode::Conflict,
            RebornServicesErrorKind::Busy,
            409,
            false,
        ),
        brassclaw_turns::TurnErrorCategory::Conflict => (
            RebornServicesErrorCode::Conflict,
            RebornServicesErrorKind::Conflict,
            409,
            false,
        ),
        brassclaw_turns::TurnErrorCategory::AdmissionRejected => (
            RebornServicesErrorCode::RateLimited,
            RebornServicesErrorKind::Busy,
            429,
            true,
        ),
        brassclaw_turns::TurnErrorCategory::CapacityExceeded => (
            RebornServicesErrorCode::RateLimited,
            RebornServicesErrorKind::Busy,
            429,
            false,
        ),
        brassclaw_turns::TurnErrorCategory::ScopeNotFound => (
            RebornServicesErrorCode::NotFound,
            RebornServicesErrorKind::NotFound,
            404,
            false,
        ),
        brassclaw_turns::TurnErrorCategory::Unauthorized => (
            RebornServicesErrorCode::Forbidden,
            RebornServicesErrorKind::ParticipantDenied,
            403,
            false,
        ),
        brassclaw_turns::TurnErrorCategory::InvalidRequest => (
            RebornServicesErrorCode::InvalidRequest,
            RebornServicesErrorKind::Validation,
            400,
            false,
        ),
        brassclaw_turns::TurnErrorCategory::Unavailable => (
            RebornServicesErrorCode::Unavailable,
            RebornServicesErrorKind::ServiceUnavailable,
            503,
            true,
        ),
    };
    RebornServicesError::from_status_kind(code, kind, status_code, retryable)
}

fn map_adapter_error(error: ProductAdapterError) -> RebornServicesError {
    match error {
        ProductAdapterError::WorkflowRejected {
            kind,
            status_code,
            retryable,
            ..
        } => RebornServicesError::from_status_kind(
            code_for_status(status_code),
            kind_for_workflow_rejection(kind),
            status_code,
            retryable,
        ),
        ProductAdapterError::WorkflowTransient { .. }
        | ProductAdapterError::EgressTransient { .. } => {
            RebornServicesError::service_unavailable(true)
        }
        ProductAdapterError::Authentication(_) => {
            RebornServicesError::from_status(RebornServicesErrorCode::Unauthenticated, 401, false)
        }
        ProductAdapterError::MalformedInboundPayload { .. }
        | ProductAdapterError::InvalidIdentifier { .. } => {
            RebornServicesError::from_status(RebornServicesErrorCode::InvalidRequest, 400, false)
        }
        ProductAdapterError::EgressDenied { .. }
        | ProductAdapterError::EgressUndeclaredHost { .. } => {
            RebornServicesError::from_status_kind(
                RebornServicesErrorCode::Forbidden,
                RebornServicesErrorKind::BlockedResource,
                403,
                false,
            )
        }
        ProductAdapterError::Internal { .. } => {
            RebornServicesError::from_status(RebornServicesErrorCode::Internal, 500, false)
        }
    }
}

fn map_auth_interaction_error(error: ProductWorkflowError) -> RebornServicesError {
    match error {
        ProductWorkflowError::AuthInteractionRejected { kind } => {
            RebornServicesError::from_status_kind(
                code_for_status(kind.status_code()),
                RebornServicesErrorKind::BlockedAuthentication,
                kind.status_code(),
                kind.retryable(),
            )
        }
        error => map_adapter_error(error.into()),
    }
}

fn map_projection_error(error: ProductAdapterError) -> RebornServicesError {
    match error {
        ProductAdapterError::WorkflowRejected {
            kind: ProductWorkflowRejectionKind::Unavailable,
            status_code,
            retryable,
            ..
        } => RebornServicesError::from_status_kind(
            code_for_status(status_code),
            RebornServicesErrorKind::ReplayUnavailable,
            status_code,
            retryable,
        ),
        ProductAdapterError::WorkflowTransient { .. }
        | ProductAdapterError::EgressTransient { .. } => RebornServicesError::from_status_kind(
            RebornServicesErrorCode::Unavailable,
            RebornServicesErrorKind::ReplayUnavailable,
            503,
            true,
        ),
        _ => map_adapter_error(error),
    }
}

fn code_for_status(status_code: u16) -> RebornServicesErrorCode {
    match status_code {
        400 => RebornServicesErrorCode::InvalidRequest,
        401 => RebornServicesErrorCode::Unauthenticated,
        403 => RebornServicesErrorCode::Forbidden,
        404 => RebornServicesErrorCode::NotFound,
        409 => RebornServicesErrorCode::Conflict,
        429 => RebornServicesErrorCode::RateLimited,
        503 => RebornServicesErrorCode::Unavailable,
        _ => RebornServicesErrorCode::Internal,
    }
}

fn kind_for_workflow_rejection(kind: ProductWorkflowRejectionKind) -> RebornServicesErrorKind {
    match kind {
        ProductWorkflowRejectionKind::ThreadBusy
        | ProductWorkflowRejectionKind::AdmissionRejected => RebornServicesErrorKind::Busy,
        ProductWorkflowRejectionKind::ScopeNotFound => RebornServicesErrorKind::NotFound,
        ProductWorkflowRejectionKind::Unauthorized => RebornServicesErrorKind::ParticipantDenied,
        ProductWorkflowRejectionKind::InvalidRequest => RebornServicesErrorKind::Validation,
        ProductWorkflowRejectionKind::Unavailable => RebornServicesErrorKind::ServiceUnavailable,
        ProductWorkflowRejectionKind::Conflict => RebornServicesErrorKind::Conflict,
    }
}

fn create_thread_metadata_json(
    client_action_id: &brassclaw_turns::IdempotencyKey,
) -> Result<String, RebornServicesError> {
    serde_json::to_string(&serde_json::json!({
        "client_action_id": client_action_id.as_str(),
    }))
    .map_err(|_| RebornServicesError::internal_invariant())
}

fn product_agent_bound_caller_from_webui(
    caller: WebUiAuthenticatedCaller,
) -> Option<ProductAgentBoundCaller> {
    let agent_id = caller.agent_id?;
    Some(ProductAgentBoundCaller::new(
        caller.tenant_id,
        caller.user_id,
        agent_id,
        caller.project_id,
    ))
}

fn generated_thread_id(
    caller: &WebUiAuthenticatedCaller,
    client_action_id: &brassclaw_turns::IdempotencyKey,
) -> ThreadId {
    let seed = format!(
        "{}{}{}{}{}{}",
        segment("surface", "webui-create-thread"),
        segment("tenant", caller.tenant_id.as_str()),
        segment("user", caller.user_id.as_str()),
        segment(
            "agent",
            caller.agent_id.as_ref().map(AgentId::as_str).unwrap_or("")
        ),
        segment(
            "project",
            caller
                .project_id
                .as_ref()
                .map(brassclaw_host_api::ProjectId::as_str)
                .unwrap_or("")
        ),
        segment("action", client_action_id.as_str())
    );
    let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes());
    // UUID text contains no path separators/control characters and is accepted by ThreadId.
    match ThreadId::new(id.to_string()) {
        Ok(thread_id) => thread_id,
        Err(error) => {
            debug_assert!(false, "generated UUID thread id should be valid: {error}");
            // Fallback remains valid under ThreadId validation rules.
            ThreadId::new("generated-thread-fallback").unwrap_or_else(|_| unreachable!())
        }
    }
}
