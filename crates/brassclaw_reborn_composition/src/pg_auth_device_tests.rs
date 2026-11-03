//! Real PostgreSQL + actual local HTTP issuer fixtures. The explicit test-only
//! adapter routes the already bounded host request to local HTTP; it does NOT
//! establish positive production HTTPS/private-IP-policy acceptance.
use super::*;
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use brassclaw_auth::{AuthSurface, NativeDeviceOAuthClient};
use brassclaw_host_api::{
    InvocationId, ResourceScope, RuntimeHttpEgress, RuntimeHttpEgressError,
    RuntimeHttpEgressRequest, RuntimeHttpEgressResponse, TenantId, UserId,
};
use brassclaw_product_workflow::MontyVmSettingsStore;
use secrecy::ExposeSecret;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum IssuerMode {
    Success,
    Throttle,
    DisconnectToken,
    LargeStart,
    DelayedStart,
    HoldStart,
    NoExpiry,
    RefreshOmit,
    RefreshInvalid,
    RefreshDisconnect,
}
struct IssuerState {
    mode: IssuerMode,
    calls: AtomicUsize,
    token_calls: AtomicUsize,
    received: Mutex<Vec<(String, Vec<u8>)>>,
    verifier: String,
    entered: tokio::sync::Notify,
    released: tokio::sync::Notify,
}
async fn usercode(State(state): State<Arc<IssuerState>>, body: Bytes) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    state
        .received
        .lock()
        .unwrap()
        .push(("usercode".into(), body.to_vec()));
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"client_id":"public-native-client"})
    );
    if state.mode == IssuerMode::HoldStart {
        state.entered.notify_one();
        state.released.notified().await;
    }
    if state.mode == IssuerMode::DelayedStart {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    let mut value = serde_json::json!({"device_auth_id":"device-http-original","user_code":"USER-CODE-HTTP",
        "verification_uri":"https://native-device.example/codex/device","interval":"5","expires_in":120});
    if state.mode == IssuerMode::NoExpiry {
        value.as_object_mut().unwrap().remove("expires_in");
    }
    if state.mode == IssuerMode::LargeStart {
        value["padding"] = serde_json::json!("x".repeat(4096));
    }
    axum::Json(value).into_response()
}
async fn poll(State(state): State<Arc<IssuerState>>, body: Bytes) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    state
        .received
        .lock()
        .unwrap()
        .push(("poll".into(), body.to_vec()));
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"device_auth_id":"device-http-original","user_code":"USER-CODE-HTTP"})
    );
    if state.mode == IssuerMode::Throttle {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "120")],
            "throttled",
        )
            .into_response();
    }
    let verifier = brassclaw_auth::PkceVerifierSecret::new(state.verifier.clone().into()).unwrap();
    axum::Json(serde_json::json!({"authorization_code":"authorization-http-original",
        "code_verifier":state.verifier,"code_challenge":brassclaw_auth::pkce_s256_challenge(&verifier).as_str()})).into_response()
}
async fn token(State(state): State<Arc<IssuerState>>, headers: HeaderMap, body: Bytes) -> Response {
    state.calls.fetch_add(1, Ordering::SeqCst);
    state.token_calls.fetch_add(1, Ordering::SeqCst);
    state
        .received
        .lock()
        .unwrap()
        .push(("token".into(), body.to_vec()));
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/x-www-form-urlencoded"
    );
    let values: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(&body).into_owned().collect();
    if values.get("grant_type").map(String::as_str) == Some("refresh_token") {
        assert_eq!(values.get("client_id").unwrap(), "public-native-client");
        assert_eq!(
            values.get("refresh_token").unwrap(),
            "refresh-http-original"
        );
        assert!(!values.contains_key("code"));
        if state.mode == IssuerMode::RefreshDisconnect {
            let stream = futures::stream::once(async {
                Err::<Bytes, std::io::Error>(std::io::Error::other("lost refresh return"))
            });
            return Response::builder()
                .status(StatusCode::OK)
                .body(Body::from_stream(stream))
                .unwrap();
        }
        if state.mode == IssuerMode::RefreshInvalid {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({"error":"invalid_grant"})),
            )
                .into_response();
        }
        let mut response = serde_json::json!({"access_token":"access-http-refreshed","expires_in":3600,"token_type":"Bearer"});
        if state.mode != IssuerMode::RefreshOmit {
            response["refresh_token"] = serde_json::json!("refresh-http-original");
        }
        return axum::Json(response).into_response();
    }
    assert_eq!(values.get("grant_type").unwrap(), "authorization_code");
    assert_eq!(values.get("code").unwrap(), "authorization-http-original");
    assert_eq!(values.get("code_verifier").unwrap(), &state.verifier);
    assert_eq!(values.get("client_id").unwrap(), "public-native-client");
    assert_eq!(
        values.get("redirect_uri").unwrap(),
        "https://native-device.example/deviceauth/callback"
    );
    if state.mode == IssuerMode::DisconnectToken {
        // Real HTTP body failure after issuer received the one-time code.
        let stream = futures::stream::once(async {
            Err::<Bytes, std::io::Error>(std::io::Error::other("issuer dropped response"))
        });
        return Response::builder()
            .status(StatusCode::OK)
            .body(Body::from_stream(stream))
            .unwrap();
    }
    axum::Json(serde_json::json!({"access_token":"access-http-original","refresh_token":"refresh-http-original",
        "expires_in":3600,"token_type":"Bearer"})).into_response()
}
struct Issuer {
    endpoint: String,
    state: Arc<IssuerState>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Issuer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Issuer {
    async fn start(mode: IssuerMode) -> Self {
        let state = Arc::new(IssuerState {
            mode,
            calls: AtomicUsize::new(0),
            token_calls: AtomicUsize::new(0),
            received: Mutex::new(Vec::new()),
            verifier: brassclaw_common::pkce::generate_code_verifier(),
            entered: tokio::sync::Notify::new(),
            released: tokio::sync::Notify::new(),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/api/accounts/deviceauth/usercode", post(usercode))
            .route("/api/accounts/deviceauth/token", post(poll))
            .route("/oauth/token", post(token))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            endpoint,
            state,
            task,
        }
    }
}
/// Actual HTTP adapter at the existing RuntimeHttpEgress trait boundary. It
/// validates and applies the real captured timeout/body/header configuration;
/// only the issuer destination is explicitly routed to the local test server.
#[derive(Debug)]
struct LocalIssuerHttp {
    endpoint: String,
    client: reqwest::Client,
}
fn network_error(reason: &str, request_bytes: u64, response_bytes: u64) -> RuntimeHttpEgressError {
    RuntimeHttpEgressError::Network {
        reason: reason.into(),
        request_bytes,
        response_bytes,
    }
}
impl LocalIssuerHttp {
    async fn exchange(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<NativeDeviceNetworkOriginal, RuntimeHttpEgressError> {
        assert_eq!(request.method, brassclaw_host_api::NetworkMethod::Post);
        let parsed = url::Url::parse(&request.url).unwrap();
        assert_eq!(parsed.scheme(), "https");
        assert_eq!(parsed.host_str(), Some("native-device.example"));
        assert!(request.network_policy.deny_private_ip_ranges);
        assert!(request.credential_injections.is_empty());
        assert!(request.save_body_to.is_none());
        let timeout = request.timeout_ms.expect("actual host timeout");
        assert!(timeout > 0);
        let limit = request.response_body_limit.expect("actual host body bound");
        assert_eq!(request.network_policy.max_egress_bytes, Some(limit));
        let mut builder = self
            .client
            .post(format!("{}{}", self.endpoint, parsed.path()))
            .timeout(std::time::Duration::from_millis(u64::from(timeout)));
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let request_bytes = request.body.len() as u64;
        let mut response = builder
            .body(request.body.clone())
            .send()
            .await
            .map_err(|_| network_error("actual HTTP request failed", request_bytes, 0))?;
        let status = response.status().as_u16();
        let resolved_ip = response.remote_addr().map(|address| address.ip());
        let headers = response
            .headers()
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_str().unwrap().to_string()))
            .collect();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| {
            network_error(
                "actual HTTP response incomplete",
                request_bytes,
                body.len() as u64,
            )
        })? {
            if (body.len() as u64)
                .checked_add(chunk.len() as u64)
                .is_none_or(|size| size > limit)
            {
                return Err(RuntimeHttpEgressError::Response {
                    reason: brassclaw_host_api::RUNTIME_HTTP_REASON_RESPONSE_BODY_LIMIT_EXCEEDED
                        .into(),
                    request_bytes,
                    response_bytes: body.len() as u64,
                });
            }
            body.extend_from_slice(&chunk);
        }
        let response_bytes = body.len() as u64;
        Ok(NativeDeviceNetworkOriginal {
            status,
            headers,
            body,
            usage: NativeDeviceNetworkUsage {
                request_bytes,
                response_bytes,
                resolved_ip,
            },
        })
    }
}
fn private_fixture_response(raw: &NativeDeviceNetworkOriginal) -> RuntimeHttpEgressResponse {
    // Test adapter has no generic raw return. Product sanitizer behavior is
    // exercised separately through the genuine HostHttpEgressService pipeline.
    RuntimeHttpEgressResponse {
        status: raw.status,
        headers: Vec::new(),
        body: Vec::new(),
        saved_body: None,
        request_bytes: raw.usage.request_bytes,
        response_bytes: raw.usage.response_bytes,
        redaction_applied: true,
    }
}
#[async_trait]
impl RuntimeHttpEgress for LocalIssuerHttp {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        self.exchange(request)
            .await
            .map(|raw| private_fixture_response(&raw))
    }
}
#[async_trait]
impl brassclaw_host_runtime::HostHttpOriginalEgress for LocalIssuerHttp {
    async fn execute_with_original_custody<'a>(
        &'a self,
        request: RuntimeHttpEgressRequest,
        mut custody: Box<dyn brassclaw_host_runtime::HostHttpOriginalCustody + 'a>,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        custody
            .authenticate_request(&request)
            .await
            .map_err(|_| network_error("actual claim preflight failed", 0, 0))?;
        let raw = self.exchange(request).await?;
        let material = SecretString::from(serde_json::to_string(&raw).unwrap());
        custody.retain(material).await.map_err(|_| {
            network_error(
                "actual original persistence failed",
                raw.usage.request_bytes,
                raw.usage.response_bytes,
            )
        })?;
        Ok(private_fixture_response(&raw))
    }
}
struct ObservedKernelEgress {
    inner: Arc<dyn RuntimeHttpEgress>,
    original: Arc<dyn brassclaw_host_runtime::HostHttpOriginalEgress>,
    errors: Arc<Mutex<Vec<brassclaw_host_api::RuntimeHttpEgressReasonCode>>>,
}
impl std::fmt::Debug for ObservedKernelEgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ObservedKernelEgress")
    }
}
#[async_trait]
impl RuntimeHttpEgress for ObservedKernelEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        let result = self.inner.execute(request).await;
        if let Err(error) = &result {
            self.errors.lock().unwrap().push(error.reason_code());
        }
        result
    }
}
#[async_trait]
impl brassclaw_host_runtime::HostHttpOriginalEgress for ObservedKernelEgress {
    async fn execute_with_original_custody<'a>(
        &'a self,
        request: RuntimeHttpEgressRequest,
        custody: Box<dyn brassclaw_host_runtime::HostHttpOriginalCustody + 'a>,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        let result = self
            .original
            .execute_with_original_custody(request, custody)
            .await;
        if let Err(error) = &result {
            self.errors.lock().unwrap().push(error.reason_code());
        }
        result
    }
}
fn scope() -> AuthProductScope {
    AuthProductScope::new(
        ResourceScope {
            tenant_id: TenantId::new("native-device-test").unwrap(),
            user_id: UserId::new("actual-test-installer").unwrap(),
            agent_id: None,
            project_id: None,
            thread_id: None,
            invocation_id: InvocationId::new(),
        },
        AuthSurface::SetupAdmin,
    )
}
fn service(pool: &Arc<brassclaw_pg::PgPool>) -> PgAuthProductServices {
    PgAuthProductServices::new(
        pool.clone(),
        Arc::new(
            PgSecretStore::new(
                pool.as_ref().clone(),
                "native-device-actual-encryption-master".into(),
                "native-device-test",
            )
            .unwrap(),
        ),
    )
}
async fn transport(
    pool: &Arc<brassclaw_pg::PgPool>,
    auth: &PgAuthProductServices,
    issuer: &Issuer,
    timeout: Option<u32>,
    body_limit: Option<u64>,
) -> crate::oauth_provider_client::HostOAuthProviderClient {
    let settings = Arc::new(crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        pool.clone(),
        "native-device-test",
        "default",
    ));
    let old = settings.get("default", "default").await.unwrap();
    let mut limits = old.execution_limits;
    if let Some(timeout) = timeout {
        limits.oauth_token_timeout_millis = timeout;
    }
    if let Some(limit) = body_limit {
        limits.oauth_token_response_body_bytes = limit;
    }
    let committed = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                serde_json::json!({"expected_revision":old.revision,"execution_limits":limits}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let source = Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
    source
        .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
            revision: committed.revision,
            limits: committed.execution_limits,
        })
        .unwrap();
    source.bind(settings).await.unwrap();
    endpoint_transport(auth, &issuer.endpoint, source)
}
fn endpoint_transport(
    auth: &PgAuthProductServices,
    endpoint: &str,
    source: Arc<dyn brassclaw_host_runtime::HostPrimitiveSettingsSource>,
) -> crate::oauth_provider_client::HostOAuthProviderClient {
    use crate::oauth_provider_client::{
        ExchangeScopePolicy, HostOAuthProviderClient, HostOAuthProviderSpec,
    };
    let obligations = brassclaw_host_runtime::BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        auth.secret_store.clone(),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
    );
    let http = Arc::new(LocalIssuerHttp {
        endpoint: endpoint.to_string(),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .unwrap(),
    });
    HostOAuthProviderClient::new(
        HostOAuthProviderSpec {
            provider_id: "openai_codex",
            capability_id: "oauth.native-device-test",
            token_endpoint: "https://native-device.example/oauth/token",
            secret_handle_prefix: "codex-native",
            resource: None,
            exchange_scope_policy: ExchangeScopePolicy::FallbackToRequested,
        },
        http.clone(),
        auth.secret_store.clone(),
        Arc::new(obligations.obligation_handler()),
        brassclaw_auth::OAuthClientId::new("public-native-client").unwrap(),
        brassclaw_auth::OAuthRedirectUri::new("https://native-device.example/deviceauth/callback")
            .unwrap(),
    )
    .unwrap()
    .with_primitive_settings_source(Some(source))
    .with_original_egress(Some(http))
}
async fn prepare(
    auth: &PgAuthProductServices,
    transport: &crate::oauth_provider_client::HostOAuthProviderClient,
    seconds: i64,
) -> NativeDeviceLoginAction {
    let scope = scope();
    let id = AuthFlowId::new();
    let now = Utc::now();
    let definition = NativeDeviceLoginDefinition {
        scope: scope.clone(),
        flow_id: id,
        account_id: CredentialAccountId::new(),
        provider: AuthProviderId::new("openai_codex").unwrap(),
        account_label: CredentialAccountLabel::new("Native operator login").unwrap(),
        client: NativeDeviceOAuthClient {
            client_id: "public-native-client".into(),
            auth_endpoint: "https://native-device.example".into(),
            refresh_endpoint: "https://native-device.example/oauth/token".into(),
            api_base_url: "https://native-device.example/backend-api".into(),
        },
        destinations: transport
            .native_device_destinations(id, scope.resource.invocation_id)
            .unwrap(),
        created_at: now,
        expires_at: now + chrono::Duration::seconds(seconds),
    };
    auth.prepare_native_device(
        definition,
        "actual encrypted original native setup context".into(),
    )
    .await
    .unwrap()
}
async fn step(
    auth: &PgAuthProductServices,
    transport: &crate::oauth_provider_client::HostOAuthProviderClient,
    original: &NativeDeviceLoginOriginal,
) -> NativeDeviceLoginAction {
    auth.run_native_device_once(transport, original, uuid::Uuid::new_v4())
        .await
        .unwrap()
}
async fn wait_until_due(action: &NativeDeviceLoginAction) {
    let Some(
        NativeDeviceOutcome::Challenge { next_poll_at, .. }
        | NativeDeviceOutcome::Pending { next_poll_at, .. },
    ) = action.requests.last().map(|r| &r.outcome)
    else {
        panic!("actual known pending response");
    };
    let remaining = next_poll_at
        .signed_duration_since(Utc::now())
        .to_std()
        .unwrap_or_default();
    // Actual protocol interval, not a concurrency scheduling guess.
    tokio::time::sleep(remaining).await;
}

