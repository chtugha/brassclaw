//! Original DCR state in the existing auth flow record and actual PostgreSQL CAS.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::{
    DcrRegistrationAdmission, DcrRegistrationChallenge, DcrRegistrationCompletion,
    DcrRegistrationOriginal, DcrRegistrationState,
};

fn selected(document: &StoredAuthFlow) -> Result<DcrRegistrationState, AuthProductError> {
    let action = document
        .auth_custody
        .dcr_registration
        .as_ref()
        .ok_or(AuthProductError::BackendConflict)?;
    document
        .auth_custody
        .dcr_original_state(&document.flow, &action.original)?
        .ok_or(AuthProductError::BackendConflict)
}

impl PgAuthProductServices {
    pub(super) async fn read_dcr_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<DcrRegistrationState>, AuthProductError> {
        let Some((document, _)) = self.read_flow_document(scope, id).await? else {
            return Ok(None);
        };
        if document.auth_custody.dcr_registration.is_none() {
            return Ok(None);
        }
        selected(&document).map(Some)
    }

    pub(super) async fn prepare_dcr_original(
        &self,
        original: DcrRegistrationOriginal,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        if let Some(existing) = self
            .read_dcr_original(&original.scope, original.flow_id)
            .await?
        {
            existing.action.original.verify(&original)?;
            return Ok(existing);
        }
        if original.expires_at <= Utc::now() {
            return self
                .retire_dcr_preparation_owned(original, brassclaw_auth::DcrCleanupReason::Expired)
                .await;
        }
        crate::auth_dcr_custody::unsubmitted_preparation(self.secret_store.as_ref(), &original)
            .await?;
        let snapshot = if let Some(binding) = &original.update_binding {
            Some(
                self.read_account(&original.scope, binding.account_id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?
                    .0,
            )
        } else {
            None
        };
        let flow = match original.pending_flow(Utc::now()) {
            Ok(flow) => flow,
            Err(AuthProductError::UnknownOrExpiredFlow) => {
                return self
                    .retire_dcr_preparation_owned(
                        original,
                        brassclaw_auth::DcrCleanupReason::Expired,
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        let mut document = StoredAuthFlow::new(flow, snapshot)?;
        document
            .auth_custody
            .attach_dcr_original(&document.flow, original.clone())?;
        if let Err(error) = self.write_flow_document(&document, None).await {
            let after = self
                .read_dcr_original(&original.scope, original.flow_id)
                .await?;
            return match after {
                Some(after) => {
                    after.action.original.verify(&original)?;
                    Ok(after)
                }
                None => Err(error),
            };
        }
        selected(&document)
    }

    pub(super) async fn retire_dcr_preparation_owned(
        &self,
        original: DcrRegistrationOriginal,
        reason: brassclaw_auth::DcrCleanupReason,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        if let Some(existing) = self
            .read_dcr_original(&original.scope, original.flow_id)
            .await?
        {
            existing.action.original.verify(&original)?;
            return Ok(existing);
        }
        crate::auth_dcr_custody::unsubmitted_preparation(self.secret_store.as_ref(), &original)
            .await?;
        let (flow, auth_custody) = brassclaw_auth::AuthFlowCustody::retired_dcr_preparation(
            original.clone(),
            reason,
            Utc::now(),
        )?;
        let document = StoredAuthFlow {
            flow,
            auth_custody,
            fs_callback_publication: None,
        };
        if let Err(error) = self.write_flow_document(&document, None).await {
            return match self
                .read_dcr_original(&original.scope, original.flow_id)
                .await?
            {
                Some(after) => {
                    after.action.original.verify(&original)?;
                    Ok(after)
                }
                None => Err(error),
            };
        }
        selected(&document)
    }

    pub(super) async fn claim_dcr_original(
        &self,
        original: &DcrRegistrationOriginal,
    ) -> Result<DcrRegistrationAdmission, AuthProductError> {
        let (mut document, version) = self
            .read_flow_document(&original.scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let admission = document.auth_custody.admit_dcr_registration(
            &mut document.flow,
            original,
            Utc::now(),
        )?;
        if matches!(admission, DcrRegistrationAdmission::Original(_)) {
            return Ok(admission);
        }
        crate::auth_dcr_custody::preparation(self.secret_store.as_ref(), original).await?;
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            // A committed write with a lost acknowledgement MUST NOT regain
            // dispatch authority from readback, even when it is our own claim.
            return match self
                .read_dcr_original(&original.scope, original.flow_id)
                .await?
            {
                Some(after) => {
                    after.action.original.verify(original)?;
                    Ok(DcrRegistrationAdmission::Original(after))
                }
                None => Err(error),
            };
        }
        Ok(admission)
    }

    pub(super) async fn record_dcr_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: DcrRegistrationCompletion,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        if *scope != completion.dispatched.original.scope {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let original = completion.dispatched.original.clone();
        let (mut document, version) = self
            .read_flow_document(scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = selected(&document)?;
        crate::auth_dcr_custody::response(
            self.secret_store.as_ref(),
            &completion.dispatched,
            &completion.outcome,
        )
        .await?;
        let after =
            document
                .auth_custody
                .record_dcr_result(&mut document.flow, completion, Utc::now())?;
        if before.action == after.action {
            return Ok(after);
        }
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            let actual = self
                .read_dcr_original(scope, original.flow_id)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            actual.action.original.verify(&original)?;
            if actual.action.outcome == after.action.outcome {
                return Ok(actual);
            }
            return Err(error);
        }
        Ok(after)
    }

    pub(super) async fn publish_dcr_original(
        &self,
        original: &DcrRegistrationOriginal,
        challenge: DcrRegistrationChallenge,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let (mut document, version) = self
            .read_flow_document(&original.scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = selected(&document)?;
        // Reading an already committed original is independent of a later
        // input retirement. Only a new publication needs live output proofs.
        if before.action.publication == brassclaw_auth::DcrChallengePublication::Pending {
            crate::auth_dcr_custody::outputs(
                self.secret_store.as_ref(),
                &before.action,
                &challenge,
            )
            .await?;
        }
        let after = document.auth_custody.publish_dcr_challenge(
            &mut document.flow,
            original,
            challenge,
            Utc::now(),
        )?;
        if before == after {
            return Ok(after);
        }
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            let actual = self
                .read_dcr_original(&original.scope, original.flow_id)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            actual.action.original.verify(original)?;
            if actual.action.publication == after.action.publication {
                return Ok(actual);
            }
            return Err(error);
        }
        Ok(after)
    }
    pub(super) async fn claim_dcr_cleanup_owned(
        &self,
        prepared: brassclaw_auth::DcrCleanupPrepared,
    ) -> Result<brassclaw_auth::DcrCleanupAdmission, AuthProductError> {
        let original = prepared.original.clone();
        let (mut document, version) = self
            .read_flow_document(&original.scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = selected(&document)?;

        let admitted = document.auth_custody.admit_dcr_cleanup(
            &mut document.flow,
            prepared.clone(),
            Utc::now(),
        )?;
        if matches!(admitted, brassclaw_auth::DcrCleanupAdmission::Original(_)) {
            return Ok(admitted);
        }
        crate::auth_dcr_custody::cleanup_management(
            self.secret_store.as_ref(),
            &before.action,
            &prepared,
        )
        .await?;
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            return match self
                .read_dcr_original(&original.scope, original.flow_id)
                .await?
            {
                Some(after) => {
                    after.action.original.verify(&original)?;
                    Ok(brassclaw_auth::DcrCleanupAdmission::Original(after))
                }
                None => Err(error),
            };
        }
        Ok(admitted)
    }

    pub(super) async fn record_dcr_cleanup_owned(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: brassclaw_auth::DcrCleanupCompletion,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let original = completion.dispatched.original.clone();
        if *scope != original.scope {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let (mut document, version) = self
            .read_flow_document(scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = selected(&document)?;
        crate::auth_dcr_custody::cleanup_response(
            self.secret_store.as_ref(),
            &before.action,
            &completion,
        )
        .await?;
        let after =
            document
                .auth_custody
                .record_dcr_cleanup(&document.flow, completion, Utc::now())?;
        if before == after {
            return Ok(after);
        }
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            let actual = self
                .read_dcr_original(scope, original.flow_id)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            actual.action.original.verify(&original)?;
            if actual.action.cleanup == after.action.cleanup {
                return Ok(actual);
            }
            return Err(error);
        }
        Ok(after)
    }

    pub(super) async fn mark_dcr_cleanup_unavailable_owned(
        &self,
        original: &DcrRegistrationOriginal,
        issue: brassclaw_auth::DcrCleanupUnavailable,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let (mut document, version) = self
            .read_flow_document(&original.scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = selected(&document)?;

        let after = document.auth_custody.mark_dcr_cleanup_unavailable(
            &mut document.flow,
            original,
            issue,
            Utc::now(),
        )?;
        if before == after {
            return Ok(after);
        }
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            let actual = self
                .read_dcr_original(&original.scope, original.flow_id)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            actual.action.original.verify(original)?;
            if actual.action.cleanup == after.action.cleanup {
                return Ok(actual);
            }
            return Err(error);
        }
        Ok(after)
    }
    pub(super) async fn update_dcr_retirement_owned(
        &self,
        original: &DcrRegistrationOriginal,
        update: brassclaw_auth::DcrRetirementUpdate,
    ) -> Result<DcrRegistrationState, AuthProductError> {
        let (mut document, version) = self
            .read_flow_document(&original.scope, original.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = selected(&document)?;

        crate::auth_dcr_retirement::validate_update(
            self.secret_store.as_ref(),
            &before.action,
            &update,
        )
        .await?;
        let after = document.auth_custody.update_dcr_retirement(
            &document.flow,
            original,
            update.clone(),
            Utc::now(),
        )?;
        if before == after {
            return Ok(after);
        }
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            let actual = self
                .read_dcr_original(&original.scope, original.flow_id)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            actual.action.original.verify(original)?;
            if update.satisfied_by(&actual.action) {
                return Ok(actual);
            }
            return Err(error);
        }
        Ok(after)
    }
    pub(super) async fn retain_legacy_dcr_owned(
        &self,
        selection: brassclaw_auth::DcrLegacySelection,
    ) -> Result<brassclaw_auth::DcrLegacyState, AuthProductError> {
        let scope = &selection.original.scope;
        let id = selection.original.flow_id;
        let (mut document, version) = self
            .read_flow_document(scope, id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = document.auth_custody.dcr_legacy.clone();
        let state = document
            .auth_custody
            .retain_legacy_dcr(&document.flow, selection.clone())?;
        if before.as_ref() == Some(&selection) {
            return Ok(state);
        }
        crate::auth_dcr_custody::legacy_selection(self.secret_store.as_ref(), &selection).await?;
        if let Err(error) = self.write_flow_document(&document, Some(version)).await {
            let (actual, _) = self
                .read_flow_document(scope, id)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            selection.original.verify_flow(&actual.flow)?;
            if actual.auth_custody.dcr_legacy.as_ref() == Some(&selection) {
                return Ok(brassclaw_auth::DcrLegacyState {
                    flow: actual.flow,
                    selection,
                });
            }
            return Err(error);
        }
        Ok(state)
    }
}
