use async_trait::async_trait;
use brassclaw_host_api::{AgentId, ExtensionId, ProjectId, TenantId, ThreadId, UserId};
use serde::{Deserialize, Serialize};

use crate::{
    AuthErrorCode, AuthProductError, AuthorizationCodeHash, CredentialAccountId,
    CredentialAccountLabel, LifecyclePackageRef, OpaqueStateHash, ProductActionRef, Timestamp,
    TurnRunRef,
    credential::{CredentialAccountProjection, CredentialAccountStatus, CredentialOwnership},
    ids::{AuthFlowId, AuthGateRef, AuthInteractionId, AuthProviderId, OAuthAuthorizationUrl},
    scope::AuthProductScope,
};

/// Auth flow kind. Identity login is represented for future shared substrate
/// support, but credential-account semantics apply only to integration flows in
/// this first slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthFlowKind {
    IntegrationCredential,
    IdentityLogin,
}

/// Durable auth-flow lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthFlowStatus {
    Pending,
    AwaitingUser,
    CallbackReceived,
    /// Reserved for production stores that split durable claim, provider
    /// exchange, and account mutation across asynchronous workers.
    Completing,
    Completed,
    Failed,
    Expired,
    Canceled,
}

/// Stable recoverable auth challenge rendered by product adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthChallenge {
    OAuthUrl {
        authorization_url: OAuthAuthorizationUrl,
        expires_at: Timestamp,
    },
    ManualTokenRequired {
        interaction_id: AuthInteractionId,
        provider: AuthProviderId,
        label: CredentialAccountLabel,
        expires_at: Timestamp,
    },
    AccountSelectionRequired {
        provider: AuthProviderId,
        accounts: Vec<CredentialAccountProjection>,
    },
    SetupRequired {
        provider: AuthProviderId,
        message: String,
    },
    ReauthorizeRequired {
        account_id: CredentialAccountId,
        provider: AuthProviderId,
    },
}

/// Typed continuation emitted after auth completion. It intentionally stores
/// references only, never raw prompt/message content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthContinuationRef {
    SetupOnly,
    LifecycleActivation {
        package_ref: LifecyclePackageRef,
    },
    TurnGateResume {
        turn_run_ref: TurnRunRef,
        gate_ref: AuthGateRef,
    },
    ProductActionResume {
        action_ref: ProductActionRef,
    },
}

/// Emitted by fake and future production services after an auth flow completes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContinuationEvent {
    pub flow_id: AuthFlowId,
    pub scope: AuthProductScope,
    pub continuation: AuthContinuationRef,
    pub credential_account_id: Option<CredentialAccountId>,
    pub emitted_at: Timestamp,
}

/// Pre-authorized credential update target captured before OAuth completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAccountUpdateBinding {
    pub account_id: CredentialAccountId,
    pub ownership: CredentialOwnership,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_extension: Option<ExtensionId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted_extensions: Vec<ExtensionId>,
}

impl CredentialAccountUpdateBinding {
    pub fn from_projection(account: &crate::CredentialAccountProjection) -> Self {
        Self {
            account_id: account.id,
            ownership: account.ownership,
            owner_extension: account.owner_extension.clone(),
            granted_extensions: account.granted_extensions.clone(),
        }
    }
}

/// Durable scoped auth flow record. OAuth state/verifier/code values are
/// represented by hashes only; raw callback material must stay in one-shot
/// provider-client inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthFlowRecord {
    pub id: AuthFlowId,
    pub scope: AuthProductScope,
    pub kind: AuthFlowKind,
    pub status: AuthFlowStatus,
    pub provider: AuthProviderId,
    pub challenge: Option<AuthChallenge>,
    pub continuation: AuthContinuationRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_account_id: Option<CredentialAccountId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_binding: Option<CredentialAccountUpdateBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opaque_state_hash: Option<OpaqueStateHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pkce_verifier_hash: Option<crate::PkceVerifierHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_code_hash: Option<AuthorizationCodeHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AuthErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation_emitted_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub expires_at: Timestamp,
}

/// Stable owner fields used by read models that project auth flows.
///
/// Invocation id, surface, session, and mission are intentionally excluded:
/// they describe how setup happened, not who owns the blocked auth interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthFlowOwnerScope {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub agent_id: Option<AgentId>,
    pub project_id: Option<ProjectId>,
    pub thread_id: ThreadId,
}

impl AuthFlowOwnerScope {
    pub fn matches(&self, flow: &AuthFlowRecord) -> bool {
        let resource = &flow.scope.resource;
        resource.tenant_id == self.tenant_id
            && resource.user_id == self.user_id
            && resource.agent_id == self.agent_id
            && resource.project_id == self.project_id
            && resource.thread_id.as_ref() == Some(&self.thread_id)
    }
}

/// Query for one auth flow that backs a blocked turn gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnGateAuthFlowQuery {
    pub owner: AuthFlowOwnerScope,
    pub turn_run_ref: TurnRunRef,
    pub gate_ref: AuthGateRef,
    pub include_terminal: bool,
}

