//! Original OAuth callback custody. Metadata is not a provider or Tool grant.
use crate::{
    AuthErrorCode, AuthFlowId, AuthProductError, AuthProductScope, AuthProviderId,
    AuthorizationCodeHash, CredentialAccount, CredentialAccountLabel, CredentialAccountSnapshotRef,
    OAuthProviderCallbackRequest, OAuthProviderExchange, OAuthTokenDestinations, OpaqueStateHash,
    PkceVerifierHash, ProviderScope, Timestamp,
};
use serde::{Deserialize, Serialize};

/// Private host/storage metadata kept beside the public hash-only flow record.
/// Absence on an old bound flow is missing evidence, never read-latest authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthFlowCustody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_login: Option<Box<crate::NativeDeviceLoginAction>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_account_snapshot: Option<CredentialAccount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_callback: Option<OAuthCallbackAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth_setup: Option<Box<crate::OAuthSetupOriginal>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dcr_registration: Option<crate::DcrRegistrationAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dcr_legacy: Option<crate::DcrLegacySelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_setup: Option<Box<crate::CredentialRequirementSetup>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_selection: Option<Box<crate::CredentialRequirementSelection>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_source: Option<Box<crate::CredentialRequirementSource>>,
}

impl AuthFlowCustody {
    pub fn is_empty(&self) -> bool {
        self.device_login.is_none()
            && self.update_account_snapshot.is_none()
            && self.oauth_callback.is_none()
            && self.oauth_setup.is_none()
            && self.dcr_registration.is_none()
            && self.dcr_legacy.is_none()
            && self.requirement_setup.is_none()
            && self.requirement_selection.is_none()
            && self.requirement_source.is_none()
    }
}

/// Explicit callback payload used by reference-only retries. PKCE is selected
/// host material, so reloading an original never requires a dummy/raw verifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthCallbackReference {
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
    pub provider: AuthProviderId,
    pub opaque_state_hash: OpaqueStateHash,
    pub authorization_code_hash: AuthorizationCodeHash,
    pub account_label: CredentialAccountLabel,
    pub requested_scopes: Vec<ProviderScope>,
}

/// Complete safe request proof. Raw code/verifier/state never enter this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthCallbackOriginal {
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
    pub provider: AuthProviderId,
    pub opaque_state_hash: OpaqueStateHash,
    pub authorization_code_hash: AuthorizationCodeHash,
    pub pkce_verifier_hash: PkceVerifierHash,
    pub account_label: CredentialAccountLabel,
    pub requested_scopes: Vec<ProviderScope>,
}
impl OAuthCallbackOriginal {
    pub fn from_request(
        scope: AuthProductScope,
        flow_id: AuthFlowId,
        opaque_state_hash: OpaqueStateHash,
        request: &OAuthProviderCallbackRequest,
    ) -> Result<Self, AuthProductError> {
        crate::validate_provider_callback_request(request)?;
        let code = request.authorization_code_hash.clone();
        let verifier = request.pkce_verifier_hash.clone();
        Ok(Self {
            scope,
            flow_id,
            provider: request.provider.clone(),
            opaque_state_hash,
            authorization_code_hash: code,
            pkce_verifier_hash: verifier,
            account_label: request.account_label.clone(),
            requested_scopes: request.scopes.clone(),
        })
    }

