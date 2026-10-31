use std::{collections::HashMap, fmt, sync::Arc, sync::Mutex, sync::Weak};

use async_trait::async_trait;
use brassclaw_host_api::{
    AgentId, ExtensionId, ProjectId, SecretHandle, TenantId, ThreadId, UserId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::OwnedMutexGuard;

use crate::{
    AuthProductError, AuthProviderClient, CredentialAccountId, CredentialAccountLabel,
    OAuthProviderRefreshRequest, ProviderScope, Timestamp, ids::AuthProviderId,
    scope::AuthProductScope,
};

/// Credential account status projected to product surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialAccountStatus {
    Configured,
    Inactive,
    Missing,
    Expired,
    RefreshFailed,
    Revoked,
    PendingSetup,
}

/// Ownership class determines uninstall/deactivate cleanup behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialOwnership {
    ExtensionOwned,
    UserReusable,
    SharedAdminManaged,
    System,
}

/// Durable credential account metadata. Secret values live behind handles and
/// never appear in this record.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAccount {
    pub id: CredentialAccountId,
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub label: CredentialAccountLabel,
    pub status: CredentialAccountStatus,
    pub ownership: CredentialOwnership,
    pub owner_extension: Option<ExtensionId>,
    pub granted_extensions: Vec<ExtensionId>,
    pub access_secret: Option<SecretHandle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_expires_at: Option<Timestamp>,
    pub refresh_secret: Option<SecretHandle>,
    pub scopes: Vec<ProviderScope>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl fmt::Debug for CredentialAccount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialAccount")
            .field("id", &self.id)
            .field("scope", &self.scope)
            .field("provider", &self.provider)
            .field("label", &self.label)
            .field("status", &self.status)
            .field("ownership", &self.ownership)
            .field("owner_extension", &self.owner_extension)
            .field("granted_extensions", &self.granted_extensions)
            .field(
                "access_secret",
                &self.access_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "refresh_secret",
                &self.refresh_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("scopes", &self.scopes)
            .field("access_expires_at", &self.access_expires_at)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl CredentialAccount {
    /// Wall-clock eligibility is a read projection. It must never change the
    /// raw snapshot used to fence refresh/publication against concurrent edits.
    pub fn effective_status_at(&self, now: Timestamp) -> CredentialAccountStatus {
        if self.status == CredentialAccountStatus::Configured
            && self.access_expires_at.is_some_and(|expiry| expiry <= now)
        {
            CredentialAccountStatus::Expired
        } else {
            self.status
        }
    }

    pub fn effective_status(&self) -> CredentialAccountStatus {
        self.effective_status_at(chrono::Utc::now())
    }
    /// Exact metadata snapshot identity, never an auth grant or a secret value.
    pub fn snapshot_ref(&self) -> Result<crate::CredentialAccountSnapshotRef, AuthProductError> {
        let bytes = serde_json::to_vec(self).map_err(|_| AuthProductError::BackendUnavailable)?;
        crate::CredentialAccountSnapshotRef::new(brassclaw_common::hashing::sha256_hex(&bytes))
    }

    pub fn projection(&self) -> CredentialAccountProjection {
        self.projection_at(chrono::Utc::now())
    }

    pub fn projection_at(&self, now: Timestamp) -> CredentialAccountProjection {
        let secret_handle_count =
            self.access_secret.iter().count() + self.refresh_secret.iter().count();
        CredentialAccountProjection {
            id: self.id,
            provider: self.provider.clone(),
            label: self.label.clone(),
            status: self.effective_status_at(now),
            access_expires_at: self.access_expires_at,
            ownership: self.ownership,
            owner_extension: self.owner_extension.clone(),
            granted_extensions: self.granted_extensions.clone(),
            secret_handle_count,
        }
    }

    pub fn is_authorized_for_requester(&self, requester_extension: Option<&ExtensionId>) -> bool {
        match self.ownership {
            CredentialOwnership::UserReusable => true,
            CredentialOwnership::ExtensionOwned => self
                .owner_extension
                .as_ref()
                .is_some_and(|owner_extension| requester_extension == Some(owner_extension)),
            CredentialOwnership::SharedAdminManaged => requester_extension
                .is_some_and(|requester| self.granted_extensions.contains(requester)),
            CredentialOwnership::System => false,
        }
    }
}

/// Adapter-safe account projection. It does not include raw secret material or
/// backend secret handle names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAccountProjection {
    pub id: CredentialAccountId,
    pub provider: AuthProviderId,
    pub label: CredentialAccountLabel,
    pub status: CredentialAccountStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_expires_at: Option<Timestamp>,
    pub ownership: CredentialOwnership,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_extension: Option<ExtensionId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted_extensions: Vec<ExtensionId>,
    pub secret_handle_count: usize,
}

