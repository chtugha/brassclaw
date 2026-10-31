//! Native PostgreSQL metadata CAS through independent product-auth owners.
use super::*;
use brassclaw_auth::{
    AuthContinuationRef, AuthFlowKind, AuthGateRef, AuthSessionId, AuthSurface, OpaqueStateHash,
    PkceVerifierHash, TurnRunRef,
};
use brassclaw_host_api::{InvocationId, ResourceScope, UserId};

use crate::credential_expiry_test_support as issuer_expiry;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_elapsed_issuer_expiry_projects_recovery_without_mutating_account_snapshot() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    issuer_expiry::verify(&service(&rig.pool), owner_scope()).await;
}

fn account_request(scope: AuthProductScope, name: &str) -> NewCredentialAccount {
    NewCredentialAccount {
        scope,
        provider: AuthProviderId::new("google").unwrap(),
        label: CredentialAccountLabel::new(name).unwrap(),
        status: CredentialAccountStatus::Configured,
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
        access_secret: Some(SecretHandle::new(format!("{name}-access")).unwrap()),
        access_expires_at: None,
        refresh_secret: Some(SecretHandle::new(format!("{name}-refresh")).unwrap()),
        scopes: vec![],
    }
}
fn owner_scope() -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope::local_default(UserId::new("refresh-owner").unwrap(), InvocationId::new())
            .unwrap(),
        AuthSurface::Web,
    )
}
fn service(pool: &Arc<brassclaw_pg::PgPool>) -> PgAuthProductServices {
    PgAuthProductServices::new(
        pool.clone(),
        Arc::new(
            brassclaw_secrets::PgSecretStore::new(
                pool.as_ref().clone(),
                "native-refresh-cas-credential-master-key".into(),
                "refresh-cas",
            )
            .unwrap(),
        ),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_independent_auth_owners_publish_only_one_original_snapshot_successor() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let second = service(&rig.pool);
    let owner = owner_scope();
    let original = first
        .create_account(account_request(owner.clone(), "original"))
        .await
        .unwrap();
    let a = account_request(owner.clone(), "first");
    let b = account_request(owner.clone(), "second");
    let (a, b) = tokio::join!(
        first.compare_and_update_account(&original, a),
        second.compare_and_update_account(&original, b)
    );
    let winner = match (a, b) {
        (Ok(a), Err(AuthProductError::BackendConflict)) => a,
        (Err(AuthProductError::BackendConflict), Ok(b)) => b,
        unexpected => panic!("exactly one original-snapshot successor is allowed: {unexpected:?}"),
    };
    let restarted = service(&rig.pool);
    let actual = restarted
        .get_account(CredentialAccountLookupRequest::new(owner, original.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(actual, winner);
    assert_eq!(actual.created_at, original.created_at);
    let revision: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
            &[
                &original.scope.resource.tenant_id.as_str(),
                &original.id.to_string(),
            ],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(revision, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_atomic_refresh_fence_rejects_revocation_and_wrong_update_identity() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let second = service(&rig.pool);
    let owner = owner_scope();
    let original = first
        .create_account(account_request(owner.clone(), "original"))
        .await
        .unwrap();
    let revoked = second
        .update_status(&owner, original.id, CredentialAccountStatus::Revoked)
        .await
        .unwrap();
    let error = first
        .compare_and_update_account(&original, account_request(owner.clone(), "stale"))
        .await
        .unwrap_err();
    assert!(matches!(error, AuthProductError::BackendConflict));
    let retained = first
        .get_account(CredentialAccountLookupRequest::new(
            owner.clone(),
            original.id,
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained, revoked);
    let mut wrong = account_request(owner.clone(), "wrong");
    wrong.scope.resource.user_id = UserId::new("another-user").unwrap();
    assert!(matches!(
        first.compare_and_update_account(&revoked, wrong).await,
        Err(AuthProductError::CrossScopeDenied)
    ));
    let retained = second
        .get_account(CredentialAccountLookupRequest::new(owner, original.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained, revoked);
}

fn oauth_flow(scope: AuthProductScope) -> NewAuthFlow {
    let expires_at = Utc::now() + chrono::Duration::minutes(5);
    NewAuthFlow {
        requirement_setup: None,
        id: Some(AuthFlowId::new()),
        scope,
        kind: AuthFlowKind::IntegrationCredential,
        provider: AuthProviderId::new("google").unwrap(),
        challenge: AuthChallenge::OAuthUrl {
            authorization_url: brassclaw_auth::OAuthAuthorizationUrl::new(
                "https://issuer.example/authorize",
            )
            .unwrap(),
            expires_at,
        },
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        opaque_state_hash: Some(OpaqueStateHash::new("a".repeat(64)).unwrap()),
        pkce_verifier_hash: Some(PkceVerifierHash::new("b".repeat(64)).unwrap()),
        expires_at,
    }
}
fn session_owner_scope() -> AuthProductScope {
    let mut scope = owner_scope();
    scope.resource.thread_id = Some(brassclaw_host_api::ThreadId::new("auth-thread").unwrap());
    scope.with_session_id(AuthSessionId::new("auth-session").unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_direct_auth_operations_and_lists_preserve_every_scope_dimension() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let service = service(&rig.pool);
    let scope = session_owner_scope();
    let account = service
        .create_account(account_request(scope.clone(), "scoped"))
        .await
        .unwrap();
    let flow = service
        .create_flow(oauth_flow(scope.clone()))
        .await
        .unwrap();
    let mut others = vec![];
    let mut other = scope.clone();
    other.resource.user_id = UserId::new("another-user").unwrap();
    others.push(other);
    let mut other = scope.clone();
    other.resource.invocation_id = InvocationId::new();
    others.push(other);
    let mut other = scope.clone();
    other.surface = AuthSurface::Api;
    others.push(other);
    let mut other = scope.clone();
    other.session_id = None;
    others.push(other);
    let mut other = scope.clone();
    other.resource.thread_id = None;
    others.push(other);
    let mut other = scope.clone();
    other.resource.agent_id = Some(brassclaw_host_api::AgentId::new("other-agent").unwrap());
    others.push(other);
    let mut other = scope.clone();
    other.resource.project_id = Some(brassclaw_host_api::ProjectId::new("other-project").unwrap());
    others.push(other);
    for other in others {
        assert!(matches!(
            service.get_flow(&other, flow.id).await,
            Err(AuthProductError::CrossScopeDenied)
        ));
        assert!(matches!(
            service.cancel_flow(&other, flow.id).await,
            Err(AuthProductError::CrossScopeDenied)
        ));
        assert!(matches!(
            service
                .mark_continuation_dispatched(&other, flow.id, Utc::now())
                .await,
            Err(AuthProductError::CrossScopeDenied)
        ));
        assert!(matches!(
            service
                .fail_oauth_callback(
                    &other,
                    OAuthCallbackFailureInput {
                        dispatched: None,
                        flow_id: flow.id,
                        opaque_state_hash: flow.opaque_state_hash.clone().unwrap(),
                        error: brassclaw_auth::AuthErrorCode::TokenExchangeFailed,
                    }
                )
                .await,
            Err(AuthProductError::CrossScopeDenied)
        ));
        assert!(matches!(
            service
                .get_account(CredentialAccountLookupRequest::new(
                    other.clone(),
                    account.id
                ))
                .await,
            Err(AuthProductError::CrossScopeDenied)
        ));
        assert!(
            service
                .list_accounts(CredentialAccountListRequest::new(
                    other.clone(),
                    account.provider.clone()
                ))
                .await
                .unwrap()
                .accounts
                .is_empty()
        );
        assert!(matches!(
            service
                .select_unique_configured_account(CredentialAccountSelectionRequest::new(
                    other.clone(),
                    account.provider.clone()
                ))
                .await,
            Err(AuthProductError::CredentialMissing)
        ));
        assert!(service.flows_for_scope(&other).await.unwrap().is_empty());
    }
    let mut other_tenant = scope.clone();
    other_tenant.resource.tenant_id = brassclaw_host_api::TenantId::new("another-tenant").unwrap();
    assert!(
        service
            .get_flow(&other_tenant, flow.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        service
            .get_account(CredentialAccountLookupRequest::new(
                other_tenant,
                account.id
            ))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(service.get_flow(&scope, flow.id).await.unwrap(), Some(flow));
    assert_eq!(
        service
            .get_account(CredentialAccountLookupRequest::new(scope, account.id))
            .await
            .unwrap(),
        Some(account)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_flow_insert_conflicts_without_overwriting_another_owner() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let second = service(&rig.pool);
    let scope = session_owner_scope();
    let original = oauth_flow(scope.clone());
    let created = first.create_flow(original.clone()).await.unwrap();
    for request in [original.clone(), {
        let mut changed = original;
        changed.scope.resource.user_id = UserId::new("different-owner").unwrap();
        changed
    }] {
        assert!(matches!(
            second.create_flow(request).await,
            Err(AuthProductError::BackendConflict)
        ));
    }
    assert_eq!(
        first.get_flow(&scope, created.id).await.unwrap(),
        Some(created)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_stable_owner_flow_queries_keep_original_invocations_and_bound_sessions() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let service = service(&rig.pool);
    let scope = session_owner_scope();
    let account = service
        .create_account(account_request(scope.clone(), "bound"))
        .await
        .unwrap();
    let mut first = oauth_flow(scope.clone());
    first.update_binding = Some(CredentialAccountUpdateBinding::from_projection(
        &account.projection(),
    ));
    first.continuation = AuthContinuationRef::TurnGateResume {
        turn_run_ref: TurnRunRef::new("original-run").unwrap(),
        gate_ref: AuthGateRef::new("original-gate").unwrap(),
    };
    let first = service.create_flow(first).await.unwrap();
    let mut second_scope = scope.clone();
    second_scope.resource.invocation_id = InvocationId::new();
    second_scope.surface = AuthSurface::Api;
    second_scope.session_id = None;
    let second = service.create_flow(oauth_flow(second_scope)).await.unwrap();
    let owner = AuthFlowOwnerScope {
        tenant_id: scope.resource.tenant_id.clone(),
        user_id: scope.resource.user_id.clone(),
        agent_id: scope.resource.agent_id.clone(),
        project_id: scope.resource.project_id.clone(),
        thread_id: scope.resource.thread_id.clone().unwrap(),
    };
    let found = service.flows_for_owner(owner.clone()).await.unwrap();
    assert_eq!(found.len(), 2);
    assert!(found.contains(&first));
    assert!(found.contains(&second));
    assert_eq!(
        service.flows_for_scope(&scope).await.unwrap(),
        vec![first.clone()]
    );
    assert_eq!(
        service
            .flow_for_turn_gate(TurnGateAuthFlowQuery {
                owner,
                turn_run_ref: TurnRunRef::new("original-run").unwrap(),
                gate_ref: AuthGateRef::new("original-gate").unwrap(),
                include_terminal: false,
            })
            .await
            .unwrap(),
        Some(first)
    );
    let challenge = service
        .request_secret_input(ManualTokenSetupRequest {
            requirement_setup: None,
            scope,
            provider: account.provider.clone(),
            label: account.label.clone(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: Some(CredentialAccountUpdateBinding::from_projection(
                &account.projection(),
            )),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
        })
        .await
        .unwrap();
    assert!(matches!(
        challenge,
        AuthChallenge::ManualTokenRequired { .. }
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_cancellation_selects_only_its_interaction_and_fences_abandon() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let service = service(&rig.pool);
    let scope = session_owner_scope();
    let unrelated = service
        .create_flow(oauth_flow(scope.clone()))
        .await
        .unwrap();
    let mut manuals = vec![];
    for name in ["first", "second"] {
        let challenge = service
            .request_manual_token_flow(ManualTokenSetupRequest {
                requirement_setup: None,
                scope: scope.clone(),
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new(name).unwrap(),
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: None,
                expires_at: Utc::now() + chrono::Duration::minutes(5),
            })
            .await
            .unwrap();
        let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
            panic!("manual challenge");
        };
        manuals.push(interaction_id);
    }
    let canceled = service
        .cancel_manual_token(&scope, manuals[1])
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(canceled.challenge,Some(AuthChallenge::ManualTokenRequired {interaction_id,..}) if interaction_id==manuals[1])
    );
    assert_eq!(canceled.status, AuthFlowStatus::Canceled);
    assert_eq!(
        service.get_flow(&scope, unrelated.id).await.unwrap(),
        Some(unrelated)
    );
    let first=service.flows_for_scope(&scope).await.unwrap().into_iter().find(|f|matches!(&f.challenge,Some(AuthChallenge::ManualTokenRequired {interaction_id,..}) if *interaction_id==manuals[0])).unwrap();
    assert_eq!(first.status, AuthFlowStatus::AwaitingUser);
    assert_eq!(
        service
            .cancel_manual_token(&scope, manuals[1])
            .await
            .unwrap()
            .unwrap()
            .id,
        canceled.id
    );
    let mut wrong = scope.clone();
    wrong.session_id = None;
    assert!(matches!(
        service.abandon_manual_token(&wrong, manuals[1]).await,
        Err(AuthProductError::CrossScopeDenied)
    ));
    assert!(
        service
            .read_interaction(&scope, manuals[1])
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        service
            .abandon_manual_token(&scope, manuals[1])
            .await
            .unwrap()
    );
    assert!(
        !service
            .abandon_manual_token(&scope, manuals[1])
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_callback_failure_and_continuation_ack_keep_validated_original_state() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let service = service(&rig.pool);
    let scope = session_owner_scope();
    let flow = service
        .create_flow(oauth_flow(scope.clone()))
        .await
        .unwrap();
    assert!(matches!(
        service
            .mark_continuation_dispatched(&scope, flow.id, Utc::now())
            .await,
        Err(AuthProductError::FlowAlreadyTerminal)
    ));
    assert!(matches!(
        service
            .fail_oauth_callback(
                &scope,
                OAuthCallbackFailureInput {
                    dispatched: None,
                    flow_id: flow.id,
                    opaque_state_hash: OpaqueStateHash::new("c".repeat(64)).unwrap(),
                    error: brassclaw_auth::AuthErrorCode::TokenExchangeFailed,
                }
            )
            .await,
        Err(AuthProductError::CrossScopeDenied)
    ));
    assert_eq!(
        service.get_flow(&scope, flow.id).await.unwrap(),
        Some(flow.clone())
    );
    let failed = service
        .fail_oauth_callback(
            &scope,
            OAuthCallbackFailureInput {
                dispatched: None,
                flow_id: flow.id,
                opaque_state_hash: flow.opaque_state_hash.clone().unwrap(),
                error: brassclaw_auth::AuthErrorCode::TokenExchangeFailed,
            },
        )
        .await
        .unwrap();
    assert_eq!(failed.status, AuthFlowStatus::Failed);
    assert!(matches!(
        service.cancel_flow(&scope, flow.id).await,
        Err(AuthProductError::FlowAlreadyTerminal)
    ));
    // This fixture sets no credential/continuation effect; exercise acknowledgement custody only.
    let (mut completed, revision) = service.read_flow(&scope, flow.id).await.unwrap().unwrap();
    completed.status = AuthFlowStatus::Completed;
    completed.error = None;
    service
        .write_flow(&completed, Some(revision))
        .await
        .unwrap();
    let first_at = Utc::now();
    let first = service
        .mark_continuation_dispatched(&scope, flow.id, first_at)
        .await
        .unwrap();
    let repeated = service
        .mark_continuation_dispatched(&scope, flow.id, first_at + chrono::Duration::seconds(1))
        .await
        .unwrap();
    assert_eq!(first, repeated);
    assert_eq!(repeated.continuation_emitted_at, Some(first_at));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_credential_selection_requires_advertised_and_current_eligible_account() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let service = service(&rig.pool);
    let scope = session_owner_scope();
    let first = service
        .create_account(account_request(scope.clone(), "first"))
        .await
        .unwrap();
    let second = service
        .create_account(account_request(scope.clone(), "second"))
        .await
        .unwrap();
    let oauth = service
        .create_flow(oauth_flow(scope.clone()))
        .await
        .unwrap();
    assert!(matches!(
        service
            .complete_credential_selection(
                &scope,
                brassclaw_auth::CredentialSelectionInput {
                    flow_id: oauth.id,
                    credential_account_id: first.id,
                }
            )
            .await,
        Err(AuthProductError::InvalidRequest { .. })
    ));
    let mut selection = oauth_flow(scope.clone());
    selection.challenge = AuthChallenge::AccountSelectionRequired {
        provider: first.provider.clone(),
        accounts: vec![first.projection()],
    };
    let selection = service.create_flow(selection).await.unwrap();
    assert!(matches!(
        service
            .complete_credential_selection(
                &scope,
                brassclaw_auth::CredentialSelectionInput {
                    flow_id: selection.id,
                    credential_account_id: second.id,
                }
            )
            .await,
        Err(AuthProductError::CredentialMissing)
    ));
    service
        .update_status(&scope, first.id, CredentialAccountStatus::Inactive)
        .await
        .unwrap();
    assert!(matches!(
        service
            .complete_credential_selection(
                &scope,
                brassclaw_auth::CredentialSelectionInput {
                    flow_id: selection.id,
                    credential_account_id: first.id,
                }
            )
            .await,
        Err(AuthProductError::CrossScopeDenied)
    ));
    assert_eq!(
        service.get_flow(&scope, selection.id).await.unwrap(),
        Some(selection.clone())
    );
    service
        .update_status(&scope, first.id, CredentialAccountStatus::Configured)
        .await
        .unwrap();
    let completed = service
        .complete_credential_selection(
            &scope,
            brassclaw_auth::CredentialSelectionInput {
                flow_id: selection.id,
                credential_account_id: first.id,
            },
        )
        .await
        .unwrap();
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    assert_eq!(completed.credential_account_id, Some(first.id));
    assert_eq!(
        service
            .complete_credential_selection(
                &scope,
                brassclaw_auth::CredentialSelectionInput {
                    flow_id: selection.id,
                    credential_account_id: first.id,
                }
            )
            .await
            .unwrap(),
        completed
    );
    assert!(matches!(
        service
            .complete_credential_selection(
                &scope,
                brassclaw_auth::CredentialSelectionInput {
                    flow_id: selection.id,
                    credential_account_id: second.id,
                }
            )
            .await,
        Err(AuthProductError::FlowAlreadyTerminal)
    ));
    let mut expired = oauth_flow(scope.clone());
    expired.challenge = AuthChallenge::AccountSelectionRequired {
        provider: first.provider.clone(),
        accounts: vec![first.projection()],
    };
    expired.expires_at = Utc::now() - chrono::Duration::seconds(1);
    let expired = service.create_flow(expired).await.unwrap();
    assert!(matches!(
        service
            .complete_credential_selection(
                &scope,
                brassclaw_auth::CredentialSelectionInput {
                    flow_id: expired.id,
                    credential_account_id: first.id,
                }
            )
            .await,
        Err(AuthProductError::UnknownOrExpiredFlow)
    ));
    assert_eq!(
        service
            .get_flow(&scope, expired.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        AuthFlowStatus::Expired
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_abandon_surfaces_each_failed_transition_and_preserves_retry_state() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let service = service(&rig.pool);
    let scope = session_owner_scope();
    let challenge = service
        .request_manual_token_flow(ManualTokenSetupRequest {
            requirement_setup: None,
            scope: scope.clone(),
            provider: AuthProviderId::new("google").unwrap(),
            label: CredentialAccountLabel::new("abandon").unwrap(),
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            expires_at: Utc::now() + chrono::Duration::minutes(5),
        })
        .await
        .unwrap();
    let AuthChallenge::ManualTokenRequired { interaction_id, .. } = challenge else {
        panic!("manual challenge");
    };
    let original_interaction = service
        .read_interaction(&scope, interaction_id)
        .await
        .unwrap()
        .unwrap();
    rig.pool.get().await.unwrap().batch_execute("CREATE FUNCTION reject_auth_cancel() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled cancel failure'; END $$; CREATE TRIGGER reject_auth_cancel BEFORE UPDATE ON brassclaw_product_auth_flows FOR EACH ROW EXECUTE FUNCTION reject_auth_cancel();").await.unwrap();
    assert!(matches!(
        service
            .abandon_manual_token_flow(&scope, interaction_id)
            .await,
        Err(AuthProductError::BackendUnavailable)
    ));
    assert_eq!(
        service
            .read_interaction(&scope, interaction_id)
            .await
            .unwrap()
            .unwrap(),
        original_interaction,
        "a rejected transition must preserve the complete original and revision"
    );
    assert_eq!(
        service.flows_for_scope(&scope).await.unwrap()[0].status,
        AuthFlowStatus::AwaitingUser
    );
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_auth_cancel ON brassclaw_product_auth_flows; DROP FUNCTION reject_auth_cancel(); CREATE FUNCTION reject_auth_abandon() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled abandon failure'; END $$; CREATE TRIGGER reject_auth_abandon BEFORE UPDATE ON brassclaw_product_auth_interactions FOR EACH ROW EXECUTE FUNCTION reject_auth_abandon();").await.unwrap();
    assert!(matches!(
        service
            .abandon_manual_token_flow(&scope, interaction_id)
            .await,
        Err(AuthProductError::BackendUnavailable)
    ));
    assert_eq!(
        service
            .read_interaction(&scope, interaction_id)
            .await
            .unwrap()
            .unwrap(),
        original_interaction,
        "a rejected transition must preserve the complete original and revision"
    );
    assert_eq!(
        service.flows_for_scope(&scope).await.unwrap()[0].status,
        AuthFlowStatus::Canceled
    );
    rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_auth_abandon ON brassclaw_product_auth_interactions; DROP FUNCTION reject_auth_abandon();").await.unwrap();
    assert!(
        service
            .abandon_manual_token_flow(&scope, interaction_id)
            .await
            .unwrap()
    );
    let canceled_interaction = service
        .read_interaction(&scope, interaction_id)
        .await
        .unwrap()
        .unwrap();
    assert!(canceled_interaction.0.canceled_at.is_some());
    let mut expected_interaction = original_interaction;
    expected_interaction.0.canceled_at = canceled_interaction.0.canceled_at;
    expected_interaction.1 += 1;
    assert_eq!(canceled_interaction, expected_interaction);

    assert!(
        service
            .abandon_manual_token_flow(&scope, interaction_id)
            .await
            .unwrap()
    );
    assert_eq!(
        service
            .read_interaction(&scope, interaction_id)
            .await
            .unwrap()
            .unwrap(),
        canceled_interaction,
        "repeated abandonment must keep the same original cancellation and revision"
    );
}

mod durable_refresh_custody {
    use super::*;
    use brassclaw_auth::{
        CredentialRefreshAction, CredentialRefreshAdmission, CredentialRefreshCompletion,
        CredentialRefreshOriginal, CredentialRefreshOutcome, CredentialRefreshOutcomeKind,
        CredentialRefreshPrepared, CredentialRefreshPublication, OAuthProviderRefresh,
        OAuthTokenDestinations,
    };

    async fn prepared(
        pool: &Arc<brassclaw_pg::PgPool>,
        account: &CredentialAccount,
        request: &CredentialRefreshRequest,
    ) -> CredentialRefreshPrepared {
        let store = service(pool).secret_store;
        let handle = account.refresh_secret.as_ref().unwrap();
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
                        "native refresh fixture material".into(),
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
        let source = brassclaw_auth::OAuthRefreshSource {
            scope: account.scope.resource.clone(),
            handle: handle.clone(),
            record_reference: store
                .record_reference(&account.scope.resource, handle)
                .await
                .unwrap()
                .unwrap(),
        };
        CredentialRefreshPrepared {
            original: CredentialRefreshOriginal::from(request),
            expected_account: account.clone(),
            preparation: Some(brassclaw_auth::OAuthProviderRefreshPreparation {
                destinations: OAuthTokenDestinations {
                    access_secret: SecretHandle::new(format!("refresh-access-{}", request.id))
                        .unwrap(),
                    refresh_secret: SecretHandle::new(format!("refresh-rotation-{}", request.id))
                        .unwrap(),
                },
                source,
            }),
        }
    }
    fn dispatched(admission: CredentialRefreshAdmission) -> CredentialRefreshAction {
        match admission {
            CredentialRefreshAdmission::Dispatch(action) => action,
            other => panic!("first owner must receive the single dispatch: {other:?}"),
        }
    }
    fn returned(action: &CredentialRefreshAction, rotate: bool) -> CredentialRefreshOutcome {
        let destinations = action.destinations.as_ref().unwrap();
        CredentialRefreshOutcome::Returned(OAuthProviderRefresh {
            provider: action.original.provider.clone(),
            access_secret: destinations.access_secret.clone(),
            access_expires_at: None,
            refresh_secret: rotate.then(|| destinations.refresh_secret.clone()),
            scopes: action.expected_account.scopes.clone(),
        })
    }
    async fn account_revision(
        pool: &Arc<brassclaw_pg::PgPool>,
        account: &CredentialAccount,
    ) -> i64 {
        pool.get()
            .await
            .unwrap()
            .query_one(
                "SELECT revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
                &[
                    &account.scope.resource.tenant_id.as_str(),
                    &account.id.to_string(),
                ],
            )
            .await
            .unwrap()
            .get(0)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_original_refresh_claim_survives_independent_owners_and_restart() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let first = service(&rig.pool);
        let second = service(&rig.pool);
        let owner = owner_scope();
        let account = first
            .create_account(account_request(owner.clone(), "claim-original"))
            .await
            .unwrap();
        let request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        let captured = prepared(&rig.pool, &account, &request).await;
        let (a, b) = tokio::join!(
            first.begin_refresh(captured.clone()),
            second.begin_refresh(captured)
        );
        let (action, original) = match (a.unwrap(), b.unwrap()) {
            (CredentialRefreshAdmission::Dispatch(a), CredentialRefreshAdmission::Original(b)) => {
                (a, b)
            }
            (CredentialRefreshAdmission::Original(a), CredentialRefreshAdmission::Dispatch(b)) => {
                (b, a)
            }
            other => panic!("one dispatch and one read-only original: {other:?}"),
        };
        assert_eq!(action, original);
        assert_eq!(account_revision(&rig.pool, &account).await, 0);
        let restarted = service(&rig.pool);
        assert_eq!(
            restarted.refresh_action(&request).await.unwrap(),
            Some(action.clone())
        );
        let replacement =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        assert!(matches!(
            restarted
                .begin_refresh(prepared(&rig.pool, &account, &replacement).await)
                .await,
            Err(AuthProductError::BackendConflict)
        ));
        let other = first
            .create_account(account_request(owner.clone(), "another-account"))
            .await
            .unwrap();
        let conflict =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), other.id)
                .with_id(request.id);
        assert!(matches!(
            restarted.refresh_action(&conflict).await,
            Err(AuthProductError::BackendConflict)
        ));
        assert!(matches!(
            restarted
                .begin_refresh(prepared(&rig.pool, &other, &conflict).await)
                .await,
            Err(AuthProductError::BackendConflict)
        ));
        let mut changed_requester = request.clone();
        changed_requester.requester_extension =
            Some(brassclaw_host_api::ExtensionId::new("different-requester").unwrap());
        assert!(matches!(
            restarted.refresh_action(&changed_requester).await,
            Err(AuthProductError::BackendConflict)
        ));
        // A returned access credential without rotation preserves the old refresh handle.
        let completion = CredentialRefreshCompletion {
            outcome: returned(&action, false),
            dispatched: action,
        };
        let published = restarted
            .complete_refresh(completion.clone())
            .await
            .unwrap();
        let stored = first
            .get_account(CredentialAccountLookupRequest::new(
                owner.clone(),
                account.id,
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.refresh_secret, account.refresh_secret);
        assert_ne!(stored.access_secret, account.access_secret);
        assert_eq!(published.published_account, Some(stored.clone()));
        assert!(published.receipt().unwrap().confirmed_refresh());
        assert_eq!(
            published.receipt().unwrap().published_account_ref,
            Some(stored.snapshot_ref().unwrap())
        );
        assert_eq!(account_revision(&rig.pool, &account).await, 1);
        // Discarding/losing the first success acknowledgement never creates another mutation.
        let again = service(&rig.pool)
            .complete_refresh(completion)
            .await
            .unwrap();
        assert_eq!(again, published);
        assert_eq!(
            service(&rig.pool).refresh_action(&request).await.unwrap(),
            Some(published)
        );
        assert_eq!(account_revision(&rig.pool, &account).await, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_returned_refresh_survives_failed_account_publication_and_reconciles_original() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let first = service(&rig.pool);
        let owner = owner_scope();
        let account = first
            .create_account(account_request(owner.clone(), "reject-result"))
            .await
            .unwrap();
        let request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        let action = dispatched(
            first
                .begin_refresh(prepared(&rig.pool, &account, &request).await)
                .await
                .unwrap(),
        );
        let completion = CredentialRefreshCompletion {
            outcome: returned(&action, true),
            dispatched: action.clone(),
        };
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION reject_refresh_result() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled refresh result rejection'; END $$; CREATE TRIGGER reject_refresh_result BEFORE UPDATE ON brassclaw_product_auth_accounts FOR EACH ROW EXECUTE FUNCTION reject_refresh_result();").await.unwrap();
        assert!(matches!(
            first.complete_refresh(completion.clone()).await,
            Err(AuthProductError::BackendUnavailable)
        ));
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
        assert_eq!(account_revision(&rig.pool, &account).await, 0);
        let retained = service(&rig.pool)
            .refresh_action(&request)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            retained.outcome,
            CredentialRefreshOutcome::Returned(_)
        ));
        assert_eq!(retained.publication, CredentialRefreshPublication::Pending);
        assert!(retained.published_account.is_none());
        // The original durable result blocks a replacement issuer action after failed publication.
        let replacement =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        assert!(matches!(
            first
                .begin_refresh(prepared(&rig.pool, &account, &replacement).await)
                .await,
            Err(AuthProductError::BackendConflict)
        ));
        client.batch_execute("DROP TRIGGER reject_refresh_result ON brassclaw_product_auth_accounts; DROP FUNCTION reject_refresh_result();").await.unwrap();
        drop(client);
        // A caller may couple publication to its own existing transaction.
        // Rolling it back must preserve the independently retained response
        // and both original account/action revisions for later reconciliation.
        let mut client = rig.pool.get().await.unwrap();
        let transaction = client.transaction().await.unwrap();
        let (staged, changed) = first
            .reconcile_refresh_action_in_transaction(&transaction, &retained.original)
            .await
            .unwrap();
        assert!(changed);
        assert!(staged.unwrap().receipt().unwrap().confirmed_refresh());
        transaction.rollback().await.unwrap();
        drop(client);
        assert_eq!(account_revision(&rig.pool, &account).await, 0);
        assert_eq!(
            service(&rig.pool).refresh_action(&request).await.unwrap(),
            Some(retained)
        );
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
        // Recreate the owner and read/reconcile only the original ID; the
        // response passed to the first call is no longer needed or supplied.
        drop(completion);
        let reconciled = service(&rig.pool)
            .reconcile_refresh(&request)
            .await
            .unwrap()
            .unwrap();
        assert!(reconciled.receipt().unwrap().confirmed_refresh());
        assert_eq!(account_revision(&rig.pool, &account).await, 1);
        assert_eq!(
            reconciled.publication,
            CredentialRefreshPublication::Applied
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_unknown_and_stale_refresh_preserve_custody_and_current_account() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let service = service(&rig.pool);
        let owner = owner_scope();
        let account = service
            .create_account(account_request(owner.clone(), "unknown-issuer"))
            .await
            .unwrap();
        let request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        let action = dispatched(
            service
                .begin_refresh(prepared(&rig.pool, &account, &request).await)
                .await
                .unwrap(),
        );
        let unknown = service
            .complete_refresh(CredentialRefreshCompletion {
                dispatched: action,
                outcome: CredentialRefreshOutcome::Unknown(
                    brassclaw_auth::AuthErrorCode::TokenExchangeFailed,
                ),
            })
            .await
            .unwrap();
        assert_eq!(
            unknown.receipt().unwrap().outcome,
            CredentialRefreshOutcomeKind::Unconfirmed
        );
        assert!(!unknown.receipt().unwrap().confirmed_refresh());
        let current = service
            .get_account(CredentialAccountLookupRequest::new(
                owner.clone(),
                account.id,
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.status, CredentialAccountStatus::RefreshFailed);
        let new_request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        assert!(matches!(
            service
                .begin_refresh(prepared(&rig.pool, &current, &new_request).await)
                .await,
            Err(AuthProductError::BackendConflict)
        ));
        // A separately authorized, actually different refresh credential is a different grant.
        let reauthorized = service
            .create_or_update_account(CredentialAccountMutation::Update(
                brassclaw_auth::CredentialAccountUpdate {
                    account_id: account.id,
                    account: account_request(owner.clone(), "new-grant"),
                },
            ))
            .await
            .unwrap();
        let next_request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        let next = dispatched(
            service
                .begin_refresh(prepared(&rig.pool, &reauthorized, &next_request).await)
                .await
                .unwrap(),
        );
        let revoked = service
            .update_status(&owner, account.id, CredentialAccountStatus::Revoked)
            .await
            .unwrap();
        let stale = service
            .complete_refresh(CredentialRefreshCompletion {
                outcome: returned(&next, true),
                dispatched: next,
            })
            .await
            .unwrap();
        assert_eq!(stale.publication, CredentialRefreshPublication::Superseded);
        assert!(stale.published_account.is_none());
        assert!(!stale.receipt().unwrap().confirmed_refresh());
        assert_eq!(
            service
                .get_account(CredentialAccountLookupRequest::new(owner, account.id))
                .await
                .unwrap(),
            Some(revoked)
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_foreign_refresh_result_is_retained_without_publishing_or_replaying() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let owner = owner_scope();
        let backend = service(&rig.pool);
        let account = backend
            .create_account(account_request(owner.clone(), "foreign-output"))
            .await
            .unwrap();
        let request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        let action = dispatched(
            backend
                .begin_refresh(prepared(&rig.pool, &account, &request).await)
                .await
                .unwrap(),
        );
        let foreign = CredentialRefreshOutcome::Returned(OAuthProviderRefresh {
            provider: account.provider.clone(),
            access_secret: SecretHandle::new("foreign-unowned-access").unwrap(),
            access_expires_at: None,
            refresh_secret: Some(SecretHandle::new("foreign-unowned-refresh").unwrap()),
            scopes: vec![],
        });
        let completion = CredentialRefreshCompletion {
            dispatched: action.clone(),
            outcome: foreign,
        };
        let retained = backend.complete_refresh(completion.clone()).await.unwrap();
        assert!(matches!(
            retained.outcome,
            CredentialRefreshOutcome::UnusableReturn(_)
        ));
        assert_eq!(
            retained.receipt().unwrap().outcome,
            CredentialRefreshOutcomeKind::Unconfirmed
        );
        assert!(!retained.receipt().unwrap().confirmed_refresh());
        let current = backend
            .get_account(CredentialAccountLookupRequest::new(
                owner.clone(),
                account.id,
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.status, CredentialAccountStatus::RefreshFailed);
        assert_eq!(current.access_secret, account.access_secret);
        assert_eq!(current.refresh_secret, account.refresh_secret);
        let revision = account_revision(&rig.pool, &account).await;
        assert_eq!(
            service(&rig.pool)
                .complete_refresh(completion)
                .await
                .unwrap(),
            retained
        );
        assert_eq!(account_revision(&rig.pool, &account).await, revision);
        // A contradictory later answer is not a repair or a second publication.
        assert!(matches!(
            backend
                .complete_refresh(CredentialRefreshCompletion {
                    outcome: returned(&action, true),
                    dispatched: action,
                })
                .await,
            Err(AuthProductError::BackendConflict)
        ));
        let replacement =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        assert!(matches!(
            backend
                .begin_refresh(prepared(&rig.pool, &current, &replacement).await)
                .await,
            Err(AuthProductError::BackendConflict)
        ));
        let receipt = serde_json::to_string(&retained.receipt().unwrap()).unwrap();
        assert!(!receipt.contains("foreign-unowned"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_read_only_refresh_originals_ignore_held_account_and_action_rows() {
        use brassclaw_auth::ProviderBackedCredentialAccountService;
        use std::time::Duration;
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let backend = Arc::new(service(&rig.pool));
        let owner = owner_scope();
        let account = backend
            .create_account(account_request(owner.clone(), "readback"))
            .await
            .unwrap();
        let request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        let original = dispatched(
            backend
                .begin_refresh(prepared(&rig.pool, &account, &request).await)
                .await
                .unwrap(),
        );
        // A removed/unavailable configured client must not be consulted for
        // original readback. The actual coordinator still uses the PG owners.
        let coordinator = ProviderBackedCredentialAccountService::new(
            backend.clone(),
            backend.clone(),
            Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
        );
        let mut expected = original.clone();
        for confirmed in [false, true] {
            let mut locker = rig.pool.get().await.unwrap();
            let transaction = locker.transaction().await.unwrap();
            transaction.query_one(
                "SELECT revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
                &[&owner.resource.tenant_id.as_str(), &account.id.to_string()],
            ).await.unwrap();
            let refresh_revision: i64 = transaction.query_one(
                "SELECT revision FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
                &[&owner.resource.tenant_id.as_str(), &request.id.to_string()],
            ).await.unwrap().get(0);
            tokio::time::timeout(Duration::from_secs(3), async {
                let missing = CredentialRefreshRequest::new(
                    owner.clone(),
                    account.provider.clone(),
                    account.id,
                );
                assert!(backend.reconcile_refresh(&missing).await.unwrap().is_none());
                assert_eq!(
                    backend.reconcile_refresh(&request).await.unwrap(),
                    Some(expected.clone())
                );
                match backend
                    .begin_refresh(prepared(&rig.pool, &account, &request).await)
                    .await
                    .unwrap()
                {
                    CredentialRefreshAdmission::Original(action) => assert_eq!(action, expected),
                    other => panic!("readback cannot authorize dispatch: {other:?}"),
                }
                let report = coordinator.refresh_account(request.clone()).await.unwrap();
                assert_eq!(report.original.as_ref(), Some(&expected.receipt().unwrap()));
                assert_eq!(report.refreshed, confirmed);
                let mut changed = request.clone();
                changed.provider = AuthProviderId::new("different-provider").unwrap();
                assert!(matches!(
                    backend.reconcile_refresh(&changed).await,
                    Err(AuthProductError::BackendConflict)
                ));
                assert!(matches!(
                    coordinator.refresh_account(changed).await,
                    Err(AuthProductError::BackendConflict)
                ));
            })
            .await
            .expect("read-only originals must not wait for either publication row lock");
            let after: i64 = transaction.query_one(
                "SELECT revision FROM brassclaw_product_auth_refreshes WHERE tenant_id=$1 AND id=$2",
                &[&owner.resource.tenant_id.as_str(), &request.id.to_string()],
            ).await.unwrap().get(0);
            assert_eq!(
                after, refresh_revision,
                "readback must not write the original"
            );
            transaction.rollback().await.unwrap();
            if !confirmed {
                expected = backend
                    .complete_refresh(CredentialRefreshCompletion {
                        dispatched: original.clone(),
                        outcome: returned(&original, true),
                    })
                    .await
                    .unwrap();
                assert!(expected.receipt().unwrap().confirmed_refresh());
            }
        }
        assert_eq!(account_revision(&rig.pool, &account).await, 1);
        assert_eq!(
            backend.refresh_action(&request).await.unwrap(),
            Some(expected)
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_known_pending_refresh_keeps_atomic_lock_and_concurrent_finalizer_fence() {
        use std::time::Duration;
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let backend = Arc::new(service(&rig.pool));
        let owner = owner_scope();
        let account = backend
            .create_account(account_request(owner.clone(), "pending-readback"))
            .await
            .unwrap();
        let request =
            CredentialRefreshRequest::new(owner.clone(), account.provider.clone(), account.id);
        let original = dispatched(
            backend
                .begin_refresh(prepared(&rig.pool, &account, &request).await)
                .await
                .unwrap(),
        );
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION reject_readback_publish() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled publication rejection'; END $$; CREATE TRIGGER reject_readback_publish BEFORE UPDATE ON brassclaw_product_auth_accounts FOR EACH ROW EXECUTE FUNCTION reject_readback_publish();").await.unwrap();
        assert!(matches!(
            backend
                .complete_refresh(CredentialRefreshCompletion {
                    dispatched: original.clone(),
                    outcome: returned(&original, true),
                })
                .await,
            Err(AuthProductError::BackendUnavailable)
        ));
        client.batch_execute("DROP TRIGGER reject_readback_publish ON brassclaw_product_auth_accounts; DROP FUNCTION reject_readback_publish();").await.unwrap();
        let retained = backend.refresh_action(&request).await.unwrap().unwrap();
        assert!(retained.needs_publication());
        assert!(matches!(
            retained.outcome,
            CredentialRefreshOutcome::Returned(_)
        ));
        let mut locker = rig.pool.get().await.unwrap();
        let transaction = locker.transaction().await.unwrap();
        let locker_pid: i32 = transaction
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .unwrap()
            .get(0);
        transaction.query_one(
            "SELECT revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
            &[&owner.resource.tenant_id.as_str(), &account.id.to_string()],
        ).await.unwrap();
        let a = Arc::new(service(&rig.pool));
        let b = Arc::new(service(&rig.pool));
        let request_a = request.clone();
        let request_b = request.clone();
        let first = tokio::spawn(async move { a.reconcile_refresh(&request_a).await });
        let second = tokio::spawn(async move { b.reconcile_refresh(&request_b).await });
        // Observe an actual PostgreSQL lock dependency, not a scheduling delay
        // mistaken for evidence that the production publication is serialized.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let blocked: i64 = client.query_one(
                    "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid))",
                    &[&locker_pid],
                ).await.unwrap().get(0);
                if blocked > 0 { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("known pending publication must use the real account row lock");
        assert!(!first.is_finished());
        assert!(!second.is_finished());
        assert_eq!(
            backend.refresh_action(&request).await.unwrap(),
            Some(retained)
        );
        transaction.rollback().await.unwrap();
        let (a, b) = tokio::time::timeout(Duration::from_secs(5), async {
            (
                first.await.unwrap().unwrap().unwrap(),
                second.await.unwrap().unwrap().unwrap(),
            )
        })
        .await
        .expect("both finalizers settle after the row is released");
        assert_eq!(a, b);
        assert!(a.receipt().unwrap().confirmed_refresh());
        assert!(!a.needs_publication());
        assert_eq!(account_revision(&rig.pool, &account).await, 1);
        assert_eq!(backend.refresh_action(&request).await.unwrap(), Some(a));
    }
}

mod durable_callback_custody {
    use super::*;
    use brassclaw_auth::{
        AuthErrorCode, OAuthCallbackAction, OAuthCallbackAdmission, OAuthCallbackOriginal,
        OAuthCallbackOutcome, OAuthCallbackPrepared, OAuthCallbackPublication,
        OAuthProviderExchange, OAuthTokenDestinations, ProviderScope,
    };

    fn prepared(flow: &AuthFlowRecord) -> OAuthCallbackPrepared {
        OAuthCallbackPrepared {
            original: OAuthCallbackOriginal {
                scope: flow.scope.clone(),
                flow_id: flow.id,
                provider: flow.provider.clone(),
                opaque_state_hash: flow.opaque_state_hash.clone().unwrap(),
                authorization_code_hash: brassclaw_auth::AuthorizationCodeHash::new("c".repeat(64))
                    .unwrap(),
                pkce_verifier_hash: flow.pkce_verifier_hash.clone().unwrap(),
                account_label: CredentialAccountLabel::new("original callback").unwrap(),
                requested_scopes: vec![ProviderScope::new("gmail.readonly").unwrap()],
            },
            destinations: OAuthTokenDestinations {
                access_secret: SecretHandle::new(format!("callback-access-{}", flow.id)).unwrap(),
                refresh_secret: SecretHandle::new(format!("callback-refresh-{}", flow.id)).unwrap(),
            },
        }
    }
    fn dispatched(admission: OAuthCallbackAdmission) -> OAuthCallbackAction {
        match admission {
            OAuthCallbackAdmission::Dispatch(state) => state.action,
            other => panic!("new original must own one dispatch: {other:?}"),
        }
    }
    fn exchange(action: &OAuthCallbackAction) -> OAuthProviderExchange {
        OAuthProviderExchange {
            provider: action.original.provider.clone(),
            account_label: action.original.account_label.clone(),
            authorization_code_hash: action.original.authorization_code_hash.clone(),
            pkce_verifier_hash: action.original.pkce_verifier_hash.clone(),
            access_secret: action.destinations.access_secret.clone(),
            access_expires_at: None,
            refresh_secret: Some(action.destinations.refresh_secret.clone()),
            scopes: action.original.requested_scopes.clone(),
            account_id: None,
        }
    }
    fn completion(
        action: &OAuthCallbackAction,
        exchange: OAuthProviderExchange,
    ) -> OAuthCallbackInput {
        OAuthCallbackInput {
            dispatched: Some(action.clone()),
            flow_id: action.original.flow_id,
            opaque_state_hash: action.original.opaque_state_hash.clone(),
            outcome: ProviderCallbackOutcome::Authorized { exchange },
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_callback_original_retains_result_before_failed_account_publish_and_restart() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let first = service(&rig.pool);
        let second = service(&rig.pool);
        let owner = owner_scope();
        let flow = first.create_flow(oauth_flow(owner.clone())).await.unwrap();
        let prepared = prepared(&flow);
        let (a, b) = tokio::join!(
            first.claim_oauth_callback(&owner, prepared.clone()),
            second.claim_oauth_callback(&owner, prepared.clone())
        );
        let original = match (a.unwrap(), b.unwrap()) {
            (OAuthCallbackAdmission::Dispatch(a), OAuthCallbackAdmission::Original(b)) => {
                assert_eq!(a.action, b.action);
                a.action
            }
            (OAuthCallbackAdmission::Original(a), OAuthCallbackAdmission::Dispatch(b)) => {
                assert_eq!(a.action, b.action);
                b.action
            }
            other => panic!("independent owners cannot both dispatch: {other:?}"),
        };
        let mut changed = prepared;
        changed
            .original
            .requested_scopes
            .push(ProviderScope::new("calendar.readonly").unwrap());
        assert!(matches!(
            second.claim_oauth_callback(&owner, changed).await,
            Err(AuthProductError::BackendConflict)
        ));
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION reject_callback_account() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled callback account rejection'; END $$; CREATE TRIGGER reject_callback_account BEFORE INSERT ON brassclaw_product_auth_accounts FOR EACH ROW EXECUTE FUNCTION reject_callback_account();").await.unwrap();
        let result = exchange(&original);
        assert!(matches!(
            first
                .complete_oauth_callback(&owner, completion(&original, result.clone()))
                .await,
            Err(AuthProductError::BackendUnavailable)
        ));
        let restarted = service(&rig.pool);
        let retained = restarted
            .read_oauth_callback_original(&owner, &original.original.reference())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.flow.status, AuthFlowStatus::Completing);
        assert_eq!(
            retained.action.outcome,
            OAuthCallbackOutcome::Returned(result)
        );
        assert!(retained.action.needs_publication());
        assert!(
            restarted
                .get_account(CredentialAccountLookupRequest::new(
                    owner.clone(),
                    original.target_account_id()
                ))
                .await
                .unwrap()
                .is_none()
        );
        client.batch_execute("DROP TRIGGER reject_callback_account ON brassclaw_product_auth_accounts; DROP FUNCTION reject_callback_account();").await.unwrap();
        let reference = original.original.reference();
        let other = service(&rig.pool);
        let (a, b) = tokio::join!(
            restarted.reconcile_oauth_callback(&owner, &reference),
            other.reconcile_oauth_callback(&owner, &reference)
        );
        let a = a.unwrap();
        let b = b.unwrap();
        assert_eq!(a.action, b.action);
        assert_eq!(a.flow, b.flow);
        assert_eq!(a.action.publication, OAuthCallbackPublication::Applied);
        assert_eq!(
            a.clone().completed_flow().unwrap().status,
            AuthFlowStatus::Completed
        );
        let account = restarted
            .get_account(CredentialAccountLookupRequest::new(
                owner.clone(),
                original.target_account_id(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.action.published_account.as_ref(), Some(&account));
        let revision: i64 = client
            .query_one(
                "SELECT revision FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
                &[&owner.resource.tenant_id.as_str(), &account.id.to_string()],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(revision, 0, "one absent-only final account publication");
        assert_eq!(
            service(&rig.pool)
                .complete_oauth_callback(&owner, completion(&original, exchange(&original)))
                .await
                .unwrap(),
            a.flow
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_callback_bound_snapshot_cancellation_and_later_account_edit_fence_publication()
    {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let backend = service(&rig.pool);
        let owner = owner_scope();
        let account = backend
            .create_account(account_request(owner.clone(), "callback-snapshot"))
            .await
            .unwrap();
        let mut request = oauth_flow(owner.clone());
        request.update_binding = Some(CredentialAccountUpdateBinding::from_projection(
            &account.projection(),
        ));
        let before_edit = backend.create_flow(request).await.unwrap();
        let revoked = backend
            .update_status(&owner, account.id, CredentialAccountStatus::Revoked)
            .await
            .unwrap();
        assert_eq!(
            backend
                .get_account(CredentialAccountLookupRequest::new(
                    owner.clone(),
                    account.id
                ))
                .await
                .unwrap(),
            Some(revoked)
        );
        assert!(matches!(
            backend
                .claim_oauth_callback(&owner, prepared(&before_edit))
                .await,
            Err(AuthProductError::BackendConflict)
        ));
        assert!(
            backend
                .read_oauth_callback_original(&owner, &prepared(&before_edit).original.reference())
                .await
                .unwrap()
                .is_none()
        );
        let second_account = backend
            .create_account(account_request(owner.clone(), "callback-second-snapshot"))
            .await
            .unwrap();
        let mut request = oauth_flow(owner.clone());
        request.update_binding = Some(CredentialAccountUpdateBinding::from_projection(
            &second_account.projection(),
        ));
        let after_edit = backend.create_flow(request).await.unwrap();
        let original = dispatched(
            backend
                .claim_oauth_callback(&owner, prepared(&after_edit))
                .await
                .unwrap(),
        );
        let winner = backend
            .update_status(&owner, second_account.id, CredentialAccountStatus::Revoked)
            .await
            .unwrap();
        assert!(matches!(
            backend
                .complete_oauth_callback(&owner, completion(&original, exchange(&original)))
                .await,
            Err(AuthProductError::FlowAlreadyTerminal)
        ));
        let retained = backend
            .read_oauth_callback_original(&owner, &original.original.reference())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            retained.action.publication,
            OAuthCallbackPublication::Superseded
        );
        assert!(matches!(
            retained.action.outcome,
            OAuthCallbackOutcome::Returned(_)
        ));
        assert_eq!(
            backend
                .get_account(CredentialAccountLookupRequest::new(
                    owner.clone(),
                    second_account.id
                ))
                .await
                .unwrap(),
            Some(winner)
        );
        let canceled_flow = backend
            .create_flow(oauth_flow(owner.clone()))
            .await
            .unwrap();
        let canceled = dispatched(
            backend
                .claim_oauth_callback(&owner, prepared(&canceled_flow))
                .await
                .unwrap(),
        );
        backend.cancel_flow(&owner, canceled_flow.id).await.unwrap();
        assert!(matches!(
            backend
                .complete_oauth_callback(&owner, completion(&canceled, exchange(&canceled)))
                .await,
            Err(AuthProductError::Canceled)
        ));
        let retained = backend
            .read_oauth_callback_original(&owner, &canceled.original.reference())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.flow.status, AuthFlowStatus::Canceled);
        assert_eq!(
            retained.action.publication,
            OAuthCallbackPublication::Superseded
        );
        assert!(retained.action.published_account.is_none());
        assert!(
            backend
                .get_account(CredentialAccountLookupRequest::new(
                    owner.clone(),
                    canceled.target_account_id()
                ))
                .await
                .unwrap()
                .is_none()
        );
        let expiring = backend
            .create_flow(oauth_flow(owner.clone()))
            .await
            .unwrap();
        let expired_action = dispatched(
            backend
                .claim_oauth_callback(&owner, prepared(&expiring))
                .await
                .unwrap(),
        );
        let (mut expired_document, revision) = backend
            .read_flow_document(&owner, expiring.id)
            .await
            .unwrap()
            .unwrap();
        // Exercise real persisted expiry using the original exact revision,
        // rather than a short scheduling-sensitive wall-clock startup deadline.
        expired_document.flow.expires_at = Utc::now() - chrono::Duration::seconds(1);
        backend
            .write_flow_document(&expired_document, Some(revision))
            .await
            .unwrap();
        assert!(matches!(
            backend
                .complete_oauth_callback(
                    &owner,
                    completion(&expired_action, exchange(&expired_action))
                )
                .await,
            Err(AuthProductError::UnknownOrExpiredFlow)
        ));
        let retained = backend
            .read_oauth_callback_original(&owner, &expired_action.original.reference())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.flow.status, AuthFlowStatus::Expired);
        assert_eq!(
            retained.action.publication,
            OAuthCallbackPublication::Superseded
        );
        assert!(matches!(
            retained.action.outcome,
            OAuthCallbackOutcome::Returned(_)
        ));
        assert!(
            backend
                .get_account(CredentialAccountLookupRequest::new(
                    owner,
                    expired_action.target_account_id()
                ))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_callback_unusable_return_and_denial_cannot_publish_success() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let backend = service(&rig.pool);
        let owner = owner_scope();
        for invalid in [
            "provider", "account", "code", "pkce", "label", "access", "scope",
        ] {
            let flow = backend
                .create_flow(oauth_flow(owner.clone()))
                .await
                .unwrap();
            let original = dispatched(
                backend
                    .claim_oauth_callback(&owner, prepared(&flow))
                    .await
                    .unwrap(),
            );
            let mut result = exchange(&original);
            match invalid {
                "provider" => result.provider = AuthProviderId::new("foreign-provider").unwrap(),
                "account" => result.account_id = Some(CredentialAccountId::new()),
                "code" => {
                    result.authorization_code_hash =
                        brassclaw_auth::AuthorizationCodeHash::new("d".repeat(64)).unwrap()
                }
                "pkce" => {
                    result.pkce_verifier_hash = PkceVerifierHash::new("e".repeat(64)).unwrap()
                }
                "label" => {
                    result.account_label = CredentialAccountLabel::new("foreign-label").unwrap()
                }
                "access" => {
                    result.access_secret = SecretHandle::new("foreign-unowned-output").unwrap()
                }
                "scope" => result.scopes.clear(),
                other => panic!("unknown fixture: {other}"),
            }
            assert!(
                matches!(
                    backend
                        .complete_oauth_callback(&owner, completion(&original, result.clone()))
                        .await,
                    Err(AuthProductError::TokenExchangeFailed)
                ),
                "{invalid}"
            );
            let retained = backend
                .read_oauth_callback_original(&owner, &original.original.reference())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                retained.action.outcome,
                OAuthCallbackOutcome::UnusableReturn(result)
            );
            assert_eq!(retained.flow.status, AuthFlowStatus::Failed);
            assert!(retained.action.published_account.is_none());
            assert!(
                backend
                    .get_account(CredentialAccountLookupRequest::new(
                        owner.clone(),
                        original.target_account_id()
                    ))
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(matches!(
                backend
                    .mark_continuation_dispatched(&owner, flow.id, Utc::now())
                    .await,
                Err(AuthProductError::FlowAlreadyTerminal)
            ));
        }
        let flow = backend
            .create_flow(oauth_flow(owner.clone()))
            .await
            .unwrap();
        assert!(matches!(
            backend
                .complete_oauth_callback(
                    &owner,
                    OAuthCallbackInput {
                        dispatched: None,
                        flow_id: flow.id,
                        opaque_state_hash: flow.opaque_state_hash.unwrap(),
                        outcome: ProviderCallbackOutcome::Denied,
                    }
                )
                .await,
            Err(AuthProductError::ProviderDenied)
        ));
        let denied = backend.get_flow(&owner, flow.id).await.unwrap().unwrap();
        assert_eq!(denied.status, AuthFlowStatus::Failed);
        assert_eq!(denied.error, Some(AuthErrorCode::ProviderDenied));
        assert!(matches!(
            backend
                .mark_continuation_dispatched(&owner, flow.id, Utc::now())
                .await,
            Err(AuthProductError::FlowAlreadyTerminal)
        ));
    }
}
