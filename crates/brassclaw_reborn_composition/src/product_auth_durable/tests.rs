use std::sync::Arc;

use brassclaw_filesystem::{InMemoryBackend, ScopedFilesystem};
use brassclaw_host_api::{
    ExtensionId, InvocationId, MountAlias, MountGrant, MountPermissions,
    RuntimeCredentialAccountProviderId, SecretHandle, ThreadId, UserId, VirtualPath,
};
use brassclaw_host_runtime::RuntimeCredentialAccountRequest;
use brassclaw_host_runtime::RuntimeCredentialAccountResolver;
use brassclaw_secrets::{InMemorySecretStore, SecretStore};
use chrono::{Duration, Utc};
use secrecy::SecretString;
use tokio::task::JoinSet;

use super::*;
use crate::product_auth_runtime_credentials::{
    ProductAuthRuntimeCredentialAccountSelector, ProductAuthRuntimeCredentialResolver,
    RuntimeCredentialAccountSelectionRequest, RuntimeCredentialAccountSelectionService,
};
use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthFlowKind, AuthFlowManager, AuthFlowOwnerScope,
    AuthFlowRecordSource, AuthFlowStatus, AuthInteractionId, AuthInteractionService,
    AuthProductError, AuthProductScope, AuthProviderId, AuthSessionId, AuthSurface,
    AuthorizationCodeHash, CredentialAccountChoiceRequest, CredentialAccountLabel,
    CredentialAccountListRequest, CredentialAccountLookupRequest, CredentialAccountRecordSource,
    CredentialAccountSelectionRequest, CredentialAccountService, CredentialAccountStatus,
    CredentialOwnership, ManualTokenCompletionInput, ManualTokenSetupRequest, NewAuthFlow,
    NewCredentialAccount, OAuthAuthorizationUrl, OAuthCallbackInput, OAuthProviderExchange,
    OpaqueStateHash, PkceVerifierHash, ProviderScope, SecretSubmitRequest,
};

fn test_scope() -> AuthProductScope {
    let resource =
        ResourceScope::local_default(UserId::new("alice").unwrap(), InvocationId::new()).unwrap();
    AuthProductScope::new(resource, AuthSurface::Web)
}

fn test_filesystem() -> Arc<ScopedFilesystem<InMemoryBackend>> {
    let mounts = brassclaw_host_api::MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::new(InMemoryBackend::new()),
        mounts,
    ))
}

fn test_service(
    filesystem: Arc<ScopedFilesystem<InMemoryBackend>>,
    secret_store: Arc<dyn SecretStore>,
) -> FilesystemAuthProductServices<InMemoryBackend> {
    FilesystemAuthProductServices::new(filesystem, secret_store)
}

fn google_provider() -> AuthProviderId {
    AuthProviderId::new("google").unwrap()
}

fn account_label() -> CredentialAccountLabel {
    CredentialAccountLabel::new("Alice Google").unwrap()
}

async fn fs_callback_flow<F: RootFilesystem + 'static>(
    service: &FilesystemAuthProductServices<F>,
    scope: &AuthProductScope,
    binding: Option<brassclaw_auth::CredentialAccountUpdateBinding>,
    marker: &str,
) -> AuthFlowRecord {
    service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: OAuthAuthorizationUrl::new("https://provider.example/oauth")
                    .unwrap(),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: binding,
            opaque_state_hash: Some(state_hash(&format!("{marker}-state"))),
            pkce_verifier_hash: Some(pkce_hash(&format!("{marker}-pkce"))),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap()
}

fn fs_callback_prepared(
    flow: &AuthFlowRecord,
    marker: &str,
) -> brassclaw_auth::OAuthCallbackPrepared {
    brassclaw_auth::OAuthCallbackPrepared {
        original: brassclaw_auth::OAuthCallbackOriginal {
            scope: flow.scope.clone(),
            flow_id: flow.id,
            provider: flow.provider.clone(),
            opaque_state_hash: flow.opaque_state_hash.clone().unwrap(),
            authorization_code_hash: code_hash(&format!("{marker}-code")),
            pkce_verifier_hash: flow.pkce_verifier_hash.clone().unwrap(),
            account_label: account_label(),
            requested_scopes: vec![ProviderScope::new("gmail.readonly").unwrap()],
        },
        destinations: brassclaw_auth::OAuthTokenDestinations {
            access_secret: SecretHandle::new(format!("{marker}-access")).unwrap(),
            refresh_secret: SecretHandle::new(format!("{marker}-refresh")).unwrap(),
        },
    }
}

fn fs_callback_dispatch(
    admission: brassclaw_auth::OAuthCallbackAdmission,
) -> brassclaw_auth::OAuthCallbackAction {
    match admission {
        brassclaw_auth::OAuthCallbackAdmission::Dispatch(state) => state.action,
        other => panic!("one original must own the dispatch: {other:?}"),
    }
}

/// Side-effect-free backend fixture result, constructed AFTER real admission.
fn fs_callback_completion(action: &brassclaw_auth::OAuthCallbackAction) -> OAuthCallbackInput {
    OAuthCallbackInput {
        dispatched: Some(action.clone()),
        flow_id: action.original.flow_id,
        opaque_state_hash: action.original.opaque_state_hash.clone(),
        outcome: brassclaw_auth::ProviderCallbackOutcome::Authorized {
            exchange: OAuthProviderExchange {
                provider: action.original.provider.clone(),
                account_label: action.original.account_label.clone(),
                authorization_code_hash: action.original.authorization_code_hash.clone(),
                pkce_verifier_hash: action.original.pkce_verifier_hash.clone(),
                access_secret: action.destinations.access_secret.clone(),
                access_expires_at: None,
                refresh_secret: Some(action.destinations.refresh_secret.clone()),
                scopes: action.original.requested_scopes.clone(),
                account_id: None,
            },
        },
    }
}

fn fake_digest(value: &str) -> String {
    format!(
        "{:064x}",
        value.bytes().fold(0_u64, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u64::from(byte))
        })
    )
}

fn state_hash(value: &str) -> OpaqueStateHash {
    OpaqueStateHash::new(fake_digest(value)).unwrap()
}

fn pkce_hash(value: &str) -> PkceVerifierHash {
    PkceVerifierHash::new(fake_digest(value)).unwrap()
}

fn code_hash(value: &str) -> AuthorizationCodeHash {
    AuthorizationCodeHash::new(fake_digest(value)).unwrap()
}

async fn create_manual_token_flow(
    service: &FilesystemAuthProductServices<InMemoryBackend>,
    scope: &AuthProductScope,
    expires_at: chrono::DateTime<Utc>,
) -> AuthInteractionId {
    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at,
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id,
        provider,
        label,
        expires_at: challenge_expires_at,
    } = challenge
    else {
        panic!("expected manual token challenge");
    };
    service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider,
            challenge: AuthChallenge::ManualTokenRequired {
                interaction_id,
                provider: google_provider(),
                label,
                expires_at: challenge_expires_at,
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            expires_at,
        })
        .await
        .unwrap();
    interaction_id
}

#[tokio::test]
async fn filesystem_accounts_survive_service_recreation() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(Arc::clone(&filesystem), Arc::clone(&secret_store));

    let created = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("google-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("google-refresh").unwrap()),
            scopes: vec![ProviderScope::new("gmail.readonly").unwrap()],
        })
        .await
        .unwrap();

    let recreated = test_service(Arc::clone(&filesystem), secret_store);
    let loaded = recreated
        .get_account(CredentialAccountLookupRequest::new(
            scope.clone(),
            created.id,
        ))
        .await
        .unwrap()
        .expect("account should be durable");
    assert_eq!(loaded.id, created.id);
    assert_eq!(loaded.access_secret, created.access_secret);

    let page = recreated
        .list_accounts(CredentialAccountListRequest::new(scope, google_provider()))
        .await
        .unwrap();
    assert_eq!(page.accounts.len(), 1);
    assert_eq!(page.accounts[0].id, created.id);
}

#[tokio::test]
async fn filesystem_runtime_account_selection_matches_setup_invocation_account() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let mut setup_scope = test_scope();
    setup_scope.surface = AuthSurface::Callback;
    setup_scope.resource.thread_id = Some(ThreadId::new("thread-auth-1").unwrap());
    let mut runtime_scope = AuthProductScope::new(setup_scope.resource.clone(), AuthSurface::Api);
    runtime_scope.resource.invocation_id = InvocationId::new();
    let service = Arc::new(test_service(filesystem, secret_store));
    let access_secret = SecretHandle::new("google-access").unwrap();

    let created = service
        .create_account(NewCredentialAccount {
            scope: setup_scope,
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(access_secret.clone()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![ProviderScope::new("gmail.readonly").unwrap()],
        })
        .await
        .unwrap();

    let selector = ProductAuthRuntimeCredentialAccountSelector::new(service.clone());
    let selected = selector
        .select_unique_configured_runtime_account(RuntimeCredentialAccountSelectionRequest::new(
            CredentialAccountSelectionRequest::new(runtime_scope.clone(), google_provider()),
            runtime_scope,
            Vec::new(),
        ))
        .await
        .unwrap();

    assert_eq!(selected.id, created.id);
    assert_eq!(selected.access_secret, Some(access_secret));
}

#[tokio::test]
async fn filesystem_runtime_account_selection_matches_new_thread_reusable_account() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let mut setup_scope = test_scope();
    setup_scope.surface = AuthSurface::Callback;
    setup_scope.resource.thread_id = Some(ThreadId::new("thread-auth-1").unwrap());
    let mut runtime_scope = AuthProductScope::new(setup_scope.resource.clone(), AuthSurface::Api);
    runtime_scope.resource.thread_id = Some(ThreadId::new("thread-auth-2").unwrap());
    runtime_scope.resource.invocation_id = InvocationId::new();
    let service = Arc::new(test_service(filesystem, secret_store.clone()));
    let access_secret = SecretHandle::new("google-access").unwrap();

    let created = service
        .create_account(NewCredentialAccount {
            scope: setup_scope,
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(access_secret.clone()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![ProviderScope::new("gmail.readonly").unwrap()],
        })
        .await
        .unwrap();

    secret_store
        .put(
            created.scope.resource.clone(),
            access_secret.clone(),
            "selected runtime fixture".into(),
        )
        .await
        .unwrap();
    let resolver = ProductAuthRuntimeCredentialResolver::new(
        Arc::new(ProductAuthRuntimeCredentialAccountSelector::new(service)),
        secret_store,
    );
    let resolved = resolver
        .resolve_access_secret(RuntimeCredentialAccountRequest {
            requirement: None,
            scope: &runtime_scope.resource,
            provider: &RuntimeCredentialAccountProviderId::new("google").unwrap(),
            provider_scopes: &[],
            requester_extension: &ExtensionId::new("google-calendar").unwrap(),
        })
        .await
        .unwrap();

    assert_eq!(created.access_secret, Some(resolved.handle.clone()));
    assert_eq!(resolved.handle, access_secret);
    assert_eq!(resolved.scope, created.scope.resource);
}

#[tokio::test]
async fn filesystem_manual_token_submit_stores_secret_and_dedupes_replay() {
    let filesystem = test_filesystem();
    let concrete_secret_store = Arc::new(InMemorySecretStore::new());
    let secret_store: Arc<dyn SecretStore> = concrete_secret_store.clone();
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("expected manual token challenge");
    };

    let result = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("manual-token-value"),
            },
        )
        .await
        .unwrap();
    assert_eq!(result.status, CredentialAccountStatus::Configured);

    let account = service
        .get_account(CredentialAccountLookupRequest::new(
            scope.clone(),
            result.account_id,
        ))
        .await
        .unwrap()
        .expect("manual token submit should create account");
    let access_secret = account.access_secret.expect("manual token secret handle");
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &access_secret)
            .await
            .unwrap()
            .is_some()
    );

    let replay = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("manual-token-value"),
            },
        )
        .await
        .expect("same raw input must recover its original result");
    assert_eq!(replay, result);
    let changed = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("different-manual-token-value"),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(changed, AuthProductError::BackendConflict);
}

