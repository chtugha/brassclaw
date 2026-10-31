#[path = "auth_google_original.rs"]
mod google_original;

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_auth::{
    AuthChallenge, AuthContinuationEvent, AuthContinuationRef, AuthErrorCode, AuthFlowId,
    AuthFlowKind, AuthFlowManager, AuthFlowOwnerScope, AuthFlowRecord, AuthFlowRecordSource,
    AuthFlowStatus, AuthGateRef, AuthInteractionId, AuthInteractionService, AuthProductError,
    AuthProductScope, AuthProviderClient, AuthProviderId, CredentialAccountChoiceRequest,
    CredentialAccountId, CredentialAccountLabel, CredentialAccountListPage,
    CredentialAccountListRequest, CredentialAccountProjection, CredentialAccountRecordSource,
    CredentialAccountService, CredentialAccountStatus, CredentialAccountUpdateBinding,
    CredentialRecoveryProjection, CredentialRecoveryRequest, CredentialRefreshReport,
    CredentialRefreshRequest, CredentialSetupService, InMemoryAuthProductServices,
    ManualTokenSetupIdentity, ManualTokenSetupRequest, NewAuthFlow, OAuthAuthorizationUrl,
    OAuthCallbackClaimRequest, OAuthCallbackFailureInput, OAuthCallbackInput,
    OAuthProviderCallbackRequest, OAuthProviderExchangeContext, OpaqueStateHash, PkceVerifierHash,
    ProviderBackedCredentialAccountService, ProviderCallbackOutcome, ProviderScope,
    SecretCleanupReport, SecretCleanupRequest, SecretCleanupService, SecretSubmitRequest,
    Timestamp, TurnGateAuthFlowQuery, TurnRunRef, scope_matches,
};
use brassclaw_product_adapters::AuthPromptChallengeKind;
use brassclaw_product_workflow::ProductAuthTurnGateResumeDispatcher;
use chrono::Utc;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

use brassclaw_host_api::UserId;
use brassclaw_turns::{TurnRunId, TurnScope};

use crate::auth_prompt::{AuthChallengeProvider, AuthChallengeView};
use crate::manual_token_flow::{PortBackedManualTokenFlowService, RebornManualTokenFlowService};
use crate::oauth_dcr::{DcrGateChallengeRequest, DcrSetupFlowRequest, OAuthDcrProviderRegistry};
use crate::oauth_gate::{GoogleOAuthGateProviderRegistry, OAuthGateChallengeRequest};
use crate::product_auth_runtime_credentials::{
    ProductAuthRuntimeCredentialAccountSelector, RuntimeCredentialAccountSelectionService,
};

/// Dispatches a typed continuation event once an OAuth callback flow has
/// completed.
///
/// # Idempotency contract
///
/// Implementations MUST be idempotent on `flow_id`.  The product-auth layer
/// guarantees *at-least-once* delivery: if `dispatch_auth_continuation`
/// succeeds but the subsequent `mark_continuation_dispatched` call fails
/// (e.g. a transient `BackendConflict` or `BackendUnavailable`), the caller
/// will retry the full callback path and dispatch the same `flow_id` again.
/// An implementation that assumes exactly-once delivery will process duplicate
/// continuations and is incorrect.
#[async_trait]
pub trait RebornAuthContinuationDispatcher: Send + Sync {
    async fn dispatch_auth_continuation(
        &self,
        event: AuthContinuationEvent,
    ) -> Result<(), AuthProductError>;
}

#[cfg(test)]
#[derive(Debug, Default)]
struct NoopAuthContinuationDispatcher;

#[cfg(test)]
#[async_trait]
impl RebornAuthContinuationDispatcher for NoopAuthContinuationDispatcher {
    async fn dispatch_auth_continuation(
        &self,
        _event: AuthContinuationEvent,
    ) -> Result<(), AuthProductError> {
        Ok(())
    }
}

#[async_trait]
impl RebornAuthContinuationDispatcher for ProductAuthTurnGateResumeDispatcher {
    async fn dispatch_auth_continuation(
        &self,
        event: AuthContinuationEvent,
    ) -> Result<(), AuthProductError> {
        ProductAuthTurnGateResumeDispatcher::dispatch_auth_continuation(self, event).await
    }
}

/// Parsed OAuth callback request handed from a host-owned HTTP route into the
/// Reborn product-auth boundary.
///
/// Raw query/body parsing and hashing are host-route responsibilities. This
/// type intentionally receives only the validated scope, flow id, state hash,
/// and one-shot provider exchange input. It is not serializable because the
/// authorized outcome can carry raw OAuth code/verifier material inside
/// [`OAuthProviderCallbackRequest`].
#[derive(Debug)]
pub struct RebornOAuthCallbackRequest {
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
    pub opaque_state_hash: OpaqueStateHash,
    pub outcome: RebornOAuthCallbackOutcome,
}

/// Typed setup OAuth start request after host-route parsing and hashing.
///
/// The browser-facing route chooses neither flow kind nor continuation. Those
/// product-auth semantics stay here with the auth service boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RebornOAuthStartFlowRequest {
    /// Complete usage and previous selection captured before the challenge.
    pub(crate) requirement_setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    pub(crate) flow_id: Option<AuthFlowId>,
    pub(crate) scope: AuthProductScope,
    pub(crate) provider: AuthProviderId,
    pub(crate) authorization_url: OAuthAuthorizationUrl,
    pub(crate) opaque_state_hash: OpaqueStateHash,
    pub(crate) pkce_verifier_hash: PkceVerifierHash,
    pub(crate) update_binding: Option<CredentialAccountUpdateBinding>,
    pub(crate) expires_at: brassclaw_auth::Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "used by the webui-v2-beta extension OAuth route through product-auth route composition"
)]
pub(crate) struct RebornDcrOAuthStartFlowRequest {
    /// Complete usage and previous selection captured before the challenge.
    pub(crate) requirement_setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    pub(crate) flow_id: AuthFlowId,
    pub(crate) scope: AuthProductScope,
    pub(crate) provider: AuthProviderId,
    pub(crate) account_label: CredentialAccountLabel,
    pub(crate) provider_scopes: Vec<ProviderScope>,
    pub(crate) update_binding: Option<CredentialAccountUpdateBinding>,
    pub(crate) expires_at: brassclaw_auth::Timestamp,
}

/// Complete explicit correlation checked before repairing a partial DCR original.
/// This is host-auth metadata, not a browser-defined credential declaration.
#[derive(Clone)]
pub(crate) struct RebornDcrSetupOriginalLookup<'a> {
    pub(crate) scope: &'a AuthProductScope,
    pub(crate) flow_id: AuthFlowId,
    pub(crate) provider: &'a AuthProviderId,
    pub(crate) account_label: &'a CredentialAccountLabel,
    pub(crate) provider_scopes: &'a [ProviderScope],
    pub(crate) expires_at: brassclaw_auth::Timestamp,
    pub(crate) requester_extension: &'a brassclaw_host_api::ExtensionId,
    pub(crate) requirement_name: Option<&'a str>,
    pub(crate) target: Option<&'a brassclaw_product_workflow::ExtensionCredentialTarget>,
    pub(crate) cancel_preparation: bool,
}

fn verify_extension_oauth_selection(
    setup: Option<&brassclaw_auth::CredentialRequirementSetup>,
    binding: Option<&CredentialAccountUpdateBinding>,
    requester: &brassclaw_host_api::ExtensionId,
    requirement_name: Option<&str>,
    target: Option<&brassclaw_product_workflow::ExtensionCredentialTarget>,
) -> Result<(), AuthProductError> {
    let Some(setup) = setup else {
        return if requirement_name.is_none() && target.is_none() {
            Ok(())
        } else {
            Err(AuthProductError::BackendConflict)
        };
    };
    if setup.requirement().requester_extension() != requester
        || requirement_name
            != Some(
                setup
                    .requirement()
                    .reference()
                    .map_err(|_| AuthProductError::BackendConflict)?
                    .as_str(),
            )
    {
        return Err(AuthProductError::BackendConflict);
    }
    let same_target = match (target, binding) {
        (Some(brassclaw_product_workflow::ExtensionCredentialTarget::New {}), None) => true,
        (
            Some(brassclaw_product_workflow::ExtensionCredentialTarget::Existing { account_id }),
            Some(binding),
        ) => *account_id == binding.account_id,
        _ => false,
    };
    if !same_target {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

impl RebornDcrSetupOriginalLookup<'_> {
    fn verify_preparation(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
    ) -> Result<(), AuthProductError> {
        if original.scope != *self.scope || original.flow_id != self.flow_id {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if &original.provider != self.provider
            || &original.account_label != self.account_label
            || original.requested_scopes.as_slice() != self.provider_scopes
            || original.expires_at != self.expires_at
            || original.continuation != AuthContinuationRef::SetupOnly
        {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(setup) = &original.requirement_setup {
            setup.verify_oauth(self.scope, self.provider, self.provider_scopes)?;
        }
        verify_extension_oauth_selection(
            original.requirement_setup.as_deref(),
            original.update_binding.as_ref(),
            self.requester_extension,
            self.requirement_name,
            self.target,
        )
    }
}

/// Host-route OAuth callback parse result.
#[derive(Debug)]
pub enum RebornOAuthCallbackOutcome {
    Authorized {
        provider_request: OAuthProviderCallbackRequest,
    },
    ProviderDenied,
    Malformed,
}

/// Stable sanitized callback response safe for Web/CLI/API surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornOAuthCallbackResponse {
    pub flow_id: AuthFlowId,
    pub status: AuthFlowStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_account_id: Option<CredentialAccountId>,
    pub continuation: AuthContinuationRef,
}

/// Stable sanitized auth failure safe for route rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornAuthProductError {
    pub code: AuthErrorCode,
    pub retryable: bool,
    /// Safe original/precondition conflict disposition, without backend details.
    /// Old serialized errors remain readable and ordinary wire bodies unchanged.
    #[serde(default, skip_serializing_if = "auth_error_is_not_conflict")]
    pub conflict: bool,
}

fn auth_error_is_not_conflict(conflict: &bool) -> bool {
    !*conflict
}

impl From<AuthProductError> for RebornAuthProductError {
    fn from(error: AuthProductError) -> Self {
        let code = error.code();
        let conflict = matches!(error, AuthProductError::BackendConflict);
        Self {
            code,
            retryable: !conflict && is_retryable_auth_error(code),
            conflict,
        }
    }
}

/// Stable sanitized callback failure safe for route rendering.
pub type RebornOAuthCallbackError = RebornAuthProductError;

/// Request to open a Reborn manual-token setup interaction.
///
/// This request is intentionally not serializable because the scope must be
/// constructed from trusted caller/session context, not copied from a browser
/// body. The raw token is submitted later through
/// [`RebornManualTokenSubmitRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebornManualTokenSetupRequest {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub label: CredentialAccountLabel,
    pub continuation: AuthContinuationRef,
    pub update_binding: Option<CredentialAccountUpdateBinding>,
    pub requirement_setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    pub expires_at: Timestamp,
}

