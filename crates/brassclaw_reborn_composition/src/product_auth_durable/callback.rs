//! Original callback custody on the actual CAS-only filesystem owner.
use super::*;
use crate::auth_flow_custody::{FilesystemCallbackPublication, StoredAuthFlow};
use brassclaw_auth::{
    OAuthCallbackAdmission, OAuthCallbackCompletion, OAuthCallbackFailureInput, OAuthCallbackInput,
    OAuthCallbackOutcome, OAuthCallbackPrepared, OAuthCallbackPublication, OAuthCallbackReference,
    OAuthCallbackState, ProviderCallbackOutcome,
};

fn state(
    document: &StoredAuthFlow,
    reference: &OAuthCallbackReference,
) -> Result<OAuthCallbackState, AuthProductError> {
    document
        .auth_custody
        .original_state(&document.flow, reference)?
        .ok_or(AuthProductError::BackendConflict)
}

fn verify_decision(
    document: &StoredAuthFlow,
    decision: &FilesystemCallbackPublication,
) -> Result<(), AuthProductError> {
    let pending = state(document, &decision.action.original.reference())?;
    if decision.account_version.is_some() != pending.action.expected_account.is_some()
        || (decision.account_body_checksum.is_some() && decision.account_version.is_none())
    {
        return Err(AuthProductError::BackendConflict);
    }
    // Reproduce the pure decision at its ORIGINAL commit point. No current
    // account lookup, fresh time, changed scope or arbitrary final flow is used.
    let mut expected_action = pending.action;
    let mut expected_flow = pending.flow;
    let expected_account = expected_action.expected_account.clone();
    let account = expected_action.publish(
        &mut expected_flow,
        expected_account.as_ref(),
        decision.action.updated_at,
    )?;
    if account.is_none()
        || expected_action != decision.action
        || expected_flow != decision.final_flow
    {
        return Err(AuthProductError::BackendConflict);
    }
    OAuthCallbackState {
        flow: decision.final_flow.clone(),
        action: decision.action.clone(),
    }
    .completed_flow()?;
    if document.auth_custody.requirement_selection != decision.requirement_selection {
        return Err(AuthProductError::BackendConflict);
    }
    if let Some(selected) = &decision.requirement_selection {
        if let Some(source) = &document.auth_custody.requirement_source {
            source.verify_selection(selected)?;
        }
        let setup = document
            .auth_custody
            .requirement_setup
            .as_deref()
            .ok_or(AuthProductError::BackendConflict)?;
        let account = decision
            .action
            .published_account
            .clone()
            .ok_or(AuthProductError::BackendConflict)?;
        if brassclaw_auth::CredentialRequirementSelection::from_completed_flow(
            &document.flow.scope,
            setup,
            &decision.final_flow,
            account,
        )? != **selected
        {
            return Err(AuthProductError::BackendConflict);
        }
    } else if document.auth_custody.requirement_source.is_some() {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}

impl<F: RootFilesystem> FilesystemAuthProductServices<F> {
    pub(super) async fn read_callback_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        reference: &OAuthCallbackReference,
    ) -> Result<Option<OAuthCallbackState>, AuthProductError> {
        if !scope_matches(scope, &reference.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let Some((document, _)) = self.read_flow_document(scope, reference.flow_id).await? else {
            return Ok(None);
        };
        document
            .auth_custody
            .original_state(&document.flow, reference)
    }

    pub(super) async fn admit_callback(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        prepared: OAuthCallbackPrepared,
    ) -> Result<OAuthCallbackAdmission, AuthProductError> {
        if !scope_matches(scope, &prepared.original.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let reference = prepared.original.reference();
        if let Some(existing) = self.read_callback_original(scope, &reference).await? {
            existing.action.original.verify(&prepared.original)?;
            return Ok(OAuthCallbackAdmission::Original(existing));
        }
        let lock = self.lock_for(format!("flow:{}", reference.flow_id));
        let _guard = lock.lock().await;
        let (mut document, version) = self
            .read_flow_document(scope, reference.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let current = match document.flow.update_binding.as_ref() {
            Some(binding) => Some(
                self.read_account(scope, binding.account_id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?
                    .0,
            ),
            None => self
                .read_account(
                    scope,
                    brassclaw_auth::CredentialAccountId::from_uuid(document.flow.id.as_uuid()),
                )
                .await?
                .map(|(account, _)| account),
        };
        let admission =
            document
                .auth_custody
                .admit(&mut document.flow, current.as_ref(), prepared, Utc::now());
        let admission = match admission {
            Ok(admission) => admission,
            Err(error @ AuthProductError::UnknownOrExpiredFlow) => {
                self.write_flow_document(&document, version.cas()).await?;
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        if matches!(admission, OAuthCallbackAdmission::Original(_)) {
            return Ok(admission);
        }
        if let Err(error) = self.write_flow_document(&document, version.cas()).await {
            return match self.read_callback_original(scope, &reference).await? {
                Some(existing) => Ok(OAuthCallbackAdmission::Original(existing)),
                None => Err(error),
            };
        }
        // Only an acknowledged CAS may return Dispatch. Lost acknowledgement
        // and a competing owner return the stored Original, never another call.
        Ok(admission)
    }

    async fn record_callback_result(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        completion: OAuthCallbackCompletion,
    ) -> Result<OAuthCallbackState, AuthProductError> {
        let reference = completion.dispatched.original.reference();
        if !scope_matches(scope, &reference.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let lock = self.lock_for(format!("flow:{}", reference.flow_id));
        let _guard = lock.lock().await;
        let (mut document, version) = self
            .read_flow_document(scope, reference.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let before = state(&document, &reference)?;
        let mut action = before.action.clone();
        action.record_result(completion, Utc::now())?;
        if action == before.action {
            return Ok(before);
        }
        if document.fs_callback_publication.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        document.auth_custody.oauth_callback = Some(action.clone());
        if !is_terminal_status(document.flow.status) {
            document.flow.status = brassclaw_auth::AuthFlowStatus::Completing;
            document.flow.updated_at = action.updated_at;
        }
        if let Err(error) = self.write_flow_document(&document, version.cas()).await {
            let after = self
                .read_callback_original(scope, &reference)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            if after.action.same_intent(&action) && after.action.outcome == action.outcome {
                return Ok(after);
            }
            return Err(error);
        }
        state(&document, &reference)
    }

    async fn save_callback_final_flow(
        &self,
        document: StoredAuthFlow,
        version: AuthRecordSnapshot,
        reference: &OAuthCallbackReference,
    ) -> Result<OAuthCallbackState, AuthProductError> {
        let expected = state(&document, reference)?;
        if let Err(error) = self.write_flow_document(&document, version.cas()).await {
            let actual = self
                .read_callback_original(&reference.scope, reference)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            if actual.action.same_intent(&expected.action)
                && actual.action.outcome == expected.action.outcome
                && actual.action.publication != OAuthCallbackPublication::Pending
            {
                return Ok(actual);
            }
            return Err(error);
        }
        Ok(expected)
    }

    pub(super) async fn reconcile_callback_original(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        reference: &OAuthCallbackReference,
    ) -> Result<OAuthCallbackState, AuthProductError> {
        let original = self
            .read_callback_original(scope, reference)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if !original.action.needs_publication() {
            return Ok(original);
        }
        let lock = self.lock_for(format!("flow:{}", reference.flow_id));
        let _guard = lock.lock().await;
        let (mut document, mut flow_version) = self
            .read_flow_document(scope, reference.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        let original = state(&document, reference)?;
        if !original.action.needs_publication() {
            return Ok(original);
        }
        if document.fs_callback_publication.is_none() {
            if document.auth_custody.requirement_selection.is_some()
                || document.auth_custody.requirement_source.is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            let current = self
                .read_account_document(scope, original.action.target_account_id())
                .await?;
            let account_snapshot = current.as_ref().map(|(_, snapshot)| *snapshot);
            let account_version = account_snapshot.map(|snapshot| snapshot.version);
            let account_body_checksum = account_snapshot.map(|snapshot| snapshot.body_checksum);
            let mut final_action = original.action;
            let mut final_flow = document.flow.clone();
            let account = final_action.publish(
                &mut final_flow,
                current.as_ref().map(|(doc, _)| &doc.account),
                Utc::now(),
            )?;
            if account.is_none() {
                document.flow = final_flow;
                document.auth_custody.oauth_callback = Some(final_action);
                return self
                    .save_callback_final_flow(document, flow_version, reference)
                    .await;
            }
            let requirement_selection = document
                .auth_custody
                .requirement_setup
                .as_deref()
                .map(|setup| {
                    brassclaw_auth::CredentialRequirementSelection::from_completed_flow(
                        scope,
                        setup,
                        &final_flow,
                        account.clone().ok_or(AuthProductError::BackendConflict)?,
                    )
                    .map(Box::new)
                })
                .transpose()?;
            document.auth_custody.requirement_source = match &requirement_selection {
                Some(selected) => self.capture_requirement_source(selected).await?,
                None => None,
            };
            document.auth_custody.requirement_selection = requirement_selection.clone();
            document.flow.status = brassclaw_auth::AuthFlowStatus::Completing;
            document.flow.updated_at = final_action.updated_at;
            document.fs_callback_publication = Some(FilesystemCallbackPublication {
                action: final_action,
                final_flow,
                account_version,
                account_body_checksum,
                requirement_selection,
            });
            // Cancellation/expiry wins before this exact CAS; after it the
            // original decision must settle before cancellation can be answered.
            match self
                .write_flow_document(&document, flow_version.cas())
                .await
            {
                Ok(version) => flow_version = version,
                Err(error) => {
                    let (actual, version) = self
                        .read_flow_document(scope, reference.flow_id)
                        .await?
                        .ok_or(AuthProductError::BackendUnavailable)?;
                    let actual_state = state(&actual, reference)?;
                    if actual.fs_callback_publication.is_none() {
                        if !actual_state.action.needs_publication() {
                            return Ok(actual_state);
                        }
                        return Err(error);
                    }
                    document = actual;
                    flow_version = version;
                }
            }
        }
        let decision = document
            .fs_callback_publication
            .clone()
            .ok_or(AuthProductError::BackendConflict)?;
        verify_decision(&document, &decision)?;
        let account_id = decision.action.target_account_id();
        let current = self.read_account_document(scope, account_id).await?;
        let current_receipt = current
            .as_ref()
            .and_then(|(doc, _)| doc.auth_callback_actions.get(&reference.flow_id));
        if let Some(receipt) = current_receipt {
            if receipt != &decision.action {
                return Err(AuthProductError::BackendConflict);
            }
        } else {
            let expected_snapshot = match (decision.account_version, decision.account_body_checksum)
            {
                (Some(version), Some(body_checksum)) => Some(AuthRecordSnapshot {
                    version,
                    body_checksum,
                }),
                (None, None) => None,
                // A historical decision without a same-account receipt has no
                // captured body identity. Preserve it as unresolved rather than
                // silently allowing its weaker version-only write after upgrade.
                _ => return Err(AuthProductError::BackendConflict),
            };
            let version = current.as_ref().map(|(_, snapshot)| *snapshot);
            if version != expected_snapshot {
                // A legitimate later account writer preserves all receipts. If
                // none exists and the selected version changed, this old CAS
                // cannot publish and must never overwrite that later writer.
                document.fs_callback_publication = None;
                let action = document
                    .auth_custody
                    .oauth_callback
                    .as_mut()
                    .ok_or(AuthProductError::BackendConflict)?;
                action.publication = OAuthCallbackPublication::Superseded;
                action.updated_at = Utc::now();
                document.flow.status = brassclaw_auth::AuthFlowStatus::Failed;
                document.flow.error = Some(brassclaw_auth::AuthErrorCode::FlowAlreadyTerminal);
                document.flow.updated_at = action.updated_at;
                return self
                    .save_callback_final_flow(document, flow_version, reference)
                    .await;
            }
            let published = decision
                .action
                .published_account
                .clone()
                .ok_or(AuthProductError::BackendConflict)?;
            let mut account = match current {
                Some((doc, _)) => {
                    if Some(&doc.account) != decision.action.expected_account.as_ref() {
                        return Err(AuthProductError::BackendConflict);
                    }
                    doc
                }
                None => refresh::StoredCredentialAccount {
                    account: published.clone(),
                    auth_refresh_actions: Default::default(),
                    auth_callback_actions: Default::default(),
                    auth_manual_actions: Default::default(),
                    auth_lifecycle_cleanup: Default::default(),
                },
            };
            account.account = published;
            account
                .auth_callback_actions
                .insert(reference.flow_id, decision.action.clone());
            let cas = expected_snapshot.map_or(CasExpectation::Absent, AuthRecordSnapshot::cas);
            if let Err(error) = self.write_account_document(&account, cas).await {
                let after = self.read_account_document(scope, account_id).await?;
                let receipt = after
                    .as_ref()
                    .and_then(|(doc, _)| doc.auth_callback_actions.get(&reference.flow_id));
                // Unknown writes stay pending unless their EXACT account receipt
                // is readable. A later call can reconcile under the same CAS.
                if receipt != Some(&decision.action) {
                    return Err(error);
                }
            }
        }
        if decision.requirement_selection.is_some()
            && let Err(error) = self
                .publish_requirement_head(&document, &decision.final_flow)
                .await
        {
            if error != AuthProductError::BackendConflict {
                return Err(error);
            }
            // The account receipt already proves a real applied effect. A head
            // conflict must not undo it, re-exchange, or report a completed flow.
            document.flow = decision.final_flow;
            document.flow.status = brassclaw_auth::AuthFlowStatus::Failed;
            document.flow.error = Some(brassclaw_auth::AuthErrorCode::AccountSelectionRequired);
            document.flow.updated_at = Utc::now();
            document.auth_custody.oauth_callback = Some(decision.action);
            document.fs_callback_publication = None;
            return self
                .save_callback_final_flow(document, flow_version, reference)
                .await;
        }
        document.flow = decision.final_flow;
        document.auth_custody.oauth_callback = Some(decision.action);
        document.fs_callback_publication = None;
        self.save_callback_final_flow(document, flow_version, reference)
            .await
    }

    async fn finish_callback_without_exchange(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: AuthFlowId,
        hash: &brassclaw_auth::OpaqueStateHash,
        error: brassclaw_auth::AuthErrorCode,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let lock = self.lock_for(format!("flow:{flow_id}"));
        let _guard = lock.lock().await;
        let (mut document, version) = self
            .read_flow_document(scope, flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if document.fs_callback_publication.is_some() {
            // The original publication commit point precedes this callback.
            // Even elapsed TTL or a denial cannot rewrite that chosen decision.
            return Err(AuthProductError::BackendConflict);
        }
        let now = Utc::now();
        match brassclaw_auth::domain::prepare_callback_flow(&mut document.flow, scope, hash, now) {
            Ok(()) => {}
            Err(error @ AuthProductError::UnknownOrExpiredFlow) => {
                self.write_flow_document(&document, version.cas()).await?;
                return Err(error);
            }
            Err(error) => return Err(error),
        }
        if document.auth_custody.oauth_callback.is_some()
            || document.fs_callback_publication.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        if !matches!(
            document.flow.challenge,
            Some(brassclaw_auth::AuthChallenge::OAuthUrl { .. })
        ) {
            return Err(AuthProductError::MalformedCallback);
        }
        document.flow.status = brassclaw_auth::AuthFlowStatus::Failed;
        document.flow.error = Some(error);
        document.flow.updated_at = now;
        self.write_flow_document(&document, version.cas()).await?;
        Ok(document.flow)
    }

    pub(super) async fn complete_callback_owned(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        match input.outcome {
            ProviderCallbackOutcome::Authorized { exchange } => {
                let dispatched = input.dispatched.ok_or(AuthProductError::BackendConflict)?;
                if dispatched.original.flow_id != input.flow_id
                    || !dispatched
                        .original
                        .opaque_state_hash
                        .constant_time_eq(&input.opaque_state_hash)
                {
                    return Err(AuthProductError::CrossScopeDenied);
                }
                let reference = dispatched.original.reference();
                self.record_callback_result(
                    scope,
                    OAuthCallbackCompletion {
                        dispatched,
                        outcome: OAuthCallbackOutcome::Returned(exchange),
                    },
                )
                .await?;
                self.reconcile_callback_original(scope, &reference)
                    .await?
                    .completed_flow()
            }
            ProviderCallbackOutcome::Denied => {
                if input.dispatched.is_some() {
                    return Err(AuthProductError::MalformedCallback);
                }
                self.finish_callback_without_exchange(
                    scope,
                    input.flow_id,
                    &input.opaque_state_hash,
                    brassclaw_auth::AuthErrorCode::ProviderDenied,
                )
                .await?;
                Err(AuthProductError::ProviderDenied)
            }
        }
    }

    pub(super) async fn fail_callback_owned(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        match input.dispatched {
            Some(dispatched) => {
                if dispatched.original.flow_id != input.flow_id
                    || !dispatched
                        .original
                        .opaque_state_hash
                        .constant_time_eq(&input.opaque_state_hash)
                {
                    return Err(AuthProductError::CrossScopeDenied);
                }
                let reference = dispatched.original.reference();
                self.record_callback_result(
                    scope,
                    OAuthCallbackCompletion {
                        dispatched,
                        outcome: OAuthCallbackOutcome::Unknown(input.error),
                    },
                )
                .await?;
                Ok(self
                    .reconcile_callback_original(scope, &reference)
                    .await?
                    .flow)
            }
            None => {
                self.finish_callback_without_exchange(
                    scope,
                    input.flow_id,
                    &input.opaque_state_hash,
                    input.error,
                )
                .await
            }
        }
    }
}
