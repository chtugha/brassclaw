//! Real protected HTTP/DB/Secret original recovery before the Auth projection.
//! Test-only issuer responses are controlled; all local durable owners are real.
use super::*;
use brassclaw_auth::{CredentialAccountLookupRequest, DcrArtifactSlot, DcrRegistrationOutcome};
use brassclaw_secrets::{SecretCustodySource, SecretCustodyState};

#[derive(Clone, Copy)]
pub(crate) enum DcrPreparationBackend {
    NativePostgres,
    #[cfg(feature = "skills-db")]
    RecordFilesystem,
}
impl DcrPreparationBackend {
    fn table(self) -> &'static str {
        match self {
            Self::NativePostgres => "brassclaw_product_auth_flows",
            #[cfg(feature = "skills-db")]
            Self::RecordFilesystem => "root_filesystem_entries",
        }
    }
    async fn reject_flow(self, pool: &brassclaw_pg::PgPool, id: AuthFlowId) {
        // Both fragments and the UUID are test-owned, not browser SQL/source.
        let condition = match self {
            Self::NativePostgres => format!("NEW.id='{id}'"),
            #[cfg(feature = "skills-db")]
            Self::RecordFilesystem => format!("NEW.path LIKE '%/flows/{id}.json'"),
        };
        pool.get().await.unwrap().batch_execute(&format!(
            "CREATE FUNCTION reject_partial_dcr_flow() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN IF {condition} THEN RAISE EXCEPTION 'controlled original flow write rejection'; END IF;
             RETURN NEW; END $$;
             CREATE TRIGGER reject_partial_dcr_flow BEFORE INSERT OR UPDATE ON {}
             FOR EACH ROW EXECUTE FUNCTION reject_partial_dcr_flow();", self.table()
        )).await.unwrap();
    }
    async fn allow_flow(self, pool: &brassclaw_pg::PgPool) {
        pool.get().await.unwrap().batch_execute(&format!(
            "DROP TRIGGER reject_partial_dcr_flow ON {}; DROP FUNCTION reject_partial_dcr_flow();", self.table()
        )).await.unwrap();
    }
    async fn remove_account(
        self,
        pool: &brassclaw_pg::PgPool,
        id: brassclaw_auth::CredentialAccountId,
    ) {
        let client = pool.get().await.unwrap();
        let count = match self {
            Self::NativePostgres => client
                .execute(
                    "DELETE FROM brassclaw_product_auth_accounts WHERE id=$1",
                    &[&id.to_string()],
                )
                .await
                .unwrap(),
            #[cfg(feature = "skills-db")]
            Self::RecordFilesystem => client
                .execute(
                    "DELETE FROM root_filesystem_entries WHERE path LIKE $1",
                    &[&format!("%/accounts/{id}.json")],
                )
                .await
                .unwrap(),
        };
        assert_eq!(
            count, 1,
            "actually remove the selected account's stored record"
        );
    }
}

struct CountedPreparationEgress {
    requests: AtomicUsize,
    issuer: Arc<ObservedRegistration>,
}
#[async_trait::async_trait]
impl RuntimeHttpEgress for CountedPreparationEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, brassclaw_host_api::RuntimeHttpEgressError> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        self.issuer.execute(request).await
    }
}

struct NoPreparationContinuation;
#[async_trait::async_trait]
impl crate::auth::RebornAuthContinuationDispatcher for NoPreparationContinuation {
    async fn dispatch_auth_continuation(
        &self,
        _: brassclaw_auth::AuthContinuationEvent,
    ) -> Result<(), AuthProductError> {
        panic!("preparation recovery must not emit a successful turn continuation")
    }
}

