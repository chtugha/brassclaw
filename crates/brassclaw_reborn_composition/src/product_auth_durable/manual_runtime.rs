//! CAS-only manual publication in the existing interaction/account/flow owners.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::{
    AuthInteractionId, AuthProductScope, ManualTokenSetupIdentity, SecretSubmitRequest,
};

fn same_pending(current: &StoredManualTokenInteraction, original: &ManualOriginal) -> bool {
    let mut normalized = current.clone();
    normalized.publication = None;
    normalized.consumed_at = None;
    normalized.canceled_at = None;
    if normalized.update_account_snapshot.is_none() {
        normalized.update_account_snapshot = original.interaction.update_account_snapshot.clone();
    }
    normalized == original.interaction
}
fn decision_matches(
    current: &StoredManualTokenInteraction,
    original: &ManualOriginal,
    source: &SecretCustodySource,
) -> Result<(), AuthProductError> {
    if !same_pending(current, original)
        || current.publication.as_ref().map(ManualDecision::original) != Some(source)
    {
        return Err(AuthProductError::BackendConflict);
    }
    match &current.publication {
        Some(ManualDecision::Selected { selected_at, .. })
            if *selected_at <= current.expires_at
                && current.consumed_at.is_none()
                && current.canceled_at.is_none() =>
        {
            Ok(())
        }
        Some(ManualDecision::Published { .. }) if current.consumed_at.is_some() => Ok(()),
        Some(ManualDecision::Superseded { .. }) => Err(AuthProductError::BackendConflict),
        _ => Err(AuthProductError::BackendConflict),
    }
}
pub(super) fn final_flow_matches(current: &AuthFlowRecord, expected: &AuthFlowRecord) -> bool {
    let mut normalized = current.clone();
    normalized.continuation_emitted_at = expected.continuation_emitted_at;
    normalized.updated_at = expected.updated_at;
    &normalized == expected
}

