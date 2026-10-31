use crate::common::*;

#[tokio::test]
async fn oauth_callback_exchanges_provider_code_then_completes_once() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let flow = oauth_flow(&services, owner.clone()).await;
    let (dispatched, exchange) = admitted_oauth_exchange(
        &services,
        &flow,
        label("work github"),
        provider_scopes(&["repo"]),
    )
    .await;
    let input = OAuthCallbackInput {
        dispatched: Some(dispatched.clone()),
        flow_id: flow.id,
        opaque_state_hash: state_hash("state-hash"),
        outcome: ProviderCallbackOutcome::Authorized { exchange },
    };
    let completed = services
        .complete_oauth_callback(&owner, input.clone())
        .await
        .unwrap();
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    assert_eq!(
        completed.credential_account_id,
        Some(CredentialAccountId::from_uuid(flow.id.as_uuid()))
    );
    assert_eq!(services.continuations().len(), 1);
    assert!(matches!(
        services
            .complete_credential_selection(
                &owner,
                CredentialSelectionInput {
                    flow_id: completed.id,
                    credential_account_id: completed.credential_account_id.unwrap(),
                }
            )
            .await,
        Err(AuthProductError::InvalidRequest { .. })
    ));
    assert_eq!(
        services
            .complete_oauth_callback(&owner, input)
            .await
            .unwrap(),
        completed
    );
    assert_eq!(
        services.continuations().len(),
        1,
        "same completed result is read-only"
    );
    let replay = services
        .complete_oauth_callback(
            &owner,
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("state-hash"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(replay, AuthProductError::FlowAlreadyTerminal);
    assert_eq!(services.continuations().len(), 1);
}

#[tokio::test]
async fn credential_selection_completes_account_selection_flow_once() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let account = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work github"),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-work-secret").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("account");
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
            continuation: AuthContinuationRef::LifecycleActivation {
                package_ref: LifecyclePackageRef::new("github-extension").expect("valid package"),
            },
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect("flow");

    let completed = services
        .complete_credential_selection(
            &owner,
            CredentialSelectionInput {
                flow_id: flow.id,
                credential_account_id: account.id,
            },
        )
        .await
        .expect("credential selection completes");

    assert_eq!(completed.status, AuthFlowStatus::Completed);
    assert_eq!(completed.credential_account_id, Some(account.id));
    assert_eq!(services.continuations().len(), 1);

    let replay = services
        .complete_credential_selection(
            &owner,
            CredentialSelectionInput {
                flow_id: flow.id,
                credential_account_id: account.id,
            },
        )
        .await
        .expect("matching completed selection is idempotent");
    assert_eq!(replay.credential_account_id, Some(account.id));
    assert_eq!(services.continuations().len(), 1);
}

#[tokio::test]
async fn auth_flow_record_source_returns_stable_sorted_snapshot() {
    let services = InMemoryAuthProductServices::new();
    let alice = oauth_flow(&services, scope("alice")).await;
    let bob = oauth_flow(&services, scope("bob")).await;

    let snapshot = services.flow_records_snapshot();

    let ids = snapshot.iter().map(|flow| flow.id).collect::<Vec<_>>();
    let mut sorted = ids.clone();
    sorted.sort_by_key(|id| id.as_uuid());
    assert_eq!(ids, sorted);
    assert!(ids.contains(&alice.id));
    assert!(ids.contains(&bob.id));
}

#[tokio::test]
async fn credential_selection_rejects_unlisted_or_cross_scope_account() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let account = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work github"),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: Vec::new(),
            access_secret: Some(SecretHandle::new("github-work-secret").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: provider_scopes(&["repo"]),
        })
        .await
        .expect("account");
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
            continuation: AuthContinuationRef::LifecycleActivation {
                package_ref: LifecyclePackageRef::new("github-extension").expect("valid package"),
            },
            update_binding: None,
            opaque_state_hash: None,
            pkce_verifier_hash: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect("flow");

    let unlisted = services
        .complete_credential_selection(
            &owner,
            CredentialSelectionInput {
                flow_id: flow.id,
                credential_account_id: CredentialAccountId::new(),
            },
        )
        .await
        .expect_err("unlisted account rejected");
    assert_eq!(unlisted, AuthProductError::CredentialMissing);

    let cross_scope = services
        .complete_credential_selection(
            &scope("bob"),
            CredentialSelectionInput {
                flow_id: flow.id,
                credential_account_id: account.id,
            },
        )
        .await
        .expect_err("cross-scope selection rejected");
    assert_eq!(cross_scope, AuthProductError::CrossScopeDenied);
    assert!(services.continuations().is_empty());
}