pub(crate) async fn verify_dcr_preparation_handoff(
    ports: crate::auth::RebornProductAuthServicePorts,
    secrets: Arc<dyn SecretStore>,
    pool: &brassclaw_pg::PgPool,
    backend: DcrPreparationBackend,
) {
    use brassclaw_auth::{
        CredentialAccountLabel, CredentialAccountStatus, CredentialOwnership, NewCredentialAccount,
    };
    let issuer = Arc::new(ObservedRegistration {
        registrations: AtomicUsize::new(0),
        token_exchanges: AtomicUsize::new(0),
        deletions: AtomicUsize::new(0),
        malformed: false,
        cleanup_status: 204,
        cleanup_unknown: false,
    });
    let observed = Arc::new(CountedPreparationEgress {
        requests: AtomicUsize::new(0),
        issuer: issuer.clone(),
    });
    let provider = Arc::new(
        OAuthDcrProvider::new(
            OAuthDcrProviderConfig {
                spec: crate::notion_oauth::notion_provider_spec(),
                callback_origin: "http://127.0.0.1:3000".into(),
                client_name: "Brassclaw".into(),
                account_label: CredentialAccountLabel::new("notion").unwrap(),
                scopes: vec![],
            },
            observed.clone(),
            secrets.clone(),
            Arc::new(NoopObligationHandler),
        )
        .unwrap(),
    );
    let registry = Arc::new(OAuthDcrProviderRegistry::new(vec![provider]));
    let auth = Arc::new(
        ports
            .into_services(Arc::new(NoPreparationContinuation))
            .with_dcr_oauth_registry(registry.clone()),
    );
    let state = ProductAuthRouteState::new(
        auth.clone(),
        TenantId::new("tenant-alpha").unwrap(),
        None,
        None,
    )
    .with_services(extension_oauth_requirement::test_services("notion"));
    let app = product_auth_route_mount(state.clone())
        .protected
        .layer(axum::Extension(test_caller()));
    let provider_id = AuthProviderId::new("notion").unwrap();

    for mode in [
        "expired",
        "cancel_existing",
        "resume",
        "race",
        "later_artifact",
    ] {
        let mut input = request().await;
        let id =
            AuthFlowId::from_uuid(Uuid::parse_str(input["flow_id"].as_str().unwrap()).unwrap());
        let scope = scope_from_authenticated_caller_parts_requiring_invocation(
            &test_caller(),
            &ScopeFields {
                session_id: None,
                thread_id: None,
                invocation_id: Some(input["invocation_id"].as_str().unwrap().into()),
            },
        )
        .unwrap();
        let account = if mode == "cancel_existing" {
            let handle =
                brassclaw_host_api::SecretHandle::new(format!("actual-original-access-{id}"))
                    .unwrap();
            secrets
                .put(
                    scope.resource.clone(),
                    handle.clone(),
                    "actual encrypted original access sentinel".into(),
                )
                .await
                .unwrap();
            let account = auth
                .credential_account_service()
                .create_account(NewCredentialAccount {
                    scope: scope.clone(),
                    provider: provider_id.clone(),
                    label: CredentialAccountLabel::new("before preparation").unwrap(),
                    status: CredentialAccountStatus::Configured,
                    ownership: CredentialOwnership::UserReusable,
                    owner_extension: None,
                    granted_extensions: vec![
                        brassclaw_host_api::ExtensionId::new("notion").unwrap(),
                    ],
                    access_secret: Some(handle),
                    access_expires_at: None,
                    refresh_secret: None,
                    scopes: vec![],
                })
                .await
                .unwrap();
            input["target"] = json!({"kind":"existing","account_id":account.id});
            Some(account)
        } else {
            None
        };
        if mode == "expired" {
            input["expires_at"] = json!((Utc::now() + ChronoDuration::seconds(5)).to_rfc3339());
        }
        backend.reject_flow(pool, id).await;
        assert!(send(&app, input.clone()).await.0.is_server_error());
        assert!(
            auth.flow_manager()
                .get_flow(&scope, id)
                .await
                .unwrap()
                .is_none()
        );
        let original = registry
            .retained_setup_preparation(&scope, id, &provider_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            original.requirement_setup.as_ref().unwrap().requirement(),
            &extension_oauth_requirement::test_requirement("notion")
        );
        let before = secrets
            .custody(&scope.resource, &original.preparation.handle)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(before.reference, original.preparation.reference);
        let calls = observed.requests.load(Ordering::SeqCst);
        let posts = issuer.registrations.load(Ordering::SeqCst);
        let deletes = issuer.deletions.load(Ordering::SeqCst);
        backend.allow_flow(pool).await;

        // Full explicit identity is checked BEFORE creating a replacement projection.
        for field in ["expires_at", "requirement", "target"] {
            let mut changed = input.clone();
            changed["action"] = json!("cancel");
            changed[field] = match field {
                "expires_at" => {
                    json!((original.expires_at + ChronoDuration::seconds(1)).to_rfc3339())
                }
                "requirement" => json!("a different complete requirement"),
                _ if account.is_some() => json!({"kind":"new"}),
                _ => {
                    json!({"kind":"existing","account_id":brassclaw_auth::CredentialAccountId::new()})
                }
            };
            assert_eq!(send(&app, changed).await.0, StatusCode::CONFLICT);
            assert!(
                auth.flow_manager()
                    .get_flow(&scope, id)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                secrets
                    .custody(&scope.resource, &original.preparation.handle)
                    .await
                    .unwrap()
                    .unwrap(),
                before
            );
        }
        if mode == "expired"
            && let Ok(remaining) = (original.expires_at - Utc::now()).to_std()
        {
            tokio::time::sleep(remaining).await;
        }
        if let Some(account) = &account {
            backend.remove_account(pool, account.id).await;
            assert!(
                auth.credential_account_service()
                    .get_account(CredentialAccountLookupRequest::new(
                        scope.clone(),
                        account.id
                    ))
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        let mut no_manifest = state.clone();
        no_manifest.services = None;
        let recovered_app = product_auth_route_mount(no_manifest)
            .protected
            .layer(axum::Extension(test_caller()));
        let mut followup = input.clone();
        followup["action"] = json!(if mode == "cancel_existing" {
            "cancel"
        } else {
            "recover"
        });
        if mode == "later_artifact" {
            // Real later-stage provenance plus a missing flow is not proof of no effect.
            secrets
                .put_retained_from_original(
                    scope.resource.clone(),
                    original
                        .artifact_handle(DcrArtifactSlot::PreparationRead)
                        .unwrap(),
                    SecretCustodySource {
                        scope: scope.resource.clone(),
                        handle: original.preparation.handle.clone(),
                        reference: original.preparation.reference.clone(),
                    },
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                send(&recovered_app, followup.clone()).await.0,
                StatusCode::CONFLICT
            );
            followup["action"] = json!("cancel");
            assert_eq!(send(&recovered_app, followup).await.0, StatusCode::CONFLICT);
            assert!(
                auth.flow_manager()
                    .get_flow(&scope, id)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(observed.requests.load(Ordering::SeqCst), calls);
            continue;
        }
        if mode == "race" {
            let manager = auth.flow_manager();
            let (live, retired) = tokio::join!(
                manager.prepare_dcr_registration(original.clone()),
                manager.retire_dcr_preparation(
                    original.clone(),
                    brassclaw_auth::DcrCleanupReason::Canceled
                )
            );
            let live = live.unwrap();
            let retired = retired.unwrap();
            assert_eq!(live.action.original, original);
            assert_eq!(retired.action.original, original);
            assert_eq!(live.flow.id, retired.flow.id);
            followup["action"] = json!("cancel");
        }
        let (status, reply) = send(&recovered_app, followup.clone()).await;
        assert!(
            matches!(status, StatusCode::OK | StatusCode::ACCEPTED),
            "{mode}: {reply}"
        );
        let restored = auth
            .flow_manager()
            .read_dcr_registration(&scope, id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.action.original, original);
        assert_eq!(restored.flow.expires_at, original.expires_at);
        assert_eq!(restored.flow.update_binding, original.update_binding);
        assert_eq!(
            observed.requests.load(Ordering::SeqCst),
            calls,
            "projection repair never rediscovers or dispatches"
        );
        assert_eq!(issuer.registrations.load(Ordering::SeqCst), posts);
        if mode == "resume" {
            assert_eq!(restored.flow.status, AuthFlowStatus::Pending);
            let (status, active) = send(&recovered_app, input.clone()).await;
            assert_eq!(status, StatusCode::OK, "{active}");
            assert_eq!(active["status"], "awaiting_user");
            assert_eq!(issuer.registrations.load(Ordering::SeqCst), posts + 1);
            assert_eq!(
                observed.requests.load(Ordering::SeqCst),
                calls + 1,
                "reuse original discovery/PKCE, only the admitted registration is sent"
            );
        } else {
            assert!(matches!(
                restored.flow.status,
                AuthFlowStatus::Expired | AuthFlowStatus::Canceled
            ));
            assert!(restored.flow.challenge.is_none());
            assert!(matches!(
                restored.action.outcome,
                DcrRegistrationOutcome::Prepared
            ));
            assert!(restored.action.cleanup.remote_confirmed());
            assert!(
                auth.flow_manager()
                    .claim_dcr_registration(&original)
                    .await
                    .is_err()
            );
        }
        followup["action"] = json!("cancel");
        let (status, canceled) = send(&recovered_app, followup.clone()).await;
        assert_eq!(status, StatusCode::OK, "{mode}: {canceled}");
        assert_eq!(canceled["cleanup"]["local"], "confirmed");
        let closed = secrets
            .custody(&scope.resource, &original.preparation.handle)
            .await
            .unwrap()
            .unwrap();
        let SecretCustodyState::RetiredOriginal {
            original_reference,
            context,
        } = closed.state
        else {
            panic!("the actual encrypted preparation must have its exact retirement receipt");
        };
        assert_eq!(original_reference, original.preparation.reference);
        assert_eq!(context, original.preparation_context());
        let (status, repeated) = send(&recovered_app, followup).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(repeated, canceled);
        assert_eq!(
            issuer.registrations.load(Ordering::SeqCst),
            posts + if mode == "resume" { 1 } else { 0 }
        );
        assert_eq!(
            issuer.deletions.load(Ordering::SeqCst),
            deletes + if mode == "resume" { 1 } else { 0 }
        );
        assert_eq!(
            observed.requests.load(Ordering::SeqCst),
            calls + if mode == "resume" { 2 } else { 0 },
            "repeated terminal operations do not rediscover or send HTTP"
        );
        assert!(
            auth.credential_account_record_source()
                .requirement_selection(
                    &scope,
                    &extension_oauth_requirement::test_requirement("notion")
                )
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(issuer.token_exchanges.load(Ordering::SeqCst), 0);
}