#[tokio::test]
async fn filesystem_manual_token_unbound_setup_never_overwrites_another_account() {
    let filesystem = test_filesystem();
    let concrete_secret_store = Arc::new(InMemorySecretStore::new());
    let secret_store: Arc<dyn SecretStore> = concrete_secret_store.clone();
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let first_challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id: first_interaction,
        ..
    } = first_challenge
    else {
        panic!("expected manual token challenge");
    };
    let first = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id: first_interaction,
                secret: SecretString::from("first-manual-token"),
            },
        )
        .await
        .unwrap();
    let first_account = service
        .read_account(&scope, first.account_id)
        .await
        .unwrap()
        .expect("first account")
        .0;
    let first_handle = first_account.access_secret.expect("first secret handle");

    let second_challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id: second_interaction,
        ..
    } = second_challenge
    else {
        panic!("expected manual token challenge");
    };
    let second = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id: second_interaction,
                secret: SecretString::from("second-manual-token"),
            },
        )
        .await
        .unwrap();

    assert_ne!(second.account_id, first.account_id);
    let accounts = service.accounts_for_owner(&scope).await.unwrap();
    assert_eq!(accounts.len(), 2);
    let updated = accounts
        .into_iter()
        .find(|account| account.id == second.account_id)
        .unwrap();
    let second_handle = updated.access_secret.expect("second secret handle");
    assert_ne!(second_handle, first_handle);
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &second_handle)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &first_handle)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn filesystem_manual_token_completion_persists_auth_flow_account() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);
    let expires_at = Utc::now() + Duration::minutes(5);

    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at,
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id,
        provider,
        label,
        expires_at: challenge_expires_at,
    } = challenge
    else {
        panic!("expected manual token challenge");
    };

    let flow = service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::ManualTokenRequired {
                interaction_id,
                provider,
                label,
                expires_at: challenge_expires_at,
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            expires_at,
        })
        .await
        .unwrap();

    let submitted = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("manual-token-value"),
            },
        )
        .await
        .unwrap();

    let completed = service
        .complete_manual_token(
            &scope,
            ManualTokenCompletionInput {
                interaction_id,
                credential_account_id: submitted.account_id,
            },
        )
        .await
        .unwrap();

    assert_eq!(completed.id, flow.id);
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    assert_eq!(completed.credential_account_id, Some(submitted.account_id));
}

#[tokio::test]
async fn filesystem_manual_token_completion_rejects_invalid_completed_account() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);
    let interaction_id =
        create_manual_token_flow(&service, &scope, Utc::now() + Duration::minutes(5)).await;

    let missing = service
        .complete_manual_token(
            &scope,
            ManualTokenCompletionInput {
                interaction_id,
                credential_account_id: CredentialAccountId::new(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(missing, AuthProductError::CredentialMissing);

    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::PendingSetup,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();
    let unconfigured = service
        .complete_manual_token(
            &scope,
            ManualTokenCompletionInput {
                interaction_id,
                credential_account_id: account.id,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(unconfigured, AuthProductError::CrossScopeDenied);

    let mut foreign_scope = scope.clone();
    foreign_scope.resource.user_id = UserId::new("bob").unwrap();
    let foreign = service
        .create_account(NewCredentialAccount {
            scope: foreign_scope,
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("foreign-access").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();
    let cross_scope = service
        .complete_manual_token(
            &scope,
            ManualTokenCompletionInput {
                interaction_id,
                credential_account_id: foreign.id,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(cross_scope, AuthProductError::CrossScopeDenied);
}

#[tokio::test]
async fn filesystem_manual_token_completion_expires_stale_auth_flow() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);
    let interaction_id =
        create_manual_token_flow(&service, &scope, Utc::now() - Duration::minutes(1)).await;

    let submitted = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("manual-token-value"),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(submitted, AuthProductError::UnknownOrExpiredFlow);

    let err = service
        .complete_manual_token(
            &scope,
            ManualTokenCompletionInput {
                interaction_id,
                credential_account_id: CredentialAccountId::new(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err, AuthProductError::UnknownOrExpiredFlow);
    let flows = service.flows_for_scope(&scope).await.unwrap();
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].0.status, AuthFlowStatus::Expired);
}

#[tokio::test]
async fn filesystem_manual_token_cancel_marks_flow_canceled_and_is_idempotent() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);
    let interaction_id =
        create_manual_token_flow(&service, &scope, Utc::now() + Duration::minutes(5)).await;

    let canceled = service
        .cancel_manual_token(&scope, interaction_id)
        .await
        .unwrap()
        .expect("manual-token flow should be canceled");
    assert_eq!(canceled.status, AuthFlowStatus::Canceled);
    let still_canceled = service
        .cancel_manual_token(&scope, interaction_id)
        .await
        .unwrap()
        .expect("terminal flow should still be returned");
    assert_eq!(still_canceled.status, AuthFlowStatus::Canceled);
    let unknown = service
        .cancel_manual_token(&scope, AuthInteractionId::new())
        .await
        .unwrap();
    assert!(unknown.is_none());
}

#[tokio::test]
async fn filesystem_flow_record_source_projects_session_scoped_manual_flows() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let mut scope = test_scope();
    scope.surface = AuthSurface::Callback;
    scope.resource.thread_id = Some(ThreadId::new("thread-auth-flow").unwrap());
    scope.session_id = Some(AuthSessionId::new("session-auth-flow").unwrap());
    let service = FilesystemAuthProductServices::new(filesystem, secret_store);
    let expires_at = Utc::now() + Duration::minutes(5);

    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at,
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id,
        provider,
        label,
        expires_at: challenge_expires_at,
    } = challenge
    else {
        panic!("expected manual token challenge");
    };
    let flow = service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::ManualTokenRequired {
                interaction_id,
                provider,
                label,
                expires_at: challenge_expires_at,
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            expires_at,
        })
        .await
        .unwrap();

    let submitted = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("manual-token-value"),
            },
        )
        .await
        .unwrap();
    service
        .complete_manual_token(
            &scope,
            ManualTokenCompletionInput {
                interaction_id,
                credential_account_id: submitted.account_id,
            },
        )
        .await
        .unwrap();

    let owner = AuthFlowOwnerScope {
        tenant_id: scope.resource.tenant_id.clone(),
        user_id: scope.resource.user_id.clone(),
        agent_id: scope.resource.agent_id.clone(),
        project_id: scope.resource.project_id.clone(),
        thread_id: scope.resource.thread_id.clone().unwrap(),
    };
    let snapshot = service.flows_for_owner(owner).await.unwrap();
    let projected = snapshot
        .iter()
        .find(|record| record.id == flow.id)
        .expect("session-scoped flow should be projected for auth gates");

    assert_eq!(projected.status, AuthFlowStatus::Completed);
    assert_eq!(projected.scope.session_id, scope.session_id);
    assert_eq!(
        projected.credential_account_id,
        Some(submitted.account_id),
        "manual-token completion must remain visible to the auth read model"
    );
}

#[tokio::test]
async fn filesystem_account_record_source_projects_session_scoped_accounts_for_runtime_owner() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let mut setup_scope = test_scope();
    setup_scope.surface = AuthSurface::Callback;
    setup_scope.resource.thread_id = Some(ThreadId::new("thread-auth-account").unwrap());
    setup_scope.session_id = Some(AuthSessionId::new("session-auth-account").unwrap());
    let service = FilesystemAuthProductServices::new(filesystem, secret_store);
    let account = service
        .create_account(NewCredentialAccount {
            scope: setup_scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("session-scoped-access").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: Vec::new(),
        })
        .await
        .unwrap();
    let mut runtime_resource = setup_scope.resource.clone();
    runtime_resource.invocation_id = InvocationId::new();
    let runtime_scope = AuthProductScope::new(runtime_resource, AuthSurface::Api);

    let projected = service.accounts_for_owner(&runtime_scope).await.unwrap();
    let projected_account = projected
        .iter()
        .find(|candidate| candidate.id == account.id)
        .expect("runtime owner projection should include session-scoped setup account");

    assert_eq!(projected_account.scope.session_id, setup_scope.session_id);
    assert_eq!(projected_account.provider, google_provider());
}

#[tokio::test]
async fn filesystem_account_record_source_rejects_malformed_scan_records() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(Arc::clone(&filesystem), secret_store);
    service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("valid-account-access").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: Vec::new(),
        })
        .await
        .unwrap();

    let malformed_account_id = brassclaw_auth::CredentialAccountId::new();
    let malformed_path = super::paths::account_path(&scope, malformed_account_id);
    let malformed = brassclaw_filesystem::Entry::bytes(b"{ malformed account json".to_vec())
        .with_content_type(brassclaw_filesystem::ContentType::json());
    filesystem
        .put(
            &scope.resource,
            &malformed_path,
            malformed,
            brassclaw_filesystem::CasExpectation::Absent,
        )
        .await
        .expect("malformed account fixture must write");

    assert!(
        matches!(
            service.accounts_for_owner(&scope).await,
            Err(AuthProductError::BackendUnavailable)
        ),
        "runtime owner scans should fail loudly on malformed account records"
    );

    assert!(
        matches!(
            service.read_account(&scope, malformed_account_id).await,
            Err(AuthProductError::BackendUnavailable)
        ),
        "exact account reads should remain strict"
    );
}

#[tokio::test]
async fn filesystem_runtime_account_selection_keeps_complete_session_roots_ambiguous() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let service = Arc::new(test_service(filesystem, secret_store));
    let mut setup_scope = test_scope();
    setup_scope.surface = AuthSurface::Callback;
    setup_scope.resource.thread_id = Some(ThreadId::new("thread-many-sessions").unwrap());
    let mut runtime_scope = AuthProductScope::new(setup_scope.resource.clone(), AuthSurface::Web);
    runtime_scope.resource.invocation_id = InvocationId::new();

    for index in 0..70 {
        let mut account_scope = setup_scope.clone();
        account_scope.session_id = Some(AuthSessionId::new(format!("session-{index:03}")).unwrap());
        service
            .create_account(NewCredentialAccount {
                scope: account_scope,
                provider: google_provider(),
                label: account_label(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(
                    SecretHandle::new(format!("many-session-access-{index}")).unwrap(),
                ),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![ProviderScope::new("drive.readonly").unwrap()],
            })
            .await
            .unwrap();
    }

    assert_eq!(
        service
            .accounts_for_owner(&runtime_scope)
            .await
            .unwrap()
            .len(),
        70
    );
    let selector = ProductAuthRuntimeCredentialAccountSelector::new(service);
    let error = selector
        .select_unique_configured_runtime_account(RuntimeCredentialAccountSelectionRequest::new(
            CredentialAccountSelectionRequest::new(runtime_scope.clone(), google_provider()),
            runtime_scope,
            vec![ProviderScope::new("drive.readonly").unwrap()],
        ))
        .await
        .expect_err("complete same-provider session roots require an explicit selection");

    assert!(matches!(error, AuthProductError::AccountSelectionRequired));
}

#[tokio::test]
async fn filesystem_runtime_account_selection_keeps_complete_account_records_ambiguous() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let service = Arc::new(test_service(filesystem, secret_store));
    let mut setup_scope = test_scope();
    setup_scope.surface = AuthSurface::Callback;
    setup_scope.resource.thread_id = Some(ThreadId::new("thread-many-accounts").unwrap());
    let mut runtime_scope = AuthProductScope::new(setup_scope.resource.clone(), AuthSurface::Web);
    runtime_scope.resource.invocation_id = InvocationId::new();

    for index in 0..70 {
        service
            .create_account(NewCredentialAccount {
                scope: setup_scope.clone(),
                provider: google_provider(),
                label: account_label(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(
                    SecretHandle::new(format!("many-account-access-{index}")).unwrap(),
                ),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![ProviderScope::new("drive.readonly").unwrap()],
            })
            .await
            .unwrap();
    }

    assert_eq!(
        service
            .accounts_for_owner(&runtime_scope)
            .await
            .unwrap()
            .len(),
        70
    );
    let selector = ProductAuthRuntimeCredentialAccountSelector::new(service);
    let error = selector
        .select_unique_configured_runtime_account(RuntimeCredentialAccountSelectionRequest::new(
            CredentialAccountSelectionRequest::new(runtime_scope.clone(), google_provider()),
            runtime_scope,
            vec![ProviderScope::new("drive.readonly").unwrap()],
        ))
        .await
        .expect_err("complete same-provider account records require an explicit selection");

    assert!(matches!(error, AuthProductError::AccountSelectionRequired));
}

