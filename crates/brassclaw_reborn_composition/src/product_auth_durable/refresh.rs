//! Single-record account/result publication for CAS-only backends.
//! A separate original intent/result reserves identity before effects and retains
//! replies before publication. It NEVER certifies account publication; the account
//! envelope owns the final receipt and every account writer preserves it.
use super::paths::refresh_action_path;
use super::*;
use brassclaw_auth::{
    CredentialRefreshAction, CredentialRefreshAdmission, CredentialRefreshCompletion,
    CredentialRefreshId, CredentialRefreshOriginal, CredentialRefreshOutcome,
    CredentialRefreshPrepared, CredentialRefreshPublication, CredentialRefreshRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StoredCredentialAccount {
    #[serde(flatten)]
    pub(super) account: CredentialAccount,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) auth_lifecycle_cleanup:
        Vec<crate::credential_cleanup_custody::CredentialCleanupReceipt>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) auth_refresh_actions: BTreeMap<CredentialRefreshId, CredentialRefreshAction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) auth_callback_actions:
        BTreeMap<brassclaw_auth::AuthFlowId, brassclaw_auth::OAuthCallbackAction>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) auth_manual_actions:
        BTreeMap<brassclaw_auth::AuthInteractionId, super::manual::ManualReceipt>,
}

fn selected_original(
    request: &CredentialRefreshRequest,
    locator: Option<CredentialRefreshAction>,
    journal: Option<&CredentialRefreshAction>,
) -> Result<Option<CredentialRefreshAction>, AuthProductError> {
    let Some(locator) = locator else {
        return if journal.is_some() {
            Err(AuthProductError::BackendConflict)
        } else {
            Ok(None)
        };
    };
    locator.original.verify(request)?;
    if let Some(journal) = journal {
        journal.original.verify(request)?;
        if !locator.same_intent(journal) {
            return Err(AuthProductError::BackendConflict);
        }
        if locator.outcome == journal.outcome {
            return Ok(Some(journal.clone()));
        }
        if !matches!(journal.outcome, CredentialRefreshOutcome::Submitted) {
            return Err(AuthProductError::BackendConflict);
        }
    }
    // Only a matching account receipt certifies publication; a known result
    // without that final receipt remains Pending and can be reconciled.
    Ok(Some(locator))
}

