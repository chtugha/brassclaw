//! Actual Auth and SecretStore owners; source metadata never grants access.
use super::*;
use brassclaw_auth::{
    AuthChallenge, AuthContinuationRef, AuthFlowKind, AuthFlowManager, AuthProviderId, AuthSurface,
    CredentialAccountLabel, CredentialAccountService, CredentialOwnership,
    CredentialRequirementSetup, CredentialSelectionInput, InMemoryAuthProductServices, NewAuthFlow,
    NewCredentialAccount,
};
use brassclaw_host_api::{
    ExtensionId, InvocationId, NetworkTargetPattern, ResourceScope,
    RuntimeCredentialAccountProviderId, RuntimeCredentialAccountSetup,
    RuntimeCredentialRequirement, RuntimeCredentialTarget, SecretHandle, UserId,
};
use brassclaw_secrets::InMemorySecretStore;

/// Test-only original-source setup shared with the invocation-view guard case;
/// mirrors capture from the actual Auth/SecretStore owners, never activation.
pub(crate) struct Fixture {
    pub(crate) accounts: Arc<InMemoryAuthProductServices>,
    pub(crate) secrets: Arc<InMemorySecretStore>,
    pub(crate) scope: AuthProductScope,
    pub(crate) requirement: RuntimeCredentialAccountRequirement,
}
impl Fixture {
    pub(crate) fn new() -> Self {
        Self {
            accounts: Arc::new(InMemoryAuthProductServices::new()),
            secrets: Arc::new(InMemorySecretStore::new()),
            scope: AuthProductScope::new(
                ResourceScope::local_default(
                    UserId::new("captured-credential-owner").unwrap(),
                    InvocationId::new(),
                )
                .unwrap(),
                AuthSurface::Api,
            ),
            requirement: RuntimeCredentialAccountRequirement::new(
                ExtensionId::new("example-extension").unwrap(),
                RuntimeCredentialRequirement {
                    handle: SecretHandle::new("access-slot").unwrap(),
                    source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                        provider: RuntimeCredentialAccountProviderId::new("example").unwrap(),
                        setup: RuntimeCredentialAccountSetup::ManualToken,
                    },
                    provider_scopes: Vec::new(),
                    audience: NetworkTargetPattern {
                        scheme: None,
                        host_pattern: "api.example.com".into(),
                        port: None,
                    },
                    target: RuntimeCredentialTarget::Header {
                        name: "Authorization".into(),
                        prefix: Some("Bearer ".into()),
                    },
                    required: true,
                },
            )
            .unwrap(),
        }
    }
    pub(crate) async fn account(&self, name: &str, with_material: bool) -> CredentialAccount {
        let handle = SecretHandle::new(name).unwrap();
        if with_material {
            self.secrets
                .put(self.scope.resource.clone(), handle.clone(), name.into())
                .await
                .unwrap();
        }
        self.accounts
            .create_account(NewCredentialAccount {
                scope: self.scope.clone(),
                provider: AuthProviderId::new("example").unwrap(),
                label: CredentialAccountLabel::new(name).unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
                access_secret: Some(handle),
                access_expires_at: None,
                refresh_secret: None,
                scopes: Vec::new(),
            })
            .await
            .unwrap()
    }
    pub(crate) async fn select(&self, account: &CredentialAccount) {
        let previous = self
            .accounts
            .requirement_selection(&self.scope, &self.requirement)
            .await
            .unwrap();
        let provider = account.provider.clone();
        let flow = self
            .accounts
            .create_flow(NewAuthFlow {
                requirement_setup: Some(Box::new(
                    CredentialRequirementSetup::new(
                        &self.scope,
                        self.requirement.clone(),
                        previous,
                    )
                    .unwrap(),
                )),
                id: None,
                scope: self.scope.clone(),
                kind: AuthFlowKind::IntegrationCredential,
                provider: provider.clone(),
                challenge: AuthChallenge::AccountSelectionRequired {
                    provider,
                    accounts: vec![account.projection()],
                },
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: None,
                opaque_state_hash: None,
                pkce_verifier_hash: None,
                expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
            })
            .await
            .unwrap();
        self.accounts
            .complete_credential_selection(
                &self.scope,
                CredentialSelectionInput {
                    flow_id: flow.id,
                    credential_account_id: account.id,
                },
            )
            .await
            .unwrap();
    }
    async fn capture(&self) -> CapturedAccountUsage {
        CapturedAccountUsage::capture(
            self.accounts.clone(),
            self.secrets.as_ref(),
            self.scope.clone(),
            self.requirement.clone(),
        )
        .await
        .unwrap()
    }
}
fn request(usage: &CapturedAccountUsage) -> RuntimeCredentialAccountRequest<'_> {
    let declaration = usage.requirement.requirement();
    let RuntimeCredentialRequirementSource::ProductAuthAccount { provider, .. } =
        &declaration.source
    else {
        panic!("account declaration");
    };
    RuntimeCredentialAccountRequest {
        requirement: Some(&usage.requirement),
        scope: &usage.scope.resource,
        provider,
        provider_scopes: &declaration.provider_scopes,
        requester_extension: usage.requirement.requester_extension(),
    }
}

