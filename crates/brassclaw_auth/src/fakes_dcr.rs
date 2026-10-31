//! In-memory model of the existing flow owner's DCR transitions.
use super::*;
use crate::{
    DcrRegistrationAdmission, DcrRegistrationChallenge, DcrRegistrationCompletion,
    DcrRegistrationOriginal, DcrRegistrationState,
};

fn selected(
    state: &AuthState,
    scope: &crate::AuthProductScope,
    id: AuthFlowId,
) -> Result<Option<DcrRegistrationState>, AuthProductError> {
    let Some(flow) = state.flows.get(&id) else {
        return Ok(None);
    };
    if flow.scope != *scope {
        return Err(AuthProductError::CrossScopeDenied);
    }
    let custody = state
        .flow_custody
        .get(&id)
        .ok_or(AuthProductError::BackendConflict)?;
    let Some(action) = custody.dcr_registration.as_ref() else {
        return Ok(None);
    };
    custody.dcr_original_state(flow, &action.original)
}

fn retire_unsubmitted(
    state: &mut AuthState,
    original: DcrRegistrationOriginal,
    reason: crate::DcrCleanupReason,
) -> Result<DcrRegistrationState, AuthProductError> {
    if let Some(existing) = selected(state, &original.scope, original.flow_id)? {
        existing.action.original.verify(&original)?;
        return Ok(existing);
    }
    if state.flows.contains_key(&original.flow_id) {
        return Err(AuthProductError::BackendConflict);
    }
    // This model does not authenticate SecretStore provenance; the concrete
    // native and record-filesystem owners supply that separate acceptance.
    let (flow, custody) =
        crate::AuthFlowCustody::retired_dcr_preparation(original, reason, Utc::now())?;
    let action = custody
        .dcr_registration
        .clone()
        .ok_or(AuthProductError::BackendConflict)?;
    let result = DcrRegistrationState {
        flow: flow.clone(),
        action,
    };
    state.flow_custody.insert(flow.id, custody);
    state.flows.insert(flow.id, flow);
    Ok(result)
}

impl InMemoryAuthProductServices {
    pub(super) async fn prepare_dcr_original(
        &self,
        original: DcrRegistrationOriginal,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let mut state = self.lock_state();
        if original.expires_at <= Utc::now() {
            return retire_unsubmitted(&mut state, original, crate::DcrCleanupReason::Expired);
        }
        if let Some(existing) = selected(&state, &original.scope, original.flow_id)? {
            existing.action.original.verify(&original)?;
            return Ok(existing);
        }
        if state.flows.contains_key(&original.flow_id) {
            return Err(AuthProductError::BackendConflict);
        }
        let snapshot = original
            .update_binding
            .as_ref()
            .map(|binding| {
                let account = state
                    .accounts
                    .get(&binding.account_id)
                    .ok_or(AuthProductError::CredentialMissing)?;
                if account.scope != original.scope || account.provider != original.provider {
                    return Err(AuthProductError::CrossScopeDenied);
                }
                crate::domain::validate_bound_update_authority(account, binding)?;
                Ok(account.clone())
            })
            .transpose()?;
        let flow = match original.pending_flow(Utc::now()) {
            Ok(flow) => flow,
            Err(AuthProductError::UnknownOrExpiredFlow) => {
                return retire_unsubmitted(&mut state, original, crate::DcrCleanupReason::Expired);
            }
            Err(error) => return Err(error),
        };
        let mut custody = crate::AuthFlowCustody {
            update_account_snapshot: snapshot,
            oauth_callback: None,
            oauth_setup: None,
            dcr_registration: None,
            dcr_legacy: None,
            requirement_setup: None,
            requirement_selection: None,
            requirement_source: None,
            device_login: None,
        };
        let prepared = custody.attach_dcr_original(&flow, original)?;
        state.flow_custody.insert(flow.id, custody);
        state.flows.insert(flow.id, flow);
        Ok(prepared)
    }

    pub(super) async fn retire_dcr_preparation_owned(
        &self,
        original: DcrRegistrationOriginal,
        reason: crate::DcrCleanupReason,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        retire_unsubmitted(&mut self.lock_state(), original, reason)
    }

