use async_trait::async_trait;
use brassclaw_filesystem::RootFilesystem;
use chrono::Utc;

use super::domain::validate_flow_update_binding;
use super::{FilesystemAuthProductServices, is_terminal_status, scope_matches};
use brassclaw_auth::{
    AuthChallenge, AuthErrorCode, AuthFlowId, AuthFlowManager, AuthFlowRecord,
    AuthFlowRecordSource, AuthFlowStatus, AuthProductError, CredentialSelectionInput,
    ManualTokenCompletionInput, NewAuthFlow, OAuthCallbackClaimRequest, OAuthCallbackFailureInput,
    OAuthCallbackInput, TurnGateAuthFlowQuery, flow_matches_turn_gate_query,
};

#[async_trait]
impl<F> AuthFlowManager for FilesystemAuthProductServices<F>
where
    F: RootFilesystem + 'static,
{
    async fn requirement_setup(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow: AuthFlowId,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSetup>, AuthProductError> {
        let (original, _) = self
            .read_flow_document(scope, flow)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let setup = original.auth_custody.requirement_setup.map(|setup| *setup);
        if let Some(setup) = &setup {
            setup
                .verify_flow(&original.flow)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
        }
        Ok(setup)
    }

    async fn claim_dcr_cleanup(
        &self,
        prepared: brassclaw_auth::DcrCleanupPrepared,
    ) -> Result<brassclaw_auth::DcrCleanupAdmission, AuthProductError> {
        self.claim_dcr_cleanup_owned(prepared).await
    }
    async fn record_dcr_cleanup(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: brassclaw_auth::DcrCleanupCompletion,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.record_dcr_cleanup_owned(scope, completion).await
    }
    async fn update_dcr_retirement(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        update: brassclaw_auth::DcrRetirementUpdate,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.update_dcr_retirement_owned(original, update).await
    }

    async fn retain_legacy_dcr(
        &self,
        selection: brassclaw_auth::DcrLegacySelection,
    ) -> Result<brassclaw_auth::DcrLegacyState, brassclaw_auth::AuthProductError> {
        self.retain_legacy_dcr_owned(selection).await
    }

    async fn mark_dcr_cleanup_unavailable(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        issue: brassclaw_auth::DcrCleanupUnavailable,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.mark_dcr_cleanup_unavailable_owned(original, issue)
            .await
    }

    async fn prepare_oauth_setup(
        &self,
        original: brassclaw_auth::OAuthSetupOriginal,
        retirement: Option<brassclaw_auth::OAuthSetupRetirement>,
    ) -> Result<brassclaw_auth::OAuthSetupState, brassclaw_auth::AuthProductError> {
        self.prepare_oauth_setup_owned(original, retirement).await
    }

    async fn read_oauth_setup(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: brassclaw_auth::AuthFlowId,
    ) -> Result<Option<brassclaw_auth::OAuthSetupState>, brassclaw_auth::AuthProductError> {
        self.read_oauth_setup_owned(scope, flow_id).await
    }

    async fn prepare_dcr_registration(
        &self,
        original: brassclaw_auth::DcrRegistrationOriginal,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.prepare_dcr_original(original).await
    }

    async fn retire_dcr_preparation(
        &self,
        original: brassclaw_auth::DcrRegistrationOriginal,
        reason: brassclaw_auth::DcrCleanupReason,
    ) -> Result<brassclaw_auth::DcrRegistrationState, brassclaw_auth::AuthProductError> {
        self.retire_dcr_preparation_owned(original, reason).await
    }
    async fn read_dcr_registration(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<brassclaw_auth::DcrRegistrationState>, AuthProductError> {
        self.read_dcr_original(scope, id).await
    }
    async fn claim_dcr_registration(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
    ) -> Result<brassclaw_auth::DcrRegistrationAdmission, AuthProductError> {
        self.claim_dcr_original(original).await
    }
    async fn record_dcr_registration(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: brassclaw_auth::DcrRegistrationCompletion,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.record_dcr_original(scope, completion).await
    }
    async fn publish_dcr_challenge(
        &self,
        original: &brassclaw_auth::DcrRegistrationOriginal,
        challenge: brassclaw_auth::DcrRegistrationChallenge,
    ) -> Result<brassclaw_auth::DcrRegistrationState, AuthProductError> {
        self.publish_dcr_original(original, challenge).await
    }

    async fn create_flow(&self, request: NewAuthFlow) -> Result<AuthFlowRecord, AuthProductError> {
        let snapshot = if let Some(binding) = &request.update_binding {
            let account = self
                .read_account(&request.scope, binding.account_id)
                .await?
                .map(|(account, _)| account)
                .ok_or(AuthProductError::CredentialMissing)?;
            validate_flow_update_binding(&account, &request)?;
            Some(account)
        } else {
            None
        };
        let now = Utc::now();
        let record = AuthFlowRecord {
            id: request.id.unwrap_or_default(),
            scope: request.scope,
            kind: request.kind,
            status: AuthFlowStatus::AwaitingUser,
            provider: request.provider,
            challenge: Some(request.challenge),
            continuation: request.continuation,
            credential_account_id: None,
            update_binding: request.update_binding,
            opaque_state_hash: request.opaque_state_hash,
            pkce_verifier_hash: request.pkce_verifier_hash,
            authorization_code_hash: None,
            error: None,
            continuation_emitted_at: None,
            created_at: now,
            updated_at: now,
            expires_at: request.expires_at,
        };
        self.write_new_flow(&record, snapshot, request.requirement_setup)
            .await?;
        Ok(record)
    }

    async fn get_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let Some((record, _)) = self.read_flow(scope, flow_id).await? else {
            return Ok(None);
        };
        if !scope_matches(scope, &record.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(Some(record))
    }

    async fn claim_oauth_callback(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        prepared: OAuthCallbackClaimRequest,
    ) -> Result<brassclaw_auth::OAuthCallbackAdmission, AuthProductError> {
        self.admit_callback(scope, prepared).await
    }

    async fn read_oauth_callback_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        reference: &brassclaw_auth::OAuthCallbackReference,
    ) -> Result<Option<brassclaw_auth::OAuthCallbackState>, AuthProductError> {
        self.read_callback_original(scope, reference).await
    }

    async fn reconcile_oauth_callback(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        reference: &brassclaw_auth::OAuthCallbackReference,
    ) -> Result<brassclaw_auth::OAuthCallbackState, AuthProductError> {
        self.reconcile_callback_original(scope, reference).await
    }

    async fn complete_oauth_callback(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.complete_callback_owned(scope, input).await
    }

    async fn complete_credential_selection(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        input: CredentialSelectionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.complete_selection_owned(scope, input).await
    }

    async fn complete_manual_token(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        input: ManualTokenCompletionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.complete_manual_flow_owned(scope, input).await
    }

    async fn cancel_manual_token(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        interaction_id: brassclaw_auth::AuthInteractionId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.cancel_manual_flow_owned(scope, interaction_id).await
    }

    async fn fail_oauth_callback(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.fail_callback_owned(scope, input).await
    }

    async fn cancel_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: AuthFlowId,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if let Some((document, _)) = self.read_flow_document(scope, flow_id).await?
            && let Some(AuthChallenge::ManualTokenRequired { interaction_id, .. }) =
                &document.flow.challenge
        {
            if is_terminal_status(document.flow.status) {
                return Err(if document.flow.status == AuthFlowStatus::Canceled {
                    AuthProductError::Canceled
                } else {
                    AuthProductError::FlowAlreadyTerminal
                });
            }
            let record = self
                .cancel_manual_flow_owned(scope, *interaction_id)
                .await?
                .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
            return if record.status == AuthFlowStatus::Canceled {
                Ok(record)
            } else {
                Err(AuthProductError::FlowAlreadyTerminal)
            };
        }
        if let Some((document, _)) = self.read_flow_document(scope, flow_id).await?
            && !is_terminal_status(document.flow.status)
            && matches!(
                document.flow.challenge,
                Some(AuthChallenge::AccountSelectionRequired { .. })
            )
            && let Some(selected) = &document.auth_custody.requirement_selection
        {
            self.complete_selection_owned(
                scope,
                CredentialSelectionInput {
                    flow_id,
                    credential_account_id: selected.account().id,
                },
            )
            .await?;
            return Err(AuthProductError::FlowAlreadyTerminal);
        }
        // An already selected publication decision is irreversible local
        // custody. Settle it before answering; never acknowledge cancellation
        // while an original account CAS can still publish.
        if let Some((document, _)) = self.read_flow_document(scope, flow_id).await?
            && let Some(decision) = document.fs_callback_publication
        {
            self.reconcile_callback_original(scope, &decision.action.original.reference())
                .await?;
            return Err(AuthProductError::FlowAlreadyTerminal);
        }
        let lock = self.lock_for(format!("flow:{flow_id}"));
        let _guard = lock.lock().await;
        let (mut document, version) = self
            .read_flow_document(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        // The earlier read is not a fence against another owner selecting an
        // account publication decision before this CURRENT document snapshot.
        if document.fs_callback_publication.is_some()
            || (document.auth_custody.requirement_selection.is_some()
                && !is_terminal_status(document.flow.status))
        {
            return Err(AuthProductError::BackendConflict);
        }
        if !scope_matches(scope, &document.flow.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if is_terminal_status(document.flow.status) {
            return Err(if document.flow.status == AuthFlowStatus::Canceled {
                AuthProductError::Canceled
            } else {
                AuthProductError::FlowAlreadyTerminal
            });
        }
        let now = Utc::now();
        document.flow.status = AuthFlowStatus::Canceled;
        document.flow.error = Some(AuthErrorCode::Canceled);
        document.flow.updated_at = now;
        document
            .auth_custody
            .require_dcr_cleanup(&mut document.flow, now)?;
        self.write_flow_document(&document, version.cas()).await?;
        Ok(document.flow)
    }

    async fn mark_continuation_dispatched(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: AuthFlowId,
        emitted_at: brassclaw_auth::Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{flow_id}"));
        let _guard = lock.lock().await;
        let (mut record, version) = self
            .read_flow(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if !scope_matches(scope, &record.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if record.status != AuthFlowStatus::Completed {
            return Err(AuthProductError::FlowAlreadyTerminal);
        }
        // Idempotent: if the continuation was already marked by a concurrent
        // caller, return the existing record without writing.
        if record.continuation_emitted_at.is_some() {
            return Ok(record);
        }
        record.continuation_emitted_at = Some(emitted_at);
        record.updated_at = emitted_at;
        self.write_flow(scope, &record, version.cas()).await?;
        Ok(record)
    }
}

#[async_trait]
impl<F> AuthFlowRecordSource for FilesystemAuthProductServices<F>
where
    F: RootFilesystem + 'static,
{
    async fn flow_for_turn_gate(
        &self,
        query: TurnGateAuthFlowQuery,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        Ok(self
            .flow_records_for_owner(&query.owner)
            .await?
            .into_iter()
            .find(|flow| flow_matches_turn_gate_query(flow, &query)))
    }

    async fn flows_for_owner(
        &self,
        owner: brassclaw_auth::AuthFlowOwnerScope,
    ) -> Result<Vec<AuthFlowRecord>, AuthProductError> {
        self.flow_records_for_owner(&owner).await
    }
}