#[tokio::test]
async fn oauth_callback_updates_existing_account_from_provider_exchange() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let existing = services
        .create_account(NewCredentialAccount {
            scope: owner.clone(),
            provider: provider(),
            label: label("work github"),
            status: CredentialAccountStatus::PendingSetup,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("github-old-access").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: provider_scopes(&["read:user"]),
        })
        .await
        .unwrap();
    let flow = oauth_update_flow(&services, owner.clone(), &existing).await;
    let (dispatched, exchange) = admitted_oauth_exchange(
        &services,
        &flow,
        label("renamed github"),
        provider_scopes(&["repo", "workflow"]),
    )
    .await;
    let expected_access = exchange.access_secret.clone();
    let expected_refresh = exchange.refresh_secret.clone();
    let completed = services
        .complete_oauth_callback(
            &owner,
            OAuthCallbackInput {
                dispatched: Some(dispatched),
                flow_id: flow.id,
                opaque_state_hash: state_hash("state-hash"),
                outcome: ProviderCallbackOutcome::Authorized { exchange },
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.credential_account_id, Some(existing.id));
    let updated = services
        .get_account(CredentialAccountLookupRequest::new(owner, existing.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.id, existing.id);
    assert_eq!(updated.created_at, existing.created_at);
    assert_eq!(updated.label, label("renamed github"));
    assert_eq!(updated.status, CredentialAccountStatus::Configured);
    assert_eq!(updated.access_secret, Some(expected_access));
    assert_eq!(updated.refresh_secret, expected_refresh);
    assert_eq!(updated.scopes, provider_scopes(&["repo", "workflow"]));
}

#[tokio::test]
async fn oauth_callback_rejects_mismatched_provider_and_invalid_existing_account_exchange() {
    for invalid in [
        "provider",
        "foreign-account",
        "unbound-account",
        "code",
        "pkce",
        "label",
        "access",
        "scopes",
    ] {
        let services = InMemoryAuthProductServices::new();
        let owner = scope("alice");
        let existing = services
            .create_account(account_request(
                owner.clone(),
                "work github",
                CredentialAccountStatus::PendingSetup,
            ))
            .await
            .unwrap();
        let foreign = services
            .create_account(account_request(
                scope("bob"),
                "foreign github",
                CredentialAccountStatus::PendingSetup,
            ))
            .await
            .unwrap();
        let flow = if invalid == "unbound-account" {
            oauth_flow(&services, owner.clone()).await
        } else {
            oauth_update_flow(&services, owner.clone(), &existing).await
        };
        let (dispatched, mut exchange) = admitted_oauth_exchange(
            &services,
            &flow,
            label("renamed github"),
            provider_scopes(&["repo"]),
        )
        .await;
        match invalid {
            "provider" => exchange.provider = AuthProviderId::new("gitlab").unwrap(),
            "foreign-account" => exchange.account_id = Some(foreign.id),
            "unbound-account" => exchange.account_id = Some(existing.id),
            "code" => exchange.authorization_code_hash = code_hash("another-code"),
            "pkce" => exchange.pkce_verifier_hash = pkce_hash("another-verifier"),
            "label" => exchange.account_label = label("changed-label"),
            "access" => {
                exchange.access_secret = SecretHandle::new("foreign-unowned-access").unwrap()
            }
            "scopes" => exchange.scopes.clear(),
            other => panic!("unknown case: {other}"),
        }
        let error = services
            .complete_oauth_callback(
                &owner,
                OAuthCallbackInput {
                    dispatched: Some(dispatched.clone()),
                    flow_id: flow.id,
                    opaque_state_hash: state_hash("state-hash"),
                    outcome: ProviderCallbackOutcome::Authorized {
                        exchange: exchange.clone(),
                    },
                },
            )
            .await
            .unwrap_err();
        assert_eq!(error, AuthProductError::TokenExchangeFailed, "{invalid}");
        assert!(services.continuations().is_empty(), "{invalid}");
        assert_eq!(
            services
                .get_account(CredentialAccountLookupRequest::new(
                    owner.clone(),
                    existing.id,
                ))
                .await
                .unwrap(),
            Some(existing)
        );
        let retained = services
            .read_oauth_callback_original(&owner, &dispatched.original.reference())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(retained.action.outcome, brassclaw_auth::OAuthCallbackOutcome::UnusableReturn(ref actual) if actual == &exchange)
        );
        assert_eq!(retained.flow.status, AuthFlowStatus::Failed);
        assert_eq!(retained.action.published_account, None);
    }
}

