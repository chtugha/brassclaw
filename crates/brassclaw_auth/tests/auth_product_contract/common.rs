pub(crate) use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthErrorCode, AuthFlowKind, AuthFlowManager,
    AuthFlowStatus, AuthGateRef, AuthInteractionService, AuthProductError, AuthProductScope,
    AuthProviderClient, AuthProviderId, AuthSessionId, AuthSurface, AuthorizationCodeHash,
    CredentialAccount, CredentialAccountChoiceRequest, CredentialAccountId, CredentialAccountLabel,
    CredentialAccountListRequest, CredentialAccountLookupRequest, CredentialAccountMutation,
    CredentialAccountProjection, CredentialAccountSelectionRequest, CredentialAccountService,
    CredentialAccountStatus, CredentialAccountUpdate, CredentialAccountUpdateBinding,
    CredentialOwnership, CredentialRecoveryKind, CredentialRecoveryProjection,
    CredentialRecoveryReason, CredentialRecoveryRequest, CredentialRefreshRequest,
    CredentialSelectionInput, CredentialSetupService, InMemoryAuthProductServices,
    LifecyclePackageRef, ManualTokenSetupRequest, NewAuthFlow, NewCredentialAccount,
    OAuthAuthorizationCode, OAuthAuthorizationUrl, OAuthCallbackInput,
    OAuthProviderCallbackRequest, OAuthProviderExchange, OAuthProviderExchangeContext,
    OAuthProviderRefreshRequest, OpaqueStateHash, PkceVerifierHash, PkceVerifierSecret,
    ProviderCallbackOutcome, ProviderScope, SecretCleanupAction, SecretCleanupQuarantineReason,
    SecretCleanupRequest, SecretCleanupService, SecretSubmitRequest, SecretSubmitResult,
    TurnRunRef,
};
pub(crate) use brassclaw_host_api::{
    ExtensionId, InvocationId, ResourceScope, SecretHandle, UserId,
};
pub(crate) use chrono::{Duration, Utc};
pub(crate) use secrecy::SecretString;

pub(crate) fn scope(user: &str) -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope::local_default(UserId::new(user).expect("valid user"), InvocationId::new())
            .expect("valid scope"),
        AuthSurface::Web,
    )
    .with_session_id(AuthSessionId::new(format!("session-{user}")).expect("valid session"))
}

pub(crate) fn provider() -> AuthProviderId {
    AuthProviderId::new("github").expect("valid provider")
}

pub(crate) fn label(value: &str) -> CredentialAccountLabel {
    CredentialAccountLabel::new(value).expect("valid label")
}

pub(crate) fn state_hash(value: &str) -> OpaqueStateHash {
    OpaqueStateHash::new(fake_digest(value)).expect("valid state hash")
}

pub(crate) fn pkce_hash(value: &str) -> PkceVerifierHash {
    brassclaw_auth::pkce_verifier_hash(
        &PkceVerifierSecret::new(secret(value)).expect("valid fixture input"),
    )
    .expect("valid pkce hash")
}

pub(crate) fn code_hash(value: &str) -> AuthorizationCodeHash {
    brassclaw_auth::authorization_code_hash(
        &OAuthAuthorizationCode::new(secret(value)).expect("valid fixture input"),
    )
    .expect("valid code hash")
}

pub(crate) fn fake_digest(value: &str) -> String {
    format!(
        "{:064x}",
        value.bytes().fold(0_u64, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u64::from(byte))
        })
    )
}

pub(crate) fn authorization_url(value: &str) -> OAuthAuthorizationUrl {
    OAuthAuthorizationUrl::new(value).expect("valid authorization url")
}

pub(crate) fn provider_scope(value: &str) -> ProviderScope {
    ProviderScope::new(value).expect("valid provider scope")
}

pub(crate) fn provider_scopes(values: &[&str]) -> Vec<ProviderScope> {
    values.iter().map(|value| provider_scope(value)).collect()
}

pub(crate) fn secret(value: &str) -> SecretString {
    SecretString::from(value.to_string())
}