impl RebornManualTokenSetupRequest {
    pub fn new(
        scope: AuthProductScope,
        provider: AuthProviderId,
        label: CredentialAccountLabel,
        continuation: AuthContinuationRef,
        expires_at: Timestamp,
    ) -> Self {
        Self {
            scope,
            provider,
            label,
            continuation,
            update_binding: None,
            requirement_setup: None,
            expires_at,
        }
    }

    pub fn with_update_binding(mut self, update_binding: CredentialAccountUpdateBinding) -> Self {
        self.update_binding = Some(update_binding);
        self
    }

    pub fn with_requirement_setup(
        mut self,
        setup: brassclaw_auth::CredentialRequirementSetup,
    ) -> Self {
        self.requirement_setup = Some(Box::new(setup));
        self
    }
}

/// Manual-token challenge safe to render to Web/CLI/API surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornManualTokenChallenge {
    pub interaction_id: AuthInteractionId,
    pub provider: AuthProviderId,
    pub label: CredentialAccountLabel,
    pub expires_at: Timestamp,
}

/// Original metadata receipt. A terminal status never becomes a fresh challenge;
/// raw acceptance and confirmed-reference continuation remain separate operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornManualTokenSetupReceipt {
    pub identity: ManualTokenSetupIdentity,
    pub status: AuthFlowStatus,
    pub challenge: RebornManualTokenChallenge,
}

/// Secure manual-token submit request.
///
/// This type intentionally does not implement serde serialization. Host-owned
/// routes may construct it after reading a dedicated secret input body, but raw
/// token material must not be written into product DTOs, projections, logs, or
/// model-visible messages.
pub struct RebornManualTokenSubmitRequest {
    pub scope: AuthProductScope,
    pub interaction_id: AuthInteractionId,
    pub secret: SecretString,
}

impl RebornManualTokenSubmitRequest {
    pub fn new(
        scope: AuthProductScope,
        interaction_id: AuthInteractionId,
        secret: SecretString,
    ) -> Self {
        Self {
            scope,
            interaction_id,
            secret,
        }
    }
}

impl std::fmt::Debug for RebornManualTokenSubmitRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RebornManualTokenSubmitRequest")
            .field("scope", &self.scope)
            .field("interaction_id", &self.interaction_id)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

/// Stable sanitized manual-token submit response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebornManualTokenSubmitResponse {
    pub account_id: CredentialAccountId,
    pub status: CredentialAccountStatus,
    pub continuation: AuthContinuationRef,
}

/// Stable sanitized manual-token setup/submit failure safe for route rendering.
pub type RebornManualTokenError = RebornAuthProductError;

/// Stable sanitized lifecycle failure safe for Web/CLI/API surfaces.
pub type RebornCredentialLifecycleError = RebornAuthProductError;

fn is_retryable_auth_error(code: AuthErrorCode) -> bool {
    matches!(code, AuthErrorCode::BackendUnavailable)
}

#[derive(Debug)]
struct UnsupportedCredentialAccountRecordSource;

#[async_trait]
impl CredentialAccountRecordSource for UnsupportedCredentialAccountRecordSource {
    async fn accounts_for_owner(
        &self,
        _scope: &AuthProductScope,
    ) -> Result<Vec<brassclaw_auth::CredentialAccount>, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }
}

#[derive(Clone)]
pub struct RebornProductAuthServicePorts {
    flow_manager: Arc<dyn AuthFlowManager>,
    interaction_service: Arc<dyn AuthInteractionService>,
    manual_token_flow_service: Arc<dyn RebornManualTokenFlowService>,
    credential_setup_service: Arc<dyn CredentialSetupService>,
    credential_account_service: Arc<dyn CredentialAccountService>,
    credential_account_record_source: Arc<dyn CredentialAccountRecordSource>,
    provider_client: Arc<dyn AuthProviderClient>,
    cleanup_service: Arc<dyn SecretCleanupService>,
}

impl std::fmt::Debug for RebornProductAuthServicePorts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RebornProductAuthServicePorts")
            .field("flow_manager", &"Arc<dyn AuthFlowManager>")
            .field("interaction_service", &"Arc<dyn AuthInteractionService>")
            .field(
                "manual_token_flow_service",
                &"Arc<dyn RebornManualTokenFlowService>",
            )
            .field(
                "credential_setup_service",
                &"Arc<dyn CredentialSetupService>",
            )
            .field(
                "credential_account_service",
                &"Arc<dyn CredentialAccountService>",
            )
            .field(
                "credential_account_record_source",
                &"Arc<dyn CredentialAccountRecordSource>",
            )
            .field("provider_client", &"Arc<dyn AuthProviderClient>")
            .field("cleanup_service", &"Arc<dyn SecretCleanupService>")
            .finish()
    }
}

impl RebornProductAuthServicePorts {
    pub fn new(
        flow_manager: Arc<dyn AuthFlowManager>,
        interaction_service: Arc<dyn AuthInteractionService>,
        credential_setup_service: Arc<dyn CredentialSetupService>,
        credential_account_service: Arc<dyn CredentialAccountService>,
        provider_client: Arc<dyn AuthProviderClient>,
        cleanup_service: Arc<dyn SecretCleanupService>,
    ) -> Self {
        let manual_token_flow_service = Arc::new(PortBackedManualTokenFlowService::new(
            flow_manager.clone(),
            interaction_service.clone(),
        ));
        Self {
            flow_manager,
            interaction_service,
            manual_token_flow_service,
            credential_setup_service,
            credential_account_service,
            credential_account_record_source: Arc::new(UnsupportedCredentialAccountRecordSource),
            provider_client,
            cleanup_service,
        }
    }

    pub fn from_shared<T>(services: Arc<T>) -> Self
    where
        T: AuthFlowManager
            + AuthInteractionService
            + CredentialSetupService
            + CredentialAccountService
            + CredentialAccountRecordSource
            + AuthProviderClient
            + SecretCleanupService
            + RebornManualTokenFlowService
            + 'static,
    {
        let provider_client: Arc<dyn AuthProviderClient> = services.clone();
        Self::from_shared_with_provider(services, provider_client)
    }

    pub fn from_shared_with_provider<T>(
        services: Arc<T>,
        provider_client: Arc<dyn AuthProviderClient>,
    ) -> Self
    where
        T: AuthFlowManager
            + AuthInteractionService
            + CredentialSetupService
            + CredentialAccountService
            + CredentialAccountRecordSource
            + SecretCleanupService
            + RebornManualTokenFlowService
            + 'static,
    {
        let flow_manager: Arc<dyn AuthFlowManager> = services.clone();
        let interaction_service: Arc<dyn AuthInteractionService> = services.clone();
        let manual_token_flow_service: Arc<dyn RebornManualTokenFlowService> = services.clone();
        let credential_setup_service: Arc<dyn CredentialSetupService> = services.clone();
        let credential_account_service: Arc<dyn CredentialAccountService> = services.clone();
        let credential_account_record_source: Arc<dyn CredentialAccountRecordSource> =
            services.clone();
        let cleanup_service: Arc<dyn SecretCleanupService> = services;

        let mut ports = Self::new(
            flow_manager,
            interaction_service,
            credential_setup_service,
            credential_account_service,
            provider_client,
            cleanup_service,
        );
        ports.manual_token_flow_service = manual_token_flow_service;
        ports.credential_account_record_source = credential_account_record_source;
        ports
    }

    pub fn credential_account_service(&self) -> Arc<dyn CredentialAccountService> {
        self.credential_account_service.clone()
    }

    pub(crate) fn into_services(
        self,
        continuation_dispatcher: Arc<dyn RebornAuthContinuationDispatcher>,
    ) -> RebornProductAuthServices {
        RebornProductAuthServices::new(
            self.flow_manager,
            self.interaction_service,
            self.credential_setup_service,
            self.credential_account_service,
            self.provider_client,
            self.cleanup_service,
            continuation_dispatcher,
        )
        .with_manual_token_flow_service(self.manual_token_flow_service)
        .with_credential_account_record_source(self.credential_account_record_source)
    }

    pub fn with_provider_client(mut self, provider_client: Arc<dyn AuthProviderClient>) -> Self {
        self.credential_account_service = Arc::new(ProviderBackedCredentialAccountService::new(
            self.credential_account_service,
            self.credential_setup_service.clone(),
            provider_client.clone(),
        ));
        self.provider_client = provider_client;
        self
    }
}

/// Reborn product-auth service bundle exposed by the composition root.
///
/// This is the single composition seam for product-facing auth flows,
/// credential accounts, secure manual-token interactions, provider exchange,
/// and lifecycle cleanup. It deliberately exposes trait-shaped ports only:
/// WebUI/setup/extension callers should enter here instead of reaching into
/// lower auth stores, provider clients, or route-local state.
#[derive(Clone)]
pub struct RebornProductAuthServices {
    flow_manager: Arc<dyn AuthFlowManager>,
    interaction_service: Arc<dyn AuthInteractionService>,
    manual_token_flow_service: Arc<dyn RebornManualTokenFlowService>,
    credential_setup_service: Arc<dyn CredentialSetupService>,
    credential_account_service: Arc<dyn CredentialAccountService>,
    credential_account_record_source: Arc<dyn CredentialAccountRecordSource>,
    provider_client: Arc<dyn AuthProviderClient>,
    cleanup_service: Arc<dyn SecretCleanupService>,
    continuation_dispatcher: Arc<dyn RebornAuthContinuationDispatcher>,
    dcr_oauth_registry: Option<Arc<OAuthDcrProviderRegistry>>,
    oauth_gate_registry: Option<Arc<GoogleOAuthGateProviderRegistry>>,
    /// Optional read projection for WebUI/local-dev auth interactions.
    ///
    /// `RebornProductAuthServices` may still support OAuth callbacks,
    /// manual-token setup, credential refresh, and continuation dispatch
    /// without this port. When absent, runtime composition must expose the
    /// WebUI pending-auth interaction surface as explicitly unavailable
    /// instead of silently fabricating an unscoped read model.
    ///
    /// arch-exempt: optional Arc, durable auth-flow read projection is tracked
    /// by product-auth issue #4112 and remains genuinely optional until the
    /// durable backend exposes the same scoped projection as the in-memory port.
    flow_record_source: Option<Arc<dyn AuthFlowRecordSource>>,
    /// Shared acknowledged instance policy. Standalone host-only composition
    /// has no Monty owner and intentionally uses the canonical defaults.
    primitive_settings: Option<Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>>,
}

