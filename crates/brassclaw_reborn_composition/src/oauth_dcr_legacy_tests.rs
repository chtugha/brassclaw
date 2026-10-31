use super::tests::{TestObligationHandler, sample_auth_scope, test_config};
use super::*;
use brassclaw_auth::{AuthFlowKind, AuthFlowRecord, NewAuthFlow};
use std::sync::atomic::{AtomicUsize, Ordering};

struct NoLegacyNetwork;
#[async_trait]
impl RuntimeHttpEgress for NoLegacyNetwork {
    async fn execute(
        &self,
        _request: RuntimeHttpEgressRequest,
    ) -> Result<
        brassclaw_host_api::RuntimeHttpEgressResponse,
        brassclaw_host_api::RuntimeHttpEgressError,
    > {
        panic!("legacy continuation must never discover/register/deregister again")
    }
}
struct LegacyTokenExchange {
    calls: AtomicUsize,
    pkce: SecretString,
    redirect: String,
}
#[async_trait]
impl RuntimeHttpEgress for LegacyTokenExchange {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<
        brassclaw_host_api::RuntimeHttpEgressResponse,
        brassclaw_host_api::RuntimeHttpEgressError,
    > {
        assert_eq!(request.method, NetworkMethod::Post);
        assert_eq!(
            request.url, "https://oauth.notion.com/token",
            "use the captured issuer, never discovery/current configuration"
        );
        let params = url::form_urlencoded::parse(&request.body)
            .into_owned()
            .collect::<BTreeMap<_, _>>();
        assert_eq!(params.get("grant_type").unwrap(), "authorization_code");
        assert_eq!(
            params.get("client_id").unwrap(),
            "old-already-registered-client"
        );
        assert_eq!(
            params.get("code_verifier").unwrap(),
            self.pkce.expose_secret()
        );
        assert_eq!(params.get("redirect_uri").unwrap(), &self.redirect);
        self.calls.fetch_add(1, Ordering::SeqCst);
        let body=br#"{"access_token":"legacy-flow-access","refresh_token":"legacy-flow-refresh","token_type":"Bearer","expires_in":3600}"#.to_vec();
        Ok(brassclaw_host_api::RuntimeHttpEgressResponse {
            status: 200,
            headers: vec![],
            response_bytes: body.len() as u64,
            body,
            saved_body: None,
            request_bytes: 0,
            redaction_applied: false,
        })
    }
}
async fn old_flow(
    provider: &OAuthDcrProvider,
    manager: &Arc<dyn AuthFlowManager>,
) -> AuthFlowRecord {
    let scope = sample_auth_scope();
    let id = AuthFlowId::new();
    let label = CredentialAccountLabel::new("old original account").unwrap();
    let verifier = PkceVerifierSecret::new(SecretString::from(
        brassclaw_common::pkce::generate_code_verifier(),
    ))
    .unwrap();
    let state = DcrOAuthCallbackState::new(
        id,
        scope.clone(),
        AuthProviderId::new("notion").unwrap(),
        label.clone(),
        vec![],
    )
    .encode()
    .unwrap();
    let raw = URL_SAFE_NO_PAD
        .decode(state.as_str().strip_prefix("icd1.").unwrap())
        .unwrap();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&raw)
            .unwrap()
            .get("surface")
            .is_none(),
        "historical Callback bytes retain their original shape"
    );
    let redirect = provider.callback_redirect_uri(&scope, id, &label).unwrap();
    let client = StoredDcrClientMaterial {
        client_id: "old-already-registered-client".into(),
        client_secret: None,
        redirect_uri: redirect.as_str().into(),
        token_endpoint: "https://oauth.notion.com/token".into(),
    };
    let endpoint = OAuthAuthorizationEndpoint::new("https://oauth.notion.com/authorize").unwrap();
    let client_id = OAuthClientId::new(client.client_id.clone()).unwrap();
    let challenge = build_authorization_url(OAuthAuthorizeUrlRequest {
        authorization_endpoint: &endpoint,
        client_id: &client_id,
        redirect_uri: &redirect,
        state: &state,
        code_challenge: &pkce_s256_challenge(&verifier),
        scopes: &[],
        extra_params: &[],
    })
    .unwrap();
    // Real historical layout: ordinary encrypted PKCE/client rows, no retained
    // preparation or registration response and no new DCR action.
    provider
        .put_secret(
            &scope.resource,
            flow_secret_handle(&provider.spec, id, "pkce").unwrap(),
            verifier.expose_secret().to_owned().into(),
        )
        .await
        .unwrap();
    provider
        .put_material(
            &scope.resource,
            flow_secret_handle(&provider.spec, id, "client").unwrap(),
            &client,
        )
        .await
        .unwrap();
    let expires = Utc::now() + ChronoDuration::minutes(5);
    manager
        .create_flow(NewAuthFlow {
            requirement_setup: None,
            id: Some(id),
            scope,
            kind: AuthFlowKind::IntegrationCredential,
            provider: AuthProviderId::new("notion").unwrap(),
            challenge: AuthChallenge::OAuthUrl {
                authorization_url: challenge,
                expires_at: expires,
            },
            continuation: AuthContinuationRef::SetupOnly,
            update_binding: None,
            opaque_state_hash: Some(opaque_state_hash(state.as_str()).unwrap()),
            pkce_verifier_hash: Some(pkce_verifier_hash(&verifier).unwrap()),
            expires_at: expires,
        })
        .await
        .unwrap()
}
async fn verify_legacy(manager: Arc<dyn AuthFlowManager>, store: Arc<dyn SecretStore>) {
    let provider = OAuthDcrProvider::new(
        test_config(),
        Arc::new(NoLegacyNetwork),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let old = old_flow(&provider, &manager).await;
    assert!(
        manager
            .read_dcr_registration(&old.scope, old.id)
            .await
            .unwrap()
            .is_none()
    );
    let legacy_pkce = flow_secret_handle(&provider.spec, old.id, "pkce").unwrap();
    let legacy_client = flow_secret_handle(&provider.spec, old.id, "client").unwrap();
    let original_pkce = provider
        .load_secret(&old.scope.resource, &legacy_pkce)
        .await
        .unwrap();
    let selected = provider
        .retain_legacy_flow(&manager, old.clone())
        .await
        .unwrap();
    assert_eq!(
        selected, old,
        "migration changes no status, expiry, IDs, challenge or account choice"
    );
    assert!(
        manager
            .read_dcr_registration(&old.scope, old.id)
            .await
            .unwrap()
            .is_none(),
        "no fictitious modern registration outcome"
    );
    store
        .put(
            old.scope.resource.clone(),
            legacy_pkce.clone(),
            "later unselected PKCE".into(),
        )
        .await
        .unwrap();
    provider
        .put_material(
            &old.scope.resource,
            legacy_client.clone(),
            &StoredDcrClientMaterial {
                client_id: "later unselected client".into(),
                client_secret: None,
                redirect_uri: "https://other.example/callback".into(),
                token_endpoint: "https://other.example/token".into(),
            },
        )
        .await
        .unwrap();
    let later_pkce = store
        .record_reference(&old.scope.resource, &legacy_pkce)
        .await
        .unwrap();
    let later_client = store
        .record_reference(&old.scope.resource, &legacy_client)
        .await
        .unwrap();
    assert_eq!(
        provider
            .cleanup_flow_material(&old.scope.resource, old.id)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict,
        "cleanup cannot delete a newer unselected generation by name"
    );
    assert_eq!(
        store
            .record_reference(&old.scope.resource, &legacy_pkce)
            .await
            .unwrap(),
        later_pkce
    );
    assert_eq!(
        store
            .record_reference(&old.scope.resource, &legacy_client)
            .await
            .unwrap(),
        later_client
    );
    let restarted = OAuthDcrProvider::new(
        test_config(),
        Arc::new(NoLegacyNetwork),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    assert_eq!(
        restarted
            .retain_legacy_flow(&manager, old.clone())
            .await
            .unwrap(),
        old
    );
    assert_eq!(
        restarted
            .pkce_verifier_for_flow(&old.scope, old.id)
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        original_pkce.expose_secret()
    );
    assert_eq!(
        restarted
            .load_flow_client_material(&old.scope.resource, old.id)
            .await
            .unwrap()
            .client_id
            .as_str(),
        "old-already-registered-client"
    );
    let selected = provider.legacy_definition(&old).unwrap();
    let continued = restarted
        .start_original_registration(
            &manager,
            original::DcrOriginalStart {
                requirement_setup: None,
                scope: old.scope.clone(),
                flow_id: old.id,
                account_label: selected.account_label,
                provider_scopes: selected.requested_scopes,
                continuation: old.continuation.clone(),
                update_binding: old.update_binding.clone(),
                expires_at: old.expires_at,
            },
        )
        .await
        .unwrap();
    assert_eq!(continued, old);
    let refresh = SecretHandle::new(format!("legacy-refresh-client-{}", old.id)).unwrap();
    restarted
        .bind_refresh_material(&old.scope.resource, old.id, &refresh)
        .await
        .unwrap();
    assert_eq!(
        restarted
            .load_refresh_client_material(&old.scope.resource, &refresh)
            .await
            .unwrap()
            .client_id
            .as_str(),
        "old-already-registered-client"
    );
    // Exercise the real provider-client and durable callback owners, not just
    // direct decoding of the sealed capsule. Only the token endpoint is allowed.
    use brassclaw_auth::{
        AuthFlowStatus, AuthProviderClient, OAuthAuthorizationCode, OAuthCallbackAdmission,
        OAuthCallbackClaimRequest, OAuthCallbackInput, OAuthCallbackOriginal,
        OAuthProviderCallbackRequest, OAuthProviderExchangeContext, ProviderCallbackOutcome,
    };
    let redirect = restarted
        .load_flow_client_material(&old.scope.resource, old.id)
        .await
        .unwrap()
        .redirect_uri
        .as_str()
        .to_owned();
    let token_egress = Arc::new(LegacyTokenExchange {
        calls: AtomicUsize::new(0),
        pkce: original_pkce.clone(),
        redirect,
    });
    let restarted = Arc::new(restarted);
    let client = crate::oauth_provider_client::HostOAuthProviderClient::new_with_client_material(
        restarted.spec.clone(),
        token_egress.clone(),
        store,
        Arc::new(TestObligationHandler),
        restarted.clone(),
    )
    .unwrap();
    let context = OAuthProviderExchangeContext {
        scope: old.scope.clone(),
        flow_id: old.id,
    };
    let code = OAuthAuthorizationCode::new("legacy-auth-code".into()).unwrap();
    let verifier = PkceVerifierSecret::new(original_pkce).unwrap();
    let request = OAuthProviderCallbackRequest {
        provider: old.provider.clone(),
        account_label: CredentialAccountLabel::new("old original account").unwrap(),
        authorization_code_hash: brassclaw_auth::authorization_code_hash(&code).unwrap(),
        authorization_code: code,
        pkce_verifier_hash: pkce_verifier_hash(&verifier).unwrap(),
        pkce_verifier: verifier,
        scopes: vec![],
    };
    let original = OAuthCallbackOriginal::from_request(
        old.scope.clone(),
        old.id,
        old.opaque_state_hash.clone().unwrap(),
        &request,
    )
    .unwrap();
    let destinations = client.prepare_callback(&context, &request).await.unwrap();
    let OAuthCallbackAdmission::Dispatch(dispatched) = manager
        .claim_oauth_callback(
            &old.scope,
            OAuthCallbackClaimRequest {
                original: original.clone(),
                destinations,
            },
        )
        .await
        .unwrap()
    else {
        panic!("first legacy callback must acquire its sole actual dispatch");
    };
    let exchange = client.exchange_callback(context, request).await.unwrap();
    let refresh = exchange
        .refresh_secret
        .clone()
        .expect("real token exchange retains refresh binding");
    let input = OAuthCallbackInput {
        dispatched: Some(dispatched.action),
        flow_id: old.id,
        opaque_state_hash: old.opaque_state_hash.clone().unwrap(),
        outcome: ProviderCallbackOutcome::Authorized { exchange },
    };
    let completed = manager
        .complete_oauth_callback(&old.scope, input.clone())
        .await
        .unwrap();
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    assert!(completed.credential_account_id.is_some());
    let projection = restarted
        .legacy_definition(&completed)
        .unwrap()
        .cleanup_projection(&completed)
        .unwrap();
    assert_eq!(
        projection.remote,
        brassclaw_auth::DcrRemoteCleanupStatus::Retained
    );
    assert!(
        projection.issue.is_none(),
        "a successful old account retains its registration without a false cleanup failure"
    );
    assert_eq!(
        manager
            .complete_oauth_callback(&old.scope, input)
            .await
            .unwrap(),
        completed
    );
    assert_eq!(
        token_egress.calls.load(Ordering::SeqCst),
        1,
        "reference recovery cannot exchange the authorization code twice"
    );
    assert_eq!(
        restarted
            .load_refresh_client_material(&old.scope.resource, &refresh)
            .await
            .unwrap()
            .client_id
            .as_str(),
        "old-already-registered-client"
    );
    assert!(
        manager
            .read_oauth_callback_original(&old.scope, &original.reference())
            .await
            .unwrap()
            .unwrap()
            .completed_flow()
            .is_ok()
    );
}
#[tokio::test]
async fn actual_legacy_flow_keeps_sealed_originals_after_mutable_old_rows_change_and_restart() {
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    verify_legacy(
        manager,
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
    )
    .await;
}
async fn verify_legacy_cleanup(manager: Arc<dyn AuthFlowManager>, store: Arc<dyn SecretStore>) {
    let provider = OAuthDcrProvider::new(
        test_config(),
        Arc::new(NoLegacyNetwork),
        store.clone(),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let old = old_flow(&provider, &manager).await;
    let pkce = flow_secret_handle(&provider.spec, old.id, "pkce").unwrap();
    let client = flow_secret_handle(&provider.spec, old.id, "client").unwrap();
    let selected_pkce = provider
        .load_secret(&old.scope.resource, &pkce)
        .await
        .unwrap();
    provider
        .retain_legacy_flow(&manager, old.clone())
        .await
        .unwrap();
    provider
        .cleanup_flow_material(&old.scope.resource, old.id)
        .await
        .unwrap();
    for handle in [&pkce, &client] {
        assert!(
            provider
                .load_secret(&old.scope.resource, handle)
                .await
                .is_err()
        );
        assert!(
            store
                .put(
                    old.scope.resource.clone(),
                    handle.clone(),
                    "late stale writer".into()
                )
                .await
                .is_err()
        );
    }
    let restarted = OAuthDcrProvider::new(
        test_config(),
        Arc::new(NoLegacyNetwork),
        store,
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    restarted
        .cleanup_flow_material(&old.scope.resource, old.id)
        .await
        .unwrap();
    assert_eq!(
        restarted
            .pkce_verifier_for_flow(&old.scope, old.id)
            .await
            .unwrap()
            .unwrap()
            .expose_secret(),
        selected_pkce.expose_secret(),
        "closure of stale raw inputs must preserve retained callback recovery"
    );
    assert_eq!(
        manager.get_flow(&old.scope, old.id).await.unwrap(),
        Some(old.clone())
    );
    let canceled = manager.cancel_flow(&old.scope, old.id).await.unwrap();
    let projection = restarted
        .legacy_definition(&canceled)
        .unwrap()
        .cleanup_projection(&canceled)
        .unwrap();
    assert_eq!(
        projection.remote,
        brassclaw_auth::DcrRemoteCleanupStatus::Unavailable
    );
    assert_eq!(
        projection.issue,
        Some(brassclaw_auth::DcrCleanupUnavailable::ManagementUnavailable)
    );
    assert_eq!(
        projection.local,
        brassclaw_auth::DcrLocalCleanupStatus::Retained
    );
    assert!(
        !projection.can_reconcile,
        "the historical schema never recorded a remote management credential"
    );
}
#[tokio::test]
async fn legacy_cleanup_closes_only_captured_inputs_and_survives_restart() {
    verify_legacy_cleanup(
        Arc::new(brassclaw_auth::InMemoryAuthProductServices::new()),
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
    )
    .await;
}
#[tokio::test]
async fn legacy_input_closure_reconciles_lost_ack_and_distinguishes_unresolved_from_replacement() {
    for after_commit in [false, true] {
        let store = Arc::new(super::tests::FaultingDcrSecretStore::new_at(
            usize::MAX,
            false,
        ));
        let manager: Arc<dyn AuthFlowManager> =
            Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let provider = OAuthDcrProvider::new(
            test_config(),
            Arc::new(NoLegacyNetwork),
            store.clone(),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let flow = old_flow(&provider, &manager).await;
        provider
            .retain_legacy_flow(&manager, flow.clone())
            .await
            .unwrap();
        store.fail_next_closure(after_commit);
        let result = provider
            .cleanup_flow_material(&flow.scope.resource, flow.id)
            .await;
        if after_commit {
            result.unwrap();
        } else {
            assert_eq!(
                result.unwrap_err(),
                AuthProductError::BackendUnavailable,
                "a rejected write without a closure receipt is not a proven generation conflict"
            );
        }
        let restarted = OAuthDcrProvider::new(
            test_config(),
            Arc::new(NoLegacyNetwork),
            store,
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        restarted
            .cleanup_flow_material(&flow.scope.resource, flow.id)
            .await
            .unwrap();
        assert!(
            restarted
                .pkce_verifier_for_flow(&flow.scope, flow.id)
                .await
                .unwrap()
                .is_some(),
            "selected callback input survives raw-source closure"
        );
        assert_eq!(
            manager.get_flow(&flow.scope, flow.id).await.unwrap(),
            Some(flow)
        );
    }
}
#[tokio::test]
async fn legacy_registry_rejects_foreign_invocation_and_corrupt_modern_marker_before_sealing() {
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    let store: Arc<dyn SecretStore> = Arc::new(brassclaw_secrets::InMemorySecretStore::new());
    let provider = Arc::new(
        OAuthDcrProvider::new(
            test_config(),
            Arc::new(NoLegacyNetwork),
            store.clone(),
            Arc::new(TestObligationHandler),
        )
        .unwrap(),
    );
    let flow = old_flow(&provider, &manager).await;
    let registry = OAuthDcrProviderRegistry::new(vec![provider.clone()]);
    let mut foreign = flow.scope.clone();
    foreign.resource.invocation_id = InvocationId::new();
    assert!(
        registry
            .pkce_verifier_for_flow(&manager, &foreign, &flow.provider, flow.id)
            .await
            .is_err()
    );
    let capsule = flow_secret_handle(&provider.spec, flow.id, "legacy-selection-original").unwrap();
    assert!(
        store
            .record_reference(&flow.scope.resource, &capsule)
            .await
            .unwrap()
            .is_none()
    );
    // A real existing preparation identity is evidence of a modern flow whose
    // missing action needs repair, never permission to manufacture old custody.
    let marker = flow_secret_handle(&provider.spec, flow.id, "preparation-original").unwrap();
    store
        .put(
            flow.scope.resource.clone(),
            marker,
            "controlled corrupt preparation marker".into(),
        )
        .await
        .unwrap();
    assert!(
        registry
            .pkce_verifier_for_flow(&manager, &flow.scope, &flow.provider, flow.id)
            .await
            .is_err()
    );
    assert!(
        store
            .record_reference(&flow.scope.resource, &capsule)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        manager.get_flow(&flow.scope, flow.id).await.unwrap(),
        Some(flow)
    );
}
#[tokio::test]
async fn legacy_capture_rejects_inconsistent_authorization_contract_and_unsafe_saved_endpoint() {
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
    let provider = OAuthDcrProvider::new(
        test_config(),
        Arc::new(NoLegacyNetwork),
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
        Arc::new(TestObligationHandler),
    )
    .unwrap();
    let flow = old_flow(&provider, &manager).await;
    let Some(AuthChallenge::OAuthUrl {
        authorization_url, ..
    }) = &flow.challenge
    else {
        panic!("OAuth challenge");
    };
    for (key, value) in [("scope", "unrequested-scope"), ("response_type", "token")] {
        let mut parsed = url::Url::parse(authorization_url.as_str()).unwrap();
        let mut pairs = parsed
            .query_pairs()
            .filter(|(name, _)| name != key)
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        pairs.push((key.into(), value.into()));
        parsed.set_query(None);
        parsed.query_pairs_mut().extend_pairs(pairs);
        let mut changed = flow.clone();
        changed.challenge = Some(AuthChallenge::OAuthUrl {
            authorization_url: brassclaw_auth::OAuthAuthorizationUrl::new(parsed.as_str()).unwrap(),
            expires_at: flow.expires_at,
        });
        assert!(provider.legacy_definition(&changed).is_err());
    }
    let client = flow_secret_handle(&provider.spec, flow.id, "client").unwrap();
    for endpoint in [
        "https://credentials@oauth.notion.com/token",
        "https://oauth.notion.com/token#fragment",
        "https://other.example/token",
    ] {
        provider
            .put_material(
                &flow.scope.resource,
                client.clone(),
                &StoredDcrClientMaterial {
                    client_id: "old-already-registered-client".into(),
                    client_secret: None,
                    redirect_uri: provider
                        .callback_redirect_uri(
                            &flow.scope,
                            flow.id,
                            &CredentialAccountLabel::new("old original account").unwrap(),
                        )
                        .unwrap()
                        .as_str()
                        .into(),
                    token_endpoint: endpoint.into(),
                },
            )
            .await
            .unwrap();
        assert!(
            provider
                .retain_legacy_flow(&manager, flow.clone())
                .await
                .is_err()
        );
        assert!(
            provider
                .secret_store
                .record_reference(
                    &flow.scope.resource,
                    &flow_secret_handle(&provider.spec, flow.id, "legacy-selection-original")
                        .unwrap()
                )
                .await
                .unwrap()
                .is_none()
        );
    }
}
#[tokio::test]
async fn changed_or_incomplete_old_material_is_rejected_without_any_registration_replay() {
    for change_pkce in [false, true] {
        let manager: Arc<dyn AuthFlowManager> =
            Arc::new(brassclaw_auth::InMemoryAuthProductServices::new());
        let provider = OAuthDcrProvider::new(
            test_config(),
            Arc::new(NoLegacyNetwork),
            Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
            Arc::new(TestObligationHandler),
        )
        .unwrap();
        let flow = old_flow(&provider, &manager).await;
        if change_pkce {
            provider
                .put_secret(
                    &flow.scope.resource,
                    flow_secret_handle(&provider.spec, flow.id, "pkce").unwrap(),
                    "wrong verifier".into(),
                )
                .await
                .unwrap();
        } else {
            provider
                .secret_store
                .delete(
                    &flow.scope.resource,
                    &flow_secret_handle(&provider.spec, flow.id, "client").unwrap(),
                )
                .await
                .unwrap();
        }
        assert!(
            provider
                .retain_legacy_flow(&manager, flow.clone())
                .await
                .is_err()
        );
        assert_eq!(
            manager.get_flow(&flow.scope, flow.id).await.unwrap(),
            Some(flow)
        );
    }
}
#[test]
fn dcr_state_preserves_explicit_surface_while_old_callback_encoding_stays_readable() {
    let mut scope = sample_auth_scope();
    scope.surface = brassclaw_auth::AuthSurface::Web;
    let state = DcrOAuthCallbackState::new(
        AuthFlowId::new(),
        scope.clone(),
        AuthProviderId::new("notion").unwrap(),
        CredentialAccountLabel::new("web-role").unwrap(),
        vec![],
    )
    .encode()
    .unwrap();
    assert_eq!(
        DcrOAuthCallbackState::decode(state.as_str())
            .unwrap()
            .scope(),
        &scope
    );
}
#[cfg(feature = "skills-db")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_and_persistent_fs_legacy_selections_retain_real_old_inputs_without_new_issuer_work()
{
    use brassclaw_filesystem::{PostgresRootFilesystem, ScopedFilesystem};
    use brassclaw_host_api::{MountAlias, MountGrant, MountPermissions, MountView, VirtualPath};
    use brassclaw_pg::test_support::{AckBoundary, AckCutRule, AckCutTransport};
    use brassclaw_secrets::{FilesystemSecretStore, PgSecretStore, SecretsCrypto};
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    rig.pool
        .get()
        .await
        .unwrap()
        .batch_execute(crate::pg_auth_product_services::MIGRATIONS)
        .await
        .unwrap();
    let store = Arc::new(
        PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "controlled-legacy-dcr-native-master-key".into(),
            "legacy-dcr-selection",
        )
        .unwrap(),
    );
    let native_ack = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![br#""dcr_legacy""#.to_vec()],
            vec![],
            AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let manager: Arc<dyn AuthFlowManager> =
        Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
            native_ack.pool.clone(),
            store.clone(),
        ));
    native_ack.arm(1).unwrap();
    verify_legacy(manager.clone(), store.clone()).await;
    assert_eq!(
        native_ack.cut_count(),
        1,
        "must actually lose the committed legacy-selection acknowledgement"
    );
    verify_legacy_cleanup(manager, store).await;
    let fs_ack = AckCutTransport::start(
        rig.url.expose_secret(),
        vec![AckCutRule::after_bind(
            1,
            vec![br#""dcr_legacy""#.to_vec()],
            vec![],
            AckBoundary::IdleReady,
        )],
    )
    .await
    .unwrap();
    let root = Arc::new(PostgresRootFilesystem::new(fs_ack.pool.as_ref().clone()));
    root.run_migrations().await.unwrap();
    let view = MountView::new(vec![MountGrant::new(
        MountAlias::new("/secrets").unwrap(),
        VirtualPath::new("/secrets/legacy-dcr-selection-tests").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let fs = Arc::new(ScopedFilesystem::with_fixed_view(root, view));
    let store: Arc<dyn SecretStore> = Arc::new(FilesystemSecretStore::new(
        fs.clone(),
        Arc::new(SecretsCrypto::new("controlled-legacy-dcr-fs-master-key".into()).unwrap()),
    ));
    let manager: Arc<dyn AuthFlowManager> = Arc::new(
        crate::product_auth_durable::FilesystemAuthProductServices::new(fs, store.clone()),
    );
    fs_ack.arm(1).unwrap();
    verify_legacy(manager.clone(), store.clone()).await;
    assert_eq!(
        fs_ack.cut_count(),
        1,
        "persistent FS must reconcile its actual lost CAS reply"
    );
    verify_legacy_cleanup(manager, store).await;
}