#[tokio::test]
async fn create_flow_rejects_invalid_update_binding() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let foreign_owner = scope("bob");
    let existing = services
        .create_account(account_request(
            owner.clone(),
            "work github",
            CredentialAccountStatus::PendingSetup,
        ))
        .await
        .expect("owner account");
    let foreign = services
        .create_account(account_request(
            foreign_owner,
            "foreign github",
            CredentialAccountStatus::PendingSetup,
        ))
        .await
        .expect("foreign account");
    let gitlab = AuthProviderId::new("gitlab").expect("valid provider");
    let mut provider_mismatch_request = account_request(
        owner.clone(),
        "gitlab account",
        CredentialAccountStatus::PendingSetup,
    );
    provider_mismatch_request.provider = gitlab.clone();
    let provider_mismatch = services
        .create_account(provider_mismatch_request)
        .await
        .expect("provider mismatch account");

    let missing = services
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: owner.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() + Duration::minutes(5),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: Some(CredentialAccountUpdateBinding {
                account_id: brassclaw_auth::CredentialAccountId::new(),
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: Vec::new(),
            }),
            opaque_state_hash: Some(state_hash("state-hash")),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect_err("missing update target is rejected");
    assert_eq!(missing, AuthProductError::CredentialMissing);

    let cross_scope = try_oauth_update_flow(&services, owner.clone(), &foreign)
        .await
        .expect_err("cross-scope update target is rejected at create time");
    assert_eq!(cross_scope, AuthProductError::CrossScopeDenied);

    let provider_mismatch_err = try_oauth_update_flow(&services, owner.clone(), &provider_mismatch)
        .await
        .expect_err("provider mismatch is rejected at create time");
    assert_eq!(provider_mismatch_err.code(), AuthErrorCode::InvalidRequest);

    let attacker_binding = CredentialAccountUpdateBinding {
        account_id: existing.id,
        ownership: CredentialOwnership::ExtensionOwned,
        owner_extension: Some(ExtensionId::new("attacker").unwrap()),
        granted_extensions: Vec::new(),
    };
    let authority_mismatch = services
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
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: Some(attacker_binding),
            opaque_state_hash: Some(state_hash("state-hash")),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .expect_err("authority mismatch is rejected at create time");
    assert_eq!(authority_mismatch, AuthProductError::CrossScopeDenied);
}

