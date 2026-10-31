//! Native acknowledged settings + actual PG Auth original and host/network
//! denial. The cached issuer fixture is data, never a successful Tool response.
use super::tests::{sample_auth_scope, test_config};
use super::*;
use brassclaw_host_api::{RuntimeHttpEgressError, RuntimeHttpEgressResponse};
use brassclaw_host_runtime::{BuiltinObligationServices, HostPrimitiveSettingsSource};
use brassclaw_product_workflow::MontyMemoryMode;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Notify;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_auth_backend_wait_captures_acknowledged_policy_without_retiming_a_call() {
    use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};
    use axum::{http::StatusCode, response::IntoResponse};
    use brassclaw_auth::AuthProductError;

    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("auth-backend-wait", directory.path())
                .with_tenant_id("default")
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let settings = runtime.webui_monty_settings_store();
    let original = settings.get("default", "default").await.unwrap();
    let auth = runtime.services().product_auth.as_ref().unwrap().clone();
    let mut limits = original.execution_limits;
    limits.auth_backend_timeout_millis = 5000;
    let first = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(serde_json::json!({
                "expected_revision": original.revision, "execution_limits": limits
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        auth.backend_timeout().await.unwrap(),
        Duration::from_secs(5)
    );

    // This observes the route's wait mechanism, not a successful Auth/Tool call.
    // No business result, credential or approval is synthesized by the future.
    let entered = Arc::new(Notify::new());
    let released = Arc::new(Notify::new());
    let pending = {
        let auth = auth.clone();
        let entered = entered.clone();
        let released = released.clone();
        tokio::spawn(async move {
            crate::product_auth_serve::run_with_backend_timeout(auth.as_ref(), async {
                entered.notify_one();
                released.notified().await;
                Err::<(), _>(AuthProductError::BackendUnavailable)
            })
            .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    limits.auth_backend_timeout_millis = 1;
    let second = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(serde_json::json!({
                "expected_revision": first.revision, "execution_limits": limits
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        auth.backend_timeout().await.unwrap(),
        Duration::from_millis(1)
    );
    let effective = settings
        .runtime_status()
        .await
        .unwrap()
        .unwrap()
        .execution_limits
        .unwrap();
    assert_eq!(effective.effective_revision, second.revision);
    assert_eq!(effective.limits.auth_backend_timeout_millis, 1);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !pending.is_finished(),
        "a live edit must not retime an accepted call"
    );
    released.notify_one();
    assert_eq!(
        pending.await.unwrap().unwrap_err().into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let timed_out = crate::product_auth_serve::run_with_backend_timeout(
        auth.as_ref(),
        std::future::pending::<Result<(), AuthProductError>>(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        timed_out.into_response().status(),
        StatusCode::GATEWAY_TIMEOUT
    );

    // An actual unbound owner fails before polling the backend, never silently
    // adopting a default when configured policy is unavailable.
    let unavailable = (*auth).clone().with_primitive_settings_source(Arc::new(
        crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new(),
    ));
    let polled = AtomicBool::new(false);
    let denied = crate::product_auth_serve::run_with_backend_timeout(&unavailable, async {
        polled.store(true, Ordering::SeqCst);
        Err::<(), _>(AuthProductError::BackendUnavailable)
    })
    .await
    .unwrap_err();
    assert_eq!(
        denied.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(!polled.load(Ordering::SeqCst));
    settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(serde_json::json!({
                "expected_revision": second.revision, "execution_limits": original.execution_limits
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        auth.backend_timeout().await.unwrap(),
        Duration::from_millis(u64::from(
            original.execution_limits.auth_backend_timeout_millis
        ),)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_manual_setup_lifetimes_follow_acknowledged_settings_and_keep_originals() {
    use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};
    use brassclaw_auth::{
        AuthContinuationRef, AuthProductError, AuthProviderId, AuthSurface, CredentialAccountLabel,
        ManualTokenSetupIdentity,
    };
    use brassclaw_product_workflow::{
        ExtensionCredentialPrepareRequest, ExtensionCredentialSetupService,
        ExtensionCredentialTarget,
    };

    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("manual-setup-lifetimes", directory.path())
                .with_tenant_id("default")
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let settings = runtime.webui_monty_settings_store();
    let original = settings.get("default", "default").await.unwrap();
    let auth = runtime.services().product_auth.as_ref().unwrap().clone();
    let mut scope = sample_auth_scope();
    scope.surface = AuthSurface::Web;
    let mut limits = original.execution_limits;
    limits.manual_token_setup_ttl_secs = 30;
    limits.extension_manual_token_setup_ttl_secs = 45;
    let first = settings.upsert("default", "default", &serde_json::from_value(
        serde_json::json!({"expected_revision":original.revision,"execution_limits":limits})
    ).unwrap()).await.unwrap();
    // Enter the existing protected handler with an authenticated caller. This
    // exercises route/facade/native storage; bearer enforcement is separate.
    use tower::ServiceExt;
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        scope.resource.tenant_id.clone(),
        scope.resource.user_id.clone(),
        scope.resource.agent_id.clone(),
        scope.resource.project_id.clone(),
    );
    let app = crate::product_auth_serve::product_auth_route_mount(
        crate::product_auth_serve::ProductAuthRouteState::new(
            auth.clone(),
            scope.resource.tenant_id.clone(),
            scope.resource.agent_id.clone(),
            scope.resource.project_id.clone(),
        ),
    )
    .protected
    .layer(axum::Extension(caller));
    async fn post_setup(app: &axum::Router, body: &serde_json::Value) -> serde_json::Value {
        use axum::{
            body::{Body, to_bytes},
            http::Request,
        };
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(crate::product_auth_serve::MANUAL_TOKEN_SETUP_PATH)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    }
    let route_request = serde_json::json!({"identity":ManualTokenSetupIdentity::default(),
        "invocation_id":InvocationId::new(),"provider":"route-lifetime","account_label":"route lifetime"});
    let before = Utc::now();
    let route_first = post_setup(&app, &route_request).await;
    let route_expiry: chrono::DateTime<Utc> =
        serde_json::from_value(route_first["expires_at"].clone()).unwrap();
    assert!(route_expiry >= before + chrono::Duration::seconds(30));
    assert!(route_expiry <= Utc::now() + chrono::Duration::seconds(30));
    let identity = ManualTokenSetupIdentity::default();
    let before = Utc::now();
    let expiry = auth.manual_token_setup_expiry().await.unwrap();
    let after = Utc::now();
    assert!(expiry >= before + chrono::Duration::seconds(30));
    assert!(expiry <= after + chrono::Duration::seconds(30));
    let request = crate::RebornManualTokenSetupRequest::new(
        scope.clone(),
        AuthProviderId::new("manual-lifetime-provider").unwrap(),
        CredentialAccountLabel::new("original lifetime").unwrap(),
        AuthContinuationRef::SetupOnly,
        expiry,
    );
    let accepted = auth
        .request_original_manual_token_setup(identity, request.clone())
        .await
        .unwrap();
    assert_eq!(accepted.challenge.expires_at, expiry);

    let concurrent_id = ManualTokenSetupIdentity::default();
    let mut another_candidate = request.clone();
    another_candidate.expires_at += chrono::Duration::seconds(5);
    let (a, b) = tokio::join!(
        auth.request_original_manual_token_setup(concurrent_id, request.clone()),
        auth.request_original_manual_token_setup(concurrent_id, another_candidate)
    );
    assert_eq!(
        a.unwrap(),
        b.unwrap(),
        "concurrent creation retains one winning first expiry"
    );

    let adapter =
        crate::webui_extension_credentials::ProductAuthExtensionCredentialSetup::new(auth.clone());
    let requirement = crate::webui_extension_credentials::test_requirement(
        "extension-lifetime-provider",
        "extension-lifetime",
        "access",
        &[],
    );
    let extension = ExtensionCredentialPrepareRequest {
        scope: scope.clone(),
        requester_extension: requirement.requester_extension().clone(),
        requirement_name: requirement.reference().unwrap(),
        candidate_requirement: Some(requirement),
        identity: ManualTokenSetupIdentity::default(),
        target: ExtensionCredentialTarget::New {},
    };
    let before = Utc::now();
    let extension_first = adapter
        .prepare_manual_token(extension.clone())
        .await
        .unwrap();
    assert!(extension_first.expires_at >= before + chrono::Duration::seconds(45));
    assert!(extension_first.expires_at <= Utc::now() + chrono::Duration::seconds(45));

    limits.manual_token_setup_ttl_secs = 1;
    limits.extension_manual_token_setup_ttl_secs = u32::MAX;
    let second = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                serde_json::json!({"expected_revision":first.revision,"execution_limits":limits}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let effective = settings
        .runtime_status()
        .await
        .unwrap()
        .unwrap()
        .execution_limits
        .unwrap();
    assert_eq!(effective.effective_revision, second.revision);
    assert_eq!(effective.limits.manual_token_setup_ttl_secs, 1);
    assert_eq!(
        effective.limits.extension_manual_token_setup_ttl_secs,
        u32::MAX
    );
    assert_eq!(
        post_setup(&app, &route_request).await,
        route_first,
        "a route retry must keep the original expiry after a reduction"
    );
    let mut retry = request.clone();
    retry.expires_at = auth.manual_token_setup_expiry().await.unwrap();
    let (a, b) = tokio::join!(
        auth.request_original_manual_token_setup(identity, retry.clone()),
        auth.request_original_manual_token_setup(identity, retry.clone())
    );
    assert_eq!(a.unwrap().challenge.expires_at, expiry);
    assert_eq!(b.unwrap().challenge.expires_at, expiry);
    let recovered = adapter
        .prepare_manual_token(extension.clone())
        .await
        .unwrap();
    assert_eq!(recovered, extension_first);
    let mut changed = request.clone();
    changed.label = CredentialAccountLabel::new("changed original").unwrap();
    assert!(
        auth.request_original_manual_token_setup(identity, changed)
            .await
            .is_err()
    );
    let mut new_extension = extension;
    new_extension.identity = ManualTokenSetupIdentity::default();
    let before = Utc::now();
    let large = adapter.prepare_manual_token(new_extension).await.unwrap();
    assert!(large.expires_at >= before + chrono::Duration::seconds(i64::from(u32::MAX)));
    assert!(large.expires_at <= Utc::now() + chrono::Duration::seconds(i64::from(u32::MAX)));
    let short_identity = ManualTokenSetupIdentity::default();
    retry.expires_at = auth.manual_token_setup_expiry().await.unwrap();
    let short = auth
        .request_original_manual_token_setup(short_identity, retry.clone())
        .await
        .unwrap();
    assert!(short.challenge.expires_at < expiry);
    tokio::time::sleep(
        (short.challenge.expires_at - Utc::now())
            .to_std()
            .unwrap_or_default()
            + Duration::from_millis(25),
    )
    .await;
    retry.expires_at = auth.manual_token_setup_expiry().await.unwrap();
    assert_eq!(
        auth.request_original_manual_token_setup(short_identity, retry)
            .await
            .unwrap_err()
            .code,
        brassclaw_auth::AuthErrorCode::UnknownOrExpiredFlow
    );
    let retained = auth
        .recover_original_manual_token_setup(&scope, short_identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.expires_at, short.challenge.expires_at);

    let unavailable = auth
        .as_ref()
        .clone()
        .with_primitive_settings_source(Arc::new(
            crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new(),
        ));
    assert!(matches!(
        unavailable.manual_token_setup_expiry().await,
        Err(AuthProductError::BackendUnavailable)
    ));
    assert!(matches!(
        unavailable.extension_manual_token_setup_expiry().await,
        Err(AuthProductError::BackendUnavailable)
    ));
    settings.upsert("default", "default", &serde_json::from_value(
        serde_json::json!({"expected_revision":second.revision,"execution_limits":original.execution_limits})
    ).unwrap()).await.unwrap();
}

#[derive(Debug, PartialEq, Eq)]
struct ObservedBounds {
    method: NetworkMethod,
    timeout: Option<u32>,
    response_bytes: Option<u64>,
    egress_bytes: Option<u64>,
}
struct ObservedEgress {
    inner: Arc<dyn RuntimeHttpEgress>,
    observations: Mutex<Vec<ObservedBounds>>,
    failures: Mutex<Vec<&'static str>>,
    pause_once: AtomicBool,
    entered: Notify,
    released: Notify,
}
impl fmt::Debug for ObservedEgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObservedEgress").finish_non_exhaustive()
    }
}
#[async_trait]
impl RuntimeHttpEgress for ObservedEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        self.observations.lock().unwrap().push(ObservedBounds {
            method: request.method,
            timeout: request.timeout_ms,
            response_bytes: request.response_body_limit,
            egress_bytes: request.network_policy.max_egress_bytes,
        });
        if self.pause_once.swap(false, Ordering::AcqRel) {
            self.entered.notify_one();
            self.released.notified().await;
        }
        let result = self.inner.execute(request).await;
        if let Err(error) = &result {
            self.failures
                .lock()
                .unwrap()
                .push(error.stable_runtime_reason());
        }
        result
    }
}
fn metadata_fixture() -> CachedAuthorizationServerMetadata {
    CachedAuthorizationServerMetadata {
        metadata: AuthorizationServerMetadata {
            authorization_endpoint: "https://127.0.0.1:9/authorize".into(),
            token_endpoint: "https://127.0.0.1:9/token".into(),
            registration_endpoint: "https://127.0.0.1:9/register".into(),
        },
        stored_at: Instant::now(),
        ttl_secs: 120,
    }
}

async fn reject_flow_insert<F>(
    pool: &brassclaw_pg::PgPool,
    flow_id: AuthFlowId,
    operation: F,
) -> Result<(), AuthProductError>
where
    F: std::future::Future<Output = Result<(), AuthProductError>>,
{
    let client = pool.get().await.unwrap();
    client
        .batch_execute(&format!(
            "CREATE FUNCTION reject_original_gate_flow() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'controlled original DCR flow publication failure'; END $$;
         CREATE TRIGGER reject_original_gate_flow BEFORE INSERT ON brassclaw_product_auth_flows
         FOR EACH ROW WHEN (NEW.id = '{flow_id}') EXECUTE FUNCTION reject_original_gate_flow();"
        ))
        .await
        .unwrap();
    let result = operation.await;
    // Remove the real fault before assertions or retrying. The native rig is
    // private to this test, including if a panic tears it down.
    client
        .batch_execute(
            "DROP TRIGGER reject_original_gate_flow ON brassclaw_product_auth_flows;
         DROP FUNCTION reject_original_gate_flow();",
        )
        .await
        .unwrap();
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_http_limits_refresh_between_requests_and_do_not_spend_unbound_claim() {
    use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("dcr-http-owner", directory.path())
                .with_tenant_id("default")
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let settings = runtime.webui_monty_settings_store();
    let original = settings.get("default", "default").await.unwrap();
    let source = runtime
        .services()
        .primitive_settings
        .as_ref()
        .unwrap()
        .clone();
    let secrets = runtime.services().secret_store.clone();
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secrets.clone(),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
    );
    let egress = Arc::new(ObservedEgress {
        inner: Arc::new(services.host_http_egress(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
        )),
        observations: Mutex::new(vec![]),
        failures: Mutex::new(vec![]),
        pause_once: AtomicBool::new(true),
        entered: Notify::new(),
        released: Notify::new(),
    });
    let mut config = test_config();
    config.spec.resource = Some("https://127.0.0.1:9/mcp");
    let provider = OAuthDcrProvider::new(
        config,
        egress.clone(),
        secrets,
        Arc::new(services.obligation_handler()),
    )
    .unwrap()
    .with_primitive_settings_source(Some(source.clone()));
    let mut first = original.execution_limits;
    first.oauth_dcr_timeout_millis = 47000;
    first.oauth_dcr_response_body_bytes = 65539;
    let changed = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                serde_json::json!({"expected_revision":original.revision,"execution_limits":first}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let active = {
        let provider = provider.clone();
        tokio::spawn(async move {
            provider
                .discover_authorization_server(&sample_auth_scope().resource)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), egress.entered.notified())
        .await
        .unwrap();
    let mut second = first;
    second.oauth_dcr_timeout_millis = 51000;
    second.oauth_dcr_response_body_bytes = 131075;
    let newer = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                serde_json::json!({"expected_revision":changed.revision,"execution_limits":second}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let captured = source.snapshot().await.unwrap();
    assert_eq!(captured.revision, newer.revision);
    assert_eq!(captured.limits, second);
    let effective = settings
        .runtime_status()
        .await
        .unwrap()
        .unwrap()
        .execution_limits
        .unwrap();
    assert_eq!(effective.effective_revision, newer.revision);
    assert_eq!(effective.limits.oauth_dcr_timeout_millis, 51000);
    assert_eq!(effective.limits.oauth_dcr_response_body_bytes, 131075);
    egress.released.notify_one();
    assert!(active.await.unwrap().is_err());
    // The first admitted GET keeps its old pair; the later fallback GET must
    // observe the newly acknowledged pair. No fields may mix revisions.
    assert_eq!(
        *egress.observations.lock().unwrap(),
        vec![
            ObservedBounds {
                method: NetworkMethod::Get,
                timeout: Some(47000),
                response_bytes: Some(65539),
                egress_bytes: Some(65539)
            },
            ObservedBounds {
                method: NetworkMethod::Get,
                timeout: Some(51000),
                response_bytes: Some(131075),
                egress_bytes: Some(131075)
            },
        ]
    );
    assert!(
        egress
            .failures
            .lock()
            .unwrap()
            .iter()
            .all(|failure| *failure == "policy_denied")
    );
    // A validated metadata data fixture isolates the original native claim
    // boundary; it does not invent a registration or management response.
    *provider.metadata_cache.write().await = Some(metadata_fixture());
    let unbound = provider
        .clone()
        .with_primitive_settings_source(Some(Arc::new(
            crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new(),
        )));
    let manager = runtime
        .services()
        .product_auth
        .as_ref()
        .unwrap()
        .flow_manager();
    let request = DcrOriginalStart {
        requirement_setup: None,
        scope: sample_auth_scope(),
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("real-native-dcr-limits").unwrap(),
        provider_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        deadline: OAuthSetupDeadline::Exact(Utc::now() + ChronoDuration::seconds(300)),
    };
    // The original is accepted under a real acknowledged cache policy. Stop
    // its real flow insert once, then verify unavailable HTTP settings cannot
    // consume the original registration claim during recovery.
    assert_eq!(
        reject_flow_insert(rig.pool.as_ref(), request.flow_id, async {
            provider
                .start_original_registration(&manager, request.clone())
                .await
                .map(|_| ())
        })
        .await
        .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    assert!(
        unbound
            .start_original_registration(&manager, request.clone())
            .await
            .is_err()
    );
    let before = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        before.action.outcome,
        brassclaw_auth::DcrRegistrationOutcome::Prepared
    ));
    assert_eq!(
        egress.observations.lock().unwrap().len(),
        2,
        "unavailable policy must not dispatch or consume the original claim"
    );
    assert!(
        provider
            .start_original_registration(&manager, request.clone())
            .await
            .is_err()
    );
    let after = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.action.original, after.action.original);
    assert!(matches!(
        after.action.outcome,
        brassclaw_auth::DcrRegistrationOutcome::Unknown { .. }
    ));
    assert_eq!(egress.observations.lock().unwrap().len(), 3);
    // Unknown original readback cannot replay, even after the live owner becomes
    // unavailable. It must consult the retained outcome, not current settings.
    assert!(matches!(
        unbound
            .start_original_registration(&manager, request.clone())
            .await,
        Err(AuthProductError::BackendUnavailable)
    ));
    let retained = manager
        .read_dcr_registration(&request.scope, request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.action, after.action);
    assert_eq!(egress.observations.lock().unwrap().len(), 3);
    let scope = sample_auth_scope().resource;
    let http = provider.capture_http_limits().await.unwrap();
    assert!(
        provider
            .execute_request(
                scope.clone(),
                NetworkMethod::Delete,
                "https://127.0.0.1:9/register/client",
                vec![],
                http,
                None
            )
            .await
            .is_err()
    );
    assert_eq!(
        &egress.observations.lock().unwrap()[2..],
        &[
            ObservedBounds {
                method: NetworkMethod::Post,
                timeout: Some(51000),
                response_bytes: Some(131075),
                egress_bytes: Some(131075)
            },
            ObservedBounds {
                method: NetworkMethod::Delete,
                timeout: Some(51000),
                response_bytes: Some(131075),
                egress_bytes: Some(131075)
            },
        ]
    );
    assert_eq!(*egress.failures.lock().unwrap(), vec!["policy_denied"; 4]);
    for (timeout, bytes) in [(0, 131075), (51000, 0), (51000, u64::MAX)] {
        let mut bad = second;
        bad.oauth_dcr_timeout_millis = timeout;
        bad.oauth_dcr_response_body_bytes = bytes;
        assert!(settings.upsert("default","default",&serde_json::from_value(serde_json::json!({"expected_revision":newer.revision,"execution_limits":bad})).unwrap()).await.is_err());
        assert_eq!(source.snapshot().await.unwrap().limits, second);
    }
    // Real metadata reuse is separately controlled from HTTP bounds and the
    // accepted registration. Cached metadata is fixture data, never Tool success.
    *provider.metadata_cache.write().await = Some(metadata_fixture());
    let requests_before_cache = egress.observations.lock().unwrap().len();
    assert_eq!(
        provider
            .discover_authorization_server(&scope)
            .await
            .unwrap()
            .registration_endpoint,
        "https://127.0.0.1:9/register"
    );
    assert_eq!(
        egress.observations.lock().unwrap().len(),
        requests_before_cache
    );
    assert!(unbound.discover_authorization_server(&scope).await.is_err());
    assert_eq!(
        egress.observations.lock().unwrap().len(),
        requests_before_cache
    );
    let mut revision = newer.revision;
    for (ttl, extra_requests) in [(0, 2), (900, 4), (u32::MAX, 6)] {
        let mut limits = second;
        limits.oauth_dcr_metadata_cache_ttl_secs = ttl;
        let saved = settings
            .upsert(
                "default",
                "default",
                &serde_json::from_value(
                    serde_json::json!({"expected_revision":revision,"execution_limits":limits}),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        revision = saved.revision;
        let acknowledged = source.snapshot().await.unwrap();
        assert_eq!(acknowledged.revision, revision);
        assert_eq!(acknowledged.limits, limits);
        let effective = settings
            .runtime_status()
            .await
            .unwrap()
            .unwrap()
            .execution_limits
            .unwrap();
        assert_eq!(effective.effective_revision, revision);
        assert_eq!(effective.limits.oauth_dcr_metadata_cache_ttl_secs, ttl);
        if ttl == u32::MAX {
            let mut old = metadata_fixture();
            old.ttl_secs = 1;
            *provider.metadata_cache.write().await = Some(old);
            // Actual monotonic age, with no synthetic clock or Instant addition.
            tokio::time::sleep(Duration::from_millis(1100)).await;
        }
        assert!(
            provider
                .discover_authorization_server(&scope)
                .await
                .is_err()
        );
        assert!(provider.metadata_cache.read().await.is_none());
        assert_eq!(
            egress.observations.lock().unwrap().len(),
            requests_before_cache + extra_requests
        );
        assert_eq!(
            manager
                .read_dcr_registration(&request.scope, request.flow_id)
                .await
                .unwrap()
                .unwrap()
                .action,
            after.action,
            "cache edits cannot renew or replay an accepted registration"
        );
    }
    settings.upsert("default","default",&serde_json::from_value(serde_json::json!({"expected_revision":revision,"execution_limits":original.execution_limits})).unwrap()).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_gate_recovers_preparation_after_flow_insert_failure_without_rebinding() {
    use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("dcr-gate-recovery", directory.path())
                .with_tenant_id("default")
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let source = runtime
        .services()
        .primitive_settings
        .as_ref()
        .unwrap()
        .clone();
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        runtime.services().secret_store.clone(),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
    );
    let egress = Arc::new(ObservedEgress {
        inner: Arc::new(services.host_http_egress(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
        )),
        observations: Mutex::new(vec![]),
        failures: Mutex::new(vec![]),
        pause_once: AtomicBool::new(false),
        entered: Notify::new(),
        released: Notify::new(),
    });
    let mut config = test_config();
    config.spec.resource = Some("https://127.0.0.1:9/mcp");
    let provider = OAuthDcrProvider::new(
        config,
        egress.clone(),
        runtime.services().secret_store.clone(),
        Arc::new(services.obligation_handler()),
    )
    .unwrap()
    .with_primitive_settings_source(Some(source));
    let auth = runtime.services().product_auth.as_ref().unwrap();
    let manager = auth.flow_manager();
    let flows = auth.flow_record_source().unwrap();
    let resource = sample_auth_scope().resource;
    let owner = resource.user_id;
    let scope = TurnScope::new(
        resource.tenant_id,
        resource.agent_id,
        resource.project_id,
        resource.thread_id.unwrap(),
    );

    for foreign_setup in [false, true] {
        let run = TurnRunId::new();
        let gate = AuthGateRef::new("native-preparation-before-flow").unwrap();
        // Build the expected stable gate identity to inspect its real encrypted
        // custody even while the actual PG flow publication is deliberately absent.
        let seed = serde_json::to_vec(&(
            "brassclaw-dcr-gate-original/1",
            scope.tenant_id.as_str(),
            owner.as_str(),
            &scope.agent_id,
            &scope.project_id,
            scope.thread_id.as_str(),
            run.to_string(),
            gate.as_str(),
            provider.spec.provider_id,
        ))
        .unwrap();
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, &seed);
        let flow_id = AuthFlowId::from_uuid(id);
        let mut auth_scope = auth_scope_for_blocked_turn(&scope, &owner);
        auth_scope.resource.invocation_id =
            InvocationId::from_uuid(uuid::Uuid::new_v5(&id, b"dcr-auth-invocation/1"));
        let before_requests = egress.observations.lock().unwrap().len();
        // This is issuer metadata data only; no successful HTTP/Tool reply is
        // synthesized. The actual POST below reaches the native host/network guard.
        *provider.metadata_cache.write().await = Some(metadata_fixture());
        let first = reject_flow_insert(rig.pool.as_ref(), flow_id, async {
            if foreign_setup {
                provider
                    .start_original_registration(
                        &manager,
                        DcrOriginalStart {
                            requirement_setup: None,
                            scope: auth_scope.clone(),
                            flow_id,
                            account_label: provider.account_label.clone(),
                            provider_scopes: provider.scopes.clone(),
                            continuation: AuthContinuationRef::SetupOnly,
                            update_binding: None,
                            deadline: OAuthSetupDeadline::Exact(
                                Utc::now() + ChronoDuration::seconds(120),
                            ),
                        },
                    )
                    .await
                    .map(|_| ())
            } else {
                provider
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .map(|_| ())
            }
        })
        .await;
        assert_eq!(first.unwrap_err(), AuthProductError::BackendUnavailable);
        assert!(
            manager
                .get_flow(&auth_scope, flow_id)
                .await
                .unwrap()
                .is_none()
        );
        let original = provider
            .retained_preparation_original(&auth_scope, flow_id)
            .await
            .unwrap()
            .expect("actual encrypted preparation survived failed flow publication");
        assert_eq!(egress.observations.lock().unwrap().len(), before_requests);

        if foreign_setup {
            assert_eq!(
                provider
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .unwrap_err(),
                AuthProductError::BackendConflict
            );
            assert!(
                manager
                    .get_flow(&auth_scope, flow_id)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(egress.observations.lock().unwrap().len(), before_requests);
        } else {
            let mut changed = provider.clone();
            changed.account_label = CredentialAccountLabel::new("later provider default").unwrap();
            changed.scopes = vec![ProviderScope::new("later-default-scope").unwrap()];
            let unbound = changed
                .clone()
                .with_primitive_settings_source(Some(Arc::new(
                    crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new(),
                )));
            assert_eq!(
                unbound
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .unwrap_err(),
                AuthProductError::BackendUnavailable
            );
            let prepared = manager
                .read_dcr_registration(&auth_scope, flow_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(prepared.action.original, original);
            assert_eq!(prepared.flow.expires_at, original.expires_at);
            assert!(matches!(
                prepared.action.outcome,
                brassclaw_auth::DcrRegistrationOutcome::Prepared
            ));
            assert_eq!(egress.observations.lock().unwrap().len(), before_requests);
            assert!(
                changed
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .is_err()
            );
            let attempted = manager
                .read_dcr_registration(&auth_scope, flow_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(attempted.action.original, original);
            assert!(matches!(
                attempted.action.outcome,
                brassclaw_auth::DcrRegistrationOutcome::Unknown { .. }
            ));
            assert_eq!(
                egress.observations.lock().unwrap().len(),
                before_requests + 1
            );
            assert_eq!(
                egress.observations.lock().unwrap().last().unwrap().method,
                NetworkMethod::Post
            );
            assert_eq!(
                egress.failures.lock().unwrap().last(),
                Some(&"policy_denied")
            );
            assert!(
                unbound
                    .challenge_for_blocked_gate(&manager, &flows, &scope, &owner, run, &gate)
                    .await
                    .is_err()
            );
            assert_eq!(
                manager
                    .read_dcr_registration(&auth_scope, flow_id)
                    .await
                    .unwrap()
                    .unwrap()
                    .action,
                attempted.action
            );
            assert_eq!(
                egress.observations.lock().unwrap().len(),
                before_requests + 1
            );
        }
        assert_eq!(
            provider
                .retained_preparation_original(&auth_scope, flow_id)
                .await
                .unwrap(),
            Some(original)
        );
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_gate_oauth_lifetimes_keep_concurrent_originals_and_observe_acknowledged_settings() {
    use crate::input::OAuthClientConfig;
    use crate::oauth_gate::{GoogleOAuthGateProvider, GoogleSetupRequest};
    use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};

    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("gate-oauth-lifetimes", directory.path())
                .with_tenant_id("default")
                .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let settings = runtime.webui_monty_settings_store();
    let original = settings.get("default", "default").await.unwrap();
    let source = runtime
        .services()
        .primitive_settings
        .as_ref()
        .unwrap()
        .clone();
    let secrets = runtime.services().secret_store.clone();
    let manager = runtime
        .services()
        .product_auth
        .as_ref()
        .unwrap()
        .flow_manager();
    let mut limits = original.execution_limits;
    limits.turn_gate_oauth_setup_ttl_secs = 30;
    let first_revision = settings.upsert("default", "default", &serde_json::from_value(
        serde_json::json!({"expected_revision":original.revision,"execution_limits":limits})
    ).unwrap()).await.unwrap();

    let google = GoogleOAuthGateProvider::new(
        OAuthClientConfig::new(
            "controlled-gate-client",
            "https://app.example/api/reborn/product-auth/oauth/google/callback",
            Some("controlled-gate-secret".into()),
        )
        .unwrap(),
        secrets.clone(),
    )
    .with_primitive_settings_source(Some(source.clone()));
    let google_request = GoogleSetupRequest {
        scope: sample_auth_scope(),
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("native google gate lifetime").unwrap(),
        requested_scopes: vec![
            ProviderScope::new(brassclaw_auth::GOOGLE_CALENDAR_READONLY_SCOPE).unwrap(),
        ],
        continuation: AuthContinuationRef::TurnGateResume {
            turn_run_ref: TurnRunRef::new(TurnRunId::new().to_string()).unwrap(),
            gate_ref: AuthGateRef::new("native-google-lifetime").unwrap(),
        },
        requirement_setup: None,
        update_binding: None,
        update_account_snapshot: None,
        deadline: OAuthSetupDeadline::GateDefault,
    };
    let before = Utc::now();
    let (a, b) = tokio::join!(
        google.prepare_setup_original(google_request.clone()),
        google.prepare_setup_original(google_request.clone())
    );
    let google_original = a.unwrap();
    google_original.verify(&b.unwrap()).unwrap();
    assert!(google_original.definition.expires_at >= before + ChronoDuration::seconds(30));
    assert!(google_original.definition.expires_at <= Utc::now() + ChronoDuration::seconds(30));
    let published = google
        .restore_setup_original(&manager, google_original.clone(), None)
        .await
        .unwrap();
    assert_eq!(
        published.flow.expires_at,
        google_original.definition.expires_at
    );

    // The existing real host/network guard denies the test endpoint. Metadata
    // is validated fixture data; no successful issuer/Tool reply is invented.
    let services = BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        secrets.clone(),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
    );
    let egress = Arc::new(ObservedEgress {
        inner: Arc::new(services.host_http_egress(
            brassclaw_network::PolicyNetworkHttpEgress::new(
                brassclaw_network::ReqwestNetworkTransport::default(),
            ),
        )),
        observations: Mutex::new(vec![]),
        failures: Mutex::new(vec![]),
        pause_once: AtomicBool::new(false),
        entered: Notify::new(),
        released: Notify::new(),
    });
    let mut config = test_config();
    config.spec.resource = Some("https://127.0.0.1:9/mcp");
    let dcr = OAuthDcrProvider::new(
        config,
        egress.clone(),
        secrets,
        Arc::new(services.obligation_handler()),
    )
    .unwrap()
    .with_primitive_settings_source(Some(source));
    *dcr.metadata_cache.write().await = Some(metadata_fixture());
    let dcr_request = DcrOriginalStart {
        requirement_setup: None,
        scope: sample_auth_scope(),
        flow_id: AuthFlowId::new(),
        account_label: CredentialAccountLabel::new("native dcr gate lifetime").unwrap(),
        provider_scopes: vec![],
        continuation: AuthContinuationRef::TurnGateResume {
            turn_run_ref: TurnRunRef::new(TurnRunId::new().to_string()).unwrap(),
            gate_ref: AuthGateRef::new("native-dcr-lifetime").unwrap(),
        },
        update_binding: None,
        deadline: OAuthSetupDeadline::GateDefault,
    };
    let before = Utc::now();
    let rejected = reject_flow_insert(rig.pool.as_ref(), dcr_request.flow_id, async {
        let (a, b) = tokio::join!(
            dcr.start_original_registration(&manager, dcr_request.clone()),
            dcr.start_original_registration(&manager, dcr_request.clone())
        );
        assert_eq!(a.unwrap_err(), AuthProductError::BackendUnavailable);
        assert_eq!(b.unwrap_err(), AuthProductError::BackendUnavailable);
        Err(AuthProductError::BackendUnavailable)
    })
    .await;
    assert_eq!(rejected.unwrap_err(), AuthProductError::BackendUnavailable);
    let dcr_original = dcr
        .retained_preparation_original(&dcr_request.scope, dcr_request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(dcr_original.expires_at >= before + ChronoDuration::seconds(30));
    assert!(dcr_original.expires_at <= Utc::now() + ChronoDuration::seconds(30));
    assert!(egress.observations.lock().unwrap().is_empty());

    limits.turn_gate_oauth_setup_ttl_secs = 1;
    let second_revision=settings.upsert("default","default",&serde_json::from_value(
        serde_json::json!({"expected_revision":first_revision.revision,"execution_limits":limits})
    ).unwrap()).await.unwrap();
    let effective = settings
        .runtime_status()
        .await
        .unwrap()
        .unwrap()
        .execution_limits
        .unwrap();
    assert_eq!(effective.effective_revision, second_revision.revision);
    assert_eq!(effective.limits.turn_gate_oauth_setup_ttl_secs, 1);
    let unbound_source: Arc<dyn HostPrimitiveSettingsSource> =
        Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
    let unbound_google = google
        .clone()
        .with_primitive_settings_source(Some(unbound_source.clone()));
    google_original
        .verify(
            &unbound_google
                .prepare_setup_original(google_request.clone())
                .await
                .unwrap(),
        )
        .unwrap();
    let unbound_dcr = dcr
        .clone()
        .with_primitive_settings_source(Some(unbound_source));
    assert_eq!(
        unbound_dcr
            .start_original_registration(&manager, dcr_request.clone())
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    let restored = manager
        .read_dcr_registration(&dcr_request.scope, dcr_request.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.action.original, dcr_original);
    assert_eq!(restored.flow.expires_at, dcr_original.expires_at);
    assert!(matches!(
        restored.action.outcome,
        brassclaw_auth::DcrRegistrationOutcome::Prepared
    ));
    assert!(
        egress.observations.lock().unwrap().is_empty(),
        "unavailable HTTP policy must not spend the recovered original claim"
    );

    let mut next_google = google_request.clone();
    next_google.flow_id = AuthFlowId::new();
    assert_eq!(
        unbound_google
            .prepare_setup_original(next_google.clone())
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    assert!(
        unbound_google
            .retained_setup_original(&next_google.scope, next_google.flow_id)
            .await
            .unwrap()
            .is_none()
    );
    let before = Utc::now();
    let reduced_google_request = next_google.clone();
    let reduced = google
        .prepare_setup_original(next_google.clone())
        .await
        .unwrap();
    assert!(reduced.definition.expires_at >= before + ChronoDuration::seconds(1));
    assert!(reduced.definition.expires_at <= Utc::now() + ChronoDuration::seconds(1));
    let mut next_dcr = dcr_request.clone();
    next_dcr.flow_id = AuthFlowId::new();
    assert_eq!(
        unbound_dcr
            .start_original_registration(&manager, next_dcr.clone())
            .await
            .unwrap_err(),
        AuthProductError::BackendUnavailable
    );
    assert!(
        unbound_dcr
            .retained_preparation_original(&next_dcr.scope, next_dcr.flow_id)
            .await
            .unwrap()
            .is_none()
    );
    let before = Utc::now();
    let reduced_outcome = reject_flow_insert(rig.pool.as_ref(), next_dcr.flow_id, async {
        match dcr
            .start_original_registration(&manager, next_dcr.clone())
            .await
        {
            Ok(flow) => {
                assert_eq!(flow.status, brassclaw_auth::AuthFlowStatus::Expired);
                assert!(flow.challenge.is_none());
                assert!(flow.credential_account_id.is_none());
                Ok(())
            }
            Err(error) => Err(error),
        }
    })
    .await;
    assert!(
        matches!(
            reduced_outcome,
            Ok(())
                | Err(AuthProductError::BackendUnavailable | AuthProductError::UnknownOrExpiredFlow)
        ),
        "one-second preparation can expire before flow insertion: {reduced_outcome:?}"
    );
    // Expiry can publish a retired flow before the rejected pending insert and
    // close its private preparation. Read the same original from its Auth owner.
    let reduced_dcr = match manager
        .read_dcr_registration(&next_dcr.scope, next_dcr.flow_id)
        .await
        .unwrap()
    {
        Some(state) => state.action.original,
        None => dcr
            .retained_preparation_original(&next_dcr.scope, next_dcr.flow_id)
            .await
            .unwrap()
            .unwrap(),
    };
    assert!(reduced_dcr.expires_at >= before + ChronoDuration::seconds(1));
    assert!(reduced_dcr.expires_at <= Utc::now() + ChronoDuration::seconds(1));

    // Host default mode is never an escape from exact standalone input checks.
    let mut invalid_google = google_request.clone();
    invalid_google.continuation = AuthContinuationRef::SetupOnly;
    assert_eq!(
        google
            .prepare_setup_original(invalid_google)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut invalid_dcr = dcr_request.clone();
    invalid_dcr.continuation = AuthContinuationRef::SetupOnly;
    assert_eq!(
        dcr.start_original_registration(&manager, invalid_dcr)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut changed_google = google_request.clone();
    changed_google.account_label = CredentialAccountLabel::new("changed gate account").unwrap();
    assert_eq!(
        google
            .prepare_setup_original(changed_google)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut changed_dcr = dcr_request.clone();
    changed_dcr.account_label = CredentialAccountLabel::new("changed gate account").unwrap();
    assert_eq!(
        dcr.start_original_registration(&manager, changed_dcr)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );

    limits.turn_gate_oauth_setup_ttl_secs = u32::MAX;
    let maximum_revision=settings.upsert("default","default",&serde_json::from_value(
        serde_json::json!({"expected_revision":second_revision.revision,"execution_limits":limits})
    ).unwrap()).await.unwrap();
    next_google.flow_id = AuthFlowId::new();
    let before = Utc::now();
    let maximum = google.prepare_setup_original(next_google).await.unwrap();
    assert!(maximum.definition.expires_at >= before + ChronoDuration::seconds(i64::from(u32::MAX)));
    assert!(
        maximum.definition.expires_at <= Utc::now() + ChronoDuration::seconds(i64::from(u32::MAX))
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    // An expired original keeps its deadline after the large live increase.
    google_original
        .verify(&google.prepare_setup_original(google_request).await.unwrap())
        .unwrap();
    reduced
        .verify(
            &google
                .prepare_setup_original(reduced_google_request)
                .await
                .unwrap(),
        )
        .unwrap();
    let retired = google
        .restore_setup_original(&manager, reduced.clone(), None)
        .await
        .unwrap();
    assert_eq!(retired.flow.status, brassclaw_auth::AuthFlowStatus::Expired);
    assert_eq!(retired.flow.expires_at, reduced.definition.expires_at);
    let retired_dcr = dcr
        .start_original_registration(&manager, next_dcr.clone())
        .await
        .unwrap();
    assert_eq!(retired_dcr.status, brassclaw_auth::AuthFlowStatus::Expired);
    assert_eq!(retired_dcr.expires_at, reduced_dcr.expires_at);
    assert_eq!(
        retired_dcr.error,
        Some(brassclaw_auth::AuthErrorCode::UnknownOrExpiredFlow)
    );
    assert!(retired_dcr.challenge.is_none());
    assert!(retired_dcr.credential_account_id.is_none());
    reduced_dcr
        .verify(
            &manager
                .read_dcr_registration(&next_dcr.scope, next_dcr.flow_id)
                .await
                .unwrap()
                .unwrap()
                .action
                .original,
        )
        .unwrap();
    assert!(egress.observations.lock().unwrap().is_empty());
    settings.upsert("default","default",&serde_json::from_value(
        serde_json::json!({"expected_revision":maximum_revision.revision,"execution_limits":original.execution_limits})
    ).unwrap()).await.unwrap();
}
