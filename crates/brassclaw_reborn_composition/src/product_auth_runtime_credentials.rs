use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_auth::{
    AuthProductError, AuthProductScope, AuthProviderId, AuthSurface, CredentialAccount,
    CredentialAccountRecordSource, CredentialAccountSelectionRequest, CredentialAccountStatus,
    CredentialOwnership, ProviderScope,
};
use brassclaw_host_api::CredentialStageError;
use brassclaw_host_runtime::{
    RuntimeCredentialAccessSecret, RuntimeCredentialAccountRequest,
    RuntimeCredentialAccountResolver,
};

#[cfg(all(feature = "postgres", feature = "skills-db"))]
pub(crate) mod captured;

#[derive(Clone)]
pub(crate) struct ProductAuthRuntimeCredentialResolver {
    accounts: Arc<dyn RuntimeCredentialAccountSelectionService>,
    secrets: Arc<dyn brassclaw_secrets::SecretStore>,
}

impl ProductAuthRuntimeCredentialResolver {
    pub(crate) fn new(
        accounts: Arc<dyn RuntimeCredentialAccountSelectionService>,
        secrets: Arc<dyn brassclaw_secrets::SecretStore>,
    ) -> Self {
        Self { accounts, secrets }
    }
}

#[async_trait]
pub(crate) trait RuntimeCredentialAccountSelectionService: Send + Sync {
    async fn select_unique_configured_runtime_account(
        &self,
        request: RuntimeCredentialAccountSelectionRequest,
    ) -> Result<CredentialAccount, AuthProductError>;
}

pub(crate) struct RuntimeCredentialAccountSelectionRequest {
    lookup: CredentialAccountSelectionRequest,
    runtime_scope: AuthProductScope,
    provider_scopes: Vec<ProviderScope>,
    requirement: Option<brassclaw_host_api::RuntimeCredentialAccountRequirement>,
}

impl RuntimeCredentialAccountSelectionRequest {
    pub(crate) fn new(
        lookup: CredentialAccountSelectionRequest,
        runtime_scope: AuthProductScope,
        provider_scopes: Vec<ProviderScope>,
    ) -> Self {
        Self {
            lookup,
            runtime_scope,
            provider_scopes,
            requirement: None,
        }
    }

    pub(crate) fn with_requirement(
        mut self,
        requirement: Option<&brassclaw_host_api::RuntimeCredentialAccountRequirement>,
    ) -> Self {
        self.requirement = requirement.cloned();
        self
    }
}

pub(crate) struct ProductAuthRuntimeCredentialAccountSelector {
    accounts: Arc<dyn CredentialAccountRecordSource>,
}

impl ProductAuthRuntimeCredentialAccountSelector {
    pub(crate) fn new(accounts: Arc<dyn CredentialAccountRecordSource>) -> Self {
        Self { accounts }
    }
}

impl std::fmt::Debug for ProductAuthRuntimeCredentialAccountSelector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductAuthRuntimeCredentialAccountSelector")
            .field("accounts", &"<credential_account_record_source>")
            .finish()
    }
}