#[tokio::test]
async fn oauth_callback_rejects_cross_scope_stale_malformed_and_denied() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let flow = oauth_flow(&services, owner.clone()).await;

    let cross_scope = services
        .complete_oauth_callback(
            &scope("bob"),
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("state-hash"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .expect_err("foreign scope denied");
    assert_eq!(cross_scope, AuthProductError::CrossScopeDenied);

    let wrong_state = services
        .complete_oauth_callback(
            &owner,
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("other-state"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .expect_err("wrong state denied");
    assert_eq!(wrong_state, AuthProductError::CrossScopeDenied);

    let wrong_pkce = services
        .claim_oauth_callback(
            &owner,
            brassclaw_auth::OAuthCallbackPrepared {
                original: brassclaw_auth::OAuthCallbackOriginal {
                    scope: owner.clone(),
                    flow_id: flow.id,
                    provider: provider(),
                    opaque_state_hash: state_hash("state-hash"),
                    authorization_code_hash: code_hash("raw-auth-code"),
                    pkce_verifier_hash: pkce_hash("other-pkce-hash"),
                    account_label: label("work github"),
                    requested_scopes: provider_scopes(&["repo"]),
                },
                destinations: brassclaw_auth::OAuthTokenDestinations {
                    access_secret: SecretHandle::new("rejected-new-access").unwrap(),
                    refresh_secret: SecretHandle::new("rejected-new-refresh").unwrap(),
                },
            },
        )
        .await
        .unwrap_err();
    assert_eq!(wrong_pkce, AuthProductError::CrossScopeDenied);
    assert!(
        services
            .read_oauth_callback_original(
                &owner,
                &brassclaw_auth::OAuthCallbackReference {
                    scope: owner.clone(),
                    flow_id: flow.id,
                    provider: provider(),
                    opaque_state_hash: state_hash("state-hash"),
                    authorization_code_hash: code_hash("raw-auth-code"),
                    account_label: label("work github"),
                    requested_scopes: provider_scopes(&["repo"]),
                }
            )
            .await
            .unwrap()
            .is_none()
    );

    let malformed_code = OAuthAuthorizationCode::new(secret("   "))
        .expect_err("empty raw code is malformed before exchange");
    assert_eq!(malformed_code.code(), AuthErrorCode::InvalidRequest);
    let padded_verifier = PkceVerifierSecret::new(secret(" verifier "))
        .expect_err("raw verifier must be caller-clean");
    assert_eq!(padded_verifier.code(), AuthErrorCode::InvalidRequest);

    let denied = services
        .complete_oauth_callback(
            &owner,
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("state-hash"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .expect_err("provider denied");
    assert_eq!(denied, AuthProductError::ProviderDenied);
}

#[tokio::test]
async fn cancel_flow_preserves_terminal_state_and_blocks_callback() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let flow = oauth_flow(&services, owner.clone()).await;

    let canceled = services
        .cancel_flow(&owner, flow.id)
        .await
        .expect("owner cancel");
    assert_eq!(canceled.status, AuthFlowStatus::Canceled);

    let second_cancel = services
        .cancel_flow(&owner, flow.id)
        .await
        .expect_err("terminal cancel rejected");
    assert_eq!(second_cancel, AuthProductError::Canceled);

    let callback = services
        .complete_oauth_callback(
            &owner,
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("state-hash"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .expect_err("callback after cancel rejected");
    assert_eq!(callback, AuthProductError::Canceled);
}

#[tokio::test]
async fn terminal_flow_status_is_not_rewritten_after_expiry() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let flow = services
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: owner.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() - Duration::seconds(1),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(state_hash("state-hash")),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() - Duration::seconds(1),
        })
        .await
        .expect("expired flow");
    services
        .cancel_flow(&owner, flow.id)
        .await
        .expect("terminal cancel");

    let callback = services
        .complete_oauth_callback(
            &owner,
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("state-hash"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .expect_err("terminal status wins over expiry");
    assert_eq!(callback, AuthProductError::Canceled);
    let record = services
        .get_flow(&owner, flow.id)
        .await
        .expect("lookup")
        .expect("flow remains");
    assert_eq!(record.status, AuthFlowStatus::Canceled);
}

#[tokio::test]
async fn oauth_callback_marks_expired_flow_and_rejects_completion() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let flow = services
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: None,
            scope: owner.clone(),
            kind: AuthFlowKind::IntegrationCredential,
            provider: provider(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: authorization_url("https://provider.example/oauth"),
                expires_at: Utc::now() - Duration::seconds(1),
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(state_hash("state-hash")),
            pkce_verifier_hash: Some(pkce_hash("raw-pkce-verifier")),
            expires_at: Utc::now() - Duration::seconds(1),
        })
        .await
        .expect("expired flow");

    let expired = services
        .complete_oauth_callback(
            &owner,
            OAuthCallbackInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: state_hash("state-hash"),
                outcome: ProviderCallbackOutcome::Denied,
            },
        )
        .await
        .expect_err("expired flow rejects completion");
    assert_eq!(expired, AuthProductError::UnknownOrExpiredFlow);
    let record = services
        .get_flow(&owner, flow.id)
        .await
        .expect("lookup")
        .expect("flow remains");
    assert_eq!(record.status, AuthFlowStatus::Expired);
    assert_eq!(record.error, Some(AuthErrorCode::UnknownOrExpiredFlow));
}

#[tokio::test]
async fn get_flow_returns_none_owner_record_and_cross_scope_denial() {
    let services = InMemoryAuthProductServices::new();
    let owner = scope("alice");
    let flow = oauth_flow(&services, owner.clone()).await;

    let found = services
        .get_flow(&owner, flow.id)
        .await
        .expect("lookup")
        .expect("record");
    assert_eq!(found.id, flow.id);
    assert!(
        services
            .get_flow(&owner, brassclaw_auth::AuthFlowId::new())
            .await
            .expect("missing lookup")
            .is_none()
    );
    let cross_scope = services
        .get_flow(&scope("bob"), flow.id)
        .await
        .expect_err("cross scope");
    assert_eq!(cross_scope, AuthProductError::CrossScopeDenied);
}