impl<F: RootFilesystem> FilesystemAuthProductServices<F> {
    pub(super) async fn read_account_document(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        account_id: CredentialAccountId,
    ) -> Result<Option<(StoredCredentialAccount, AuthRecordSnapshot)>, AuthProductError> {
        let document: Option<(StoredCredentialAccount, AuthRecordSnapshot)> = self
            .read_record(&scope.resource, &account_path(scope, account_id))
            .await?;
        if let Some((document, _)) = &document
            && (document.account.id != account_id || !scope_matches(scope, &document.account.scope))
        {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if let Some((document, _)) = &document {
            for receipt in &document.auth_lifecycle_cleanup {
                receipt.validate(&document.account)?;
            }
        }
        Ok(document)
    }
    pub(super) async fn write_account_document(
        &self,
        document: &StoredCredentialAccount,
        cas: CasExpectation,
    ) -> Result<AuthRecordSnapshot, AuthProductError> {
        for receipt in &document.auth_lifecycle_cleanup {
            receipt.validate(&document.account)?;
        }
        self.write_record(
            &document.account.scope.resource,
            &account_path(&document.account.scope, document.account.id),
            document,
            cas,
        )
        .await
    }

    pub(super) async fn read_refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<CredentialRefreshAction>, AuthProductError> {
        let original = CredentialRefreshOriginal::from(request);
        let locator: Option<(CredentialRefreshAction, AuthRecordSnapshot)> = self
            .read_record(&request.scope.resource, &refresh_action_path(&original)?)
            .await?;
        let document = self
            .read_account_document(&request.scope, request.account_id)
            .await?;
        let journal = document
            .as_ref()
            .and_then(|(doc, _)| doc.auth_refresh_actions.get(&request.id));
        selected_original(request, locator.map(|(action, _)| action), journal)
    }
    pub(super) async fn begin_refresh_action(
        &self,
        prepared: CredentialRefreshPrepared,
    ) -> Result<CredentialRefreshAdmission, AuthProductError> {
        let original = prepared.original.clone();
        if let Some(action) = self.read_refresh_action(&original.request()).await? {
            return Ok(CredentialRefreshAdmission::Original(action));
        }
        let lock = self.lock_for(format!("account:{}", original.account_id));
        let _guard = lock.lock().await;
        let path = refresh_action_path(&original)?;
        let locator: Option<(CredentialRefreshAction, AuthRecordSnapshot)> =
            self.read_record(&original.scope.resource, &path).await?;
        if let Some((locator, _)) = locator {
            if locator.original != original {
                return Err(AuthProductError::BackendConflict);
            }
            let request = CredentialRefreshRequest {
                id: original.id,
                scope: original.scope,
                provider: original.provider,
                account_id: original.account_id,
                requester_extension: original.requester_extension,
            };
            let action = self
                .read_refresh_action(&request)
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            return Ok(CredentialRefreshAdmission::Original(action));
        }
        let (mut document, version) = self
            .read_account_document(&original.scope, original.account_id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if let Some(action) = document.auth_refresh_actions.get(&original.id) {
            if action.original != original {
                return Err(AuthProductError::BackendConflict);
            }
            return Ok(CredentialRefreshAdmission::Original(action.clone()));
        }
        let existing = document
            .auth_refresh_actions
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let admission =
            CredentialRefreshAction::begin(&mut document.account, &existing, prepared, Utc::now())?;
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
                .record_reference(&source.scope, &source.handle)
                .await
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            if selected.as_deref() != Some(source.record_reference.as_str()) {
                return Err(AuthProductError::BackendConflict);
            }
            // This read and the intent CAS are not one secret transaction.
            // Host lease/consume recheck the exact selection before any HTTP.
        }

        let mut reservation = action.clone();
        reservation.publication = CredentialRefreshPublication::Pending;
        reservation.published_account = None;
        // Intent creation is absent-only. Losing this acknowledgement causes zero
        // issuer calls; a retry reads the retained reservation rather than issuing.
        if let Err(write_error) = self
            .write_record(
                &original.scope.resource,
                &path,
                &reservation,
                CasExpectation::Absent,
            )
            .await
        {
            let request = CredentialRefreshRequest {
                id: original.id,
                scope: original.scope.clone(),
                provider: original.provider.clone(),
                account_id: original.account_id,
                requester_extension: original.requester_extension.clone(),
            };
            return match self.read_refresh_action(&request).await {
                Ok(Some(original)) => Ok(CredentialRefreshAdmission::Original(original)),
                Ok(None) => Err(write_error),
                Err(read_error) => Err(read_error),
            };
        }
        document
            .auth_refresh_actions
            .insert(original.id, action.clone());
        self.write_account_document(&document, version.cas())
            .await?;
        Ok(admission)
    }

    async fn record_refresh_result(
        &self,
        completion: CredentialRefreshCompletion,
    ) -> Result<CredentialRefreshAction, AuthProductError> {
        let original = &completion.dispatched.original;
        let path = refresh_action_path(original)?;
        let (mut action, version): (CredentialRefreshAction, AuthRecordSnapshot) = self
            .read_record(&original.scope.resource, &path)
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        let before = action.clone();
        action.record_result(completion.clone(), Utc::now())?;
        if action == before {
            return Ok(action);
        }
        if let Err(write_error) = self
            .write_record(&original.scope.resource, &path, &action, version.cas())
            .await
        {
            // Exact original readback handles a lost result acknowledgement or
            // an identical concurrent result. No retry invokes the issuer.
            let after: Option<(CredentialRefreshAction, AuthRecordSnapshot)> =
                self.read_record(&original.scope.resource, &path).await?;
            if let Some((after, _)) = after {
                let mut expected = after.clone();
                expected.record_result(completion, Utc::now())?;
                if expected == after {
                    return Ok(after);
                }
            }
            return Err(write_error);
        }
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
        let lock = self.lock_for(format!("account:{}", request.account_id));
        let _guard = lock.lock().await;

        let original = CredentialRefreshOriginal::from(request);
        let locator: Option<(CredentialRefreshAction, AuthRecordSnapshot)> = self
            .read_record(&request.scope.resource, &refresh_action_path(&original)?)
            .await?;
        let document = self
            .read_account_document(&request.scope, request.account_id)
            .await?;
        let journal = document
            .as_ref()
            .and_then(|(doc, _)| doc.auth_refresh_actions.get(&request.id));
        let Some(mut action) =
            selected_original(request, locator.map(|(action, _)| action), journal)?
        else {
            return Ok(None);
        };
        if !action.needs_publication() {
            return Ok(Some(action));
        }
        // Selection and final CAS use this ONE account snapshot/version. A
        // concurrently committed final receipt cannot be overwritten as stale.
        let (mut document, version) = document.ok_or(AuthProductError::CredentialMissing)?;
        action.publish(&mut document.account, Utc::now())?;
        document
            .auth_refresh_actions
            .insert(request.id, action.clone());
        // The original result already exists durably. Only this same-record CAS
        // certifies final account/publication; all other writers retain it.
        self.write_account_document(&document, version.cas())
            .await?;
        Ok(Some(action))
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
