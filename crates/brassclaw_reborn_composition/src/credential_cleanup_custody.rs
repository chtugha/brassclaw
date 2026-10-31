//! Lifecycle cleanup evidence belongs to its existing account, not a second
//! registry/job/credential store. Contains only opaque references and metadata.
use brassclaw_auth::{
    AuthProductError, AuthProductScope, AuthProviderId, CredentialAccount, CredentialAccountId,
    CredentialAccountStatus, CredentialOwnership, SecretCleanupAction, SecretCleanupQuarantine,
    SecretCleanupQuarantineReason, SecretCleanupReport, SecretCleanupRequest, Timestamp,
};
use brassclaw_host_api::{ExtensionId, ResourceScope, SecretHandle};
use brassclaw_secrets::{
    SecretCustodyMetadata, SecretCustodyState, SecretIdentityClosure, SecretStore,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CredentialCleanupTarget {
    pub(crate) scope: ResourceScope,
    pub(crate) handle: SecretHandle,
    pub(crate) expected: SecretIdentityClosure,
    pub(crate) confirmed: Option<String>,
    pub(crate) error: Option<SecretCleanupQuarantineReason>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CredentialCleanupReceipt {
    pub(crate) scope: AuthProductScope,
    pub(crate) account_id: CredentialAccountId,
    pub(crate) provider: AuthProviderId,
    pub(crate) extension_id: ExtensionId,
    pub(crate) captured_account_at: Timestamp,
    pub(crate) original_access: Option<SecretHandle>,
    pub(crate) original_refresh: Option<SecretHandle>,
    /// Absent on historical receipts, which predate these client associations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) material_secrets: Vec<SecretHandle>,
    pub(crate) targets: Vec<CredentialCleanupTarget>,
}
impl CredentialCleanupReceipt {
    pub(crate) fn new(
        account: &CredentialAccount,
        extension: ExtensionId,
    ) -> Result<Self, AuthProductError> {
        let material_secrets = match (account.provider.as_str(), &account.refresh_secret) {
            (brassclaw_auth::GOOGLE_PROVIDER_ID, Some(refresh)) => {
                crate::oauth_gate::owned_google_refresh_material_handles(refresh)?
            }
            _ => vec![],
        };
        Ok(Self {
            scope: account.scope.clone(),
            account_id: account.id,
            provider: account.provider.clone(),
            extension_id: extension,
            captured_account_at: account.updated_at,
            original_access: account.access_secret.clone(),
            original_refresh: account.refresh_secret.clone(),
            material_secrets,
            targets: vec![],
        })
    }
    pub(crate) fn same_original(&self, other: &Self) -> bool {
        self.scope == other.scope
            && self.account_id == other.account_id
            && self.provider == other.provider
            && self.extension_id == other.extension_id
            && self.captured_account_at == other.captured_account_at
            && self.original_access == other.original_access
            && self.original_refresh == other.original_refresh
            && self.material_secrets == other.material_secrets
            && self.targets.len() == other.targets.len()
            && self.targets.iter().zip(&other.targets).all(|(a, b)| {
                a.scope == b.scope && a.handle == b.handle && a.expected == b.expected
            })
    }
    pub(crate) fn handles(&self) -> Vec<SecretHandle> {
        let mut handles = Vec::new();
        for handle in [&self.original_access, &self.original_refresh]
            .into_iter()
            .flatten()
        {
            if !handles.contains(handle) {
                handles.push(handle.clone());
            }
        }
        for handle in &self.material_secrets {
            if !handles.contains(handle) {
                handles.push(handle.clone());
            }
        }
        handles.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        handles
    }
    pub(crate) fn context(&self) -> serde_json::Value {
        let mut context = serde_json::json!({"format":"credential-lifecycle-cleanup/1","scope":self.scope,"account_id":self.account_id,
            "provider":self.provider,"extension_id":self.extension_id,"captured_account_at":self.captured_account_at,
            "access":self.original_access,"refresh":self.original_refresh});
        if !self.material_secrets.is_empty() {
            context["format"] = serde_json::json!("credential-lifecycle-cleanup/2");
            context["material_secrets"] = serde_json::json!(self.material_secrets);
        }
        context
    }
    pub(crate) fn confirmed(&self) -> bool {
        let handles = self.handles();
        self.targets.len() == handles.len()
            && handles.iter().all(|handle| {
                self.targets.iter().any(|target| {
                    &target.handle == handle
                        && target
                            .confirmed
                            .as_ref()
                            .is_some_and(|reference| !reference.is_empty())
                        && target.error.is_none()
                })
            })
    }
    pub(crate) fn validate(&self, account: &CredentialAccount) -> Result<(), AuthProductError> {
        if !self.material_secrets.is_empty() {
            let expected = match (self.provider.as_str(), &self.original_refresh) {
                (brassclaw_auth::GOOGLE_PROVIDER_ID, Some(refresh)) => {
                    crate::oauth_gate::owned_google_refresh_material_handles(refresh)?
                }
                _ => return Err(AuthProductError::BackendConflict),
            };
            if self.material_secrets != expected {
                return Err(AuthProductError::BackendConflict);
            }
        }
        if self.account_id != account.id
            || self.provider != account.provider
            || self.scope != account.scope
            || !self.targets.iter().all(|target| {
                same_secret_owner(&target.scope, &account.scope.resource)
                    && self.handles().contains(&target.handle)
                    && !(target.confirmed.is_some() && target.error.is_some())
                    && !target
                        .confirmed
                        .as_ref()
                        .is_some_and(|reference| reference.is_empty())
            })
            || self.targets.len() != self.handles().len()
            || self.targets.iter().enumerate().any(|(i, target)| {
                self.targets[..i]
                    .iter()
                    .any(|old| old.handle == target.handle)
            })
        {
            return Err(AuthProductError::BackendConflict);
        }
        if !self.confirmed()
            && (account.status != CredentialAccountStatus::Revoked
                || account.ownership != CredentialOwnership::ExtensionOwned
                || account.owner_extension.as_ref() != Some(&self.extension_id)
                || account.access_secret != self.original_access
                || account.refresh_secret != self.original_refresh)
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(())
    }
}
pub(crate) fn same_secret_owner(a: &ResourceScope, b: &ResourceScope) -> bool {
    a.tenant_id == b.tenant_id
        && a.user_id == b.user_id
        && a.agent_id == b.agent_id
        && a.project_id == b.project_id
}

pub(crate) fn selected_target(
    receipt: &CredentialCleanupReceipt,
    handle: SecretHandle,
    metadata: Option<SecretCustodyMetadata>,
    reference: Option<String>,
) -> Result<CredentialCleanupTarget, AuthProductError> {
    let (scope, expected) = if let Some(metadata) = metadata {
        if metadata.handle != handle || !same_secret_owner(&metadata.scope, &receipt.scope.resource)
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        let expected = match metadata.state {
            SecretCustodyState::ClosedIdentity { context } => {
                SecretIdentityClosure::Absent { context }
            }
            SecretCustodyState::Original { context } => SecretIdentityClosure::Original {
                reference: metadata.reference,
                context,
            },
            SecretCustodyState::Retained { source } => SecretIdentityClosure::Retained {
                reference: metadata.reference,
                source,
            },
            SecretCustodyState::RetiredOriginal {
                original_reference,
                context,
            } => SecretIdentityClosure::Original {
                reference: original_reference,
                context,
            },
            SecretCustodyState::RetiredRetained {
                retained_reference,
                source,
            } => SecretIdentityClosure::Retained {
                reference: retained_reference,
                source,
            },
            SecretCustodyState::RetiredOrdinary {
                original_reference,
                context,
            } => SecretIdentityClosure::Ordinary {
                reference: original_reference,
                context,
            },
        };
        (metadata.scope, expected)
    } else {
        (
            receipt.scope.resource.clone(),
            match reference {
                Some(reference) => SecretIdentityClosure::Ordinary {
                    reference,
                    context: receipt.context(),
                },
                None => SecretIdentityClosure::Absent {
                    context: receipt.context(),
                },
            },
        )
    };
    Ok(CredentialCleanupTarget {
        scope,
        handle,
        expected,
        confirmed: None,
        error: None,
    })
}

pub(crate) async fn capture(
    store: &dyn SecretStore,
    receipt: &mut CredentialCleanupReceipt,
) -> Result<(), AuthProductError> {
    for handle in receipt.handles() {
        let mut metadata = store
            .custody(&receipt.scope.resource, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?;
        let reference = if metadata.is_none() {
            let reference = store
                .record_reference(&receipt.scope.resource, &handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            if reference.is_some() {
                metadata = store
                    .custody(&receipt.scope.resource, &handle)
                    .await
                    .map_err(|_| AuthProductError::BackendUnavailable)?;
            }
            reference
        } else {
            None
        };
        receipt
            .targets
            .push(selected_target(receipt, handle, metadata, reference)?);
    }
    Ok(())
}
pub(crate) async fn close_target(store: &dyn SecretStore, target: &mut CredentialCleanupTarget) {
    if target.confirmed.is_some() {
        return;
    }
    let result = store
        .close_identity(
            target.scope.clone(),
            target.handle.clone(),
            target.expected.clone(),
        )
        .await;
    let mut unavailable = result.is_err();
    let actual = match result {
        Ok(Some(actual)) => Some(actual),
        Ok(None) | Err(_) => match store.custody(&target.scope, &target.handle).await {
            Ok(actual) => actual,
            Err(_) => {
                unavailable = true;
                None
            }
        },
    };
    if let Some(actual) = actual
        && actual.handle == target.handle
        && target.expected.accepts(&target.scope, &actual)
    {
        target.confirmed = Some(actual.reference);
        target.error = None;
    } else {
        target.error = Some(if unavailable {
            SecretCleanupQuarantineReason::BackendUnavailable
        } else {
            SecretCleanupQuarantineReason::TombstoneFailed
        });
    }
}

/// Re-evaluate eligibility from the current CAS/locked account, never the scan's
/// earlier ownership/grant snapshot. Returns whether a material purge is needed.
pub(crate) fn apply_lifecycle(
    account: &mut CredentialAccount,
    request: &SecretCleanupRequest,
    report: &mut SecretCleanupReport,
) -> bool {
    let owns = account.ownership == CredentialOwnership::ExtensionOwned
        && account.owner_extension.as_ref() == Some(&request.extension_id);
    let had_grant = account.granted_extensions.contains(&request.extension_id);
    account
        .granted_extensions
        .retain(|extension| extension != &request.extension_id);
    if had_grant {
        report.removed_grants.push(account.id);
    }
    if owns {
        match request.action {
            SecretCleanupAction::Deactivate => {
                if account.status != CredentialAccountStatus::Revoked {
                    account.status = CredentialAccountStatus::Inactive;
                }
                report.retained_accounts.push(account.id);
                false
            }
            SecretCleanupAction::Uninstall => {
                account.status = CredentialAccountStatus::Revoked;
                report.revoked_accounts.push(account.id);
                true
            }
        }
    } else {
        if had_grant {
            report.retained_accounts.push(account.id);
        }
        false
    }
}
pub(crate) fn quarantine(
    report: &mut SecretCleanupReport,
    id: CredentialAccountId,
    reason: SecretCleanupQuarantineReason,
) {
    if !report
        .quarantined_accounts
        .iter()
        .any(|entry| entry.account_id == id)
    {
        report.quarantined_accounts.push(SecretCleanupQuarantine {
            account_id: id,
            reason,
        });
    }
}