impl std::fmt::Debug for RebornProductAuthServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RebornProductAuthServices")
            .field("flow_manager", &"Arc<dyn AuthFlowManager>")
            .field("interaction_service", &"Arc<dyn AuthInteractionService>")
            .field(
                "manual_token_flow_service",
                &"Arc<dyn RebornManualTokenFlowService>",
            )
            .field(
                "credential_setup_service",
                &"Arc<dyn CredentialSetupService>",
            )
            .field(
                "credential_account_service",
                &"Arc<dyn CredentialAccountService>",
            )
            .field(
                "credential_account_record_source",
                &"Arc<dyn CredentialAccountRecordSource>",
            )
            .field("provider_client", &"Arc<dyn AuthProviderClient>")
            .field("cleanup_service", &"Arc<dyn SecretCleanupService>")
            .field(
                "continuation_dispatcher",
                &"Arc<dyn RebornAuthContinuationDispatcher>",
            )
            .field("flow_record_source", &self.flow_record_source.is_some())
            .field("dcr_oauth_registry", &self.dcr_oauth_registry.is_some())
            .field("oauth_gate_registry", &self.oauth_gate_registry.is_some())
            .finish()
    }
}

impl RebornProductAuthServices {
    #[cfg(all(feature = "postgres", feature = "skills-db"))]
    pub(crate) fn with_primitive_settings_source(
        mut self,
        source: Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>,
    ) -> Self {
        self.primitive_settings = Some(source);
        self
    }

    /// Only new standalone API inputs are checked here. Callers first recover
    /// authenticated originals; a policy edit never retimes an accepted flow.
    pub(crate) async fn validate_standalone_oauth_expiry(
        &self,
        expiry: chrono::DateTime<Utc>,
    ) -> Result<(), AuthProductError> {
        let limits = crate::oauth_provider_client::acknowledged_oauth_limits(
            self.primitive_settings.as_ref(),
        )
        .await?;
        let now = Utc::now();
        let maximum = now
            .checked_add_signed(chrono::Duration::seconds(i64::from(
                limits.oauth_setup_max_ttl_secs,
            )))
            .ok_or(AuthProductError::BackendUnavailable)?;
        if expiry <= now || expiry > maximum {
            return Err(AuthProductError::InvalidRequest {
                reason: "new standalone OAuth expiry exceeds the acknowledged policy".into(),
            });
        }
        Ok(())
    }

    pub(crate) async fn backend_timeout(&self) -> Result<std::time::Duration, AuthProductError> {
        let limits = crate::oauth_provider_client::acknowledged_oauth_limits(
            self.primitive_settings.as_ref(),
        )
        .await?;
        Ok(std::time::Duration::from_millis(u64::from(
            limits.auth_backend_timeout_millis,
        )))
    }

    /// Candidate for a new original only. The native original owner compares
    /// the complete request and keeps its first expiry on every repeat/CAS race.
    pub(crate) async fn manual_token_setup_expiry(
        &self,
    ) -> Result<chrono::DateTime<Utc>, AuthProductError> {
        let limits = crate::oauth_provider_client::acknowledged_oauth_limits(
            self.primitive_settings.as_ref(),
        )
        .await?;
        Utc::now()
            .checked_add_signed(chrono::Duration::seconds(i64::from(
                limits.manual_token_setup_ttl_secs,
            )))
            .ok_or(AuthProductError::BackendUnavailable)
    }

    /// Called after original recovery by the Extension credential adapter.
    /// A missing/unacknowledged configured owner must never choose a default.
    pub(crate) async fn extension_manual_token_setup_expiry(
        &self,
    ) -> Result<chrono::DateTime<Utc>, AuthProductError> {
        let limits = crate::oauth_provider_client::acknowledged_oauth_limits(
            self.primitive_settings.as_ref(),
        )
        .await?;
        Utc::now()
            .checked_add_signed(chrono::Duration::seconds(i64::from(
                limits.extension_manual_token_setup_ttl_secs,
            )))
            .ok_or(AuthProductError::BackendUnavailable)
    }

    pub fn new(
        flow_manager: Arc<dyn AuthFlowManager>,
        interaction_service: Arc<dyn AuthInteractionService>,
        credential_setup_service: Arc<dyn CredentialSetupService>,
        credential_account_service: Arc<dyn CredentialAccountService>,
        provider_client: Arc<dyn AuthProviderClient>,
        cleanup_service: Arc<dyn SecretCleanupService>,
        continuation_dispatcher: Arc<dyn RebornAuthContinuationDispatcher>,
    ) -> Self {
        let manual_token_flow_service = Arc::new(PortBackedManualTokenFlowService::new(
            flow_manager.clone(),
            interaction_service.clone(),
        ));
        Self {
            flow_manager,
            interaction_service,
            manual_token_flow_service,
            credential_setup_service,
            credential_account_service,
            credential_account_record_source: Arc::new(UnsupportedCredentialAccountRecordSource),
            provider_client,
            cleanup_service,
            continuation_dispatcher,
            dcr_oauth_registry: None,
            oauth_gate_registry: None,
            flow_record_source: None,
            primitive_settings: None,
        }
    }

    /// Builds a bundle from one object that implements every product-auth port.
    ///
    /// This is primarily for unified fakes such as
    /// [`InMemoryAuthProductServices`]. Production composition should prefer
    /// [`Self::new`] so storage, provider egress, interaction, and cleanup can
    /// be supplied by separate implementations.
    pub fn from_shared<T>(
        services: Arc<T>,
        continuation_dispatcher: Arc<dyn RebornAuthContinuationDispatcher>,
    ) -> Self
    where
        T: AuthFlowManager
            + AuthInteractionService
            + CredentialSetupService
            + CredentialAccountService
            + CredentialAccountRecordSource
            + AuthProviderClient
            + SecretCleanupService
            + RebornManualTokenFlowService
            + 'static,
    {
        let flow_manager: Arc<dyn AuthFlowManager> = services.clone();
        let interaction_service: Arc<dyn AuthInteractionService> = services.clone();
        let manual_token_flow_service: Arc<dyn RebornManualTokenFlowService> = services.clone();
        let credential_setup_service: Arc<dyn CredentialSetupService> = services.clone();
        let credential_account_service: Arc<dyn CredentialAccountService> = services.clone();
        let credential_account_record_source: Arc<dyn CredentialAccountRecordSource> =
            services.clone();
        let provider_client: Arc<dyn AuthProviderClient> = services.clone();
        let cleanup_service: Arc<dyn SecretCleanupService> = services;

        Self::new(
            flow_manager,
            interaction_service,
            credential_setup_service,
            credential_account_service,
            provider_client,
            cleanup_service,
            continuation_dispatcher,
        )
        .with_manual_token_flow_service(manual_token_flow_service)
        .with_credential_account_record_source(credential_account_record_source)
    }

    #[cfg(test)]
    pub fn from_shared_with_noop_dispatcher_for_tests<T>(services: Arc<T>) -> Self
    where
        T: AuthFlowManager
            + AuthInteractionService
            + CredentialSetupService
            + CredentialAccountService
            + CredentialAccountRecordSource
            + AuthProviderClient
            + SecretCleanupService
            + RebornManualTokenFlowService
            + 'static,
    {
        Self::from_shared(services, Arc::new(NoopAuthContinuationDispatcher))
    }

    pub fn flow_manager(&self) -> Arc<dyn AuthFlowManager> {
        self.flow_manager.clone()
    }

    /// Auth-flow read projection used only by product/WebUI interaction views.
    ///
    /// `None` is an intentional unsupported mode for bundles that can perform
    /// product-auth side effects but do not provide a scoped pending-auth
    /// projection. Callers must map it to a stable unavailable surface.
    pub(crate) fn flow_record_source(&self) -> Option<Arc<dyn AuthFlowRecordSource>> {
        self.flow_record_source.clone()
    }

    pub fn interaction_service(&self) -> Arc<dyn AuthInteractionService> {
        self.interaction_service.clone()
    }

    pub fn credential_setup_service(&self) -> Arc<dyn CredentialSetupService> {
        self.credential_setup_service.clone()
    }

    pub fn credential_account_service(&self) -> Arc<dyn CredentialAccountService> {
        self.credential_account_service.clone()
    }

    pub(crate) fn credential_account_record_source(
        &self,
    ) -> Arc<dyn CredentialAccountRecordSource> {
        self.credential_account_record_source.clone()
    }

