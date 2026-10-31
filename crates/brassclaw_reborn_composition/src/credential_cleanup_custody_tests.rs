//! Real local-owner lifecycle contracts. No issuer deregistration or global
//! retained-source garbage-collection acceptance follows from these cases.
use crate::product_auth_durable::FilesystemAuthProductServices;
use brassclaw_auth::*;
use brassclaw_filesystem::{InMemoryBackend, ScopedFilesystem};
use brassclaw_host_api::{
    ExtensionId, InvocationId, MountAlias, MountGrant, MountPermissions, MountView, ResourceScope,
    SecretHandle, UserId, VirtualPath,
};
use brassclaw_secrets::{InMemorySecretStore, SecretCustodyState, SecretStore};
use std::sync::Arc;

fn scope() -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope::local_default(UserId::new("lifecycle-owner").unwrap(), InvocationId::new())
            .unwrap(),
        AuthSurface::Web,
    )
}
fn fs() -> Arc<ScopedFilesystem<InMemoryBackend>> {
    let view = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/secrets/lifecycle-custody-tests").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::new(InMemoryBackend::new()),
        view,
    ))
}
async fn account<S: CredentialAccountService>(
    service: &S,
    store: &dyn SecretStore,
    scope: &AuthProductScope,
) -> CredentialAccount {
    let identity = uuid::Uuid::new_v4();
    let access = SecretHandle::new(format!("lifecycle-exact-access-{identity}")).unwrap();
    let refresh = SecretHandle::new(format!("lifecycle-exact-refresh-{identity}")).unwrap();
    store
        .put(
            scope.resource.clone(),
            access.clone(),
            "private original access".into(),
        )
        .await
        .unwrap();
    store
        .put(
            scope.resource.clone(),
            refresh.clone(),
            "private original refresh".into(),
        )
        .await
        .unwrap();
    service
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: AuthProviderId::new("notion").unwrap(),
            label: CredentialAccountLabel::new("lifecycle account").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::ExtensionOwned,
            owner_extension: Some(ExtensionId::new("notion-extension").unwrap()),
            granted_extensions: vec![],
            access_secret: Some(access),
            access_expires_at: None,
            refresh_secret: Some(refresh),
            scopes: vec![],
        })
        .await
        .unwrap()
}
fn request(scope: &AuthProductScope) -> SecretCleanupRequest {
    let mut event_scope = scope.clone();
    event_scope.resource.invocation_id = InvocationId::new();
    event_scope.surface = AuthSurface::Callback;
    SecretCleanupRequest {
        scope: event_scope,
        extension_id: ExtensionId::new("notion-extension").unwrap(),
        action: SecretCleanupAction::Uninstall,
    }
}
async fn verify_owner_boundaries<S: CredentialAccountService + SecretCleanupService>(
    service: &S,
    store: &dyn SecretStore,
) {
    let mut selected_scope = scope();
    selected_scope.resource.thread_id =
        Some(brassclaw_host_api::ThreadId::new("lifecycle-selected-thread").unwrap());
    selected_scope.session_id = Some(AuthSessionId::new("lifecycle-selected-session").unwrap());
    let selected = account(service, store, &selected_scope).await;
    let mut foreign_scopes = Vec::new();
    let mut foreign = selected_scope.clone();
    foreign.resource.user_id = UserId::new("other-lifecycle-owner").unwrap();
    foreign_scopes.push(foreign);
    let mut foreign = selected_scope.clone();
    foreign.resource.project_id =
        Some(brassclaw_host_api::ProjectId::new("other-lifecycle-project").unwrap());
    foreign_scopes.push(foreign);
    let mut foreign = selected_scope.clone();
    foreign.resource.thread_id =
        Some(brassclaw_host_api::ThreadId::new("other-lifecycle-thread").unwrap());
    foreign_scopes.push(foreign);
    let mut foreign = selected_scope.clone();
    foreign.session_id = Some(AuthSessionId::new("other-lifecycle-session").unwrap());
    foreign_scopes.push(foreign);
    let mut unselected = Vec::new();
    for foreign in foreign_scopes {
        unselected.push(account(service, store, &foreign).await);
    }
    let report = service
        .cleanup_for_lifecycle(request(&selected_scope))
        .await
        .unwrap();
    assert_eq!(report.revoked_accounts, vec![selected.id]);
    assert!(report.quarantined_accounts.is_empty());
    for original in unselected {
        let actual = service
            .get_account(
                CredentialAccountLookupRequest::new(original.scope.clone(), original.id)
                    .for_extension(ExtensionId::new("notion-extension").unwrap()),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            actual, original,
            "a later lifecycle request cannot expand its stable owner selection"
        );
        assert!(
            store
                .matches(
                    &original.scope.resource,
                    original.access_secret.as_ref().unwrap(),
                    &"private original access".into()
                )
                .await
                .unwrap()
        );
    }
}
async fn verify<S: CredentialAccountService + SecretCleanupService>(
    service: &S,
    store: &dyn SecretStore,
    scope: &AuthProductScope,
    account: &CredentialAccount,
) {
    let report = service.cleanup_for_lifecycle(request(scope)).await.unwrap();
    assert_eq!(report.revoked_accounts, vec![account.id]);
    assert!(report.quarantined_accounts.is_empty());
    let after = service
        .get_account(
            CredentialAccountLookupRequest::new(scope.clone(), account.id)
                .for_extension(ExtensionId::new("notion-extension").unwrap()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, CredentialAccountStatus::Revoked);
    assert!(after.access_secret.is_none() && after.refresh_secret.is_none());
    for handle in [&account.access_secret, &account.refresh_secret]
        .into_iter()
        .flatten()
    {
        let metadata = store
            .custody(&scope.resource, handle)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            metadata.state,
            SecretCustodyState::RetiredOrdinary { .. }
        ));
        assert!(store.lease_once(&scope.resource, handle).await.is_err());
        assert!(
            store
                .put(
                    scope.resource.clone(),
                    handle.clone(),
                    "late replacement".into()
                )
                .await
                .is_err()
        );
    }
    let text = serde_json::to_string(&report).unwrap();
    assert!(!text.contains("lifecycle-exact-access"));
    assert!(!text.contains("private original"));
    let repeated = service.cleanup_for_lifecycle(request(scope)).await.unwrap();
    assert!(repeated.quarantined_accounts.is_empty());
    let mut deactivate = request(scope);
    deactivate.action = SecretCleanupAction::Deactivate;
    service.cleanup_for_lifecycle(deactivate).await.unwrap();
    assert_eq!(
        service
            .get_account(
                CredentialAccountLookupRequest::new(scope.clone(), account.id)
                    .for_extension(ExtensionId::new("notion-extension").unwrap())
            )
            .await
            .unwrap()
            .unwrap()
            .status,
        CredentialAccountStatus::Revoked,
        "deactivation cannot undo an already revoked account state"
    );
}
#[tokio::test]
async fn filesystem_lifecycle_keeps_account_owned_receipts_after_success_and_restart() {
    let fs = fs();
    let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let scope = scope();
    let service = FilesystemAuthProductServices::new(fs.clone(), store.clone());
    let original = account(&service, store.as_ref(), &scope).await;
    verify(&service, store.as_ref(), &scope, &original).await;
    let restarted = FilesystemAuthProductServices::new(fs, store);
    assert!(
        restarted
            .cleanup_for_lifecycle(request(&scope))
            .await
            .unwrap()
            .quarantined_accounts
            .is_empty()
    );
}
#[tokio::test]
async fn filesystem_lifecycle_uses_stable_owner_and_preserves_declared_owner_boundaries() {
    let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let service = FilesystemAuthProductServices::new(fs(), store.clone());
    verify_owner_boundaries(&service, store.as_ref()).await;
}
#[tokio::test]
async fn filesystem_lifecycle_rejects_duplicated_account_identity_before_any_secret_effect() {
    use brassclaw_filesystem::{CasExpectation, Entry};
    use brassclaw_host_api::ScopedPath;
    let fs = fs();
    let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let service = FilesystemAuthProductServices::new(fs.clone(), store.clone());
    let scope = scope();
    let original = account(&service, store.as_ref(), &scope).await;
    // A controlled corrupt second row reuses an existing real account UUID in
    // another surface. It must not be silently deduplicated during lifecycle scan.
    let root = ScopedPath::from_trusted(format!(
        "/secrets/{}/product-auth/callback/accounts",
        scope.resource.within_agent_project_segment()
    ));
    let path = ScopedPath::from_trusted(format!("{}/{}.json", root.as_str(), original.id));
    fs.put(
        &scope.resource,
        &path,
        Entry::bytes(serde_json::to_vec(&original).unwrap()),
        CasExpectation::Absent,
    )
    .await
    .unwrap();
    assert!(
        service
            .cleanup_for_lifecycle(request(&scope))
            .await
            .is_err()
    );
    assert_eq!(
        service
            .get_account(
                CredentialAccountLookupRequest::new(scope.clone(), original.id)
                    .for_extension(ExtensionId::new("notion-extension").unwrap())
            )
            .await
            .unwrap()
            .unwrap(),
        original
    );
    assert!(
        store
            .matches(
                &scope.resource,
                original.access_secret.as_ref().unwrap(),
                &"private original access".into()
            )
            .await
            .unwrap()
    );
}
#[tokio::test]
async fn filesystem_lifecycle_covers_all_accounts_and_sessions_beyond_the_former_owner_ceiling() {
    let store: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());
    let service = FilesystemAuthProductServices::new(fs(), store);
    let mut owner = scope();
    owner.resource.thread_id =
        Some(brassclaw_host_api::ThreadId::new("bulk-lifecycle-thread").unwrap());
    let mut expected = std::collections::BTreeSet::new();
    let mut expected_flows = std::collections::BTreeSet::new();
    let expiry = chrono::Utc::now() + chrono::Duration::minutes(10);
    // The old reader rejected either more than 1024 accounts in one root or
    // more than 1024 session roots. Neither is a lifecycle selection boundary.
    for index in 0..1025 {
        for separate_session in [false, true] {
            let mut source = owner.clone();
            if separate_session {
                source.session_id =
                    Some(AuthSessionId::new(format!("bulk-lifecycle-session-{index}")).unwrap());
            }
            let account = service
                .create_account(NewCredentialAccount {
                    scope: source.clone(),
                    provider: AuthProviderId::new("notion").unwrap(),
                    label: CredentialAccountLabel::new(format!(
                        "bulk lifecycle {index} {separate_session}"
                    ))
                    .unwrap(),
                    status: CredentialAccountStatus::PendingSetup,
                    ownership: CredentialOwnership::ExtensionOwned,
                    owner_extension: Some(ExtensionId::new("notion-extension").unwrap()),
                    granted_extensions: vec![],
                    access_secret: None,
                    access_expires_at: None,
                    refresh_secret: None,
                    scopes: vec![],
                })
                .await
                .unwrap();
            assert!(expected.insert(account.id));
            let flow = service
                .create_flow(NewAuthFlow {
                    requirement_setup: None,
                    id: None,
                    scope: source,
                    kind: AuthFlowKind::IntegrationCredential,
                    provider: AuthProviderId::new("notion").unwrap(),
                    challenge: AuthChallenge::OAuthUrl {
                        authorization_url: OAuthAuthorizationUrl::new(
                            "https://provider.invalid/authorize",
                        )
                        .unwrap(),
                        expires_at: expiry,
                    },
                    continuation: AuthContinuationRef::SetupOnly,
                    update_binding: None,
                    opaque_state_hash: None,
                    pkce_verifier_hash: None,
                    expires_at: expiry,
                })
                .await
                .unwrap();
            assert!(expected_flows.insert(flow.id));
        }
    }
    let listed = service
        .flows_for_owner(AuthFlowOwnerScope {
            tenant_id: owner.resource.tenant_id.clone(),
            user_id: owner.resource.user_id.clone(),
            agent_id: owner.resource.agent_id.clone(),
            project_id: owner.resource.project_id.clone(),
            thread_id: owner.resource.thread_id.clone().unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), expected_flows.len());
    assert_eq!(
        listed
            .into_iter()
            .map(|flow| flow.id)
            .collect::<std::collections::BTreeSet<_>>(),
        expected_flows
    );
    let report = service
        .cleanup_for_lifecycle(request(&owner))
        .await
        .unwrap();
    assert!(report.quarantined_accounts.is_empty());
    assert_eq!(report.revoked_accounts.len(), expected.len());
    assert_eq!(
        report
            .revoked_accounts
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
}
#[tokio::test]
async fn filesystem_lifecycle_retains_pending_handles_and_reconciles_before_and_after_closure_failure()
 {
    for after_commit in [false, true] {
        let fs = fs();
        let store = Arc::new(crate::oauth_dcr::TestSecretFaultStore::new_at(
            usize::MAX,
            false,
        ));
        let scope = scope();
        let service = FilesystemAuthProductServices::new(fs.clone(), store.clone());
        let original = account(&service, store.as_ref(), &scope).await;
        store.fail_next_closure(after_commit);
        let report = service
            .cleanup_for_lifecycle(request(&scope))
            .await
            .unwrap();
        assert_eq!(report.quarantined_accounts.is_empty(), after_commit);
        let current = service
            .get_account(
                CredentialAccountLookupRequest::new(scope.clone(), original.id)
                    .for_extension(ExtensionId::new("notion-extension").unwrap()),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.status, CredentialAccountStatus::Revoked);
        if !after_commit {
            assert_eq!(current.access_secret, original.access_secret);
            assert_eq!(current.refresh_secret, original.refresh_secret);
        }
        let restarted = FilesystemAuthProductServices::new(fs, store.clone());
        verify(&restarted, store.as_ref(), &scope, &original).await;
    }
}
#[tokio::test]
async fn filesystem_lifecycle_never_purges_a_replacement_generation_after_pending_failure() {
    let store = Arc::new(crate::oauth_dcr::TestSecretFaultStore::new_at(
        usize::MAX,
        false,
    ));
    let scope = scope();
    let service = FilesystemAuthProductServices::new(fs(), store.clone());
    let original = account(&service, store.as_ref(), &scope).await;
    store.fail_next_closure(false);
    assert!(
        !service
            .cleanup_for_lifecycle(request(&scope))
            .await
            .unwrap()
            .quarantined_accounts
            .is_empty()
    );
    let access = original.access_secret.clone().unwrap();
    store
        .put(
            scope.resource.clone(),
            access.clone(),
            "newer unselected generation".into(),
        )
        .await
        .unwrap();
    let report = service
        .cleanup_for_lifecycle(request(&scope))
        .await
        .unwrap();
    assert_eq!(report.quarantined_accounts.len(), 1);
    assert!(
        store
            .matches(
                &scope.resource,
                &access,
                &"newer unselected generation".into()
            )
            .await
            .unwrap()
    );
    let current = service
        .get_account(
            CredentialAccountLookupRequest::new(scope.clone(), original.id)
                .for_extension(ExtensionId::new("notion-extension").unwrap()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.access_secret, Some(access));
    assert_eq!(current.status, CredentialAccountStatus::Revoked);
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_commits_account_and_exact_secret_closures_together_with_ack_readback() {
    use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
    use secrecy::ExposeSecret;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(crate::pg_auth_product_services::MIGRATIONS)
        .await
        .unwrap();
    let store = Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "controlled-native-lifecycle-master-key".into(),
            "native-lifecycle-custody",
        )
        .unwrap(),
    );
    let plain = crate::pg_auth_product_services::PgAuthProductServices::new(
        rig.pool.clone(),
        store.clone(),
    );
    let scope = scope();
    let original = account(&plain, store.as_ref(), &scope).await;
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let writer = crate::pg_auth_product_services::PgAuthProductServices::new(
        transport.pool.clone(),
        store.clone(),
    );
    transport.arm(1).unwrap();
    verify(&writer, store.as_ref(), &scope, &original).await;
    assert_eq!(transport.cut_count(), 1);
    let client = rig.pool.get().await.unwrap();
    let row = client
        .query_one(
            "SELECT data,revision FROM brassclaw_product_auth_accounts WHERE id=$1",
            &[&original.id.to_string()],
        )
        .await
        .unwrap();
    let data: serde_json::Value = row.get(0);
    let revision: i64 = row.get(1);
    assert_eq!(data["auth_lifecycle_cleanup"].as_array().unwrap().len(), 1);
    assert_eq!(
        data["auth_lifecycle_cleanup"][0]["targets"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    plain.cleanup_for_lifecycle(request(&scope)).await.unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT revision FROM brassclaw_product_auth_accounts WHERE id=$1",
                &[&original.id.to_string()]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        revision,
        "repeat/readback of finished cleanup makes no new account revision"
    );
    verify_owner_boundaries(&plain, store.as_ref()).await;
}

#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_lifecycle_rejected_secret_close_rolls_back_account_and_keeps_original_handles() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(crate::pg_auth_product_services::MIGRATIONS)
        .await
        .unwrap();
    let store = Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "controlled-native-lifecycle-rollback-key".into(),
            "native-lifecycle-rollback",
        )
        .unwrap(),
    );
    let service = crate::pg_auth_product_services::PgAuthProductServices::new(
        rig.pool.clone(),
        store.clone(),
    );
    let scope = scope();
    let original = account(&service, store.as_ref(), &scope).await;
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_lifecycle_close() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled lifecycle close rejection'; END; $$; CREATE TRIGGER reject_lifecycle_close BEFORE UPDATE ON brassclaw_secrets FOR EACH ROW EXECUTE FUNCTION reject_lifecycle_close();").await.unwrap();
    let report = service
        .cleanup_for_lifecycle(request(&scope))
        .await
        .unwrap();
    assert!(report.revoked_accounts.is_empty());
    assert_eq!(report.quarantined_accounts.len(), 1);
    let actual = service
        .get_account(
            CredentialAccountLookupRequest::new(scope.clone(), original.id)
                .for_extension(ExtensionId::new("notion-extension").unwrap()),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        actual, original,
        "failed secret closure cannot commit handle removal or account revocation"
    );
    assert!(
        store
            .matches(
                &scope.resource,
                original.access_secret.as_ref().unwrap(),
                &"private original access".into()
            )
            .await
            .unwrap()
    );
    client.batch_execute("DROP TRIGGER reject_lifecycle_close ON brassclaw_secrets; DROP FUNCTION reject_lifecycle_close();").await.unwrap();
    verify(&service, store.as_ref(), &scope, &original).await;
}