#[tokio::test]
async fn original_source_survives_head_change_but_never_ciphertext_replacement() {
    let fixture = Fixture::new();
    let first = fixture.account("account-a", true).await;
    let second = fixture.account("account-b", true).await;
    fixture.select(&first).await;
    let captured = fixture.capture().await;
    let original = captured.resolve(request(&captured)).await.unwrap();
    fixture.select(&second).await;
    assert_eq!(
        captured.resolve(request(&captured)).await.unwrap(),
        original
    );
    assert_eq!(captured.previous().unwrap().account().id, first.id);
    let newly_captured = fixture.capture().await;
    assert_eq!(newly_captured.previous().unwrap().account().id, second.id);
    assert_ne!(
        newly_captured
            .resolve(request(&newly_captured))
            .await
            .unwrap()
            .handle,
        original.handle
    );
    // The source owner, not copied metadata, rejects a changed actual ciphertext.
    fixture
        .secrets
        .put(
            original.scope.clone(),
            original.handle.clone(),
            "replacement".into(),
        )
        .await
        .unwrap();
    let still_original = captured.resolve(request(&captured)).await.unwrap();
    assert_eq!(still_original, original);
    assert!(
        fixture
            .secrets
            .lease_once_if_record_reference(
                &still_original.scope,
                &still_original.handle,
                &still_original.record_reference,
            )
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn captured_absence_cannot_adopt_later_global_selection_or_material() {
    let fixture = Fixture::new();
    let absent_selection = fixture.capture().await;
    let account = fixture.account("selected-without-material", false).await;
    fixture.select(&account).await;
    let absent_material = fixture.capture().await;
    fixture
        .secrets
        .put(
            account.scope.resource.clone(),
            account.access_secret.unwrap(),
            "late".into(),
        )
        .await
        .unwrap();
    for usage in [&absent_selection, &absent_material] {
        assert!(matches!(
            usage.resolve(request(usage)).await,
            Err(CredentialStageError::AuthRequired)
        ));
    }
    assert!(absent_selection.previous().is_none());
    assert_eq!(absent_material.previous().unwrap().account().id, account.id);
}

#[tokio::test]
async fn original_selection_is_retained_when_live_account_is_revoked() {
    let fixture = Fixture::new();
    let account = fixture.account("revoked-account", true).await;
    fixture.select(&account).await;
    let captured = fixture.capture().await;
    fixture
        .accounts
        .update_status(&account.scope, account.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    assert!(matches!(
        captured.resolve(request(&captured)).await,
        Err(CredentialStageError::AuthRequired)
    ));
    let captured_revoked = fixture.capture().await;
    assert_eq!(
        captured_revoked.previous().unwrap().account().id,
        account.id
    );
    assert!(matches!(
        captured_revoked.resolve(request(&captured_revoked)).await,
        Err(CredentialStageError::AuthRequired)
    ));
}