    pub fn reference(&self) -> OAuthCallbackReference {
        OAuthCallbackReference {
            scope: self.scope.clone(),
            flow_id: self.flow_id,
            provider: self.provider.clone(),
            opaque_state_hash: self.opaque_state_hash.clone(),
            authorization_code_hash: self.authorization_code_hash.clone(),
            account_label: self.account_label.clone(),
            requested_scopes: self.requested_scopes.clone(),
        }
    }
    pub fn verify_reference(&self, other: &OAuthCallbackReference) -> Result<(), AuthProductError> {
        if self.scope != other.scope
            || self.flow_id != other.flow_id
            || self.provider != other.provider
            || self.account_label != other.account_label
            || self.requested_scopes != other.requested_scopes
            || !self
                .opaque_state_hash
                .constant_time_eq(&other.opaque_state_hash)
            || !self
                .authorization_code_hash
                .constant_time_eq(&other.authorization_code_hash)
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
    pub fn verify(&self, other: &Self) -> Result<(), AuthProductError> {
        self.verify_reference(&other.reference())?;
        if !self
            .pkce_verifier_hash
            .constant_time_eq(&other.pkce_verifier_hash)
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OAuthCallbackOutcome {
    Submitted,
    Returned(OAuthProviderExchange),
    UnusableReturn(OAuthProviderExchange),
    Unknown(AuthErrorCode),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthCallbackPublication {
    Pending,
    Applied,
    Superseded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthCallbackOutcomeKind {
    Unconfirmed,
    Returned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthCallbackReceipt {
    pub flow_id: AuthFlowId,
    pub provider: AuthProviderId,
    pub outcome: OAuthCallbackOutcomeKind,
    pub publication: OAuthCallbackPublication,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AuthErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_account_ref: Option<CredentialAccountSnapshotRef>,
}

/// Stored under the existing original flow; no raw provider payloads or tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthCallbackAction {
    pub original: OAuthCallbackOriginal,
    pub destinations: OAuthTokenDestinations,
    /// Selected at flow creation, never repaired by reading a later account.
    pub expected_account: Option<CredentialAccount>,
    pub outcome: OAuthCallbackOutcome,
    pub publication: OAuthCallbackPublication,
    pub published_account: Option<CredentialAccount>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
#[derive(Debug, Clone)]
pub struct OAuthCallbackPrepared {
    pub original: OAuthCallbackOriginal,
    pub destinations: OAuthTokenDestinations,
}
/// One coherent internal owner snapshot; public routes render only its safe
/// flow fields and receipt, never serialize the action's storage metadata.
#[derive(Debug, Clone)]
pub struct OAuthCallbackState {
    pub flow: crate::AuthFlowRecord,
    pub action: OAuthCallbackAction,
}
#[derive(Debug, Clone)]
pub enum OAuthCallbackAdmission {
    Dispatch(OAuthCallbackState),
    Original(OAuthCallbackState),
}
#[derive(Debug, Clone)]
pub struct OAuthCallbackCompletion {
    pub dispatched: OAuthCallbackAction,
    pub outcome: OAuthCallbackOutcome,
}
impl OAuthCallbackAction {
    pub fn needs_publication(&self) -> bool {
        self.publication == OAuthCallbackPublication::Pending
            && !matches!(self.outcome, OAuthCallbackOutcome::Submitted)
    }
    pub fn receipt(&self) -> Result<OAuthCallbackReceipt, AuthProductError> {
        let (outcome, error) = match &self.outcome {
            OAuthCallbackOutcome::Submitted => (OAuthCallbackOutcomeKind::Unconfirmed, None),
            OAuthCallbackOutcome::Returned(_) => (OAuthCallbackOutcomeKind::Returned, None),
            OAuthCallbackOutcome::UnusableReturn(_) => (
                OAuthCallbackOutcomeKind::Unconfirmed,
                Some(AuthErrorCode::TokenExchangeFailed),
            ),
            OAuthCallbackOutcome::Unknown(error) => {
                (OAuthCallbackOutcomeKind::Unconfirmed, Some(*error))
            }
        };
        Ok(OAuthCallbackReceipt {
            flow_id: self.original.flow_id,
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
    pub fn same_intent(&self, other: &Self) -> bool {
        self.original == other.original
            && self.expected_account == other.expected_account
            && self.destinations == other.destinations
            && self.created_at == other.created_at
    }
    /// Persist this result before any final account/flow publication attempt.
    /// Repeated same metadata is read-only; contradictory replies are conflicts.
    pub fn record_result(
        &mut self,
        completion: OAuthCallbackCompletion,
        now: Timestamp,
    ) -> Result<(), AuthProductError> {
        if !self.same_intent(&completion.dispatched)
            || !matches!(
                completion.dispatched.outcome,
                OAuthCallbackOutcome::Submitted
            )
            || completion.dispatched.publication != OAuthCallbackPublication::Pending
        {
            return Err(AuthProductError::BackendConflict);
        }
        let outcome = match completion.outcome {
            OAuthCallbackOutcome::Returned(result) => {
                let account_id = self
                    .expected_account
                    .as_ref()
                    .map(|account| account.id)
                    .unwrap_or_else(|| {
                        crate::CredentialAccountId::from_uuid(self.original.flow_id.as_uuid())
                    });
                let valid = result.provider == self.original.provider
                    && result.account_label == self.original.account_label
                    && result
                        .authorization_code_hash
                        .constant_time_eq(&self.original.authorization_code_hash)
                    && result
                        .pkce_verifier_hash
                        .constant_time_eq(&self.original.pkce_verifier_hash)
                    && self
                        .original
                        .requested_scopes
                        .iter()
                        .all(|requested| result.scopes.contains(requested))
                    && result.access_secret == self.destinations.access_secret
                    && result
                        .refresh_secret
                        .as_ref()
                        .is_none_or(|handle| handle == &self.destinations.refresh_secret)
                    && result
                        .account_id
                        .is_none_or(|returned| returned == account_id);
                if valid {
                    OAuthCallbackOutcome::Returned(result)
                } else {
                    OAuthCallbackOutcome::UnusableReturn(result)
                }
            }
            outcome @ OAuthCallbackOutcome::Unknown(_) => outcome,
            _ => return Err(AuthProductError::BackendConflict),
        };
        if !matches!(self.outcome, OAuthCallbackOutcome::Submitted) {
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
}

impl AuthFlowCustody {
    /// Pure transition under the selected owner's original flow/account snapshot.
    /// Existing originals keep automatic selections and never grant another call.
    pub fn admit(
        &mut self,
        flow: &mut crate::AuthFlowRecord,
        current: Option<&CredentialAccount>,
        prepared: OAuthCallbackPrepared,
        now: Timestamp,
    ) -> Result<OAuthCallbackAdmission, AuthProductError> {
        let original = &prepared.original;
        if original.flow_id != flow.id || !crate::scope_matches(&original.scope, &flow.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if let Some(state) = self.original_state(flow, &original.reference())? {
            state.action.original.verify(original)?;
            return Ok(OAuthCallbackAdmission::Original(state));
        }
        if !matches!(&flow.challenge, Some(crate::AuthChallenge::OAuthUrl { .. })) {
            return Err(AuthProductError::MalformedCallback);
        }
        crate::domain::validate_callback_claim(flow, &original.scope, &prepared, now)?;
        match (&flow.update_binding, &self.update_account_snapshot, current) {
            (None, None, None) => {}
            (Some(binding), Some(expected), Some(current)) => {
                if expected.id != binding.account_id
                    || expected.provider != flow.provider
                    || !crate::scope_matches(&expected.scope, &flow.scope)
                {
                    return Err(AuthProductError::CrossScopeDenied);
                }
                crate::domain::validate_bound_update_authority(expected, binding)?;
                if current != expected {
                    return Err(AuthProductError::BackendConflict);
                }
            }
            (Some(_), None, _) => return Err(AuthProductError::BackendUnavailable),
            _ => return Err(AuthProductError::BackendConflict),
        }
        if prepared.destinations.access_secret == prepared.destinations.refresh_secret {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(account) = &self.update_account_snapshot {
            let reused = |handle: &brassclaw_host_api::SecretHandle| {
                account.access_secret.as_ref() == Some(handle)
                    || account.refresh_secret.as_ref() == Some(handle)
            };
            if reused(&prepared.destinations.access_secret)
                || reused(&prepared.destinations.refresh_secret)
            {
                return Err(AuthProductError::BackendConflict);
            }
        }
        let action = OAuthCallbackAction {
            original: prepared.original,
            destinations: prepared.destinations,
            expected_account: self.update_account_snapshot.clone(),
            outcome: OAuthCallbackOutcome::Submitted,
            publication: OAuthCallbackPublication::Pending,
            published_account: None,
            created_at: now,
            updated_at: now,
        };
        flow.status = crate::AuthFlowStatus::CallbackReceived;
        flow.updated_at = now;
        flow.authorization_code_hash = Some(action.original.authorization_code_hash.clone());
        self.oauth_callback = Some(action.clone());
        Ok(OAuthCallbackAdmission::Dispatch(OAuthCallbackState {
            flow: flow.clone(),
            action,
        }))
    }
}
impl OAuthCallbackAction {
    pub fn target_account_id(&self) -> crate::CredentialAccountId {
        self.expected_account
            .as_ref()
            .map(|account| account.id)
            .unwrap_or_else(|| {
                crate::CredentialAccountId::from_uuid(self.original.flow_id.as_uuid())
            })
    }
    /// Stage one result without changing the account. Backend finalization
    /// commits the returned account and this final flow/receipt together.
    pub fn publish(
        &mut self,
        flow: &mut crate::AuthFlowRecord,
        current: Option<&CredentialAccount>,
        now: Timestamp,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        if !self.needs_publication() {
            return Ok(None);
        }
        if flow.id != self.original.flow_id
            || !crate::scope_matches(&flow.scope, &self.original.scope)
            || flow.provider != self.original.provider
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if crate::is_terminal_status(flow.status) {
            // Cancellation, expiry and any independently terminal result win.
            self.publication = OAuthCallbackPublication::Superseded;
            self.updated_at = now;
            return Ok(None);
        }
        if flow.expires_at <= now {
            flow.status = crate::AuthFlowStatus::Expired;
            flow.error = Some(AuthErrorCode::UnknownOrExpiredFlow);
            flow.updated_at = now;
            self.publication = OAuthCallbackPublication::Superseded;
            self.updated_at = now;
            return Ok(None);
        }
        if !matches!(
            flow.status,
            crate::AuthFlowStatus::CallbackReceived | crate::AuthFlowStatus::Completing
        ) {
            return Err(AuthProductError::BackendConflict);
        }
        self.updated_at = now;
        let result = match &self.outcome {
            OAuthCallbackOutcome::Returned(result) => result,
            OAuthCallbackOutcome::Unknown(error) => {
                flow.status = crate::AuthFlowStatus::Failed;
                flow.error = Some(*error);
                flow.updated_at = now;
                self.publication = OAuthCallbackPublication::Applied;
                return Ok(None);
            }
            OAuthCallbackOutcome::UnusableReturn(_) => {
                flow.status = crate::AuthFlowStatus::Failed;
                flow.error = Some(AuthErrorCode::TokenExchangeFailed);
                flow.updated_at = now;
                self.publication = OAuthCallbackPublication::Applied;
                return Ok(None);
            }
            OAuthCallbackOutcome::Submitted => return Ok(None),
        };
        if current != self.expected_account.as_ref() {
            flow.status = crate::AuthFlowStatus::Failed;
            flow.error = Some(AuthErrorCode::FlowAlreadyTerminal);
            flow.updated_at = now;
            self.publication = OAuthCallbackPublication::Superseded;
            return Ok(None);
        }
        let account = if let Some(expected) = &self.expected_account {
            let mut account = expected.clone();
            crate::domain::update_account_from_exchange(&mut account, result, now);
            account
        } else {
            CredentialAccount {
                id: self.target_account_id(),
                scope: flow.scope.clone(),
                provider: flow.provider.clone(),
                label: result.account_label.clone(),
                status: crate::CredentialAccountStatus::Configured,
                ownership: crate::CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(result.access_secret.clone()),
                access_expires_at: result.access_expires_at,
                refresh_secret: result.refresh_secret.clone(),
                scopes: result.scopes.clone(),
                created_at: now,
                updated_at: now,
            }
        };
        crate::flow::complete_credential_flow(
            flow,
            account.id,
            self.original.authorization_code_hash.clone(),
            self.original.pkce_verifier_hash.clone(),
            now,
        );
        self.publication = OAuthCallbackPublication::Applied;
        self.published_account = Some(account.clone());
        Ok(Some(account))
    }
}

impl AuthFlowCustody {
    pub fn original_state(
        &self,
        flow: &crate::AuthFlowRecord,
        reference: &OAuthCallbackReference,
    ) -> Result<Option<OAuthCallbackState>, AuthProductError> {
        if flow.id != reference.flow_id || !crate::scope_matches(&flow.scope, &reference.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        self.verify_oauth_setup_callback(flow, reference)?;
        let Some(action) = &self.oauth_callback else {
            return Ok(None);
        };
        action.original.verify_reference(reference)?;
        if action.expected_account != self.update_account_snapshot
            || flow.provider != action.original.provider
            || !flow
                .opaque_state_hash
                .as_ref()
                .is_some_and(|hash| hash.constant_time_eq(&action.original.opaque_state_hash))
            || !flow
                .pkce_verifier_hash
                .as_ref()
                .is_some_and(|hash| hash.constant_time_eq(&action.original.pkce_verifier_hash))
            || !flow
                .authorization_code_hash
                .as_ref()
                .is_some_and(|hash| hash.constant_time_eq(&action.original.authorization_code_hash))
        {
            return Err(AuthProductError::BackendConflict);
        }
        match (&flow.update_binding, &action.expected_account) {
            (None, None) => {}
            (Some(binding), Some(account))
                if binding.account_id == account.id
                    && account.provider == flow.provider
                    && crate::scope_matches(&account.scope, &flow.scope) =>
            {
                crate::domain::validate_bound_update_authority(account, binding)?;
            }
            _ => return Err(AuthProductError::BackendConflict),
        }
        Ok(Some(OAuthCallbackState {
            flow: flow.clone(),
            action: action.clone(),
        }))
    }
}
impl OAuthCallbackState {
    pub fn completed_flow(self) -> Result<crate::AuthFlowRecord, AuthProductError> {
        if self.flow.status == crate::AuthFlowStatus::Completed
            && self.action.publication == OAuthCallbackPublication::Applied
            && matches!(self.action.outcome, OAuthCallbackOutcome::Returned(_))
            && self
                .action
                .published_account
                .as_ref()
                .is_some_and(|account| {
                    Some(account.id) == self.flow.credential_account_id
                        && account.id == self.action.target_account_id()
                        && account.provider == self.action.original.provider
                        && crate::scope_matches(&account.scope, &self.action.original.scope)
                })
        {
            return Ok(self.flow);
        }
        let code = self.flow.error.unwrap_or(match self.flow.status {
            crate::AuthFlowStatus::Canceled => AuthErrorCode::Canceled,
            crate::AuthFlowStatus::Expired => AuthErrorCode::UnknownOrExpiredFlow,
            _ => AuthErrorCode::BackendUnavailable,
        });
        Err(match code {
            AuthErrorCode::UnknownOrExpiredFlow => AuthProductError::UnknownOrExpiredFlow,
            AuthErrorCode::CrossScopeDenied => AuthProductError::CrossScopeDenied,
            AuthErrorCode::ProviderDenied => AuthProductError::ProviderDenied,
            AuthErrorCode::TokenExchangeFailed => AuthProductError::TokenExchangeFailed,
            AuthErrorCode::RefreshFailed => AuthProductError::RefreshFailed,
            AuthErrorCode::CredentialMissing => AuthProductError::CredentialMissing,
            AuthErrorCode::AccountSelectionRequired => AuthProductError::AccountSelectionRequired,
            AuthErrorCode::BackendUnavailable => AuthProductError::BackendUnavailable,
            AuthErrorCode::MalformedConfig => AuthProductError::MalformedConfig,
            AuthErrorCode::MalformedCallback => AuthProductError::MalformedCallback,
            AuthErrorCode::Canceled => AuthProductError::Canceled,
            AuthErrorCode::FlowAlreadyTerminal => AuthProductError::FlowAlreadyTerminal,
            AuthErrorCode::InvalidRequest => AuthProductError::InvalidRequest {
                reason: "invalid callback metadata".into(),
            },
        })
    }
}