    pub(crate) fn runtime_credential_account_selection_service(
        &self,
    ) -> Arc<dyn RuntimeCredentialAccountSelectionService> {
        Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(
            self.credential_account_record_source(),
        ))
    }

    pub fn provider_client(&self) -> Arc<dyn AuthProviderClient> {
        self.provider_client.clone()
    }

    pub fn cleanup_service(&self) -> Arc<dyn SecretCleanupService> {
        self.cleanup_service.clone()
    }

    pub fn with_provider_client(mut self, provider_client: Arc<dyn AuthProviderClient>) -> Self {
        self.credential_account_service = Arc::new(ProviderBackedCredentialAccountService::new(
            self.credential_account_service,
            self.credential_setup_service.clone(),
            provider_client.clone(),
        ));
        self.provider_client = provider_client;
        self
    }

    pub(crate) fn with_dcr_oauth_registry(
        mut self,
        registry: Arc<OAuthDcrProviderRegistry>,
    ) -> Self {
        self.dcr_oauth_registry = Some(registry);
        self
    }

    pub(crate) fn with_oauth_gate_registry(
        mut self,
        registry: Arc<GoogleOAuthGateProviderRegistry>,
    ) -> Self {
        self.oauth_gate_registry = Some(registry);
        self
    }

    /// Attach a concrete shared owner for original setup/publication custody.
    /// Composing unrelated lower ports does not establish that contract.
    pub fn with_manual_token_flow_service(
        mut self,
        service: Arc<dyn RebornManualTokenFlowService>,
    ) -> Self {
        self.manual_token_flow_service = service;
        self
    }

    /// Tests only: the exact native owner attached by the production Auth factory.
    /// Enables acknowledgement loss after its real publication, before the facade
    /// dispatches continuation; it creates no replacement store or authority.
    #[cfg(all(test, feature = "skills-db"))]
    pub(crate) fn manual_token_flow_service_for_tests(
        &self,
    ) -> Arc<dyn RebornManualTokenFlowService> {
        self.manual_token_flow_service.clone()
    }

    fn with_credential_account_record_source(
        mut self,
        source: Arc<dyn CredentialAccountRecordSource>,
    ) -> Self {
        self.credential_account_record_source = source;
        self
    }

    pub fn with_continuation_dispatcher(
        mut self,
        dispatcher: Arc<dyn RebornAuthContinuationDispatcher>,
    ) -> Self {
        self.continuation_dispatcher = dispatcher;
        self
    }

    /// Enable WebUI/local-dev auth-flow projection source.
    ///
    /// Exported `pub` so integration-test harnesses outside the crate can
    /// wire an in-memory fake. Not part of the stable product API — do not
    /// call this from production composition paths; use `as_auth_challenge_provider()`
    /// only when `product_auth` exposes a `flow_record_source` via the bundle.
    #[doc(hidden)]
    pub fn with_flow_record_source(mut self, source: Arc<dyn AuthFlowRecordSource>) -> Self {
        self.flow_record_source = Some(source);
        self
    }

    /// Expose this service as an `Arc<dyn AuthChallengeProvider>` so product
    /// surfaces can enrich `AuthPromptView` payloads with `challenge_kind`,
    /// `provider`, `account_label`, and `authorization_url`.
    ///
    /// Returns `None` when no `flow_record_source` is configured (meaning this
    /// bundle was built without the in-memory projection source, e.g. in
    /// production deployments that use durable DB backends not yet wired to
    /// `AuthFlowRecordSource`). Product auth prompts fall back to the plain
    /// 4-field view in that case, which is backward-compatible.
    #[doc(hidden)]
    pub fn as_auth_challenge_provider(self: &Arc<Self>) -> Option<Arc<dyn AuthChallengeProvider>> {
        if self.flow_record_source.is_some() {
            Some(Arc::clone(self) as Arc<dyn AuthChallengeProvider>)
        } else {
            None
        }
    }

    /// Refresh a credential account through the injected product-auth port.
    ///
    /// Concrete account services own the durable account update and provider
    /// egress wiring; callers enter here so WebUI/setup/lifecycle code does not
    /// reconstruct refresh authority locally.
    pub async fn refresh_credential_account(
        &self,
        request: CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, RebornCredentialLifecycleError> {
        self.credential_account_service
            .refresh_account(request)
            .await
            .map_err(RebornCredentialLifecycleError::from)
    }

    /// List redacted credential account projections through the injected
    /// account port.
    ///
    /// Routes/CLIs/extensions enter here so they never bypass the account
    /// port's grant filtering, status redaction, or extension-scoped
    /// visibility rules.
    pub async fn list_credential_accounts(
        &self,
        request: CredentialAccountListRequest,
    ) -> Result<CredentialAccountListPage, RebornCredentialLifecycleError> {
        self.credential_account_service
            .list_accounts(request)
            .await
            .map_err(RebornCredentialLifecycleError::from)
    }

    /// Select a single configured credential account through the injected
    /// account port.
    pub async fn select_credential_account(
        &self,
        request: CredentialAccountChoiceRequest,
    ) -> Result<CredentialAccountProjection, RebornCredentialLifecycleError> {
        self.credential_account_service
            .select_configured_account(request)
            .await
            .map_err(RebornCredentialLifecycleError::from)
    }

    /// Project the stable credential recovery state for a provider through
    /// the injected account port. The projection drives WebUI/CLI/API
    /// recovery, refresh, and reauthorize prompts without exposing backend
    /// errors or secret handles.
    pub async fn project_credential_recovery(
        &self,
        request: CredentialRecoveryRequest,
    ) -> Result<CredentialRecoveryProjection, RebornCredentialLifecycleError> {
        self.credential_account_service
            .project_credential_recovery(request)
            .await
            .map_err(RebornCredentialLifecycleError::from)
    }

    /// Apply ownership-aware credential cleanup for extension lifecycle events.
    ///
    /// This facade keeps lifecycle callers on the Reborn product-auth boundary
    /// instead of depending on V1 extension-manager cleanup or route-local
    /// secret authority.
    pub async fn cleanup_credentials_for_lifecycle(
        &self,
        request: SecretCleanupRequest,
    ) -> Result<SecretCleanupReport, RebornCredentialLifecycleError> {
        self.cleanup_service
            .cleanup_for_lifecycle(request)
            .await
            .map_err(RebornCredentialLifecycleError::from)
    }

    /// Resume only the retained explicit callback. This does not need a current
    /// provider configuration or the one-shot PKCE verifier that was consumed.
    pub(crate) async fn resume_oauth_callback_original(
        &self,
        reference: &brassclaw_auth::OAuthCallbackReference,
    ) -> Result<Option<RebornOAuthCallbackResponse>, RebornOAuthCallbackError> {
        let Some(original) = self
            .flow_manager
            .read_oauth_callback_original(&reference.scope, reference)
            .await
            .map_err(RebornOAuthCallbackError::from)?
        else {
            return Ok(None);
        };
        let flow = self
            .resolve_callback_original(original)
            .await
            .map_err(RebornOAuthCallbackError::from)?;
        self.finish_oauth_callback(flow).await.map(Some)
    }

    async fn resolve_callback_original(
        &self,
        original: brassclaw_auth::OAuthCallbackState,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let state = if original.action.needs_publication() {
            self.flow_manager
                .reconcile_oauth_callback(
                    &original.action.original.scope,
                    &original.action.original.reference(),
                )
                .await?
        } else {
            original
        };
        state.completed_flow()
    }

    async fn finish_oauth_callback(
        &self,
        completed: AuthFlowRecord,
    ) -> Result<RebornOAuthCallbackResponse, RebornOAuthCallbackError> {
        self.retire_google_original_if_terminal(&completed)
            .await
            .map_err(RebornOAuthCallbackError::from)?;
        let completed = self
            .dispatch_completed_continuation(completed)
            .await
            .map_err(RebornOAuthCallbackError::from)?;
        Ok(RebornOAuthCallbackResponse {
            flow_id: completed.id,
            status: completed.status,
            credential_account_id: completed.credential_account_id,
            continuation: completed.continuation,
        })
    }

    pub async fn handle_oauth_callback(
        &self,
        request: RebornOAuthCallbackRequest,
    ) -> Result<RebornOAuthCallbackResponse, RebornOAuthCallbackError> {
        let completed = match request.outcome {
            RebornOAuthCallbackOutcome::Authorized { provider_request } => {
                let original = brassclaw_auth::OAuthCallbackOriginal::from_request(
                    request.scope.clone(),
                    request.flow_id,
                    request.opaque_state_hash.clone(),
                    &provider_request,
                )
                .map_err(RebornOAuthCallbackError::from)?;
                let reference = original.reference();
                let admission = match self
                    .flow_manager
                    .read_oauth_callback_original(&request.scope, &reference)
                    .await
                    .map_err(RebornOAuthCallbackError::from)?
                {
                    Some(state) => {
                        state
                            .action
                            .original
                            .verify(&original)
                            .map_err(RebornOAuthCallbackError::from)?;
                        brassclaw_auth::OAuthCallbackAdmission::Original(state)
                    }
                    None => {
                        let context = OAuthProviderExchangeContext {
                            scope: request.scope.clone(),
                            flow_id: request.flow_id,
                        };
                        let destinations = self
                            .provider_client
                            .prepare_callback(&context, &provider_request)
                            .await
                            .map_err(RebornOAuthCallbackError::from)?;
                        self.flow_manager
                            .claim_oauth_callback(
                                &request.scope,
                                OAuthCallbackClaimRequest {
                                    original: original.clone(),
                                    destinations,
                                },
                            )
                            .await
                            .map_err(RebornOAuthCallbackError::from)?
                    }
                };
                match admission {
                    brassclaw_auth::OAuthCallbackAdmission::Original(state) => {
                        state
                            .action
                            .original
                            .verify(&original)
                            .map_err(RebornOAuthCallbackError::from)?;
                        self.resolve_callback_original(state)
                            .await
                            .map_err(RebornOAuthCallbackError::from)?
                    }
                    brassclaw_auth::OAuthCallbackAdmission::Dispatch(state) => {
                        // Exactly this newly owned durable admission may call the
                        // issuer. All recovery below reads the same original.
                        state
                            .action
                            .original
                            .verify(&original)
                            .map_err(RebornOAuthCallbackError::from)?;
                        let dispatched = state.action;
                        let write = match self
                            .provider_client
                            .exchange_callback(
                                OAuthProviderExchangeContext {
                                    scope: request.scope.clone(),
                                    flow_id: request.flow_id,
                                },
                                provider_request,
                            )
                            .await
                        {
                            Ok(exchange) => {
                                self.flow_manager
                                    .complete_oauth_callback(
                                        &request.scope,
                                        OAuthCallbackInput {
                                            dispatched: Some(dispatched),
                                            flow_id: request.flow_id,
                                            opaque_state_hash: request.opaque_state_hash,
                                            outcome: ProviderCallbackOutcome::Authorized {
                                                exchange,
                                            },
                                        },
                                    )
                                    .await
                            }
                            Err(error) => self
                                .flow_manager
                                .fail_oauth_callback(
                                    &request.scope,
                                    OAuthCallbackFailureInput {
                                        dispatched: Some(dispatched),
                                        flow_id: request.flow_id,
                                        opaque_state_hash: request.opaque_state_hash,
                                        error: error.code(),
                                    },
                                )
                                .await
                                .and(Err(error)),
                        };
                        // A failed acknowledgement never proves unpublished
                        // credentials. Keep the original artifacts; no cleanup
                        // or second issuer submission follows an uncertain write.
                        let stored = self
                            .flow_manager
                            .read_oauth_callback_original(&request.scope, &reference)
                            .await
                            .map_err(RebornOAuthCallbackError::from)?
                            .ok_or(RebornOAuthCallbackError::from(
                                AuthProductError::BackendUnavailable,
                            ))?;
                        if matches!(
                            stored.action.outcome,
                            brassclaw_auth::OAuthCallbackOutcome::Submitted
                        ) {
                            return Err(write
                                .err()
                                .unwrap_or(AuthProductError::BackendUnavailable)
                                .into());
                        }
                        self.resolve_callback_original(stored)
                            .await
                            .map_err(RebornOAuthCallbackError::from)?
                    }
                }
            }
            RebornOAuthCallbackOutcome::ProviderDenied => self
                .flow_manager
                .complete_oauth_callback(
                    &request.scope,
                    OAuthCallbackInput {
                        dispatched: None,
                        flow_id: request.flow_id,
                        opaque_state_hash: request.opaque_state_hash,
                        outcome: ProviderCallbackOutcome::Denied,
                    },
                )
                .await
                .map_err(RebornOAuthCallbackError::from)?,
            RebornOAuthCallbackOutcome::Malformed => {
                return Err(AuthProductError::MalformedCallback.into());
            }
        };
        self.finish_oauth_callback(completed).await
    }

    /// A public callback may release its cached verifier only after proving
    /// ownership of the exact durable OAuth flow. Error codes alone do not
    /// establish ownership or a terminal outcome; no expiry or configuration
    /// lookup may turn a still-pending original into cleanup authority.
    pub(crate) async fn oauth_callback_flow_is_terminal(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        state_hash: &OpaqueStateHash,
    ) -> Result<bool, RebornOAuthCallbackError> {
        let record = self
            .flow_manager
            .get_flow(scope, flow_id)
            .await
            .map_err(RebornOAuthCallbackError::from)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if record.id != flow_id
            || !scope_matches(scope, &record.scope)
            || !record
                .opaque_state_hash
                .as_ref()
                .is_some_and(|expected| expected.constant_time_eq(state_hash))
        {
            return Err(AuthProductError::CrossScopeDenied.into());
        }
        if !matches!(record.challenge, Some(AuthChallenge::OAuthUrl { .. })) {
            return Err(AuthProductError::MalformedCallback.into());
        }
        Ok(brassclaw_auth::is_terminal_status(record.status))
    }

    #[allow(dead_code, reason = "used by upcoming Reborn OAuth setup route wiring")]
    pub(crate) async fn ensure_oauth_callback_flow_known(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<AuthProviderId, RebornOAuthCallbackError> {
        let Some(record) = self
            .flow_manager
            .get_flow(scope, flow_id)
            .await
            .map_err(RebornOAuthCallbackError::from)?
        else {
            return Err(AuthProductError::UnknownOrExpiredFlow.into());
        };
        if record.expires_at <= Utc::now() {
            return Err(AuthProductError::UnknownOrExpiredFlow.into());
        }
        Ok(record.provider)
    }

    #[allow(
        dead_code,
        reason = "used by the webui-v2-beta OAuth callback route when DCR fallback PKCE storage is enabled"
    )]
    pub(crate) async fn oauth_pkce_verifier_for_flow(
        &self,
        scope: &AuthProductScope,
        provider: &AuthProviderId,
        flow_id: AuthFlowId,
    ) -> Result<Option<SecretString>, RebornOAuthCallbackError> {
        if let Some(registry) = &self.oauth_gate_registry
            && let Some(pkce) = registry
                .pkce_verifier_for_flow(&self.flow_manager, scope, provider, flow_id)
                .await
                .map_err(RebornOAuthCallbackError::from)?
        {
            return Ok(Some(pkce));
        }
        let Some(registry) = &self.dcr_oauth_registry else {
            return Ok(None);
        };
        registry
            .pkce_verifier_for_flow(&self.flow_manager, scope, provider, flow_id)
            .await
            .map_err(RebornOAuthCallbackError::from)
    }

    /// Capture one explicit OAuth usage/target before its first challenge.
    /// The source declaration is supplied by the existing Product resolver;
    /// original recovery never calls this method against today's head/account.
    pub(crate) async fn prepare_extension_oauth_requirement(
        &self,
        scope: &AuthProductScope,
        requirement: brassclaw_host_api::RuntimeCredentialAccountRequirement,
        requested_account: Option<CredentialAccountId>,
    ) -> Result<
        (
            brassclaw_auth::CredentialRequirementSetup,
            Option<CredentialAccountUpdateBinding>,
        ),
        AuthProductError,
    > {
        let brassclaw_host_api::RuntimeCredentialRequirementSource::ProductAuthAccount {
            provider,
            setup: brassclaw_host_api::RuntimeCredentialAccountSetup::OAuth { scopes },
        } = &requirement.requirement().source
        else {
            return Err(AuthProductError::BackendConflict);
        };
        let provider = AuthProviderId::new(provider.as_str())?;
        let scopes = scopes
            .iter()
            .cloned()
            .map(ProviderScope::new)
            .collect::<Result<Vec<_>, _>>()?;
        let previous = self
            .credential_account_record_source
            .requirement_selection(scope, &requirement)
            .await?;
        let binding = if let Some(account_id) = requested_account {
            let account = self
                .credential_account_service
                .get_account(
                    brassclaw_auth::CredentialAccountLookupRequest::new(scope.clone(), account_id)
                        .for_extension(requirement.requester_extension().clone()),
                )
                .await?
                .ok_or(AuthProductError::CredentialMissing)?;
            if !scope_matches(scope, &account.scope)
                || account.provider != provider
                || !account.is_authorized_for_requester(Some(requirement.requester_extension()))
            {
                return Err(AuthProductError::CrossScopeDenied);
            }
            Some(CredentialAccountUpdateBinding::from_projection(
                &account.projection(),
            ))
        } else {
            None
        };
        let setup = brassclaw_auth::CredentialRequirementSetup::new(scope, requirement, previous)?;
        setup.verify_oauth(scope, &provider, &scopes)?;
        Ok((setup, binding))
    }

    /// Validate browser correlation against immutable stored usage, never a
    /// current manifest. Legacy originals remain metadata-only, without backfill.
    pub(crate) async fn verify_extension_oauth_original(
        &self,
        scope: &AuthProductScope,
        flow: &AuthFlowRecord,
        requester: &brassclaw_host_api::ExtensionId,
        requirement_name: Option<&str>,
        target: Option<&brassclaw_product_workflow::ExtensionCredentialTarget>,
    ) -> Result<(), AuthProductError> {
        let setup = self.flow_manager.requirement_setup(scope, flow.id).await?;
        if let Some(setup) = &setup {
            setup.verify_flow(flow)?;
        }
        verify_extension_oauth_selection(
            setup.as_ref(),
            flow.update_binding.as_ref(),
            requester,
            requirement_name,
            target,
        )
    }

    /// Recover only the accepted Google setup, without current provider/account
    /// selection or fresh PKCE preparation. SetupOnly is never a task grant.
    async fn recover_google_setup_original_legacy(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        label: &CredentialAccountLabel,
        scopes: &[ProviderScope],
        expires_at: Timestamp,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let Some(flow) = self.flow_manager.get_flow(scope, flow_id).await? else {
            return Ok(None);
        };
        if flow.scope != *scope
            || flow.kind != AuthFlowKind::IntegrationCredential
            || flow.provider.as_str() != brassclaw_auth::GOOGLE_PROVIDER_ID
            || flow.continuation != AuthContinuationRef::SetupOnly
            || flow.expires_at != expires_at
        {
            return Err(AuthProductError::BackendConflict);
        }
        let Some(AuthChallenge::OAuthUrl {
            authorization_url,
            expires_at: challenge_expiry,
        }) = &flow.challenge
        else {
            return Err(AuthProductError::BackendConflict);
        };
        if *challenge_expiry != flow.expires_at {
            return Err(AuthProductError::BackendConflict);
        }
        let url = url::Url::parse(authorization_url.as_str())
            .map_err(|_| AuthProductError::BackendConflict)?;
        let mut states = url.query_pairs().filter(|(name, _)| name == "state");
        let (_, original_state) = states.next().ok_or(AuthProductError::BackendConflict)?;
        if states.next().is_some()
            || flow.opaque_state_hash.as_ref()
                != Some(&brassclaw_auth::opaque_state_hash(&original_state)?)
        {
            return Err(AuthProductError::BackendConflict);
        }
        let decoded = brassclaw_auth::GoogleOAuthCallbackState::decode(&original_state)?;
        if decoded.flow_id() != flow_id
            || decoded.scope().resource != scope.resource
            || decoded.scope().session_id != scope.session_id
            || decoded.account_label() != label
            || decoded.requested_scopes() != scopes
        {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(setup) = self.flow_manager.requirement_setup(scope, flow_id).await? {
            setup.verify_oauth(scope, &flow.provider, decoded.requested_scopes())?;
        }
        Ok(Some(flow))
    }

    /// Cancel/reconcile this exact setup. A completed callback wins; readback
    /// cannot authorize a new setup, issuer request or continuation.
    async fn cancel_google_setup_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        label: &CredentialAccountLabel,
        scopes: &[ProviderScope],
        expires_at: Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let original = self
            .recover_google_setup_original(scope, flow_id, label, scopes, expires_at)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if brassclaw_auth::is_terminal_status(original.status) {
            return Ok(original);
        }
        match self.flow_manager.cancel_flow(scope, flow_id).await {
            Ok(flow) => Ok(flow),
            Err(error) => match self
                .recover_google_setup_original(scope, flow_id, label, scopes, expires_at)
                .await?
            {
                Some(actual) if brassclaw_auth::is_terminal_status(actual.status) => Ok(actual),
                _ => Err(error),
            },
        }
    }

    #[allow(dead_code, reason = "used by upcoming Reborn OAuth setup route wiring")]
    pub(crate) async fn start_setup_oauth_flow(
        &self,
        request: RebornOAuthStartFlowRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.flow_manager
            .create_flow(NewAuthFlow {
                requirement_setup: request.requirement_setup,
                id: request.flow_id,
                scope: request.scope,
                kind: AuthFlowKind::IntegrationCredential,
                provider: request.provider,
                challenge: AuthChallenge::OAuthUrl {
                    authorization_url: request.authorization_url,
                    expires_at: request.expires_at,
                },
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: request.update_binding,
                opaque_state_hash: Some(request.opaque_state_hash),
                pkce_verifier_hash: Some(request.pkce_verifier_hash),
                expires_at: request.expires_at,
            })
            .await
    }

    #[allow(
        dead_code,
        reason = "used by the webui-v2-beta extension OAuth route through product-auth route composition"
    )]
    /// Cancel this already retained original only. A terminal callback wins;
    /// lost acknowledgement is reconciled by exact original readback.
    pub(crate) async fn cancel_dcr_setup_original(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let modern = self
            .flow_manager
            .read_dcr_registration(scope, flow_id)
            .await?;
        let Some(original) = modern else {
            let flow = self
                .flow_manager
                .get_flow(scope, flow_id)
                .await?
                .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
            let registry = self
                .dcr_oauth_registry
                .as_ref()
                .ok_or(AuthProductError::BackendUnavailable)?;
            let selected = registry.legacy_definition(&flow).await?;
            if brassclaw_auth::is_terminal_status(flow.status) {
                return Ok(flow);
            }
            return match self.flow_manager.cancel_flow(scope, flow_id).await {
                Ok(flow) => {
                    selected.verify_flow(&flow)?;
                    Ok(flow)
                }
                Err(error) => {
                    let actual = self
                        .flow_manager
                        .get_flow(scope, flow_id)
                        .await?
                        .ok_or(AuthProductError::BackendUnavailable)?;
                    selected.verify_flow(&actual)?;
                    if brassclaw_auth::is_terminal_status(actual.status) {
                        Ok(actual)
                    } else {
                        Err(error)
                    }
                }
            };
        };
        if let Err(error) = self.flow_manager.cancel_flow(scope, flow_id).await {
            let actual = self
                .flow_manager
                .read_dcr_registration(scope, flow_id)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            actual.action.original.verify(&original.action.original)?;
            if !brassclaw_auth::is_terminal_status(actual.flow.status) {
                return Err(error);
            }
        }
        let actual = self
            .flow_manager
            .read_dcr_registration(scope, flow_id)
            .await?
            .ok_or(AuthProductError::BackendUnavailable)?;
        actual.action.original.verify(&original.action.original)?;
        if actual.flow.status == AuthFlowStatus::Completed {
            return Ok(actual.flow);
        }
        self.dcr_oauth_registry
            .as_ref()
            .ok_or(AuthProductError::BackendUnavailable)?
            .cleanup_original(&self.flow_manager, actual)
            .await
    }

    pub(crate) async fn dcr_cleanup_status(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<brassclaw_auth::DcrCleanupProjection, AuthProductError> {
        if let Some(state) = self
            .flow_manager
            .read_dcr_registration(scope, flow_id)
            .await?
        {
            return Ok(state.cleanup_projection());
        }
        let flow = self
            .flow_manager
            .get_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        self.dcr_oauth_registry
            .as_ref()
            .ok_or(AuthProductError::BackendUnavailable)?
            .legacy_definition(&flow)
            .await?
            .cleanup_projection(&flow)
    }

    /// Recover the first definition before current declaration selection. A
    /// missing Auth projection may be repaired only from its authenticated
    /// encrypted preparation, after checking the complete explicit correlation.
    /// This never dispatches registration, cleanup HTTP or fresh preparation.
    pub(crate) async fn recover_dcr_setup_original(
        &self,
        request: RebornDcrSetupOriginalLookup<'_>,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        if let Some(state) = self
            .flow_manager
            .read_dcr_registration(request.scope, request.flow_id)
            .await?
        {
            request.verify_preparation(&state.action.original)?;
            return Ok(Some(state.flow));
        }
        if let Some(flow) = self
            .flow_manager
            .get_flow(request.scope, request.flow_id)
            .await?
        {
            if flow.kind != AuthFlowKind::IntegrationCredential
                || &flow.provider != request.provider
                || flow.continuation != AuthContinuationRef::SetupOnly
                || flow.expires_at != request.expires_at
            {
                return Err(AuthProductError::BackendConflict);
            }
            let original = self
                .dcr_oauth_registry
                .as_ref()
                .ok_or(AuthProductError::BackendUnavailable)?
                .legacy_definition(&flow)
                .await?;
            if &original.provider != request.provider
                || &original.account_label != request.account_label
                || original.requested_scopes.as_slice() != request.provider_scopes
                || original.continuation != AuthContinuationRef::SetupOnly
            {
                return Err(AuthProductError::BackendConflict);
            }
            self.verify_extension_oauth_original(
                request.scope,
                &flow,
                request.requester_extension,
                request.requirement_name,
                request.target,
            )
            .await?;
            return Ok(Some(flow));
        }
        let Some(registry) = &self.dcr_oauth_registry else {
            return Ok(None);
        };
        let Some(original) = registry
            .retained_setup_preparation(request.scope, request.flow_id, request.provider)
            .await?
        else {
            return Ok(None);
        };
        request.verify_preparation(&original)?;
        let restored = if request.cancel_preparation || original.expires_at <= chrono::Utc::now() {
            let reason = if request.cancel_preparation {
                brassclaw_auth::DcrCleanupReason::Canceled
            } else {
                brassclaw_auth::DcrCleanupReason::Expired
            };
            self.flow_manager
                .retire_dcr_preparation(original, reason)
                .await?
        } else {
            self.flow_manager.prepare_dcr_registration(original).await?
        };
        request.verify_preparation(&restored.action.original)?;
        Ok(Some(restored.flow))
    }

    pub(crate) async fn start_dcr_setup_oauth_flow(
        &self,
        request: RebornDcrOAuthStartFlowRequest,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let Some(registry) = &self.dcr_oauth_registry else {
            return Ok(None);
        };
        registry
            .start_setup_flow(
                &self.flow_manager,
                DcrSetupFlowRequest {
                    requirement_setup: request.requirement_setup,
                    flow_id: request.flow_id,
                    scope: request.scope,
                    provider: request.provider,
                    account_label: request.account_label,
                    provider_scopes: request.provider_scopes,
                    update_binding: request.update_binding,
                    expires_at: request.expires_at,
                },
            )
            .await
    }

    pub async fn request_manual_token_setup(
        &self,
        request: RebornManualTokenSetupRequest,
    ) -> Result<RebornManualTokenChallenge, RebornManualTokenError> {
        let challenge = self
            .manual_token_flow_service
            .request_manual_token_flow(ManualTokenSetupRequest {
                requirement_setup: request.requirement_setup,
                scope: request.scope,
                provider: request.provider,
                label: request.label,
                continuation: request.continuation,
                update_binding: request.update_binding,
                expires_at: request.expires_at,
            })
            .await
            .map_err(RebornManualTokenError::from)?;

        match challenge {
            brassclaw_auth::AuthChallenge::ManualTokenRequired {
                interaction_id,
                provider,
                label,
                expires_at,
            } => Ok(RebornManualTokenChallenge {
                interaction_id,
                provider,
                label,
                expires_at,
            }),
            _ => Err(AuthProductError::InvalidRequest {
                reason: "manual token setup returned an unexpected challenge".to_string(),
            }
            .into()),
        }
    }

    /// Metadata recovery for a consumer that lost its original setup receipt.
    /// This cannot create a setup, prove raw input or resume a task. The consumer
    /// must compare its provider/label/continuation/original account selection
    /// before reusing the retained binding for the explicit original operation.
    pub(crate) async fn recover_original_manual_token_setup(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, RebornManualTokenError> {
        let Some(flow) = self
            .manual_token_flow_service
            .recover_original_manual_token_flow(scope, identity)
            .await
            .map_err(RebornManualTokenError::from)?
        else {
            return Ok(None);
        };
        let Some(AuthChallenge::ManualTokenRequired { label, .. }) = flow.challenge.as_ref() else {
            return Err(AuthProductError::BackendConflict.into());
        };
        let original = ManualTokenSetupRequest {
            // Flow-only structural projection. The original setup owner has
            // verified its custody; usage consumers read the retained setup
            // through AuthFlowManager instead of inferring it from this DTO.
            requirement_setup: None,
            scope: scope.clone(),
            provider: flow.provider.clone(),
            label: label.clone(),
            continuation: flow.continuation.clone(),
            update_binding: flow.update_binding.clone(),
            expires_at: flow.expires_at,
        };
        brassclaw_auth::domain::validate_original_manual_setup_flow(
            identity, &original, &original, &flow,
        )
        .map_err(RebornManualTokenError::from)?;
        Ok(Some(flow))
    }

    pub async fn request_original_manual_token_setup(
        &self,
        identity: ManualTokenSetupIdentity,
        request: RebornManualTokenSetupRequest,
    ) -> Result<RebornManualTokenSetupReceipt, RebornManualTokenError> {
        let requested = ManualTokenSetupRequest {
            requirement_setup: request.requirement_setup,
            scope: request.scope,
            provider: request.provider,
            label: request.label,
            continuation: request.continuation,
            update_binding: request.update_binding,
            expires_at: request.expires_at,
        };
        let flow = self
            .manual_token_flow_service
            .request_original_manual_token_flow(identity, requested.clone())
            .await
            .map_err(RebornManualTokenError::from)?;
        let mut original = requested.clone();
        original.expires_at = flow.expires_at;
        brassclaw_auth::domain::validate_original_manual_setup_flow(
            identity, &original, &requested, &flow,
        )
        .map_err(RebornManualTokenError::from)?;
        if flow.status == AuthFlowStatus::AwaitingUser && flow.expires_at <= Utc::now() {
            return Err(AuthProductError::UnknownOrExpiredFlow.into());
        }
        let Some(AuthChallenge::ManualTokenRequired {
            interaction_id,
            provider,
            label,
            expires_at,
        }) = flow.challenge
        else {
            return Err(AuthProductError::BackendConflict.into());
        };
        Ok(RebornManualTokenSetupReceipt {
            identity,
            status: flow.status,
            challenge: RebornManualTokenChallenge {
                interaction_id,
                provider,
                label,
                expires_at,
            },
        })
    }

    pub async fn submit_original_manual_token(
        &self,
        identity: ManualTokenSetupIdentity,
        request: RebornManualTokenSubmitRequest,
    ) -> Result<RebornManualTokenSubmitResponse, RebornManualTokenError> {
        if request.interaction_id != identity.interaction_id {
            return Err(AuthProductError::BackendConflict.into());
        }
        let scope = request.scope;
        let (result, completed) = self
            .manual_token_flow_service
            .submit_original_manual_token_flow(&scope, identity, request.secret)
            .await
            .map_err(RebornManualTokenError::from)?;
        Self::validate_original_manual_completion(&scope, identity, &result, &completed)?;
        self.dispatch_completed_continuation(completed)
            .await
            .map_err(RebornManualTokenError::from)?;
        Ok(RebornManualTokenSubmitResponse {
            account_id: result.account_id,
            status: result.status,
            continuation: result.continuation,
        })
    }

    fn validate_original_manual_completion(
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
        result: &brassclaw_auth::SecretSubmitResult,
        completed: &AuthFlowRecord,
    ) -> Result<(), RebornManualTokenError> {
        if completed.id != identity.flow_id
            || !scope_matches(scope, &completed.scope)
            || completed.kind != AuthFlowKind::IntegrationCredential
            || completed.status != AuthFlowStatus::Completed
            || completed.error.is_some()
            || completed.credential_account_id != Some(result.account_id)
            || completed.continuation != result.continuation
            || result.status != CredentialAccountStatus::Configured
            || !matches!(&completed.challenge,Some(AuthChallenge::ManualTokenRequired{interaction_id,..}) if *interaction_id==identity.interaction_id)
        {
            return Err(AuthProductError::BackendConflict.into());
        }
        Ok(())
    }

    pub async fn continue_original_manual_token(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<RebornManualTokenSubmitResponse, RebornManualTokenError> {
        let (result, completed) = self
            .manual_token_flow_service
            .continue_original_manual_token_flow(scope, identity)
            .await
            .map_err(RebornManualTokenError::from)?;
        Self::validate_original_manual_completion(scope, identity, &result, &completed)?;
        self.dispatch_completed_continuation(completed)
            .await
            .map_err(RebornManualTokenError::from)?;
        Ok(RebornManualTokenSubmitResponse {
            account_id: result.account_id,
            status: result.status,
            continuation: result.continuation,
        })
    }

    pub async fn submit_manual_token(
        &self,
        request: RebornManualTokenSubmitRequest,
    ) -> Result<RebornManualTokenSubmitResponse, RebornManualTokenError> {
        let scope = request.scope;
        let interaction_id = request.interaction_id;
        // Raw acceptance always belongs to the original-input owner. A completed
        // flow or today's account cannot prove that this submitted token matches.
        let (result, completed) = self
            .manual_token_flow_service
            .submit_manual_token_flow(
                &scope,
                SecretSubmitRequest {
                    interaction_id,
                    secret: request.secret,
                },
            )
            .await
            .map_err(RebornManualTokenError::from)?;
        self.dispatch_completed_continuation(completed)
            .await
            .map_err(RebornManualTokenError::from)?;

        Ok(RebornManualTokenSubmitResponse {
            account_id: result.account_id,
            status: result.status,
            continuation: result.continuation,
        })
    }

    pub async fn abandon_manual_token(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<bool, RebornManualTokenError> {
        self.manual_token_flow_service
            .abandon_manual_token_flow(scope, interaction_id)
            .await
            .map_err(RebornManualTokenError::from)
    }

    async fn dispatch_completed_continuation(
        &self,
        completed: AuthFlowRecord,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        // Check before the dispatcher has any effect, including already marked
        // records. A denied/failed/unknown flow cannot emit a success event.
        if completed.status != AuthFlowStatus::Completed {
            return Err(AuthProductError::FlowAlreadyTerminal);
        }
        if completed.continuation_emitted_at.is_some() {
            return Ok(completed);
        }
        let emitted_at = Utc::now();
        let event = AuthContinuationEvent {
            flow_id: completed.id,
            scope: completed.scope.clone(),
            continuation: completed.continuation.clone(),
            credential_account_id: completed.credential_account_id,
            emitted_at,
        };
        if let Err(error) = self
            .continuation_dispatcher
            .dispatch_auth_continuation(event)
            .await
        {
            tracing::debug!(
                flow_id = %completed.id,
                error_code = ?error.code(),
                "reborn auth flow completed but continuation dispatch failed"
            );
            let error = match error {
                AuthProductError::TokenExchangeFailed
                | AuthProductError::ProviderDenied
                | AuthProductError::MalformedCallback => AuthProductError::BackendUnavailable,
                error => error,
            };
            return Err(error);
        }
        self.flow_manager
            .mark_continuation_dispatched(&completed.scope, completed.id, emitted_at)
            .await
    }

    #[allow(
        dead_code,
        reason = "used by feature-scoped product-auth route tests that do not compile in every lib-test target"
    )]
    pub(crate) fn local_dev_in_memory(
        continuation_dispatcher: Arc<dyn RebornAuthContinuationDispatcher>,
    ) -> Self {
        let services = Arc::new(InMemoryAuthProductServices::new());
        RebornProductAuthServicePorts::from_shared(services.clone())
            .into_services(continuation_dispatcher)
            .with_flow_record_source(services)
    }
}

fn auth_challenge_to_view(
    challenge: &brassclaw_auth::AuthChallenge,
    flow: &brassclaw_auth::AuthFlowRecord,
) -> AuthChallengeView {
    match challenge {
        brassclaw_auth::AuthChallenge::OAuthUrl {
            authorization_url,
            expires_at,
        } => AuthChallengeView {
            kind: AuthPromptChallengeKind::OAuthUrl,
            provider: flow.provider.clone(),
            account_label: None,
            authorization_url: Some(authorization_url.clone()),
            expires_at: Some(*expires_at),
        },
        brassclaw_auth::AuthChallenge::ManualTokenRequired {
            provider,
            label,
            expires_at,
            ..
        } => AuthChallengeView {
            kind: AuthPromptChallengeKind::ManualToken,
            provider: provider.clone(),
            account_label: Some(label.clone()),
            authorization_url: None,
            expires_at: Some(*expires_at),
        },
        brassclaw_auth::AuthChallenge::AccountSelectionRequired { .. }
        | brassclaw_auth::AuthChallenge::ReauthorizeRequired { .. }
        | brassclaw_auth::AuthChallenge::SetupRequired { .. } => AuthChallengeView {
            kind: AuthPromptChallengeKind::Other,
            provider: flow.provider.clone(),
            account_label: None,
            authorization_url: None,
            expires_at: None,
        },
    }
}

#[async_trait]
impl AuthChallengeProvider for RebornProductAuthServices {
    async fn challenge_for_gate(
        &self,
        scope: &TurnScope,
        owner_user_id: &UserId,
        run_id: TurnRunId,
        gate_ref: &str,
        credential_requirements: &[brassclaw_host_api::RuntimeCredentialAuthRequirement],
    ) -> Result<Option<AuthChallengeView>, AuthProductError> {
        let gate_ref = AuthGateRef::new(gate_ref.to_string())
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        let Some(source) = self.flow_record_source.as_ref() else {
            return Ok(None);
        };
        if let Some(registry) = &self.oauth_gate_registry
            && let Some(view) = registry
                .challenge_for_blocked_gate(OAuthGateChallengeRequest {
                    flow_manager: &self.flow_manager,
                    flow_source: source,
                    requirements: credential_requirements,
                    scope,
                    owner_user_id,
                    run_id,
                    gate_ref: &gate_ref,
                })
                .await?
        {
            return Ok(Some(view));
        }
        if let Some(registry) = &self.dcr_oauth_registry
            && let Some(view) = registry
                .challenge_for_blocked_gate(DcrGateChallengeRequest {
                    flow_manager: &self.flow_manager,
                    flow_source: source,
                    requirements: credential_requirements,
                    scope,
                    owner_user_id,
                    run_id,
                    gate_ref: &gate_ref,
                })
                .await?
        {
            return Ok(Some(view));
        }
        // The flow source may include records from multiple product surfaces;
        // query by stable owner and gate continuation before exposing metadata.
        let flow = source
            .flow_for_turn_gate(TurnGateAuthFlowQuery {
                owner: AuthFlowOwnerScope {
                    tenant_id: scope.tenant_id.clone(),
                    user_id: owner_user_id.clone(),
                    agent_id: scope.agent_id.clone(),
                    project_id: scope.project_id.clone(),
                    thread_id: scope.thread_id.clone(),
                },
                turn_run_ref: TurnRunRef::new(run_id.to_string())
                    .map_err(|_| AuthProductError::BackendUnavailable)?,
                gate_ref,
                include_terminal: false,
            })
            .await?;
        let Some(flow) = flow else {
            return Ok(None);
        };
        let Some(challenge) = flow.challenge.as_ref() else {
            return Ok(None);
        };
        Ok(Some(auth_challenge_to_view(challenge, &flow)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_auth::{
        AuthChallenge, AuthFlowId, AuthFlowRecord, AuthProductError, AuthProductScope,
        CredentialAccount, CredentialAccountChoiceRequest, CredentialAccountId,
        CredentialAccountListPage, CredentialAccountListRequest, CredentialAccountLookupRequest,
        CredentialAccountMutation, CredentialAccountProjection, CredentialAccountSelectionRequest,
        CredentialAccountStatus, CredentialRecoveryProjection, CredentialRecoveryRequest,
        CredentialRefreshReport, CredentialRefreshRequest, NewAuthFlow, NewCredentialAccount,
        OAuthCallbackClaimRequest, OAuthCallbackFailureInput, OAuthCallbackInput,
        OAuthProviderCallbackRequest, OAuthProviderExchange, OAuthProviderRefresh,
        OAuthProviderRefreshRequest, SecretCleanupReport, SecretCleanupRequest,
        SecretSubmitRequest, SecretSubmitResult,
    };

    struct SharedAuthTestDouble;

    fn arc_data_ptr<T: ?Sized>(arc: &Arc<T>) -> *const () {
        Arc::as_ptr(arc) as *const ()
    }

    #[test]
    fn reborn_product_auth_services_new_accepts_separate_impls() {
        let flow_manager: Arc<dyn AuthFlowManager> = Arc::new(SharedAuthTestDouble);
        let interaction_service: Arc<dyn AuthInteractionService> = Arc::new(SharedAuthTestDouble);
        let credential_setup_service: Arc<dyn CredentialSetupService> =
            Arc::new(SharedAuthTestDouble);
        let credential_account_service: Arc<dyn CredentialAccountService> =
            Arc::new(SharedAuthTestDouble);
        let provider_client: Arc<dyn AuthProviderClient> = Arc::new(SharedAuthTestDouble);
        let cleanup_service: Arc<dyn SecretCleanupService> = Arc::new(SharedAuthTestDouble);

        let services = RebornProductAuthServices::new(
            flow_manager.clone(),
            interaction_service.clone(),
            credential_setup_service.clone(),
            credential_account_service.clone(),
            provider_client.clone(),
            cleanup_service.clone(),
            Arc::new(NoopAuthContinuationDispatcher),
        );

        assert_eq!(
            arc_data_ptr(&services.flow_manager()),
            arc_data_ptr(&flow_manager)
        );
        assert_eq!(
            arc_data_ptr(&services.interaction_service()),
            arc_data_ptr(&interaction_service)
        );
        assert_eq!(
            arc_data_ptr(&services.credential_setup_service()),
            arc_data_ptr(&credential_setup_service)
        );
        assert_eq!(
            arc_data_ptr(&services.credential_account_service()),
            arc_data_ptr(&credential_account_service)
        );
        assert_eq!(
            arc_data_ptr(&services.provider_client()),
            arc_data_ptr(&provider_client)
        );
        assert_eq!(
            arc_data_ptr(&services.cleanup_service()),
            arc_data_ptr(&cleanup_service)
        );
    }

    #[test]
    fn reborn_product_auth_services_from_shared_clones_arc_per_trait() {
        let shared = Arc::new(SharedAuthTestDouble);
        let shared_ptr = arc_data_ptr(&shared);

        let services = RebornProductAuthServices::from_shared(
            shared,
            Arc::new(NoopAuthContinuationDispatcher),
        );

        assert_eq!(arc_data_ptr(&services.flow_manager()), shared_ptr);
        assert_eq!(arc_data_ptr(&services.interaction_service()), shared_ptr);
        assert_eq!(
            arc_data_ptr(&services.credential_setup_service()),
            shared_ptr
        );
        assert_eq!(
            arc_data_ptr(&services.credential_account_service()),
            shared_ptr
        );
        assert_eq!(arc_data_ptr(&services.provider_client()), shared_ptr);
        assert_eq!(arc_data_ptr(&services.cleanup_service()), shared_ptr);
    }

    #[test]
    fn reborn_product_auth_ports_from_shared_with_provider_uses_separate_provider_client() {
        let shared = Arc::new(SharedAuthTestDouble);
        let provider_client: Arc<dyn AuthProviderClient> = Arc::new(SharedAuthTestDouble);
        let shared_ptr = arc_data_ptr(&shared);
        let provider_ptr = arc_data_ptr(&provider_client);

        let ports =
            RebornProductAuthServicePorts::from_shared_with_provider(shared, provider_client);
        let services = ports.into_services(Arc::new(NoopAuthContinuationDispatcher));

        assert_eq!(arc_data_ptr(&services.flow_manager()), shared_ptr);
        assert_eq!(arc_data_ptr(&services.interaction_service()), shared_ptr);
        assert_eq!(
            arc_data_ptr(&services.credential_setup_service()),
            shared_ptr
        );
        assert_eq!(
            arc_data_ptr(&services.credential_account_service()),
            shared_ptr
        );
        assert_eq!(arc_data_ptr(&services.provider_client()), provider_ptr);
        assert_eq!(arc_data_ptr(&services.cleanup_service()), shared_ptr);
    }

    #[async_trait::async_trait]
    impl AuthFlowManager for SharedAuthTestDouble {
        async fn claim_dcr_cleanup(
            &self,
            _prepared: brassclaw_auth::DcrCleanupPrepared,
        ) -> Result<brassclaw_auth::DcrCleanupAdmission, brassclaw_auth::AuthProductError> {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn record_dcr_cleanup(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _completion: brassclaw_auth::DcrCleanupCompletion,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn update_dcr_retirement(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
            _update: brassclaw_auth::DcrRetirementUpdate,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn retain_legacy_dcr(
            &self,
            _selection: brassclaw_auth::DcrLegacySelection,
        ) -> Result<brassclaw_auth::DcrLegacyState, brassclaw_auth::AuthProductError> {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn mark_dcr_cleanup_unavailable(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
            _issue: brassclaw_auth::DcrCleanupUnavailable,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn prepare_oauth_setup(
            &self,
            _original: brassclaw_auth::OAuthSetupOriginal,
            _retirement: Option<brassclaw_auth::OAuthSetupRetirement>,
        ) -> Result<brassclaw_auth::OAuthSetupState, brassclaw_auth::AuthProductError> {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn read_oauth_setup(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _flow_id: brassclaw_auth::AuthFlowId,
        ) -> Result<Option<brassclaw_auth::OAuthSetupState>, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn prepare_dcr_registration(
            &self,
            _original: brassclaw_auth::DcrRegistrationOriginal,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn retire_dcr_preparation(
            &self,
            _original: brassclaw_auth::DcrRegistrationOriginal,
            _reason: brassclaw_auth::DcrCleanupReason,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn read_dcr_registration(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _id: brassclaw_auth::AuthFlowId,
        ) -> Result<Option<brassclaw_auth::DcrRegistrationState>, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn claim_dcr_registration(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
        ) -> Result<brassclaw_auth::DcrRegistrationAdmission, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn record_dcr_registration(
            &self,
            _scope: &brassclaw_auth::AuthProductScope,
            _completion: brassclaw_auth::DcrRegistrationCompletion,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn publish_dcr_challenge(
            &self,
            _original: &brassclaw_auth::DcrRegistrationOriginal,
            _challenge: brassclaw_auth::DcrRegistrationChallenge,
        ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError>
        {
            Err(brassclaw_auth::AuthProductError::BackendUnavailable)
        }

        async fn create_flow(
            &self,
            _request: NewAuthFlow,
        ) -> Result<AuthFlowRecord, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn get_flow(
            &self,
            _scope: &AuthProductScope,
            _flow_id: AuthFlowId,
        ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn claim_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _request: OAuthCallbackClaimRequest,
        ) -> Result<brassclaw_auth::OAuthCallbackAdmission, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn read_oauth_callback_original(
            &self,
            _scope: &AuthProductScope,
            _reference: &brassclaw_auth::OAuthCallbackReference,
        ) -> Result<Option<brassclaw_auth::OAuthCallbackState>, AuthProductError> {
            unreachable!("constructor tests do not use callback custody")
        }

        async fn reconcile_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _reference: &brassclaw_auth::OAuthCallbackReference,
        ) -> Result<brassclaw_auth::OAuthCallbackState, AuthProductError> {
            unreachable!("constructor tests do not use callback custody")
        }

        async fn complete_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _input: OAuthCallbackInput,
        ) -> Result<AuthFlowRecord, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn complete_credential_selection(
            &self,
            _scope: &AuthProductScope,
            _input: brassclaw_auth::CredentialSelectionInput,
        ) -> Result<AuthFlowRecord, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn complete_manual_token(
            &self,
            _scope: &AuthProductScope,
            _input: brassclaw_auth::ManualTokenCompletionInput,
        ) -> Result<AuthFlowRecord, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn cancel_manual_token(
            &self,
            _scope: &AuthProductScope,
            _interaction_id: AuthInteractionId,
        ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn fail_oauth_callback(
            &self,
            _scope: &AuthProductScope,
            _input: OAuthCallbackFailureInput,
        ) -> Result<AuthFlowRecord, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn cancel_flow(
            &self,
            _scope: &AuthProductScope,
            _flow_id: AuthFlowId,
        ) -> Result<AuthFlowRecord, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }

        async fn mark_continuation_dispatched(
            &self,
            _scope: &AuthProductScope,
            _flow_id: AuthFlowId,
            _emitted_at: brassclaw_auth::Timestamp,
        ) -> Result<AuthFlowRecord, AuthProductError> {
            unreachable!("constructor tests do not call auth-flow methods")
        }
    }

    #[async_trait::async_trait]
    impl AuthInteractionService for SharedAuthTestDouble {
        async fn request_secret_input(
            &self,
            _request: brassclaw_auth::ManualTokenSetupRequest,
        ) -> Result<AuthChallenge, AuthProductError> {
            unreachable!("constructor tests do not call auth-interaction methods")
        }

        async fn submit_manual_token(
            &self,
            _scope: &AuthProductScope,
            _request: SecretSubmitRequest,
        ) -> Result<SecretSubmitResult, AuthProductError> {
            unreachable!("constructor tests do not call auth-interaction methods")
        }

        async fn abandon_manual_token(
            &self,
            _scope: &AuthProductScope,
            _interaction_id: AuthInteractionId,
        ) -> Result<bool, AuthProductError> {
            unreachable!("constructor tests do not call auth-interaction methods")
        }
    }

    #[async_trait::async_trait]
    impl CredentialSetupService for SharedAuthTestDouble {
        async fn refresh_action(
            &self,
            _request: &CredentialRefreshRequest,
        ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
            unreachable!("constructor tests do not call credential-setup methods")
        }
        async fn begin_refresh(
            &self,
            _prepared: brassclaw_auth::CredentialRefreshPrepared,
        ) -> Result<brassclaw_auth::CredentialRefreshAdmission, AuthProductError> {
            unreachable!("constructor tests do not call credential-setup methods")
        }
        async fn reconcile_refresh(
            &self,
            _request: &brassclaw_auth::CredentialRefreshRequest,
        ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
            unreachable!("constructor-only test double")
        }

        async fn complete_refresh(
            &self,
            _completion: brassclaw_auth::CredentialRefreshCompletion,
        ) -> Result<brassclaw_auth::CredentialRefreshAction, AuthProductError> {
            unreachable!("constructor tests do not call credential-setup methods")
        }

        async fn compare_and_update_account(
            &self,
            _expected: &CredentialAccount,
            _request: NewCredentialAccount,
        ) -> Result<CredentialAccount, AuthProductError> {
            unreachable!("constructor tests do not call credential-setup methods")
        }

        async fn create_or_update_account(
            &self,
            _request: CredentialAccountMutation,
        ) -> Result<CredentialAccount, AuthProductError> {
            unreachable!("constructor tests do not call credential-setup methods")
        }
    }

    #[async_trait::async_trait]
    impl CredentialAccountService for SharedAuthTestDouble {
        async fn create_account(
            &self,
            _request: NewCredentialAccount,
        ) -> Result<CredentialAccount, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }

        async fn get_account(
            &self,
            _request: CredentialAccountLookupRequest,
        ) -> Result<Option<CredentialAccount>, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }

        async fn list_accounts(
            &self,
            _request: CredentialAccountListRequest,
        ) -> Result<CredentialAccountListPage, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }

        async fn update_status(
            &self,
            _scope: &AuthProductScope,
            _account_id: CredentialAccountId,
            _status: CredentialAccountStatus,
        ) -> Result<CredentialAccount, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }

        async fn select_unique_configured_account(
            &self,
            _request: CredentialAccountSelectionRequest,
        ) -> Result<CredentialAccountProjection, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }

        async fn project_credential_recovery(
            &self,
            _request: CredentialRecoveryRequest,
        ) -> Result<CredentialRecoveryProjection, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }

        async fn select_configured_account(
            &self,
            _request: CredentialAccountChoiceRequest,
        ) -> Result<CredentialAccountProjection, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }

        async fn refresh_account(
            &self,
            _request: CredentialRefreshRequest,
        ) -> Result<CredentialRefreshReport, AuthProductError> {
            unreachable!("constructor tests do not call credential-account methods")
        }
    }

    #[async_trait::async_trait]
    impl CredentialAccountRecordSource for SharedAuthTestDouble {
        async fn accounts_for_owner(
            &self,
            _scope: &AuthProductScope,
        ) -> Result<Vec<CredentialAccount>, AuthProductError> {
            unreachable!("constructor tests do not call credential-account read-model methods")
        }
    }

    #[async_trait::async_trait]
    impl RebornManualTokenFlowService for SharedAuthTestDouble {
        async fn request_manual_token_flow(
            &self,
            _request: brassclaw_auth::ManualTokenSetupRequest,
        ) -> Result<AuthChallenge, AuthProductError> {
            unreachable!("constructor tests do not call manual-token flow methods")
        }

        async fn submit_manual_token_flow(
            &self,
            _scope: &AuthProductScope,
            _request: SecretSubmitRequest,
        ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
            unreachable!("constructor tests do not call manual-token flow methods")
        }

        async fn abandon_manual_token_flow(
            &self,
            _scope: &AuthProductScope,
            _interaction_id: AuthInteractionId,
        ) -> Result<bool, AuthProductError> {
            unreachable!("constructor tests do not call manual-token flow methods")
        }
    }

    #[async_trait::async_trait]
    impl AuthProviderClient for SharedAuthTestDouble {
        async fn prepare_callback(
            &self,
            _context: &brassclaw_auth::OAuthProviderExchangeContext,
            _request: &brassclaw_auth::OAuthProviderCallbackRequest,
        ) -> Result<brassclaw_auth::OAuthTokenDestinations, AuthProductError> {
            unreachable!("constructor-only test double")
        }

        async fn prepare_refresh(
            &self,
            _request: &OAuthProviderRefreshRequest,
        ) -> Result<brassclaw_auth::OAuthProviderRefreshPreparation, AuthProductError> {
            unreachable!("constructor tests do not call provider-client methods")
        }

        async fn exchange_callback(
            &self,
            _context: OAuthProviderExchangeContext,
            _request: OAuthProviderCallbackRequest,
        ) -> Result<OAuthProviderExchange, AuthProductError> {
            unreachable!("constructor tests do not call provider-client methods")
        }

        async fn refresh_token(
            &self,
            _request: OAuthProviderRefreshRequest,
            _preparation: brassclaw_auth::OAuthProviderRefreshPreparation,
        ) -> Result<OAuthProviderRefresh, AuthProductError> {
            unreachable!("constructor tests do not call provider-client methods")
        }
    }

    #[async_trait::async_trait]
    impl SecretCleanupService for SharedAuthTestDouble {
        async fn cleanup_for_lifecycle(
            &self,
            _request: SecretCleanupRequest,
        ) -> Result<SecretCleanupReport, AuthProductError> {
            unreachable!("constructor tests do not call cleanup methods")
        }
    }
}
