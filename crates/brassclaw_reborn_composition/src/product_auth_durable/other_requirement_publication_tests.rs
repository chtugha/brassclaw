//! Actual record backends and encrypted secrets; issuer return data is a fixture.
use super::*;
use brassclaw_auth::{CredentialSelectionInput, OAuthCallbackAdmission, OAuthCallbackPublication};
use brassclaw_host_api::{
    RuntimeCredentialAccountRequirement, RuntimeCredentialAccountSetup,
    RuntimeCredentialRequirementSource,
};

fn usage(oauth: bool, legacy: bool) -> RuntimeCredentialAccountRequirement {
    let request = setup(&test_scope(), None);
    let source = request.requirement_setup.unwrap();
    let mut requirement = source.requirement().requirement().clone();
    if legacy {
        requirement.handle = SecretHandle::new("legacy-usage").unwrap();
    }
    if oauth {
        requirement.source = RuntimeCredentialRequirementSource::ProductAuthAccount {
            provider: RuntimeCredentialAccountProviderId::new("manual-provider").unwrap(),
            setup: RuntimeCredentialAccountSetup::OAuth {
                scopes: vec!["read".into()],
            },
        };
    }
    RuntimeCredentialAccountRequirement::new(
        source.requirement().requester_extension().clone(),
        requirement,
    )
    .unwrap()
}

async fn account<F: RootFilesystem + 'static>(
    auth: &FilesystemAuthProductServices<F>,
    scope: &AuthProductScope,
    name: &str,
) -> CredentialAccount {
    let handle = SecretHandle::new(format!("choice-{name}")).unwrap();
    auth.secret_store
        .put(
            scope.resource.clone(),
            handle.clone(),
            "actual test credential".into(),
        )
        .await
        .unwrap();
    auth.create_account(NewCredentialAccount {
        scope: scope.clone(),
        provider: AuthProviderId::new("manual-provider").unwrap(),
        label: CredentialAccountLabel::new(name).unwrap(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: Some(handle),
        access_expires_at: None,
        refresh_secret: None,
        scopes: vec![ProviderScope::new("read").unwrap()],
    })
    .await
    .unwrap()
}

async fn flow<F: RootFilesystem + 'static>(
    auth: &FilesystemAuthProductServices<F>,
    scope: &AuthProductScope,
    previous: Option<CredentialRequirementSelection>,
    accounts: Option<&[CredentialAccount]>,
    legacy: bool,
    expired: bool,
) -> AuthFlowRecord {
    let oauth = accounts.is_none();
    let now = Utc::now();
    let expires_at = if expired {
        now - Duration::seconds(1)
    } else {
        now + Duration::minutes(5)
    };
    auth.create_flow(NewAuthFlow {
        requirement_setup: Some(Box::new(
            CredentialRequirementSetup::new(scope, usage(oauth, legacy), previous).unwrap(),
        )),
        id: None,
        scope: scope.clone(),
        kind: AuthFlowKind::IntegrationCredential,
        provider: AuthProviderId::new("manual-provider").unwrap(),
        challenge: if let Some(accounts) = accounts {
            AuthChallenge::AccountSelectionRequired {
                provider: AuthProviderId::new("manual-provider").unwrap(),
                accounts: accounts.iter().map(CredentialAccount::projection).collect(),
            }
        } else {
            AuthChallenge::OAuthUrl {
                authorization_url: OAuthAuthorizationUrl::new("https://provider.example/oauth")
                    .unwrap(),
                expires_at,
            }
        },
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        opaque_state_hash: oauth.then(|| state_hash("requirement-state")),
        pkce_verifier_hash: oauth.then(|| pkce_hash("requirement-pkce")),
        expires_at,
    })
    .await
    .unwrap()
}

fn choice(flow: &AuthFlowRecord, account: &CredentialAccount) -> CredentialSelectionInput {
    CredentialSelectionInput {
        flow_id: flow.id,
        credential_account_id: account.id,
    }
}

