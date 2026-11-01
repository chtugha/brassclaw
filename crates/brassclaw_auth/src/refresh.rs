//! Durable original refresh metadata. This is custody, never an execution grant.
use crate::{
    AuthErrorCode, AuthProductError, AuthProductScope, AuthProviderId, CredentialAccount,
    CredentialAccountId, CredentialAccountStatus, CredentialRefreshId, CredentialRefreshRequest,
    OAuthProviderRefresh, OAuthProviderRefreshPreparation, OAuthProviderRefreshRequest,
    OAuthRefreshSource, OAuthTokenDestinations, Timestamp, scope_matches,
};
use brassclaw_host_api::{ExtensionId, SecretHandle};
use serde::{Deserialize, Serialize};

/// Serializable original facts; executable provider requests remain host-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRefreshOriginal {
    pub id: CredentialRefreshId,
    pub scope: AuthProductScope,
    pub provider: AuthProviderId,
    pub account_id: CredentialAccountId,
    pub requester_extension: Option<ExtensionId>,
}
impl From<&CredentialRefreshRequest> for CredentialRefreshOriginal {
    fn from(request: &CredentialRefreshRequest) -> Self {
        Self {
            id: request.id,
            scope: request.scope.clone(),
            provider: request.provider.clone(),
            account_id: request.account_id,
            requester_extension: request.requester_extension.clone(),
        }
    }
}
impl CredentialRefreshOriginal {
    pub fn scope_ref(&self) -> Result<String, AuthProductError> {
        let bytes =
            serde_json::to_vec(&self.scope).map_err(|_| AuthProductError::BackendUnavailable)?;
        Ok(brassclaw_common::hashing::sha256_hex(&bytes))
    }
    pub fn verify(&self, request: &CredentialRefreshRequest) -> Result<(), AuthProductError> {
        if self == &Self::from(request) {
            Ok(())
        } else {
            Err(AuthProductError::BackendConflict)
        }
    }
    /// Reconstructs only original typed metadata; it confers no authority.
    pub fn request(&self) -> CredentialRefreshRequest {
        CredentialRefreshRequest {
            id: self.id,
            scope: self.scope.clone(),
            provider: self.provider.clone(),
            account_id: self.account_id,
            requester_extension: self.requester_extension.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefreshStopReason {
    MissingRefreshCredential,
    SnapshotChanged,
}

/// A failed/malformed HTTP reply never establishes that no issuer effect occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CredentialRefreshOutcome {
    Submitted,
    Returned(OAuthProviderRefresh),
    Unknown(AuthErrorCode),
    UnusableReturn(OAuthProviderRefresh),
    NotSubmitted(CredentialRefreshStopReason),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefreshOutcomeKind {
    Unconfirmed,
    Returned,
    NotSubmitted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefreshPublication {
    Pending,
    Applied,
    Superseded,
}

/// Safe public acknowledgement; contains no secret handle or raw provider data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRefreshReceipt {
    pub id: CredentialRefreshId,
    pub account_id: CredentialAccountId,
    pub provider: AuthProviderId,
    pub outcome: CredentialRefreshOutcomeKind,
    pub publication: CredentialRefreshPublication,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AuthErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_account_ref: Option<crate::CredentialAccountSnapshotRef>,
}
impl CredentialRefreshReceipt {
    pub fn confirmed_refresh(&self) -> bool {
        self.outcome == CredentialRefreshOutcomeKind::Returned
            && self.publication == CredentialRefreshPublication::Applied
    }
}

/// Complete accountable original, retained by the existing account owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRefreshAction {
    pub original: CredentialRefreshOriginal,
    pub expected_account: CredentialAccount,
    pub destinations: Option<OAuthTokenDestinations>,
    // Legacy originals remain readable, but never gain a new dispatch source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<OAuthRefreshSource>,
    pub outcome: CredentialRefreshOutcome,
    pub publication: CredentialRefreshPublication,
    pub published_account: Option<CredentialAccount>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
#[derive(Debug, Clone)]
pub struct CredentialRefreshPrepared {
    pub original: CredentialRefreshOriginal,
    pub expected_account: CredentialAccount,
    pub preparation: Option<OAuthProviderRefreshPreparation>,
}
#[derive(Debug, Clone)]
pub enum CredentialRefreshAdmission {
    Dispatch(CredentialRefreshAction),
    Original(CredentialRefreshAction),
}
#[derive(Debug, Clone)]
pub struct CredentialRefreshCompletion {
    pub dispatched: CredentialRefreshAction,
    pub outcome: CredentialRefreshOutcome,
}

/// Pure subject validation for the SAME refresh state machine. Neither branch
/// authenticates a secret, authorizes transport or grants dispatch. The native
/// instance owner must authenticate its account/client lineage and exact source,
/// durably admit the action, then use its private host transport capability.
#[derive(Clone, Copy)]
enum RefreshSubject {
    Scoped,
    HostInstance,
}
impl RefreshSubject {
    fn authorized(self, account: &CredentialAccount, request: &CredentialRefreshRequest) -> bool {
        match self {
            Self::Scoped => {
                account.is_authorized_for_requester(request.requester_extension.as_ref())
            }
            Self::HostInstance => {
                let resource = &account.scope.resource;
                !resource.is_system()
                    && account.scope.surface == crate::AuthSurface::SetupAdmin
                    && resource.agent_id.is_none()
                    && resource.project_id.is_none()
                    && resource.thread_id.is_none()
                    && account.scope.session_id.is_none()
                    && account.ownership == crate::CredentialOwnership::System
                    && account.owner_extension.is_none()
                    && account.granted_extensions.is_empty()
                    && request.requester_extension.is_none()
            }
        }
    }
    fn validate(
        self,
        account: &CredentialAccount,
        request: &CredentialRefreshRequest,
    ) -> Result<(), AuthProductError> {
        match self {
            Self::Scoped => crate::domain::validate_refresh_target(account, request),
            Self::HostInstance => {
                if !self.authorized(account, request)
                    || account.id != request.account_id
                    || account.scope != request.scope
                    || account.provider != request.provider
                {
                    return Err(AuthProductError::CrossScopeDenied);
                }
                crate::domain::validate_refresh_account_state(account)
            }
        }
    }
}

impl CredentialRefreshAction {
    /// Reconstruct the exact selected provider metadata, never latest material.
    /// This is not authority: only the winning durable admission may dispatch it.
    /// Retained legacy intents without a source stay read-only/reconcilable.
    pub fn provider_dispatch(
        &self,
    ) -> Result<(OAuthProviderRefreshRequest, OAuthProviderRefreshPreparation), AuthProductError>
    {
        self.provider_dispatch_for(RefreshSubject::Scoped)
    }

    /// Native instance metadata only. This is NOT a dispatch grant. Generic
    /// refresh/extension paths still reject System-owned accounts; only the
    /// private host owner can authenticate and submit its actual credential.
    pub fn provider_dispatch_host_instance(
        &self,
    ) -> Result<(OAuthProviderRefreshRequest, OAuthProviderRefreshPreparation), AuthProductError>
    {
        self.provider_dispatch_for(RefreshSubject::HostInstance)
    }

    fn provider_dispatch_for(
        &self,
        subject: RefreshSubject,
    ) -> Result<(OAuthProviderRefreshRequest, OAuthProviderRefreshPreparation), AuthProductError>
    {
        if self.outcome != CredentialRefreshOutcome::Submitted
            || self.publication != CredentialRefreshPublication::Pending
        {
            return Err(AuthProductError::BackendConflict);
        }
        if self.expected_account.id != self.original.account_id {
            return Err(AuthProductError::BackendConflict);
        }
        subject.validate(&self.expected_account, &self.original.request())?;
        let request = OAuthProviderRefreshRequest {
            refresh_id: self.original.id,
            provider: self.expected_account.provider.clone(),
            scope: self.expected_account.scope.clone(),
            account_id: self.expected_account.id,
            refresh_secret: self
                .expected_account
                .refresh_secret
                .clone()
                .ok_or(AuthProductError::BackendUnavailable)?,
            scopes: self.expected_account.scopes.clone(),
        };
        let preparation = OAuthProviderRefreshPreparation {
            destinations: self
                .destinations
                .clone()
                .ok_or(AuthProductError::BackendUnavailable)?,
            source: self
                .source
                .clone()
                .ok_or(AuthProductError::BackendUnavailable)?,
        };
        preparation.source.verify(&request)?;
        Ok((request, preparation))
    }

    /// Read-only originals need no publication lock. Only a stored outcome
    /// whose account/receipt is still pending can advance through reconciliation.
    pub fn needs_publication(&self) -> bool {
        self.publication == CredentialRefreshPublication::Pending
            && !matches!(self.outcome, CredentialRefreshOutcome::Submitted)
    }
    pub fn receipt(&self) -> Result<CredentialRefreshReceipt, AuthProductError> {
        let (outcome, error) = match &self.outcome {
            CredentialRefreshOutcome::Submitted => {
                (CredentialRefreshOutcomeKind::Unconfirmed, None)
            }
            CredentialRefreshOutcome::Unknown(error) => {
                (CredentialRefreshOutcomeKind::Unconfirmed, Some(*error))
            }
            CredentialRefreshOutcome::UnusableReturn(_) => (
                CredentialRefreshOutcomeKind::Unconfirmed,
                Some(AuthErrorCode::TokenExchangeFailed),
            ),
            CredentialRefreshOutcome::Returned(_) => (CredentialRefreshOutcomeKind::Returned, None),
            CredentialRefreshOutcome::NotSubmitted(reason) => (
                CredentialRefreshOutcomeKind::NotSubmitted,
                (*reason == CredentialRefreshStopReason::MissingRefreshCredential)
                    .then_some(AuthErrorCode::CredentialMissing),
            ),
        };
        Ok(CredentialRefreshReceipt {
            id: self.original.id,
            account_id: self.original.account_id,
            provider: self.original.provider.clone(),
            outcome,
            publication: self.publication,
            error,
            published_account_ref: self
                .published_account
                .as_ref()
                .map(CredentialAccount::snapshot_ref)
                .transpose()?,
        })
    }
    /// Blocks an alternate identity while the same refresh credential has an
    /// unresolved or unadopted effect. A legitimate different grant is distinct.
    pub fn blocks_credential(&self, handle: &SecretHandle) -> bool {
        self.expected_account.refresh_secret.as_ref() == Some(handle)
            && !matches!(self.outcome, CredentialRefreshOutcome::NotSubmitted(_))
            && !(matches!(self.outcome, CredentialRefreshOutcome::Returned(_))
                && self.publication == CredentialRefreshPublication::Applied)
    }
    /// Storage-side identity check across an original intent/result and its
    /// final account receipt; equality never grants dispatch authority.
    pub fn same_intent(&self, other: &Self) -> bool {
        self.original == other.original
            && self.expected_account == other.expected_account
            && self.destinations == other.destinations
            && self.source == other.source
            && self.created_at == other.created_at
    }
    pub fn begin(
        current: &mut CredentialAccount,
        existing: &[Self],
        prepared: CredentialRefreshPrepared,
        now: Timestamp,
    ) -> Result<CredentialRefreshAdmission, AuthProductError> {
        Self::begin_for(current, existing, prepared, now, RefreshSubject::Scoped)
    }

    /// Use the existing state transitions for host-owned instance metadata.
    /// The caller must first authenticate its original instance subject and
    /// source through the native store and commit the winning journal admission
    /// before HTTP. Caller labels or this pure result grant no authority.
    pub fn begin_host_instance(
        current: &mut CredentialAccount,
        existing: &[Self],
        prepared: CredentialRefreshPrepared,
        now: Timestamp,
    ) -> Result<CredentialRefreshAdmission, AuthProductError> {
        Self::begin_for(
            current,
            existing,
            prepared,
            now,
            RefreshSubject::HostInstance,
        )
    }

    fn begin_for(
        current: &mut CredentialAccount,
        existing: &[Self],
        prepared: CredentialRefreshPrepared,
        now: Timestamp,
        subject: RefreshSubject,
    ) -> Result<CredentialRefreshAdmission, AuthProductError> {
        if let Some(original) = existing
            .iter()
            .find(|a| a.original.id == prepared.original.id)
        {
            original.original.verify(&prepared.original.request())?;
            return Ok(CredentialRefreshAdmission::Original(original.clone()));
        }
        let expected = &prepared.expected_account;
        let request = prepared.original.request();
        subject.validate(expected, &request)?;
        if current.id != expected.id
            || current.provider != expected.provider
            || !scope_matches(&current.scope, &expected.scope)
            || !subject.authorized(current, &request)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let mut action = Self {
            original: prepared.original,
            expected_account: prepared.expected_account,
            destinations: prepared
                .preparation
                .as_ref()
                .map(|p| p.destinations.clone()),
            source: prepared.preparation.map(|p| p.source),
            outcome: CredentialRefreshOutcome::Submitted,
            publication: CredentialRefreshPublication::Pending,
            published_account: None,
            created_at: now,
            updated_at: now,
        };
        if *current != action.expected_account {
            action.outcome = CredentialRefreshOutcome::NotSubmitted(
                CredentialRefreshStopReason::SnapshotChanged,
            );
            action.publication = CredentialRefreshPublication::Superseded;
            return Ok(CredentialRefreshAdmission::Original(action));
        }
        match (
            &current.refresh_secret,
            &action.destinations,
            &action.source,
        ) {
            (None, None, None) => {
                current.status = CredentialAccountStatus::RefreshFailed;
                current.updated_at = now;
                action.outcome = CredentialRefreshOutcome::NotSubmitted(
                    CredentialRefreshStopReason::MissingRefreshCredential,
                );
                action.publication = CredentialRefreshPublication::Applied;
                action.published_account = Some(current.clone());
                Ok(CredentialRefreshAdmission::Original(action))
            }
            (Some(handle), Some(destinations), Some(_)) => {
                action.provider_dispatch_for(subject)?;
                let reused = |candidate: &SecretHandle| {
                    current.access_secret.as_ref() == Some(candidate)
                        || current.refresh_secret.as_ref() == Some(candidate)
                };
                if destinations.access_secret == destinations.refresh_secret
                    || reused(&destinations.access_secret)
                    || reused(&destinations.refresh_secret)
                {
                    return Err(AuthProductError::BackendConflict);
                }
                if existing.iter().any(|a| a.blocks_credential(handle)) {
                    return Err(AuthProductError::BackendConflict);
                }
                Ok(CredentialRefreshAdmission::Dispatch(action))
            }
            _ => Err(AuthProductError::BackendUnavailable),
        }
    }
    /// Retain the exact returned metadata independently of account publication.
    /// Repeated identical acknowledgements are read-only; changed results conflict.
    pub fn record_result(
        &mut self,
        completion: CredentialRefreshCompletion,
        now: Timestamp,
    ) -> Result<(), AuthProductError> {
        if !self.same_intent(&completion.dispatched)
            || !matches!(
                completion.dispatched.outcome,
                CredentialRefreshOutcome::Submitted
            )
            || completion.dispatched.publication != CredentialRefreshPublication::Pending
        {
            return Err(AuthProductError::BackendConflict);
        }
        let outcome = match completion.outcome {
            CredentialRefreshOutcome::Returned(result) => {
                let matches = self.destinations.as_ref().is_some_and(|destinations| {
                    result.provider == self.expected_account.provider
                        && result.access_secret == destinations.access_secret
                        && result
                            .refresh_secret
                            .as_ref()
                            .is_none_or(|handle| handle == &destinations.refresh_secret)
                });
                if matches {
                    CredentialRefreshOutcome::Returned(result)
                } else {
                    CredentialRefreshOutcome::UnusableReturn(result)
                }
            }
            outcome @ CredentialRefreshOutcome::Unknown(_) => outcome,
            _ => return Err(AuthProductError::BackendConflict),
        };
        if !matches!(self.outcome, CredentialRefreshOutcome::Submitted) {
            return if self.outcome == outcome {
                Ok(())
            } else {
                Err(AuthProductError::BackendConflict)
            };
        }
        self.outcome = outcome;
        self.updated_at = now;
        Ok(())
    }

    /// Publication is a separate owned storage transition. An unconfirmed
    /// submission never becomes an invitation to repeat the issuer call.
    pub fn publish(
        &mut self,
        current: &mut CredentialAccount,
        now: Timestamp,
    ) -> Result<(), AuthProductError> {
        if self.publication != CredentialRefreshPublication::Pending
            || matches!(self.outcome, CredentialRefreshOutcome::Submitted)
        {
            return Ok(());
        }
        if current.id != self.expected_account.id
            || current.provider != self.expected_account.provider
            || !scope_matches(&current.scope, &self.expected_account.scope)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        self.updated_at = now;
        if *current != self.expected_account {
            self.publication = CredentialRefreshPublication::Superseded;
            return Ok(());
        }
        match &self.outcome {
            CredentialRefreshOutcome::Returned(result) => {
                current.status = CredentialAccountStatus::Configured;
                current.access_secret = Some(result.access_secret.clone());
                current.access_expires_at = result.access_expires_at;
                if let Some(refresh) = &result.refresh_secret {
                    current.refresh_secret = Some(refresh.clone());
                }
                current.scopes = result.scopes.clone();
            }
            CredentialRefreshOutcome::Unknown(_) | CredentialRefreshOutcome::UnusableReturn(_) => {
                current.status = CredentialAccountStatus::RefreshFailed;
            }
            _ => return Err(AuthProductError::BackendUnavailable),
        }
        current.updated_at = now;
        self.publication = CredentialRefreshPublication::Applied;
        self.published_account = Some(current.clone());
        Ok(())
    }

    /// In-memory convenience; durable owners record first, then publish using
    /// their own second transaction/CAS and original reconciliation boundary.
    pub fn complete(
        &mut self,
        current: &mut CredentialAccount,
        completion: CredentialRefreshCompletion,
        now: Timestamp,
    ) -> Result<(), AuthProductError> {
        self.record_result(completion, now)?;
        self.publish(current, now)
    }
}