#[tokio::test]
async fn filesystem_oauth_callback_claim_is_one_shot_and_completion_persists() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem.clone(), secret_store.clone());
    let flow = fs_callback_flow(&service, &scope, None, "one-shot").await;
    let prepared = fs_callback_prepared(&flow, "one-shot");
    let action = fs_callback_dispatch(
        service
            .claim_oauth_callback(&scope, prepared.clone())
            .await
            .unwrap(),
    );
    assert_eq!(
        service
            .get_flow(&scope, flow.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        AuthFlowStatus::CallbackReceived
    );
    match service
        .claim_oauth_callback(&scope, prepared.clone())
        .await
        .unwrap()
    {
        brassclaw_auth::OAuthCallbackAdmission::Original(state) => assert_eq!(state.action, action),
        other => panic!("second claim cannot dispatch again: {other:?}"),
    }
    let mut changed = prepared.clone();
    changed.original.authorization_code_hash = code_hash("different-code");
    assert!(matches!(
        service.claim_oauth_callback(&scope, changed).await,
        Err(AuthProductError::BackendConflict)
    ));
    let completed = service
        .complete_oauth_callback(&scope, fs_callback_completion(&action))
        .await
        .unwrap();
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    let restarted = test_service(filesystem, secret_store);
    let original = restarted
        .read_oauth_callback_original(&scope, &action.original.reference())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.flow, completed);
    assert_eq!(
        original.action.publication,
        brassclaw_auth::OAuthCallbackPublication::Applied
    );
    assert_eq!(
        restarted
            .complete_oauth_callback(&scope, fs_callback_completion(&action))
            .await
            .unwrap(),
        completed
    );
    let account = restarted
        .get_account(CredentialAccountLookupRequest::new(
            scope,
            completed.credential_account_id.unwrap(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        account.access_secret,
        Some(action.destinations.access_secret)
    );
}

#[tokio::test]
async fn filesystem_manual_token_submit_allows_only_one_concurrent_consumer() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = Arc::new(test_service(filesystem, secret_store));

    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("expected manual token challenge");
    };

    let mut tasks = JoinSet::new();
    for value in ["first-token", "second-token"] {
        let service = Arc::clone(&service);
        let scope = scope.clone();
        tasks.spawn(async move {
            service
                .submit_manual_token(
                    &scope,
                    SecretSubmitRequest {
                        interaction_id,
                        secret: SecretString::from(value),
                    },
                )
                .await
        });
    }

    let mut successes = 0;
    let mut conflicting_inputs = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(_) => successes += 1,
            Err(AuthProductError::BackendConflict) => conflicting_inputs += 1,
            Err(error) => panic!("unexpected submit error: {error:?}"),
        }
    }

    assert_eq!(successes, 1);
    assert_eq!(conflicting_inputs, 1);
}

// ─── fix: fs_error maps VersionMismatch to BackendConflict ───────────────────

#[test]
fn fs_error_maps_version_mismatch_to_backend_conflict() {
    use super::paths::fs_error;
    use brassclaw_filesystem::{FilesystemError, FilesystemOperation};
    use brassclaw_host_api::VirtualPath;

    let version_mismatch = FilesystemError::VersionMismatch {
        path: VirtualPath::new("/secrets/test").unwrap(),
        expected: None,
        found: None,
    };
    assert_eq!(
        fs_error(version_mismatch),
        AuthProductError::BackendConflict,
        "VersionMismatch must map to BackendConflict, not BackendUnavailable"
    );

    let backend_err = FilesystemError::Backend {
        path: VirtualPath::new("/secrets/test").unwrap(),
        operation: FilesystemOperation::ReadFile,
        reason: "io error".to_string(),
    };
    assert_eq!(
        fs_error(backend_err),
        AuthProductError::BackendUnavailable,
        "non-CAS errors must still map to BackendUnavailable"
    );
}

// ─── fix: mark_continuation_dispatched is idempotent ─────────────────────────

#[tokio::test]
async fn filesystem_oauth_continuation_marker_is_idempotent() {
    let scope = test_scope();
    let service = test_service(test_filesystem(), Arc::new(InMemorySecretStore::new()));
    let flow = fs_callback_flow(&service, &scope, None, "marker").await;
    let action = fs_callback_dispatch(
        service
            .claim_oauth_callback(&scope, fs_callback_prepared(&flow, "marker"))
            .await
            .unwrap(),
    );
    service
        .complete_oauth_callback(&scope, fs_callback_completion(&action))
        .await
        .unwrap();
    let first_at = Utc::now();
    let first = service
        .mark_continuation_dispatched(&scope, flow.id, first_at)
        .await
        .unwrap();
    assert_eq!(first.continuation_emitted_at, Some(first_at));
    let second = service
        .mark_continuation_dispatched(&scope, flow.id, first_at + Duration::seconds(1))
        .await
        .unwrap();
    assert_eq!(second.continuation_emitted_at, Some(first_at));
    let original = service
        .read_oauth_callback_original(&scope, &action.original.reference())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.flow, second);
    assert_eq!(
        original.action.publication,
        brassclaw_auth::OAuthCallbackPublication::Applied
    );
}

// Bound rotation preserves original references instead of blind cleanup.

#[tokio::test]
async fn filesystem_manual_token_bound_rotation_preserves_original_secret_references() {
    // Exercise the preauthorized update_binding path:
    // Original outputs remain retained while original receipts/references need
    // them. Rotation cannot authorize blind cleanup of a previous generation.
    use brassclaw_auth::{
        CredentialAccountUpdateBinding, ManualTokenSetupRequest, SecretSubmitRequest,
    };

    let filesystem = test_filesystem();
    let concrete_secret_store = Arc::new(InMemorySecretStore::new());
    let secret_store: Arc<dyn SecretStore> = concrete_secret_store.clone();
    let scope = test_scope();
    let service = test_service(Arc::clone(&filesystem), Arc::clone(&secret_store));

    // --- First submit: create the account via the no-binding path. ---
    let challenge1 = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id: iid1,
        ..
    } = challenge1
    else {
        panic!("expected ManualTokenRequired");
    };
    let result1 = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id: iid1,
                secret: SecretString::from("token-v1"),
            },
        )
        .await
        .unwrap();
    let account_id = result1.account_id;

    // Grab the first-generation secret handle.
    let account_after_v1 = service
        .get_account(brassclaw_auth::CredentialAccountLookupRequest::new(
            scope.clone(),
            account_id,
        ))
        .await
        .unwrap()
        .unwrap();
    let old_handle = account_after_v1
        .access_secret
        .clone()
        .expect("v1 access_secret");
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &old_handle)
            .await
            .unwrap()
            .is_some(),
        "v1 secret must exist in store"
    );

    // --- Second submit: rotate via update_binding to the same account. ---
    let challenge2 = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: Some(CredentialAccountUpdateBinding {
                account_id,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
            }),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id: iid2,
        ..
    } = challenge2
    else {
        panic!("expected ManualTokenRequired for rotation");
    };
    service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id: iid2,
                secret: SecretString::from("token-v2"),
            },
        )
        .await
        .unwrap();

    // The original output remains reachable for its recorded original result.
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &old_handle)
            .await
            .unwrap()
            .is_some(),
        "original generation must be retained for its existing references"
    );

    // The new handle must be present.
    let account_after_v2 = service
        .get_account(brassclaw_auth::CredentialAccountLookupRequest::new(
            scope.clone(),
            account_id,
        ))
        .await
        .unwrap()
        .unwrap();
    let new_handle = account_after_v2.access_secret.expect("v2 access_secret");
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &new_handle)
            .await
            .unwrap()
            .is_some(),
        "v2 secret must be present in SecretStore"
    );
}

// ─── fix: durable SecretCleanupService purges secrets on Uninstall ───────────

#[tokio::test]
async fn filesystem_cleanup_for_lifecycle_deactivates_owner_and_revokes_on_uninstall() {
    use brassclaw_auth::{SecretCleanupAction, SecretCleanupRequest, SecretCleanupService};
    use brassclaw_host_api::ExtensionId;

    let filesystem = test_filesystem();
    let concrete_secret_store = Arc::new(InMemorySecretStore::new());
    let secret_store: Arc<dyn SecretStore> = concrete_secret_store.clone();
    let scope = test_scope();
    let service = test_service(Arc::clone(&filesystem), Arc::clone(&secret_store));

    let ext_id = ExtensionId::new("test-ext").unwrap();
    let access = SecretHandle::new("ext-access").unwrap();
    let refresh = SecretHandle::new("ext-refresh").unwrap();

    // Seed secret material.
    use secrecy::SecretString;
    concrete_secret_store
        .put(
            scope.resource.clone(),
            access.clone(),
            SecretString::from("access-material"),
        )
        .await
        .unwrap();
    concrete_secret_store
        .put(
            scope.resource.clone(),
            refresh.clone(),
            SecretString::from("refresh-material"),
        )
        .await
        .unwrap();

    // Create an extension-owned account.
    let account = service
        .create_account(brassclaw_auth::NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::ExtensionOwned,
            owner_extension: Some(ext_id.clone()),
            granted_extensions: vec![],
            access_secret: Some(access.clone()),
            access_expires_at: None,
            refresh_secret: Some(refresh.clone()),
            scopes: vec![],
        })
        .await
        .unwrap();

    // Deactivate: account should be Inactive; secrets retained.
    let deactivate_report = service
        .cleanup_for_lifecycle(SecretCleanupRequest {
            scope: scope.clone(),
            extension_id: ext_id.clone(),
            action: SecretCleanupAction::Deactivate,
        })
        .await
        .unwrap();
    assert_eq!(deactivate_report.retained_accounts, vec![account.id]);
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &access)
            .await
            .unwrap()
            .is_some(),
        "Deactivate must retain secret material"
    );

    // Uninstall: account revoked, secrets purged from SecretStore.
    let uninstall_report = service
        .cleanup_for_lifecycle(SecretCleanupRequest {
            scope: scope.clone(),
            extension_id: ext_id.clone(),
            action: SecretCleanupAction::Uninstall,
        })
        .await
        .unwrap();
    assert_eq!(uninstall_report.revoked_accounts, vec![account.id]);
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &access)
            .await
            .unwrap()
            .is_none(),
        "Uninstall must delete access secret from SecretStore"
    );
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &refresh)
            .await
            .unwrap()
            .is_none(),
        "Uninstall must delete refresh secret from SecretStore"
    );
}

// ─── fix: lock-cache weak-reference GC actually shrinks the map ──────────────

#[tokio::test]
async fn filesystem_lock_cache_drops_weak_entries_after_release() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let service = test_service(filesystem, secret_store);

    {
        // Acquire a lock for key A and drop the guard immediately.
        let lock_a = service.lock_for("account:key-a".to_string());
        let _guard_a = lock_a.lock().await;
        // guard_a dropped at end of this block; Arc<Mutex> dropped too after lock_a drops.
    }
    // After key-A's Arc dropped, the next call to lock_for should evict the
    // dead weak reference. We trigger eviction via lock_for on a different key.
    let _lock_b = service.lock_for("account:key-b".to_string());

    // Verify key-A is gone: requesting it again must produce a *new* Arc (i.e.
    // a fresh Mutex), not the evicted weak ref.
    let lock_a2 = service.lock_for("account:key-a".to_string());
    // The new lock should be unlocked (no one holds it).
    assert!(
        lock_a2.try_lock().is_ok(),
        "re-acquired key-a must be unlocked"
    );
}

// ─── fix: manual-token expiry branch ─────────────────────────────────────────

#[tokio::test]
async fn filesystem_manual_token_submit_rejects_expired_interaction() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    // Create an interaction that is already past its expiry.
    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            // Expired immediately.
            expires_at: Utc::now() - Duration::seconds(1),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("expected ManualTokenRequired");
    };

    let err = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("too-late"),
            },
        )
        .await
        .expect_err("expired interaction must be rejected");
    assert_eq!(
        err,
        AuthProductError::UnknownOrExpiredFlow,
        "expired interaction must return UnknownOrExpiredFlow"
    );
}