async fn verify_choices<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let root = Arc::new(Observed {
        inner,
        fault: std::sync::Mutex::new(None),
        triggered: AtomicUsize::new(0),
    });
    let auth = service(root.clone());
    let scope = test_scope();
    let a = account(&auth, &scope, "first").await;
    let b = account(&auth, &scope, "second").await;
    let mut previous = None;
    let mut original_result = None;
    for fault in [
        Fault::StageAck,
        Fault::HeadAck,
        Fault::FinalAck,
        Fault::FinalReject,
    ] {
        let pending = flow(
            &auth,
            &scope,
            previous.clone(),
            Some(std::slice::from_ref(&a)),
            false,
            false,
        )
        .await;
        *root.fault.lock().unwrap() = Some(fault);
        let before = root.triggered.load(Ordering::SeqCst);
        let result = auth
            .complete_credential_selection(&scope, choice(&pending, &a))
            .await;
        assert_eq!(root.triggered.load(Ordering::SeqCst), before + 1);
        let reopened = service(root.clone());
        if matches!(fault, Fault::FinalReject) {
            assert_eq!(result.unwrap_err(), AuthProductError::BackendUnavailable);
            let current = reopened
                .get_flow(&scope, pending.id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(current.status, AuthFlowStatus::AwaitingUser);
            assert!(current.continuation_emitted_at.is_none());
            assert_eq!(
                reopened
                    .requirement_selection(&scope, &usage(false, false))
                    .await
                    .unwrap_err(),
                AuthProductError::BackendUnavailable
            );
            // Cancellation cannot acknowledge while the chosen head can publish.
            assert_eq!(
                reopened.cancel_flow(&scope, pending.id).await.unwrap_err(),
                AuthProductError::FlowAlreadyTerminal
            );
        } else {
            assert_eq!(result.unwrap().status, AuthFlowStatus::Completed);
        }
        let completed = reopened
            .complete_credential_selection(&scope, choice(&pending, &a))
            .await
            .unwrap();
        original_result.get_or_insert_with(|| (pending.clone(), completed.clone()));
        let original = reopened
            .requirement_selection(&scope, &usage(false, false))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(original.original_flow(), pending.id);
        assert_eq!(original.account(), &a);
        let next = flow(
            &reopened,
            &scope,
            Some(original),
            Some(std::slice::from_ref(&b)),
            false,
            false,
        )
        .await;
        reopened
            .complete_credential_selection(&scope, choice(&next, &b))
            .await
            .unwrap();
        let head = reopened
            .requirement_selection(&scope, &usage(false, false))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            reopened
                .complete_credential_selection(&scope, choice(&pending, &a))
                .await
                .unwrap(),
            completed
        );
        assert_eq!(
            reopened
                .complete_credential_selection(&scope, choice(&pending, &b))
                .await
                .unwrap_err(),
            AuthProductError::FlowAlreadyTerminal
        );
        assert_eq!(
            reopened
                .requirement_selection(&scope, &usage(false, false))
                .await
                .unwrap(),
            Some(head.clone())
        );
        previous = Some(head);
    }
    let x = flow(
        &auth,
        &scope,
        previous.clone(),
        Some(std::slice::from_ref(&a)),
        false,
        false,
    )
    .await;
    let y = flow(
        &auth,
        &scope,
        previous,
        Some(std::slice::from_ref(&b)),
        false,
        false,
    )
    .await;
    let other = service(root.clone());
    let (one, two) = tokio::join!(
        auth.complete_credential_selection(&scope, choice(&x, &a)),
        other.complete_credential_selection(&scope, choice(&y, &b))
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let (winner, loser, losing, selected_account) = if one.is_ok() {
        (x, y, two, b.clone())
    } else {
        (y, x, one, a.clone())
    };
    assert_eq!(
        losing.unwrap_err(),
        AuthProductError::AccountSelectionRequired
    );
    assert_eq!(
        other
            .complete_credential_selection(&scope, choice(&loser, &selected_account))
            .await
            .unwrap_err(),
        AuthProductError::AccountSelectionRequired
    );
    let head = auth
        .requirement_selection(&scope, &usage(false, false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(head.original_flow(), winner.id);
    assert_eq!(auth.read_account(&scope, a.id).await.unwrap().unwrap().0, a);
    assert_eq!(auth.read_account(&scope, b.id).await.unwrap().unwrap().0, b);
    let expired = flow(
        &auth,
        &scope,
        Some(head.clone()),
        Some(std::slice::from_ref(&a)),
        false,
        true,
    )
    .await;
    assert_eq!(
        auth.complete_credential_selection(&scope, choice(&expired, &a))
            .await
            .unwrap_err(),
        AuthProductError::UnknownOrExpiredFlow
    );
    assert_eq!(
        other
            .get_flow(&scope, expired.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        AuthFlowStatus::Expired
    );
    assert_eq!(
        auth.requirement_selection(&scope, &usage(false, false))
            .await
            .unwrap(),
        Some(head)
    );
    auth.update_status(&scope, a.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    let (original, result) = original_result.unwrap();
    assert_eq!(
        auth.complete_credential_selection(&scope, choice(&original, &a))
            .await
            .unwrap(),
        result
    );
}

async fn admitted<F: RootFilesystem + 'static>(
    auth: &FilesystemAuthProductServices<F>,
    scope: &AuthProductScope,
    flow: &AuthFlowRecord,
) -> brassclaw_auth::OAuthCallbackAction {
    let marker = flow.id.to_string();
    let mut prepared = fs_callback_prepared(flow, &marker);
    prepared.original.requested_scopes = vec![ProviderScope::new("read").unwrap()];
    let action = fs_callback_dispatch(auth.claim_oauth_callback(scope, prepared).await.unwrap());
    for handle in [
        &action.destinations.access_secret,
        &action.destinations.refresh_secret,
    ] {
        auth.secret_store
            .put(
                scope.resource.clone(),
                handle.clone(),
                "actual retained test credential".into(),
            )
            .await
            .unwrap();
    }
    action
}

async fn verify_oauth<F: RootFilesystem + 'static>(inner: Arc<F>) {
    let root = Arc::new(Observed {
        inner,
        fault: std::sync::Mutex::new(None),
        triggered: AtomicUsize::new(0),
    });
    let auth = service(root.clone());
    let scope = test_scope();
    let mut previous = None;
    for fault in [
        Fault::StageAck,
        Fault::HeadAck,
        Fault::FinalAck,
        Fault::FinalReject,
    ] {
        let pending = flow(&auth, &scope, previous, None, false, false).await;
        let action = admitted(&auth, &scope, &pending).await;
        *root.fault.lock().unwrap() = Some(fault);
        let before = root.triggered.load(Ordering::SeqCst);
        let first = auth
            .complete_oauth_callback(&scope, fs_callback_completion(&action))
            .await;
        assert_eq!(root.triggered.load(Ordering::SeqCst), before + 1);
        let reopened = service(root.clone());
        if matches!(fault, Fault::FinalReject) {
            assert_eq!(first.unwrap_err(), AuthProductError::BackendUnavailable);
            assert_eq!(
                reopened
                    .requirement_selection(&scope, &usage(true, false))
                    .await
                    .unwrap_err(),
                AuthProductError::BackendUnavailable
            );
            assert_eq!(
                reopened.cancel_flow(&scope, pending.id).await.unwrap_err(),
                AuthProductError::FlowAlreadyTerminal
            );
        } else {
            assert_eq!(first.unwrap().status, AuthFlowStatus::Completed);
        }
        let original = reopened
            .reconcile_oauth_callback(&scope, &action.original.reference())
            .await
            .unwrap();
        assert_eq!(original.flow.status, AuthFlowStatus::Completed);
        assert_eq!(
            original.action.publication,
            OAuthCallbackPublication::Applied
        );
        let selected = reopened
            .requirement_selection(&scope, &usage(true, false))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(selected.original_flow(), pending.id);
        let original_source = crate::credential_requirement_custody_tests::published_source(
            &reopened,
            reopened.secret_store.as_ref(),
            &scope,
            &selected,
        )
        .await;
        let successor = flow(&reopened, &scope, Some(selected), None, false, false).await;
        let next = admitted(&reopened, &scope, &successor).await;
        reopened
            .complete_oauth_callback(&scope, fs_callback_completion(&next))
            .await
            .unwrap();
        let current = reopened
            .requirement_selection(&scope, &usage(true, false))
            .await
            .unwrap()
            .unwrap();
        let recovered = reopened
            .reconcile_oauth_callback(&scope, &action.original.reference())
            .await
            .unwrap();
        assert_eq!(recovered.flow, original.flow);
        assert_eq!(
            reopened
                .requirement_source_for_flow(&scope, pending.id, &usage(true, false))
                .await
                .unwrap(),
            Some(original_source)
        );
        assert_eq!(recovered.action, original.action);
        assert_eq!(
            reopened
                .requirement_selection(&scope, &usage(true, false))
                .await
                .unwrap(),
            Some(current.clone())
        );
        assert!(matches!(
            reopened
                .claim_oauth_callback(&scope, {
                    let mut prepared = fs_callback_prepared(&pending, &pending.id.to_string());
                    prepared.original.requested_scopes = vec![ProviderScope::new("read").unwrap()];
                    prepared
                })
                .await
                .unwrap(),
            OAuthCallbackAdmission::Original(_)
        ));
        assert!(
            reopened
                .complete_credential_selection(
                    &scope,
                    CredentialSelectionInput {
                        flow_id: pending.id,
                        credential_account_id: action.target_account_id()
                    }
                )
                .await
                .is_err()
        );
        previous = Some(current);
    }
    // Two admitted original issuer results may have real account effects, but
    // only one captured head wins. The other original stays explicitly failed.
    let x = flow(&auth, &scope, previous.clone(), None, false, false).await;
    let y = flow(&auth, &scope, previous, None, false, false).await;
    let a = admitted(&auth, &scope, &x).await;
    let b = admitted(&auth, &scope, &y).await;
    let other = service(root.clone());
    let (one, two) = tokio::join!(
        auth.complete_oauth_callback(&scope, fs_callback_completion(&a)),
        other.complete_oauth_callback(&scope, fs_callback_completion(&b))
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let (winner, loser, losing, action) = if one.is_ok() {
        (x, y, two, b)
    } else {
        (y, x, one, a)
    };
    assert_eq!(
        losing.unwrap_err(),
        AuthProductError::AccountSelectionRequired
    );
    let original = other
        .reconcile_oauth_callback(&scope, &action.original.reference())
        .await
        .unwrap();
    assert_eq!(original.flow.id, loser.id);
    assert_eq!(original.flow.status, AuthFlowStatus::Failed);
    assert_eq!(
        original.flow.error,
        Some(brassclaw_auth::AuthErrorCode::AccountSelectionRequired)
    );
    assert_eq!(
        original.action.publication,
        OAuthCallbackPublication::Applied
    );
    assert!(original.flow.continuation_emitted_at.is_none());
    assert!(
        other
            .read_account(&scope, action.target_account_id())
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        auth.requirement_selection(&scope, &usage(true, false))
            .await
            .unwrap()
            .unwrap()
            .original_flow(),
        winner.id
    );

    // An archive-shaped old decision with no requirement receipt must complete
    // its original result without backfilling a new requirement head.
    let legacy = flow(&auth, &scope, None, None, true, false).await;
    let action = admitted(&auth, &scope, &legacy).await;
    *root.fault.lock().unwrap() = Some(Fault::CallbackAccountReject);
    assert_eq!(
        auth.complete_oauth_callback(&scope, fs_callback_completion(&action))
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    let (staged, snapshot) = auth
        .read_flow_document(&scope, legacy.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        staged
            .fs_callback_publication
            .as_ref()
            .unwrap()
            .requirement_selection
            .is_some()
    );
    let mut archived = serde_json::to_value(staged).unwrap();
    archived["auth_custody"]
        .as_object_mut()
        .unwrap()
        .remove("requirement_selection");
    archived["fs_callback_publication"]
        .as_object_mut()
        .unwrap()
        .remove("requirement_selection");
    // Keeping a source while removing its selection is corruption, not an
    // old archive. The actual owner rejects it before publishing any account.
    let snapshot = auth
        .write_flow_document(
            &serde_json::from_value(archived.clone()).unwrap(),
            snapshot.cas(),
        )
        .await
        .unwrap();
    assert_eq!(
        auth.reconcile_oauth_callback(&scope, &action.original.reference())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    archived["auth_custody"]
        .as_object_mut()
        .unwrap()
        .remove("requirement_source");
    auth.write_flow_document(&serde_json::from_value(archived).unwrap(), snapshot.cas())
        .await
        .unwrap();
    let reopened = service(root);
    assert_eq!(
        reopened
            .reconcile_oauth_callback(&scope, &action.original.reference())
            .await
            .unwrap()
            .completed_flow()
            .unwrap()
            .status,
        AuthFlowStatus::Completed
    );
    assert!(
        reopened
            .requirement_selection(&scope, &usage(true, true))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_fs_account_choice_retains_head_originals_and_cancellation_fence() {
    verify_choices(Arc::new(InMemoryBackend::new())).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_fs_oauth_retains_head_originals_and_partial_effects() {
    verify_oauth(Arc::new(InMemoryBackend::new())).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_account_choice_retains_head_originals_and_cancellation_fence() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_choices(root).await;
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fs_oauth_retains_head_originals_and_partial_effects() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let root = Arc::new(brassclaw_filesystem::PostgresRootFilesystem::new(
        rig.pool.as_ref().clone(),
    ));
    root.run_migrations().await.unwrap();
    verify_oauth(root).await;
}
