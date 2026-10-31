//! DCR cleanup intent/results beside the original AuthFlow. No raw management
//! URL/token and no additional registry or autonomous task workflow live here.
use crate::{
    AuthErrorCode, AuthFlowCustody, AuthFlowRecord, AuthFlowStatus, AuthProductError,
    DcrMaterialReference, DcrRegistrationOriginal, DcrRegistrationOutcome, DcrRegistrationState,
    Timestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcrCleanupReason {
    Canceled,
    Expired,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcrCleanupUnavailable {
    RegistrationUnconfirmed,
    RegistrationResponseUnusable,
    ManagementUnavailable,
    InvalidManagementEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrCleanupClaim {
    pub original: DcrRegistrationOriginal,
    pub registration_response: DcrMaterialReference,
    pub management: DcrMaterialReference,
    pub reason: DcrCleanupReason,
    pub requested_at: Timestamp,
    pub submitted_at: Timestamp,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DcrCleanupState {
    #[default]
    Retained,
    Required {
        reason: DcrCleanupReason,
        requested_at: Timestamp,
    },
    /// A retained original which never obtained a registration dispatch claim.
    /// This is the only no-remote-effect path; timeout/absence cannot prove it.
    NotRegistered {
        reason: DcrCleanupReason,
        requested_at: Timestamp,
    },
    Unavailable {
        reason: DcrCleanupReason,
        requested_at: Timestamp,
        issue: DcrCleanupUnavailable,
    },
    Submitted {
        claim: DcrCleanupClaim,
    },
    Returned {
        claim: DcrCleanupClaim,
        status: u16,
        empty_body: bool,
        response: DcrMaterialReference,
    },
    Unknown {
        claim: DcrCleanupClaim,
        error: AuthErrorCode,
    },
}
impl DcrCleanupState {
    /// Confirms the remote obligation only. Local artifact closure/retirement
    /// requires its own authenticated receipt and is not implied here.
    pub fn remote_confirmed(&self) -> bool {
        matches!(
            self,
            Self::NotRegistered { .. }
                | Self::Returned {
                    status: 204,
                    empty_body: true,
                    ..
                }
        )
    }
    pub fn claim(&self) -> Option<&DcrCleanupClaim> {
        match self {
            Self::Submitted { claim }
            | Self::Returned { claim, .. }
            | Self::Unknown { claim, .. } => Some(claim),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DcrCleanupPrepared {
    pub original: DcrRegistrationOriginal,
    pub registration_response: DcrMaterialReference,
    pub management: DcrMaterialReference,
}
#[derive(Debug, Clone)]
pub enum DcrCleanupAdmission {
    Dispatch(DcrRegistrationState),
    Original(DcrRegistrationState),
}
#[derive(Debug, Clone)]
pub enum DcrCleanupOutcome {
    Returned {
        status: u16,
        empty_body: bool,
        response: DcrMaterialReference,
    },
    Unknown(AuthErrorCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcrRemoteCleanupStatus {
    Retained,
    Pending,
    NotRegistered,
    Unavailable,
    Unconfirmed,
    Confirmed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcrLocalCleanupStatus {
    Retained,
    Pending,
    Unresolved,
    Confirmed,
}
/// Operator projection: no material, URLs, handles, backend text or opaque refs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcrCleanupProjection {
    pub remote: DcrRemoteCleanupStatus,
    pub local: DcrLocalCleanupStatus,
    pub issue: Option<DcrCleanupUnavailable>,
    pub http_status: Option<u16>,
    pub error: Option<AuthErrorCode>,
    pub can_reconcile: bool,
}
impl DcrRegistrationState {
    pub fn cleanup_projection(&self) -> DcrCleanupProjection {
        let (remote, issue, http_status, error) = match &self.action.cleanup {
            DcrCleanupState::Retained => (DcrRemoteCleanupStatus::Retained, None, None, None),
            DcrCleanupState::Required { .. } | DcrCleanupState::Submitted { .. } => {
                (DcrRemoteCleanupStatus::Pending, None, None, None)
            }
            DcrCleanupState::NotRegistered { .. } => {
                (DcrRemoteCleanupStatus::NotRegistered, None, None, None)
            }
            DcrCleanupState::Unavailable { issue, .. } => (
                DcrRemoteCleanupStatus::Unavailable,
                Some(*issue),
                None,
                None,
            ),
            DcrCleanupState::Returned {
                status, empty_body, ..
            } => (
                if *status == 204 && *empty_body {
                    DcrRemoteCleanupStatus::Confirmed
                } else {
                    DcrRemoteCleanupStatus::Unconfirmed
                },
                None,
                Some(*status),
                None,
            ),
            DcrCleanupState::Unknown { error, .. } => (
                DcrRemoteCleanupStatus::Unconfirmed,
                None,
                None,
                Some(*error),
            ),
        };
        let local = match &self.action.retirement {
            Some(plan) if plan.confirmed() => DcrLocalCleanupStatus::Confirmed,
            Some(plan)
                if plan.entries.iter().any(|entry| {
                    matches!(
                        entry.outcome,
                        crate::DcrRetirementOutcome::Unresolved { .. }
                    )
                }) =>
            {
                DcrLocalCleanupStatus::Unresolved
            }
            Some(_) => DcrLocalCleanupStatus::Pending,
            None => DcrLocalCleanupStatus::Retained,
        };
        DcrCleanupProjection {
            remote,
            local,
            issue,
            http_status,
            error,
            can_reconcile: matches!(
                self.flow.status,
                AuthFlowStatus::Canceled | AuthFlowStatus::Failed | AuthFlowStatus::Expired
            ) && local != DcrLocalCleanupStatus::Confirmed,
        }
    }
}
#[derive(Debug, Clone)]
pub struct DcrCleanupCompletion {
    pub dispatched: DcrCleanupClaim,
    pub outcome: DcrCleanupOutcome,
}

impl crate::DcrRegistrationAction {
    /// Exact safe context for the encrypted URI/token selected from the original
    /// registration reply. The concrete owner authenticates this before claim.
    pub fn cleanup_management_context(&self) -> Result<serde_json::Value, AuthProductError> {
        let DcrRegistrationOutcome::Returned {
            submitted_at,
            status: 200..=299,
            response,
        } = &self.outcome
        else {
            return Err(AuthProductError::BackendConflict);
        };
        Ok(
            serde_json::json!({"format":"dcr-cleanup-management/1","original":self.original,
            "registration_submitted_at":submitted_at,"registration_response":response}),
        )
    }
    pub fn cleanup_response_context(
        &self,
        status: u16,
        empty_body: bool,
    ) -> Result<serde_json::Value, AuthProductError> {
        let claim = self
            .cleanup
            .claim()
            .ok_or(AuthProductError::BackendConflict)?;
        Ok(
            serde_json::json!({"format":"dcr-cleanup-response/1","claim":claim,
            "status":status,"empty_body":empty_body}),
        )
    }
}

impl AuthFlowCustody {
    /// Persist this alongside cancellation/expiry, never after acknowledging
    /// them. Filesystem owners must first settle any account publication decision.
    pub fn require_dcr_cleanup(
        &mut self,
        flow: &mut AuthFlowRecord,
        now: Timestamp,
    ) -> Result<(), AuthProductError> {
        let Some(action) = &self.dcr_registration else {
            return Ok(());
        };
        action.original.verify_flow(flow)?;
        // A completed account can still use this registration for refresh. Its
        // actual account/lifecycle owner decides when those references end.
        if flow.status == AuthFlowStatus::Completed {
            return Ok(());
        }
        if flow.credential_account_id.is_some()
            || self
                .oauth_callback
                .as_ref()
                .is_some_and(|a| a.published_account.is_some())
        {
            return Err(AuthProductError::BackendConflict);
        }
        let reason = match flow.status {
            AuthFlowStatus::Canceled => DcrCleanupReason::Canceled,
            AuthFlowStatus::Expired => DcrCleanupReason::Expired,
            AuthFlowStatus::Failed => DcrCleanupReason::Failed,
            _ if flow.expires_at <= now => {
                flow.status = AuthFlowStatus::Expired;
                flow.error = Some(AuthErrorCode::UnknownOrExpiredFlow);
                flow.updated_at = now;
                DcrCleanupReason::Expired
            }
            _ => return Err(AuthProductError::BackendConflict),
        };
        let action = self
            .dcr_registration
            .as_mut()
            .ok_or(AuthProductError::BackendConflict)?;
        if action.cleanup == DcrCleanupState::Retained {
            action.cleanup = DcrCleanupState::Required {
                reason,
                requested_at: now,
            };
            action.updated_at = now;
        }
        Ok(())
    }

    pub fn mark_dcr_cleanup_unavailable(
        &mut self,
        flow: &mut AuthFlowRecord,
        original: &DcrRegistrationOriginal,
        issue: DcrCleanupUnavailable,
        now: Timestamp,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        self.dcr_original_state(flow, original)?
            .ok_or(AuthProductError::BackendConflict)?;
        self.require_dcr_cleanup(flow, now)?;
        let state = self
            .dcr_original_state(flow, original)?
            .ok_or(AuthProductError::BackendConflict)?;
        let (reason, requested_at) = match state.action.cleanup {
            DcrCleanupState::Required {
                reason,
                requested_at,
            }
            | DcrCleanupState::Unavailable {
                reason,
                requested_at,
                ..
            } => (reason, requested_at),
            _ => return Ok(state),
        };
        let mut action = state.action;
        action.cleanup = if matches!(action.outcome, DcrRegistrationOutcome::Prepared) {
            DcrCleanupState::NotRegistered {
                reason,
                requested_at,
            }
        } else {
            DcrCleanupState::Unavailable {
                reason,
                requested_at,
                issue,
            }
        };
        action.updated_at = now;
        self.dcr_registration = Some(action.clone());
        Ok(DcrRegistrationState {
            flow: flow.clone(),
            action,
        })
    }

    /// Only a newly ACKNOWLEDGED concrete-store CAS may expose Dispatch.
    /// Authenticated encrypted management provenance must be checked by that owner.
    pub fn admit_dcr_cleanup(
        &mut self,
        flow: &mut AuthFlowRecord,
        prepared: DcrCleanupPrepared,
        now: Timestamp,
    ) -> Result<DcrCleanupAdmission, AuthProductError> {
        self.dcr_original_state(flow, &prepared.original)?
            .ok_or(AuthProductError::BackendConflict)?;
        self.require_dcr_cleanup(flow, now)?;
        let state = self
            .dcr_original_state(flow, &prepared.original)?
            .ok_or(AuthProductError::BackendConflict)?;
        let (reason, requested_at) = match state.action.cleanup {
            DcrCleanupState::Required {
                reason,
                requested_at,
            }
            | DcrCleanupState::Unavailable {
                reason,
                requested_at,
                ..
            } => (reason, requested_at),
            _ => return Ok(DcrCleanupAdmission::Original(state)),
        };
        let DcrRegistrationOutcome::Returned {
            status: 200..=299,
            response,
            ..
        } = &state.action.outcome
        else {
            return Err(AuthProductError::BackendConflict);
        };
        prepared.management.validate()?;
        if *response != prepared.registration_response
            || prepared.management.handle == response.handle
            || prepared.management.handle == prepared.original.preparation.handle
        {
            return Err(AuthProductError::BackendConflict);
        }
        let claim = DcrCleanupClaim {
            original: prepared.original,
            registration_response: prepared.registration_response,
            management: prepared.management,
            reason,
            requested_at,
            submitted_at: now,
        };
        let mut action = state.action;
        action.cleanup = DcrCleanupState::Submitted { claim };
        action.updated_at = now;
        self.dcr_registration = Some(action.clone());
        Ok(DcrCleanupAdmission::Dispatch(DcrRegistrationState {
            flow: flow.clone(),
            action,
        }))
    }

    /// Save the exact response independently of retirement. RFC7592 §2.3 uses
    /// an empty HTTP204 acknowledgement; JSON decoding is not a deletion proof.
    pub fn record_dcr_cleanup(
        &mut self,
        flow: &AuthFlowRecord,
        completion: DcrCleanupCompletion,
        now: Timestamp,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let state = self
            .dcr_original_state(flow, &completion.dispatched.original)?
            .ok_or(AuthProductError::BackendConflict)?;
        if state.action.cleanup.claim() != Some(&completion.dispatched) {
            return Err(AuthProductError::BackendConflict);
        }
        let result = match completion.outcome {
            DcrCleanupOutcome::Returned {
                status,
                empty_body,
                response,
            } => {
                response.validate()?;
                if !(200..600).contains(&status)
                    || response.handle == completion.dispatched.management.handle
                    || response.handle == completion.dispatched.registration_response.handle
                {
                    return Err(AuthProductError::BackendConflict);
                }
                DcrCleanupState::Returned {
                    claim: completion.dispatched,
                    status,
                    empty_body,
                    response,
                }
            }
            DcrCleanupOutcome::Unknown(error) => DcrCleanupState::Unknown {
                claim: completion.dispatched,
                error,
            },
        };
        if state.action.cleanup == result {
            return Ok(state);
        }
        if !matches!(state.action.cleanup, DcrCleanupState::Submitted { .. }) {
            return Err(AuthProductError::BackendConflict);
        }
        let mut action = state.action;
        action.cleanup = result;
        action.updated_at = now;
        self.dcr_registration = Some(action.clone());
        Ok(DcrRegistrationState {
            flow: flow.clone(),
            action,
        })
    }
}
