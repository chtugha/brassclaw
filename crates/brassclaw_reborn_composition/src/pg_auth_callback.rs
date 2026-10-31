//! Existing AuthProduct flow transactions. No connection spans issuer I/O.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::{
    AuthErrorCode, OAuthCallbackAction, OAuthCallbackAdmission, OAuthCallbackCompletion,
    OAuthCallbackOutcome, OAuthCallbackReference, OAuthCallbackState, OpaqueStateHash,
};

fn target(document: &StoredAuthFlow) -> CredentialAccountId {
    document
        .auth_custody
        .oauth_callback
        .as_ref()
        .map(OAuthCallbackAction::target_account_id)
        .or_else(|| {
            document
                .flow
                .update_binding
                .as_ref()
                .map(|binding| binding.account_id)
        })
        .unwrap_or_else(|| CredentialAccountId::from_uuid(document.flow.id.as_uuid()))
}
pub(super) fn decode_scoped_flow(
    scope: &AuthProductScope,
    id: AuthFlowId,
    data: serde_json::Value,
) -> Result<StoredAuthFlow, AuthProductError> {
    let document: StoredAuthFlow =
        serde_json::from_value(data).map_err(|_| AuthProductError::BackendUnavailable)?;
    if document.flow.id != id || !scope_matches(scope, &document.flow.scope) {
        return Err(AuthProductError::CrossScopeDenied);
    }
    Ok(document)
}
pub(super) async fn locked_flow(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &AuthProductScope,
    id: AuthFlowId,
) -> Result<(StoredAuthFlow, i64), AuthProductError> {
    let row=transaction.query_opt(
        "SELECT data,revision FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
        &[&scope.resource.tenant_id.as_str(),&id.to_string()],
    ).await.map_err(db_err)?.ok_or(AuthProductError::UnknownOrExpiredFlow)?;
    let document = decode_scoped_flow(scope, id, row.get(0))?;
    Ok((document, row.get(1)))
}
pub(super) async fn locked_account(
    transaction: &tokio_postgres::Transaction<'_>,
    scope: &AuthProductScope,
    id: CredentialAccountId,
) -> Result<Option<(CredentialAccount, i64)>, AuthProductError> {
    let row=transaction.query_opt(
        "SELECT data,revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
        &[&scope.resource.tenant_id.as_str(),&id.to_string()],
    ).await.map_err(db_err)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let account: CredentialAccount =
        serde_json::from_value(row.get(0)).map_err(|_| AuthProductError::BackendUnavailable)?;
    if account.id != id || !scope_matches(scope, &account.scope) {
        return Err(AuthProductError::CrossScopeDenied);
    }
    Ok(Some((account, row.get(1))))
}
pub(super) async fn save_flow(
    transaction: &tokio_postgres::Transaction<'_>,
    document: &StoredAuthFlow,
    revision: i64,
) -> Result<(), AuthProductError> {
    let flow = &document.flow;
    let data = serde_json::to_value(document).map_err(|_| AuthProductError::BackendUnavailable)?;
    let changed=transaction.execute(
        "UPDATE brassclaw_product_auth_flows SET data=$3,revision=revision+1,updated_at=NOW() WHERE tenant_id=$1 AND id=$2 AND revision=$4",
        &[&flow.scope.resource.tenant_id.as_str(),&flow.id.to_string(),&data,&revision],
    ).await.map_err(db_err)?;
    if changed != 1 {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}
pub(super) async fn save_account(
    transaction: &tokio_postgres::Transaction<'_>,
    account: &CredentialAccount,
    revision: Option<i64>,
) -> Result<(), AuthProductError> {
    let data = serde_json::to_value(account).map_err(|_| AuthProductError::BackendUnavailable)?;
    let changed=match revision {
        Some(revision)=>transaction.execute(
            "UPDATE brassclaw_product_auth_accounts SET data=jsonb_set($3::jsonb,'{auth_lifecycle_cleanup}',COALESCE(data->'auth_lifecycle_cleanup','[]'::jsonb),true),revision=revision+1,updated_at=NOW() WHERE tenant_id=$1 AND id=$2 AND revision=$4",
            &[&account.scope.resource.tenant_id.as_str(),&account.id.to_string(),&data,&revision],
        ).await.map_err(db_err)?,
        None=>transaction.execute(
            "INSERT INTO brassclaw_product_auth_accounts (tenant_id,id,user_id,data) VALUES ($1,$2,$3,$4) ON CONFLICT (tenant_id,id) DO NOTHING",
            &[&account.scope.resource.tenant_id.as_str(),&account.id.to_string(),&account.scope.resource.user_id.as_str(),&data],
        ).await.map_err(db_err)?,
    };
    if changed != 1 {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}
impl PgAuthProductServices {
    pub(super) async fn read_callback_original(
        &self,
        scope: &AuthProductScope,
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
        scope: &AuthProductScope,
        request: OAuthCallbackClaimRequest,
    ) -> Result<OAuthCallbackAdmission, AuthProductError> {
        if !scope_matches(scope, &request.original.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let reference = request.original.reference();
        let (initial, _) = self
            .read_flow_document(scope, reference.flow_id)
            .await?
            .ok_or(AuthProductError::UnknownOrExpiredFlow)?;
        if let Some(original) = initial
            .auth_custody
            .original_state(&initial.flow, &reference)?
        {
            original.action.original.verify(&request.original)?;
            return Ok(OAuthCallbackAdmission::Original(original));
        }
        let selected = target(&initial);
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        // Use the same flow-before-account order as final publication.
        let (mut document, revision) = locked_flow(&transaction, scope, reference.flow_id).await?;
        if target(&document) != selected {
            return Err(AuthProductError::BackendConflict);
        }
        let current = locked_account(&transaction, scope, selected).await?;
        let before = document.clone();
        let result = document.auth_custody.admit(
            &mut document.flow,
            current.as_ref().map(|(account, _)| account),
            request,
            Utc::now(),
        );
        match result {
            Ok(OAuthCallbackAdmission::Dispatch(state)) => {
                save_flow(&transaction, &document, revision).await?;
                transaction.commit().await.map_err(db_err)?;
                Ok(OAuthCallbackAdmission::Dispatch(state))
            }
            Ok(OAuthCallbackAdmission::Original(state)) => {
                transaction.rollback().await.map_err(db_err)?;
                Ok(OAuthCallbackAdmission::Original(state))
            }
            Err(error) => {
                if document != before && document.flow.status == AuthFlowStatus::Expired {
                    save_flow(&transaction, &document, revision).await?;
                    transaction.commit().await.map_err(db_err)?;
                }
                Err(error)
            }
        }
    }
    async fn record_callback_result(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        state: &OpaqueStateHash,
        dispatched: OAuthCallbackAction,
        outcome: OAuthCallbackOutcome,
    ) -> Result<OAuthCallbackReference, AuthProductError> {
        let reference = dispatched.original.reference();
        if reference.flow_id != flow_id
            || !scope_matches(scope, &reference.scope)
            || !reference.opaque_state_hash.constant_time_eq(state)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        let (mut document, revision) = locked_flow(&transaction, scope, flow_id).await?;
        document
            .auth_custody
            .original_state(&document.flow, &reference)?
            .ok_or(AuthProductError::BackendUnavailable)?;
        let before = document.clone();
        let action = document
            .auth_custody
            .oauth_callback
            .as_mut()
            .ok_or(AuthProductError::BackendUnavailable)?;
        action.record_result(
            OAuthCallbackCompletion {
                dispatched,
                outcome,
            },
            Utc::now(),
        )?;
        if document.flow.status == AuthFlowStatus::CallbackReceived {
            document.flow.status = AuthFlowStatus::Completing;
            document.flow.updated_at = Utc::now();
        }
        if document != before {
            save_flow(&transaction, &document, revision).await?;
            // Returned metadata survives failed account/final-flow publication.
            transaction.commit().await.map_err(db_err)?;
        } else {
            transaction.rollback().await.map_err(db_err)?;
        }
        Ok(reference)
    }
    pub(super) async fn reconcile_callback_original(
        &self,
        scope: &AuthProductScope,
        reference: &OAuthCallbackReference,
    ) -> Result<OAuthCallbackState, AuthProductError> {
        let initial = self
            .read_callback_original(scope, reference)
            .await?
            .ok_or(AuthProductError::BackendUnavailable)?;
        if !initial.action.needs_publication() {
            return Ok(initial);
        }
        let selected = initial.action.target_account_id();
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        let (mut document, revision) = locked_flow(&transaction, scope, reference.flow_id).await?;
        let locked = document
            .auth_custody
            .original_state(&document.flow, reference)?
            .ok_or(AuthProductError::BackendUnavailable)?;
        if !locked.action.same_intent(&initial.action) {
            return Err(AuthProductError::BackendConflict);
        }
        // Keep the same flow -> account -> requirement-head order as manual
        // publication and explicit account selection. Issuer I/O is finished.
        let current = locked_account(&transaction, scope, selected).await?;
        let before = document.clone();
        let action = document
            .auth_custody
            .oauth_callback
            .as_mut()
            .ok_or(AuthProductError::BackendUnavailable)?;
        let account = action.publish(
            &mut document.flow,
            current.as_ref().map(|(account, _)| account),
            Utc::now(),
        )?;
        let result = OAuthCallbackState {
            flow: document.flow.clone(),
            action: action.clone(),
        };
        if let Some(account) = account {
            self.publish_requirement_in_transaction(&transaction, &mut document, &account)
                .await?;
            save_account(
                &transaction,
                &account,
                current.as_ref().map(|(_, revision)| *revision),
            )
            .await?;
        }
        if document != before {
            save_flow(&transaction, &document, revision).await?;
            // Exactly one final account/flow/receipt transaction.
            transaction.commit().await.map_err(db_err)?;
        } else {
            transaction.rollback().await.map_err(db_err)?;
        }
        Ok(result)
    }
    async fn finish_callback_without_exchange(
        &self,
        scope: &AuthProductScope,
        flow_id: AuthFlowId,
        state: &OpaqueStateHash,
        error: AuthErrorCode,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        let (mut document, revision) = locked_flow(&transaction, scope, flow_id).await?;
        let prepared = prepare_callback_flow(&mut document.flow, scope, state, Utc::now());
        if let Err(error) = prepared {
            if document.flow.status == AuthFlowStatus::Expired {
                save_flow(&transaction, &document, revision).await?;
                transaction.commit().await.map_err(db_err)?;
            }
            return Err(error);
        }
        if document.auth_custody.oauth_callback.is_some()
            || document.fs_callback_publication.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        if !matches!(
            document.flow.challenge,
            Some(AuthChallenge::OAuthUrl { .. })
        ) {
            return Err(AuthProductError::MalformedCallback);
        }
        document.flow.status = AuthFlowStatus::Failed;
        document.flow.error = Some(error);
        document.flow.updated_at = Utc::now();
        save_flow(&transaction, &document, revision).await?;
        transaction.commit().await.map_err(db_err)?;
        Ok(document.flow)
    }
    pub(super) async fn complete_callback_owned(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        match input.outcome {
            ProviderCallbackOutcome::Denied => {
                if input.dispatched.is_some() {
                    return Err(AuthProductError::MalformedCallback);
                }
                self.finish_callback_without_exchange(
                    scope,
                    input.flow_id,
                    &input.opaque_state_hash,
                    AuthErrorCode::ProviderDenied,
                )
                .await?;
                Err(AuthProductError::ProviderDenied)
            }
            ProviderCallbackOutcome::Authorized { exchange } => {
                let dispatched = input
                    .dispatched
                    .ok_or(AuthProductError::BackendUnavailable)?;
                let reference = self
                    .record_callback_result(
                        scope,
                        input.flow_id,
                        &input.opaque_state_hash,
                        dispatched,
                        OAuthCallbackOutcome::Returned(exchange),
                    )
                    .await?;
                self.reconcile_callback_original(scope, &reference)
                    .await?
                    .completed_flow()
            }
        }
    }
    pub(super) async fn fail_callback_owned(
        &self,
        scope: &AuthProductScope,
        input: OAuthCallbackFailureInput,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if let Some(dispatched) = input.dispatched {
            let reference = self
                .record_callback_result(
                    scope,
                    input.flow_id,
                    &input.opaque_state_hash,
                    dispatched,
                    OAuthCallbackOutcome::Unknown(input.error),
                )
                .await?;
            return Ok(self
                .reconcile_callback_original(scope, &reference)
                .await?
                .flow);
        }
        self.finish_callback_without_exchange(
            scope,
            input.flow_id,
            &input.opaque_state_hash,
            input.error,
        )
        .await
    }
}