/// Product-facing credential recovery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRecoveryKind {
    Configured,
    SetupRequired,
    ReauthorizeRequired,
    AccountSelectionRequired,
}

/// Stable reason a product surface can render without inspecting backend
/// errors or secret handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRecoveryReason {
    Configured,
    NoAccount,
    AccountMissing,
    PendingSetup,
    AccountExpired,
    RefreshFailed,
    AccountRevoked,
    AccountInactive,
    AmbiguousAccount,
}

/// Adapter-safe credential recovery projection. Account entries are filtered
/// to choices authorized for the requester and never include backend secret
/// handle names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRecoveryProjection {
    pub provider: AuthProviderId,
    pub reason: CredentialRecoveryReason,
    #[serde(flatten)]
    pub state: CredentialRecoveryState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CredentialRecoveryState {
    Configured {
        selected_account: CredentialAccountProjection,
    },
    SetupRequired {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        choices: Vec<CredentialAccountProjection>,
    },
    ReauthorizeRequired {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        choices: Vec<CredentialAccountProjection>,
    },
    AccountSelectionRequired {
        choices: Vec<CredentialAccountProjection>,
    },
}

impl CredentialRecoveryProjection {
    pub fn configured(
        provider: AuthProviderId,
        selected_account: CredentialAccountProjection,
    ) -> Self {
        Self {
            provider,
            reason: CredentialRecoveryReason::Configured,
            state: CredentialRecoveryState::Configured { selected_account },
        }
    }

    pub fn setup_required(
        provider: AuthProviderId,
        reason: CredentialRecoveryReason,
        choices: Vec<CredentialAccountProjection>,
    ) -> Self {
        Self {
            provider,
            reason,
            state: CredentialRecoveryState::SetupRequired { choices },
        }
    }

    pub fn reauthorize_required(
        provider: AuthProviderId,
        reason: CredentialRecoveryReason,
        choices: Vec<CredentialAccountProjection>,
    ) -> Self {
        Self {
            provider,
            reason,
            state: CredentialRecoveryState::ReauthorizeRequired { choices },
        }
    }

    pub fn account_selection_required(
        provider: AuthProviderId,
        choices: Vec<CredentialAccountProjection>,
    ) -> Self {
        Self {
            provider,
            reason: CredentialRecoveryReason::AmbiguousAccount,
            state: CredentialRecoveryState::AccountSelectionRequired { choices },
        }
    }

    pub fn kind(&self) -> CredentialRecoveryKind {
        match &self.state {
            CredentialRecoveryState::Configured { .. } => CredentialRecoveryKind::Configured,
            CredentialRecoveryState::SetupRequired { .. } => CredentialRecoveryKind::SetupRequired,
            CredentialRecoveryState::ReauthorizeRequired { .. } => {
                CredentialRecoveryKind::ReauthorizeRequired
            }
            CredentialRecoveryState::AccountSelectionRequired { .. } => {
                CredentialRecoveryKind::AccountSelectionRequired
            }
        }
    }

    pub fn selected_account(&self) -> Option<&CredentialAccountProjection> {
        match &self.state {
            CredentialRecoveryState::Configured { selected_account } => Some(selected_account),
            CredentialRecoveryState::SetupRequired { .. }
            | CredentialRecoveryState::ReauthorizeRequired { .. }
            | CredentialRecoveryState::AccountSelectionRequired { .. } => None,
        }
    }