/// Input used to create an auth flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAuthFlow {
    /// Exact usage and selection precondition captured before the challenge.
    pub requirement_setup: Option<Box<crate::CredentialRequirementSetup>>,
    pub id: Option<AuthFlowId>,
    pub scope: AuthProductScope,
    pub kind: AuthFlowKind,
    pub provider: AuthProviderId,
    pub challenge: AuthChallenge,
    pub continuation: AuthContinuationRef,
    pub update_binding: Option<CredentialAccountUpdateBinding>,
    pub opaque_state_hash: Option<OpaqueStateHash>,
    pub pkce_verifier_hash: Option<crate::PkceVerifierHash>,
    pub expires_at: Timestamp,
}

/// Provider callback result after route parsing and provider exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCallbackOutcome {
    Authorized {
        exchange: crate::OAuthProviderExchange,
    },
    Denied,
}

/// Typed OAuth callback completion input. It carries only state/code hashes and
/// provider-exchange output. Raw code/verifier material belongs in
/// [`crate::OAuthProviderCallbackRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCallbackInput {
    /// Exact original dispatch evidence for Authorized, absent for Denied.
    /// This is custody/fencing metadata, never an invocation grant.
    pub dispatched: Option<crate::OAuthCallbackAction>,
    pub flow_id: AuthFlowId,
    pub opaque_state_hash: OpaqueStateHash,
    pub outcome: ProviderCallbackOutcome,
}

/// Terminal failure input for an already-claimed OAuth callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCallbackFailureInput {
    pub dispatched: Option<crate::OAuthCallbackAction>,
    pub flow_id: AuthFlowId,
    pub opaque_state_hash: OpaqueStateHash,
    pub error: AuthErrorCode,
}

/// User-selected configured credential that completes an account-selection
/// auth flow without exposing credential internals to product surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialSelectionInput {
    pub flow_id: AuthFlowId,
    pub credential_account_id: CredentialAccountId,
}

/// User-submitted manual token that completed a manual-token auth flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualTokenCompletionInput {
    pub interaction_id: AuthInteractionId,
    pub credential_account_id: CredentialAccountId,
}

/// Complete prepared original for a single owned authorized exchange.
/// Preparation is metadata only; only a returned Dispatch owns the first call.
pub type OAuthCallbackClaimRequest = crate::OAuthCallbackPrepared;

#[async_trait]
pub trait AuthFlowManager: Send + Sync {
    /// Authenticate and persist the first OAuth setup in the existing flow/CAS
    /// owner. Readback of an existing exact original never grants another call.
    /// Only an absent flow may acquire a canceled/expired projection here; normal
    /// cancellation of an existing flow uses cancel_flow. Preparation/later-use
    /// custody must be checked before the first insert. Do not select today's
    /// account: the authenticated original already carries its first snapshot.
    async fn prepare_oauth_setup(
        &self,
        original: crate::OAuthSetupOriginal,
        retirement: Option<crate::OAuthSetupRetirement>,
    ) -> Result<crate::OAuthSetupState, AuthProductError>;

