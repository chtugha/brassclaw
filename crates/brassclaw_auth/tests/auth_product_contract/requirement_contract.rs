//! Selection metadata and atomic in-memory fake transitions. Actual native/FS
//! publication and task-family custody require their own integration checks.
use crate::common::*;
use brassclaw_auth::{
    CredentialAccountRecordSource, CredentialRequirementOwner, CredentialRequirementPublication,
    CredentialRequirementSelection, CredentialRequirementSetup, ManualTokenSetupIdentity,
};
use brassclaw_host_api::{
    NetworkTargetPattern, RuntimeCredentialAccountProviderId, RuntimeCredentialAccountRequirement,
    RuntimeCredentialAccountSetup, RuntimeCredentialRequirement,
    RuntimeCredentialRequirementSource, RuntimeCredentialTarget,
};

fn usage(host: &str) -> RuntimeCredentialAccountRequirement {
    RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("github-extension").unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("access").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("github").unwrap(),
                setup: RuntimeCredentialAccountSetup::ManualToken,
            },
            provider_scopes: vec!["read".into()],
            audience: NetworkTargetPattern {
                scheme: None,
                host_pattern: host.into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: None,
            },
            required: true,
        },
    )
    .unwrap()
}

async fn configured(
    services: &InMemoryAuthProductServices,
    owner: &AuthProductScope,
    name: &str,
) -> CredentialAccount {
    let mut request = account_request(owner.clone(), name, CredentialAccountStatus::Configured);
    request.access_secret = Some(SecretHandle::new(format!("access-{name}")).unwrap());
    request.scopes = provider_scopes(&["read"]);
    services.create_account(request).await.unwrap()
}

async fn select(
    services: &InMemoryAuthProductServices,
    owner: &AuthProductScope,
    account: &CredentialAccount,
) -> brassclaw_auth::AuthFlowRecord {
    let flow = services
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: owner.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::AccountSelectionRequired {
                provider: provider(),
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
    services
        .complete_credential_selection(
            owner,
            CredentialSelectionInput {
                flow_id: flow.id,
                credential_account_id: account.id,
            },
        )
        .await
        .unwrap()
}

#[test]
fn requirement_keys_are_stable_across_surfaces_and_preserve_owner_and_usage() {
    let owner = scope("alice");
    let original = CredentialRequirementOwner::from_scope(&owner.resource)
        .key(&usage("one.example.com"))
        .unwrap();
    let mut later = scope("alice");
    later.surface = AuthSurface::Api;
    later.resource.thread_id = Some(brassclaw_host_api::ThreadId::new("later-thread").unwrap());
    assert_eq!(
        CredentialRequirementOwner::from_scope(&later.resource)
            .key(&usage("one.example.com"))
            .unwrap(),
        original
    );
    assert_ne!(
        CredentialRequirementOwner::from_scope(&scope("bob").resource)
            .key(&usage("one.example.com"))
            .unwrap(),
        original
    );
    assert_ne!(
        CredentialRequirementOwner::from_scope(&owner.resource)
            .key(&usage("two.example.com"))
            .unwrap(),
        original
    );
}

#[tokio::test]
async fn original_selection_metadata_preserves_cas_and_exact_recovery() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let first = configured(&services, &owner, "first").await;
    let second = configured(&services, &owner, "second").await;
    let initial = CredentialRequirementSetup::new(&owner, usage("api.example.com"), None).unwrap();
    let a = CredentialRequirementSelection::from_completed_flow(
        &owner,
        &initial,
        &select(&services, &owner, &first).await,
        first,
    )
    .unwrap();
    assert_eq!(
        initial.publication(None, &a).unwrap(),
        CredentialRequirementPublication::Publish
    );
    assert_eq!(
        initial.publication(Some(&a), &a).unwrap(),
        CredentialRequirementPublication::AlreadyPublished
    );
    let replacement =
        CredentialRequirementSetup::new(&owner, usage("api.example.com"), Some(a.clone())).unwrap();
    let b = CredentialRequirementSelection::from_completed_flow(
        &owner,
        &replacement,
        &select(&services, &owner, &second).await,
        second,
    )
    .unwrap();
    assert_eq!(
        replacement.publication(Some(&a), &b).unwrap(),
        CredentialRequirementPublication::Publish
    );
    assert!(replacement.publication(None, &b).is_err());
    assert!(initial.publication(Some(&b), &a).is_err());
    let restored: CredentialRequirementSelection =
        serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    assert_eq!(restored, a);
    assert_eq!(restored.reference().unwrap(), a.reference().unwrap());
    assert!(
        CredentialRequirementSetup::new(&scope("bob"), usage("api.example.com"), Some(a)).is_err()
    );
}

