//! Original setup uses the existing interaction intent and flow projection.
//! No multi-record filesystem transaction or secret publication is invented.
use super::interactions::StoredManualTokenInteraction;
use super::manual::ManualDecision;
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use crate::manual_token_flow::{
    build_original_manual_setup_flow, validate_original_manual_setup_flow,
};
use brassclaw_auth::{
    AuthInteractionId, AuthProductScope, ManualTokenSetupIdentity, ManualTokenSetupRequest,
};

fn definition(pending: &StoredManualTokenInteraction) -> ManualTokenSetupRequest {
    ManualTokenSetupRequest {
        requirement_setup: pending.requirement_setup.clone(),
        scope: pending.scope.clone(),
        provider: pending.provider.clone(),
        label: pending.label.clone(),
        continuation: pending.continuation.clone(),
        update_binding: pending.update_binding.clone(),
        expires_at: pending.expires_at,
    }
}
fn identity(
    pending: &StoredManualTokenInteraction,
) -> Result<ManualTokenSetupIdentity, AuthProductError> {
    if pending.setup_created_at.is_none() {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(ManualTokenSetupIdentity {
        interaction_id: pending.id,
        flow_id: pending
            .setup_flow_id
            .ok_or(AuthProductError::BackendConflict)?,
    })
}
fn validate_projection(
    pending: &StoredManualTokenInteraction,
    document: &StoredAuthFlow,
) -> Result<(), AuthProductError> {
    let original = definition(pending);
    validate_original_manual_setup_flow(identity(pending)?, &original, &original, &document.flow)?;
    if Some(document.flow.created_at) != pending.setup_created_at
        || document.auth_custody.update_account_snapshot != pending.update_account_snapshot
        || document.auth_custody.requirement_setup != pending.requirement_setup
        || document.auth_custody.oauth_callback.is_some()
        || document.fs_callback_publication.is_some()
    {
        return Err(AuthProductError::BackendConflict);
    }
    StoredAuthFlow::new(
        document.flow.clone(),
        pending.update_account_snapshot.clone(),
    )?;
    Ok(())
}

impl<F: RootFilesystem + 'static> FilesystemAuthProductServices<F> {
    async fn original_setup_interaction(
        &self,
        scope: &AuthProductScope,
        id: AuthInteractionId,
    ) -> Result<Option<StoredManualTokenInteraction>, AuthProductError> {
        let row: Option<(StoredManualTokenInteraction, AuthRecordSnapshot)> = self
            .read_record(&scope.resource, &super::paths::interaction_path(scope, id))
            .await?;
        if let Some((pending, _)) = &row
            && (pending.id != id || !scope_matches(scope, &pending.scope))
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(row.map(|(pending, _)| pending))
    }

    async fn finish_original_setup_projection(
        &self,
        pending: &StoredManualTokenInteraction,
        mut document: StoredAuthFlow,
        snapshot: AuthRecordSnapshot,
        allow_expired_metadata: bool,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        validate_projection(pending, &document)?;
        if pending.consumed_at.is_some() || pending.publication.is_some() {
            if matches!(pending.publication, Some(ManualDecision::Published { .. }))
                && pending.consumed_at.is_some()
                && pending.canceled_at.is_none()
                && document.flow.status == brassclaw_auth::AuthFlowStatus::Completed
            {
                return Ok(document.flow);
            }
            if !matches!(pending.publication, Some(ManualDecision::Superseded { .. }))
                || pending.canceled_at.is_none()
            {
                return Err(AuthProductError::BackendConflict);
            }
        }
        if document.flow.status == brassclaw_auth::AuthFlowStatus::Completed {
            return Err(AuthProductError::BackendConflict);
        }
        if is_terminal_status(document.flow.status) {
            return Ok(document.flow);
        }
        let Some(canceled_at) = pending.canceled_at else {
            if !allow_expired_metadata && pending.expires_at <= Utc::now() {
                return Err(AuthProductError::UnknownOrExpiredFlow);
            }
            return Ok(document.flow);
        };
        document.flow.status = brassclaw_auth::AuthFlowStatus::Canceled;
        document.flow.error = Some(brassclaw_auth::AuthErrorCode::Canceled);
        document.flow.updated_at = canceled_at;
        if let Err(error) = self.write_flow_document(&document, snapshot.cas()).await {
            let current = self
                .read_flow_document(&pending.scope, document.flow.id)
                .await?
                .ok_or(error.clone())?
                .0;
            validate_projection(pending, &current)?;
            if is_terminal_status(current.flow.status)
                && current.flow.status != brassclaw_auth::AuthFlowStatus::Completed
            {
                return Ok(current.flow);
            }
            return Err(error);
        }
        Ok(document.flow)
    }

    pub(in crate::product_auth_durable) async fn reconcile_original_fs_setup_projection(
        &self,
        pending: &StoredManualTokenInteraction,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.reconcile_original_fs_setup_metadata(pending, false)
            .await
    }

    async fn reconcile_original_fs_setup_metadata(
        &self,
        pending: &StoredManualTokenInteraction,
        allow_expired_metadata: bool,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let id = identity(pending)?;
        if let Some((document, snapshot)) =
            self.read_flow_document(&pending.scope, id.flow_id).await?
        {
            return self
                .finish_original_setup_projection(
                    pending,
                    document,
                    snapshot,
                    allow_expired_metadata,
                )
                .await;
        }
        if pending.consumed_at.is_some() || pending.publication.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        let original = definition(pending);
        let mut flow = build_original_manual_setup_flow(
            id,
            &original,
            pending
                .setup_created_at
                .ok_or(AuthProductError::BackendConflict)?,
        )?;
        if let Some(canceled_at) = pending.canceled_at {
            flow.status = brassclaw_auth::AuthFlowStatus::Canceled;
            flow.error = Some(brassclaw_auth::AuthErrorCode::Canceled);
            flow.updated_at = canceled_at;
        } else if !allow_expired_metadata && pending.expires_at <= Utc::now() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let document = StoredAuthFlow::new(flow, pending.update_account_snapshot.clone())?
            .with_requirement_setup(pending.requirement_setup.clone())?;
        let write = self
            .write_flow_document(&document, CasExpectation::Absent)
            .await;
        let (actual, snapshot) = self
            .read_flow_document(&pending.scope, id.flow_id)
            .await?
            .ok_or_else(|| write.err().unwrap_or(AuthProductError::BackendConflict))?;
        self.finish_original_setup_projection(pending, actual, snapshot, allow_expired_metadata)
            .await
    }

    pub(crate) async fn request_fs_manual_setup_original(
        &self,
        id: ManualTokenSetupIdentity,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let pending = if let Some(pending) = self
            .original_setup_interaction(&request.scope, id.interaction_id)
            .await?
        {
            pending
        } else {
            // Never attach a new interaction to an unrelated/legacy flow ID.
            if self
                .read_flow_document(&request.scope, id.flow_id)
                .await?
                .is_some()
            {
                return Err(AuthProductError::BackendConflict);
            }
            let now = Utc::now();
            build_original_manual_setup_flow(id, &request, now)?;
            let account = if let Some(binding) = &request.update_binding {
                let account = self
                    .read_account(&request.scope, binding.account_id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?
                    .0;
                super::domain::validate_manual_token_update_binding(&account, &request, binding)?;
                Some(account)
            } else {
                None
            };
            let candidate = StoredManualTokenInteraction {
                requirement_setup: request.requirement_setup.clone(),
                id: id.interaction_id,
                scope: request.scope.clone(),
                provider: request.provider.clone(),
                label: request.label.clone(),
                continuation: request.continuation.clone(),
                update_binding: request.update_binding.clone(),
                update_account_snapshot: account,
                publication: None,
                canceled_at: None,
                consumed_at: None,
                expires_at: request.expires_at,
                setup_flow_id: Some(id.flow_id),
                setup_created_at: Some(now),
            };
            match self
                .write_record(
                    &request.scope.resource,
                    &super::paths::interaction_path(&request.scope, id.interaction_id),
                    &candidate,
                    CasExpectation::Absent,
                )
                .await
            {
                Ok(_) => candidate,
                Err(error) => self
                    .original_setup_interaction(&request.scope, id.interaction_id)
                    .await?
                    .ok_or(error)?,
            }
        };
        self.finish_original_setup_readback(id, &request, pending, false)
            .await
    }

    pub(crate) async fn recover_fs_manual_setup_original(
        &self,
        scope: &AuthProductScope,
        id: ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let Some(pending) = self
            .original_setup_interaction(scope, id.interaction_id)
            .await?
        else {
            if self.read_flow_document(scope, id.flow_id).await?.is_some() {
                return Err(AuthProductError::BackendConflict);
            }
            return Ok(None);
        };
        let request = definition(&pending);
        self.finish_original_setup_readback(id, &request, pending, true)
            .await
            .map(Some)
    }

    async fn finish_original_setup_readback(
        &self,
        id: ManualTokenSetupIdentity,
        request: &ManualTokenSetupRequest,
        pending: StoredManualTokenInteraction,
        allow_expired_metadata: bool,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if identity(&pending)? != id {
            return Err(AuthProductError::BackendConflict);
        }
        let original = definition(&pending);
        let expected = build_original_manual_setup_flow(
            id,
            &original,
            pending
                .setup_created_at
                .ok_or(AuthProductError::BackendConflict)?,
        )?;
        validate_original_manual_setup_flow(id, &original, request, &expected)?;
        self.reconcile_original_fs_setup_metadata(&pending, allow_expired_metadata)
            .await?;
        // Check any competing cancellation/publication instead of returning the
        // stale initial candidate. No latest account or replacement ID is used.
        let current = self
            .original_setup_interaction(&request.scope, id.interaction_id)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if identity(&current)? != id
            || definition(&current) != original
            || current.setup_created_at != pending.setup_created_at
            || current.update_account_snapshot != pending.update_account_snapshot
        {
            return Err(AuthProductError::BackendConflict);
        }
        self.reconcile_original_fs_setup_metadata(&current, allow_expired_metadata)
            .await
    }
}
