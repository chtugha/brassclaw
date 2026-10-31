use async_trait::async_trait;
use brassclaw_filesystem::{CasExpectation, RootFilesystem};
use chrono::Utc;

use super::domain::{
    account_is_authorized_for_requester, recovery_projection_for_single_account,
    recovery_projection_for_unconfigured_accounts, update_account_from_request,
    validate_credential_status_transition,
};
use super::{FilesystemAuthProductServices, scope_matches};
use brassclaw_auth::{
    AuthProductError, CredentialAccount, CredentialAccountChoiceRequest, CredentialAccountId,
    CredentialAccountListPage, CredentialAccountListRequest, CredentialAccountLookupRequest,
    CredentialAccountMutation, CredentialAccountOwnerScope, CredentialAccountProjection,
    CredentialAccountRecordSource, CredentialAccountSelectionRequest, CredentialAccountService,
    CredentialAccountStatus, CredentialRecoveryProjection, CredentialRecoveryReason,
    CredentialRecoveryRequest, CredentialRefreshReport, CredentialRefreshRequest,
    CredentialSetupService, NewCredentialAccount,
};

#[async_trait]
impl<F> CredentialAccountService for FilesystemAuthProductServices<F>
where
    F: RootFilesystem + 'static,
{
    async fn create_account(
        &self,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        self.create_account_with_id(CredentialAccountId::new(), request, CasExpectation::Absent)
            .await
    }

    async fn get_account(
        &self,
        request: CredentialAccountLookupRequest,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        let Some((account, _)) = self
            .read_account(&request.scope, request.account_id)
            .await?
        else {
            return Ok(None);
        };
        if !scope_matches(&request.scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if !account_is_authorized_for_requester(&account, request.requester_extension.as_ref()) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(Some(account))
    }

    async fn list_accounts(
        &self,
        request: CredentialAccountListRequest,
    ) -> Result<CredentialAccountListPage, AuthProductError> {
        request.validate()?;
        // accounts_for_scope reads all accounts then filters; we cannot push
        // provider/cursor/auth filters to the storage layer without a per-
        // provider directory layout, so pagination is applied in-memory here.
        let mut accounts = self
            .accounts_for_scope(&request.scope)
            .await?
            .into_iter()
            .filter(|account| {
                account.provider == request.provider
                    && request.cursor.is_none_or(|cursor| account.id > cursor)
                    && account_is_authorized_for_requester(
                        account,
                        request.requester_extension.as_ref(),
                    )
            })
            .map(|account| account.projection())
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);
        let next_cursor = if accounts.len() > request.limit {
            accounts.truncate(request.limit);
            accounts.last().map(|account| account.id)
        } else {
            None
        };
        Ok(CredentialAccountListPage {
            accounts,
            next_cursor,
        })
    }

    async fn update_status(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        account_id: CredentialAccountId,
        status: CredentialAccountStatus,
    ) -> Result<CredentialAccount, AuthProductError> {
        let lock = self.lock_for(format!("account:{account_id}"));
        let _guard = lock.lock().await;
        let (mut account, version) = self
            .read_account(scope, account_id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if !scope_matches(scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        validate_credential_status_transition(account.status, status)?;
        account.status = status;
        account.updated_at = Utc::now();
        self.write_account(&account, version.cas()).await?;
        Ok(account)
    }

    async fn select_unique_configured_account(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        let configured = self
            .accounts_for_scope(&request.scope)
            .await?
            .into_iter()
            .filter(|account| {
                account.provider == request.provider
                    && account.effective_status() == CredentialAccountStatus::Configured
            })
            .collect::<Vec<_>>();
        if configured.is_empty() {
            return Err(AuthProductError::CredentialMissing);
        }
        let selectable = configured
            .iter()
            .filter(|account| {
                account_is_authorized_for_requester(account, request.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        match selectable.as_slice() {
            [] => Err(AuthProductError::CrossScopeDenied),
            [account] => Ok(account.projection()),
            _ => Err(AuthProductError::AccountSelectionRequired),
        }
    }

    async fn project_credential_recovery(
        &self,
        request: CredentialRecoveryRequest,
    ) -> Result<CredentialRecoveryProjection, AuthProductError> {
        let mut accounts = self
            .accounts_for_scope(&request.scope)
            .await?
            .into_iter()
            .filter(|account| account.provider == request.provider)
            .collect::<Vec<_>>();
        accounts.sort_by_key(|account| account.id);
        if accounts.is_empty() {
            return Ok(CredentialRecoveryProjection::setup_required(
                request.provider,
                CredentialRecoveryReason::NoAccount,
                Vec::new(),
            ));
        }
        let authorized = accounts
            .iter()
            .filter(|account| {
                account_is_authorized_for_requester(account, request.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        if authorized.is_empty() {
            return Ok(CredentialRecoveryProjection::setup_required(
                request.provider,
                CredentialRecoveryReason::NoAccount,
                Vec::new(),
            ));
        }
        let configured = authorized
            .iter()
            .copied()
            .filter(|account| account.effective_status() == CredentialAccountStatus::Configured)
            .collect::<Vec<_>>();
        match configured.as_slice() {
            [account] => {
                return Ok(CredentialRecoveryProjection::configured(
                    request.provider,
                    account.projection(),
                ));
            }
            [_, ..] => {
                return Ok(CredentialRecoveryProjection::account_selection_required(
                    request.provider,
                    configured
                        .iter()
                        .map(|account| account.projection())
                        .collect(),
                ));
            }
            [] => {}
        }
        if let [account] = authorized.as_slice() {
            return Ok(recovery_projection_for_single_account(
                request.provider,
                account,
            ));
        }
        Ok(recovery_projection_for_unconfigured_accounts(
            request.provider,
            &authorized,
        ))
    }

    async fn select_configured_account(
        &self,
        request: CredentialAccountChoiceRequest,
    ) -> Result<CredentialAccountProjection, AuthProductError> {
        let account = self
            .read_account(&request.scope, request.account_id)
            .await?
            .map(|(account, _)| account)
            .ok_or(AuthProductError::CredentialMissing)?;
        if !scope_matches(&request.scope, &account.scope) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        if account.provider != request.provider {
            return Err(AuthProductError::CredentialMissing);
        }
        if account.effective_status() != CredentialAccountStatus::Configured {
            return Err(AuthProductError::CredentialMissing);
        }
        if !account_is_authorized_for_requester(&account, request.requester_extension.as_ref()) {
            return Err(AuthProductError::CrossScopeDenied);
        }
        Ok(account.projection())
    }

    async fn refresh_account(
        &self,
        _request: CredentialRefreshRequest,
    ) -> Result<CredentialRefreshReport, AuthProductError> {
        Err(AuthProductError::BackendUnavailable)
    }
}

#[async_trait]
impl<F> CredentialAccountRecordSource for FilesystemAuthProductServices<F>
where
    F: RootFilesystem + 'static,
{
    async fn account_record(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        account: CredentialAccountId,
    ) -> Result<Option<CredentialAccount>, AuthProductError> {
        self.read_account(scope, account)
            .await
            .map(|record| record.map(|(account, _)| account))
    }

    async fn requirement_selection(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSelection>, AuthProductError> {
        let key = brassclaw_auth::CredentialRequirementOwner::from_scope(&scope.resource)
            .key(requirement)?;
        let path = super::paths::requirement_selection_path(scope, &key);
        let selected: Option<(
            brassclaw_auth::CredentialRequirementSelection,
            super::AuthRecordSnapshot,
        )> = self.read_record(&scope.resource, &path).await?;
        let selected = selected.map(|(selected, _)| selected);
        if let Some(selected) = &selected {
            selected
                .verify_requested(scope, requirement)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            let (origin, _) = self
                .read_flow_document(selected.original_scope(), selected.original_flow())
                .await?
                .ok_or(AuthProductError::BackendUnavailable)?;
            let setup = origin
                .auth_custody
                .requirement_setup
                .as_deref()
                .ok_or(AuthProductError::BackendUnavailable)?;
            if origin.auth_custody.requirement_selection.as_deref() != Some(selected) {
                return Err(AuthProductError::BackendUnavailable);
            }
            selected
                .verify_receipt(&origin.flow, setup)
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            if let Some(source) = &origin.auth_custody.requirement_source {
                source
                    .verify_selection(selected)
                    .map_err(|_| AuthProductError::BackendUnavailable)?;
            }
        }
        Ok(selected)
    }

    async fn requirement_selection_for_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: brassclaw_auth::AuthFlowId,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSelection>, AuthProductError> {
        let Some((document, _)) = self.read_flow_document(scope, flow_id).await? else {
            return Ok(None);
        };
        document.auth_custody.original_requirement_selection(
            scope,
            flow_id,
            &document.flow,
            requirement,
        )
    }

    async fn requirement_source_for_flow(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
        flow_id: brassclaw_auth::AuthFlowId,
        requirement: &brassclaw_host_api::RuntimeCredentialAccountRequirement,
    ) -> Result<Option<brassclaw_auth::CredentialRequirementSource>, AuthProductError> {
        let Some((document, _)) = self.read_flow_document(scope, flow_id).await? else {
            return Ok(None);
        };
        document.auth_custody.original_requirement_source(
            scope,
            flow_id,
            &document.flow,
            requirement,
        )
    }

    async fn accounts_for_owner(
        &self,
        scope: &brassclaw_auth::AuthProductScope,
    ) -> Result<Vec<CredentialAccount>, AuthProductError> {
        let owner = CredentialAccountOwnerScope::from_scope(scope);
        self.account_records_for_owner(&owner).await
    }

    async fn select_unique_configured_account_for_owner(
        &self,
        request: CredentialAccountSelectionRequest,
    ) -> Result<CredentialAccount, AuthProductError> {
        self.select_configured_account_for_owner(request).await
    }
}

#[async_trait]
impl<F> CredentialSetupService for FilesystemAuthProductServices<F>
where
    F: RootFilesystem + 'static,
{
    async fn refresh_action(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
        self.read_refresh_action(request).await
    }
    async fn begin_refresh(
        &self,
        prepared: brassclaw_auth::CredentialRefreshPrepared,
    ) -> Result<brassclaw_auth::CredentialRefreshAdmission, AuthProductError> {
        self.begin_refresh_action(prepared).await
    }
    async fn reconcile_refresh(
        &self,
        request: &CredentialRefreshRequest,
    ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError> {
        self.reconcile_refresh_action(request).await
    }

    async fn complete_refresh(
        &self,
        completion: brassclaw_auth::CredentialRefreshCompletion,
    ) -> Result<brassclaw_auth::CredentialRefreshAction, AuthProductError> {
        self.complete_refresh_action(completion).await
    }

    async fn compare_and_update_account(
        &self,
        expected: &CredentialAccount,
        request: NewCredentialAccount,
    ) -> Result<CredentialAccount, AuthProductError> {
        let lock = self.lock_for(format!("account:{}", expected.id));
        let _guard = lock.lock().await;
        let (mut current, version) = self
            .read_account(&expected.scope, expected.id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if &current != expected {
            return Err(AuthProductError::BackendConflict);
        }
        update_account_from_request(&mut current, request, Utc::now())?;
        self.write_account(&current, version.cas()).await?;
        Ok(current)
    }

    async fn create_or_update_account(
        &self,
        request: CredentialAccountMutation,
    ) -> Result<CredentialAccount, AuthProductError> {
        match request {
            CredentialAccountMutation::Create(account) => self.create_account(account).await,
            CredentialAccountMutation::Update(update) => {
                let lock = self.lock_for(format!("account:{}", update.account_id));
                let _guard = lock.lock().await;
                let (mut account, version) = self
                    .read_account(&update.account.scope, update.account_id)
                    .await?
                    .ok_or(AuthProductError::CredentialMissing)?;
                update_account_from_request(&mut account, update.account, Utc::now())?;
                self.write_account(&account, version.cas()).await?;
                Ok(account)
            }
        }
    }
}