#[tokio::test]
async fn explicit_shared_account_has_distinct_requirement_selections() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let account = configured(&services, &owner, "shared").await;
    let mut selections = Vec::new();
    for host in ["first.example.com", "second.example.com"] {
        let setup = CredentialRequirementSetup::new(&owner, usage(host), None).unwrap();
        let flow = select(&services, &owner, &account).await;
        let selected = CredentialRequirementSelection::from_completed_flow(
            &owner,
            &setup,
            &flow,
            account.clone(),
        )
        .unwrap();
        assert_eq!(selected.account().id, account.id);
        selections.push(selected);
    }
    assert_ne!(selections[0].key().unwrap(), selections[1].key().unwrap());
    assert!(
        selections[0]
            .verify_requested(&owner, selections[1].requirement())
            .is_err()
    );
}

fn manual_request(
    owner: &AuthProductScope,
    previous: Option<CredentialRequirementSelection>,
) -> ManualTokenSetupRequest {
    ManualTokenSetupRequest {
        requirement_setup: Some(Box::new(
            CredentialRequirementSetup::new(owner, usage("api.example.com"), previous).unwrap(),
        )),
        scope: owner.clone(),
        provider: provider(),
        label: label("required manual account"),
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + Duration::minutes(5),
    }
}

#[tokio::test]
async fn fake_manual_selection_is_atomic_and_original_recovery_keeps_later_choice() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let first = ManualTokenSetupIdentity::default();
    let stale = ManualTokenSetupIdentity::default();
    services
        .request_manual_setup_original_for_tests(first, manual_request(&owner, None))
        .unwrap();
    services
        .request_manual_setup_original_for_tests(stale, manual_request(&owner, None))
        .unwrap();
    assert_eq!(
        services
            .submit_manual_token(
                &owner,
                SecretSubmitRequest {
                    interaction_id: first.interaction_id,
                    secret: secret("bare credential"),
                }
            )
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert!(
        services
            .accounts_for_owner(&owner)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(services.continuations().is_empty());
    let (result, flow) = services
        .submit_original_manual_token_flow_for_tests(&owner, first, secret("first credential"))
        .unwrap();
    let selected = services
        .requirement_selection(&owner, &usage("api.example.com"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.account().id, result.account_id);
    assert_eq!(selected.account().scopes, provider_scopes(&["read"]));
    assert_eq!(selected.original_flow(), flow.id);
    let accounts = services.accounts_for_owner(&owner).await.unwrap();
    let events = services.continuations();
    assert_eq!(
        services
            .submit_original_manual_token_flow_for_tests(&owner, stale, secret("stale credential"),)
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(services.accounts_for_owner(&owner).await.unwrap(), accounts);
    assert_eq!(services.continuations(), events);
    assert_eq!(
        services
            .recover_manual_setup_original_for_tests(&owner, stale)
            .unwrap()
            .unwrap()
            .status,
        AuthFlowStatus::AwaitingUser
    );

    let next = ManualTokenSetupIdentity::default();
    services
        .request_manual_setup_original_for_tests(next, manual_request(&owner, Some(selected)))
        .unwrap();
    services
        .submit_original_manual_token_flow_for_tests(&owner, next, secret("new credential"))
        .unwrap();
    let latest = services
        .requirement_selection(&owner, &usage("api.example.com"))
        .await
        .unwrap()
        .unwrap();
    let before = services.continuations().len();
    let reopened = services.clone();
    assert_eq!(
        reopened
            .continue_manual_setup_original_for_tests(&owner, first)
            .unwrap(),
        (result.clone(), flow)
    );
    assert_eq!(
        reopened
            .submit_original_manual_token_flow_for_tests(&owner, first, secret("first credential"))
            .unwrap()
            .0,
        result
    );
    assert!(
        reopened
            .submit_original_manual_token_flow_for_tests(
                &owner,
                first,
                secret("changed credential")
            )
            .is_err()
    );
    assert_eq!(reopened.continuations().len(), before);
    assert_eq!(
        reopened
            .requirement_selection(&owner, &usage("api.example.com"))
            .await
            .unwrap(),
        Some(latest)
    );
}

#[tokio::test]
async fn fake_stale_bound_selection_cannot_mutate_its_existing_account() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let initial = ManualTokenSetupIdentity::default();
    services
        .request_manual_setup_original_for_tests(initial, manual_request(&owner, None))
        .unwrap();
    let (original_result, _) = services
        .submit_original_manual_token_flow_for_tests(&owner, initial, secret("original credential"))
        .unwrap();
    let selected = services
        .requirement_selection(&owner, &usage("api.example.com"))
        .await
        .unwrap()
        .unwrap();
    let stale = ManualTokenSetupIdentity::default();
    let mut bound = manual_request(&owner, Some(selected.clone()));
    bound.update_binding = Some(CredentialAccountUpdateBinding::from_projection(
        &selected.account().projection(),
    ));
    bound.label = label("must not replace label or handle");
    services
        .request_manual_setup_original_for_tests(stale, bound)
        .unwrap();
    let next = ManualTokenSetupIdentity::default();
    services
        .request_manual_setup_original_for_tests(next, manual_request(&owner, Some(selected)))
        .unwrap();
    services
        .submit_original_manual_token_flow_for_tests(&owner, next, secret("successor credential"))
        .unwrap();
    let account = services
        .account_record(&owner, original_result.account_id)
        .await
        .unwrap()
        .unwrap();
    let events = services.continuations().len();
    assert_eq!(
        services
            .submit_original_manual_token_flow_for_tests(
                &owner,
                stale,
                secret("rejected credential")
            )
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        services
            .account_record(&owner, original_result.account_id)
            .await
            .unwrap(),
        Some(account)
    );
    assert_eq!(services.continuations().len(), events);
}

#[tokio::test]
async fn fake_bound_requirement_update_preserves_declared_scopes_and_old_original() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let initial = ManualTokenSetupIdentity::default();
    services
        .request_manual_setup_original_for_tests(initial, manual_request(&owner, None))
        .unwrap();
    let original = services
        .submit_original_manual_token_flow_for_tests(&owner, initial, secret("original credential"))
        .unwrap();
    let selected = services
        .requirement_selection(&owner, &usage("api.example.com"))
        .await
        .unwrap()
        .unwrap();
    let next = ManualTokenSetupIdentity::default();
    let mut request = manual_request(&owner, Some(selected.clone()));
    request.update_binding = Some(CredentialAccountUpdateBinding::from_projection(
        &selected.account().projection(),
    ));
    services
        .request_manual_setup_original_for_tests(next, request)
        .unwrap();
    let updated = services
        .submit_original_manual_token_flow_for_tests(&owner, next, secret("replacement credential"))
        .unwrap();
    assert_eq!(updated.0.account_id, original.0.account_id);
    let latest = services
        .requirement_selection(&owner, &usage("api.example.com"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.original_flow(), next.flow_id);
    assert_eq!(latest.account().scopes, provider_scopes(&["read"]));
    assert_ne!(
        latest.account().access_secret,
        selected.account().access_secret
    );
    assert_eq!(
        services
            .continue_manual_setup_original_for_tests(&owner, initial)
            .unwrap(),
        original
    );
}

async fn required_flow(
    services: &InMemoryAuthProductServices,
    owner: &AuthProductScope,
    previous: Option<CredentialRequirementSelection>,
    accounts: Option<&[CredentialAccount]>,
) -> brassclaw_auth::AuthFlowRecord {
    let oauth = accounts.is_none();
    let mut declaration = usage("required.example.com").requirement().clone();
    if oauth {
        declaration.source = RuntimeCredentialRequirementSource::ProductAuthAccount {
            provider: RuntimeCredentialAccountProviderId::new("github").unwrap(),
            setup: RuntimeCredentialAccountSetup::OAuth {
                scopes: vec!["read".into()],
            },
        };
    }
    let requirement = RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("github-extension").unwrap(),
        declaration,
    )
    .unwrap();
    services
        .create_flow(NewAuthFlow {
            requirement_setup: Some(Box::new(
                CredentialRequirementSetup::new(owner, requirement, previous).unwrap(),
            )),
            id: None,
            scope: owner.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: if let Some(accounts) = accounts {
                AuthChallenge::AccountSelectionRequired {
                    provider: provider(),
                    accounts: accounts.iter().map(CredentialAccount::projection).collect(),
                }
            } else {
                AuthChallenge::OAuthUrl {
                    authorization_url: authorization_url("https://provider.example/oauth"),
                    expires_at: Utc::now() + Duration::minutes(5),
                }
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: oauth.then(|| state_hash("state-hash")),
            pkce_verifier_hash: oauth.then(|| pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn fake_account_choice_commits_original_head_once_and_rejects_stale_selection() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let a = configured(&services, &owner, "first").await;
    let b = configured(&services, &owner, "second").await;
    let first = required_flow(&services, &owner, None, Some(std::slice::from_ref(&a))).await;
    let stale = required_flow(&services, &owner, None, Some(std::slice::from_ref(&b))).await;
    let input = |flow: &brassclaw_auth::AuthFlowRecord, account: &CredentialAccount| {
        CredentialSelectionInput {
            flow_id: flow.id,
            credential_account_id: account.id,
        }
    };
    let completed = services
        .complete_credential_selection(&owner, input(&first, &a))
        .await
        .unwrap();
    let requirement = services
        .requirement_setup(&owner, first.id)
        .await
        .unwrap()
        .unwrap();
    let original = services
        .requirement_selection(&owner, requirement.requirement())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.original_flow(), first.id);
    assert_eq!(
        services
            .complete_credential_selection(&owner, input(&stale, &b))
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        services.get_flow(&owner, stale.id).await.unwrap().unwrap(),
        stale
    );
    assert_eq!(services.continuations().len(), 1);
    assert_eq!(
        services.account_record(&owner, a.id).await.unwrap(),
        Some(a.clone())
    );
    assert_eq!(
        services.account_record(&owner, b.id).await.unwrap(),
        Some(b.clone())
    );
    let next = required_flow(
        &services,
        &owner,
        Some(original),
        Some(std::slice::from_ref(&b)),
    )
    .await;
    services
        .complete_credential_selection(&owner, input(&next, &b))
        .await
        .unwrap();
    let latest = services
        .requirement_selection(&owner, requirement.requirement())
        .await
        .unwrap()
        .unwrap();
    services
        .update_status(&owner, a.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    assert_eq!(
        services
            .complete_credential_selection(&owner, input(&first, &a))
            .await
            .unwrap(),
        completed
    );
    assert_eq!(
        services
            .requirement_selection(&owner, requirement.requirement())
            .await
            .unwrap(),
        Some(latest)
    );
    assert_eq!(services.continuations().len(), 2);
}

#[tokio::test]
async fn fake_oauth_publishes_original_requirement_atomically_without_reselecting() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let first = required_flow(&services, &owner, None, None).await;
    let stale = required_flow(&services, &owner, None, None).await;
    let (a, exchange_a) = admitted_oauth_exchange(
        &services,
        &first,
        label("first"),
        provider_scopes(&["read"]),
    )
    .await;
    let (b, exchange_b) = admitted_oauth_exchange(
        &services,
        &stale,
        label("second"),
        provider_scopes(&["read"]),
    )
    .await;
    let callback = |action: &brassclaw_auth::OAuthCallbackAction, exchange| OAuthCallbackInput {
        flow_id: action.original.flow_id,
        opaque_state_hash: action.original.opaque_state_hash.clone(),
        dispatched: Some(action.clone()),
        outcome: ProviderCallbackOutcome::Authorized { exchange },
    };
    let completed = services
        .complete_oauth_callback(&owner, callback(&a, exchange_a))
        .await
        .unwrap();
    let setup = services
        .requirement_setup(&owner, first.id)
        .await
        .unwrap()
        .unwrap();
    let original = services
        .requirement_selection(&owner, setup.requirement())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        services
            .complete_oauth_callback(&owner, callback(&b, exchange_b))
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    let unresolved = services
        .read_oauth_callback_original(&owner, &b.original.reference())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unresolved.flow.status, AuthFlowStatus::Completing);
    assert_eq!(
        unresolved.action.publication,
        brassclaw_auth::OAuthCallbackPublication::Pending
    );
    assert!(matches!(
        unresolved.action.outcome,
        brassclaw_auth::OAuthCallbackOutcome::Returned(_)
    ));
    assert!(
        services
            .account_record(&owner, b.target_account_id())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(services.continuations().len(), 1);
    let next = required_flow(&services, &owner, Some(original), None).await;
    let (c, exchange_c) =
        admitted_oauth_exchange(&services, &next, label("third"), provider_scopes(&["read"])).await;
    services
        .complete_oauth_callback(&owner, callback(&c, exchange_c))
        .await
        .unwrap();
    let latest = services
        .requirement_selection(&owner, setup.requirement())
        .await
        .unwrap()
        .unwrap();
    services
        .update_status(
            &owner,
            a.target_account_id(),
            CredentialAccountStatus::Revoked,
        )
        .await
        .unwrap();
    assert_eq!(
        services
            .reconcile_oauth_callback(&owner, &a.original.reference())
            .await
            .unwrap()
            .completed_flow()
            .unwrap(),
        completed
    );
    assert_eq!(
        services
            .reconcile_oauth_callback(&owner, &b.original.reference())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        services
            .requirement_selection(&owner, setup.requirement())
            .await
            .unwrap(),
        Some(latest)
    );
    assert_eq!(services.continuations().len(), 2);
}