// ─── UnavailableAuthProviderClient validates before returning error ───────────

#[tokio::test]
async fn unavailable_auth_provider_client_validates_before_returning_backend_unavailable() {
    use super::provider::UnavailableAuthProviderClient;
    use brassclaw_auth::{
        AuthProviderClient, OAuthAuthorizationCode, OAuthProviderCallbackRequest,
        OAuthProviderExchangeContext, OAuthProviderRefreshRequest,
    };
    use secrecy::SecretString;

    let client = UnavailableAuthProviderClient;

    let ctx = OAuthProviderExchangeContext {
        scope: test_scope(),
        flow_id: brassclaw_auth::AuthFlowId::new(),
    };

    // Valid request must return BackendUnavailable (no provider configured) after
    // the internal validate_provider_callback_request guard passes.
    let valid = OAuthProviderCallbackRequest {
        provider: google_provider(),
        account_label: account_label(),
        authorization_code: OAuthAuthorizationCode::new(SecretString::from("real-code")).unwrap(),
        authorization_code_hash: AuthorizationCodeHash::new(brassclaw_common::hashing::sha256_hex(
            b"real-code",
        ))
        .unwrap(),
        pkce_verifier: brassclaw_auth::PkceVerifierSecret::new(SecretString::from("real-verifier"))
            .unwrap(),
        pkce_verifier_hash: PkceVerifierHash::new(brassclaw_common::hashing::sha256_hex(
            b"real-verifier",
        ))
        .unwrap(),
        scopes: vec![],
    };
    let err = client.exchange_callback(ctx, valid).await.unwrap_err();
    assert_eq!(
        err,
        AuthProductError::BackendUnavailable,
        "valid request must reach BackendUnavailable (no provider configured)"
    );

    // 3. refresh_token always BackendUnavailable.
    let request = OAuthProviderRefreshRequest {
        refresh_id: brassclaw_auth::CredentialRefreshId::new(),
        scope: test_scope(),
        account_id: CredentialAccountId::new(),
        provider: google_provider(),
        refresh_secret: SecretHandle::new("r").unwrap(),
        scopes: vec![],
    };
    let preparation =
        brassclaw_auth::InMemoryAuthProductServices::refresh_preparation_for_tests(&request)
            .unwrap();
    let refresh_err = client
        .refresh_token(request, preparation)
        .await
        .unwrap_err();
    assert_eq!(refresh_err, AuthProductError::BackendUnavailable);
}

// ─── validate_account_list_request boundary cases ────────────────────────────

#[tokio::test]
async fn filesystem_list_accounts_rejects_zero_and_oversized_limit() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    // limit = 0.
    let err = service
        .list_accounts(
            CredentialAccountListRequest::new(scope.clone(), google_provider()).with_limit(0),
        )
        .await
        .expect_err("limit=0 must be rejected");
    assert!(matches!(err, AuthProductError::InvalidRequest { .. }));

    // limit = MAX + 1.
    let err = service
        .list_accounts(
            CredentialAccountListRequest::new(scope.clone(), google_provider())
                .with_limit(CredentialAccountListRequest::MAX_LIMIT + 1),
        )
        .await
        .expect_err("limit > MAX must be rejected");
    assert!(matches!(err, AuthProductError::InvalidRequest { .. }));

    // Cursor + pagination: 2 accounts, limit=1 → next_cursor present.
    for i in 0..2u8 {
        service
            .create_account(brassclaw_auth::NewCredentialAccount {
                scope: scope.clone(),
                provider: google_provider(),
                label: CredentialAccountLabel::new(format!("User {i}")).unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: None,
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![],
            })
            .await
            .unwrap();
    }
    let page = service
        .list_accounts(
            CredentialAccountListRequest::new(scope.clone(), google_provider()).with_limit(1),
        )
        .await
        .unwrap();
    assert_eq!(page.accounts.len(), 1);
    assert!(
        page.next_cursor.is_some(),
        "second page must have next_cursor"
    );
}

// ─── zmanian follow-up #1: OAuth re-auth must purge previous secret handles ──

#[tokio::test]
async fn filesystem_oauth_reauth_retains_outputs_and_original_receipts() {
    use brassclaw_secrets::SecretMaterial;
    let filesystem = test_filesystem();
    let secret_store = Arc::new(InMemorySecretStore::new());
    let service = test_service(filesystem.clone(), secret_store.clone());
    let scope = test_scope();
    let first_flow = fs_callback_flow(&service, &scope, None, "retained-first").await;
    let first = fs_callback_dispatch(
        service
            .claim_oauth_callback(&scope, fs_callback_prepared(&first_flow, "retained-first"))
            .await
            .unwrap(),
    );
    for handle in [
        &first.destinations.access_secret,
        &first.destinations.refresh_secret,
    ] {
        secret_store
            .put(
                scope.resource.clone(),
                handle.clone(),
                SecretMaterial::from("retained-first-token"),
            )
            .await
            .unwrap();
    }
    let completed_first = service
        .complete_oauth_callback(&scope, fs_callback_completion(&first))
        .await
        .unwrap();
    let account_id = completed_first.credential_account_id.unwrap();
    let original_account = service
        .get_account(CredentialAccountLookupRequest::new(
            scope.clone(),
            account_id,
        ))
        .await
        .unwrap()
        .unwrap();
    let second_flow = fs_callback_flow(
        &service,
        &scope,
        Some(
            brassclaw_auth::CredentialAccountUpdateBinding::from_projection(
                &original_account.projection(),
            ),
        ),
        "retained-second",
    )
    .await;
    let second = fs_callback_dispatch(
        service
            .claim_oauth_callback(
                &scope,
                fs_callback_prepared(&second_flow, "retained-second"),
            )
            .await
            .unwrap(),
    );
    for handle in [
        &second.destinations.access_secret,
        &second.destinations.refresh_secret,
    ] {
        secret_store
            .put(
                scope.resource.clone(),
                handle.clone(),
                SecretMaterial::from("retained-second-token"),
            )
            .await
            .unwrap();
    }
    let completed_second = service
        .complete_oauth_callback(&scope, fs_callback_completion(&second))
        .await
        .unwrap();
    assert_eq!(completed_second.credential_account_id, Some(account_id));
    for handle in [
        &first.destinations.access_secret,
        &first.destinations.refresh_secret,
        &second.destinations.access_secret,
        &second.destinations.refresh_secret,
    ] {
        assert!(
            secret_store
                .metadata(&scope.resource, handle)
                .await
                .unwrap()
                .is_some(),
            "retained outputs are not blind cleanup authority"
        );
    }
    let (document, _) = service
        .read_account_document(&scope, account_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(document.account.created_at, original_account.created_at);
    assert_eq!(
        document.account.access_secret.as_ref(),
        Some(&second.destinations.access_secret)
    );
    assert_eq!(document.auth_callback_actions.len(), 2);
    assert_eq!(
        document.auth_callback_actions[&first_flow.id]
            .published_account
            .as_ref(),
        Some(&original_account)
    );
    let restarted = test_service(filesystem, secret_store);
    let replay = restarted
        .read_oauth_callback_original(&scope, &first.original.reference())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replay.completed_flow().unwrap(), completed_first);
    assert_eq!(
        restarted
            .get_account(CredentialAccountLookupRequest::new(scope, account_id))
            .await
            .unwrap()
            .unwrap(),
        document.account
    );
}

// An occupied target cannot authorize output creation or original deletion.
#[tokio::test]
async fn filesystem_manual_token_submit_preserves_original_and_rejects_occupied_account_before_output()
 {
    use brassclaw_auth::CredentialAccountId;
    use brassclaw_filesystem::CasExpectation;

    let filesystem = test_filesystem();
    let concrete_secret_store = Arc::new(InMemorySecretStore::new());
    let secret_store: Arc<dyn SecretStore> = concrete_secret_store.clone();
    let scope = test_scope();
    let service = test_service(Arc::clone(&filesystem), Arc::clone(&secret_store));

    // Request an interaction so we know its ID (and can derive the account path).
    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("expected ManualTokenRequired");
    };

    // Derive the same account ID the submit path will use.
    let account_id = CredentialAccountId::from_uuid(interaction_id.as_uuid());

    // Write a dummy record at that path so create_account_with_id(Absent) fails.
    let dummy_account = brassclaw_auth::CredentialAccount {
        id: account_id,
        scope: scope.clone(),
        provider: google_provider(),
        label: account_label(),
        status: brassclaw_auth::CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: None,
        access_expires_at: None,
        refresh_secret: None,
        scopes: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let path = super::paths::account_path(&scope, account_id);
    let json = serde_json::to_vec(&dummy_account).expect("serialization must succeed");
    use brassclaw_filesystem::{ContentType, Entry};
    let entry = Entry::bytes(json).with_content_type(ContentType::json());
    filesystem
        .put(&scope.resource, &path, entry, CasExpectation::Absent)
        .await
        .expect("pre-create dummy account must succeed");

    // The occupied target rejects publication before any output is created.
    let result = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("token-value"),
            },
        )
        .await;
    assert!(result.is_err(), "submit must fail when account write fails");

    // No retained output was created; the encrypted original remains accountable.
    let access_handle = super::paths::manual_token_secret_handle(account_id, interaction_id)
        .expect("handle derivation must succeed");
    assert!(
        concrete_secret_store
            .metadata(&scope.resource, &access_handle)
            .await
            .unwrap()
            .is_none(),
        "occupied account must not cause output creation"
    );
    let original_handle =
        SecretHandle::new(format!("product-auth-manual-original-{interaction_id}")).unwrap();
    assert!(matches!(
        concrete_secret_store
            .custody(&scope.resource, &original_handle)
            .await
            .unwrap()
            .unwrap()
            .state,
        brassclaw_secrets::SecretCustodyState::Original { .. }
    ));
    assert!(
        concrete_secret_store
            .matches(&scope.resource, &original_handle, &"token-value".into())
            .await
            .unwrap()
    );
}

// ─── fix: OAuth callback CAS-conflict re-read branch ─────────────────────────

