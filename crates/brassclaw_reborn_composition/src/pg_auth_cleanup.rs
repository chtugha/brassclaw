//! Native credential lifecycle custody. One actual database transaction contains
//! account revocation, selected secret closures and their retained receipts.
use super::*;
use crate::credential_cleanup_custody::{self as custody, CredentialCleanupReceipt};
use brassclaw_auth::SecretCleanupQuarantineReason as Reason;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LifecycleAccount {
    #[serde(flatten)]
    account: CredentialAccount,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    auth_lifecycle_cleanup: Vec<CredentialCleanupReceipt>,
}
impl LifecycleAccount {
    fn validate(
        &self,
        scope: &AuthProductScope,
        id: CredentialAccountId,
    ) -> Result<(), AuthProductError> {
        if self.account.id != id
            || !CredentialAccountOwnerScope::from_scope(scope).matches(&self.account)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        for receipt in &self.auth_lifecycle_cleanup {
            receipt.validate(&self.account)?;
        }
        Ok(())
    }
}
async fn save(
    transaction: &tokio_postgres::Transaction<'_>,
    document: &LifecycleAccount,
    revision: i64,
) -> Result<(), AuthProductError> {
    let data = serde_json::to_value(document).map_err(|_| AuthProductError::BackendUnavailable)?;
    let changed=transaction.execute("UPDATE brassclaw_product_auth_accounts SET data=$3,revision=revision+1,updated_at=NOW() WHERE tenant_id=$1 AND id=$2 AND revision=$4",
        &[&document.account.scope.resource.tenant_id.as_str(),&document.account.id.to_string(),&data,&revision]).await.map_err(db_err)?;
    if changed != 1 {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(())
}
impl PgAuthProductServices {
    async fn cleanup_account(
        &self,
        request: &SecretCleanupRequest,
        id: CredentialAccountId,
    ) -> Result<SecretCleanupReport, AuthProductError> {
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let transaction = client.transaction().await.map_err(db_err)?;
        // Same account-before-secret ordering as callback/manual publication.
        let row=transaction.query_opt("SELECT data,revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
            &[&request.scope.resource.tenant_id.as_str(),&id.to_string()]).await.map_err(db_err)?.ok_or(AuthProductError::CredentialMissing)?;
        let mut document: LifecycleAccount =
            serde_json::from_value(row.get(0)).map_err(|_| AuthProductError::BackendUnavailable)?;
        document.validate(&request.scope, id)?;
        let revision: i64 = row.get(1);
        let original_account = document.account.clone();
        let mut report = SecretCleanupReport::default();
        let purge = custody::apply_lifecycle(&mut document.account, request, &mut report);
        let existing = document.auth_lifecycle_cleanup.iter().rposition(|receipt| {
            receipt.extension_id == request.extension_id
                && (!receipt.confirmed()
                    || (original_account.status == CredentialAccountStatus::Revoked
                        && original_account.access_secret == receipt.original_access
                        && original_account.refresh_secret == receipt.original_refresh))
        });
        let already_complete = document.account.status == CredentialAccountStatus::Revoked
            && document.account.access_secret.is_none()
            && document.account.refresh_secret.is_none()
            && document
                .auth_lifecycle_cleanup
                .iter()
                .any(|receipt| receipt.extension_id == request.extension_id && receipt.confirmed());
        let selected = if purge && !already_complete {
            if let Some(index) = existing {
                Some(index)
            } else {
                if original_account.status == CredentialAccountStatus::Revoked
                    && original_account.access_secret.is_none()
                    && original_account.refresh_secret.is_none()
                {
                    return Err(AuthProductError::BackendConflict);
                }
                let mut receipt =
                    CredentialCleanupReceipt::new(&original_account, request.extension_id.clone())?;
                // These reads take no secret row locks before its writer advisory
                // lock. A racing generation change fails the exact close below.
                for handle in receipt.handles() {
                    let metadata = self
                        .secret_store
                        .custody_in_transaction(&transaction, &receipt.scope.resource, &handle)
                        .await
                        .map_err(|_| AuthProductError::BackendUnavailable)?;
                    let reference = if metadata.is_none() {
                        self.secret_store
                            .inspect_record_reference_in_transaction(
                                &transaction,
                                &receipt.scope.resource,
                                &handle,
                            )
                            .await
                            .map_err(|_| AuthProductError::BackendUnavailable)?
                    } else {
                        None
                    };
                    let target = custody::selected_target(&receipt, handle, metadata, reference)?;
                    receipt.targets.push(target);
                }
                document.auth_lifecycle_cleanup.push(receipt);
                Some(document.auth_lifecycle_cleanup.len() - 1)
            }
        } else {
            None
        };
        if selected.is_none() && document.account == original_account {
            return Ok(report);
        }
        document.account.updated_at = Utc::now();
        document.validate(&request.scope, id)?;
        // This original intent precedes every material mutation inside the same
        // transaction. No intermediate state can be committed independently.
        save(&transaction, &document, revision).await?;
        let original = selected.map(|index| document.auth_lifecycle_cleanup[index].clone());
        if let Some(index) = selected {
            for target in &mut document.auth_lifecycle_cleanup[index].targets {
                if target.confirmed.is_some() {
                    continue;
                }
                let actual = self
                    .secret_store
                    .close_identity_in_transaction(
                        &transaction,
                        target.scope.clone(),
                        target.handle.clone(),
                        target.expected.clone(),
                    )
                    .await
                    .map_err(|_| AuthProductError::BackendUnavailable)?
                    .ok_or(AuthProductError::BackendConflict)?;
                if actual.handle != target.handle
                    || !target.expected.accepts(&target.scope, &actual)
                {
                    return Err(AuthProductError::BackendConflict);
                }
                target.confirmed = Some(actual.reference);
                target.error = None;
            }
            if !document.auth_lifecycle_cleanup[index].confirmed() {
                return Err(AuthProductError::BackendConflict);
            }
            document.account.access_secret = None;
            document.account.access_expires_at = None;
            document.account.refresh_secret = None;
            document.validate(&request.scope, id)?;
            save(
                &transaction,
                &document,
                revision
                    .checked_add(1)
                    .ok_or(AuthProductError::BackendUnavailable)?,
            )
            .await?;
        }
        if transaction.commit().await.is_err() {
            let actual: Option<(LifecycleAccount, i64)> = self
                .get_record(
                    "brassclaw_product_auth_accounts",
                    request.scope.resource.tenant_id.as_str(),
                    &id.to_string(),
                )
                .await?;
            let (actual, _) = actual.ok_or(AuthProductError::BackendUnavailable)?;
            actual.validate(&request.scope, id)?;
            let confirmed = match original {
                Some(original) => actual
                    .auth_lifecycle_cleanup
                    .iter()
                    .any(|receipt| receipt.same_original(&original) && receipt.confirmed()),
                None => {
                    actual.account == document.account
                        && actual.auth_lifecycle_cleanup == document.auth_lifecycle_cleanup
                }
            };
            if !confirmed {
                return Err(AuthProductError::BackendUnavailable);
            }
        }
        Ok(report)
    }
}
#[async_trait]
impl SecretCleanupService for PgAuthProductServices {
    async fn cleanup_for_lifecycle(
        &self,
        request: SecretCleanupRequest,
    ) -> Result<SecretCleanupReport, AuthProductError> {
        let mut report = SecretCleanupReport::default();
        for account in self
            .accounts_for_owner(&CredentialAccountOwnerScope::from_scope(&request.scope))
            .await?
        {
            if !(account.granted_extensions.contains(&request.extension_id)
                || account.ownership == CredentialOwnership::ExtensionOwned
                    && account.owner_extension.as_ref() == Some(&request.extension_id))
            {
                continue;
            }
            match self.cleanup_account(&request, account.id).await {
                Ok(mut local) => {
                    report.revoked_accounts.append(&mut local.revoked_accounts);
                    report
                        .retained_accounts
                        .append(&mut local.retained_accounts);
                    report.removed_grants.append(&mut local.removed_grants);
                }
                Err(error) => custody::quarantine(
                    &mut report,
                    account.id,
                    if error == AuthProductError::BackendConflict {
                        Reason::TombstoneFailed
                    } else {
                        Reason::BackendUnavailable
                    },
                ),
            }
        }
        Ok(report)
    }
}
