//! The existing native auth owner publishes one original metadata pair.
//! No raw token, provider call or continuation dispatch belongs in setup.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use crate::manual_token_flow::{
    build_original_manual_setup_flow, validate_original_manual_setup_flow,
};

fn original_request(pending: &StoredManualTokenInteraction) -> ManualTokenSetupRequest {
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

impl PgAuthProductServices {
    async fn read_native_manual_setup_original(
        &self,
        identity: ManualTokenSetupIdentity,
        scope: &AuthProductScope,
        requested: Option<&ManualTokenSetupRequest>,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        let client = self.pool.get().await.map_err(pool_err)?;
        // One statement snapshot cannot observe half of an atomic creation or
        // publication by two separate read transactions.
        let row = client
            .query_one(
                "SELECT
                (SELECT data FROM brassclaw_product_auth_interactions WHERE tenant_id=$1 AND id=$2),
                (SELECT data FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$3)",
                &[
                    &scope.resource.tenant_id.as_str(),
                    &identity.interaction_id.to_string(),
                    &identity.flow_id.to_string(),
                ],
            )
            .await
            .map_err(db_err)?;
        let pending: Option<serde_json::Value> = row.get(0);
        let flow: Option<serde_json::Value> = row.get(1);
        let (pending, flow) = match (pending, flow) {
            (None, None) => return Ok(None),
            (Some(pending), Some(flow)) => (pending, flow),
            _ => return Err(AuthProductError::BackendConflict),
        };
        let pending: StoredManualTokenInteraction =
            serde_json::from_value(pending).map_err(|_| AuthProductError::BackendUnavailable)?;
        let document: StoredAuthFlow =
            serde_json::from_value(flow).map_err(|_| AuthProductError::BackendUnavailable)?;
        if !scope_matches(scope, &pending.scope) || !scope_matches(scope, &document.flow.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if pending.id != identity.interaction_id
            || pending.setup_flow_id != Some(identity.flow_id)
            || document.auth_custody.update_account_snapshot != pending.update_account_snapshot
            || document.auth_custody.requirement_setup != pending.requirement_setup
            || document.auth_custody.oauth_callback.is_some()
            || document.fs_callback_publication.is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        let original = original_request(&pending);
        validate_original_manual_setup_flow(
            identity,
            &original,
            requested.unwrap_or(&original),
            &document.flow,
        )?;
        // This also validates the retained binding/snapshot combination without
        // replacing it with today's account.
        StoredAuthFlow::new(
            document.flow.clone(),
            pending.update_account_snapshot.clone(),
        )?;
        if pending.canceled_at.is_some() && document.flow.status != AuthFlowStatus::Canceled {
            return Err(AuthProductError::Canceled);
        }
        if document.flow.status == AuthFlowStatus::AwaitingUser && pending.consumed_at.is_some() {
            return Err(AuthProductError::BackendConflict);
        }
        if document.flow.status == AuthFlowStatus::Completed && pending.consumed_at.is_none() {
            return Err(AuthProductError::BackendConflict);
        }
        if requested.is_some()
            && document.flow.status == AuthFlowStatus::AwaitingUser
            && document.flow.expires_at <= Utc::now()
        {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        Ok(Some(document.flow))
    }

    pub(super) async fn recover_native_manual_setup_original(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<Option<AuthFlowRecord>, AuthProductError> {
        self.read_native_manual_setup_original(identity, scope, None)
            .await
    }

    pub(super) async fn request_native_manual_setup_original(
        &self,
        identity: ManualTokenSetupIdentity,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        if let Some(original) = self
            .read_native_manual_setup_original(identity, &request.scope, Some(&request))
            .await?
        {
            return Ok(original);
        }
        let created_at = Utc::now();
        let flow = build_original_manual_setup_flow(identity, &request, created_at)?;
        let snapshot = if let Some(binding) = &request.update_binding {
            let account = self
                .read_account(&request.scope, binding.account_id)
                .await?
                .ok_or(AuthProductError::CredentialMissing)?
                .0;
            validate_manual_token_update_binding(&account, &request, binding)?;
            Some(account)
        } else {
            None
        };
        let document = StoredAuthFlow::new(flow.clone(), snapshot.clone())?
            .with_requirement_setup(request.requirement_setup.clone())?;
        let pending = StoredManualTokenInteraction {
            requirement_setup: request.requirement_setup.clone(),
            id: identity.interaction_id,
            setup_flow_id: Some(identity.flow_id),
            scope: request.scope.clone(),
            provider: request.provider.clone(),
            label: request.label.clone(),
            continuation: request.continuation.clone(),
            update_binding: request.update_binding.clone(),
            update_account_snapshot: snapshot.clone(),
            expires_at: request.expires_at,
            consumed_at: None,
            canceled_at: None,
        };
        let flow_bytes =
            serde_json::to_value(&document).map_err(|_| AuthProductError::BackendUnavailable)?;
        let interaction_bytes =
            serde_json::to_value(&pending).map_err(|_| AuthProductError::BackendUnavailable)?;
        let write: Result<(), AuthProductError> = async {
            let mut client = self.pool.get().await.map_err(pool_err)?;
            let transaction = client.transaction().await.map_err(db_err)?;
            // Same lock order as publication: flow, interaction, account. Read
            // the candidate account before entering; verify it under lock last.
            let inserted = transaction
                .execute(
                    "INSERT INTO brassclaw_product_auth_flows (tenant_id,id,user_id,data)
                 VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",
                    &[
                        &request.scope.resource.tenant_id.as_str(),
                        &identity.flow_id.to_string(),
                        &request.scope.resource.user_id.as_str(),
                        &flow_bytes,
                    ],
                )
                .await
                .map_err(db_err)?;
            if inserted != 1 {
                return Err(AuthProductError::BackendConflict);
            }
            let inserted = transaction
                .execute(
                    "INSERT INTO brassclaw_product_auth_interactions (tenant_id,id,user_id,data)
                 VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",
                    &[
                        &request.scope.resource.tenant_id.as_str(),
                        &identity.interaction_id.to_string(),
                        &request.scope.resource.user_id.as_str(),
                        &interaction_bytes,
                    ],
                )
                .await
                .map_err(db_err)?;
            if inserted != 1 {
                return Err(AuthProductError::BackendConflict);
            }
            if let Some(expected) = &snapshot {
                let selected = callback::locked_account(&transaction, &request.scope, expected.id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?
                    .0;
                if &selected != expected {
                    return Err(AuthProductError::BackendConflict);
                }
            }
            if request.expires_at <= Utc::now() {
                return Err(AuthProductError::UnknownOrExpiredFlow);
            }
            transaction.commit().await.map_err(db_err)
        }
        .await;
        match write {
            Ok(()) => Ok(flow),
            Err(write_error) => {
                // Original conflict/commit loss never starts a replacement or
                // deletes either record. The winner's first expiry stays fixed.
                self.read_native_manual_setup_original(identity, &request.scope, Some(&request))
                    .await?
                    .ok_or(write_error)
            }
        }
    }
}
