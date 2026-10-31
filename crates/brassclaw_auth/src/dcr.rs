//! Original dynamic-client registration custody within the existing AuthFlow.
//! These records fence technical issuer I/O; they are not invocation grants.
use crate::{
    AuthChallenge, AuthContinuationRef, AuthErrorCode, AuthFlowCustody, AuthFlowId, AuthFlowKind,
    AuthFlowRecord, AuthFlowStatus, AuthProductError, AuthProductScope, AuthProviderId,
    CredentialAccountLabel, CredentialAccountUpdateBinding, OAuthAuthorizationUrl, OpaqueStateHash,
    PkceVerifierHash, ProviderScope, Timestamp,
};
use brassclaw_host_api::SecretHandle;
use serde::{Deserialize, Serialize};

/// Authenticated encrypted-generation identity, never a raw-value fingerprint.
/// The concrete owner must verify the live custody kind and exact context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrMaterialReference {
    pub handle: SecretHandle,
    pub reference: String,
}

impl DcrMaterialReference {
    pub(crate) fn validate(&self) -> Result<(), AuthProductError> {
        if self.reference.is_empty() {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}

/// Complete safe first-request identity. Raw preparation (including PKCE),
/// registration replies and registration tokens belong to SecretStore originals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrRegistrationOriginal {
    /// Complete usage and previous selection captured before the challenge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_setup: Option<Box<crate::CredentialRequirementSetup>>,
    pub scope: AuthProductScope,
    pub flow_id: AuthFlowId,
    pub provider: AuthProviderId,
    pub account_label: CredentialAccountLabel,
    pub requested_scopes: Vec<ProviderScope>,
    pub continuation: AuthContinuationRef,
    pub update_binding: Option<CredentialAccountUpdateBinding>,
    pub opaque_state_hash: OpaqueStateHash,
    pub pkce_verifier_hash: PkceVerifierHash,
    pub expires_at: Timestamp,
    pub preparation: DcrMaterialReference,
}

impl DcrRegistrationOriginal {
    /// Context shared with the existing encrypted original owner. The source's
    /// own reference is deliberately absent, avoiding a circular identity.
    pub fn preparation_context(&self) -> serde_json::Value {
        let mut context = serde_json::json!({
            "format":"dcr-registration-preparation/1",
            "scope":self.scope, "flow_id":self.flow_id, "provider":self.provider,
            "account_label":self.account_label, "requested_scopes":self.requested_scopes,
            "continuation":self.continuation, "update_binding":self.update_binding,
            "opaque_state_hash":self.opaque_state_hash,
            "pkce_verifier_hash":self.pkce_verifier_hash, "expires_at":self.expires_at,
        });
        // Absent custody preserves the exact historical context shape/hash.
        if let Some(setup) = &self.requirement_setup {
            context["format"] = serde_json::json!("dcr-registration-preparation/2");
            context["requirement_setup"] = serde_json::json!(setup);
        }
        context
    }