#[async_trait]
impl RuntimeCredentialAccountSelectionService for ProductAuthRuntimeCredentialAccountSelector {
    async fn select_unique_configured_runtime_account(
        &self,
        request: RuntimeCredentialAccountSelectionRequest,
    ) -> Result<CredentialAccount, AuthProductError> {
        if let Some(requirement) = &request.requirement {
            let declaration = requirement.requirement();
            let scopes = request
                .provider_scopes
                .iter()
                .map(|scope| scope.as_str().to_owned())
                .collect::<Vec<_>>();
            let provider = brassclaw_host_api::RuntimeCredentialAccountProviderId::new(
                request.lookup.provider.as_str(),
            )
            .map_err(|_| AuthProductError::BackendConflict)?;
            let requester = request
                .lookup
                .requester_extension
                .as_ref()
                .ok_or(AuthProductError::BackendConflict)?;
            if !requirement.matches_dispatch(&declaration.handle, &provider, &scopes, requester) {
                return Err(AuthProductError::BackendConflict);
            }
            let selected = self
                .accounts
                .requirement_selection(&request.runtime_scope, requirement)
                .await?
                .ok_or(AuthProductError::CredentialMissing)?;
            selected.verify_requested(&request.runtime_scope, requirement)?;
            let account = self
                .accounts
                .account_record(selected.original_scope(), selected.account().id)
                .await?
                .ok_or(AuthProductError::CredentialMissing)?;
            if account.id != selected.account().id
                || account.provider != request.lookup.provider
                || account.scope != selected.account().scope
            {
                return Err(AuthProductError::BackendConflict);
            }
            if account.effective_status() != CredentialAccountStatus::Configured
                || !account_has_provider_scopes(&account, &request.provider_scopes)
            {
                return Err(AuthProductError::CredentialMissing);
            }
            if !account_visible_from_runtime_scope(&account, &request.runtime_scope)
                || !account.is_authorized_for_requester(Some(requester))
            {
                return Err(AuthProductError::CrossScopeDenied);
            }
            return Ok(account);
        }
        let configured = self
            .accounts
            .accounts_for_owner(&request.lookup.scope)
            .await?
            .into_iter()
            .filter(|account| {
                account.provider == request.lookup.provider
                    && account.effective_status() == CredentialAccountStatus::Configured
                    && account_has_provider_scopes(account, &request.provider_scopes)
                    && account_visible_from_runtime_scope(account, &request.runtime_scope)
            })
            .collect::<Vec<_>>();
        if configured.is_empty() {
            return Err(AuthProductError::CredentialMissing);
        }
        let selectable = configured
            .into_iter()
            .filter(|account| {
                account.is_authorized_for_requester(request.lookup.requester_extension.as_ref())
            })
            .collect::<Vec<_>>();
        match selectable.as_slice() {
            [] => Err(AuthProductError::CrossScopeDenied),
            [account] => Ok(account.clone()),
            // Labels, recency and identical ownership are not an account
            // selection. The existing setup owner must retain an explicit
            // requirement/account binding before runtime can disambiguate.
            _ => Err(AuthProductError::AccountSelectionRequired),
        }
    }
}

impl std::fmt::Debug for ProductAuthRuntimeCredentialResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductAuthRuntimeCredentialResolver")
            .field("accounts", &"<credential_account_service>")
            .finish()
    }
}

