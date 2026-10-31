use super::*;
use crate::credential_cleanup_custody::{self as custody, CredentialCleanupReceipt};
use brassclaw_auth::{
    CredentialOwnership, SecretCleanupQuarantineReason as Reason, SecretCleanupReport,
    SecretCleanupRequest, SecretCleanupService,
};

impl<F: RootFilesystem> FilesystemAuthProductServices<F> {
    async fn cleanup_account(
        &self,
        request: &SecretCleanupRequest,
        account_scope: &brassclaw_auth::AuthProductScope,
        id: CredentialAccountId,
    ) -> Result<SecretCleanupReport, AuthProductError> {
        let lock = self.lock_for(format!("account:{id}"));
        let _guard = lock.lock().await;
        let (mut document, mut version) = self
            .read_account_document(account_scope, id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if !CredentialAccountOwnerScope::from_scope(&request.scope).matches(&document.account) {
            return Err(AuthProductError::CrossScopeDenied);
        }
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
        let mut added = false;
        let selected = if purge && !already_complete {
            if let Some(index) = existing {
                Some(index)
            } else {
                // Historical revoked rows without handles cannot prove what an
                // older best-effort cleanup lost. Never invent a receipt.
                if original_account.status == CredentialAccountStatus::Revoked
                    && original_account.access_secret.is_none()
                    && original_account.refresh_secret.is_none()
                {
                    return Err(AuthProductError::BackendConflict);
                }
                let mut receipt =
                    CredentialCleanupReceipt::new(&original_account, request.extension_id.clone())?;
                custody::capture(self.secret_store.as_ref(), &mut receipt).await?;
                document.auth_lifecycle_cleanup.push(receipt);
                added = true;
                Some(document.auth_lifecycle_cleanup.len() - 1)
            }
        } else {
            None
        };
        if added || document.account != original_account {
            document.account.updated_at = Utc::now();
            match self.write_account_document(&document, version.cas()).await {
                Ok(after) => version = after,
                Err(error) => {
                    let Some((actual, after)) =
                        self.read_account_document(account_scope, id).await?
                    else {
                        return Err(error);
                    };
                    if actual.account != document.account
                        || !document
                            .auth_lifecycle_cleanup
                            .iter()
                            .all(|wanted| actual.auth_lifecycle_cleanup.contains(wanted))
                    {
                        return Err(error);
                    }
                    document = actual;
                    version = after;
                }
            }
        }
        let Some(index) = selected else {
            return Ok(report);
        };
        let original = document.auth_lifecycle_cleanup[index].clone();
        for position in 0..original.targets.len() {
            if document.auth_lifecycle_cleanup[index].targets[position]
                .confirmed
                .is_some()
            {
                continue;
            }
            let mut result = document.auth_lifecycle_cleanup[index].targets[position].clone();
            // The exact selection is durable beside the revoked account first.
            custody::close_target(self.secret_store.as_ref(), &mut result).await;
            document.auth_lifecycle_cleanup[index].targets[position] = result.clone();
            match self.write_account_document(&document, version.cas()).await {
                Ok(after) => version = after,
                Err(error) => {
                    let Some((actual, after)) =
                        self.read_account_document(account_scope, id).await?
                    else {
                        return Err(error);
                    };
                    let Some(actual_index) = actual
                        .auth_lifecycle_cleanup
                        .iter()
                        .position(|receipt| receipt.same_original(&original))
                    else {
                        return Err(error);
                    };
                    let observed = &actual.auth_lifecycle_cleanup[actual_index].targets[position];
                    if observed.handle != result.handle
                        || observed.scope != result.scope
                        || observed.expected != result.expected
                        || (observed.confirmed.is_none() && observed != &result)
                    {
                        return Err(error);
                    }
                    document = actual;
                    version = after;
                    if actual_index != index {
                        return Err(AuthProductError::BackendConflict);
                    }
                }
            }
        }
        if !document.auth_lifecycle_cleanup[index].confirmed() {
            let reason = document.auth_lifecycle_cleanup[index]
                .targets
                .iter()
                .find_map(|target| target.error)
                .unwrap_or(Reason::TombstoneFailed);
            custody::quarantine(&mut report, id, reason);
            return Ok(report);
        }
        // Retained receipts keep original references after the material is gone.
        document.account.access_secret = None;
        document.account.access_expires_at = None;
        document.account.refresh_secret = None;
        document.account.updated_at = Utc::now();
        if let Err(error) = self.write_account_document(&document, version.cas()).await {
            let Some((actual, _)) = self.read_account_document(account_scope, id).await? else {
                return Err(error);
            };
            if !actual
                .auth_lifecycle_cleanup
                .iter()
                .any(|receipt| receipt.same_original(&original) && receipt.confirmed())
            {
                return Err(error);
            }
            // Never clear a later confirmed publication on retry.
            if actual.account.status == CredentialAccountStatus::Revoked
                && (actual.account.access_secret.is_some()
                    || actual.account.refresh_secret.is_some())
            {
                return Err(error);
            }
        }
        Ok(report)
    }
}
#[async_trait::async_trait]
impl<F: RootFilesystem + 'static> SecretCleanupService for FilesystemAuthProductServices<F> {
    async fn cleanup_for_lifecycle(
        &self,
        request: SecretCleanupRequest,
    ) -> Result<SecretCleanupReport, AuthProductError> {
        let mut report = SecretCleanupReport::default();
        for account in self
            .account_records_for_owner(&CredentialAccountOwnerScope::from_scope(&request.scope))
            .await?
        {
            if !(account.granted_extensions.contains(&request.extension_id)
                || account.ownership == CredentialOwnership::ExtensionOwned
                    && account.owner_extension.as_ref() == Some(&request.extension_id))
            {
                continue;
            }
            match self
                .cleanup_account(&request, &account.scope, account.id)
                .await
            {
                Ok(mut local) => {
                    report.revoked_accounts.append(&mut local.revoked_accounts);
                    report
                        .retained_accounts
                        .append(&mut local.retained_accounts);
                    report.removed_grants.append(&mut local.removed_grants);
                    report
                        .quarantined_accounts
                        .append(&mut local.quarantined_accounts);
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