    pub(super) async fn read_dcr_original(
        &self,
        scope: &crate::AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<DcrRegistrationState>, AuthProductError> {
        selected(&self.lock_state(), scope, id)
    }

    pub(super) async fn claim_dcr_original(
        &self,
        original: &DcrRegistrationOriginal,
    ) -> Result<DcrRegistrationAdmission, AuthProductError> {
        let mut state = self.lock_state();
        let before = selected(&state, &original.scope, original.flow_id)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut flow = before.flow;
        let mut custody = state
            .flow_custody
            .get(&flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        let admitted = custody.admit_dcr_registration(&mut flow, original, Utc::now())?;
        state.flow_custody.insert(flow.id, custody);
        state.flows.insert(flow.id, flow);
        Ok(admitted)
    }

    pub(super) async fn record_dcr_original(
        &self,
        scope: &crate::AuthProductScope,
        completion: DcrRegistrationCompletion,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let mut state = self.lock_state();
        let before = selected(&state, scope, completion.dispatched.original.flow_id)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut custody = state
            .flow_custody
            .get(&before.flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        let mut flow = before.flow;
        let after = custody.record_dcr_result(&mut flow, completion, Utc::now())?;
        state.flow_custody.insert(after.flow.id, custody);
        state.flows.insert(flow.id, flow);
        Ok(after)
    }

    pub(super) async fn publish_dcr_original(
        &self,
        original: &DcrRegistrationOriginal,
        challenge: DcrRegistrationChallenge,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let mut state = self.lock_state();
        let before = selected(&state, &original.scope, original.flow_id)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut flow = before.flow;
        let mut custody = state
            .flow_custody
            .get(&flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        // The fake models transition contracts only; it is never production
        // evidence of SecretStore custody authentication.
        let after = custody.publish_dcr_challenge(&mut flow, original, challenge, Utc::now())?;
        state.flow_custody.insert(flow.id, custody);
        state.flows.insert(flow.id, flow);
        Ok(after)
    }
    pub(super) async fn claim_dcr_cleanup_owned(
        &self,
        prepared: crate::DcrCleanupPrepared,
    ) -> Result<crate::DcrCleanupAdmission, AuthProductError> {
        let mut state = self.lock_state();
        let original = prepared.original.clone();
        let before = selected(&state, &original.scope, original.flow_id)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut flow = before.flow;
        let mut custody = state
            .flow_custody
            .get(&flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        let admitted = custody.admit_dcr_cleanup(&mut flow, prepared, Utc::now())?;
        state.flow_custody.insert(flow.id, custody);
        state.flows.insert(flow.id, flow);
        Ok(admitted)
    }
    pub(super) async fn record_dcr_cleanup_owned(
        &self,
        scope: &crate::AuthProductScope,
        completion: crate::DcrCleanupCompletion,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let mut state = self.lock_state();
        let original = completion.dispatched.original.clone();
        if *scope != original.scope {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let before = selected(&state, scope, original.flow_id)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut custody = state
            .flow_custody
            .get(&before.flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        let after = custody.record_dcr_cleanup(&before.flow, completion, Utc::now())?;
        state.flow_custody.insert(after.flow.id, custody);
        Ok(after)
    }
    pub(super) async fn mark_dcr_cleanup_unavailable_owned(
        &self,
        original: &DcrRegistrationOriginal,
        issue: crate::DcrCleanupUnavailable,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let mut state = self.lock_state();
        let before = selected(&state, &original.scope, original.flow_id)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut flow = before.flow;
        let mut custody = state
            .flow_custody
            .get(&flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        let after = custody.mark_dcr_cleanup_unavailable(&mut flow, original, issue, Utc::now())?;
        state.flow_custody.insert(flow.id, custody);
        state.flows.insert(flow.id, flow);
        Ok(after)
    }
    pub(super) async fn update_dcr_retirement_owned(
        &self,
        original: &DcrRegistrationOriginal,
        update: crate::DcrRetirementUpdate,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let mut state = self.lock_state();
        let before = selected(&state, &original.scope, original.flow_id)?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut custody = state
            .flow_custody
            .get(&before.flow.id)
            .cloned()
            .ok_or(AuthProductError::BackendConflict)?;
        let after = custody.update_dcr_retirement(&before.flow, original, update, Utc::now())?;
        state.flow_custody.insert(after.flow.id, custody);
        Ok(after)
    }
    pub(super) async fn retain_legacy_dcr_owned(
        &self,
        selection: crate::DcrLegacySelection,
    ) -> Result<crate::DcrLegacyState, AuthProductError> {
        let mut state = self.lock_state();
        let flow = state
            .flows
            .get(&selection.original.flow_id)
            .cloned()
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let mut custody = state
            .flow_custody
            .get(&flow.id)
            .cloned()
            .unwrap_or_default();
        let selected = custody.retain_legacy_dcr(&flow, selection)?;
        state.flow_custody.insert(flow.id, custody);
        Ok(selected)
    }
}