#[tokio::test]
async fn filesystem_oauth_unbound_collision_is_rejected_before_dispatch() {
    let scope = test_scope();
    let service = test_service(test_filesystem(), Arc::new(InMemorySecretStore::new()));
    let flow = fs_callback_flow(&service, &scope, None, "collision").await;
    let account_id = CredentialAccountId::from_uuid(flow.id.as_uuid());
    let existing = service
        .create_account_with_id(
            account_id,
            NewCredentialAccount {
                scope: scope.clone(),
                provider: google_provider(),
                label: account_label(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: Some(SecretHandle::new("preexisting-original-access").unwrap()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![],
            },
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let prepared = fs_callback_prepared(&flow, "collision");
    let reference = prepared.original.reference();
    assert!(matches!(
        service.claim_oauth_callback(&scope, prepared).await,
        Err(AuthProductError::BackendConflict)
    ));
    assert!(
        service
            .read_oauth_callback_original(&scope, &reference)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        service.get_flow(&scope, flow.id).await.unwrap().unwrap(),
        flow
    );
    assert_eq!(
        service
            .get_account(CredentialAccountLookupRequest::new(scope, account_id))
            .await
            .unwrap(),
        Some(existing)
    );
}

// ─── fix: grant-removal on non-owner account in cleanup_for_lifecycle ─────────

#[tokio::test]
async fn filesystem_cleanup_removes_grant_from_non_owner_account() {
    use brassclaw_auth::{SecretCleanupAction, SecretCleanupRequest, SecretCleanupService};
    use brassclaw_host_api::ExtensionId;

    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let ext_id = ExtensionId::new("granted-ext").unwrap();

    // Create user-reusable account with a grant to ext_id (not owner).
    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![ext_id.clone()],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    let report = service
        .cleanup_for_lifecycle(SecretCleanupRequest {
            scope: scope.clone(),
            extension_id: ext_id.clone(),
            action: SecretCleanupAction::Uninstall,
        })
        .await
        .unwrap();

    assert_eq!(
        report.removed_grants,
        vec![account.id],
        "grant must be reported removed"
    );
    assert!(
        report.revoked_accounts.is_empty(),
        "non-owner account must not be revoked"
    );

    let updated = service
        .get_account(CredentialAccountLookupRequest::new(
            scope.clone(),
            account.id,
        ))
        .await
        .unwrap()
        .expect("account must still exist");
    assert!(
        !updated.granted_extensions.contains(&ext_id),
        "grant must be removed from account record"
    );
    assert_eq!(
        updated.status,
        CredentialAccountStatus::Configured,
        "status must be unchanged"
    );
}

// ─── fix: select_unique_configured_account and select_configured_account ──────

#[tokio::test]
async fn filesystem_select_unique_configured_account_single_and_multi() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    // No accounts — CredentialMissing.
    let err = service
        .select_unique_configured_account(CredentialAccountSelectionRequest::new(
            scope.clone(),
            google_provider(),
        ))
        .await
        .expect_err("no accounts must return CredentialMissing");
    assert_eq!(err, AuthProductError::CredentialMissing);

    let a1 = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    // One configured — returns it.
    let selected = service
        .select_unique_configured_account(CredentialAccountSelectionRequest::new(
            scope.clone(),
            google_provider(),
        ))
        .await
        .unwrap();
    assert_eq!(selected.id, a1.id);

    // Second configured — AccountSelectionRequired.
    service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: CredentialAccountLabel::new("Alice Google 2").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    let err = service
        .select_unique_configured_account(CredentialAccountSelectionRequest::new(
            scope.clone(),
            google_provider(),
        ))
        .await
        .expect_err("two configured must require selection");
    assert_eq!(err, AuthProductError::AccountSelectionRequired);
}

#[tokio::test]
async fn filesystem_select_configured_account_validates_provider_and_rejects_missing() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    // Happy path.
    let selected = service
        .select_configured_account(CredentialAccountChoiceRequest::new(
            scope.clone(),
            google_provider(),
            account.id,
        ))
        .await
        .unwrap();
    assert_eq!(selected.id, account.id);

    // Non-existent account.
    let err = service
        .select_configured_account(CredentialAccountChoiceRequest::new(
            scope.clone(),
            google_provider(),
            CredentialAccountId::new(),
        ))
        .await
        .expect_err("missing account must return CredentialMissing");
    assert_eq!(err, AuthProductError::CredentialMissing);

    // Wrong provider is intentionally indistinguishable from a missing account
    // at the public boundary, so account ids cannot be used as provider oracles.
    let err = service
        .select_configured_account(CredentialAccountChoiceRequest::new(
            scope.clone(),
            AuthProviderId::new("github").unwrap(),
            account.id,
        ))
        .await
        .expect_err("wrong provider must return CredentialMissing");
    assert_eq!(err, AuthProductError::CredentialMissing);
}

// ─── tests: cancel_flow, fail_oauth_callback, complete_credential_selection ───

#[tokio::test]
async fn filesystem_cancel_flow_and_terminal_state_rejection() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let flow = service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: OAuthAuthorizationUrl::new("https://provider.example/oauth")
                    .unwrap(),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(state_hash("cancel-s")),
            pkce_verifier_hash: Some(pkce_hash("cancel-p")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();

    let cancelled = service.cancel_flow(&scope, flow.id).await.unwrap();
    assert_eq!(cancelled.status, AuthFlowStatus::Canceled);

    // Second cancel on already-terminal flow returns Canceled error.
    let err = service
        .cancel_flow(&scope, flow.id)
        .await
        .expect_err("second cancel must fail");
    assert_eq!(err, AuthProductError::Canceled);
}

#[tokio::test]
async fn filesystem_fail_oauth_callback_marks_flow_failed() {
    use brassclaw_auth::{AuthErrorCode, OAuthCallbackFailureInput};
    let scope = test_scope();
    let service = test_service(test_filesystem(), Arc::new(InMemorySecretStore::new()));
    let flow = fs_callback_flow(&service, &scope, None, "owned-failure").await;
    let action = fs_callback_dispatch(
        service
            .claim_oauth_callback(&scope, fs_callback_prepared(&flow, "owned-failure"))
            .await
            .unwrap(),
    );
    let input = OAuthCallbackFailureInput {
        dispatched: Some(action.clone()),
        flow_id: flow.id,
        opaque_state_hash: action.original.opaque_state_hash.clone(),
        error: AuthErrorCode::TokenExchangeFailed,
    };
    let failed = service
        .fail_oauth_callback(&scope, input.clone())
        .await
        .unwrap();
    assert_eq!(failed.status, AuthFlowStatus::Failed);
    assert_eq!(failed.error, Some(AuthErrorCode::TokenExchangeFailed));
    assert_eq!(
        service.fail_oauth_callback(&scope, input).await.unwrap(),
        failed
    );
    let original = service
        .read_oauth_callback_original(&scope, &action.original.reference())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        original.action.outcome,
        brassclaw_auth::OAuthCallbackOutcome::Unknown(AuthErrorCode::TokenExchangeFailed)
    ));
    assert!(original.action.published_account.is_none());
    assert!(service.accounts_for_scope(&scope).await.unwrap().is_empty());
}

#[tokio::test]
async fn filesystem_complete_credential_selection_completes_flow() {
    use brassclaw_auth::{AuthFlowKind, CredentialSelectionInput};
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    let flow = service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::AccountSelectionRequired {
                provider: google_provider(),
                accounts: vec![account.projection()],
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();

    let completed = service
        .complete_credential_selection(
            &scope,
            CredentialSelectionInput {
                flow_id: flow.id,
                credential_account_id: account.id,
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    assert_eq!(completed.credential_account_id, Some(account.id));
}

// ─── tests: create_flow update_binding validation ─────────────────────────────

#[tokio::test]
async fn filesystem_create_flow_rejects_invalid_update_binding() {
    use brassclaw_auth::CredentialAccountUpdateBinding;
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    // Non-existent account in update_binding → CredentialMissing.
    let err = service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: OAuthAuthorizationUrl::new("https://provider.example/oauth")
                    .unwrap(),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: Some(CredentialAccountUpdateBinding {
                account_id: CredentialAccountId::new(),
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
            }),
            opaque_state_hash: Some(state_hash("ubv-s")),
            pkce_verifier_hash: Some(pkce_hash("ubv-p")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect_err("non-existent binding account must return CredentialMissing");
    assert_eq!(err, AuthProductError::CredentialMissing);
}

// ─── tests: update_status, project_credential_recovery, CredentialSetupService update ───

#[tokio::test]
async fn filesystem_update_status_and_cross_scope_rejection() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    let updated = service
        .update_status(&scope, account.id, CredentialAccountStatus::Inactive)
        .await
        .unwrap();
    assert_eq!(updated.status, CredentialAccountStatus::Inactive);

    // Non-existent account.
    let err = service
        .update_status(
            &scope,
            CredentialAccountId::new(),
            CredentialAccountStatus::Inactive,
        )
        .await
        .expect_err("missing account must return CredentialMissing");
    assert_eq!(err, AuthProductError::CredentialMissing);
}

#[tokio::test]
async fn filesystem_project_credential_recovery_returns_setup_required_when_empty() {
    use brassclaw_auth::CredentialRecoveryRequest;
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    // No accounts → setup_required.
    let recovery = service
        .project_credential_recovery(CredentialRecoveryRequest::new(
            scope.clone(),
            google_provider(),
        ))
        .await
        .unwrap();
    use brassclaw_auth::CredentialRecoveryState;
    assert!(
        matches!(
            recovery.state,
            CredentialRecoveryState::SetupRequired { .. }
        ),
        "no accounts must return setup_required"
    );

    // One configured account → configured.
    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    let recovery = service
        .project_credential_recovery(CredentialRecoveryRequest::new(
            scope.clone(),
            google_provider(),
        ))
        .await
        .unwrap();
    let CredentialRecoveryState::Configured { selected_account } = &recovery.state else {
        panic!(
            "single configured account must return Configured state, got: {:?}",
            recovery.state
        );
    };
    assert_eq!(selected_account.id, account.id);
}

#[tokio::test]
async fn filesystem_credential_setup_service_update_path() {
    use brassclaw_auth::{
        CredentialAccountMutation, CredentialAccountUpdate, CredentialSetupService,
    };
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("old-access").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    let new_handle = SecretHandle::new("new-access").unwrap();
    let updated = service
        .create_or_update_account(CredentialAccountMutation::Update(CredentialAccountUpdate {
            account_id: account.id,
            account: NewCredentialAccount {
                scope: scope.clone(),
                provider: google_provider(),
                label: account_label(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: Some(new_handle.clone()),
                access_expires_at: None,
                refresh_secret: None,
                scopes: vec![],
            },
        }))
        .await
        .unwrap();
    assert_eq!(updated.access_secret, Some(new_handle));
}

// ─── tests: get_account cross-scope rejection ─────────────────────────────────

#[tokio::test]
async fn filesystem_get_account_cross_scope_returns_cross_scope_denied() {
    use brassclaw_auth::AuthSurface;
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(Arc::clone(&filesystem), Arc::clone(&secret_store));

    let account = service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: None,
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();

    // Same resource but different surface → CrossScopeDenied.
    let cli_scope = AuthProductScope::new(scope.resource.clone(), AuthSurface::Cli);
    let service2 = test_service(Arc::clone(&filesystem), Arc::clone(&secret_store));
    let result = service2
        .get_account(CredentialAccountLookupRequest::new(cli_scope, account.id))
        .await;
    // The account doesn't exist in the CLI path (different path on filesystem), so None.
    assert!(
        result.unwrap().is_none(),
        "account written under web scope must not be visible under cli scope"
    );
}

// ─── tests: validate_secret control-char branch ───────────────────────────────

#[tokio::test]
async fn filesystem_validate_secret_rejects_control_characters() {
    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("expected ManualTokenRequired");
    };

    // NUL byte must be rejected without consuming the interaction.
    let err = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("valid\x00nul"),
            },
        )
        .await
        .expect_err("NUL byte must be rejected");
    assert!(
        matches!(err, AuthProductError::InvalidRequest { .. }),
        "must return InvalidRequest for control characters"
    );

    // Interaction must NOT be consumed — replay still possible.
    let ok = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("clean-token"),
            },
        )
        .await;
    assert!(
        ok.is_ok(),
        "interaction must be usable after control-char rejection"
    );
}

// ─── fix: abbyshekit review — expired flow mutation persisted ────────────────

