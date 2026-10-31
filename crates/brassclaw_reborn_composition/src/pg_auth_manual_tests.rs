//! Native publication failures and independent callers, using actual storage.
use super::*;
use brassclaw_auth::{AuthContinuationRef, AuthSurface};
use brassclaw_host_api::{InvocationId, ResourceScope, ThreadId, UserId};
use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
use chrono::Duration;
use secrecy::ExposeSecret;

#[derive(Default)]
struct RejectFirstContinuation(std::sync::atomic::AtomicUsize);

#[async_trait]
impl crate::auth::RebornAuthContinuationDispatcher for RejectFirstContinuation {
    async fn dispatch_auth_continuation(
        &self,
        _event: brassclaw_auth::AuthContinuationEvent,
    ) -> Result<(), AuthProductError> {
        if self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            Err(AuthProductError::BackendUnavailable)
        } else {
            Ok(())
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_actual_facade_never_recovers_a_changed_raw_input_as_latest_account() {
    use crate::auth::{
        RebornManualTokenSetupRequest, RebornManualTokenSubmitRequest,
        RebornProductAuthServicePorts,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let native = Arc::new(service(&rig.pool));
    let dispatcher = Arc::new(RejectFirstContinuation::default());
    // Same port construction as the production factory. Manual input never
    // invokes an issuer, so the actual unavailable-provider port is sufficient.
    let facade = RebornProductAuthServicePorts::from_shared_with_provider(
        native.clone(),
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(dispatcher.clone())
    .with_flow_record_source(native.clone());
    let scope = owner();
    let challenge = facade
        .request_manual_token_setup(RebornManualTokenSetupRequest::new(
            scope.clone(),
            AuthProviderId::new("manual-provider").unwrap(),
            CredentialAccountLabel::new("manual-account").unwrap(),
            AuthContinuationRef::SetupOnly,
            Utc::now() + Duration::minutes(5),
        ))
        .await
        .unwrap();
    let id = challenge.interaction_id;
    let first = facade
        .submit_manual_token(RebornManualTokenSubmitRequest::new(
            scope.clone(),
            id,
            "facade original raw input".into(),
        ))
        .await
        .unwrap_err();
    assert!(first.retryable);
    let published = native.flows_for_scope(&scope).await.unwrap().pop().unwrap();
    assert_eq!(published.status, AuthFlowStatus::Completed);
    assert!(published.continuation_emitted_at.is_none());
    let wrong = facade
        .submit_manual_token(RebornManualTokenSubmitRequest::new(
            scope.clone(),
            id,
            "changed input after dispatcher failure".into(),
        ))
        .await
        .unwrap_err();
    assert!(wrong.conflict);
    assert!(!wrong.retryable);
    assert_eq!(dispatcher.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        native
            .read_flow(&scope, published.id)
            .await
            .unwrap()
            .unwrap()
            .0
            .continuation_emitted_at
            .is_none()
    );
    let restored = facade
        .submit_manual_token(RebornManualTokenSubmitRequest::new(
            scope.clone(),
            id,
            "facade original raw input".into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        restored.account_id,
        published.credential_account_id.unwrap()
    );
    assert_eq!(dispatcher.0.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(
        native
            .read_flow(&scope, published.id)
            .await
            .unwrap()
            .unwrap()
            .0
            .continuation_emitted_at
            .is_some()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_legacy_bound_interaction_uses_its_original_flow_snapshot() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let id = setup(&auth, &scope).await;
    let (first, _) = submit(&auth, &scope, id, "first legacy-bound account input")
        .await
        .unwrap();
    let before = auth
        .read_account(&scope, first.account_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    let challenge = auth
        .request_manual_token_flow(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: before.provider.clone(),
            label: before.label.clone(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: Some(CredentialAccountUpdateBinding {
                account_id: before.id,
                ownership: before.ownership,
                owner_extension: before.owner_extension.clone(),
                granted_extensions: before.granted_extensions.clone(),
            }),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("manual challenge required");
    };
    let client = rig.pool.get().await.unwrap();
    client
        .execute(
            "UPDATE brassclaw_product_auth_interactions SET data=data-'update_account_snapshot'
        WHERE tenant_id=$1 AND id=$2",
            &[
                &scope.resource.tenant_id.as_str(),
                &interaction_id.to_string(),
            ],
        )
        .await
        .unwrap();
    assert!(
        auth.read_interaction(&scope, interaction_id)
            .await
            .unwrap()
            .unwrap()
            .0
            .update_account_snapshot
            .is_none()
    );
    let (result, _) = submit(
        &auth,
        &scope,
        interaction_id,
        "replacement using original flow snapshot",
    )
    .await
    .unwrap();
    assert_eq!(result.account_id, before.id);
    let actual = auth
        .read_account(&scope, before.id)
        .await
        .unwrap()
        .unwrap()
        .0;
    assert!(
        auth.secret_store
            .matches(
                &scope.resource,
                actual.access_secret.as_ref().unwrap(),
                &"replacement using original flow snapshot".into()
            )
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_occupied_output_rolls_back_new_original_without_cleanup_or_overwrite() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let id = setup(&auth, &scope).await;
    let target_account = CredentialAccountId::from_uuid(id.as_uuid());
    let target = access_handle(target_account, id).unwrap();
    auth.secret_store
        .put(
            scope.resource.clone(),
            target.clone(),
            "occupied actual output".into(),
        )
        .await
        .unwrap();
    let before = auth
        .secret_store
        .record_reference(&scope.resource, &target)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        submit(&auth, &scope, id, "must not replace occupied material").await,
        Err(AuthProductError::BackendConflict)
    ));
    assert!(
        auth.secret_store
            .custody(&scope.resource, &original_handle(id).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        auth.secret_store
            .record_reference(&scope.resource, &target)
            .await
            .unwrap(),
        Some(before)
    );
    assert!(
        auth.secret_store
            .matches(&scope.resource, &target, &"occupied actual output".into())
            .await
            .unwrap()
    );
    assert!(
        auth.read_account(&scope, target_account)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        auth.read_interaction(&scope, id)
            .await
            .unwrap()
            .unwrap()
            .0
            .consumed_at
            .is_none()
    );
    assert_eq!(
        auth.flows_for_scope(&scope)
            .await
            .unwrap()
            .pop()
            .unwrap()
            .status,
        AuthFlowStatus::AwaitingUser
    );
}

fn owner() -> AuthProductScope {
    let mut resource = ResourceScope::local_default(
        UserId::new("native-manual-owner").unwrap(),
        InvocationId::new(),
    )
    .unwrap();
    resource.thread_id = Some(ThreadId::new("native-manual-thread").unwrap());
    AuthProductScope::new(resource, AuthSurface::Web)
}

fn service(pool: &Arc<brassclaw_pg::PgPool>) -> PgAuthProductServices {
    PgAuthProductServices::new(
        pool.clone(),
        Arc::new(
            PgSecretStore::new(
                pool.as_ref().clone(),
                "native-manual-real-encrypted-store-master-key".into(),
                "native-manual-custody",
            )
            .unwrap(),
        ),
    )
}

async fn setup(service: &PgAuthProductServices, scope: &AuthProductScope) -> AuthInteractionId {
    match service
        .request_manual_token_flow(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: AuthProviderId::new("manual-provider").unwrap(),
            label: CredentialAccountLabel::new("manual-account").unwrap(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap()
    {
        AuthChallenge::ManualTokenRequired { interaction_id, .. } => interaction_id,
        other => panic!("unexpected challenge: {other:?}"),
    }
}

async fn submit(
    service: &PgAuthProductServices,
    scope: &AuthProductScope,
    interaction_id: AuthInteractionId,
    raw: &str,
) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
    service
        .submit_manual_token_flow(
            scope,
            SecretSubmitRequest {
                interaction_id,
                secret: raw.into(),
            },
        )
        .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_original_survives_restart_acknowledgement_and_later_account_change() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let scope = owner();
    let id = setup(&first, &scope).await;
    let (original, flow) = submit(&first, &scope, id, "native original raw input")
        .await
        .unwrap();
    let input = first
        .secret_store
        .custody(&scope.resource, &original_handle(id).unwrap())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&input.state, SecretCustodyState::Original { context }
        if !context.to_string().contains("native original raw input"))
    );
    assert!(
        first
            .secret_store
            .lease_once(&scope.resource, &input.handle)
            .await
            .is_err()
    );
    let account = first
        .get_account(CredentialAccountLookupRequest::new(
            scope.clone(),
            original.account_id,
        ))
        .await
        .unwrap()
        .unwrap();
    let target = account.access_secret.clone().unwrap();
    assert!(
        first
            .secret_store
            .matches(
                &account.scope.resource,
                &target,
                &"native original raw input".into()
            )
            .await
            .unwrap()
    );
    let output = first
        .secret_store
        .custody(&account.scope.resource, &target)
        .await
        .unwrap()
        .unwrap();
    first
        .update_status(&scope, account.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    let acknowledged = first
        .mark_continuation_dispatched(&scope, flow.id, Utc::now())
        .await
        .unwrap();
    let restarted = service(&rig.pool);
    // A completed original must not wait on an unrelated current account row.
    let mut connection = rig.pool.get().await.unwrap();
    let held = connection.transaction().await.unwrap();
    held.query_one(
        "SELECT id FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
        &[&scope.resource.tenant_id.as_str(), &account.id.to_string()],
    )
    .await
    .unwrap();
    let (recovered, resumed) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        submit(&restarted, &scope, id, "native original raw input"),
    )
    .await
    .expect("original-only recovery does not lock the current account")
    .unwrap();
    assert_eq!(recovered, original);
    assert_eq!(resumed, acknowledged);
    assert!(matches!(
        submit(&restarted, &scope, id, "changed raw input must conflict").await,
        Err(AuthProductError::BackendConflict)
    ));
    held.rollback().await.unwrap();
    assert_eq!(
        restarted
            .secret_store
            .custody(&scope.resource, &input.handle)
            .await
            .unwrap(),
        Some(input)
    );
    assert_eq!(
        restarted
            .secret_store
            .custody(&account.scope.resource, &target)
            .await
            .unwrap(),
        Some(output)
    );
    let current = restarted
        .get_account(CredentialAccountLookupRequest::new(scope, account.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.status, CredentialAccountStatus::Revoked);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_flow_rejection_rolls_back_input_output_account_and_interaction() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let id = setup(&auth, &scope).await;
    let before = auth.read_interaction(&scope, id).await.unwrap().unwrap();
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_manual_publication() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF NEW.data->>'status'='completed' THEN RAISE EXCEPTION 'actual native manual rejection'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER reject_manual_publication BEFORE UPDATE ON brassclaw_product_auth_flows
        FOR EACH ROW EXECUTE FUNCTION reject_manual_publication();").await.unwrap();
    assert!(
        submit(&auth, &scope, id, "rollback actual raw input")
            .await
            .is_err()
    );
    assert_eq!(
        auth.read_interaction(&scope, id).await.unwrap().unwrap(),
        before
    );
    assert!(
        auth.secret_store
            .custody(&scope.resource, &original_handle(id).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    let account_id = CredentialAccountId::from_uuid(id.as_uuid());
    assert!(
        auth.secret_store
            .custody(&scope.resource, &access_handle(account_id, id).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        auth.read_account(&scope, account_id)
            .await
            .unwrap()
            .is_none()
    );
    let flow = auth.flows_for_scope(&scope).await.unwrap().pop().unwrap();
    assert_eq!(flow.status, AuthFlowStatus::AwaitingUser);
    client
        .batch_execute(
            "DROP TRIGGER reject_manual_publication ON brassclaw_product_auth_flows;
        DROP FUNCTION reject_manual_publication();",
        )
        .await
        .unwrap();
    assert_eq!(
        submit(&auth, &scope, id, "rollback actual raw input")
            .await
            .unwrap()
            .0
            .account_id,
        account_id
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_lost_commit_recovers_original_without_reselecting_a_newer_account() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let direct = service(&rig.pool);
    let scope = owner();
    let id = setup(&direct, &scope).await;
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let unknown = service(&transport.pool);
    transport.arm(1).unwrap();
    assert!(
        submit(&unknown, &scope, id, "raw input with lost native commit")
            .await
            .is_err()
    );
    assert_eq!(transport.cut_count(), 1);
    let original = direct.flows_for_scope(&scope).await.unwrap().pop().unwrap();
    assert_eq!(original.status, AuthFlowStatus::Completed);
    let original_account = original.credential_account_id.unwrap();
    let replacement = direct
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: AuthProviderId::new("manual-provider").unwrap(),
            label: CredentialAccountLabel::new("manual-account").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(SecretHandle::new("newer-account-access").unwrap()),
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![],
        })
        .await
        .unwrap();
    assert_ne!(replacement.id, original_account);
    drop(unknown);
    transport.shutdown().await.unwrap();
    let restarted = service(&rig.pool);
    let (result, flow) = submit(&restarted, &scope, id, "raw input with lost native commit")
        .await
        .unwrap();
    assert_eq!(result.account_id, original_account);
    assert_eq!(flow, original);
    assert!(matches!(
        submit(&restarted, &scope, id, "another input after commit loss").await,
        Err(AuthProductError::BackendConflict)
    ));
    assert_eq!(
        restarted
            .read_account(&scope, replacement.id)
            .await
            .unwrap()
            .unwrap()
            .0,
        replacement
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_independent_changed_inputs_publish_exactly_one_winner() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let second = service(&rig.pool);
    let scope = owner();
    let id = setup(&first, &scope).await;
    let (a, b) = tokio::join!(
        submit(&first, &scope, id, "first native contender"),
        submit(&second, &scope, id, "second native contender")
    );
    let (winner, raw) = match (a, b) {
        (Ok(value), Err(AuthProductError::BackendConflict)) => (value, "first native contender"),
        (Err(AuthProductError::BackendConflict), Ok(value)) => (value, "second native contender"),
        other => panic!("one actual atomic winner required: {other:?}"),
    };
    let account = first
        .read_account(&scope, winner.0.account_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    assert!(
        first
            .secret_store
            .matches(
                &account.scope.resource,
                account.access_secret.as_ref().unwrap(),
                &raw.into()
            )
            .await
            .unwrap()
    );
    assert_eq!(submit(&second, &scope, id, raw).await.unwrap(), winner);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_cancellation_and_stale_bound_setup_cannot_publish_secrets() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let canceled = setup(&auth, &scope).await;
    auth.cancel_manual_token(&scope, canceled)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        submit(&auth, &scope, canceled, "canceled input must not publish").await,
        Err(AuthProductError::Canceled)
    ));
    assert!(
        auth.secret_store
            .custody(&scope.resource, &original_handle(canceled).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    let active = setup(&auth, &scope).await;
    let (account, _) = submit(&auth, &scope, active, "first account input")
        .await
        .unwrap();
    let original_account = auth
        .read_account(&scope, account.account_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    let challenge = auth
        .request_manual_token_flow(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: original_account.provider.clone(),
            label: original_account.label.clone(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: Some(CredentialAccountUpdateBinding {
                account_id: original_account.id,
                ownership: original_account.ownership,
                owner_extension: original_account.owner_extension.clone(),
                granted_extensions: original_account.granted_extensions.clone(),
            }),
            expires_at: Utc::now() + Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("manual challenge required");
    };
    let revoked = auth
        .update_status(
            &scope,
            original_account.id,
            CredentialAccountStatus::Revoked,
        )
        .await
        .unwrap();
    assert!(matches!(
        submit(
            &auth,
            &scope,
            interaction_id,
            "stale bound input must not publish"
        )
        .await,
        Err(AuthProductError::BackendConflict)
    ));
    assert!(
        auth.secret_store
            .custody(&scope.resource, &original_handle(interaction_id).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        auth.read_account(&scope, revoked.id)
            .await
            .unwrap()
            .unwrap()
            .0,
        revoked
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_unbound_setup_never_overwrites_another_account_without_its_binding() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let original_scope = owner();
    let first = setup(&auth, &original_scope).await;
    let (original, _) = submit(&auth, &original_scope, first, "earlier owner account input")
        .await
        .unwrap();
    let mut scope = original_scope.clone();
    scope.resource.invocation_id = InvocationId::new();
    scope.resource.thread_id = Some(ThreadId::new("another-owner-thread").unwrap());
    let id = setup(&auth, &scope).await;
    let (replacement, flow) = submit(&auth, &scope, id, "replacement from another owner thread")
        .await
        .unwrap();
    assert_ne!(replacement.account_id, original.account_id);
    assert_eq!(flow.scope, scope);
    let account = auth
        .read_account(&original_scope, original.account_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(account.scope, original_scope);
    assert!(
        auth.secret_store
            .matches(
                &original_scope.resource,
                account.access_secret.as_ref().unwrap(),
                &"earlier owner account input".into()
            )
            .await
            .unwrap()
    );
    let (_, recovered) = submit(
        &service(&rig.pool),
        &scope,
        id,
        "replacement from another owner thread",
    )
    .await
    .unwrap();
    assert_eq!(recovered, flow);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_abandon_preserves_completed_publication_and_consumed_proof() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let one = service(&rig.pool);
    let two = service(&rig.pool);
    let scope = owner();
    let id = setup(&one, &scope).await;
    let published = submit(&one, &scope, id, "completed disposition sentinel")
        .await
        .unwrap();
    let before = one.read_interaction(&scope, id).await.unwrap().unwrap();
    assert!(before.0.consumed_at.is_some());
    assert!(!two.abandon_manual_token(&scope, id).await.unwrap());
    assert!(!two.abandon_manual_token_flow(&scope, id).await.unwrap());
    assert_eq!(
        two.read_interaction(&scope, id).await.unwrap().unwrap(),
        before
    );
    assert_eq!(
        submit(&two, &scope, id, "completed disposition sentinel")
            .await
            .unwrap(),
        published
    );
    assert_eq!(
        two.read_flow(&scope, published.1.id)
            .await
            .unwrap()
            .unwrap()
            .0
            .status,
        AuthFlowStatus::Completed
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_abandon_retains_canceled_disposition_without_account_effects() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let one = service(&rig.pool);
    let two = service(&rig.pool);
    let scope = owner();
    let id = setup(&one, &scope).await;
    assert!(one.abandon_manual_token(&scope, id).await.unwrap());
    let closed = two.read_interaction(&scope, id).await.unwrap().unwrap();
    assert!(closed.0.canceled_at.is_some());
    assert!(closed.0.consumed_at.is_none());
    assert!(!two.abandon_manual_token(&scope, id).await.unwrap());
    assert_eq!(
        submit(&two, &scope, id, "canceled disposition sentinel")
            .await
            .unwrap_err(),
        AuthProductError::Canceled
    );
    assert!(two.accounts_for_scope(&scope).await.unwrap().is_empty());
    assert_eq!(
        two.read_interaction(&scope, id).await.unwrap().unwrap(),
        closed
    );
    assert!(two.abandon_manual_token_flow(&scope, id).await.unwrap());
    assert_eq!(
        two.flows_for_scope(&scope).await.unwrap()[0].status,
        AuthFlowStatus::Canceled
    );
    assert!(
        two.read_interaction(&scope, id)
            .await
            .unwrap()
            .unwrap()
            .0
            .canceled_at
            .is_some()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_abandon_reconciles_real_committed_update_with_lost_commit_ack() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let direct = service(&rig.pool);
    let scope = owner();
    let id = setup(&direct, &scope).await;
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let losing = service(&transport.pool);
    transport.arm(1).unwrap();
    assert!(losing.abandon_manual_token(&scope, id).await.unwrap());
    assert_eq!(transport.cut_count(), 1);
    assert!(
        direct
            .read_interaction(&scope, id)
            .await
            .unwrap()
            .unwrap()
            .0
            .canceled_at
            .is_some()
    );
    assert_eq!(
        submit(&direct, &scope, id, "canceled lost ack sentinel")
            .await
            .unwrap_err(),
        AuthProductError::Canceled
    );
    assert!(direct.accounts_for_scope(&scope).await.unwrap().is_empty());
    transport.shutdown().await.unwrap();
}

fn original_setup_request(scope: &AuthProductScope) -> ManualTokenSetupRequest {
    ManualTokenSetupRequest {
        requirement_setup: None,
        scope: scope.clone(),
        provider: AuthProviderId::new("manual-provider").unwrap(),
        label: CredentialAccountLabel::new("original-manual-account").unwrap(),
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        expires_at: Utc::now() + Duration::minutes(5),
    }
}

fn requirement_setup_request(
    scope: &AuthProductScope,
    previous: Option<brassclaw_auth::CredentialRequirementSelection>,
) -> ManualTokenSetupRequest {
    use brassclaw_host_api::{
        ExtensionId, NetworkScheme, NetworkTargetPattern, RuntimeCredentialAccountProviderId,
        RuntimeCredentialAccountRequirement, RuntimeCredentialAccountSetup,
        RuntimeCredentialRequirement, RuntimeCredentialRequirementSource, RuntimeCredentialTarget,
    };
    let usage = RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("manual-requirement-extension").unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("manual-usage-token").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("manual-provider").unwrap(),
                setup: RuntimeCredentialAccountSetup::ManualToken,
            },
            provider_scopes: vec!["read".into()],
            audience: NetworkTargetPattern {
                scheme: Some(NetworkScheme::Https),
                host_pattern: "api.manual.test".into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap();
    let mut request = original_setup_request(scope);
    request.requirement_setup = Some(Box::new(
        brassclaw_auth::CredentialRequirementSetup::new(scope, usage, previous).unwrap(),
    ));
    request
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_v1_original_keeps_its_historical_bound_scope_result() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let old_handle = SecretHandle::new("legacy-manual-access").unwrap();
    auth.secret_store
        .put(
            scope.resource.clone(),
            old_handle.clone(),
            "legacy preceding token".into(),
        )
        .await
        .unwrap();
    let previous = auth
        .create_account(NewCredentialAccount {
            scope: scope.clone(),
            provider: AuthProviderId::new("manual-provider").unwrap(),
            label: CredentialAccountLabel::new("legacy account").unwrap(),
            status: CredentialAccountStatus::Configured,
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
            access_secret: Some(old_handle),
            access_expires_at: None,
            refresh_secret: None,
            scopes: vec![brassclaw_auth::ProviderScope::new("read").unwrap()],
        })
        .await
        .unwrap();
    let identity = ManualTokenSetupIdentity::default();
    let mut request = original_setup_request(&scope);
    request.update_binding = Some(CredentialAccountUpdateBinding {
        account_id: previous.id,
        ownership: previous.ownership,
        owner_extension: previous.owner_extension.clone(),
        granted_extensions: previous.granted_extensions.clone(),
    });
    auth.request_original_manual_token_flow(identity, request)
        .await
        .unwrap();
    let (mut document, flow_revision) = auth
        .read_flow_document(&scope, identity.flow_id)
        .await
        .unwrap()
        .unwrap();
    let (mut pending, interaction_revision) = auth
        .read_interaction(&scope, identity.interaction_id)
        .await
        .unwrap()
        .unwrap();
    let now = Utc::now();
    let mut account = prepare_account(&pending, Some(&previous), now).unwrap();
    // Archive-shaped v1 producer: the old owner cleared these declared scopes.
    // Retain real encrypted input/output and native account/flow/interaction
    // records; this fixture does not claim to execute an old application image.
    account.scopes.clear();
    document.flow.status = AuthFlowStatus::Completed;
    document.flow.error = None;
    document.flow.credential_account_id = Some(account.id);
    document.flow.updated_at = now;
    let publication = ManualPublication {
        kind: LEGACY_ORIGINAL_KIND.into(),
        interaction: pending.clone(),
        expected_account: Some(previous.clone()),
        account: account.clone(),
        flow: Some(document.flow.clone()),
    };
    publication.verify(&scope, identity.interaction_id).unwrap();
    let mut mislabeled = publication.clone();
    mislabeled.kind = ORIGINAL_KIND.into();
    assert_eq!(
        mislabeled.verify(&scope, identity.interaction_id),
        Err(AuthProductError::BackendConflict)
    );
    let mut client = rig.pool.get().await.unwrap();
    let transaction = client.transaction().await.unwrap();
    let (_, account_revision) = callback::locked_account(&transaction, &scope, previous.id)
        .await
        .unwrap()
        .unwrap();
    auth.secret_store
        .put_original_and_retained_in_transaction(
            &transaction,
            (
                scope.resource.clone(),
                original_handle(identity.interaction_id).unwrap(),
            ),
            "legacy retained raw input".into(),
            serde_json::to_value(&publication).unwrap(),
            (
                account.scope.resource.clone(),
                access_handle(account.id, identity.interaction_id).unwrap(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
    callback::save_account(&transaction, &account, Some(account_revision))
        .await
        .unwrap();
    pending.consumed_at = Some(now);
    assert_eq!(
        transaction
            .execute(
                "UPDATE brassclaw_product_auth_interactions SET data=$3,revision=revision+1
         WHERE tenant_id=$1 AND id=$2 AND revision=$4",
                &[
                    &scope.resource.tenant_id.as_str(),
                    &identity.interaction_id.to_string(),
                    &serde_json::to_value(&pending).unwrap(),
                    &interaction_revision
                ],
            )
            .await
            .unwrap(),
        1
    );
    callback::save_flow(&transaction, &document, flow_revision)
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    let reopened = service(&rig.pool);
    assert_eq!(
        reopened
            .continue_original_manual_token_flow(&scope, identity)
            .await
            .unwrap()
            .0,
        publication.result()
    );
    assert_eq!(
        reopened
            .submit_original_manual_token_flow(&scope, identity, "legacy retained raw input".into())
            .await
            .unwrap()
            .0,
        publication.result()
    );
    assert!(
        reopened
            .read_account(&scope, previous.id)
            .await
            .unwrap()
            .unwrap()
            .0
            .scopes
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_requirement_publication_survives_commit_loss_and_later_selection() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let direct = service(&rig.pool);
    let scope = owner();
    let identity = ManualTokenSetupIdentity::default();
    let request = requirement_setup_request(&scope, None);
    let usage = request
        .requirement_setup
        .as_ref()
        .unwrap()
        .requirement()
        .clone();
    direct
        .request_original_manual_token_flow(identity, request)
        .await
        .unwrap();
    assert_eq!(
        direct
            .submit_manual_token(
                &scope,
                SecretSubmitRequest {
                    interaction_id: identity.interaction_id,
                    secret: "bare interaction cannot publish the requirement".into(),
                },
            )
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict,
    );
    assert!(
        direct
            .requirement_selection(&scope, &usage)
            .await
            .unwrap()
            .is_none()
    );
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let losing = service(&transport.pool);
    transport.arm(1).unwrap();
    assert_eq!(
        losing
            .submit_original_manual_token_flow(&scope, identity, "first requirement input".into())
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable,
    );
    assert_eq!(transport.cut_count(), 1);
    let reopened = service(&rig.pool);
    let first = reopened
        .requirement_selection(&scope, &usage)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.original_flow(), identity.flow_id);
    let original_source = crate::credential_requirement_custody_tests::published_source(
        &reopened,
        reopened.secret_store.as_ref(),
        &scope,
        &first,
    )
    .await;
    assert_eq!(first.account().scopes[0].as_str(), "read");
    let (original_result, original_flow) = reopened
        .continue_original_manual_token_flow(&scope, identity)
        .await
        .unwrap();
    assert_eq!(original_result.account_id, first.account().id);
    assert_eq!(original_flow.status, AuthFlowStatus::Completed);

    let successor = ManualTokenSetupIdentity::default();
    reopened
        .request_original_manual_token_flow(
            successor,
            requirement_setup_request(&scope, Some(first.clone())),
        )
        .await
        .unwrap();
    let stale = ManualTokenSetupIdentity::default();
    reopened
        .request_original_manual_token_flow(
            stale,
            requirement_setup_request(&scope, Some(first.clone())),
        )
        .await
        .unwrap();
    let (next, _) = reopened
        .submit_original_manual_token_flow(&scope, successor, "successor requirement input".into())
        .await
        .unwrap();
    let current = reopened
        .requirement_selection(&scope, &usage)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.account().id, next.account_id);
    assert_ne!(current.account().id, first.account().id);
    assert_eq!(current.previous(), Some(&first.reference().unwrap()));
    assert_eq!(
        reopened
            .requirement_source_for_flow(&scope, identity.flow_id, &usage)
            .await
            .unwrap(),
        Some(original_source)
    );
    assert_eq!(
        reopened
            .submit_original_manual_token_flow(&scope, stale, "stale selection input".into())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict,
    );
    assert!(
        reopened
            .secret_store
            .custody(
                &scope.resource,
                &original_handle(stale.interaction_id).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    // Recovering a completed operation neither reads today's account nor
    // republishes its old selection over a later deliberate successor.
    reopened
        .update_status(&scope, first.account().id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    assert_eq!(
        reopened
            .submit_original_manual_token_flow(&scope, identity, "first requirement input".into())
            .await
            .unwrap()
            .0,
        original_result,
    );
    assert_eq!(
        reopened
            .continue_original_manual_token_flow(&scope, identity)
            .await
            .unwrap()
            .0,
        original_result,
    );
    assert_eq!(
        reopened
            .submit_original_manual_token_flow(&scope, identity, "changed requirement input".into())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict,
    );
    assert_eq!(
        reopened
            .requirement_selection(&scope, &usage)
            .await
            .unwrap(),
        Some(current)
    );
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_requirement_competition_rolls_back_the_losing_whole_publication() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let second = service(&rig.pool);
    let scope = owner();
    let request = requirement_setup_request(&scope, None);
    let usage = request
        .requirement_setup
        .as_ref()
        .unwrap()
        .requirement()
        .clone();
    let a = ManualTokenSetupIdentity::default();
    let b = ManualTokenSetupIdentity::default();
    first
        .request_original_manual_token_flow(a, request.clone())
        .await
        .unwrap();
    second
        .request_original_manual_token_flow(b, request)
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        first.submit_original_manual_token_flow(&scope, a, "first concurrent input".into()),
        second.submit_original_manual_token_flow(&scope, b, "second concurrent input".into()),
    );
    let (winner, loser) = match (left, right) {
        (Ok(_), Err(AuthProductError::BackendConflict)) => (a, b),
        (Err(AuthProductError::BackendConflict), Ok(_)) => (b, a),
        other => panic!("one whole publication must win: {other:?}"),
    };
    let selected = first
        .requirement_selection(&scope, &usage)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.original_flow(), winner.flow_id);
    assert_eq!(first.accounts_for_scope(&scope).await.unwrap().len(), 1);
    assert_eq!(
        first
            .read_flow(&scope, loser.flow_id)
            .await
            .unwrap()
            .unwrap()
            .0
            .status,
        AuthFlowStatus::AwaitingUser,
    );
    assert!(
        first
            .read_interaction(&scope, loser.interaction_id)
            .await
            .unwrap()
            .unwrap()
            .0
            .consumed_at
            .is_none()
    );
    let account = CredentialAccountId::from_uuid(loser.interaction_id.as_uuid());
    assert!(
        first
            .secret_store
            .custody(
                &scope.resource,
                &original_handle(loser.interaction_id).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .secret_store
            .custody(
                &scope.resource,
                &access_handle(account, loser.interaction_id).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_requirement_flow_failure_cannot_leave_a_published_selection() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let identity = ManualTokenSetupIdentity::default();
    let request = requirement_setup_request(&scope, None);
    let usage = request
        .requirement_setup
        .as_ref()
        .unwrap()
        .requirement()
        .clone();
    auth.request_original_manual_token_flow(identity, request)
        .await
        .unwrap();
    let client = rig.pool.get().await.unwrap();
    client
        .batch_execute(
            "CREATE FUNCTION reject_requirement_flow() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'controlled requirement flow failure'; END $$;
         CREATE TRIGGER reject_requirement_flow BEFORE UPDATE ON brassclaw_product_auth_flows
         FOR EACH ROW EXECUTE FUNCTION reject_requirement_flow();",
        )
        .await
        .unwrap();
    assert_eq!(
        auth.submit_original_manual_token_flow(&scope, identity, "failed flow input".into())
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable,
    );
    assert!(
        auth.requirement_selection(&scope, &usage)
            .await
            .unwrap()
            .is_none()
    );
    assert!(auth.accounts_for_scope(&scope).await.unwrap().is_empty());
    assert!(
        auth.secret_store
            .custody(
                &scope.resource,
                &original_handle(identity.interaction_id).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        auth.read_flow(&scope, identity.flow_id)
            .await
            .unwrap()
            .unwrap()
            .0
            .status,
        AuthFlowStatus::AwaitingUser
    );
    client
        .batch_execute(
            "DROP TRIGGER reject_requirement_flow ON brassclaw_product_auth_flows;
         DROP FUNCTION reject_requirement_flow();",
        )
        .await
        .unwrap();
    let (result, flow) = auth
        .submit_original_manual_token_flow(&scope, identity, "failed flow input".into())
        .await
        .unwrap();
    assert_eq!(flow.status, AuthFlowStatus::Completed);
    assert_eq!(
        auth.requirement_selection(&scope, &usage)
            .await
            .unwrap()
            .unwrap()
            .account()
            .id,
        result.account_id
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_requirement_custody_survives_reopen_and_rejects_reinterpretation() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    transport.arm(1).unwrap();
    crate::credential_requirement_custody_tests::verify(
        &service(&transport.pool),
        &service(&rig.pool),
        owner(),
    )
    .await;
    assert_eq!(transport.cut_count(), 1);
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_setup_original_reconciles_actual_commit_loss_and_keeps_first_definition() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let scope = owner();
    let identity = ManualTokenSetupIdentity::default();
    let request = original_setup_request(&scope);
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let losing = service(&transport.pool);
    transport.arm(1).unwrap();
    let initial = losing
        .request_original_manual_token_flow(identity, request.clone())
        .await
        .unwrap();
    assert_eq!(transport.cut_count(), 1);
    assert_eq!(initial.id, identity.flow_id);
    assert_eq!(initial.status, AuthFlowStatus::AwaitingUser);
    assert_eq!(initial.expires_at, request.expires_at);
    let first = service(&rig.pool);
    let second = service(&rig.pool);
    let mut retried = request.clone();
    retried.expires_at += Duration::hours(2);
    let (a, b) = tokio::join!(
        first.request_original_manual_token_flow(identity, retried.clone()),
        second.request_original_manual_token_flow(identity, retried.clone())
    );
    assert_eq!(a.unwrap(), initial);
    assert_eq!(b.unwrap(), initial);
    let mut changed = retried.clone();
    changed.label = CredentialAccountLabel::new("changed-definition").unwrap();
    assert_eq!(
        first
            .request_original_manual_token_flow(identity, changed)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut foreign = retried.clone();
    foreign.scope.resource.invocation_id = InvocationId::new();
    assert_eq!(
        first
            .request_original_manual_token_flow(identity, foreign)
            .await
            .unwrap_err(),
        AuthProductError::CrossScopeDenied
    );
    assert_eq!(first.flows_for_scope(&scope).await.unwrap(), vec![initial]);
    assert!(first.accounts_for_scope(&scope).await.unwrap().is_empty());
    assert!(
        first
            .continue_original_manual_token_flow(&scope, identity)
            .await
            .is_err()
    );
    assert!(
        first
            .abandon_manual_token_flow(&scope, identity.interaction_id)
            .await
            .unwrap()
    );
    let terminal = second
        .request_original_manual_token_flow(identity, retried.clone())
        .await
        .unwrap();
    assert_eq!(terminal.status, AuthFlowStatus::Canceled);
    assert_eq!(terminal.expires_at, request.expires_at);
    assert_eq!(
        second
            .submit_original_manual_token_flow(&scope, identity, "canceled original".into())
            .await
            .unwrap_err(),
        AuthProductError::Canceled
    );
    assert_eq!(
        second
            .request_original_manual_token_flow(identity, retried)
            .await
            .unwrap(),
        terminal
    );
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_setup_original_fences_pair_before_effects_and_reference_uses_confirmed_original()
 {
    use crate::auth::{
        RebornManualTokenSetupRequest, RebornManualTokenSubmitRequest,
        RebornProductAuthServicePorts,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let native = Arc::new(service(&rig.pool));
    let scope = owner();
    let identity = ManualTokenSetupIdentity::default();
    let dispatcher = Arc::new(RejectFirstContinuation::default());
    let facade = RebornProductAuthServicePorts::from_shared_with_provider(
        native.clone(),
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(dispatcher.clone())
    .with_flow_record_source(native.clone());
    let request = RebornManualTokenSetupRequest::new(
        scope.clone(),
        AuthProviderId::new("manual-provider").unwrap(),
        CredentialAccountLabel::new("manual-account").unwrap(),
        AuthContinuationRef::SetupOnly,
        Utc::now() + Duration::minutes(5),
    );
    let receipt = facade
        .request_original_manual_token_setup(identity, request.clone())
        .await
        .unwrap();
    let wrong = ManualTokenSetupIdentity {
        flow_id: AuthFlowId::new(),
        ..identity
    };
    assert!(
        facade
            .submit_original_manual_token(
                wrong,
                RebornManualTokenSubmitRequest::new(
                    scope.clone(),
                    identity.interaction_id,
                    "original raw".into()
                )
            )
            .await
            .unwrap_err()
            .conflict
    );
    assert!(native.accounts_for_scope(&scope).await.unwrap().is_empty());
    assert!(
        native
            .secret_store
            .custody(
                &scope.resource,
                &original_handle(identity.interaction_id).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        facade
            .continue_original_manual_token(&scope, identity)
            .await
            .unwrap_err()
            .conflict
    );
    assert!(
        facade
            .submit_original_manual_token(
                identity,
                RebornManualTokenSubmitRequest::new(
                    scope.clone(),
                    identity.interaction_id,
                    "  original raw ü  ".into()
                )
            )
            .await
            .unwrap_err()
            .retryable
    );
    let completed = native
        .read_flow(&scope, identity.flow_id)
        .await
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    assert!(completed.continuation_emitted_at.is_none());
    assert!(
        facade
            .submit_original_manual_token(
                identity,
                RebornManualTokenSubmitRequest::new(
                    scope.clone(),
                    identity.interaction_id,
                    "changed raw".into()
                )
            )
            .await
            .unwrap_err()
            .conflict
    );
    assert_eq!(dispatcher.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    let account_id = completed.credential_account_id.unwrap();
    native
        .update_status(&scope, account_id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    let resumed = facade
        .continue_original_manual_token(&scope, identity)
        .await
        .unwrap();
    assert_eq!(resumed.account_id, account_id);
    assert_eq!(dispatcher.0.load(std::sync::atomic::Ordering::SeqCst), 2);
    let mut retry = request;
    retry.expires_at += Duration::hours(1);
    let terminal = facade
        .request_original_manual_token_setup(identity, retry)
        .await
        .unwrap();
    assert_eq!(terminal.status, AuthFlowStatus::Completed);
    assert_eq!(terminal.challenge, receipt.challenge);
    assert_eq!(
        native
            .get_account(CredentialAccountLookupRequest::new(
                scope.clone(),
                account_id
            ))
            .await
            .unwrap()
            .unwrap()
            .status,
        CredentialAccountStatus::Revoked
    );
    assert_eq!(
        facade
            .continue_original_manual_token(&scope, identity)
            .await
            .unwrap()
            .account_id,
        account_id
    );
    assert_eq!(dispatcher.0.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(
        !native
            .abandon_manual_token_flow(&scope, identity.interaction_id)
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_setup_recovery_reads_original_binding_without_new_setup_or_latest_account() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let service = service(&rig.pool);
    let scope = owner();
    let first_id = ManualTokenSetupIdentity::default();
    assert!(
        service
            .recover_original_manual_token_flow(&scope, first_id)
            .await
            .unwrap()
            .is_none()
    );
    service
        .request_original_manual_token_flow(first_id, original_setup_request(&scope))
        .await
        .unwrap();
    let (result, completed) = service
        .submit_original_manual_token_flow(
            &scope,
            first_id,
            secrecy::SecretString::from("original recovery fixture token".to_owned()),
        )
        .await
        .unwrap();
    let account = service
        .get_account(brassclaw_auth::CredentialAccountLookupRequest::new(
            scope.clone(),
            result.account_id,
        ))
        .await
        .unwrap()
        .unwrap();
    let second_id = ManualTokenSetupIdentity::default();
    let mut request = original_setup_request(&scope);
    request.update_binding = Some(
        brassclaw_auth::CredentialAccountUpdateBinding::from_projection(&account.projection()),
    );
    let original = service
        .request_original_manual_token_flow(second_id, request.clone())
        .await
        .unwrap();
    service
        .update_status(&scope, account.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    assert_eq!(
        service
            .recover_original_manual_token_flow(&scope, second_id)
            .await
            .unwrap(),
        Some(original.clone())
    );
    assert_eq!(
        service
            .recover_original_manual_token_flow(&scope, first_id)
            .await
            .unwrap(),
        Some(completed)
    );
    let mut wrong_scope = scope.clone();
    wrong_scope.resource.invocation_id = InvocationId::new();
    assert_eq!(
        service
            .recover_original_manual_token_flow(&wrong_scope, second_id)
            .await
            .unwrap_err(),
        AuthProductError::CrossScopeDenied
    );
    for wrong in [
        ManualTokenSetupIdentity {
            flow_id: AuthFlowId::new(),
            ..second_id
        },
        ManualTokenSetupIdentity {
            interaction_id: AuthInteractionId::new(),
            ..second_id
        },
    ] {
        assert_eq!(
            service
                .recover_original_manual_token_flow(&scope, wrong)
                .await
                .unwrap_err(),
            AuthProductError::BackendConflict
        );
    }
    assert!(
        service
            .abandon_manual_token_flow(&scope, second_id.interaction_id)
            .await
            .unwrap()
    );
    let canceled = service
        .recover_original_manual_token_flow(&scope, second_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(canceled.status, AuthFlowStatus::Canceled);
    assert_eq!(canceled.expires_at, original.expires_at);
    assert_eq!(canceled.update_binding, original.update_binding);
    let actual = service
        .get_account(brassclaw_auth::CredentialAccountLookupRequest::new(
            scope.clone(),
            account.id,
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(actual.status, CredentialAccountStatus::Revoked);
    let row=rig.pool.get().await.unwrap().query_one(
        "SELECT (SELECT count(*) FROM brassclaw_product_auth_interactions), (SELECT count(*) FROM brassclaw_product_auth_flows), (SELECT count(*) FROM brassclaw_product_auth_accounts)", &[],
    ).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 2);
    assert_eq!(row.get::<_, i64>(1), 2);
    assert_eq!(row.get::<_, i64>(2), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_extension_adapter_keeps_original_target_expiry_and_secret_proof() {
    use crate::auth::RebornProductAuthServicePorts;
    use brassclaw_product_workflow::{
        ExtensionCredentialPrepareRequest, ExtensionCredentialReferenceRequest,
        ExtensionCredentialSetupService, ExtensionCredentialSubmitRequest,
        ExtensionCredentialTarget,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let native = Arc::new(service(&rig.pool));
    let facade = RebornProductAuthServicePorts::from_shared_with_provider(
        native.clone(),
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(
        std::sync::atomic::AtomicUsize::new(1),
    )));
    let adapter = crate::webui_extension_credentials::ProductAuthExtensionCredentialSetup::new(
        Arc::new(facade),
    );
    let mut scope = owner();
    scope.surface = AuthSurface::Web;
    let first = ExtensionCredentialPrepareRequest {
        candidate_requirement: Some(crate::webui_extension_credentials::test_requirement(
            (AuthProviderId::new("native-extension").unwrap()).as_str(),
            (brassclaw_host_api::ExtensionId::new("native-extension").unwrap()).as_str(),
            "api_token",
            &[],
        )),

        requirement_name: "api_token".into(),

        scope: scope.clone(),
        requester_extension: brassclaw_host_api::ExtensionId::new("native-extension").unwrap(),
        identity: ManualTokenSetupIdentity::default(),
        target: ExtensionCredentialTarget::New {},
    };
    let reference =
        |request: &ExtensionCredentialPrepareRequest| ExtensionCredentialReferenceRequest {
            requirement_name: request.requirement_name.clone(),

            scope: request.scope.clone(),
            requester_extension: request.requester_extension.clone(),
            identity: request.identity,
        };
    assert_eq!(
        adapter
            .continue_manual_token(reference(&first))
            .await
            .unwrap_err()
            .status_code,
        404
    );
    let receipt = adapter.prepare_manual_token(first.clone()).await.unwrap();
    let raw = |request: &ExtensionCredentialPrepareRequest, value: &str| {
        ExtensionCredentialSubmitRequest {
            requirement_name: request.requirement_name.clone(),

            scope: request.scope.clone(),
            requester_extension: request.requester_extension.clone(),
            identity: request.identity,
            secret: value.to_owned().into(),
        }
    };
    let published = adapter
        .submit_manual_token(raw(&first, "native raw ü"))
        .await
        .unwrap();
    assert_eq!(
        adapter
            .submit_manual_token(raw(&first, "native raw ü"))
            .await
            .unwrap(),
        published
    );
    assert_eq!(
        adapter
            .submit_manual_token(raw(&first, "changed raw"))
            .await
            .unwrap_err()
            .status_code,
        409
    );
    let update = ExtensionCredentialPrepareRequest {
        identity: ManualTokenSetupIdentity::default(),
        target: ExtensionCredentialTarget::Existing {
            account_id: published.credential_ref,
        },
        ..first.clone()
    };
    let bound = adapter.prepare_manual_token(update.clone()).await.unwrap();
    native
        .update_status(
            &scope,
            published.credential_ref,
            CredentialAccountStatus::Revoked,
        )
        .await
        .unwrap();
    assert_eq!(
        adapter.prepare_manual_token(update.clone()).await.unwrap(),
        bound,
        "metadata repeat retains its prechallenge binding after account changes"
    );
    assert_eq!(
        adapter
            .prepare_manual_token(ExtensionCredentialPrepareRequest {
                target: ExtensionCredentialTarget::New {},
                ..update.clone()
            })
            .await
            .unwrap_err()
            .status_code,
        409
    );
    assert_eq!(
        adapter
            .prepare_manual_token(first.clone())
            .await
            .unwrap()
            .expires_at,
        receipt.expires_at
    );
    assert_eq!(
        adapter
            .continue_manual_token(reference(&first))
            .await
            .unwrap(),
        published
    );
    let canceled = adapter
        .cancel_manual_token(reference(&update))
        .await
        .unwrap();
    assert_eq!(canceled.status, AuthFlowStatus::Canceled);
    let client = rig.pool.get().await.unwrap();
    let counts=client.query_one("SELECT (SELECT count(*) FROM brassclaw_product_auth_flows), (SELECT count(*) FROM brassclaw_product_auth_interactions), (SELECT count(*) FROM brassclaw_product_auth_accounts)",&[]).await.unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, i64>(1),
            counts.get::<_, i64>(2)
        ),
        (2, 2, 1)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_extension_elapsed_expiry_reads_metadata_without_accepting_input() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let native = Arc::new(service(&rig.pool));
    let facade = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native.clone(),
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(
        std::sync::atomic::AtomicUsize::new(1),
    )));
    let (scope, pending, _) =
        crate::webui_extension_credentials::verify_extension_metadata_expiry(facade, owner()).await;
    assert!(
        native
            .secret_store
            .custody(
                &scope.resource,
                &original_handle(pending.interaction_id).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    let target = access_handle(
        CredentialAccountId::from_uuid(pending.interaction_id.as_uuid()),
        pending.interaction_id,
    )
    .unwrap();
    assert!(
        native
            .secret_store
            .record_reference(&scope.resource, &target)
            .await
            .unwrap()
            .is_none()
    );
    let client = rig.pool.get().await.unwrap();
    let row=client.query_one("SELECT (SELECT count(*) FROM brassclaw_product_auth_flows), (SELECT count(*) FROM brassclaw_product_auth_interactions), (SELECT count(*) FROM brassclaw_product_auth_accounts)",&[]).await.unwrap();
    assert_eq!(
        (
            row.get::<_, i64>(0),
            row.get::<_, i64>(1),
            row.get::<_, i64>(2)
        ),
        (2, 2, 1)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_extension_requirement_binding_controls_readiness_and_original_recovery() {
    use crate::auth::RebornProductAuthServicePorts;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let native = Arc::new(service(&rig.pool));
    let facade = RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(
        std::sync::atomic::AtomicUsize::new(1),
    )));
    crate::webui_extension_credentials::verify_extension_requirement_readiness(facade, owner())
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_google_setup_preserves_original_identity_and_verifier() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let native = Arc::new(service(&rig.pool));
    let secrets = native.secret_store.clone();
    let facade = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(
        std::sync::atomic::AtomicUsize::new(1),
    )));
    crate::product_auth_serve::verify_google_original_lifecycle(Arc::new(facade), secrets).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_google_setup_recovers_actual_lost_insert_ack_without_new_pkce() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    // This owner writes its initial flow with an autocommit INSERT. The real
    // driver awaits ReadyForQuery after CommandComplete; withhold that actual
    // idle acknowledgement only for the bound initial Google flow document.
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"google".to_vec(),
                b"opaque_state_hash".to_vec(),
                b"awaiting_user".to_vec(),
            ],
            vec![],
            brassclaw_pg::test_support::AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let native = Arc::new(service(&transport.pool));
    let secrets = native.secret_store.clone();
    let facade = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    )
    .into_services(Arc::new(RejectFirstContinuation(
        std::sync::atomic::AtomicUsize::new(1),
    )));
    transport.arm(1).unwrap();
    crate::product_auth_serve::verify_google_original_lifecycle(Arc::new(facade), secrets).await;
    assert_eq!(transport.cut_count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_oauth_requirement_handoff_keeps_target_head_and_callback_original() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let native = Arc::new(service(&rig.pool));
    let secrets = native.secret_store.clone();
    let ports = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    );
    crate::product_auth_serve::verify_dcr_oauth_usage(ports, secrets).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_preparation_restores_expired_cancel_and_live_originals_without_replay() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![
                b"dcr_registration".to_vec(),
                b"not_registered".to_vec(),
                b"expired".to_vec(),
            ],
            vec![],
            brassclaw_pg::test_support::AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let native = Arc::new(service(&transport.pool));
    let secrets = native.secret_store.clone();
    let ports = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native,
        Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
    );
    transport.arm(1).unwrap();
    crate::product_auth_serve::verify_dcr_preparation_handoff(
        ports,
        secrets,
        &rig.pool,
        crate::product_auth_serve::DcrPreparationBackend::NativePostgres,
    )
    .await;
    assert_eq!(
        transport.cut_count(),
        1,
        "the actual terminal flow INSERT acknowledgement was cut"
    );
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_oauth_setup_original_keeps_custody_and_reconciles_insert_loss() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let transport = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![b"oauth_setup".to_vec(), b"expired".to_vec()],
            vec![],
            brassclaw_pg::test_support::AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let first = Arc::new(service(&transport.pool));
    let second = Arc::new(service(&transport.pool));
    let secrets = first.secret_store.clone();
    transport.arm(1).unwrap();
    crate::auth_oauth_setup_custody::tests::verify_storage(
        first.clone(),
        second,
        first,
        secrets,
        &rig.pool,
        crate::auth_oauth_setup_custody::tests::Backend::NativePostgres,
        owner(),
    )
    .await;
    assert_eq!(
        transport.cut_count(),
        1,
        "actual expired setup INSERT acknowledgement was cut"
    );
    transport.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_google_private_client_and_pkce_survive_restart_callback_and_refresh() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = Arc::new(service(&rig.pool));
    let second = Arc::new(service(&rig.pool));
    let first_secrets = first.secret_store.clone();
    let second_secrets = second.secret_store.clone();
    crate::oauth_gate::verify_native_private_runtime(
        crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
            first,
            Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
        ),
        crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
            second,
            Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
        ),
        first_secrets,
        second_secrets,
        &rig.pool,
        crate::auth_oauth_setup_custody::tests::Backend::NativePostgres,
        owner(),
    )
    .await;
}