    pub fn verify(&self, other: &Self) -> Result<(), AuthProductError> {
        if self.scope != other.scope || self.flow_id != other.flow_id {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if self != other {
            return Err(AuthProductError::BackendConflict);
        }
        self.preparation.validate()
    }

    pub(crate) fn verify_flow(&self, flow: &AuthFlowRecord) -> Result<(), AuthProductError> {
        if flow.id != self.flow_id || flow.scope != self.scope {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if flow.kind != AuthFlowKind::IntegrationCredential
            || flow.provider != self.provider
            || flow.continuation != self.continuation
            || flow.update_binding != self.update_binding
            || flow.expires_at != self.expires_at
            || flow.opaque_state_hash.as_ref() != Some(&self.opaque_state_hash)
            || flow.pkce_verifier_hash.as_ref() != Some(&self.pkce_verifier_hash)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some(setup) = &self.requirement_setup {
            setup.verify_flow(flow)?;
            setup.verify_oauth(&self.scope, &self.provider, &self.requested_scopes)?;
        }
        self.preparation.validate()
    }

    /// Build the pending local record before ANY registration dispatch.
    /// Account snapshot capture belongs to the same concrete flow owner.
    pub fn pending_flow(&self, now: Timestamp) -> Result<AuthFlowRecord, AuthProductError> {
        self.preparation.validate()?;
        if self.expires_at <= now {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        Ok(self.preparation_flow(now, AuthFlowStatus::Pending, None))
    }

    fn preparation_flow(
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
            challenge: None,
            continuation: self.continuation.clone(),
            credential_account_id: None,
            update_binding: self.update_binding.clone(),
            opaque_state_hash: Some(self.opaque_state_hash.clone()),
            pkce_verifier_hash: Some(self.pkce_verifier_hash.clone()),
            authorization_code_hash: None,
            error,
            continuation_emitted_at: None,
            created_at: now,
            updated_at: now,
            expires_at: self.expires_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DcrRegistrationOutcome {
    Prepared,
    Submitted {
        submitted_at: Timestamp,
    },
    Returned {
        submitted_at: Timestamp,
        status: u16,
        response: DcrMaterialReference,
    },
    Unknown {
        submitted_at: Timestamp,
        error: AuthErrorCode,
    },
}

/// Ready metadata names the exact retained outputs. A live SecretStore proof is
/// required before publication; declarations alone must never expose a URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrRegistrationChallenge {
    pub authorization_url: OAuthAuthorizationUrl,
    pub pkce: DcrMaterialReference,
    pub client: DcrMaterialReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DcrChallengePublication {
    Pending,
    Ready(DcrRegistrationChallenge),
    Suppressed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrRegistrationAction {
    pub original: DcrRegistrationOriginal,
    pub outcome: DcrRegistrationOutcome,
    pub publication: DcrChallengePublication,
    #[serde(default)]
    pub cleanup: crate::DcrCleanupState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retirement: Option<crate::DcrRetirement>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DcrRegistrationState {
    pub flow: AuthFlowRecord,
    pub action: DcrRegistrationAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DcrRegistrationAdmission {
    /// Only the owner that ACKNOWLEDGED the first durable claim may dispatch.
    Dispatch(DcrRegistrationState),
    /// Includes lost-write acknowledgement: this never permits a new POST.
    Original(DcrRegistrationState),
}

#[derive(Debug, Clone)]
pub struct DcrRegistrationCompletion {
    pub dispatched: DcrRegistrationAction,
    pub outcome: DcrRegistrationOutcome,
}

impl DcrRegistrationAction {
    pub fn prepared(original: DcrRegistrationOriginal, now: Timestamp) -> Self {
        Self {
            original,
            outcome: DcrRegistrationOutcome::Prepared,
            publication: DcrChallengePublication::Pending,
            cleanup: crate::DcrCleanupState::Retained,
            retirement: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn response_context(&self, status: u16) -> Result<serde_json::Value, AuthProductError> {
        let submitted_at = self
            .submitted_at()
            .ok_or(AuthProductError::BackendConflict)?;
        Ok(serde_json::json!({"format":"dcr-registration-response/1",
            "original":self.original, "submitted_at":submitted_at, "status":status}))
    }

    pub fn submitted_at(&self) -> Option<Timestamp> {
        match self.outcome {
            DcrRegistrationOutcome::Prepared => None,
            DcrRegistrationOutcome::Submitted { submitted_at }
            | DcrRegistrationOutcome::Returned { submitted_at, .. }
            | DcrRegistrationOutcome::Unknown { submitted_at, .. } => Some(submitted_at),
        }
    }

    pub fn output_context(
        &self,
        usage: &str,
        authorization_url: &OAuthAuthorizationUrl,
    ) -> Result<serde_json::Value, AuthProductError> {
        if !matches!(usage, "pkce" | "client") {
            return Err(AuthProductError::BackendConflict);
        }
        let DcrRegistrationOutcome::Returned {
            submitted_at,
            status: 200..=299,
            response,
        } = &self.outcome
        else {
            return Err(AuthProductError::BackendConflict);
        };
        Ok(
            serde_json::json!({"format":"dcr-registration-output/1", "usage":usage,
            "original":self.original, "submitted_at":submitted_at, "response":response,
            "authorization_url":authorization_url}),
        )
    }

    /// Record replies even after cancellation/expiry: the effect already began.
    /// No cancellation/error may erase the exact original response identity.
    pub fn record_result(
        &mut self,
        completion: DcrRegistrationCompletion,
        now: Timestamp,
    ) -> Result<(), AuthProductError> {
        if self.original != completion.dispatched.original
            || self.created_at != completion.dispatched.created_at
            || !matches!(
                completion.dispatched.publication,
                DcrChallengePublication::Pending
            )
        {
            return Err(AuthProductError::BackendConflict);
        }
        let DcrRegistrationOutcome::Submitted { submitted_at } = completion.dispatched.outcome
        else {
            return Err(AuthProductError::BackendConflict);
        };
        let returned_at = match &completion.outcome {
            DcrRegistrationOutcome::Returned {
                submitted_at,
                status,
                response,
            } => {
                if !(200..600).contains(status) {
                    return Err(AuthProductError::BackendConflict);
                }
                response.validate()?;
                if response.handle == self.original.preparation.handle {
                    return Err(AuthProductError::BackendConflict);
                }
                *submitted_at
            }
            DcrRegistrationOutcome::Unknown { submitted_at, .. } => *submitted_at,
            _ => return Err(AuthProductError::BackendConflict),
        };
        if returned_at != submitted_at || self.submitted_at() != Some(submitted_at) {
            return Err(AuthProductError::BackendConflict);
        }
        if self.outcome == completion.outcome {
            return Ok(());
        }
        if !matches!(self.outcome, DcrRegistrationOutcome::Submitted { .. }) {
            return Err(AuthProductError::BackendConflict);
        }
        self.outcome = completion.outcome;
        self.updated_at = now;
        Ok(())
    }
}

impl AuthFlowCustody {
    /// Restore a terminal projection of an authenticated, unsubmitted preparation.
    /// Concrete owners must authenticate the encrypted original, reject every
    /// later-stage artifact and win an absent-flow CAS before claiming this state.
    /// No current account snapshot is selected: this state can never dispatch.
    pub fn retired_dcr_preparation(
        original: DcrRegistrationOriginal,
        reason: crate::DcrCleanupReason,
        now: Timestamp,
    ) -> Result<(AuthFlowRecord, Self), AuthProductError> {
        original.preparation.validate()?;
        let reason = match reason {
            crate::DcrCleanupReason::Expired if original.expires_at <= now => reason,
            crate::DcrCleanupReason::Canceled if original.expires_at <= now => {
                crate::DcrCleanupReason::Expired
            }
            crate::DcrCleanupReason::Canceled => reason,
            _ => return Err(AuthProductError::BackendConflict),
        };
        let (status, error) = match reason {
            crate::DcrCleanupReason::Expired => {
                (AuthFlowStatus::Expired, AuthErrorCode::UnknownOrExpiredFlow)
            }
            crate::DcrCleanupReason::Canceled => {
                (AuthFlowStatus::Canceled, AuthErrorCode::Canceled)
            }
            crate::DcrCleanupReason::Failed => return Err(AuthProductError::BackendConflict),
        };
        let flow = original.preparation_flow(now, status, Some(error));
        original.verify_flow(&flow)?;
        let requirement_setup = original.requirement_setup.clone();
        let mut action = DcrRegistrationAction::prepared(original, now);
        action.publication = DcrChallengePublication::Suppressed;
        action.cleanup = crate::DcrCleanupState::NotRegistered {
            reason,
            requested_at: now,
        };
        Ok((
            flow,
            Self {
                update_account_snapshot: None,
                oauth_callback: None,
                oauth_setup: None,
                dcr_registration: Some(action),
                dcr_legacy: None,
                requirement_setup,
                requirement_selection: None,
                requirement_source: None,
            },
        ))
    }

    pub fn attach_dcr_original(
        &mut self,
        flow: &AuthFlowRecord,
        original: DcrRegistrationOriginal,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        original.verify_flow(flow)?;
        if self.dcr_registration.is_some()
            || self.oauth_setup.is_some()
            || self.dcr_legacy.is_some()
            || self.oauth_callback.is_some()
            || self.requirement_setup.is_some()
            || self.requirement_selection.is_some()
            || self.requirement_source.is_some()
            || flow.status != AuthFlowStatus::Pending
            || flow.challenge.is_some()
            || flow.update_binding.is_some() != self.update_account_snapshot.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        self.requirement_setup = original.requirement_setup.clone();
        let action = DcrRegistrationAction::prepared(original, flow.created_at);
        self.dcr_registration = Some(action.clone());
        Ok(DcrRegistrationState {
            flow: flow.clone(),
            action,
        })
    }

    pub fn record_dcr_result(
        &mut self,
        flow: &mut AuthFlowRecord,
        completion: DcrRegistrationCompletion,
        now: Timestamp,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let state = self
            .dcr_original_state(flow, &completion.dispatched.original)?
            .ok_or(AuthProductError::BackendConflict)?;
        let mut action = state.action;
        action.record_result(completion, now)?;
        if action.publication == DcrChallengePublication::Pending
            && (crate::is_terminal_status(flow.status) || flow.expires_at <= now)
        {
            action.publication = DcrChallengePublication::Suppressed;
            action.updated_at = now;
            if !crate::is_terminal_status(flow.status) {
                flow.status = AuthFlowStatus::Expired;
                flow.error = Some(AuthErrorCode::UnknownOrExpiredFlow);
                flow.updated_at = now;
            }
        }
        self.dcr_registration = Some(action);
        if crate::is_terminal_status(flow.status) || flow.expires_at <= now {
            self.require_dcr_cleanup(flow, now)?;
        }
        let action = self
            .dcr_registration
            .clone()
            .ok_or(AuthProductError::BackendConflict)?;
        Ok(DcrRegistrationState {
            flow: flow.clone(),
            action,
        })
    }

    pub fn dcr_original_state(
        &self,
        flow: &AuthFlowRecord,
        original: &DcrRegistrationOriginal,
    ) -> Result<Option<DcrRegistrationState>, AuthProductError> {
        original.verify_flow(flow)?;
        let Some(action) = &self.dcr_registration else {
            return Ok(None);
        };
        if self.dcr_legacy.is_some()
            || self.oauth_setup.is_some()
            || self.requirement_setup != original.requirement_setup
        {
            return Err(AuthProductError::BackendConflict);
        }
        action.original.verify(original)?;
        if flow.update_binding.is_some() && self.update_account_snapshot.is_none() {
            let terminal_preparation =
                matches!(
                    (&flow.status, &action.cleanup),
                    (
                        AuthFlowStatus::Canceled,
                        crate::DcrCleanupState::NotRegistered {
                            reason: crate::DcrCleanupReason::Canceled,
                            ..
                        }
                    ) | (
                        AuthFlowStatus::Expired,
                        crate::DcrCleanupState::NotRegistered {
                            reason: crate::DcrCleanupReason::Expired,
                            ..
                        }
                    )
                ) && matches!(action.outcome, DcrRegistrationOutcome::Prepared)
                    && action.publication == DcrChallengePublication::Suppressed
                    && flow.challenge.is_none()
                    && flow.credential_account_id.is_none()
                    && flow.authorization_code_hash.is_none()
                    && flow.continuation_emitted_at.is_none()
                    && self.oauth_callback.is_none()
                    && self.requirement_selection.is_none()
                    && self.requirement_source.is_none();
            if !terminal_preparation {
                return Err(AuthProductError::BackendConflict);
            }
        }
        if let Some(retirement) = &action.retirement {
            if !action.cleanup.remote_confirmed() {
                return Err(AuthProductError::BackendConflict);
            }
            retirement.validate(original)?;
        }
        Ok(Some(DcrRegistrationState {
            flow: flow.clone(),
            action: action.clone(),
        }))
    }

    pub fn admit_dcr_registration(
        &mut self,
        flow: &mut AuthFlowRecord,
        original: &DcrRegistrationOriginal,
        now: Timestamp,
    ) -> Result<DcrRegistrationAdmission, AuthProductError> {
        let state = self
            .dcr_original_state(flow, original)?
            .ok_or(AuthProductError::BackendConflict)?;
        if !matches!(state.action.outcome, DcrRegistrationOutcome::Prepared) {
            return Ok(DcrRegistrationAdmission::Original(state));
        }
        if crate::is_terminal_status(flow.status) || flow.expires_at <= now {
            return Err(if flow.status == AuthFlowStatus::Canceled {
                AuthProductError::Canceled
            } else {
                AuthProductError::UnknownOrExpiredFlow
            });
        }
        if flow.status != AuthFlowStatus::Pending
            || flow.challenge.is_some()
            || self.oauth_callback.is_some()
            || state.action.publication != DcrChallengePublication::Pending
        {
            return Err(AuthProductError::BackendConflict);
        }
        let mut action = state.action;
        action.outcome = DcrRegistrationOutcome::Submitted { submitted_at: now };
        action.updated_at = now;
        flow.updated_at = now;
        self.dcr_registration = Some(action.clone());
        Ok(DcrRegistrationAdmission::Dispatch(DcrRegistrationState {
            flow: flow.clone(),
            action,
        }))
    }

    /// Called only after the concrete owner authenticates the retained PKCE and
    /// client generations against their original contexts and exact response.
    pub fn publish_dcr_challenge(
        &mut self,
        flow: &mut AuthFlowRecord,
        original: &DcrRegistrationOriginal,
        challenge: DcrRegistrationChallenge,
        now: Timestamp,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let state = self
            .dcr_original_state(flow, original)?
            .ok_or(AuthProductError::BackendConflict)?;
        match &state.action.publication {
            DcrChallengePublication::Ready(saved) => {
                if saved != &challenge {
                    return Err(AuthProductError::BackendConflict);
                }
                return Ok(state);
            }
            DcrChallengePublication::Suppressed => return Ok(state),
            DcrChallengePublication::Pending => {}
        }
        if !matches!(
            state.action.outcome,
            DcrRegistrationOutcome::Returned {
                status: 200..=299,
                ..
            }
        ) {
            return Err(AuthProductError::BackendConflict);
        }
        challenge.pkce.validate()?;
        challenge.client.validate()?;
        if challenge.pkce.handle == challenge.client.handle
            || challenge.pkce.handle == original.preparation.handle
            || challenge.client.handle == original.preparation.handle
        {
            return Err(AuthProductError::BackendConflict);
        }
        let mut action = state.action;
        if crate::is_terminal_status(flow.status) || flow.expires_at <= now {
            action.publication = DcrChallengePublication::Suppressed;
            if !crate::is_terminal_status(flow.status) {
                flow.status = AuthFlowStatus::Expired;
                flow.error = Some(AuthErrorCode::UnknownOrExpiredFlow);
                flow.updated_at = now;
            }
        } else {
            if flow.status != AuthFlowStatus::Pending
                || flow.challenge.is_some()
                || self.oauth_callback.is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            flow.challenge = Some(AuthChallenge::OAuthUrl {
                authorization_url: challenge.authorization_url.clone(),
                expires_at: flow.expires_at,
            });
            flow.status = AuthFlowStatus::AwaitingUser;
            flow.updated_at = now;
            action.publication = DcrChallengePublication::Ready(challenge);
        }
        action.updated_at = now;
        self.dcr_registration = Some(action.clone());
        Ok(DcrRegistrationState {
            flow: flow.clone(),
            action,
        })
    }
}