#[async_trait]
impl RuntimeCredentialAccountResolver for ProductAuthRuntimeCredentialResolver {
    async fn resolve_access_secret(
        &self,
        request: RuntimeCredentialAccountRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, CredentialStageError> {
        let auth_scope =
            AuthProductScope::new(runtime_account_owner_scope(request.scope), AuthSurface::Api);
        let provider = AuthProviderId::new(request.provider.as_str()).map_err(|e| {
            tracing::debug!(
                provider = %request.provider.as_str(),
                err = %e,
                "product-auth provider id is invalid"
            );
            CredentialStageError::Backend
        })?;
        let provider_scopes = request
            .provider_scopes
            .iter()
            .map(|scope| {
                ProviderScope::new(scope.clone()).map_err(|e| {
                    tracing::debug!(
                        scope = %scope,
                        err = %e,
                        "runtime credential provider scope is invalid"
                    );
                    CredentialStageError::Backend
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let account = self
            .accounts
            .select_unique_configured_runtime_account(
                RuntimeCredentialAccountSelectionRequest::new(
                    CredentialAccountSelectionRequest::new(auth_scope, provider)
                        .for_extension(request.requester_extension.clone()),
                    AuthProductScope::new(request.scope.clone(), AuthSurface::Api),
                    provider_scopes,
                )
                .with_requirement(request.requirement),
            )
            .await
            .map_err(map_account_error)?;
        if account.effective_status() != CredentialAccountStatus::Configured {
            return Err(CredentialStageError::AuthRequired);
        }
        // A Configured account missing access_secret indicates data corruption,
        // not a re-auth prompt. The durable product-auth store (#4234) preserves
        // the Configured ↔ access_secret=Some invariant (manual-token submit sets
        // both together; cleanup/uninstall clears status to Revoked together with
        // the handle), so this branch can only fire on corrupt state. Return
        // Backend so the caller does not loop through re-auth.
        let handle = account.access_secret.ok_or(CredentialStageError::Backend)?;
        RuntimeCredentialAccessSecret::capture(
            self.secrets.as_ref(),
            account.scope.resource,
            handle,
            account.access_expires_at,
        )
        .await
    }
}

fn account_has_provider_scopes(
    account: &CredentialAccount,
    required_scopes: &[ProviderScope],
) -> bool {
    required_scopes
        .iter()
        .all(|required| account.scopes.iter().any(|scope| scope == required))
}

fn account_visible_from_runtime_scope(
    account: &CredentialAccount,
    runtime_scope: &AuthProductScope,
) -> bool {
    let account_resource = &account.scope.resource;
    let runtime_resource = &runtime_scope.resource;
    if account_resource.tenant_id != runtime_resource.tenant_id
        || account_resource.user_id != runtime_resource.user_id
        || account_resource.agent_id != runtime_resource.agent_id
        || account_resource.project_id != runtime_resource.project_id
    {
        return false;
    }
    if account.ownership == CredentialOwnership::UserReusable {
        return true;
    }
    account_resource.thread_id == runtime_resource.thread_id
        && account.scope.session_id == runtime_scope.session_id
}

fn runtime_account_owner_scope(
    scope: &brassclaw_host_api::ResourceScope,
) -> brassclaw_host_api::ResourceScope {
    let mut owner = scope.clone();
    owner.thread_id = None;
    owner
}

fn map_account_error(error: AuthProductError) -> CredentialStageError {
    match error {
        AuthProductError::CredentialMissing
        | AuthProductError::CrossScopeDenied
        | AuthProductError::AccountSelectionRequired => CredentialStageError::AuthRequired,
        _ => CredentialStageError::Backend,
    }
}

#[cfg(test)]
mod tests {
    use brassclaw_auth::{
        CredentialAccountLabel, CredentialAccountService, CredentialOwnership,
        InMemoryAuthProductServices, NewCredentialAccount,
    };
    use brassclaw_host_api::{
        ExtensionId, InvocationId, ResourceScope, RuntimeCredentialAccountProviderId, SecretHandle,
        ThreadId, UserId,
    };

    use super::*;

    async fn source_material_for_accounts(
        accounts: &dyn CredentialAccountRecordSource,
        scope: &ResourceScope,
    ) -> Arc<dyn brassclaw_secrets::SecretStore> {
        let store = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
        for account in accounts
            .accounts_for_owner(&AuthProductScope::new(
                runtime_account_owner_scope(scope),
                AuthSurface::Api,
            ))
            .await
            .unwrap()
        {
            if account.effective_status() != CredentialAccountStatus::Configured {
                continue;
            }
            if let Some(handle) = account.access_secret {
                brassclaw_secrets::SecretStore::put_with_expiry(
                    store.as_ref(),
                    account.scope.resource,
                    handle,
                    "runtime credential fixture".into(),
                    account.access_expires_at,
                )
                .await
                .unwrap();
            }
        }
        store
    }

    #[tokio::test]
    async fn resolver_returns_configured_product_auth_access_secret() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        let access_secret = SecretHandle::new("github_manual_access").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(access_secret.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let resolved = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap();

        assert_eq!(resolved.handle, access_secret);
        assert_eq!(resolved.scope, scope);
    }

    #[tokio::test]
    async fn resolver_rechecks_issuer_expiry_of_a_raw_configured_account() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: AuthProductScope::new(scope.clone(), AuthSurface::Api),
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("expired github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: Some(SecretHandle::new("expired-github-access").unwrap()),
                access_expires_at: Some(chrono::Utc::now() - chrono::Duration::seconds(1)),
                refresh_secret: None,
                scopes: vec![],
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );
        assert!(matches!(
            resolver
                .resolve_access_secret(RuntimeCredentialAccountRequest {
                    requirement: None,
                    scope: &scope,
                    provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                    provider_scopes: &[],
                    requester_extension: &ExtensionId::new("github").unwrap(),
                })
                .await,
            Err(CredentialStageError::AuthRequired)
        ));
    }

    #[tokio::test]
    async fn resolver_matches_callback_setup_account_from_runtime_invocation() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let mut setup_scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        setup_scope.thread_id = Some(ThreadId::new("thread-auth-1").unwrap());
        let mut runtime_scope = setup_scope.clone();
        runtime_scope.invocation_id = InvocationId::new();
        let access_secret = SecretHandle::new("github_manual_access").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: AuthProductScope::new(setup_scope.clone(), AuthSurface::Callback),
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(access_secret.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &runtime_scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let resolved = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &runtime_scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap();

        assert_eq!(resolved.handle, access_secret);
        assert_eq!(resolved.scope, setup_scope);
    }

    #[tokio::test]
    async fn resolver_matches_reusable_setup_account_from_new_thread() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let mut setup_scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        setup_scope.thread_id = Some(ThreadId::new("thread-auth-1").unwrap());
        let mut runtime_scope = setup_scope.clone();
        runtime_scope.thread_id = Some(ThreadId::new("thread-auth-2").unwrap());
        runtime_scope.invocation_id = InvocationId::new();
        let access_secret = SecretHandle::new("github_manual_access").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: AuthProductScope::new(setup_scope.clone(), AuthSurface::Callback),
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(access_secret.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &runtime_scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let resolved = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &runtime_scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap();

        assert_eq!(resolved.handle, access_secret);
        assert_eq!(resolved.scope, setup_scope);
    }

    #[tokio::test]
    async fn resolver_matches_reusable_setup_account_from_new_mission() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let setup_scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let mut runtime_scope = setup_scope.clone();
        runtime_scope.invocation_id = InvocationId::new();
        let access_secret = SecretHandle::new("github_manual_access").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: AuthProductScope::new(setup_scope.clone(), AuthSurface::Callback),
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(access_secret.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &runtime_scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let resolved = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &runtime_scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap();

        assert_eq!(resolved.handle, access_secret);
        assert_eq!(resolved.scope, setup_scope);
    }

    #[tokio::test]
    async fn resolver_rejects_extension_owned_account_from_new_thread() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let mut setup_scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        setup_scope.thread_id = Some(ThreadId::new("thread-auth-1").unwrap());
        let mut runtime_scope = setup_scope.clone();
        runtime_scope.thread_id = Some(ThreadId::new("thread-auth-2").unwrap());
        runtime_scope.invocation_id = InvocationId::new();
        accounts
            .create_account(NewCredentialAccount {
                scope: AuthProductScope::new(setup_scope, AuthSurface::Callback),
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::ExtensionOwned,
                owner_extension: Some(ExtensionId::new("github").unwrap()),
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("github_manual_access").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &runtime_scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &runtime_scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap_err();

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[tokio::test]
    async fn resolver_maps_missing_account_to_auth_required() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
        );
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap_err();

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[tokio::test]
    async fn resolver_requires_requested_provider_scopes() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("work google").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("google_manual_access").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![
                    ProviderScope::new("https://www.googleapis.com/auth/gmail.send").unwrap(),
                ],
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );
        let required_scopes = vec!["https://www.googleapis.com/auth/drive".to_string()];

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("google").unwrap(),
                provider_scopes: &required_scopes,
                requester_extension: &ExtensionId::new("google-drive").unwrap(),
            })
            .await
            .unwrap_err();

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[tokio::test]
    async fn resolver_maps_unconfigured_account_status_to_auth_required() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::PendingSetup,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: None,
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap_err();

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[tokio::test]
    async fn resolver_maps_configured_account_without_access_secret_to_backend() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: None, // Configured but missing secret — data corruption
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .unwrap_err();

        // Data corruption: should be Backend, not AuthRequired (re-auth would not fix it).
        // The durable product-auth store preserves Configured ↔ access_secret=Some,
        // so this state cannot arise from legitimate cleanup or rotation paths.
        assert_eq!(error, CredentialStageError::Backend);
    }

    #[tokio::test]
    async fn resolver_requires_selection_across_multiple_reusable_logins() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        let latest_secret = SecretHandle::new("work-token").unwrap();
        // Two reusable accounts for the same provider under distinct labels.
        // The newer account must not silently replace the first selection.
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope.clone(),
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("personal github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("personal-token").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("work github").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(latest_secret.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .expect_err("newer same-provider credentials require an explicit selection");

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[tokio::test]
    async fn resolver_does_not_treat_identical_labels_as_one_credential() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        let first_secret = SecretHandle::new("old-token").unwrap();
        let latest_secret = SecretHandle::new("new-token").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope.clone(),
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("GitHub").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(first_secret),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("github").unwrap(),
                label: CredentialAccountLabel::new("GitHub").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(latest_secret.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("github").unwrap(),
                provider_scopes: &[],
                requester_extension: &ExtensionId::new("github").unwrap(),
            })
            .await
            .expect_err("identical labels do not prove an identical credential selection");

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[cfg(feature = "skills-db")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_ambiguous_accounts_require_binding_and_keep_both_originals() {
        use crate::pg_auth_product_services::{PgAuthProductServices, run_auth_migrations};
        use brassclaw_auth::{CredentialAccountChoiceRequest, CredentialAccountLookupRequest};

        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let secrets = Arc::new(
            brassclaw_secrets::PgSecretStore::new(
                rig.pool.as_ref().clone(),
                "native-credential-ambiguity-master-key-2026".into(),
                "native-runtime-ambiguity",
            )
            .unwrap(),
        );
        let accounts = Arc::new(PgAuthProductServices::new(
            rig.pool.clone(),
            secrets.clone(),
        ));
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        let provider = AuthProviderId::new("github").unwrap();
        let mut originals = Vec::new();
        for label in ["personal", "work"] {
            let mut account_scope = auth_scope.clone();
            if label == "work" {
                account_scope.resource.invocation_id = InvocationId::new();
            }
            originals.push(
                accounts
                    .create_account(NewCredentialAccount {
                        scope: account_scope,
                        provider: provider.clone(),
                        label: CredentialAccountLabel::new(label).unwrap(),
                        status: CredentialAccountStatus::Configured,
                        ownership: CredentialOwnership::UserReusable,
                        owner_extension: None,
                        granted_extensions: Vec::new(),
                        access_secret: Some(SecretHandle::new(format!("{label}-token")).unwrap()),
                        access_expires_at: None,
                        refresh_secret: None,
                        scopes: Vec::new(),
                    })
                    .await
                    .unwrap(),
            );
        }
        let selector = Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(
            accounts.clone(),
        ));
        let unbound = selector
            .select_unique_configured_runtime_account(
                RuntimeCredentialAccountSelectionRequest::new(
                    CredentialAccountSelectionRequest::new(auth_scope.clone(), provider.clone()),
                    auth_scope.clone(),
                    Vec::new(),
                ),
            )
            .await;
        assert!(
            matches!(unbound, Err(AuthProductError::AccountSelectionRequired)),
            "visible owner accounts require an explicit choice, not an invocation-based guess"
        );
        let resolver = ProductAuthRuntimeCredentialResolver::new(selector, secrets);
        let runtime_provider = RuntimeCredentialAccountProviderId::new("github").unwrap();
        let requester = ExtensionId::new("github").unwrap();
        for lookup_only in [false, true] {
            if lookup_only {
                // This port returns a validated projection; it does not persist
                // a requirement binding or authorize changing runtime's target.
                let selected = accounts
                    .select_configured_account(CredentialAccountChoiceRequest::new(
                        auth_scope.clone(),
                        provider.clone(),
                        originals[0].id,
                    ))
                    .await
                    .unwrap();
                assert_eq!(selected.id, originals[0].id);
            }
            assert_eq!(
                resolver
                    .resolve_access_secret(RuntimeCredentialAccountRequest {
                        requirement: None,
                        scope: &scope,
                        provider: &runtime_provider,
                        provider_scopes: &[],
                        requester_extension: &requester,
                    })
                    .await
                    .unwrap_err(),
                CredentialStageError::AuthRequired,
            );
        }
        for original in originals {
            let current = accounts
                .get_account(CredentialAccountLookupRequest::new(
                    original.scope.clone(),
                    original.id,
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                current, original,
                "selection must not touch either account's recency"
            );
            assert_eq!(
                current.snapshot_ref().unwrap(),
                original.snapshot_ref().unwrap()
            );
        }
    }

    /// Capability-like labels do not establish that two accounts came from the
    /// same login, or bind either credential to the requested capability.
    #[tokio::test]
    async fn resolver_requires_selection_for_capability_labeled_accounts() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        let gmail_scope =
            ProviderScope::new("https://www.googleapis.com/auth/gmail.modify").unwrap();
        let latest_secret = SecretHandle::new("calendar-surface-token").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope.clone(),
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("gmail google").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("gmail-surface-token").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![gmail_scope.clone()],
            })
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("google-calendar google").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(latest_secret.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![gmail_scope.clone()],
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("google").unwrap(),
                provider_scopes: &[gmail_scope.as_str().to_string()],
                requester_extension: &ExtensionId::new("gmail").unwrap(),
            })
            .await
            .expect_err("capability-like labels cannot select an account");

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[tokio::test]
    async fn resolver_does_not_auto_select_mixed_reusable_and_extension_owned_accounts() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        let requester = ExtensionId::new("gmail").unwrap();
        let google_scope =
            ProviderScope::new("https://www.googleapis.com/auth/gmail.readonly").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope.clone(),
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("reusable google").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("reusable-token").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![google_scope.clone()],
            })
            .await
            .unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("extension google").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::ExtensionOwned,
                owner_extension: Some(requester.clone()),
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("extension-token").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![google_scope.clone()],
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("google").unwrap(),
                provider_scopes: &[google_scope.as_str().to_string()],
                requester_extension: &requester,
            })
            .await
            .expect_err("mixed ownership must require explicit account selection");

        assert_eq!(error, CredentialStageError::AuthRequired);
    }

    #[tokio::test]
    async fn resolver_does_not_auto_select_mixed_reusable_and_shared_admin_accounts() {
        let accounts = Arc::new(InMemoryAuthProductServices::new());
        let scope =
            ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new())
                .unwrap();
        let auth_scope = AuthProductScope::new(scope.clone(), AuthSurface::Api);
        let requester = ExtensionId::new("gmail").unwrap();
        let google_scope =
            ProviderScope::new("https://www.googleapis.com/auth/gmail.readonly").unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope.clone(),
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("reusable google").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(SecretHandle::new("reusable-token").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![google_scope.clone()],
            })
            .await
            .unwrap();
        accounts
            .create_account(NewCredentialAccount {
                scope: auth_scope,
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("shared google").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::SharedAdminManaged,
                owner_extension: None,
                granted_extensions: vec![requester.clone()],
                access_secret: Some(SecretHandle::new("shared-token").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![google_scope.clone()],
            })
            .await
            .unwrap();
        let secrets = source_material_for_accounts(accounts.as_ref(), &scope).await;
        let resolver = ProductAuthRuntimeCredentialResolver::new(
            Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(accounts)),
            secrets,
        );

        let error = resolver
            .resolve_access_secret(RuntimeCredentialAccountRequest {
                requirement: None,
                scope: &scope,
                provider: &RuntimeCredentialAccountProviderId::new("google").unwrap(),
                provider_scopes: &[google_scope.as_str().to_string()],
                requester_extension: &requester,
            })
            .await
            .expect_err("mixed sharing semantics must require explicit account selection");

        assert_eq!(error, CredentialStageError::AuthRequired);
    }
}