#[tokio::test]
async fn native_device_http_originals_restart_and_ready_keep_exact_scope_client_expiry_and_raw() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let initial = prepare(&auth, &transport, 60).await;
    let original = initial.original;
    let challenge = step(&auth, &transport, &original).await;
    assert!(matches!(
        challenge.requests[0].outcome,
        NativeDeviceOutcome::Challenge { .. }
    ));
    let display = auth
        .native_device_challenge(&original)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(display.user_code.expose_secret(), "USER-CODE-HTTP");
    assert_eq!(
        display.verification_uri,
        "https://native-device.example/codex/device"
    );
    assert_eq!(display.expires_at, challenge.expiry());
    let restarted = service(&rig.pool);
    assert_eq!(
        restarted
            .read_native_device_state(&original.definition.scope, original.definition.flow_id)
            .await
            .unwrap()
            .unwrap()
            .action,
        challenge
    );
    let early = step(&restarted, &transport, &original).await;
    assert_eq!(early, challenge);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    wait_until_due(&challenge).await;
    let authorized = step(&restarted, &transport, &original).await;
    assert!(matches!(
        authorized.requests.last().unwrap().outcome,
        NativeDeviceOutcome::Authorized { .. }
    ));
    let returned = step(&restarted, &transport, &original).await;
    assert!(matches!(
        returned.requests.last().unwrap().outcome,
        NativeDeviceOutcome::Tokens { .. }
    ));
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 1);
    // Caller loses/drops the actual successful result: a fresh owner reads it
    // back and never automatically repeats the consumed authorization code.
    let reopened = service(&rig.pool);
    assert_eq!(step(&reopened, &transport, &original).await, returned);
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 1);
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    let mut ready = reopened
        .native_device_ready_in_transaction(&tx, &original)
        .await
        .unwrap();
    assert_eq!(
        ready.request.destinations.as_ref(),
        Some(&original.definition.destinations)
    );
    assert_eq!(
        ready.access_expires_at,
        ready.request_started_at + chrono::Duration::seconds(3600)
    );
    assert!(ready.request.authorization_code_hash.is_some());
    assert!(ready.request.pkce_verifier_hash.is_some());
    let exact_result = ready.response.reference.clone();
    ready.response.reference = "copied-but-wrong-result-reference".into();
    assert!(
        reopened
            .native_device_tokens_in_transaction(&tx, &ready)
            .await
            .is_err()
    );
    ready.response.reference = exact_result;
    let tokens = reopened
        .native_device_tokens_in_transaction(&tx, &ready)
        .await
        .unwrap();
    assert_eq!(tokens.access_token.expose_secret(), "access-http-original");
    assert_eq!(
        tokens.refresh_token.expose_secret(),
        "refresh-http-original"
    );
    assert_eq!(tokens.expires_at, ready.access_expires_at);
    tx.rollback().await.unwrap();
    let state = reopened
        .read_native_device_state(&original.definition.scope, original.definition.flow_id)
        .await
        .unwrap()
        .unwrap();
    let public = serde_json::to_string(&state.flow).unwrap();
    let custody = serde_json::to_string(&state.action).unwrap();
    for secret in [
        "USER-CODE-HTTP",
        "device-http-original",
        "authorization-http-original",
        &issuer.state.verifier,
        "access-http-original",
        "refresh-http-original",
    ] {
        assert!(!public.contains(secret));
        assert!(!custody.contains(secret));
    }
    assert!(state.flow.opaque_state_hash.is_none());
    assert!(state.flow.credential_account_id.is_none());
    assert!(
        reopened
            .secret_store
            .metadata(
                &original.definition.scope.resource,
                &original.definition.destinations.access_secret
            )
            .await
            .unwrap()
            .is_none()
    );
    let accounts: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT count(*) FROM brassclaw_product_auth_accounts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        accounts, 0,
        "Root alone owns atomic account/head/provider publication"
    );
    // Controlled publisher clock, derived from the actual original deadline:
    // no rewritten timestamps and no sleep until expiry. Already issued tokens
    // remain publishable; no new issuer action or changed expiry is involved.
    let late = original.definition.expires_at + chrono::Duration::seconds(1);
    assert!(late < ready.access_expires_at);
    let published = CredentialAccount {
        id: original.definition.account_id,
        scope: original.definition.scope.clone(),
        provider: original.definition.provider.clone(),
        label: original.definition.account_label.clone(),
        status: brassclaw_auth::CredentialAccountStatus::Configured,
        ownership: brassclaw_auth::CredentialOwnership::System,
        owner_extension: None,
        granted_extensions: Vec::new(),
        access_secret: Some(original.definition.destinations.access_secret.clone()),
        access_expires_at: Some(ready.access_expires_at),
        refresh_secret: Some(original.definition.destinations.refresh_secret.clone()),
        scopes: Vec::new(),
        created_at: late,
        updated_at: late,
    };
    // Fixture account only: this is flow-transaction acceptance, NOT Root's full
    // account/head/provider/role publication or credential availability proof.
    let tx = connection.transaction().await.unwrap();
    let late_ready = reopened
        .native_device_ready_at(&tx, &original, late)
        .await
        .unwrap();
    assert_eq!(late_ready.response, ready.response);
    super::super::callback::save_account(&tx, &published, None)
        .await
        .unwrap();
    let completed = reopened
        .finish_native_device_at(&tx, &late_ready, &published, late)
        .await
        .unwrap();
    assert_eq!(completed.status, AuthFlowStatus::Completed);
    tx.rollback().await.unwrap();
    assert_eq!(
        reopened
            .read_native_device_state(&original.definition.scope, original.definition.flow_id)
            .await
            .unwrap()
            .unwrap()
            .flow
            .status,
        AuthFlowStatus::Completing
    );
    let count: i64 = connection
        .query_one(
            "SELECT count(*) FROM brassclaw_product_auth_accounts WHERE id=$1",
            &[&published.id.to_string()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    let tx = connection.transaction().await.unwrap();
    let late_ready = reopened
        .native_device_ready_at(&tx, &original, late)
        .await
        .unwrap();
    super::super::callback::save_account(&tx, &published, None)
        .await
        .unwrap();
    reopened
        .finish_native_device_at(&tx, &late_ready, &published, late)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let stored = reopened
        .read_native_device_state(&original.definition.scope, original.definition.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.flow.status, AuthFlowStatus::Completed);
    assert_eq!(
        stored.action.published_account,
        Some(published.snapshot_ref().unwrap())
    );
    assert_eq!(stored.flow.expires_at, returned.expiry());
    assert_eq!(step(&reopened, &transport, &original).await, stored.action);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 3);
    // Original setup remains recoverable even after completion, without issuer I/O.
    let tx = connection.transaction().await.unwrap();
    let context = reopened
        .native_device_preparation_in_transaction(&tx, &original)
        .await
        .unwrap();
    assert_eq!(
        context.expose_secret(),
        "actual encrypted original native setup context"
    );
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn native_device_independent_claims_commit_before_http_and_never_rebuild_missing_submitted_history()
 {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let first = service(&rig.pool);
    let second = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let transport = transport(&rig.pool, &first, &issuer, None, None).await;
    let original = prepare(&first, &transport, 60).await.original;
    let (left, right) = tokio::join!(
        first.claim_native_device(&original, uuid::Uuid::new_v4()),
        second.claim_native_device(&original, uuid::Uuid::new_v4())
    );
    let dispatch = match (left.unwrap(), right.unwrap()) {
        (NativeDeviceAdmission::Dispatch(dispatch), NativeDeviceAdmission::Original(_))
        | (NativeDeviceAdmission::Original(_), NativeDeviceAdmission::Dispatch(dispatch)) => {
            dispatch
        }
        _ => panic!("one real PostgreSQL claim winner only"),
    };
    assert_eq!(dispatch.request.sequence, 0);
    // Stop after acknowledged durable claim, before HTTP. A reconstructed owner
    // cannot infer no-effect or steal/restart that unresolved submitted request.
    drop(dispatch);
    let reopened = service(&rig.pool);
    let action = step(&reopened, &transport, &original).await;
    assert_eq!(action.requests.len(), 1);
    assert!(matches!(
        action.requests[0].outcome,
        NativeDeviceOutcome::Submitted
    ));
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 0);
    let input = reopened
        .secret_store
        .custody(
            &original.definition.scope.resource,
            &input_handle(original.definition.flow_id, 0).unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(input.state, SecretCustodyState::Retained { .. }));
    let mut changed = original.clone();
    changed.definition.client.client_id = "different-client".into();
    assert!(matches!(
        reopened
            .claim_native_device(&changed, uuid::Uuid::new_v4())
            .await,
        Err(AuthProductError::BackendConflict)
    ));
    rig.pool
        .get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM brassclaw_product_auth_flows WHERE id=$1",
            &[&original.definition.flow_id.to_string()],
        )
        .await
        .unwrap();
    assert_eq!(
        reopened
            .prepare_native_device(
                original.definition.clone(),
                "actual encrypted original native setup context".into()
            )
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn native_device_actual_commit_and_raw_write_faults_never_dispatch_or_replay_unknown_results()
{
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let original = prepare(&auth, &transport, 60).await.original;
    let client = rig.pool.get().await.unwrap();
    let mut setup = original.definition.clone();
    setup.flow_id = AuthFlowId::new();
    setup.account_id = CredentialAccountId::new();
    setup.destinations = transport
        .native_device_destinations(setup.flow_id, setup.scope.resource.invocation_id)
        .unwrap();
    client.batch_execute("CREATE FUNCTION fail_device_origin_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private origin fault' USING ERRCODE='23514'; END $$;
        CREATE CONSTRAINT TRIGGER device_origin_fault AFTER INSERT ON brassclaw_product_auth_flows DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_device_origin_commit()"
    ).await.unwrap();
    assert!(
        auth.prepare_native_device(setup.clone(), "original setup whose commit fails".into())
            .await
            .is_err()
    );
    assert!(
        auth.secret_store
            .custody(
                &setup.scope.resource,
                &preparation_handle(setup.flow_id).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    let absent: i64 = client
        .query_one(
            "SELECT count(*) FROM brassclaw_product_auth_flows WHERE id=$1",
            &[&setup.flow_id.to_string()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(absent, 0);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 0);
    client
        .batch_execute("DROP TRIGGER device_origin_fault ON brassclaw_product_auth_flows")
        .await
        .unwrap();

    client.batch_execute("CREATE FUNCTION fail_device_claim_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private claim fault' USING ERRCODE='23514'; END $$;
        CREATE CONSTRAINT TRIGGER device_claim_fault AFTER UPDATE ON brassclaw_product_auth_flows DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_device_claim_commit()"
    ).await.unwrap();
    assert!(
        auth.run_native_device_once(&transport, &original, uuid::Uuid::new_v4())
            .await
            .is_err()
    );
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 0);
    client
        .batch_execute("DROP TRIGGER device_claim_fault ON brassclaw_product_auth_flows")
        .await
        .unwrap();
    assert!(
        auth.read_native_device_state(&original.definition.scope, original.definition.flow_id)
            .await
            .unwrap()
            .unwrap()
            .action
            .requests
            .is_empty()
    );
    // The issuer now really returns a challenge. Its encrypted-result INSERT
    // fails: this does not authorize another usercode request on reconstruction.
    client.batch_execute("CREATE FUNCTION fail_device_raw() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.name LIKE 'codex-device-result-%' THEN RAISE EXCEPTION 'private raw fault' USING ERRCODE='23514'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER device_raw_fault BEFORE INSERT ON brassclaw_secrets FOR EACH ROW EXECUTE FUNCTION fail_device_raw()"
    ).await.unwrap();
    assert!(matches!(
        step(&auth, &transport, &original).await.requests[0].outcome,
        NativeDeviceOutcome::Unknown { .. }
    ));
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    client
        .batch_execute("DROP TRIGGER device_raw_fault ON brassclaw_secrets")
        .await
        .unwrap();
    let reopened = service(&rig.pool);
    let actual = step(&reopened, &transport, &original).await;
    assert_eq!(actual.requests.len(), 1);
    assert!(matches!(
        actual.requests[0].outcome,
        NativeDeviceOutcome::Unknown { .. }
    ));
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    assert!(
        reopened
            .secret_store
            .custody(
                &original.definition.scope.resource,
                &result_handle(original.definition.flow_id, 0).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    let next = prepare(&reopened, &transport, 60).await.original;
    client.batch_execute("CREATE FUNCTION fail_device_return_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private return fault' USING ERRCODE='23514'; END $$;
        CREATE CONSTRAINT TRIGGER device_return_fault AFTER UPDATE ON brassclaw_product_auth_flows DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
        WHEN (NEW.data#>>'{auth_custody,device_login,requests,0,outcome,kind}'='challenge') EXECUTE FUNCTION fail_device_return_commit()"
    ).await.unwrap();
    assert!(matches!(
        step(&reopened, &transport, &next).await.requests[0].outcome,
        NativeDeviceOutcome::Unknown { .. }
    ));
    client
        .batch_execute("DROP TRIGGER device_return_fault ON brassclaw_product_auth_flows")
        .await
        .unwrap();
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 2);
    let recovered = step(&service(&rig.pool), &transport, &next).await;
    assert!(matches!(
        recovered.requests[0].outcome,
        NativeDeviceOutcome::Unknown { .. }
    ));
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 2);
    assert!(
        reopened
            .secret_store
            .custody(
                &next.definition.scope.resource,
                &result_handle(next.definition.flow_id, 0).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn native_device_actual_http_timeout_and_body_bound_results_remain_unknown_after_restart() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    for (mode, timeout, limit) in [
        (IssuerMode::DelayedStart, Some(200), None),
        (IssuerMode::LargeStart, Some(5000), Some(128)),
    ] {
        let issuer = Issuer::start(mode).await;
        let transport = transport(&rig.pool, &auth, &issuer, timeout, limit).await;
        let original = prepare(&auth, &transport, 60).await.original;
        let returned = step(&auth, &transport, &original).await;
        assert!(matches!(
            returned.requests[0].outcome,
            NativeDeviceOutcome::Unknown { .. }
        ));
        assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
        let reopened = service(&rig.pool);
        assert_eq!(step(&reopened, &transport, &original).await, returned);
        assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn native_device_issuer_throttle_has_no_artificial_sixty_second_cap_or_deadline_renewal() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Throttle).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let original = prepare(&auth, &transport, 60).await.original;
    let challenge = step(&auth, &transport, &original).await;
    wait_until_due(&challenge).await;
    let pending = step(&auth, &transport, &original).await;
    let NativeDeviceOutcome::Pending {
        interval_secs,
        next_poll_at,
        ..
    } = pending.requests.last().unwrap().outcome
    else {
        panic!("actual 429 pending result");
    };
    assert_eq!(interval_secs, 120);
    assert_eq!(next_poll_at, challenge.expiry());
    assert_eq!(pending.expiry(), challenge.expiry());
    let reopened = service(&rig.pool);
    assert_eq!(step(&reopened, &transport, &original).await, pending);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 2);
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn native_device_consumed_token_exchange_with_actual_lost_http_return_is_never_replayed() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::DisconnectToken).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let original = prepare(&auth, &transport, 60).await.original;
    let challenge = step(&auth, &transport, &original).await;
    wait_until_due(&challenge).await;
    step(&auth, &transport, &original).await;
    let unknown = step(&auth, &transport, &original).await;
    assert!(matches!(
        unknown.requests.last().unwrap().outcome,
        NativeDeviceOutcome::Unknown { .. }
    ));
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 1);
    let reopened = service(&rig.pool);
    assert_eq!(step(&reopened, &transport, &original).await, unknown);
    assert_eq!(issuer.state.token_calls.load(Ordering::SeqCst), 1);
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    assert!(
        reopened
            .native_device_ready_in_transaction(&tx, &original)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn native_device_cancellation_retains_actual_late_return_without_publishing_or_restarting() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let reopened = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::HoldStart).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let original = prepare(&auth, &transport, 60).await.original;
    let (returned, ()) = tokio::join!(
        auth.run_native_device_once(&transport, &original, uuid::Uuid::new_v4()),
        async {
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                issuer.state.entered.notified(),
            )
            .await
            .unwrap();
            reopened
                .cancel_flow(&original.definition.scope, original.definition.flow_id)
                .await
                .unwrap();
            issuer.state.released.notify_one();
        }
    );
    let returned = returned.unwrap();
    assert!(matches!(
        returned.requests[0].outcome,
        NativeDeviceOutcome::Challenge { .. }
    ));
    let state = reopened
        .read_native_device_state(&original.definition.scope, original.definition.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.flow.status, AuthFlowStatus::Canceled);
    assert!(
        reopened
            .native_device_challenge(&original)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(step(&reopened, &transport, &original).await, returned);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn native_device_host_expiry_and_actual_kernel_denial_never_start_or_renew_login() {
    use crate::oauth_provider_client::{
        ExchangeScopePolicy, HostOAuthProviderClient, HostOAuthProviderSpec,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let local = transport(&rig.pool, &auth, &issuer, None, None).await;
    let template = prepare(&auth, &local, 60).await.original;
    let mut definition = template.definition.clone();
    definition.flow_id = AuthFlowId::new();
    definition.account_id = CredentialAccountId::new();
    definition.destinations = local
        .native_device_destinations(definition.flow_id, definition.scope.resource.invocation_id)
        .unwrap();
    definition.created_at = Utc::now();
    definition.expires_at = definition.created_at + chrono::Duration::milliseconds(50);
    let original = auth
        .prepare_native_device(definition, "original short host intent".into())
        .await
        .unwrap()
        .original;
    tokio::time::sleep(
        original
            .definition
            .expires_at
            .signed_duration_since(Utc::now())
            .to_std()
            .unwrap_or_default(),
    )
    .await;
    let reopened = service(&rig.pool);
    step(&reopened, &local, &original).await;
    let state = reopened
        .read_native_device_state(&original.definition.scope, original.definition.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.flow.status, AuthFlowStatus::Expired);
    assert!(state.action.requests.is_empty());
    assert_eq!(state.flow.expires_at, original.definition.expires_at);
    let mut renewed = original.definition.clone();
    renewed.expires_at = Utc::now() + chrono::Duration::seconds(60);
    assert_eq!(
        reopened
            .prepare_native_device(renewed, "original short host intent".into())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 0);
    let obligations = brassclaw_host_runtime::BuiltinObligationServices::new(
        Arc::new(brassclaw_events::InMemoryAuditSink::new()),
        auth.secret_store.clone(),
        Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
    );
    let kernel_errors = Arc::new(Mutex::new(Vec::new()));
    let host = Arc::new(obligations.host_http_egress(
        brassclaw_network::PolicyNetworkHttpEgress::new(
            brassclaw_network::ReqwestNetworkTransport::default(),
        ),
    ));
    let actual_kernel = Arc::new(ObservedKernelEgress {
        inner: host.clone(),
        original: host,
        errors: kernel_errors.clone(),
    });
    let network = HostOAuthProviderClient::new(
        HostOAuthProviderSpec {
            provider_id: "openai_codex",
            capability_id: "oauth.native-device-test",
            token_endpoint: "https://127.0.0.1:9/oauth/token",
            secret_handle_prefix: "codex-native",
            resource: None,
            exchange_scope_policy: ExchangeScopePolicy::FallbackToRequested,
        },
        actual_kernel.clone(),
        auth.secret_store.clone(),
        Arc::new(obligations.obligation_handler()),
        brassclaw_auth::OAuthClientId::new("public-native-client").unwrap(),
        brassclaw_auth::OAuthRedirectUri::new("https://127.0.0.1:9/deviceauth/callback").unwrap(),
    )
    .unwrap()
    .with_original_egress(Some(actual_kernel));
    let mut definition = template.definition.clone();
    definition.flow_id = AuthFlowId::new();
    definition.account_id = CredentialAccountId::new();
    definition.destinations = network
        .native_device_destinations(definition.flow_id, definition.scope.resource.invocation_id)
        .unwrap();
    definition.created_at = Utc::now();
    definition.expires_at = definition.created_at + chrono::Duration::seconds(60);
    let actual_url = url::Url::parse(&issuer.endpoint).unwrap();
    definition.client.auth_endpoint = format!("https://127.0.0.1:{}", actual_url.port().unwrap());
    let denied = auth
        .prepare_native_device(definition, "original denied by actual kernel".into())
        .await
        .unwrap()
        .original;
    let unknown = step(&auth, &network, &denied).await;
    assert!(matches!(
        unknown.requests[0].outcome,
        NativeDeviceOutcome::Unknown { .. }
    ));
    assert_eq!(
        issuer.state.calls.load(Ordering::SeqCst),
        0,
        "actual production HTTPS/private-IP policy denies before the local issuer"
    );
    assert_eq!(
        *kernel_errors.lock().unwrap(),
        vec![brassclaw_host_api::RuntimeHttpEgressReasonCode::PolicyDenied]
    );
}

#[tokio::test]
async fn native_device_absent_issuer_expiry_uses_exact_original_host_deadline_not_legacy_default() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::NoExpiry).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    // An explicit original host setting above the historical File fallback.
    let original = prepare(&auth, &transport, 1800).await.original;
    let first = step(&auth, &transport, &original).await;
    let NativeDeviceOutcome::Challenge {
        issuer_expires_at,
        expires_at,
        ..
    } = &first.requests[0].outcome
    else {
        panic!("actual original challenge");
    };
    assert!(issuer_expires_at.is_none());
    assert_eq!(*expires_at, original.definition.expires_at);
    assert_eq!(
        service(&rig.pool)
            .read_native_device_state(&original.definition.scope, original.definition.flow_id)
            .await
            .unwrap()
            .unwrap()
            .action
            .expiry(),
        *expires_at
    );
    let raw = issuer.state.received.lock().unwrap();
    assert_eq!(raw.len(), 1);
}

#[tokio::test]
async fn native_device_original_reads_do_not_wait_for_flow_writer_and_preparation_recovery_is_exact()
 {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let scope = scope();
    assert!(
        auth.read_native_device_state(&scope, AuthFlowId::new())
            .await
            .unwrap()
            .is_none()
    );
    let original = prepare(&auth, &transport, 60).await.original;
    let mut writer = rig.pool.get().await.unwrap();
    let held = writer.transaction().await.unwrap();
    super::super::callback::locked_flow(
        &held,
        &original.definition.scope,
        original.definition.flow_id,
    )
    .await
    .unwrap();
    // Database-enforced row lock is held already. Timeout is a failure bound,
    // never a scheduler delay or assertion that a sleep proves concurrency.
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        auth.read_native_device_state(&original.definition.scope, original.definition.flow_id),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(read.action.original, original);
    held.rollback().await.unwrap();
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    let mut forged = original.clone();
    forged.preparation.reference = "other-selected-original".into();
    assert!(
        auth.native_device_preparation_in_transaction(&tx, &forged)
            .await
            .is_err()
    );
    let context = auth
        .native_device_preparation_in_transaction(&tx, &original)
        .await
        .unwrap();
    assert_eq!(
        context.expose_secret(),
        "actual encrypted original native setup context"
    );
    tx.rollback().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    let context = auth
        .native_device_preparation_in_transaction(&tx, &original)
        .await
        .unwrap();
    assert_eq!(
        context.expose_secret(),
        "actual encrypted original native setup context"
    );
    tx.commit().await.unwrap();
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 0);
    // Missing flow plus authenticated retained preparation is corruption, NOT
    // absence permitting automatic creation of a new login.
    connection
        .execute(
            "DELETE FROM brassclaw_product_auth_flows WHERE id=$1",
            &[&original.definition.flow_id.to_string()],
        )
        .await
        .unwrap();
    assert!(matches!(
        auth.read_native_device_state(&original.definition.scope, original.definition.flow_id)
            .await,
        Err(AuthProductError::BackendConflict)
    ));
    let fresh = prepare(&auth, &transport, 60).await.original;
    connection
        .execute(
            "UPDATE brassclaw_product_auth_flows SET data='{}'::jsonb WHERE id=$1",
            &[&fresh.definition.flow_id.to_string()],
        )
        .await
        .unwrap();
    assert!(
        auth.read_native_device_state(&fresh.definition.scope, fresh.definition.flow_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn native_device_actual_custodian_rejects_changed_request_duplicate_preflight_and_failed_request_commit()
 {
    use brassclaw_host_runtime::HostHttpOriginalCustody;
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let original = prepare(&auth, &transport, 60).await.original;
    let NativeDeviceAdmission::Dispatch(dispatch) = auth
        .claim_native_device(&original, uuid::Uuid::new_v4())
        .await
        .unwrap()
    else {
        panic!("fresh acknowledged claim");
    };
    let _original_input = auth.consume_native_device_input(&dispatch).await.unwrap();
    let request = transport
        .prepare_oauth_http_request(
            original.definition.scope.resource.clone(),
            "https://native-device.example/api/accounts/deviceauth/usercode",
            "application/json",
            serde_json::to_vec(
                &serde_json::json!({"client_id":original.definition.client.client_id}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut sink = NativeDeviceCustodian {
        auth: &auth,
        dispatch: &dispatch,
        expected: request.clone(),
        transport: None,
    };
    let mut changed = request.clone();
    changed.body = b"different-client-request".to_vec();
    assert!(sink.authenticate_request(&changed).await.is_err());
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 0);
    transport
        .native_original_egress()
        .unwrap()
        .execute_with_original_custody(request.clone(), Box::new(sink))
        .await
        .unwrap();
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    let mut duplicate = NativeDeviceCustodian {
        auth: &auth,
        dispatch: &dispatch,
        expected: request.clone(),
        transport: None,
    };
    assert!(duplicate.authenticate_request(&request).await.is_err());
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    let state = auth
        .read_native_device_state(&original.definition.scope, original.definition.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.action.transport_originals.len(), 1);
    let saved = &state.action.transport_originals[0];
    assert!(
        auth.secret_store
            .lease_once_if_record_reference(
                &original.definition.scope.resource,
                &saved.source.handle,
                &saved.source.reference,
            )
            .await
            .unwrap()
            .is_none(),
        "an immutable request Original must never be leased as a runtime credential"
    );
    let mut connection = rig.pool.get().await.unwrap();
    let tx = connection.transaction().await.unwrap();
    let authenticated = auth
        .authenticate_device_source(
            &tx,
            &original,
            &saved.source,
            request_context(&original, &dispatch.request),
        )
        .await
        .unwrap();
    let captured = auth
        .read_native_device_original_in_transaction(&tx, &source_from(&authenticated))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    // System runtime identity is host-assigned and intentionally cannot be
    // deserialized from JSON. Verify the exact stored request bytes instead.
    assert_eq!(
        captured.expose_secret(),
        &serde_json::to_string(&request).unwrap()
    );

    let failing = prepare(&auth, &transport, 60).await.original;
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION fail_device_request_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'request original commit fault' USING ERRCODE='23514'; END $$;
        CREATE CONSTRAINT TRIGGER device_request_fault AFTER UPDATE ON brassclaw_product_auth_flows DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
        WHEN (jsonb_array_length(NEW.data#>'{auth_custody,device_login,transport_originals}')=1 AND NEW.data#>>'{auth_custody,device_login,requests,0,outcome,kind}'='submitted') EXECUTE FUNCTION fail_device_request_commit()"
    ).await.unwrap();
    let unknown = step(&auth, &transport, &failing).await;
    assert!(matches!(
        unknown.requests[0].outcome,
        NativeDeviceOutcome::Unknown { .. }
    ));
    assert!(unknown.transport_originals.is_empty());
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    client
        .batch_execute("DROP TRIGGER device_request_fault ON brassclaw_product_auth_flows")
        .await
        .unwrap();
    assert!(
        auth.secret_store
            .custody(
                &failing.definition.scope.resource,
                &transport_handle(failing.definition.flow_id, 0).unwrap()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        step(&service(&rig.pool), &transport, &failing).await,
        unknown
    );
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn native_device_lost_caller_return_after_actual_http_and_pg_capture_reconciles_without_replay()
 {
    // Fault injection at the caller-return boundary AFTER genuine HTTP and PG
    // capture, not an assertion that a PostgreSQL wire COMMIT ACK was dropped.
    struct LostCallerReturn {
        inner: Arc<dyn brassclaw_host_runtime::HostHttpOriginalEgress>,
        dropped: Arc<AtomicUsize>,
    }
    #[async_trait]
    impl brassclaw_host_runtime::HostHttpOriginalEgress for LostCallerReturn {
        async fn execute_with_original_custody<'a>(
            &'a self,
            request: RuntimeHttpEgressRequest,
            custody: Box<dyn brassclaw_host_runtime::HostHttpOriginalCustody + 'a>,
        ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
            let returned = self
                .inner
                .execute_with_original_custody(request, custody)
                .await?;
            self.dropped.fetch_add(1, Ordering::SeqCst);
            Err(network_error(
                "injected lost caller return after actual custody",
                returned.request_bytes,
                returned.response_bytes,
            ))
        }
    }
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    run_auth_migrations(&rig.pool).await.unwrap();
    let auth = service(&rig.pool);
    let issuer = Issuer::start(IssuerMode::Success).await;
    let transport = transport(&rig.pool, &auth, &issuer, None, None).await;
    let dropped = Arc::new(AtomicUsize::new(0));
    let transport = transport
        .clone()
        .with_original_egress(Some(Arc::new(LostCallerReturn {
            inner: transport.native_original_egress().unwrap(),
            dropped: dropped.clone(),
        })));
    let original = prepare(&auth, &transport, 60).await.original;
    let returned = step(&auth, &transport, &original).await;
    assert!(matches!(
        returned.requests[0].outcome,
        NativeDeviceOutcome::Challenge { .. }
    ));
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    let reopened = service(&rig.pool);
    assert_eq!(step(&reopened, &transport, &original).await, returned);
    assert_eq!(issuer.state.calls.load(Ordering::SeqCst), 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(
        reopened
            .secret_store
            .custody(
                &original.definition.scope.resource,
                &result_handle(original.definition.flow_id, 0).unwrap()
            )
            .await
            .unwrap()
            .is_some()
    );
}

#[path = "pg_auth_codex_refresh_tests.rs"]
mod codex_refresh_tests;
