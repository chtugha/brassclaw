//! Refresh records extend the selected AuthProduct backend; they are not another
//! provider registry, credential store or token executor. No transaction crosses HTTP.
use super::*;
use brassclaw_auth::{
    CredentialRefreshAction, CredentialRefreshAdmission, CredentialRefreshCompletion,
    CredentialRefreshOriginal, CredentialRefreshPrepared,
};

type RefreshAdmission = fn(
    &mut CredentialAccount,
    &[CredentialRefreshAction],
    CredentialRefreshPrepared,
    brassclaw_auth::Timestamp,
) -> Result<CredentialRefreshAdmission, AuthProductError>;

fn decode_action(row: &tokio_postgres::Row) -> Result<CredentialRefreshAction, AuthProductError> {
    serde_json::from_value(row.get::<_, serde_json::Value>(0))
        .map_err(|_| AuthProductError::BackendUnavailable)
}
fn key(original: &CredentialRefreshOriginal) -> Result<(String, String, String), AuthProductError> {
    Ok((
        original.scope.resource.tenant_id.as_str().to_owned(),
        original.scope_ref()?,
        original.id.to_string(),
    ))
}
fn blocks(action: &CredentialRefreshAction) -> bool {
    action
        .expected_account
        .refresh_secret
        .as_ref()
        .is_some_and(|handle| action.blocks_credential(handle))
}
async fn account_for_update(
    transaction: &tokio_postgres::Transaction<'_>,
    original: &CredentialRefreshOriginal,
) -> Result<(CredentialAccount, i64), AuthProductError> {
    let row=transaction.query_opt(
        "SELECT data,revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
        &[&original.scope.resource.tenant_id.as_str(),&original.account_id.to_string()],
    ).await.map_err(db_err)?.ok_or(AuthProductError::CredentialMissing)?;
    let account: CredentialAccount =
        serde_json::from_value(row.get(0)).map_err(|_| AuthProductError::BackendUnavailable)?;
    if !scope_matches(&original.scope, &account.scope) || account.id != original.account_id {
        return Err(AuthProductError::CrossScopeDenied);
    }
    Ok((account, row.get(1)))
}
async fn update_account(
    transaction: &tokio_postgres::Transaction<'_>,
    account: &CredentialAccount,
    revision: i64,
) -> Result<(), AuthProductError> {
    let json = serde_json::to_value(account).map_err(|_| AuthProductError::BackendUnavailable)?;
    let rows=transaction.execute(
        "UPDATE brassclaw_product_auth_accounts SET data=jsonb_set($3::jsonb,'{auth_lifecycle_cleanup}',COALESCE(data->'auth_lifecycle_cleanup','[]'::jsonb),true),revision=revision+1,updated_at=NOW() WHERE tenant_id=$1 AND id=$2 AND revision=$4",
        &[&account.scope.resource.tenant_id.as_str(),&account.id.to_string(),&json,&revision],
    ).await.map_err(db_err)?;
    if rows != 1 {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}
impl PgAuthProductServices {
    pub(super) async fn read_refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<CredentialRefreshAction>, AuthProductError> {
        let original = CredentialRefreshOriginal::from(request);
        let (tenant, scope_ref, id) = key(&original)?;
        let client = self.pool.get().await.map_err(pool_err)?;
        let row=client.query_opt(
            "SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3",
            &[&tenant,&scope_ref,&id],
        ).await.map_err(db_err)?;
        let action = row.as_ref().map(decode_action).transpose()?;
        if let Some(action) = &action {
            action.original.verify(request)?;
        }
        Ok(action)
    }
    pub(super) async fn begin_refresh_action(
        &self,
        prepared: CredentialRefreshPrepared,
    ) -> Result<CredentialRefreshAdmission, AuthProductError> {
        let original = prepared.original.clone();
        // A repeat does not need the account publication lock. A racing first
        // admission is still serialized and checked again in the transaction.
        if let Some(action) = self.read_refresh_action(&original.request()).await? {
            return Ok(CredentialRefreshAdmission::Original(action));
        }
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        let (admission, changed) = self
            .begin_refresh_in_transaction(&transaction, prepared, CredentialRefreshAction::begin)
            .await?;
        if changed {
            transaction.commit().await.map_err(db_err)?;
        } else {
            transaction.rollback().await.map_err(db_err)?;
        }
        Ok(admission)
    }

    /// Native metadata admission in the EXISTING refresh journal. The caller
    /// must authenticate its original instance binding under the same account ->
    /// action -> encrypted-head lock order and COMMIT before actual issuer I/O.
    /// Caller labels, this metadata or a returned Dispatch are not permission
    /// before that winning acknowledged commit. No HTTP occurs in this helper.
    #[allow(dead_code)] // PRIVATE036: used by the native host refresh caller once login/publication wiring lands.
    pub(crate) async fn begin_host_instance_refresh_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        prepared: CredentialRefreshPrepared,
    ) -> Result<CredentialRefreshAdmission, AuthProductError> {
        self.begin_refresh_in_transaction(
            transaction,
            prepared,
            CredentialRefreshAction::begin_host_instance,
        )
        .await
        .map(|(admission, _)| admission)
    }

    async fn begin_refresh_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        prepared: CredentialRefreshPrepared,
        admit: RefreshAdmission,
    ) -> Result<(CredentialRefreshAdmission, bool), AuthProductError> {
        let original = prepared.original.clone();
        let (tenant, scope_ref, id) = key(&original)?;
        let (mut account, account_revision) = account_for_update(transaction, &original).await?;
        if let Some(row)=transaction.query_opt(
            "SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 FOR UPDATE",
            &[&tenant,&scope_ref,&id],
        ).await.map_err(db_err)? {
            let action=decode_action(&row)?;
            if action.original!=original {return Err(AuthProductError::BackendConflict);}
            return Ok((CredentialRefreshAdmission::Original(action), false));
        }
        let mut existing = Vec::new();
        if let Some(handle) = &account.refresh_secret {
            let rows=transaction.query(
                "SELECT data FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND account_id=$2 AND refresh_handle=$3 AND blocks_refresh",
                &[&tenant,&original.account_id.to_string(),&handle.as_str()],
            ).await.map_err(db_err)?;
            for row in &rows {
                existing.push(decode_action(row)?);
            }
        }
        let before = account.clone();
        let admission = admit(&mut account, &existing, prepared, Utc::now())?;
        let action = match &admission {
            CredentialRefreshAdmission::Dispatch(action)
            | CredentialRefreshAdmission::Original(action) => action,
        };
        if let CredentialRefreshAdmission::Dispatch(action) = &admission {
            let source = action
                .source
                .as_ref()
                .ok_or(AuthProductError::BackendUnavailable)?;
            let selected = self
                .secret_store
                .inspect_record_reference_in_transaction(transaction, &source.scope, &source.handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            if selected.as_deref() != Some(source.record_reference.as_str()) {
                return Err(AuthProductError::BackendConflict);
            }
        }

        let json =
            serde_json::to_value(action).map_err(|_| AuthProductError::BackendUnavailable)?;
        let refresh = action
            .expected_account
            .refresh_secret
            .as_ref()
            .map(|h| h.as_str().to_owned());
        let inserted=transaction.execute(
            "INSERT INTO brassclaw_product_auth_refreshes (tenant_id,scope_ref,id,account_id,refresh_handle,blocks_refresh,data) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (tenant_id,scope_ref,id) DO NOTHING",
            &[&tenant,&scope_ref,&id,&original.account_id.to_string(),&refresh,&blocks(action),&json],
        ).await.map_err(db_err)?;
        if inserted != 1 {
            return Err(AuthProductError::BackendConflict);
        }
        if before != account {
            update_account(transaction, &account, account_revision).await?;
        }
        // The owning wrapper must confirm its COMMIT before returning Dispatch.
        // A native caller verifies its exact provider binding in this SAME tx.
        Ok((admission, true))
    }

    async fn record_refresh_result(
        &self,
        completion: CredentialRefreshCompletion,
    ) -> Result<CredentialRefreshAction, AuthProductError> {
        let (tenant, scope_ref, id) = key(&completion.dispatched.original)?;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        let row=transaction.query_opt(
            "SELECT data,revision FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 FOR UPDATE",
            &[&tenant,&scope_ref,&id],
        ).await.map_err(db_err)?.ok_or(AuthProductError::BackendConflict)?;
        let mut action = decode_action(&row)?;
        let revision: i64 = row.get(1);
        let before = action.clone();
        action.record_result(completion, Utc::now())?;
        if before == action {
            transaction.rollback().await.map_err(db_err)?;
            return Ok(action);
        }
        let json =
            serde_json::to_value(&action).map_err(|_| AuthProductError::BackendUnavailable)?;
        let rows=transaction.execute(
            "UPDATE brassclaw_product_auth_refreshes SET data=$4,blocks_refresh=$5,revision=revision+1 WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 AND revision=$6",
            &[&tenant,&scope_ref,&id,&json,&blocks(&action),&revision],
        ).await.map_err(db_err)?;
        if rows != 1 {
            return Err(AuthProductError::BackendConflict);
        }
        // The exact response survives a later failed account publication.
        transaction.commit().await.map_err(db_err)?;
        Ok(action)
    }
    pub(super) async fn reconcile_refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<CredentialRefreshAction>, AuthProductError> {
        let original_action = self.read_refresh_action(request).await?;
        if original_action
            .as_ref()
            .is_none_or(|action| !action.needs_publication())
        {
            return Ok(original_action);
        }
        let original = CredentialRefreshOriginal::from(request);
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        let (action, changed) = self
            .reconcile_refresh_action_in_transaction(&transaction, &original)
            .await?;
        if changed {
            transaction.commit().await.map_err(db_err)?;
        } else {
            transaction.rollback().await.map_err(db_err)?;
        }
        Ok(action)
    }

    /// Existing account -> action publication in a caller-owned transaction.
    /// The trusted instance adapter can add its exact descriptor-head CAS before
    /// committing this SAME transaction. This grants no refresh/instance authority
    /// and makes no HTTP request; callers must authenticate the selected subject.
    pub(crate) async fn reconcile_refresh_action_in_transaction(
        &self,
        transaction: &tokio_postgres::Transaction<'_>,
        original: &CredentialRefreshOriginal,
    ) -> Result<(Option<CredentialRefreshAction>, bool), AuthProductError> {
        let (tenant, scope_ref, id) = key(original)?;
        // Account then action is the same order as admission. The result-only
        // writer locks no account and cannot form the inverse lock order.
        let (mut account, account_revision) = account_for_update(transaction, original).await?;
        let Some(row)=transaction.query_opt(
            "SELECT data,revision FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 FOR UPDATE",
            &[&tenant,&scope_ref,&id],
        ).await.map_err(db_err)? else {
            return Ok((None, false));
        };
        let mut action = decode_action(&row)?;
        if action.original != *original {
            return Err(AuthProductError::BackendConflict);
        }
        let action_revision: i64 = row.get(1);
        let before_action = action.clone();
        let before_account = account.clone();
        action.publish(&mut account, Utc::now())?;
        if action == before_action {
            return Ok((Some(action), false));
        }
        if account != before_account {
            update_account(transaction, &account, account_revision).await?;
        }
        let json =
            serde_json::to_value(&action).map_err(|_| AuthProductError::BackendUnavailable)?;
        let rows=transaction.execute(
            "UPDATE brassclaw_product_auth_refreshes SET data=$4,blocks_refresh=$5,revision=revision+1 WHERE tenant_id=$1 AND scope_ref=$2 AND id=$3 AND revision=$6",
            &[&tenant,&scope_ref,&id,&json,&blocks(&action),&action_revision],
        ).await.map_err(db_err)?;
        if rows != 1 {
            return Err(AuthProductError::BackendConflict);
        }
        // Final account and its applied/superseded receipt are one transaction.
        Ok((Some(action), true))
    }
    pub(super) async fn complete_refresh_action(
        &self,
        completion: CredentialRefreshCompletion,
    ) -> Result<CredentialRefreshAction, AuthProductError> {
        let request = completion.dispatched.original.request();
        self.record_refresh_result(completion).await?;
        self.reconcile_refresh_action(&request)
            .await?
            .ok_or(AuthProductError::BackendUnavailable)
    }
}