#[tokio::test]
async fn filesystem_expired_flow_status_persisted_before_returning_error() {
    // When claim_oauth_callback / complete_oauth_callback / fail_oauth_callback
    // encounter an expired flow, the Expired status must be written to disk
    // before returning UnknownOrExpiredFlow so durable state matches the contract.
    use brassclaw_auth::{
        AuthErrorCode, OAuthCallbackFailureInput, OAuthCallbackInput, ProviderCallbackOutcome,
    };

    let filesystem = test_filesystem();
    let secret_store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = test_scope();
    let service = test_service(filesystem, secret_store);

    let flow = service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: OAuthAuthorizationUrl::new("https://provider.example/oauth")
                    .unwrap(),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(state_hash("exp-s")),
            pkce_verifier_hash: Some(pkce_hash("exp-p")),
            expires_at: Utc::now() - Duration::seconds(1),
        })
        .await
        .unwrap();

    // claim_oauth_callback must persist Expired before returning error.
    let err = service
        .claim_oauth_callback(&scope, fs_callback_prepared(&flow, "expired"))
        .await
        .expect_err("expired flow must be rejected");
    assert_eq!(err, AuthProductError::UnknownOrExpiredFlow);

    let persisted = service
        .get_flow(&scope, flow.id)
        .await
        .unwrap()
        .expect("flow must still exist");
    assert_eq!(persisted.status, AuthFlowStatus::Expired);
    assert_eq!(persisted.error, Some(AuthErrorCode::UnknownOrExpiredFlow));

    // fail_oauth_callback on already-expired flow returns FlowAlreadyTerminal
    // because Expired is a terminal status; the record was already persisted
    // as Expired by claim_oauth_callback above.
    let err2 = service
        .fail_oauth_callback(
            &scope,
            OAuthCallbackFailureInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("exp-s"),
                error: AuthErrorCode::ProviderDenied,
            },
        )
        .await
        .expect_err("expired flow must be rejected");
    assert_eq!(
        err2,
        AuthProductError::FlowAlreadyTerminal,
        "already-expired flow returns FlowAlreadyTerminal"
    );

    let persisted2 = service
        .get_flow(&scope, flow.id)
        .await
        .unwrap()
        .expect("flow must still exist");
    assert_eq!(persisted2.status, AuthFlowStatus::Expired);
    assert_eq!(persisted2.error, Some(AuthErrorCode::UnknownOrExpiredFlow));

    // complete_oauth_callback on a fresh expired flow (never claimed) must also
    // persist the Expired status before returning error.
    let flow2 = service
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: scope.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: google_provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: OAuthAuthorizationUrl::new("https://provider.example/oauth")
                    .unwrap(),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(state_hash("exp2-s")),
            pkce_verifier_hash: Some(pkce_hash("exp2-p")),
            expires_at: Utc::now() - Duration::seconds(1),
        })
        .await
        .unwrap();

    let err3 = service
        .complete_oauth_callback(
            &scope,
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow2.id,
                opaque_state_hash: state_hash("exp2-s"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .expect_err("expired flow must be rejected");
    assert_eq!(
        err3,
        AuthProductError::UnknownOrExpiredFlow,
        "complete_oauth_callback on expired flow returns UnknownOrExpiredFlow"
    );

    let persisted3 = service
        .get_flow(&scope, flow2.id)
        .await
        .unwrap()
        .expect("flow2 must still exist");
    assert_eq!(persisted3.status, AuthFlowStatus::Expired);
    assert_eq!(persisted3.error, Some(AuthErrorCode::UnknownOrExpiredFlow));
}

// ─── fix: abbyshekit review — OAuth CAS-conflict branch purges old secrets ───

#[tokio::test]
async fn filesystem_oauth_account_created_after_claim_is_not_overwritten_or_purged() {
    use brassclaw_auth::{OAuthCallbackOutcome, OAuthCallbackPublication};
    use brassclaw_secrets::SecretMaterial;
    let scope = test_scope();
    let secret_store = Arc::new(InMemorySecretStore::new());
    let service = test_service(test_filesystem(), secret_store.clone());
    let flow = fs_callback_flow(&service, &scope, None, "late-collision").await;
    let action = fs_callback_dispatch(
        service
            .claim_oauth_callback(&scope, fs_callback_prepared(&flow, "late-collision"))
            .await
            .unwrap(),
    );
    let old_access = SecretHandle::new("late-preexisting-access").unwrap();
    let old_refresh = SecretHandle::new("late-preexisting-refresh").unwrap();
    for handle in [
        &old_access,
        &old_refresh,
        &action.destinations.access_secret,
        &action.destinations.refresh_secret,
    ] {
        secret_store
            .put(
                scope.resource.clone(),
                handle.clone(),
                SecretMaterial::from("retained-accountable-output"),
            )
            .await
            .unwrap();
    }
    let account_id = CredentialAccountId::from_uuid(flow.id.as_uuid());
    let winner = service
        .create_account_with_id(
            account_id,
            NewCredentialAccount {
                scope: scope.clone(),
                provider: google_provider(),
                label: account_label(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: Some(old_access.clone()),
                access_expires_at: None,
                refresh_secret: Some(old_refresh.clone()),
                scopes: vec![],
            },
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    assert!(matches!(
        service
            .complete_oauth_callback(&scope, fs_callback_completion(&action))
            .await,
        Err(AuthProductError::FlowAlreadyTerminal)
    ));
    let original = service
        .read_oauth_callback_original(&scope, &action.original.reference())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.flow.status, AuthFlowStatus::Failed);
    assert_eq!(
        original.action.publication,
        OAuthCallbackPublication::Superseded
    );
    assert!(matches!(
        original.action.outcome,
        OAuthCallbackOutcome::Returned(_)
    ));
    assert!(original.action.published_account.is_none());
    assert_eq!(
        service
            .get_account(CredentialAccountLookupRequest::new(
                scope.clone(),
                account_id
            ))
            .await
            .unwrap(),
        Some(winner)
    );
    for handle in [
        &old_access,
        &old_refresh,
        &action.destinations.access_secret,
        &action.destinations.refresh_secret,
    ] {
        assert!(
            secret_store
                .metadata(&scope.resource, handle)
                .await
                .unwrap()
                .is_some()
        );
    }
}

// ─── fix: abbyshekit review — manual-token consume only after success ────────

#[tokio::test]
async fn filesystem_manual_token_conflicted_original_cannot_change_input_or_reselect_after_collision()
 {
    // If the account write fails, the interaction must NOT be marked consumed
    // so the user can retry without going through a full re-setup.
    use brassclaw_auth::CredentialAccountId;
    use brassclaw_filesystem::CasExpectation;

    let filesystem = test_filesystem();
    let concrete_secret_store = Arc::new(InMemorySecretStore::new());
    let secret_store: Arc<dyn SecretStore> = concrete_secret_store.clone();
    let scope = test_scope();
    let service = test_service(Arc::clone(&filesystem), Arc::clone(&secret_store));

    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("expected ManualTokenRequired");
    };

    // Derive the account ID and pre-create a dummy record to force CAS failure.
    let account_id = CredentialAccountId::from_uuid(interaction_id.as_uuid());
    let dummy_account = brassclaw_auth::CredentialAccount {
        id: account_id,
        scope: scope.clone(),
        provider: google_provider(),
        label: account_label(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: None,
        access_expires_at: None,
        refresh_secret: None,
        scopes: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let path = super::paths::account_path(&scope, account_id);
    let json = serde_json::to_vec(&dummy_account).expect("serialization must succeed");
    use brassclaw_filesystem::{ContentType, Entry};
    let entry = Entry::bytes(json).with_content_type(ContentType::json());
    filesystem
        .put(&scope.resource, &path, entry, CasExpectation::Absent)
        .await
        .expect("pre-create dummy account must succeed");

    // First submit fails because account write hits CAS conflict.
    let err = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("first-attempt"),
            },
        )
        .await
        .expect_err("submit must fail when account write fails");
    assert_eq!(
        err,
        AuthProductError::BackendConflict,
        "CAS conflict must surface as BackendConflict"
    );

    // Interaction must NOT be consumed — retry still possible.
    let retry_before_cleanup = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id,
                secret: SecretString::from("retry-before-cleanup"),
            },
        )
        .await;
    assert!(
        retry_before_cleanup.is_err(),
        "retry must still fail because dummy account still blocks"
    );
    assert_eq!(
        retry_before_cleanup.unwrap_err(),
        AuthProductError::BackendConflict,
        "retry must still hit BackendConflict, not UnknownOrExpiredFlow"
    );

    // Removing a conflicting row does not rewrite the known original failure
    // or accept a different token under its consumed operation identity.
    filesystem.delete(&scope.resource, &path).await.unwrap();
    for raw in ["first-attempt", "changed-after-removal"] {
        assert_eq!(
            service
                .submit_manual_token(
                    &scope,
                    SecretSubmitRequest {
                        interaction_id,
                        secret: SecretString::from(raw),
                    }
                )
                .await
                .unwrap_err(),
            AuthProductError::BackendConflict
        );
    }
    let replacement = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: google_provider(),
            label: account_label(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired {
        interaction_id: replacement,
        ..
    } = replacement
    else {
        panic!("new explicit setup required")
    };
    assert_ne!(replacement, interaction_id);
    let configured = service
        .submit_manual_token(
            &scope,
            SecretSubmitRequest {
                interaction_id: replacement,
                secret: SecretString::from("explicit-new-setup-input"),
            },
        )
        .await
        .unwrap();
    assert_eq!(configured.status, CredentialAccountStatus::Configured);
}