    /// Original metadata only, including after private material retirement.
    /// Legacy absence is not permission to backfill from current configuration.
    async fn read_oauth_setup(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<crate::OAuthSetupState>, AuthProductError>;

    /// Read only the original setup custody of an existing flow. Missing old
    /// metadata is not permission to reconstruct it from current settings.
    async fn requirement_setup(
        &self,
        _scope: &AuthProductScope,
        _flow: AuthFlowId,
    ) -> Result<Option<crate::CredentialRequirementSetup>, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    /// Attach only authenticated existing legacy material, without creating a
    /// registration result, replacing the flow or reading a new account target.
    async fn retain_legacy_dcr(
        &self,
        selection: crate::DcrLegacySelection,
    ) -> Result<crate::DcrLegacyState, AuthProductError>;

    /// Persist the exact local cleanup plan or authenticated closure readback.
    /// A caller must receive/read this plan before closing any secret identity.
    async fn update_dcr_retirement(
        &self,
        original: &crate::DcrRegistrationOriginal,
        update: crate::DcrRetirementUpdate,
    ) -> Result<crate::DcrRegistrationState, AuthProductError>;

    /// First acknowledged durable management intent only; readback never grants
    /// another DELETE. Concrete owners authenticate encrypted management inputs.
    async fn claim_dcr_cleanup(
        &self,
        prepared: crate::DcrCleanupPrepared,
    ) -> Result<crate::DcrCleanupAdmission, AuthProductError>;
    /// Record the exact management result before retiring any local artifact.
    async fn record_dcr_cleanup(
        &self,
        scope: &AuthProductScope,
        completion: crate::DcrCleanupCompletion,
    ) -> Result<crate::DcrRegistrationState, AuthProductError>;
    /// Retain a visible non-dispatchable cleanup disposition, never a false
    /// success when registration outcome or management support is unavailable.
    async fn mark_dcr_cleanup_unavailable(
        &self,
        original: &crate::DcrRegistrationOriginal,
        issue: crate::DcrCleanupUnavailable,
    ) -> Result<crate::DcrRegistrationState, AuthProductError>;

    /// Atomically retain the original local flow and pre-challenge account
    /// snapshot before any dynamic-client registration. Existing identities
    /// return their original state without selecting a later account. A first
    /// insertion requires authenticated preparation and no later-stage artifact;
    /// missing flow state alone never proves that registration was not sent.
    /// Elapsed preparations use the terminal restoration contract below.
    async fn prepare_dcr_registration(
        &self,
        original: crate::DcrRegistrationOriginal,
    ) -> Result<crate::DcrRegistrationState, AuthProductError>;

    /// Restore only a canceled/expired, never-dispatched preparation under its
    /// original ID. Concrete owners authenticate its encrypted source and all
    /// later-stage absences before the absent-flow CAS. An existing original
    /// wins unchanged; this operation neither snapshots today's account nor
    /// dispatches registration, cleanup HTTP or a continuation.
    async fn retire_dcr_preparation(
        &self,
        original: crate::DcrRegistrationOriginal,
        reason: crate::DcrCleanupReason,
    ) -> Result<crate::DcrRegistrationState, AuthProductError>;

    /// Read the original even after cancellation or expiry. This is metadata,
    /// not dispatch authority, and does not create or repair a missing flow.
    async fn read_dcr_registration(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<crate::DcrRegistrationState>, AuthProductError>;

    /// Only the acknowledged first durable claim may return Dispatch.
    /// Lost acknowledgement/recovery returns Original and cannot repeat POST.
    async fn claim_dcr_registration(
        &self,
        original: &crate::DcrRegistrationOriginal,
    ) -> Result<crate::DcrRegistrationAdmission, AuthProductError>;

    /// Retain exact encrypted response/outcome evidence even after cancellation.
    async fn record_dcr_registration(
        &self,
        scope: &AuthProductScope,
        completion: crate::DcrRegistrationCompletion,
    ) -> Result<crate::DcrRegistrationState, AuthProductError>;

    /// Publish only after authenticating the exact retained output generations;
    /// terminal flow state wins over a late registration response.
    async fn publish_dcr_challenge(
        &self,
        original: &crate::DcrRegistrationOriginal,
        challenge: crate::DcrRegistrationChallenge,
    ) -> Result<crate::DcrRegistrationState, AuthProductError>;

    async fn create_flow(&self, request: NewAuthFlow) -> Result<AuthFlowRecord, AuthProductError>;

    async fn get_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError>;

    async fn claim_oauth_callback(
        &self,
        scope: &AuthProductScope,
        request: OAuthCallbackClaimRequest,
    ) -> Result<crate::OAuthCallbackAdmission, AuthProductError>;

    /// Read one coherent original before automatic provider/PKCE preparation.
    async fn read_oauth_callback_original(
        &self,
        scope: &AuthProductScope,
        reference: &crate::OAuthCallbackReference,
    ) -> Result<Option<crate::OAuthCallbackState>, AuthProductError>;

    /// Store-only original reconciliation. Never consumes code or calls issuer.
    async fn reconcile_oauth_callback(
        &self,
        scope: &AuthProductScope,
        reference: &crate::OAuthCallbackReference,
    ) -> Result<crate::OAuthCallbackState, AuthProductError>;

    async fn complete_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError>;

    async fn complete_credential_selection(
        &self,
        scope: &AuthProductScope,
        input: CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError>;

    async fn complete_manual_token(
        &self,
        scope: &AuthProductScope,
        input: ManualTokenCompletionInput,
    ) -> Result<AuthFlowRecord, AuthProductError>;

    async fn cancel_manual_token(
        &self,
        scope: &AuthProductScope,
        interaction_id: AuthInteractionId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError>;

    async fn fail_oauth_callback(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError>;

    async fn mark_continuation_dispatched(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        emitted_at: Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError>;

    async fn cancel_flow(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<AuthFlowRecord, AuthProductError>;
}

/// Read-only auth-flow projection source for product interaction views.
///
/// This is intentionally smaller than [`AuthFlowManager`]: callers can list
/// sanitized flow records for scoped read-model composition, but cannot mutate
/// auth-flow state or bypass manager validation.
#[async_trait]
pub trait AuthFlowRecordSource: Send + Sync {
    async fn flow_for_turn_gate(
        &self,
        query: TurnGateAuthFlowQuery,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError>;

    async fn flows_for_owner(
        &self,
        owner: AuthFlowOwnerScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError>;
}

pub fn flow_matches_turn_gate_query(flow: &AuthFlowRecord, query: &TurnGateAuthFlowQuery) -> bool {
    if !query.include_terminal && crate::is_terminal_status(flow.status) {
        return false;
    }
    if !query.owner.matches(flow) {
        return false;
    }
    matches!(
        &flow.continuation,
        AuthContinuationRef::TurnGateResume {
            turn_run_ref,
            gate_ref,
        } if turn_run_ref == &query.turn_run_ref && gate_ref == &query.gate_ref
    )
}

pub fn credential_status_for_completed_flow() -> CredentialAccountStatus {
    CredentialAccountStatus::Configured
}
