//! Original OAuth setup metadata. Private verifier/client bytes stay in the
//! existing encrypted host owner; these records never establish a grant.
use crate::{
    AuthChallenge, AuthContinuationRef, AuthErrorCode, AuthFlowId, AuthFlowKind, AuthFlowRecord,
    AuthFlowStatus, AuthProductError, AuthProductScope, AuthProviderId, CredentialAccount,
    CredentialAccountLabel, CredentialAccountUpdateBinding, CredentialRequirementSetup,
    OAuthAuthorizationUrl, OpaqueStateHash, PkceVerifierHash, ProviderScope, Timestamp,
};
use brassclaw_host_api::SecretHandle;
use serde::{Deserialize, Serialize};

/// Exact first definition, captured together with private client/PKCE material.
/// Account metadata is a fixed comparison snapshot, never a fresh selection or
/// permission. It remains valid as historical data after account deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthSetupDefinition {
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
    pub provider: AuthProviderId,
    pub account_label: CredentialAccountLabel,
    pub requested_scopes: Vec<ProviderScope>,
    pub continuation: AuthContinuationRef,
    pub requirement_setup: Option<Box<CredentialRequirementSetup>>,
    pub update_binding: Option<CredentialAccountUpdateBinding>,
    pub update_account_snapshot: Option<CredentialAccount>,
    pub authorization_url: OAuthAuthorizationUrl,
    pub opaque_state_hash: OpaqueStateHash,
    pub pkce_verifier_hash: PkceVerifierHash,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
}

