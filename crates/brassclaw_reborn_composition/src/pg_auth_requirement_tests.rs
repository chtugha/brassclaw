//! Actual native transactions; no credential generation/issuer call is replayed.
use super::*;
use crate::runtime::test_pg::native_pg::NativePostgres;
use brassclaw_auth::{
    AuthContinuationRef, AuthFlowKind, AuthSurface, CredentialRequirementSelection,
    CredentialRequirementSetup, CredentialSelectionInput, OAuthCallbackAdmission,
    OAuthCallbackOriginal, OAuthCallbackPrepared, OAuthProviderExchange, OAuthTokenDestinations,
    ProviderScope,
};
use brassclaw_host_api::{
    ExtensionId, InvocationId, NetworkTargetPattern, ResourceScope,
    RuntimeCredentialAccountProviderId, RuntimeCredentialAccountRequirement,
    RuntimeCredentialAccountSetup, RuntimeCredentialRequirement,
    RuntimeCredentialRequirementSource, RuntimeCredentialTarget, UserId,
};
use brassclaw_pg::test_support::{AckCutRule, AckCutTransport};
use secrecy::ExposeSecret;

fn owner() -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope::local_default(
            UserId::new("other-requirement-owner").unwrap(),
            InvocationId::new(),
        )
        .unwrap(),
        AuthSurface::Web,
    )
}
fn service(pool: &Arc<brassclaw_pg::PgPool>) -> PgAuthProductServices {
    PgAuthProductServices::new(
        pool.clone(),
        Arc::new(
            PgSecretStore::new(
                pool.as_ref().clone(),
                "native-other-requirement-secret-master-key".into(),
                "other-requirement-tests",
            )
            .unwrap(),
        ),
    )
}
fn usage(oauth: bool) -> RuntimeCredentialAccountRequirement {
    RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("requirement-extension").unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("usage-token").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("requirement-provider").unwrap(),
                setup: if oauth {
                    RuntimeCredentialAccountSetup::OAuth {
                        scopes: vec!["read".into()],
                    }
                } else {
                    RuntimeCredentialAccountSetup::ManualToken
                },
            },
            provider_scopes: vec!["read".into()],
            audience: NetworkTargetPattern {
                scheme: None,
                host_pattern: "api.requirement.test".into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap()
}
async fn account(
    auth: &PgAuthProductServices,
    scope: &AuthProductScope,
    name: &str,
) -> CredentialAccount {
    let handle = SecretHandle::new(format!("requirement-access-{name}")).unwrap();
    auth.secret_store
        .put(
            scope.resource.clone(),
            handle.clone(),
            "actual retained test credential".into(),
        )
        .await
        .unwrap();
    auth.create_account(NewCredentialAccount {
        scope: scope.clone(),
        provider: AuthProviderId::new("requirement-provider").unwrap(),
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
async fn flow(
    auth: &PgAuthProductServices,
    scope: &AuthProductScope,
    previous: Option<CredentialRequirementSelection>,
    accounts: Option<&[CredentialAccount]>,
) -> AuthFlowRecord {
    let oauth = accounts.is_none();
    let expires_at = Utc::now() + chrono::Duration::minutes(5);
    let id = AuthFlowId::new();
    auth.create_flow(NewAuthFlow {
        requirement_setup: Some(Box::new(
            CredentialRequirementSetup::new(scope, usage(oauth), previous).unwrap(),
        )),
        id: Some(id),
        scope: scope.clone(),
        kind: AuthFlowKind::IntegrationCredential,
        provider: AuthProviderId::new("requirement-provider").unwrap(),
        challenge: if let Some(accounts) = accounts {
            AuthChallenge::AccountSelectionRequired {
                provider: AuthProviderId::new("requirement-provider").unwrap(),
                accounts: accounts.iter().map(CredentialAccount::projection).collect(),
            }
        } else {
            AuthChallenge::OAuthUrl {
                authorization_url: brassclaw_auth::OAuthAuthorizationUrl::new(
                    "https://issuer.requirement.test/authorize",
                )
                .unwrap(),
                expires_at,
            }
        },
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        opaque_state_hash: oauth
            .then(|| brassclaw_auth::OpaqueStateHash::new("a".repeat(64)).unwrap()),
        pkce_verifier_hash: oauth
            .then(|| brassclaw_auth::PkceVerifierHash::new("b".repeat(64)).unwrap()),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_runtime_requirement_selects_exact_account_and_authenticated_material() {
    use brassclaw_host_runtime::{
        RuntimeCredentialAccountRequest, RuntimeCredentialAccountResolver,
    };

    use crate::product_auth_runtime_credentials::{
        ProductAuthRuntimeCredentialAccountSelector, ProductAuthRuntimeCredentialResolver,
        RuntimeCredentialAccountSelectionRequest, RuntimeCredentialAccountSelectionService,
    };

    let rig = NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = Arc::new(service(&rig.pool));
    let scope = owner();
    let a = account(&auth, &scope, "runtime-first").await;
    let b = account(&auth, &scope, "runtime-second").await;
    let original = flow(&auth, &scope, None, Some(&[a.clone(), b.clone()])).await;
    auth.complete_credential_selection(&scope, choice(&original, &a))
        .await
        .unwrap();
    let requirement = usage(false);
    let mut runtime_scope = scope.clone();
    runtime_scope.surface = AuthSurface::Api;
    runtime_scope.resource.invocation_id = InvocationId::new();
    runtime_scope.resource.thread_id =
        Some(brassclaw_host_api::ThreadId::new("native-runtime-requirement").unwrap());
    let mut owner_scope = runtime_scope.clone();
    owner_scope.resource.thread_id = None;
    let selector = ProductAuthRuntimeCredentialAccountSelector::new(auth.clone());
    let request = || {
        RuntimeCredentialAccountSelectionRequest::new(
            CredentialAccountSelectionRequest::new(owner_scope.clone(), a.provider.clone())
                .for_extension(requirement.requester_extension().clone()),
            runtime_scope.clone(),
            a.scopes.clone(),
        )
    };
    assert_eq!(
        selector
            .select_unique_configured_runtime_account(request())
            .await
            .unwrap_err(),
        AuthProductError::AccountSelectionRequired
    );
    assert_eq!(
        selector
            .select_unique_configured_runtime_account(
                request().with_requirement(Some(&requirement))
            )
            .await
            .unwrap()
            .id,
        a.id
    );
    let mut different = requirement.requirement().clone();
    different.audience.host_pattern = "different.requirement.test".into();
    let different = RuntimeCredentialAccountRequirement::new(
        requirement.requester_extension().clone(),
        different,
    )
    .unwrap();
    assert_eq!(
        selector
            .select_unique_configured_runtime_account(request().with_requirement(Some(&different)))
            .await
            .unwrap_err(),
        AuthProductError::CredentialMissing
    );
    let resolver =
        ProductAuthRuntimeCredentialResolver::new(Arc::new(selector), auth.secret_store.clone());
    let provider = RuntimeCredentialAccountProviderId::new(a.provider.as_str()).unwrap();
    let provider_scopes = requirement.requirement().provider_scopes.clone();
    let runtime_request = || RuntimeCredentialAccountRequest {
        requirement: Some(&requirement),
        scope: &runtime_scope.resource,
        provider: &provider,
        provider_scopes: &provider_scopes,
        requester_extension: requirement.requester_extension(),
    };
    let source = resolver
        .resolve_access_secret(runtime_request())
        .await
        .unwrap();
    assert_eq!(Some(source.handle.clone()), a.access_secret);
    assert_eq!(source.scope, a.scope.resource);
    assert_eq!(
        Some(source.record_reference.clone()),
        auth.secret_store
            .record_reference(&source.scope, &source.handle)
            .await
            .unwrap()
    );
    // The exact source produced by the real resolver cannot lease a later
    // replacement, including equal plaintext under a newly encrypted generation.
    auth.secret_store
        .put(
            source.scope.clone(),
            source.handle.clone(),
            "actual retained test credential".into(),
        )
        .await
        .unwrap();
    assert!(
        auth.secret_store
            .lease_once_if_record_reference(
                &source.scope,
                &source.handle,
                &source.record_reference,
            )
            .await
            .unwrap()
            .is_none()
    );
    auth.update_status(&scope, a.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    assert_eq!(
        resolver
            .resolve_access_secret(runtime_request())
            .await
            .unwrap_err(),
        brassclaw_host_api::CredentialStageError::AuthRequired
    );
    assert_eq!(
        auth.account_record(&scope, b.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        CredentialAccountStatus::Configured
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_account_choice_head_commits_with_original_and_recovers_after_lost_ack() {
    let rig = NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let a = account(&auth, &scope, "first").await;
    let b = account(&auth, &scope, "second").await;
    let initial = flow(&auth, &scope, None, Some(&[a.clone(), b.clone()])).await;
    let transport =
        AckCutTransport::start(rig.url.expose_secret(), vec![AckCutRule::next_commit(1)])
            .await
            .unwrap();
    let cut = service(&transport.pool);
    transport.arm(1).unwrap();
    assert_eq!(
        cut.complete_credential_selection(&scope, choice(&initial, &a))
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    let selected = auth
        .requirement_selection(&scope, &usage(false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.original_flow(), initial.id);
    let original = auth
        .complete_credential_selection(&scope, choice(&initial, &a))
        .await
        .unwrap();
    let next = flow(&auth, &scope, Some(selected), Some(&[a.clone(), b.clone()])).await;
    auth.complete_credential_selection(&scope, choice(&next, &b))
        .await
        .unwrap();
    let current = auth
        .requirement_selection(&scope, &usage(false))
        .await
        .unwrap()
        .unwrap();
    auth.update_status(&scope, a.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    assert_eq!(
        auth.complete_credential_selection(&scope, choice(&initial, &a))
            .await
            .unwrap(),
        original
    );
    assert_eq!(
        auth.requirement_selection(&scope, &usage(false))
            .await
            .unwrap(),
        Some(current.clone())
    );
    let c = account(&auth, &scope, "third").await;
    let pending = flow(
        &auth,
        &scope,
        Some(current.clone()),
        Some(std::slice::from_ref(&c)),
    )
    .await;
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_requirement_choice() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.data->>'status'='completed' THEN RAISE EXCEPTION 'controlled final choice rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_requirement_choice BEFORE UPDATE ON brassclaw_product_auth_flows FOR EACH ROW EXECUTE FUNCTION reject_requirement_choice();").await.unwrap();
    assert_eq!(
        auth.complete_credential_selection(&scope, choice(&pending, &c))
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    assert_eq!(
        auth.requirement_selection(&scope, &usage(false))
            .await
            .unwrap(),
        Some(current)
    );
    assert_eq!(
        auth.get_flow(&scope, pending.id).await.unwrap(),
        Some(pending.clone())
    );
    client.batch_execute("DROP TRIGGER reject_requirement_choice ON brassclaw_product_auth_flows; DROP FUNCTION reject_requirement_choice();").await.unwrap();
    auth.complete_credential_selection(&scope, choice(&pending, &c))
        .await
        .unwrap();
    assert_eq!(
        auth.requirement_selection(&scope, &usage(false))
            .await
            .unwrap()
            .unwrap()
            .original_flow(),
        pending.id
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_account_choice_competitors_cannot_complete_losing_originals() {
    let rig = NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let other = service(&rig.pool);
    let scope = owner();
    let a = account(&auth, &scope, "first").await;
    let b = account(&auth, &scope, "second").await;
    let first = flow(&auth, &scope, None, Some(&[a.clone(), b.clone()])).await;
    let second = flow(&auth, &scope, None, Some(&[a.clone(), b.clone()])).await;
    let (one, two) = tokio::join!(
        auth.complete_credential_selection(&scope, choice(&first, &a)),
        other.complete_credential_selection(&scope, choice(&second, &b))
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let (winner, loser, error) = if one.is_ok() {
        (first, second, two)
    } else {
        (second, first, one)
    };
    assert_eq!(error.unwrap_err(), AuthProductError::BackendConflict);
    assert_eq!(auth.get_flow(&scope, loser.id).await.unwrap(), Some(loser));
    assert_eq!(
        auth.requirement_selection(&scope, &usage(false))
            .await
            .unwrap()
            .unwrap()
            .original_flow(),
        winner.id
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_oauth_requirement_uses_retained_return_through_publication_failure() {
    let rig = NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let scope = owner();
    let initial = flow(&auth, &scope, None, None).await;
    let prepared = OAuthCallbackPrepared {
        original: OAuthCallbackOriginal {
            scope: scope.clone(),
            flow_id: initial.id,
            provider: initial.provider.clone(),
            opaque_state_hash: initial.opaque_state_hash.clone().unwrap(),
            authorization_code_hash: brassclaw_auth::AuthorizationCodeHash::new("c".repeat(64))
                .unwrap(),
            pkce_verifier_hash: initial.pkce_verifier_hash.clone().unwrap(),
            account_label: CredentialAccountLabel::new("oauth original").unwrap(),
            requested_scopes: vec![ProviderScope::new("read").unwrap()],
        },
        destinations: OAuthTokenDestinations {
            access_secret: SecretHandle::new("oauth-access").unwrap(),
            refresh_secret: SecretHandle::new("oauth-refresh").unwrap(),
        },
    };
    let action = match auth
        .claim_oauth_callback(&scope, prepared.clone())
        .await
        .unwrap()
    {
        OAuthCallbackAdmission::Dispatch(state) => state.action,
        other => panic!("expected actual original dispatch: {other:?}"),
    };
    auth.secret_store
        .put(
            scope.resource.clone(),
            action.destinations.access_secret.clone(),
            "actual oauth-returned test credential".into(),
        )
        .await
        .unwrap();
    let exchange = OAuthProviderExchange {
        provider: initial.provider.clone(),
        account_label: action.original.account_label.clone(),
        authorization_code_hash: action.original.authorization_code_hash.clone(),
        pkce_verifier_hash: action.original.pkce_verifier_hash.clone(),
        access_secret: action.destinations.access_secret.clone(),
        access_expires_at: None,
        refresh_secret: None,
        scopes: action.original.requested_scopes.clone(),
        account_id: None,
    };
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_requirement_oauth() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.data->>'status'='completed' THEN RAISE EXCEPTION 'controlled final oauth rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_requirement_oauth BEFORE UPDATE ON brassclaw_product_auth_flows FOR EACH ROW EXECUTE FUNCTION reject_requirement_oauth();").await.unwrap();
    assert_eq!(
        auth.complete_oauth_callback(
            &scope,
            OAuthCallbackInput {
                dispatched: Some(action.clone()),
                flow_id: initial.id,
                opaque_state_hash: initial.opaque_state_hash.clone().unwrap(),
                outcome: ProviderCallbackOutcome::Authorized {
                    exchange: exchange.clone()
                },
            }
        )
        .await
        .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    assert!(
        auth.requirement_selection(&scope, &usage(true))
            .await
            .unwrap()
            .is_none()
    );
    let reference = action.original.reference();
    let retained = auth
        .read_oauth_callback_original(&scope, &reference)
        .await
        .unwrap()
        .unwrap();
    assert!(retained.action.needs_publication());
    assert_eq!(
        retained.action.outcome,
        brassclaw_auth::OAuthCallbackOutcome::Returned(exchange)
    );
    assert!(
        auth.account_record(&scope, action.target_account_id())
            .await
            .unwrap()
            .is_none()
    );
    client.batch_execute("DROP TRIGGER reject_requirement_oauth ON brassclaw_product_auth_flows; DROP FUNCTION reject_requirement_oauth();").await.unwrap();
    let reopened = service(&rig.pool);
    let settled = reopened
        .reconcile_oauth_callback(&scope, &reference)
        .await
        .unwrap();
    assert_eq!(settled.flow.status, AuthFlowStatus::Completed);
    let selected = reopened
        .requirement_selection(&scope, &usage(true))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.original_flow(), initial.id);
    assert_eq!(selected.account().id, action.target_account_id());
    let original_source = crate::credential_requirement_custody_tests::published_source(
        &reopened,
        reopened.secret_store.as_ref(),
        &scope,
        &selected,
    )
    .await;
    let replay = reopened
        .reconcile_oauth_callback(&scope, &reference)
        .await
        .unwrap();
    assert_eq!(replay.flow, settled.flow);
    assert_eq!(replay.action, settled.action);
    assert_eq!(
        reopened
            .requirement_source_for_flow(&scope, initial.id, &usage(true))
            .await
            .unwrap(),
        Some(original_source)
    );
    assert!(matches!(
        reopened
            .claim_oauth_callback(&scope, prepared)
            .await
            .unwrap(),
        OAuthCallbackAdmission::Original(_)
    ));
    assert!(matches!(
        reopened
            .complete_credential_selection(
                &scope,
                CredentialSelectionInput {
                    flow_id: initial.id,
                    credential_account_id: action.target_account_id(),
                }
            )
            .await,
        Err(AuthProductError::InvalidRequest { .. })
    ));
}

/// Actual PG runner custody only; no native Tool/Auth-flow activation claim.
#[tokio::test]
async fn native_auth_wait_retains_pg_claim_on_resume_and_fences_missing_cancel_ack() {
    use brassclaw_host_api::{AgentId, AuthDispatchBoundary, ProjectId, TenantId, ThreadId};
    use brassclaw_turns::{
        AcceptedMessageRef, AllowAllTurnAdmissionPolicy, CancelRunRequest, IdempotencyKey,
        InMemoryRunProfileResolver, PgTurnStateStore, ReplyTargetBindingRef,
        ResumeTurnPrecondition, ResumeTurnRequest, SanitizedCancelReason, SourceBindingRef,
        SubmitTurnRequest, TurnActor, TurnLeaseToken, TurnRunnerId, TurnScope,
        TurnSpawnTreeStateStore, TurnStateStore, TurnStatus,
        runner::{
            ClaimRunRequest, HeartbeatRequest, RecoverExpiredLeasesRequest, TurnRunTransitionPort,
            native_auth::BlockNativeAuthRequest,
        },
    };
    let pg = NativePostgres::start().await;
    let tenant = "native-auth-wait-pg";
    let store = PgTurnStateStore::new(pg.pool.clone(), tenant);
    let actor = TurnActor::new(UserId::new("native-auth-wait-owner").unwrap());
    for cancel in [false, true] {
        let label = if cancel {
            "native-auth-cancel"
        } else {
            "native-auth-resume"
        };
        let scope = TurnScope::new(
            TenantId::new(tenant).unwrap(),
            Some(AgentId::new("native-auth-agent").unwrap()),
            Some(ProjectId::new("native-auth-project").unwrap()),
            ThreadId::new(label).unwrap(),
        );
        store
            .submit_turn(
                SubmitTurnRequest {
                    scope: scope.clone(),
                    actor: actor.clone(),
                    accepted_message_ref: AcceptedMessageRef::new(format!("message-{label}"))
                        .unwrap(),
                    source_binding_ref: SourceBindingRef::new("source-web").unwrap(),
                    reply_target_binding_ref: ReplyTargetBindingRef::new("reply-web").unwrap(),
                    requested_run_profile: None,
                    idempotency_key: IdempotencyKey::new(label).unwrap(),
                    received_at: Utc::now(),
                    requested_run_id: None,
                    parent_run_id: None,
                    subagent_depth: 0,
                    spawn_tree_root_run_id: None,
                },
                &AllowAllTurnAdmissionPolicy,
                &InMemoryRunProfileResolver::default(),
            )
            .await
            .unwrap();
        let claim = store
            .claim_next_run(ClaimRunRequest {
                runner_id: TurnRunnerId::new(),
                lease_token: TurnLeaseToken::new(),
                scope_filter: Some(scope.clone()),
            })
            .await
            .unwrap()
            .unwrap();
        let wait = BlockNativeAuthRequest {
            scope: scope.clone(),
            run_id: claim.state.run_id,
            runner_id: claim.runner_id,
            lease_token: claim.lease_token,
            gate_ref: brassclaw_turns::GateRef::new(format!(
                "gate:auth:native:{}",
                InvocationId::new()
            ))
            .unwrap(),
            boundary: AuthDispatchBoundary::BeforeDispatch,
            credential_requirements: vec![brassclaw_host_api::RuntimeCredentialAuthRequirement {
                requirement: Some(usage(false)),
                provider: RuntimeCredentialAccountProviderId::new("requirement-provider").unwrap(),
                requester_extension: ExtensionId::new("requirement-extension").unwrap(),
                provider_scopes: vec!["read".into()],
            }],
        };
        let blocked = store.block_native_auth(wait.clone()).await.unwrap();
        assert_eq!(blocked.status, TurnStatus::BlockedAuth);
        let reopened = PgTurnStateStore::new(pg.pool.clone(), tenant);
        let original = reopened
            .get_run_record(&scope, wait.run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(original.runner_id, Some(claim.runner_id));
        assert_eq!(original.lease_token, Some(claim.lease_token));
        assert_eq!(original.claim_count, 1);
        assert!(original.checkpoint_id.is_none());
        assert_eq!(
            original
                .native_auth_wait
                .as_ref()
                .unwrap()
                .gate_ref()
                .unwrap(),
            wait.gate_ref
        );
        assert_eq!(
            reopened
                .heartbeat(HeartbeatRequest {
                    run_id: wait.run_id,
                    runner_id: wait.runner_id,
                    lease_token: wait.lease_token,
                })
                .await
                .unwrap(),
            blocked.event_cursor
        );
        let input = ResumeTurnRequest {
            scope: scope.clone(),
            actor: actor.clone(),
            run_id: wait.run_id,
            gate_resolution_ref: blocked.gate_ref.unwrap(),
            source_binding_ref: blocked.source_binding_ref,
            reply_target_binding_ref: blocked.reply_target_binding_ref,
            idempotency_key: IdempotencyKey::new("native-auth-pg-resume").unwrap(),
            precondition: ResumeTurnPrecondition::BlockedAuthGate,
        };
        if cancel {
            let cancelled = reopened
                .request_cancel(CancelRunRequest {
                    scope: scope.clone(),
                    actor: actor.clone(),
                    run_id: wait.run_id,
                    reason: SanitizedCancelReason::OperatorRequested,
                    idempotency_key: IdempotencyKey::new("native-auth-pg-cancel").unwrap(),
                })
                .await
                .unwrap();
            assert_eq!(cancelled.status, TurnStatus::CancelRequested);
            assert!(
                reopened
                    .heartbeat(HeartbeatRequest {
                        run_id: wait.run_id,
                        runner_id: wait.runner_id,
                        lease_token: wait.lease_token,
                    })
                    .await
                    .is_err()
            );
            assert!(reopened.resume_turn(input).await.is_err());
            let recovery = reopened
                .recover_expired_leases(RecoverExpiredLeasesRequest {
                    now: Utc::now() + chrono::Duration::days(1),
                    scope_filter: Some(scope.clone()),
                })
                .await
                .unwrap();
            assert_eq!(recovery.recovered.len(), 1);
            assert_eq!(recovery.recovered[0].status, TurnStatus::Failed);
            assert_eq!(
                recovery.recovered[0].failure.as_ref().unwrap().category(),
                "native_cancel_ack_expired"
            );
        } else {
            let response = reopened.resume_turn(input.clone()).await.unwrap();
            assert_eq!(response.status, TurnStatus::Running);
            let another = PgTurnStateStore::new(pg.pool.clone(), tenant);
            assert_eq!(another.resume_turn(input.clone()).await.unwrap(), response);
            let mut conflict = input;
            conflict.reply_target_binding_ref =
                ReplyTargetBindingRef::new("another-reply").unwrap();
            assert!(matches!(
                another.resume_turn(conflict).await,
                Err(brassclaw_turns::TurnError::Conflict { .. })
            ));
            let retained = another
                .get_run_record(&scope, wait.run_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(retained.runner_id, original.runner_id);
            assert_eq!(retained.lease_token, original.lease_token);
            assert_eq!(retained.claim_count, original.claim_count);
            assert!(!retained.native_auth_wait.unwrap().is_waiting());
        }
        assert!(
            reopened
                .claim_next_run(ClaimRunRequest {
                    runner_id: TurnRunnerId::new(),
                    lease_token: TurnLeaseToken::new(),
                    scope_filter: Some(scope),
                })
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_pg_original_flow_selection_survives_head_change_without_latest_fallback() {
    let rig = NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let reopened = service(&rig.pool);
    crate::credential_requirement_custody_tests::verify_original_flow_selection(
        &first,
        &reopened,
        owner(),
        usage(false),
        Some(first.secret_store.as_ref()),
    )
    .await;
}