    pub fn choices(&self) -> &[CredentialAccountProjection] {
        match &self.state {
            CredentialRecoveryState::Configured { .. } => &[],
            CredentialRecoveryState::SetupRequired { choices }
            | CredentialRecoveryState::ReauthorizeRequired { choices }
            | CredentialRecoveryState::AccountSelectionRequired { choices } => choices,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialAccountListRequest {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub requester_extension: Option<ExtensionId>,
    pub cursor: Option<CredentialAccountId>,
    pub limit: usize,
}

impl CredentialAccountListRequest {
    pub const DEFAULT_LIMIT: usize = 50;
    pub const MAX_LIMIT: usize = 100;

    pub fn new(scope: AuthProductScope, provider: AuthProviderId) -> Self {
        Self {
            scope,
            provider,
            requester_extension: None,
            cursor: None,
            limit: Self::DEFAULT_LIMIT,
        }
    }

    pub fn for_extension(mut self, extension_id: ExtensionId) -> Self {
        self.requester_extension = Some(extension_id);
        self
    }

    pub fn with_cursor(mut self, cursor: CredentialAccountId) -> Self {
        self.cursor = Some(cursor);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    pub fn validate(&self) -> Result<(), AuthProductError> {
        if self.limit == 0 {
            return Err(AuthProductError::invalid_request(
                "credential account list limit must be non-zero",
            ));
        }
        if self.limit > Self::MAX_LIMIT {
            return Err(AuthProductError::invalid_request(format!(
                "credential account list limit must be at most {}",
                Self::MAX_LIMIT
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialAccountLookupRequest {
    pub scope: AuthProductScope,
    pub account_id: CredentialAccountId,
    pub requester_extension: Option<ExtensionId>,
}

impl CredentialAccountLookupRequest {
    pub fn new(scope: AuthProductScope, account_id: CredentialAccountId) -> Self {
        Self {
            scope,
            account_id,
            requester_extension: None,
        }
    }

    pub fn for_extension(mut self, extension_id: ExtensionId) -> Self {
        self.requester_extension = Some(extension_id);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAccountListPage {
    pub accounts: Vec<CredentialAccountProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<CredentialAccountId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRecoveryRequest {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub requester_extension: Option<ExtensionId>,
}

impl CredentialRecoveryRequest {
    pub fn new(scope: AuthProductScope, provider: AuthProviderId) -> Self {
        Self {
            scope,
            provider,
            requester_extension: None,
        }
    }

    pub fn for_extension(mut self, extension_id: ExtensionId) -> Self {
        self.requester_extension = Some(extension_id);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialAccountSelectionRequest {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub requester_extension: Option<ExtensionId>,
}

impl CredentialAccountSelectionRequest {
    pub fn new(scope: AuthProductScope, provider: AuthProviderId) -> Self {
        Self {
            scope,
            provider,
            requester_extension: None,
        }
    }

    pub fn for_extension(mut self, extension_id: ExtensionId) -> Self {
        self.requester_extension = Some(extension_id);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialAccountChoiceRequest {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub account_id: CredentialAccountId,
    pub requester_extension: Option<ExtensionId>,
}

impl CredentialAccountChoiceRequest {
    pub fn new(
        scope: AuthProductScope,
        provider: AuthProviderId,
        account_id: CredentialAccountId,
    ) -> Self {
        Self {
            scope,
            provider,
            account_id,
            requester_extension: None,
        }
    }

    pub fn for_extension(mut self, extension_id: ExtensionId) -> Self {
        self.requester_extension = Some(extension_id);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRefreshRequest {
    pub id: crate::CredentialRefreshId,
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub account_id: CredentialAccountId,
    pub requester_extension: Option<ExtensionId>,
}

impl CredentialRefreshRequest {
    pub fn new(
        scope: AuthProductScope,
        provider: AuthProviderId,
        account_id: CredentialAccountId,
    ) -> Self {
        Self {
            id: crate::CredentialRefreshId::new(),
            scope,
            provider,
            account_id,
            requester_extension: None,
        }
    }

    /// Reuse the exact original operation on a transport retry or readback.
    pub fn with_id(mut self, id: crate::CredentialRefreshId) -> Self {
        self.id = id;
        self
    }

    pub fn for_extension(mut self, extension_id: ExtensionId) -> Self {
        self.requester_extension = Some(extension_id);
        self
    }
}

/// Adapter-safe refresh result. It carries the updated redacted account plus
/// the stable recovery state after refresh. It never carries backend error
/// strings, secret handles, or provider response bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRefreshReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<crate::CredentialRefreshReceipt>,
    pub account: CredentialAccountProjection,
    pub recovery: CredentialRecoveryProjection,
    pub refreshed: bool,
}

/// Input used to create or update an account from an OAuth/manual setup result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCredentialAccount {
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub label: CredentialAccountLabel,
    pub status: CredentialAccountStatus,
    pub ownership: CredentialOwnership,
    pub owner_extension: Option<ExtensionId>,
    pub granted_extensions: Vec<ExtensionId>,
    pub access_secret: Option<SecretHandle>,
    pub access_expires_at: Option<Timestamp>,
    pub refresh_secret: Option<SecretHandle>,
    pub scopes: Vec<ProviderScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialAccountUpdate {
    pub account_id: CredentialAccountId,
    pub account: NewCredentialAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialAccountMutation {
    Create(NewCredentialAccount),
    Update(CredentialAccountUpdate),
}

#[async_trait]
pub trait CredentialAccountService: Send + Sync {
    async fn create_account(
        &self,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError>;

    async fn get_account(
        &self,
        request: CredentialAccountLookupRequest,
    ) -> Result<Option<CredentialAccount>, AuthProductError>;

    async fn list_accounts(
        &self,
        request: CredentialAccountListRequest,
    ) -> Result<CredentialAccountListPage, AuthProductError>;

    async fn update_status(
        &self,
        scope: &AuthProductScope,
        account_id: CredentialAccountId,
        status: CredentialAccountStatus,
    ) -> Result<CredentialAccount, AuthProductError>;

    async fn select_unique_configured_account(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError>;

    async fn project_credential_recovery(
        &self,
        request: CredentialRecoveryRequest,
    ) -> Result<CredentialRecoveryProjection, AuthProductError>;

    async fn select_configured_account(
        &self,
        request: CredentialAccountChoiceRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError>;

    async fn refresh_account(
        &self,
        request: CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, AuthProductError>;
}

/// Stable credential-account owner fields used by read models that need to
/// find accounts across transient invocation ids, product surfaces, or runtime
/// sub-scopes. Missing mission/thread/session ids match both global and
/// scoped accounts for the owner; present ids match only that exact scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialAccountOwnerScope {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub agent_id: Option<AgentId>,
    pub project_id: Option<ProjectId>,
    pub thread_id: Option<ThreadId>,
    pub session_id: Option<crate::AuthSessionId>,
}

impl CredentialAccountOwnerScope {
    pub fn from_scope(scope: &AuthProductScope) -> Self {
        Self {
            tenant_id: scope.resource.tenant_id.clone(),
            user_id: scope.resource.user_id.clone(),
            agent_id: scope.resource.agent_id.clone(),
            project_id: scope.resource.project_id.clone(),
            thread_id: scope.resource.thread_id.clone(),
            session_id: scope.session_id.clone(),
        }
    }

    pub fn matches(&self, account: &CredentialAccount) -> bool {
        let resource = &account.scope.resource;
        resource.tenant_id == self.tenant_id
            && resource.user_id == self.user_id
            && resource.agent_id == self.agent_id
            && resource.project_id == self.project_id
            && self
                .thread_id
                .as_ref()
                .is_none_or(|thread_id| resource.thread_id.as_ref() == Some(thread_id))
            && self
                .session_id
                .as_ref()
                .is_none_or(|session_id| account.scope.session_id.as_ref() == Some(session_id))
    }
}

/// Read-only credential-account projection source for account owner queries.
///
/// This intentionally does not encode host-runtime credential selection. It
/// only exposes accounts owned by a stable product-auth owner; composition
/// layers decide which account, provider, or requester policy to apply.
#[async_trait]
pub trait CredentialAccountRecordSource: Send + Sync {
    /// Read one original account location without choosing by label/recency.
    /// This host metadata port grants no runtime or external-service access.
    async fn account_record(
        &self,
        _scope: &AuthProductScope,
        _account: CredentialAccountId,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    /// Canonical requirement/account metadata in this same Auth owner. A
    /// read neither selects another account nor grants secret access.
    async fn requirement_selection(
        &self,
        _scope: &AuthProductScope,
        _requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<crate::CredentialRequirementSelection>, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    /// Read the protected selection from this exact original completed flow,
    /// independently of a later requirement-head/account-choice change. This
    /// returns historical metadata, not current eligibility, an encrypted
    /// generation, task-family lineage or runtime access. Unsupported owners
    /// reject; callers must never substitute the current selection on failure.
    async fn requirement_selection_for_flow(
        &self,
        _scope: &AuthProductScope,
        _flow: crate::AuthFlowId,
        _requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<crate::CredentialRequirementSelection>, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    /// Protected generation captured with this original completed flow's own
    /// selection. Historical missing proof is not read-latest authority. This
    /// metadata grants no material, live eligibility or task-family ownership.
    async fn requirement_source_for_flow(
        &self,
        _scope: &AuthProductScope,
        _flow: crate::AuthFlowId,
        _requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<crate::CredentialRequirementSource>, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }

    async fn accounts_for_owner(
        &self,
        scope: &AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError>;

    async fn select_unique_configured_account_for_owner(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccount, AuthProductError> {
        let configured = self
            .accounts_for_owner(&request.scope)
            .await?
            .into_iter()
            .filter(|account| {
                account.provider == request.provider
                    && account.effective_status() == CredentialAccountStatus::Configured
            })
            .collect::<Vec<_>>();
        if configured.is_empty() {
            return Err(AuthProductError::CredentialMissing);
        }
        let selectable = configured
            .into_iter()
            .filter(|account| {
                account.is_authorized_for_requester(request.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        match selectable.as_slice() {
            [] => Err(AuthProductError::CrossScopeDenied),
            [account] => Ok(account.clone()),
            _ => Err(AuthProductError::AccountSelectionRequired),
        }
    }
}

#[async_trait]
pub trait CredentialSetupService: Send + Sync {
    async fn refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<crate::CredentialRefreshAction>, AuthProductError>;

    async fn begin_refresh(
        &self,
        prepared: crate::CredentialRefreshPrepared,
    ) -> Result<crate::CredentialRefreshAdmission, AuthProductError>;

    /// Reconcile only the original stored result; never call the issuer.
    async fn reconcile_refresh(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<crate::CredentialRefreshAction>, AuthProductError>;

    async fn complete_refresh(
        &self,
        completion: crate::CredentialRefreshCompletion,
    ) -> Result<crate::CredentialRefreshAction, AuthProductError>;

    /// Atomically replace the exact observed account snapshot. A later status,
    /// credential, ownership or grant mutation must cause BackendConflict.
    /// This is a storage fence, not a permission or a refresh-attempt receipt.
    async fn compare_and_update_account(
        &self,
        expected: &CredentialAccount,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError>;

    async fn create_or_update_account(
        &self,
        request: CredentialAccountMutation,
    ) -> Result<CredentialAccount, AuthProductError>;
}

/// Credential account service that refreshes through the provider client and
/// persists account mutations through the backing account/setup services.
pub struct ProviderBackedCredentialAccountService {
    accounts: Arc<dyn CredentialAccountService>,
    setup: Arc<dyn CredentialSetupService>,
    provider: Arc<dyn AuthProviderClient>,
    refresh_locks: Mutex<HashMap<CredentialAccountId, Weak<tokio::sync::Mutex<()>>>>,
}

impl ProviderBackedCredentialAccountService {
    pub fn new(
        accounts: Arc<dyn CredentialAccountService>,
        setup: Arc<dyn CredentialSetupService>,
        provider: Arc<dyn AuthProviderClient>,
    ) -> Self {
        Self {
            accounts,
            setup,
            provider,
            refresh_locks: Mutex::new(HashMap::new()),
        }
    }

    fn refresh_lock(&self, account_id: CredentialAccountId) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .refresh_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&account_id).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(account_id, Arc::downgrade(&lock));
        lock
    }
    async fn acquire_refresh_lock(&self, account_id: CredentialAccountId) -> OwnedMutexGuard<()> {
        self.refresh_lock(account_id).lock_owned().await
    }

    fn refresh_lookup_request(
        request: &CredentialRefreshRequest,
    ) -> CredentialAccountLookupRequest {
        let mut lookup =
            CredentialAccountLookupRequest::new(request.scope.clone(), request.account_id);
        if let Some(requester_extension) = request.requester_extension.clone() {
            lookup = lookup.for_extension(requester_extension);
        }
        lookup
    }

    async fn reconcile_known_original(
        &self,
        original: crate::CredentialRefreshAction,
        request: &CredentialRefreshRequest,
    ) -> Result<crate::CredentialRefreshAction, AuthProductError> {
        original.original.verify(request)?;
        if original.needs_publication() {
            self.setup
                .reconcile_refresh(request)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)
        } else {
            Ok(original)
        }
    }

    async fn read_original_refresh(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<crate::CredentialRefreshAction>, AuthProductError> {
        match self.setup.refresh_action(request).await? {
            Some(original) => self
                .reconcile_known_original(original, request)
                .await
                .map(Some),
            None => Ok(None),
        }
    }

    async fn report_action(
        &self,
        action: &crate::CredentialRefreshAction,
        request: &CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, AuthProductError> {
        action.original.verify(request)?;
        let account = self
            .accounts
            .get_account(Self::refresh_lookup_request(request))
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        let recovery_request =
            CredentialRecoveryRequest::new(request.scope.clone(), request.provider.clone());
        // This read may fail independently of the original committed result.
        // Propagation is safe: an original retry never repeats the issuer call.
        let recovery = self
            .accounts
            .project_credential_recovery(match &request.requester_extension {
                Some(extension) => recovery_request.for_extension(extension.clone()),
                None => recovery_request,
            })
            .await?;
        let receipt = action.receipt()?;
        let refreshed =
            receipt.confirmed_refresh() && action.published_account.as_ref() == Some(&account);
        Ok(CredentialRefreshReport {
            original: Some(receipt),
            account: account.projection(),
            recovery,
            refreshed,
        })
    }
}

#[async_trait]
impl CredentialAccountService for ProviderBackedCredentialAccountService {
    async fn create_account(
        &self,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        self.accounts.create_account(request).await
    }

    async fn get_account(
        &self,
        request: CredentialAccountLookupRequest,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        self.accounts.get_account(request).await
    }

    async fn list_accounts(
        &self,
        request: CredentialAccountListRequest,
    ) -> Result<CredentialAccountListPage, AuthProductError> {
        self.accounts.list_accounts(request).await
    }

    async fn update_status(
        &self,
        scope: &AuthProductScope,
        account_id: CredentialAccountId,
        status: CredentialAccountStatus,
    ) -> Result<CredentialAccount, AuthProductError> {
        self.accounts.update_status(scope, account_id, status).await
    }

    async fn select_unique_configured_account(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        self.accounts
            .select_unique_configured_account(request)
            .await
    }

    async fn project_credential_recovery(
        &self,
        request: CredentialRecoveryRequest,
    ) -> Result<CredentialRecoveryProjection, AuthProductError> {
        self.accounts.project_credential_recovery(request).await
    }

    async fn select_configured_account(
        &self,
        request: CredentialAccountChoiceRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        self.accounts.select_configured_account(request).await
    }

    async fn refresh_account(
        &self,
        request: CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, AuthProductError> {
        let initial = self
            .accounts
            .get_account(Self::refresh_lookup_request(&request))
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if let Some(original) = self.read_original_refresh(&request).await? {
            return self.report_action(&original, &request).await;
        }
        crate::domain::validate_refresh_target(&initial, &request)?;
        let _guard = self.acquire_refresh_lock(initial.id).await;
        // A waiting caller and an independent wrapper both read the original.
        if let Some(original) = self.read_original_refresh(&request).await? {
            return self.report_action(&original, &request).await;
        }
        let current = self
            .accounts
            .get_account(Self::refresh_lookup_request(&request))
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        let provider_request =
            initial
                .refresh_secret
                .as_ref()
                .map(|refresh_secret| OAuthProviderRefreshRequest {
                    refresh_id: request.id,
                    provider: initial.provider.clone(),
                    scope: initial.scope.clone(),
                    account_id: initial.id,
                    refresh_secret: refresh_secret.clone(),
                    scopes: initial.scopes.clone(),
                });
        let preparation = match (&provider_request, current == initial) {
            (Some(provider_request), true) => {
                Some(self.provider.prepare_refresh(provider_request).await?)
            }
            _ => None,
        };
        let admission = self
            .setup
            .begin_refresh(crate::CredentialRefreshPrepared {
                original: crate::CredentialRefreshOriginal::from(&request),
                expected_account: initial,
                preparation,
            })
            .await?;
        let action = match admission {
            crate::CredentialRefreshAdmission::Original(original) => {
                self.reconcile_known_original(original, &request).await?
            }
            crate::CredentialRefreshAdmission::Dispatch(dispatched) => {
                let (provider_request, preparation) = dispatched.provider_dispatch()?;
                let outcome = match self
                    .provider
                    .refresh_token(provider_request, preparation)
                    .await
                {
                    Ok(result) => crate::CredentialRefreshOutcome::Returned(result),
                    Err(error) => crate::CredentialRefreshOutcome::Unknown(error.code()),
                };
                // No cleanup/retry follows an unreadable or unconfirmed write.
                // The prepared destinations already belong to this durable intent.
                self.setup
                    .complete_refresh(crate::CredentialRefreshCompletion {
                        dispatched,
                        outcome,
                    })
                    .await?
            }
        };
        self.report_action(&action, &request).await
    }
}