pub(crate) fn account_request(
    owner: AuthProductScope,
    label_value: &str,
    status: CredentialAccountStatus,
) -> NewCredentialAccount {
    NewCredentialAccount {
        scope: owner,
        provider: provider(),
        label: label(label_value),
        status,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: Vec::new(),
        access_secret: None,
        access_expires_at: None,
        refresh_secret: None,
        scopes: Vec::new(),
    }
}

pub(crate) fn account_ids(accounts: &[CredentialAccountProjection]) -> Vec<CredentialAccountId> {
    let mut ids = accounts
        .iter()
        .map(|account| account.id)
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

pub(crate) fn update_binding(account: &CredentialAccount) -> CredentialAccountUpdateBinding {
    CredentialAccountUpdateBinding {
        account_id: account.id,
        ownership: account.ownership,
        owner_extension: account.owner_extension.clone(),
        granted_extensions: account.granted_extensions.clone(),
    }
}

pub(crate) async fn oauth_flow(
    services: &InMemoryAuthProductServices,
    owner: AuthProductScope,
) -> brassclaw_auth::AuthFlowRecord {
    services
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: owner,
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::LifecycleActivation {
                package_ref: LifecyclePackageRef::new("github-extension").expect("valid package"),
            },
            update_binding: None,
            opaque_state_hash: Some(state_hash("state-hash")),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect("flow")
}

pub(crate) async fn oauth_update_flow(
    services: &InMemoryAuthProductServices,
    owner: AuthProductScope,
    account: &CredentialAccount,
) -> brassclaw_auth::AuthFlowRecord {
    try_oauth_update_flow(services, owner, account)
        .await
        .expect("update flow")
}

pub(crate) async fn try_oauth_update_flow(
    services: &InMemoryAuthProductServices,
    owner: AuthProductScope,
    account: &CredentialAccount,
) -> Result<brassclaw_auth::AuthFlowRecord, AuthProductError> {
    services
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: owner,
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::LifecycleActivation {
                package_ref: LifecyclePackageRef::new("github-extension").expect("valid package"),
            },
            update_binding: Some(update_binding(account)),
            opaque_state_hash: Some(state_hash("state-hash")),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
}

/// Prepare and claim before the test provider can return any artifact. This is
/// the real Auth port sequence; no completion fixture bypasses original custody.
pub(crate) async fn admitted_oauth_exchange(
    services: &InMemoryAuthProductServices,
    flow: &brassclaw_auth::AuthFlowRecord,
    account_label: CredentialAccountLabel,
    scopes: Vec<ProviderScope>,
) -> (brassclaw_auth::OAuthCallbackAction, OAuthProviderExchange) {
    let request = OAuthProviderCallbackRequest {
        provider: flow.provider.clone(),
        account_label,
        authorization_code: OAuthAuthorizationCode::new(secret("raw-auth-code")).unwrap(),
        authorization_code_hash: code_hash("raw-auth-code"),
        pkce_verifier: PkceVerifierSecret::new(secret("raw-pkce-verifier")).unwrap(),
        pkce_verifier_hash: pkce_hash("raw-pkce-verifier"),
        scopes,
    };
    let context = OAuthProviderExchangeContext {
        scope: flow.scope.clone(),
        flow_id: flow.id,
    };
    let destinations = services.prepare_callback(&context, &request).await.unwrap();
    let original = brassclaw_auth::OAuthCallbackOriginal::from_request(
        flow.scope.clone(),
        flow.id,
        flow.opaque_state_hash.clone().unwrap(),
        &request,
    )
    .unwrap();
    let dispatched = match services
        .claim_oauth_callback(
            &flow.scope,
            brassclaw_auth::OAuthCallbackPrepared {
                original,
                destinations,
            },
        )
        .await
        .unwrap()
    {
        brassclaw_auth::OAuthCallbackAdmission::Dispatch(action) => action.action,
        other => panic!("fixture must acquire the single original dispatch: {other:?}"),
    };
    let exchange = services.exchange_callback(context, request).await.unwrap();
    (dispatched, exchange)
}