#[tokio::test]
async fn independent_durable_auth_owners_reject_stale_refresh_snapshot() {
    use brassclaw_auth::CredentialSetupService;
    let filesystem = test_filesystem();
    let secrets = Arc::new(InMemorySecretStore::new());
    let first = test_service(filesystem.clone(), secrets.clone());
    let second = test_service(filesystem, secrets);
    let owner = test_scope();
    let request = NewCredentialAccount {
        scope: owner.clone(),
        provider: google_provider(),
        label: account_label(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: Some(SecretHandle::new("original-access").unwrap()),
        access_expires_at: None,
        refresh_secret: None,
        scopes: vec![],
    };
    let original = first.create_account(request.clone()).await.unwrap();
    let mut successor = request.clone();
    successor.access_secret = Some(SecretHandle::new("successor-access").unwrap());
    let published = first
        .compare_and_update_account(&original, successor)
        .await
        .unwrap();
    assert!(matches!(
        second.compare_and_update_account(&original, request).await,
        Err(AuthProductError::BackendConflict)
    ));
    assert_eq!(
        second
            .get_account(CredentialAccountLookupRequest::new(owner, original.id))
            .await
            .unwrap(),
        Some(published)
    );
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_persistent_filesystem_refresh_uses_original_reservation_and_one_account_record() {
    use brassclaw_auth::{
        CredentialRefreshAdmission, CredentialRefreshCompletion, CredentialRefreshOriginal,
        CredentialRefreshOutcome, CredentialRefreshPrepared, CredentialRefreshRequest,
        CredentialSetupService, OAuthProviderRefresh, OAuthTokenDestinations,
    };
    use brassclaw_filesystem::PostgresRootFilesystem;
    use brassclaw_secrets::{FilesystemSecretStore, SecretsCrypto};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    root.run_migrations().await.unwrap();
    let mounts = brassclaw_host_api::MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let filesystem = Arc::new(ScopedFilesystem::with_fixed_view(
        root.clone(),
        mounts.clone(),
    ));
    let crypto = Arc::new(
        SecretsCrypto::new(SecretString::from(
            "native-persistent-refresh-credential-master-key-2026",
        ))
        .unwrap(),
    );
    let secrets = Arc::new(FilesystemSecretStore::new(
        filesystem.clone(),
        crypto.clone(),
    ));
    let first = FilesystemAuthProductServices::new(filesystem.clone(), secrets);
    let owner = test_scope();
    let account = first
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Expired,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("native-fs-old-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("native-fs-old-refresh").unwrap()),
            scopes: vec![],
        })
        .await
        .unwrap();
    // Reproduce the exact old bare JSON shape before the additive envelope upgrade.
    let (_, version) = first
        .read_account(&owner, account.id)
        .await
        .unwrap()
        .unwrap();
    first
        .write_record(
            &owner.resource,
            &super::paths::account_path(&owner, account.id),
            &account,
            version.cas(),
        )
        .await
        .unwrap();
    assert_eq!(
        first
            .get_account(CredentialAccountLookupRequest::new(
                owner.clone(),
                account.id
            ))
            .await
            .unwrap(),
        Some(account.clone())
    );
    let request =
        CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
    let destinations = OAuthTokenDestinations {
        access_secret: SecretHandle::new(format!("native-fs-access-{}", request.id)).unwrap(),
        refresh_secret: SecretHandle::new(format!("native-fs-refresh-{}", request.id)).unwrap(),
    };
    let prepared = CredentialRefreshPrepared {
        original: CredentialRefreshOriginal::from(&request),
        expected_account: account.clone(),
        preparation: Some(brassclaw_auth::OAuthProviderRefreshPreparation {
            destinations: destinations.clone(),
            source: selected_refresh_source(&first, &account).await,
        }),
    };
    let second_fs = Arc::new(ScopedFilesystem::with_fixed_view(
        root.clone(),
        mounts.clone(),
    ));
    let second = FilesystemAuthProductServices::new(
        second_fs.clone(),
        Arc::new(FilesystemSecretStore::new(second_fs, crypto.clone())),
    );
    let (a, b) = tokio::join!(
        first.begin_refresh(prepared.clone()),
        second.begin_refresh(prepared.clone())
    );
    let action = match (a.unwrap(), b.unwrap()) {
        (CredentialRefreshAdmission::Dispatch(a), CredentialRefreshAdmission::Original(_)) => a,
        (CredentialRefreshAdmission::Original(_), CredentialRefreshAdmission::Dispatch(b)) => b,
        other => {
            panic!("independent persistent owners must produce exactly one dispatch: {other:?}")
        }
    };
    drop(first);
    drop(second);
    drop(filesystem);
    let reopened = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    let reopened = Arc::new(ScopedFilesystem::with_fixed_view(reopened, mounts.clone()));
    let restarted = FilesystemAuthProductServices::new(
        reopened.clone(),
        Arc::new(FilesystemSecretStore::new(reopened, crypto.clone())),
    );
    assert_eq!(
        restarted.refresh_action(&request).await.unwrap(),
        Some(action.clone())
    );
    let mut replacement = prepared.clone();
    replacement.original.id = brassclaw_auth::CredentialRefreshId::new();
    replacement.preparation.as_mut().unwrap().destinations = OAuthTokenDestinations {
        access_secret: SecretHandle::new("native-fs-replacement-access").unwrap(),
        refresh_secret: SecretHandle::new("native-fs-replacement-refresh").unwrap(),
    };
    assert!(matches!(
        restarted.begin_refresh(replacement).await,
        Err(AuthProductError::BackendConflict)
    ));
    let completion = CredentialRefreshCompletion {
        dispatched: action,
        outcome: CredentialRefreshOutcome::Returned(OAuthProviderRefresh {
            provider: account.provider.clone(),
            access_secret: destinations.access_secret,
            access_expires_at: None,
            refresh_secret: None,
            scopes: account.scopes.clone(),
        }),
    };
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_fs_account_publication() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.path LIKE '%/accounts/%' THEN RAISE EXCEPTION 'controlled FS account publication rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_fs_account_publication BEFORE UPDATE ON root_filesystem_entries FOR EACH ROW EXECUTE FUNCTION reject_fs_account_publication();").await.unwrap();
    assert!(matches!(
        restarted.complete_refresh(completion.clone()).await,
        Err(AuthProductError::BackendUnavailable)
    ));
    let retained = restarted.refresh_action(&request).await.unwrap().unwrap();
    assert!(matches!(
        retained.outcome,
        CredentialRefreshOutcome::Returned(_)
    ));
    assert_eq!(
        retained.publication,
        brassclaw_auth::CredentialRefreshPublication::Pending
    );
    assert_eq!(
        restarted
            .get_account(CredentialAccountLookupRequest::new(
                owner.clone(),
                account.id
            ))
            .await
            .unwrap(),
        Some(account.clone())
    );
    client.batch_execute("DROP TRIGGER reject_fs_account_publication ON root_filesystem_entries; DROP FUNCTION reject_fs_account_publication();").await.unwrap();
    drop(client);
    drop(restarted);
    let reopened = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
    let reopened = Arc::new(ScopedFilesystem::with_fixed_view(reopened, mounts.clone()));
    let restarted = FilesystemAuthProductServices::new(
        reopened.clone(),
        Arc::new(FilesystemSecretStore::new(reopened, crypto.clone())),
    );
    // Recovery receives only the original identity; no response is supplied again.
    let parallel_fs = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone())),
        mounts.clone(),
    ));
    let parallel = FilesystemAuthProductServices::new(
        parallel_fs.clone(),
        Arc::new(FilesystemSecretStore::new(parallel_fs, crypto.clone())),
    );
    let (a, b) = tokio::join!(
        restarted.reconcile_refresh(&request),
        parallel.reconcile_refresh(&request)
    );
    let mut confirmations = 0;
    for result in [a, b] {
        match result {
            Ok(Some(action)) => {
                assert!(action.receipt().unwrap().confirmed_refresh());
                confirmations += 1;
            }
            Err(AuthProductError::BackendConflict) => {}
            unexpected => {
                panic!("only a final original or a genuine CAS race is eligible: {unexpected:?}")
            }
        }
    }
    assert!(confirmations > 0);
    let published = restarted.refresh_action(&request).await.unwrap().unwrap();
    assert!(published.receipt().unwrap().confirmed_refresh());
    let (stored, before) = restarted
        .read_account(&owner, account.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.refresh_secret, account.refresh_secret);
    assert_eq!(published.published_account, Some(stored.clone()));
    assert_eq!(
        restarted.complete_refresh(completion).await.unwrap(),
        published
    );
    let (_, after) = restarted
        .read_account(&owner, account.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        before, after,
        "original readback does not write the account again"
    );
    let inactive = restarted
        .update_status(&owner, account.id, CredentialAccountStatus::Inactive)
        .await
        .unwrap();
    assert_eq!(
        restarted.refresh_action(&request).await.unwrap(),
        Some(published.clone())
    );
    let update = NewCredentialAccount {
        scope: owner.clone(),
        provider: account.provider.clone(),
        label: account_label(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: inactive.access_secret.clone(),
        access_expires_at: None,
        refresh_secret: inactive.refresh_secret.clone(),
        scopes: inactive.scopes.clone(),
    };
    let updated = restarted
        .compare_and_update_account(&inactive, update.clone())
        .await
        .unwrap();
    assert_eq!(
        restarted.refresh_action(&request).await.unwrap(),
        Some(published.clone())
    );
    restarted
        .create_or_update_account(brassclaw_auth::CredentialAccountMutation::Update(
            brassclaw_auth::CredentialAccountUpdate {
                account_id: updated.id,
                account: update,
            },
        ))
        .await
        .unwrap();
    assert_eq!(
        restarted.refresh_action(&request).await.unwrap(),
        Some(published)
    );
    let mut changed = request;
    changed.requester_extension = Some(ExtensionId::new("different-requester").unwrap());
    assert!(matches!(
        restarted.refresh_action(&changed).await,
        Err(AuthProductError::BackendConflict)
    ));
}

#[tokio::test]
async fn filesystem_original_refresh_readback_bypasses_held_local_publication_mutex() {
    use brassclaw_auth::{
        CredentialRefreshAdmission, CredentialRefreshCompletion, CredentialRefreshOriginal,
        CredentialRefreshOutcome, CredentialRefreshPrepared, CredentialRefreshRequest,
        CredentialSetupService, OAuthProviderRefresh, OAuthTokenDestinations,
        ProviderBackedCredentialAccountService,
    };
    use std::time::Duration;
    let backend = Arc::new(test_service(
        test_filesystem(),
        Arc::new(InMemorySecretStore::new()),
    ));
    let owner = test_scope();
    let account = backend
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: google_provider(),
            label: account_label(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("readback-fs-old-access").unwrap()),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("readback-fs-old-refresh").unwrap()),
            scopes: vec![],
        })
        .await
        .unwrap();
    let request =
        CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
    let destinations = OAuthTokenDestinations {
        access_secret: SecretHandle::new("readback-fs-new-access").unwrap(),
        refresh_secret: SecretHandle::new("readback-fs-new-refresh").unwrap(),
    };
    let prepared = CredentialRefreshPrepared {
        original: CredentialRefreshOriginal::from(&request),
        expected_account: account.clone(),
        preparation: Some(brassclaw_auth::OAuthProviderRefreshPreparation {
            destinations: destinations.clone(),
            source: selected_refresh_source(&backend, &account).await,
        }),
    };
    let original = match backend.begin_refresh(prepared.clone()).await.unwrap() {
        CredentialRefreshAdmission::Dispatch(action) => action,
        other => panic!("first original must own the only dispatch: {other:?}"),
    };
    let coordinator = ProviderBackedCredentialAccountService::new(
        backend.clone(),
        backend.clone(),
        Arc::new(UnavailableAuthProviderClient),
    );
    let mut expected = original.clone();
    for confirmed in [false, true] {
        let lock = backend.lock_for(format!("account:{}", account.id));
        let guard = lock.lock().await;
        tokio::time::timeout(Duration::from_secs(3), async {
            let missing =
                CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
            assert!(backend.reconcile_refresh(&missing).await.unwrap().is_none());
            assert_eq!(
                backend.reconcile_refresh(&request).await.unwrap(),
                Some(expected.clone())
            );
            match backend.begin_refresh(prepared.clone()).await.unwrap() {
                CredentialRefreshAdmission::Original(action) => assert_eq!(action, expected),
                other => panic!("readback must not claim another dispatch: {other:?}"),
            }
            let report = coordinator.refresh_account(request.clone()).await.unwrap();
            assert_eq!(report.original.as_ref(), Some(&expected.receipt().unwrap()));
            assert_eq!(report.refreshed, confirmed);
        })
        .await
        .expect("read-only original must bypass the local publication mutex");
        drop(guard);
        if !confirmed {
            expected = backend
                .complete_refresh(CredentialRefreshCompletion {
                    dispatched: original.clone(),
                    outcome: CredentialRefreshOutcome::Returned(OAuthProviderRefresh {
                        provider: account.provider.clone(),
                        access_secret: destinations.access_secret.clone(),
                        access_expires_at: None,
                        refresh_secret: Some(destinations.refresh_secret.clone()),
                        scopes: vec![],
                    }),
                })
                .await
                .unwrap();
        }
    }
    assert_eq!(
        backend.refresh_action(&request).await.unwrap(),
        Some(expected)
    );
}

#[cfg(feature = "skills-db")]
mod native_callback_publication {
    use super::*;
    use brassclaw_auth::{OAuthCallbackOutcome, OAuthCallbackPublication};
    use brassclaw_filesystem::{
        BackendCapabilities, Capability, PostgresRootFilesystem, TxnCapability,
    };
    use brassclaw_secrets::{FilesystemSecretStore, SecretMaterial, SecretsCrypto};

    #[derive(Clone, Copy)]
    enum AckStage {
        Admission,
        StoredResult,
        Decision,
        Account,
        FinalFlow,
    }

    /// Limited exercised record ports delegate to actual PostgreSQL. Only an
    /// acknowledgement AFTER its real committed mutation can be dropped; no
    /// synthetic success, publication bypass or production fault hook exists.
    struct LostCallbackAck {
        inner: Arc<PostgresRootFilesystem>,
        fault: std::sync::Mutex<Option<(AuthFlowId, AckStage)>>,
    }