/// Authenticated encrypted generation. The actual host owner must authenticate
/// this reference and context, then read only this generation conditionally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthSetupMaterialReference {
    pub handle: SecretHandle,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthSetupOriginal {
    pub definition: OAuthSetupDefinition,
    pub preparation: OAuthSetupMaterialReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthSetupRetirement {
    Canceled,
    Expired,
}

impl OAuthSetupDefinition {
    /// Structural checks only. No current account, live eligibility or encrypted
    /// generation can be established by deserializing this data.
    pub fn validate(&self) -> Result<(), AuthProductError> {
        if self.flow_id.as_uuid().is_nil() || self.expires_at <= self.created_at {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(setup) = &self.requirement_setup {
            setup.verify_oauth(&self.scope, &self.provider, &self.requested_scopes)?;
        }
        match (&self.update_binding, &self.update_account_snapshot) {
            (None, None) => {}
            (Some(binding), Some(account)) => {
                if binding.account_id != account.id || account.id.as_uuid().is_nil() {
                    return Err(AuthProductError::BackendConflict);
                }
                if account.scope != self.scope || account.provider != self.provider {
                    return Err(AuthProductError::CrossScopeDenied);
                }
                crate::domain::validate_bound_update_authority(account, binding)?;
            }
            _ => return Err(AuthProductError::BackendConflict),
        }
        Ok(())
    }

    /// This is not a persisted flow, dispatch or new grant. The concrete Auth
    /// owner must authenticate the original and win its absent-record CAS.
    pub fn awaiting_flow(&self, now: Timestamp) -> Result<AuthFlowRecord, AuthProductError> {
        self.validate()?;
        if self.expires_at <= now {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        Ok(self.flow_record(now, AuthFlowStatus::AwaitingUser, None))
    }

    /// Reconstruct an expired/cancelled projection without reading today's
    /// account. It cannot publish a challenge or admit a provider exchange.
    pub fn retired_flow(
        &self,
        now: Timestamp,
        reason: OAuthSetupRetirement,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.validate()?;
        let (status, error) = match reason {
            OAuthSetupRetirement::Expired if self.expires_at > now => {
                return Err(AuthProductError::BackendConflict);
            }
            OAuthSetupRetirement::Expired => {
                (AuthFlowStatus::Expired, AuthErrorCode::UnknownOrExpiredFlow)
            }
            OAuthSetupRetirement::Canceled if self.expires_at <= now => {
                (AuthFlowStatus::Expired, AuthErrorCode::UnknownOrExpiredFlow)
            }
            OAuthSetupRetirement::Canceled => (AuthFlowStatus::Canceled, AuthErrorCode::Canceled),
        };
        Ok(self.flow_record(now, status, Some(error)))
    }

    fn flow_record(
        &self,
        now: Timestamp,
        status: AuthFlowStatus,
        error: Option<AuthErrorCode>,
    ) -> AuthFlowRecord {
        AuthFlowRecord {
            id: self.flow_id,
            scope: self.scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            status,
            provider: self.provider.clone(),
            challenge: (status == AuthFlowStatus::AwaitingUser).then(|| AuthChallenge::OAuthUrl {
                authorization_url: self.authorization_url.clone(),
                expires_at: self.expires_at,
            }),
            continuation: self.continuation.clone(),
            credential_account_id: None,
            update_binding: self.update_binding.clone(),
            opaque_state_hash: Some(self.opaque_state_hash.clone()),
            pkce_verifier_hash: Some(self.pkce_verifier_hash.clone()),
            authorization_code_hash: None,
            error,
            continuation_emitted_at: None,
            created_at: self.created_at,
            updated_at: now,
            expires_at: self.expires_at,
        }
    }

    /// Mutable completion fields are not a replacement setup definition.
    pub fn verify_flow(&self, flow: &AuthFlowRecord) -> Result<(), AuthProductError> {
        self.validate()?;
        if flow.id != self.flow_id || flow.scope != self.scope {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if flow.kind != AuthFlowKind::IntegrationCredential
            || flow.status == AuthFlowStatus::Pending
            || flow.provider != self.provider
            || flow.continuation != self.continuation
            || flow.update_binding != self.update_binding
            || flow.created_at != self.created_at
            || flow.expires_at != self.expires_at
            || !flow
                .opaque_state_hash
                .as_ref()
                .is_some_and(|h| h.constant_time_eq(&self.opaque_state_hash))
            || !flow
                .pkce_verifier_hash
                .as_ref()
                .is_some_and(|h| h.constant_time_eq(&self.pkce_verifier_hash))
        {
            return Err(AuthProductError::BackendConflict);
        }
        match &flow.challenge {
            Some(AuthChallenge::OAuthUrl {
                authorization_url,
                expires_at,
            }) if authorization_url == &self.authorization_url
                && expires_at == &self.expires_at => {}
            None if crate::is_terminal_status(flow.status) => {}
            _ => return Err(AuthProductError::BackendConflict),
        }
        Ok(())
    }

    /// No own generation reference appears here: context and private bytes are
    /// written atomically by the existing encrypted owner, without a hash cycle.
    pub fn preparation_context(&self) -> serde_json::Value {
        serde_json::json!({"format":"oauth-setup-original/1", "definition":self})
    }
}

impl OAuthSetupOriginal {
    pub fn validate(&self) -> Result<(), AuthProductError> {
        self.definition.validate()?;
        if self.preparation.reference.is_empty()
            || self.preparation.handle == self.definition.exchange_use_handle()?
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }

    pub fn verify(&self, other: &Self) -> Result<(), AuthProductError> {
        self.validate()?;
        other.validate()?;
        if self.definition.scope != other.definition.scope
            || self.definition.flow_id != other.definition.flow_id
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if !self
            .definition
            .opaque_state_hash
            .constant_time_eq(&other.definition.opaque_state_hash)
            || !self
                .definition
                .pkce_verifier_hash
                .constant_time_eq(&other.definition.pkce_verifier_hash)
            || self != other
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}

/// State retained by the existing Auth-flow owner. This is readback, not a
/// callback claim or permission to replay an issuer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthSetupState {
    pub flow: AuthFlowRecord,
    pub original: OAuthSetupOriginal,
}

impl OAuthSetupDefinition {
    /// A stable later-use identity survives input retirement. Its presence is
    /// never evidence of success, but prevents recreating a missing first flow.
    pub fn exchange_use_handle(&self) -> Result<SecretHandle, AuthProductError> {
        SecretHandle::new(format!("oauth-setup-exchange-use-{}", self.flow_id))
            .map_err(|_| AuthProductError::BackendConflict)
    }
}

impl crate::AuthFlowCustody {
    /// Pure construction only. Native owners authenticate the encrypted first
    /// definition and all later-use absences before the existing absent-flow CAS.
    pub fn prepared_oauth_setup(
        original: OAuthSetupOriginal,
        retirement: Option<OAuthSetupRetirement>,
        now: Timestamp,
    ) -> Result<(AuthFlowRecord, Self), AuthProductError> {
        original.validate()?;
        let flow = match retirement {
            Some(reason) => original.definition.retired_flow(now, reason)?,
            None if original.definition.expires_at <= now => original
                .definition
                .retired_flow(now, OAuthSetupRetirement::Expired)?,
            None => original.definition.awaiting_flow(now)?,
        };
        let custody = Self {
            update_account_snapshot: original.definition.update_account_snapshot.clone(),
            requirement_setup: original.definition.requirement_setup.clone(),
            oauth_setup: Some(Box::new(original)),
            ..Self::default()
        };
        custody.checked_oauth_setup(&flow)?;
        Ok((flow, custody))
    }

    pub(crate) fn checked_oauth_setup(
        &self,
        flow: &AuthFlowRecord,
    ) -> Result<Option<&OAuthSetupOriginal>, AuthProductError> {
        let Some(original) = self.oauth_setup.as_deref() else {
            return Ok(None);
        };
        original.validate()?;
        original.definition.verify_flow(flow)?;
        if self.dcr_registration.is_some()
            || self.dcr_legacy.is_some()
            || self.requirement_setup != original.definition.requirement_setup
            || self.update_account_snapshot != original.definition.update_account_snapshot
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(Some(original))
    }

    pub fn oauth_setup_state(
        &self,
        flow: &AuthFlowRecord,
    ) -> Result<Option<OAuthSetupState>, AuthProductError> {
        Ok(self
            .checked_oauth_setup(flow)?
            .map(|original| OAuthSetupState {
                flow: flow.clone(),
                original: original.clone(),
            }))
    }

    pub(crate) fn verify_oauth_setup_callback(
        &self,
        flow: &AuthFlowRecord,
        reference: &crate::OAuthCallbackReference,
    ) -> Result<(), AuthProductError> {
        if let Some(original) = self.checked_oauth_setup(flow)? {
            let definition = &original.definition;
            if reference.flow_id != definition.flow_id || reference.scope != definition.scope {
                return Err(AuthProductError::CrossScopeDenied);
            }
            if reference.provider != definition.provider
                || reference.account_label != definition.account_label
                || reference.requested_scopes != definition.requested_scopes
                || !reference
                    .opaque_state_hash
                    .constant_time_eq(&definition.opaque_state_hash)
            {
                return Err(AuthProductError::BackendConflict);
            }
        }
        Ok(())
    }
}