impl<F: RootFilesystem + 'static> FilesystemAuthProductServices<F> {
    pub(crate) async fn continue_manual_original_owned(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let handle = original_handle(identity.interaction_id)?;
        let metadata = self
            .secret_store
            .custody(&scope.resource, &handle)
            .await
            .map_err(custody_error)?
            .ok_or(AuthProductError::BackendConflict)?;
        if metadata.scope != scope.resource || metadata.handle != handle {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let (reference, context) = match metadata.state {
            brassclaw_secrets::SecretCustodyState::Original { context } => {
                (metadata.reference, context)
            }
            brassclaw_secrets::SecretCustodyState::RetiredOriginal {
                original_reference,
                context,
            } => (original_reference, context),
            _ => return Err(AuthProductError::BackendConflict),
        };
        let source = SecretCustodySource {
            scope: metadata.scope,
            handle: metadata.handle,
            reference,
        };
        let original: ManualOriginal =
            serde_json::from_value(context).map_err(|_| AuthProductError::BackendConflict)?;
        original.verify()?;
        if original.interaction.id != identity.interaction_id
            || !scope_matches(scope, &original.interaction.scope)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let current = self
            .manual_interaction(scope, identity.interaction_id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?
            .0;
        decision_matches(&current, &original, &source)?;
        if !matches!(current.publication, Some(ManualDecision::Published { .. })) {
            return Err(AuthProductError::BackendConflict);
        }
        let expected = original
            .completed_flow()?
            .ok_or(AuthProductError::BackendConflict)?;
        if expected.id != identity.flow_id {
            return Err(AuthProductError::BackendConflict);
        }
        let document = self
            .read_flow_document(scope, identity.flow_id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?
            .0;
        original.verify_completed_document(&document)?;
        // An intent cannot pass this confirmed publication/flow receipt check.
        Ok((original.result(), document.flow))
    }

    async fn manual_interaction(
        &self,
        scope: &AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<Option<(StoredManualTokenInteraction, AuthRecordSnapshot)>, AuthProductError> {
        let row: Option<(StoredManualTokenInteraction, AuthRecordSnapshot)> = self
            .read_record(
                &scope.resource,
                &super::super::paths::interaction_path(scope, id),
            )
            .await?;
        if let Some((pending, _)) = &row
            && (pending.id != id || !scope_matches(scope, &pending.scope))
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(row)
    }
    async fn manual_flow_document(
        &self,
        scope: &AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<Option<(StoredAuthFlow, AuthRecordSnapshot)>, AuthProductError> {
        let candidates=self.flows_for_scope(scope).await?.into_iter().filter_map(|(flow,_)| {
            matches!(&flow.challenge,Some(AuthChallenge::ManualTokenRequired{interaction_id,..}) if *interaction_id==id)
                .then_some(flow.id)
        }).collect::<Vec<_>>();
        if candidates.len() > 1 {
            return Err(AuthProductError::BackendConflict);
        }
        let Some(flow) = candidates.first() else {
            return Ok(None);
        };
        let result = self
            .read_flow_document(scope, *flow)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if result.0.flow.kind != AuthFlowKind::IntegrationCredential
            || !matches!(&result.0.flow.challenge,Some(AuthChallenge::ManualTokenRequired{provider,expires_at,..})
                if *provider==result.0.flow.provider && *expires_at==result.0.flow.expires_at)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if result.0.fs_callback_publication.is_some()
            || result.0.auth_custody.oauth_callback.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(Some(result))
    }
    async fn original_from_reference(
        &self,
        scope: &AuthProductScope,
        source: &SecretCustodySource,
    ) -> Result<ManualOriginal, AuthProductError> {
        if source.scope != scope.resource {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let metadata = self
            .secret_store
            .custody(&source.scope, &source.handle)
            .await
            .map_err(custody_error)?
            .ok_or(AuthProductError::BackendConflict)?;
        if metadata.scope != source.scope || metadata.handle != source.handle {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let (reference, context) = match metadata.state {
            brassclaw_secrets::SecretCustodyState::Original { context } => {
                (metadata.reference, context)
            }
            brassclaw_secrets::SecretCustodyState::RetiredOriginal {
                original_reference,
                context,
            } => (original_reference, context),
            _ => return Err(AuthProductError::BackendConflict),
        };
        if reference != source.reference {
            return Err(AuthProductError::BackendConflict);
        }
        let original: ManualOriginal =
            serde_json::from_value(context).map_err(|_| AuthProductError::BackendConflict)?;
        original.verify()?;
        if !scope_matches(scope, &original.interaction.scope)
            || source.handle != original_handle(original.interaction.id)?
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(original)
    }
    async fn prepare_manual_original(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
        complete_flow: bool,
        expected_flow: Option<AuthFlowId>,
    ) -> Result<(ManualOriginal, SecretCustodySource), AuthProductError> {
        if let Some(original) = self.prove_manual_original(scope, &request).await? {
            if expected_flow.is_some()
                && expected_flow != original.0.pending_flow.as_ref().map(|flow| flow.id)
            {
                return Err(AuthProductError::BackendConflict);
            }
            if complete_flow && !original.0.complete_flow {
                return Err(AuthProductError::BackendConflict);
            }
            return Ok(original);
        }
        let flow = self
            .manual_flow_document(scope, request.interaction_id)
            .await?;
        if expected_flow.is_some()
            && expected_flow != flow.as_ref().map(|(document, _)| document.flow.id)
        {
            return Err(AuthProductError::BackendConflict);
        }
        if let Some((document, _)) = &flow {
            match document.flow.status {
                AuthFlowStatus::Completed => return Err(AuthProductError::BackendConflict),
                AuthFlowStatus::Canceled => return Err(AuthProductError::Canceled),
                AuthFlowStatus::Expired => return Err(AuthProductError::UnknownOrExpiredFlow),
                AuthFlowStatus::Failed => return Err(AuthProductError::FlowAlreadyTerminal),
                _ => {}
            }
        }
        let (mut pending, _) = self
            .manual_interaction(scope, request.interaction_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if pending.canceled_at.is_some() {
            return Err(AuthProductError::Canceled);
        }
        if pending.consumed_at.is_some() || pending.publication.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        if pending.requirement_setup.is_some() && !complete_flow {
            return Err(AuthProductError::BackendConflict);
        }
        if complete_flow && flow.is_none() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        if pending.update_binding.is_some() && pending.update_account_snapshot.is_none() {
            pending.update_account_snapshot = flow
                .as_ref()
                .and_then(|(doc, _)| doc.auth_custody.update_account_snapshot.clone());
        }
        let (selection, candidate) = if let Some(binding) = &pending.update_binding {
            let candidate = self
                .read_account_document(&pending.scope, binding.account_id)
                .await?
                .ok_or(AuthProductError::CredentialMissing)?;
            (
                SelectionKind::Bound,
                Some((candidate.0.account, candidate.1)),
            )
        } else {
            (SelectionKind::New, None)
        };
        if let Some((doc, _)) = &flow
            && (doc.auth_custody.update_account_snapshot != pending.update_account_snapshot
                || doc.auth_custody.requirement_setup != pending.requirement_setup)
        {
            return Err(AuthProductError::BackendConflict);
        }
        let proposed = ManualOriginal::prepare(
            pending,
            selection,
            complete_flow,
            candidate,
            flow,
            Utc::now(),
        )?;
        let chosen = self
            .retain_manual_original(scope, request, proposed)
            .await?;
        if complete_flow && !chosen.0.complete_flow {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(chosen)
    }
    async fn select_manual_decision(
        &self,
        original: &ManualOriginal,
        source: &SecretCustodySource,
    ) -> Result<StoredManualTokenInteraction, AuthProductError> {
        let scope = &original.interaction.scope;
        let (pending, snapshot) = self
            .manual_interaction(scope, original.interaction.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if pending.publication.is_some() {
            decision_matches(&pending, original, source)?;
            return Ok(pending);
        }
        if pending.canceled_at.is_some() {
            return Err(AuthProductError::Canceled);
        }
        if pending.consumed_at.is_some() || !same_pending(&pending, original) {
            return Err(AuthProductError::BackendConflict);
        }
        let now = Utc::now();
        if now > pending.expires_at {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let mut selected = original.interaction.clone();
        selected.publication = Some(ManualDecision::Selected {
            original: source.clone(),
            selected_at: now,
        });
        if let Err(error) = self
            .write_record(
                &scope.resource,
                &super::super::paths::interaction_path(scope, selected.id),
                &selected,
                snapshot.cas(),
            )
            .await
        {
            let current = self
                .manual_interaction(scope, selected.id)
                .await?
                .ok_or(error.clone())?
                .0;
            if current.canceled_at.is_some() && current.publication.is_none() {
                return Err(AuthProductError::Canceled);
            }
            decision_matches(&current, original, source)?;
            return Ok(current);
        }
        Ok(selected)
    }
    async fn ensure_manual_output(
        &self,
        original: &ManualOriginal,
        source: &SecretCustodySource,
    ) -> Result<(), AuthProductError> {
        let handle = original
            .account
            .access_secret
            .as_ref()
            .ok_or(AuthProductError::BackendConflict)?;
        let scope = &original.account.scope.resource;
        let matches = |metadata: &brassclaw_secrets::SecretCustodyMetadata| {
            metadata.scope == *scope
                && metadata.handle == *handle
                && matches!(&metadata.state,brassclaw_secrets::SecretCustodyState::Retained{source:actual} if actual==source)
        };
        if let Some(metadata) = self
            .secret_store
            .custody(scope, handle)
            .await
            .map_err(custody_error)?
        {
            return if matches(&metadata) {
                Ok(())
            } else {
                Err(AuthProductError::BackendConflict)
            };
        }
        let result = self
            .secret_store
            .put_retained_from_original(scope.clone(), handle.clone(), source.clone())
            .await;
        match result {
            Ok(Some(metadata)) if matches(&metadata) => Ok(()),
            Ok(Some(_)) => Err(AuthProductError::BackendConflict),
            other => {
                let error = match other {
                    Err(e) => custody_error(e),
                    _ => AuthProductError::BackendConflict,
                };
                let metadata = self
                    .secret_store
                    .custody(scope, handle)
                    .await
                    .map_err(custody_error)?;
                if metadata.as_ref().is_some_and(matches) {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }
    async fn mark_manual_phase(
        &self,
        original: &ManualOriginal,
        source: &SecretCustodySource,
        published: bool,
    ) -> Result<(), AuthProductError> {
        let scope = &original.interaction.scope;
        let (mut pending, snapshot) = self
            .manual_interaction(scope, original.interaction.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        decision_matches(&pending, original, source)?;
        if matches!(pending.publication, Some(ManualDecision::Published { .. })) {
            return Ok(());
        }
        pending.publication = Some(if published {
            ManualDecision::Published {
                original: source.clone(),
            }
        } else {
            ManualDecision::Superseded {
                original: source.clone(),
            }
        });
        if published {
            pending.consumed_at = Some(Utc::now());
        }
        if let Err(error) = self
            .write_record(
                &scope.resource,
                &super::super::paths::interaction_path(scope, pending.id),
                &pending,
                snapshot.cas(),
            )
            .await
        {
            let actual = self
                .manual_interaction(scope, pending.id)
                .await?
                .ok_or(error.clone())?
                .0;
            if published
                && matches!(&actual.publication,Some(ManualDecision::Published{original:actual_source}) if actual_source==source)
            {
                decision_matches(&actual, original, source)?;
                return Ok(());
            }
            return Err(error);
        }
        Ok(())
    }
    async fn publish_manual_account(
        &self,
        original: &ManualOriginal,
        source: &SecretCustodySource,
    ) -> Result<(), AuthProductError> {
        let scope = &original.interaction.scope;
        let (pending, _) = self
            .manual_interaction(scope, original.interaction.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        decision_matches(&pending, original, source)?;
        if matches!(pending.publication, Some(ManualDecision::Published { .. })) {
            return Ok(());
        }
        let expected_receipt = ManualReceipt::new(original, source);
        let current = self
            .read_account_document(&original.account.scope, original.account.id)
            .await?;
        if let Some(receipt) = current
            .as_ref()
            .and_then(|(doc, _)| doc.auth_manual_actions.get(&original.interaction.id))
        {
            if receipt != &expected_receipt {
                return Err(AuthProductError::BackendConflict);
            }
        } else {
            let selected = original.account_entry.map(SelectedEntry::snapshot);
            if current.as_ref().map(|(_, snapshot)| *snapshot) != selected
                || current.as_ref().map(|(doc, _)| &doc.account)
                    != original.expected_account.as_ref()
            {
                self.mark_manual_phase(original, source, false).await?;
                return Err(AuthProductError::BackendConflict);
            }
            if let (Some(flow), Some(selected_flow)) = (&original.pending_flow, original.flow_entry)
            {
                let (current_flow, current_snapshot) = self
                    .read_flow_document(scope, flow.id)
                    .await?
                    .ok_or(AuthProductError::BackendConflict)?;
                if current_snapshot != selected_flow.snapshot() {
                    return Err(match current_flow.flow.status {
                        AuthFlowStatus::Canceled => AuthProductError::Canceled,
                        AuthFlowStatus::Expired => AuthProductError::UnknownOrExpiredFlow,
                        _ => AuthProductError::BackendConflict,
                    });
                }
            }
            self.ensure_manual_output(original, source).await?;
            let mut document = current.map(|(doc, _)| doc).unwrap_or_else(|| {
                super::super::refresh::StoredCredentialAccount {
                    account: original.account.clone(),
                    auth_refresh_actions: Default::default(),
                    auth_callback_actions: Default::default(),
                    auth_manual_actions: Default::default(),
                    auth_lifecycle_cleanup: Default::default(),
                }
            });
            document.account = original.account.clone();
            document
                .auth_manual_actions
                .insert(original.interaction.id, expected_receipt.clone());
            if let Err(error) = self
                .write_account_document(
                    &document,
                    selected.map_or(CasExpectation::Absent, AuthRecordSnapshot::cas),
                )
                .await
            {
                let after = self
                    .read_account_document(&original.account.scope, original.account.id)
                    .await?;
                if after
                    .as_ref()
                    .and_then(|(doc, _)| doc.auth_manual_actions.get(&original.interaction.id))
                    != Some(&expected_receipt)
                {
                    // Unknown writes remain pending; no rollback/cleanup. A
                    // later same-source attempt checks its actual receipt/CAS.
                    return Err(error);
                }
            }
        }
        self.mark_manual_phase(original, source, true).await
    }
    async fn finish_manual_flow(
        &self,
        original: &ManualOriginal,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if original.requirement_selection()?.is_some() {
            return self.finish_manual_requirement(original).await;
        }
        let final_flow = original
            .completed_flow()?
            .ok_or(AuthProductError::BackendConflict)?;
        let scope = &original.interaction.scope;
        let (mut document, snapshot) = self
            .read_flow_document(scope, final_flow.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if final_flow_matches(&document.flow, &final_flow) {
            original.verify_completed_document(&document)?;
            return Ok(document.flow);
        }
        match document.flow.status {
            AuthFlowStatus::Canceled => return Err(AuthProductError::Canceled),
            AuthFlowStatus::Expired => return Err(AuthProductError::UnknownOrExpiredFlow),
            AuthFlowStatus::Failed | AuthFlowStatus::Completed => {
                return Err(AuthProductError::FlowAlreadyTerminal);
            }
            _ => {}
        }
        if Some(snapshot) != original.flow_entry.map(SelectedEntry::snapshot)
            || document.fs_callback_publication.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        if !original.complete_flow
            && let Err(error) = super::super::domain::validate_manual_token_flow(
                &mut document.flow,
                scope,
                &ManualTokenCompletionInput {
                    interaction_id: original.interaction.id,
                    credential_account_id: original.account.id,
                },
                Utc::now(),
            )
        {
            if error == AuthProductError::UnknownOrExpiredFlow {
                self.write_flow_document(&document, snapshot.cas()).await?;
            }
            return Err(error);
        }
        document.flow = final_flow.clone();
        if let Err(error) = self.write_flow_document(&document, snapshot.cas()).await {
            let after = self
                .read_flow(scope, final_flow.id)
                .await?
                .ok_or(error.clone())?
                .0;
            if final_flow_matches(&after, &final_flow) {
                return Ok(after);
            }
            return Err(error);
        }
        Ok(final_flow)
    }
    pub(in crate::product_auth_durable) async fn settle_manual_reference(
        &self,
        scope: &AuthProductScope,
        source: &SecretCustodySource,
        finish_flow: bool,
    ) -> Result<(SecretSubmitResult, Option<AuthFlowRecord>), AuthProductError> {
        let original = self.original_from_reference(scope, source).await?;
        // This port settles an EXISTING decision, never creates a new one from
        // a copied metadata reference or an unaccepted original intent.
        let (pending, _) = self
            .manual_interaction(scope, original.interaction.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        decision_matches(&pending, &original, source)?;
        self.publish_manual_account(&original, source).await?;
        let flow = if finish_flow {
            Some(self.finish_manual_flow(&original).await?)
        } else {
            None
        };
        Ok((original.result(), flow))
    }
    pub(crate) async fn submit_manual_owned(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
        complete_flow: bool,
        expected_flow: Option<AuthFlowId>,
    ) -> Result<(SecretSubmitResult, Option<AuthFlowRecord>), AuthProductError> {
        super::super::interactions::validate_secret(&request)?;
        let (original, source) = self
            .prepare_manual_original(scope, request, complete_flow, expected_flow)
            .await?;
        self.select_manual_decision(&original, &source).await?;
        self.settle_manual_reference(scope, &source, complete_flow)
            .await
    }
}

impl<F: RootFilesystem + 'static> FilesystemAuthProductServices<F> {
    /// Only close an undecided interaction. A chosen account publication first
    /// settles through its same-record receipt; this primitive never completes
    /// the flow or deletes its original input/result.
    pub(in crate::product_auth_durable) async fn abandon_manual_interaction_owned(
        &self,
        scope: &AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        let Some((mut pending, snapshot)) = self.manual_interaction(scope, id).await? else {
            return Ok(false);
        };
        if let Some(decision) = &pending.publication {
            match decision {
                ManualDecision::Selected { original, .. } => {
                    self.settle_manual_reference(scope, original, false).await?;
                    return Ok(false);
                }
                ManualDecision::Published { .. } => return Ok(false),
                ManualDecision::Superseded { .. } => {}
            }
        }
        if pending.canceled_at.is_some() {
            if pending.setup_flow_id.is_some() {
                self.reconcile_original_fs_setup_projection(&pending)
                    .await?;
            }
            return Ok(false);
        }
        pending.canceled_at = Some(Utc::now());
        if let Err(error) = self
            .write_record(
                &scope.resource,
                &super::super::paths::interaction_path(scope, id),
                &pending,
                snapshot.cas(),
            )
            .await
        {
            let current = self
                .manual_interaction(scope, id)
                .await?
                .ok_or(error.clone())?
                .0;
            if current.canceled_at.is_some() {
                if current.setup_flow_id.is_some() {
                    self.reconcile_original_fs_setup_projection(&current)
                        .await?;
                }
                return Ok(true);
            }
            match current.publication {
                Some(ManualDecision::Selected { original, .. }) => {
                    self.settle_manual_reference(scope, &original, false)
                        .await?;
                    return Ok(false);
                }
                Some(ManualDecision::Published { .. }) => return Ok(false),
                _ => return Err(error),
            }
        }
        if pending.setup_flow_id.is_some() {
            self.reconcile_original_fs_setup_projection(&pending)
                .await?;
        }
        Ok(true)
    }
    async fn cancel_manual_barrier(
        &self,
        scope: &AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        if let Err(error) = self.abandon_manual_interaction_owned(scope, id).await {
            let current = self.manual_interaction(scope, id).await?;
            if !current.as_ref().is_some_and(|(pending, _)| {
                matches!(pending.publication, Some(ManualDecision::Superseded { .. }))
            }) {
                return Err(error);
            }
        }
        let Some((pending, _)) = self.manual_interaction(scope, id).await? else {
            return Ok(None);
        };
        match pending.publication {
            Some(ManualDecision::Selected {
                original: source, ..
            })
            | Some(ManualDecision::Published { original: source }) => {
                let original = self.original_from_reference(scope, &source).await?;
                if original.complete_flow {
                    let (_, flow) = self.settle_manual_reference(scope, &source, true).await?;
                    return Ok(flow);
                }
            }
            _ => {}
        }
        Ok(None)
    }
    pub(in crate::product_auth_durable) async fn cancel_manual_flow_owned(
        &self,
        scope: &AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let Some((before, _)) = self.manual_flow_document(scope, id).await? else {
            self.abandon_manual_interaction_owned(scope, id).await?;
            return Ok(None);
        };
        if let Some(completed) = self.cancel_manual_barrier(scope, id).await? {
            return Ok(Some(completed));
        }
        let (mut document, snapshot) = self
            .read_flow_document(scope, before.flow.id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if is_terminal_status(document.flow.status) {
            return Ok(Some(document.flow));
        }
        document.flow.status = AuthFlowStatus::Canceled;
        document.flow.error = Some(brassclaw_auth::AuthErrorCode::Canceled);
        document.flow.updated_at = Utc::now();
        if let Err(error) = self.write_flow_document(&document, snapshot.cas()).await {
            let actual = self
                .read_flow(scope, document.flow.id)
                .await?
                .ok_or(error.clone())?
                .0;
            if is_terminal_status(actual.status) {
                return Ok(Some(actual));
            }
            return Err(error);
        }
        Ok(Some(document.flow))
    }
    pub(in crate::product_auth_durable) async fn complete_manual_flow_owned(
        &self,
        scope: &AuthProductScope,
        input: ManualTokenCompletionInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let Some((document, _)) = self
            .manual_flow_document(scope, input.interaction_id)
            .await?
        else {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        };
        if document.flow.status == AuthFlowStatus::Completed {
            if document.auth_custody.requirement_setup.is_some() {
                let (original, _) = self
                    .read_manual_original(scope, input.interaction_id)
                    .await?
                    .ok_or(AuthProductError::BackendConflict)?;
                original.verify_completed_document(&document)?;
            }
            return if document.flow.credential_account_id == Some(input.credential_account_id)
                && document.flow.error.is_none()
            {
                Ok(document.flow)
            } else {
                Err(AuthProductError::CrossScopeDenied)
            };
        }
        let Some((pending, _)) = self.manual_interaction(scope, input.interaction_id).await? else {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        };
        let Some(decision) = pending.publication else {
            if Utc::now() > document.flow.expires_at {
                if let Some(completed) = self
                    .cancel_manual_barrier(scope, input.interaction_id)
                    .await?
                {
                    return if completed.credential_account_id == Some(input.credential_account_id) {
                        Ok(completed)
                    } else {
                        Err(AuthProductError::CrossScopeDenied)
                    };
                }
                let (mut actual, snapshot) = self
                    .read_flow_document(scope, document.flow.id)
                    .await?
                    .ok_or(AuthProductError::BackendConflict)?;
                let error = match super::super::domain::validate_manual_token_flow(
                    &mut actual.flow,
                    scope,
                    &input,
                    Utc::now(),
                ) {
                    Err(error) => error,
                    Ok(()) => {
                        return if actual.flow.status == AuthFlowStatus::Completed {
                            Ok(actual.flow)
                        } else {
                            Err(AuthProductError::BackendConflict)
                        };
                    }
                };
                if error == AuthProductError::UnknownOrExpiredFlow {
                    self.write_flow_document(&actual, snapshot.cas()).await?;
                }
                return Err(error);
            }
            if pending.canceled_at.is_some() {
                return Err(AuthProductError::Canceled);
            }
            let account = self
                .read_account(scope, input.credential_account_id)
                .await?
                .ok_or(AuthProductError::CredentialMissing)?
                .0;
            if account.provider != document.flow.provider
                || account.effective_status() != brassclaw_auth::CredentialAccountStatus::Configured
            {
                return Err(AuthProductError::CrossScopeDenied);
            }
            // A current eligible account is not the original submit result.
            return Err(AuthProductError::BackendConflict);
        };
        let source = decision.original().clone();
        let original = self.original_from_reference(scope, &source).await?;
        if original.account.id != input.credential_account_id {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let (_, flow) = self.settle_manual_reference(scope, &source, true).await?;
        flow.ok_or(AuthProductError::BackendConflict)
    }
    pub(crate) async fn abandon_manual_flow_owned(
        &self,
        scope: &AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        if self.manual_flow_document(scope, id).await?.is_none() {
            return self.abandon_manual_interaction_owned(scope, id).await;
        }
        let result = self.cancel_manual_flow_owned(scope, id).await?;
        Ok(result.is_some_and(|flow| flow.status == AuthFlowStatus::Canceled))
    }
}