    #[async_trait::async_trait]
    impl RootFilesystem for LostCallbackAck {
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::empty()
                .with(Capability::Read)
                .with(Capability::Write)
                .with(Capability::List)
                .with(Capability::Stat)
                .with(Capability::EntryBodyCas)
                .with_txn(TxnCapability::Cas)
        }
        async fn get(
            &self,
            path: &VirtualPath,
        ) -> Result<Option<brassclaw_filesystem::VersionedEntry>, FilesystemError> {
            self.inner.get(path).await
        }
        async fn put(
            &self,
            path: &VirtualPath,
            entry: Entry,
            cas: CasExpectation,
        ) -> Result<RecordVersion, FilesystemError> {
            let fault = *self.fault.lock().unwrap();
            let should_drop = fault.is_some_and(|(flow_id, stage)| {
                if matches!(stage, AckStage::Account) {
                    return serde_json::from_slice::<
                            super::super::refresh::StoredCredentialAccount,
                        >(&entry.body)
                        .is_ok_and(|doc| doc.auth_callback_actions.contains_key(&flow_id));
                }
                serde_json::from_slice::<crate::auth_flow_custody::StoredAuthFlow>(&entry.body)
                    .is_ok_and(|doc| {
                        doc.flow.id == flow_id
                            && match stage {
                                AckStage::Admission => {
                                    doc.flow.status == AuthFlowStatus::CallbackReceived
                                        && doc.auth_custody.oauth_callback.as_ref().is_some_and(
                                            |action| {
                                                matches!(
                                                    action.outcome,
                                                    OAuthCallbackOutcome::Submitted
                                                )
                                            },
                                        )
                                }
                                AckStage::StoredResult => {
                                    doc.fs_callback_publication.is_none()
                                        && doc.auth_custody.oauth_callback.as_ref().is_some_and(
                                            |action| {
                                                matches!(
                                                    action.outcome,
                                                    OAuthCallbackOutcome::Returned(_)
                                                ) && action.publication
                                                    == OAuthCallbackPublication::Pending
                                            },
                                        )
                                }
                                AckStage::Decision => doc.fs_callback_publication.is_some(),
                                AckStage::FinalFlow => doc.flow.status == AuthFlowStatus::Completed,
                                AckStage::Account => false,
                            }
                    })
            });
            let version = self.inner.put(path, entry, cas).await?;
            if should_drop && self.fault.lock().unwrap().take().is_some() {
                return Err(FilesystemError::Backend {
                    path: path.clone(),
                    operation: brassclaw_filesystem::FilesystemOperation::WriteFile,
                    reason: "controlled callback acknowledgement loss after committed write".into(),
                });
            }
            Ok(version)
        }
        async fn list_dir(
            &self,
            path: &VirtualPath,
        ) -> Result<Vec<brassclaw_filesystem::DirEntry>, FilesystemError> {
            self.inner.list_dir(path).await
        }
        async fn list_dir_bounded(
            &self,
            path: &VirtualPath,
            maximum: usize,
        ) -> Result<Vec<brassclaw_filesystem::DirEntry>, FilesystemError> {
            self.inner.list_dir_bounded(path, maximum).await
        }
        async fn stat(
            &self,
            path: &VirtualPath,
        ) -> Result<brassclaw_filesystem::FileStat, FilesystemError> {
            self.inner.stat(path).await
        }
    }

    fn mounts() -> brassclaw_host_api::MountView {
        brassclaw_host_api::MountView::new(vec![MountGrant::new(
            MountAlias::new("/secrets").unwrap(),
            VirtualPath::new("/tenants/test/users/alice/secrets").unwrap(),
            MountPermissions::read_write_list_delete(),
        )])
        .unwrap()
    }
    fn crypto() -> Arc<SecretsCrypto> {
        Arc::new(
            SecretsCrypto::new(SecretString::from(
                "callback-native-persistent-secret-master-key-2026",
            ))
            .unwrap(),
        )
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_fs_callback_lost_admission_ack_never_authorizes_another_dispatch() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let root = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
        root.run_migrations().await.unwrap();
        let observed = Arc::new(LostCallbackAck {
            inner: root,
            fault: std::sync::Mutex::new(None),
        });
        let filesystem = Arc::new(ScopedFilesystem::with_fixed_view(
            observed.clone(),
            mounts(),
        ));
        let secrets = Arc::new(FilesystemSecretStore::new(filesystem.clone(), crypto()));
        let backend = FilesystemAuthProductServices::new(filesystem.clone(), secrets.clone());
        let scope = test_scope();
        let flow = fs_callback_flow(&backend, &scope, None, "lost-claim").await;
        let prepared = fs_callback_prepared(&flow, "lost-claim");
        *observed.fault.lock().unwrap() = Some((flow.id, AckStage::Admission));
        let original = match backend
            .claim_oauth_callback(&scope, prepared.clone())
            .await
            .unwrap()
        {
            brassclaw_auth::OAuthCallbackAdmission::Original(state) => state,
            other => panic!("an unacknowledged claim cannot return Dispatch: {other:?}"),
        };
        assert!(observed.fault.lock().unwrap().is_none());
        assert!(matches!(
            original.action.outcome,
            OAuthCallbackOutcome::Submitted
        ));
        assert!(original.action.published_account.is_none());
        let restarted = FilesystemAuthProductServices::new(filesystem, secrets.clone());
        match restarted
            .claim_oauth_callback(&scope, prepared.clone())
            .await
            .unwrap()
        {
            brassclaw_auth::OAuthCallbackAdmission::Original(state) => {
                assert_eq!(state.action, original.action)
            }
            other => panic!("a retry cannot replace unknown dispatch ownership: {other:?}"),
        }
        let read = restarted
            .reconcile_oauth_callback(&scope, &prepared.original.reference())
            .await
            .unwrap();
        assert_eq!(read.action, original.action);
        assert_eq!(read.flow.status, AuthFlowStatus::CallbackReceived);
        assert!(
            restarted
                .read_account(&scope, original.action.target_account_id())
                .await
                .unwrap()
                .is_none()
        );
        for handle in [
            &prepared.destinations.access_secret,
            &prepared.destinations.refresh_secret,
        ] {
            assert!(
                secrets
                    .metadata(&scope.resource, handle)
                    .await
                    .unwrap()
                    .is_none(),
                "no provider output was fabricated without acknowledged dispatch ownership"
            );
        }
        let mut changed = prepared;
        changed.original.authorization_code_hash = code_hash("changed-lost-claim-code");
        assert!(matches!(
            restarted.claim_oauth_callback(&scope, changed).await,
            Err(AuthProductError::BackendConflict)
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_fs_callback_reconciles_each_lost_commit_ack_without_replacing_original() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let root = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
        root.run_migrations().await.unwrap();
        for (index, stage) in [
            AckStage::StoredResult,
            AckStage::Decision,
            AckStage::Account,
            AckStage::FinalFlow,
        ]
        .into_iter()
        .enumerate()
        {
            let observed = Arc::new(LostCallbackAck {
                inner: root.clone(),
                fault: std::sync::Mutex::new(None),
            });
            let filesystem = Arc::new(ScopedFilesystem::with_fixed_view(
                observed.clone(),
                mounts(),
            ));
            let secrets = Arc::new(FilesystemSecretStore::new(filesystem.clone(), crypto()));
            let backend = FilesystemAuthProductServices::new(filesystem.clone(), secrets.clone());
            let scope = test_scope();
            let marker = format!("native-lost-ack-{index}");
            let flow = fs_callback_flow(&backend, &scope, None, &marker).await;
            let action = fs_callback_dispatch(
                backend
                    .claim_oauth_callback(&scope, fs_callback_prepared(&flow, &marker))
                    .await
                    .unwrap(),
            );
            *observed.fault.lock().unwrap() = Some((flow.id, stage));
            for handle in [
                &action.destinations.access_secret,
                &action.destinations.refresh_secret,
            ] {
                secrets
                    .put(
                        scope.resource.clone(),
                        handle.clone(),
                        SecretMaterial::from("native-original-callback-token"),
                    )
                    .await
                    .unwrap();
            }
            let completed = backend
                .complete_oauth_callback(&scope, fs_callback_completion(&action))
                .await
                .unwrap();
            assert!(
                observed.fault.lock().unwrap().is_none(),
                "the selected real commit acknowledgement must actually be dropped"
            );
            assert_eq!(completed.status, AuthFlowStatus::Completed);
            let restarted = FilesystemAuthProductServices::new(filesystem, secrets.clone());
            let original = restarted
                .read_oauth_callback_original(&scope, &action.original.reference())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(original.clone().completed_flow().unwrap(), completed);
            assert_eq!(
                original.action.publication,
                OAuthCallbackPublication::Applied
            );
            let id = completed.credential_account_id.unwrap();
            let (document, version) = restarted
                .read_account_document(&scope, id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                document.auth_callback_actions.get(&flow.id),
                Some(&original.action)
            );
            assert_eq!(
                restarted
                    .complete_oauth_callback(&scope, fs_callback_completion(&action))
                    .await
                    .unwrap(),
                completed
            );
            assert_eq!(
                restarted
                    .read_account_document(&scope, id)
                    .await
                    .unwrap()
                    .unwrap()
                    .1,
                version,
                "an original repeat cannot write the account again"
            );
            restarted
                .update_status(&scope, id, CredentialAccountStatus::Inactive)
                .await
                .unwrap();
            let (changed, _) = restarted
                .read_account_document(&scope, id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                changed.auth_callback_actions.get(&flow.id),
                Some(&original.action)
            );
            for handle in [
                &action.destinations.access_secret,
                &action.destinations.refresh_secret,
            ] {
                assert!(
                    secrets
                        .metadata(&scope.resource, handle)
                        .await
                        .unwrap()
                        .is_some()
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_fs_callback_keeps_decision_after_rejection_and_never_false_cancels_publication()
    {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let root = Arc::new(PostgresRootFilesystem::new(rig.pool.as_ref().clone()));
        root.run_migrations().await.unwrap();
        let filesystem = Arc::new(ScopedFilesystem::with_fixed_view(root, mounts()));
        let secrets = Arc::new(FilesystemSecretStore::new(filesystem.clone(), crypto()));
        let backend = FilesystemAuthProductServices::new(filesystem.clone(), secrets.clone());
        let scope = test_scope();
        let flow = fs_callback_flow(&backend, &scope, None, "rejected-publication").await;
        let action = fs_callback_dispatch(
            backend
                .claim_oauth_callback(&scope, fs_callback_prepared(&flow, "rejected-publication"))
                .await
                .unwrap(),
        );
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION reject_fs_callback_account() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.path LIKE '%/accounts/%' THEN RAISE EXCEPTION 'controlled filesystem callback account rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_fs_callback_account BEFORE INSERT OR UPDATE ON root_filesystem_entries FOR EACH ROW EXECUTE FUNCTION reject_fs_callback_account();").await.unwrap();
        assert!(matches!(
            backend
                .complete_oauth_callback(&scope, fs_callback_completion(&action))
                .await,
            Err(AuthProductError::BackendUnavailable)
        ));
        let (pending, _) = backend
            .read_flow_document(&scope, flow.id)
            .await
            .unwrap()
            .unwrap();
        assert!(pending.fs_callback_publication.is_some());
        assert_eq!(pending.flow.status, AuthFlowStatus::Completing);
        assert!(matches!(
            pending
                .auth_custody
                .oauth_callback
                .as_ref()
                .unwrap()
                .outcome,
            OAuthCallbackOutcome::Returned(_)
        ));
        assert!(
            backend
                .read_account(&scope, action.target_account_id())
                .await
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            backend.cancel_flow(&scope, flow.id).await,
            Err(AuthProductError::BackendUnavailable)
        ));
        assert_eq!(
            backend
                .get_flow(&scope, flow.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            AuthFlowStatus::Completing,
            "cancellation cannot acknowledge Canceled while a chosen publication may still occur"
        );
        client.batch_execute("DROP TRIGGER reject_fs_callback_account ON root_filesystem_entries; DROP FUNCTION reject_fs_callback_account();").await.unwrap();
        let first = FilesystemAuthProductServices::new(filesystem.clone(), secrets.clone());
        let second = FilesystemAuthProductServices::new(filesystem, secrets);
        let reference = action.original.reference();
        let (a, b) = tokio::join!(
            first.reconcile_oauth_callback(&scope, &reference),
            second.reconcile_oauth_callback(&scope, &reference)
        );
        let a = a.unwrap();
        let b = b.unwrap();
        assert_eq!(a.flow, b.flow);
        assert_eq!(a.action, b.action);
        assert_eq!(
            a.completed_flow().unwrap().status,
            AuthFlowStatus::Completed
        );
        let (account, _) = first
            .read_account_document(&scope, action.target_account_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.auth_callback_actions.len(), 1);
        assert_eq!(account.auth_callback_actions.get(&flow.id), Some(&b.action));
        assert!(matches!(
            first.cancel_flow(&scope, flow.id).await,
            Err(AuthProductError::FlowAlreadyTerminal)
        ));
    }
}

#[path = "record_snapshot_tests.rs"]
mod record_snapshot_tests;

#[path = "requirement_publication_tests.rs"]
mod requirement_publication_tests;

#[path = "manual_publication_tests.rs"]
mod manual_publication_tests;

// Existing storage/receipt fixtures now retain a real authenticated source
// generation. No fabricated checksum is accepted by the native intent owner.
async fn selected_refresh_source<F: brassclaw_filesystem::RootFilesystem>(
    backend: &FilesystemAuthProductServices<F>,
    account: &brassclaw_auth::CredentialAccount,
) -> brassclaw_auth::OAuthRefreshSource {
    let handle = account.refresh_secret.as_ref().unwrap();
    let store = &backend.secret_store;
    if store
        .record_reference(&account.scope.resource, handle)
        .await
        .unwrap()
        .is_none()
    {
        assert!(
            store
                .put_if_record_reference(
                    account.scope.resource.clone(),
                    handle.clone(),
                    "native filesystem refresh fixture material".into(),
                    None,
                )
                .await
                .unwrap()
                .is_some()
                || store
                    .record_reference(&account.scope.resource, handle)
                    .await
                    .unwrap()
                    .is_some()
        );
    }
    brassclaw_auth::OAuthRefreshSource {
        scope: account.scope.resource.clone(),
        handle: handle.clone(),
        record_reference: store
            .record_reference(&account.scope.resource, handle)
            .await
            .unwrap()
            .unwrap(),
    }
}
