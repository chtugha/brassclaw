use super::*;
use brassclaw_auth::{
    AuthProductScope, AuthProviderId, AuthSurface, AuthorizationCodeHash, CredentialAccountLabel,
    OAuthAuthorizationCode, PkceVerifierHash, PkceVerifierSecret,
};
use brassclaw_host_api::{
    InvocationId, RuntimeHttpEgressError, RuntimeHttpEgressResponse, TenantId, UserId,
};
use brassclaw_secrets::{
    InMemorySecretStore, SecretLease, SecretLeaseId, SecretMaterial, SecretMetadata,
    SecretStoreError,
};
use std::sync::Mutex;

#[test]
fn authorization_code_body_adds_provider_resource_only_when_configured() {
    let with_resource = authorization_code_body(
        &notion_spec(),
        "client-id",
        "https://app.example/callback",
        None,
        "code",
        "pkce",
    );
    let without_resource = authorization_code_body(
        &google_spec(),
        "client-id",
        "https://app.example/callback",
        None,
        "code",
        "pkce",
    );

    let with_resource = form_params(&with_resource);
    let without_resource = form_params(&without_resource);
    assert_eq!(
        with_resource.get("resource").map(String::as_str),
        Some("https://mcp.notion.com/mcp")
    );
    assert!(!without_resource.contains_key("resource"));
}

#[tokio::test]
async fn token_sink_rolls_back_access_token_when_refresh_write_fails() {
    let store = Arc::new(RecordingSecretStore::failing_refresh_put());
    let client = HostOAuthProviderClient::new(
        notion_spec(),
        Arc::new(NoopEgress),
        store.clone(),
        Arc::new(NoopObligationHandler),
        OAuthClientId::new("client-id").unwrap(),
        OAuthRedirectUri::new("https://app.example/callback").unwrap(),
    )
    .unwrap();
    let scope = sample_scope();
    let destinations =
        TokenDestinations::exchange(&client.spec, AuthFlowId::new(), scope.invocation_id).unwrap();
    let result = client
        .store_tokens(
            scope,
            destinations,
            OAuthTokenResponse::new(
                SecretString::from("access-token".to_string()),
                Some(SecretString::from("refresh-token".to_string())),
                Some("workspace"),
                None,
            )
            .unwrap(),
            chrono::Utc::now(),
        )
        .await;

    assert_eq!(
        result.expect_err("refresh write failure").code(),
        brassclaw_auth::AuthErrorCode::BackendUnavailable
    );
    assert!(
        store
            .deleted_handles()
            .iter()
            .any(|handle| handle.contains("access")),
        "access token written before the failed refresh token must be cleaned up"
    );
}

#[tokio::test]
async fn google_exchange_fails_closed_when_response_omits_scope() {
    let egress = Arc::new(RecordingEgress::ok(
        br#"{"access_token":"access-token","refresh_token":"refresh-token","expires_in":3600}"#
            .to_vec(),
    ));
    let store = Arc::new(RecordingSecretStore::recording());
    let client = HostOAuthProviderClient::new(
        google_spec(),
        egress,
        store.clone(),
        Arc::new(NoopObligationHandler),
        OAuthClientId::new("google-client").unwrap(),
        OAuthRedirectUri::new("https://app.example/callback").unwrap(),
    )
    .unwrap();

    let error = client
        .exchange_callback(
            exchange_context(),
            callback_request("google", "work google", &["gmail.readonly"]),
        )
        .await
        .expect_err("google must not trust requested scopes when provider omits scope");

    assert_eq!(
        error.code(),
        brassclaw_auth::AuthErrorCode::TokenExchangeFailed
    );
    assert!(store.put_handles().is_empty());
}

#[tokio::test]
async fn issuer_lifetime_uses_pre_request_origin_and_rejects_overflow_before_secret_writes() {
    let duration_overflowing_timestamp =
        u64::try_from(chrono::Duration::MAX.num_seconds()).unwrap();
    for lifetime in [
        None,
        Some(0),
        Some(3600),
        Some(u64::MAX),
        Some(duration_overflowing_timestamp),
    ] {
        let mut body =
            serde_json::json!({"access_token":"finite-access", "scope":"gmail.readonly"});
        if let Some(seconds) = lifetime {
            body["expires_in"] = seconds.into();
        }
        let mut observed = RecordingEgress::ok(serde_json::to_vec(&body).unwrap());
        observed.response_delay = std::time::Duration::from_millis(100);
        let egress = Arc::new(observed);
        let store = Arc::new(RecordingSecretStore::recording());
        let client = HostOAuthProviderClient::new(
            google_spec(),
            egress.clone(),
            store.clone(),
            Arc::new(NoopObligationHandler),
            OAuthClientId::new("google-client").unwrap(),
            OAuthRedirectUri::new("https://app.example/callback").unwrap(),
        )
        .unwrap();
        let context = exchange_context();
        let before = chrono::Utc::now();
        let result = client
            .exchange_callback(
                context.clone(),
                callback_request("google", "finite", &["gmail.readonly"]),
            )
            .await;
        assert_eq!(egress.requests.lock().unwrap().len(), 1);
        if lifetime == Some(u64::MAX) || lifetime == Some(duration_overflowing_timestamp) {
            assert!(matches!(result, Err(AuthProductError::TokenExchangeFailed)));
            assert!(store.put_handles().is_empty());
            continue;
        }
        let result = result.unwrap();
        match lifetime {
            Some(seconds) => {
                let duration = chrono::Duration::seconds(i64::try_from(seconds).unwrap());
                let deadline = result.access_expires_at.unwrap();
                assert!(deadline >= before + duration);
                assert!(
                    deadline < egress.response_at.lock().unwrap().unwrap() + duration,
                    "a delayed reply must not extend the issuer's lifetime"
                );
                if seconds == 0 {
                    assert!(
                        store
                            .lease_once(&context.scope.resource, &result.access_secret)
                            .await
                            .unwrap_err()
                            .is_expired()
                    );
                    continue;
                }
            }
            None => assert_eq!(result.access_expires_at, None),
        }
        let selected = store
            .lease_once(&context.scope.resource, &result.access_secret)
            .await
            .unwrap();
        assert_eq!(selected.secret_expires_at, result.access_expires_at);
        assert_eq!(
            store
                .consume(&context.scope.resource, selected.id)
                .await
                .unwrap()
                .expose_secret(),
            "finite-access"
        );
    }
}

#[tokio::test]
async fn explicit_invalid_issuer_lifetime_is_not_treated_as_omitted() {
    for lifetime in [
        serde_json::Value::Null,
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!("3600"),
    ] {
        let body = serde_json::json!({"access_token":"must not publish", "scope":"gmail.readonly", "expires_in":lifetime});
        let egress = Arc::new(RecordingEgress::ok(serde_json::to_vec(&body).unwrap()));
        let store = Arc::new(RecordingSecretStore::recording());
        let client = HostOAuthProviderClient::new(
            google_spec(),
            egress.clone(),
            store.clone(),
            Arc::new(NoopObligationHandler),
            OAuthClientId::new("google-client").unwrap(),
            OAuthRedirectUri::new("https://app.example/callback").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            client
                .exchange_callback(
                    exchange_context(),
                    callback_request("google", "invalid lifetime", &["gmail.readonly"])
                )
                .await,
            Err(AuthProductError::TokenExchangeFailed)
        ));
        assert_eq!(egress.requests.lock().unwrap().len(), 1);
        assert!(store.put_handles().is_empty());
    }
}

#[tokio::test]
async fn exchange_maps_provider_5xx_to_retryable_backend_unavailable() {
    let egress = Arc::new(RecordingEgress::with_status(
        503,
        br#"{"error":"temporarily_unavailable"}"#.to_vec(),
    ));
    let client = HostOAuthProviderClient::new(
        google_spec(),
        egress,
        Arc::new(RecordingSecretStore::recording()),
        Arc::new(NoopObligationHandler),
        OAuthClientId::new("google-client").unwrap(),
        OAuthRedirectUri::new("https://app.example/callback").unwrap(),
    )
    .unwrap();

    let error = client
        .exchange_callback(
            exchange_context(),
            callback_request("google", "work google", &["gmail.readonly"]),
        )
        .await
        .expect_err("provider 5xx should be retryable");

    assert_eq!(
        error.code(),
        brassclaw_auth::AuthErrorCode::BackendUnavailable
    );
}

#[tokio::test]
async fn exchange_request_includes_client_secret_and_derived_network_policy_host() {
    let egress = Arc::new(RecordingEgress::ok(
            br#"{"access_token":"access-token","refresh_token":"refresh-token","scope":"gmail.readonly","expires_in":3600}"#.to_vec(),
        ));
    let client = HostOAuthProviderClient::new(
        google_spec(),
        egress.clone(),
        Arc::new(RecordingSecretStore::recording()),
        Arc::new(NoopObligationHandler),
        OAuthClientId::new("google-client").unwrap(),
        OAuthRedirectUri::new("https://app.example/callback").unwrap(),
    )
    .unwrap()
    .with_client_secret(SecretString::from("google-secret".to_string()));

    client
        .exchange_callback(
            exchange_context(),
            callback_request("google", "work google", &["gmail.readonly"]),
        )
        .await
        .expect("exchange succeeds");

    let request = egress.single_request();
    assert_eq!(request.url, "https://oauth2.googleapis.com/token");
    assert_eq!(
        request
            .network_policy
            .allowed_targets
            .first()
            .map(|target| target.host_pattern.as_str()),
        Some("oauth2.googleapis.com")
    );
    let body = form_params(&request.body);
    assert_eq!(
        body.get("client_secret").map(String::as_str),
        Some("google-secret")
    );
}

#[tokio::test]
async fn exchange_uses_dynamic_client_material_and_binds_refresh_secret() {
    let egress = Arc::new(RecordingEgress::ok(
        br#"{"access_token":"access-token","refresh_token":"refresh-token","expires_in":3600}"#
            .to_vec(),
    ));
    let material_source = Arc::new(RecordingMaterialSource::new());
    let client = HostOAuthProviderClient::new_with_client_material(
        notion_spec(),
        egress.clone(),
        Arc::new(InMemorySecretStore::new()),
        Arc::new(NoopObligationHandler),
        material_source.clone(),
    )
    .unwrap();
    let context = exchange_context();
    let flow_id = context.flow_id;

    client
        .exchange_callback(context, callback_request("notion", "notion", &[]))
        .await
        .expect("exchange succeeds");

    let request = egress.single_request();
    assert_eq!(request.url, "https://issuer.example/token");
    let body = form_params(&request.body);
    assert_eq!(
        body.get("client_id").map(String::as_str),
        Some("dcr-client")
    );
    assert_eq!(
        material_source.bound_flow_ids(),
        vec![flow_id],
        "refresh-capable exchanges must bind the DCR client material to the refresh secret"
    );
}

#[tokio::test]
async fn refresh_request_uses_stored_refresh_token_and_preserves_scope_fallback() {
    let egress = Arc::new(RecordingEgress::ok(
            br#"{"access_token":"new-access-token","refresh_token":"new-refresh-token","expires_in":3600}"#
                .to_vec(),
        ));
    let store = Arc::new(InMemorySecretStore::new());
    let scope = sample_scope();
    let refresh_secret = SecretHandle::new("google-refresh-input").unwrap();
    store
        .put(
            scope.clone(),
            refresh_secret.clone(),
            SecretString::from("stored-refresh-token".to_string()),
        )
        .await
        .expect("store refresh token");
    let client = HostOAuthProviderClient::new(
        google_spec(),
        egress.clone(),
        store,
        Arc::new(NoopObligationHandler),
        OAuthClientId::new("google-client").unwrap(),
        OAuthRedirectUri::new("https://app.example/callback").unwrap(),
    )
    .unwrap();

    let refresh = prepared_refresh(
        &client,
        OAuthProviderRefreshRequest {
            refresh_id: brassclaw_auth::CredentialRefreshId::new(),
            scope: AuthProductScope::new(scope, AuthSurface::Callback),
            provider: AuthProviderId::new("google").unwrap(),
            account_id: CredentialAccountId::new(),
            refresh_secret,
            scopes: vec![ProviderScope::new("gmail.readonly").unwrap()],
        },
    )
    .await
    .expect("refresh succeeds");

    assert_eq!(
        refresh.scopes,
        vec![ProviderScope::new("gmail.readonly").unwrap()]
    );
    let request = egress.single_request();
    let body = form_params(&request.body);
    assert_eq!(
        body.get("grant_type").map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        body.get("refresh_token").map(String::as_str),
        Some("stored-refresh-token")
    );
}

#[test]
fn token_endpoint_host_is_derived_and_rejects_non_https_endpoints() {
    assert_eq!(
        oauth_endpoint_host("https://oauth2.googleapis.com/token").unwrap(),
        "oauth2.googleapis.com"
    );
    assert_eq!(
        oauth_endpoint_host("http://oauth2.googleapis.com/token")
            .expect_err("http endpoints are rejected")
            .code(),
        brassclaw_auth::AuthErrorCode::BackendUnavailable
    );
}

fn form_params(body: &[u8]) -> std::collections::BTreeMap<String, String> {
    url::form_urlencoded::parse(body).into_owned().collect()
}

fn exchange_context() -> OAuthProviderExchangeContext {
    OAuthProviderExchangeContext {
        scope: AuthProductScope::new(sample_scope(), AuthSurface::Callback),
        flow_id: AuthFlowId::new(),
    }
}

fn callback_request(provider: &str, label: &str, scopes: &[&str]) -> OAuthProviderCallbackRequest {
    OAuthProviderCallbackRequest {
        provider: AuthProviderId::new(provider).unwrap(),
        account_label: CredentialAccountLabel::new(label).unwrap(),
        authorization_code: OAuthAuthorizationCode::new(SecretString::from(
            "raw-auth-code".to_string(),
        ))
        .unwrap(),
        authorization_code_hash: AuthorizationCodeHash::new(brassclaw_common::hashing::sha256_hex(
            b"raw-auth-code",
        ))
        .unwrap(),
        pkce_verifier: PkceVerifierSecret::new(SecretString::from("raw-pkce-verifier".to_string()))
            .unwrap(),
        pkce_verifier_hash: PkceVerifierHash::new(brassclaw_common::hashing::sha256_hex(
            b"raw-pkce-verifier",
        ))
        .unwrap(),
        scopes: scopes
            .iter()
            .map(|scope| ProviderScope::new(*scope).unwrap())
            .collect(),
    }
}

fn google_spec() -> HostOAuthProviderSpec {
    HostOAuthProviderSpec {
        provider_id: "google",
        capability_id: "brassclaw_auth.google_oauth",
        token_endpoint: "https://oauth2.googleapis.com/token",
        secret_handle_prefix: "google",
        resource: None,
        exchange_scope_policy: ExchangeScopePolicy::RequireProviderScope,
    }
}

fn notion_spec() -> HostOAuthProviderSpec {
    HostOAuthProviderSpec {
        provider_id: "notion",
        capability_id: "brassclaw_auth.notion_oauth",
        token_endpoint: "https://mcp.notion.com/token",
        secret_handle_prefix: "notion",
        resource: Some("https://mcp.notion.com/mcp"),
        exchange_scope_policy: ExchangeScopePolicy::FallbackToRequested,
    }
}

fn sample_scope() -> ResourceScope {
    ResourceScope {
        tenant_id: TenantId::new("tenant-a").unwrap(),
        user_id: UserId::new("user-a").unwrap(),
        agent_id: None,
        project_id: None,
        thread_id: None,
        invocation_id: InvocationId::new(),
    }
}

struct RecordingSecretStore {
    inner: brassclaw_secrets::InMemorySecretStore,
    puts: Mutex<Vec<String>>,
    deleted: Mutex<Vec<String>>,
    fail_refresh_put: bool,
}

impl RecordingSecretStore {
    fn recording() -> Self {
        Self {
            inner: brassclaw_secrets::InMemorySecretStore::new(),
            puts: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
            fail_refresh_put: false,
        }
    }

    fn failing_refresh_put() -> Self {
        Self {
            inner: brassclaw_secrets::InMemorySecretStore::new(),
            puts: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
            fail_refresh_put: true,
        }
    }

    fn deleted_handles(&self) -> Vec<String> {
        self.deleted.lock().unwrap().clone()
    }

    fn put_handles(&self) -> Vec<String> {
        self.puts.lock().unwrap().clone()
    }
}

#[async_trait]
impl SecretStore for RecordingSecretStore {
    async fn close_identity(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        expected: brassclaw_secrets::SecretIdentityClosure,
    ) -> Result<Option<brassclaw_secrets::SecretCustodyMetadata>, brassclaw_secrets::SecretStoreError>
    {
        self.inner.close_identity(scope, handle, expected).await
    }

    async fn put(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
    ) -> Result<SecretMetadata, SecretStoreError> {
        self.put_with_expiry(scope, handle, material, None).await
    }

    async fn put_with_expiry(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        expires_at: Option<brassclaw_host_api::Timestamp>,
    ) -> Result<SecretMetadata, SecretStoreError> {
        let name = handle.as_str().to_string();
        self.puts.lock().unwrap().push(name.clone());
        if self.fail_refresh_put && name.contains("refresh") {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "refresh write failed".into(),
            });
        }
        self.inner
            .put_with_expiry(scope, handle, material, expires_at)
            .await
    }

    async fn record_reference(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<String>, SecretStoreError> {
        self.inner.record_reference(scope, handle).await
    }

    async fn put_if_record_reference(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        expected: Option<&str>,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        let name = handle.as_str().to_string();
        self.puts.lock().unwrap().push(name.clone());
        if self.fail_refresh_put && name.contains("refresh") {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "refresh write failed".into(),
            });
        }
        self.inner
            .put_if_record_reference(scope, handle, material, expected)
            .await
    }

    async fn matches(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        candidate: &SecretMaterial,
    ) -> Result<bool, SecretStoreError> {
        self.inner.matches(scope, handle, candidate).await
    }

    async fn put_original(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        material: SecretMaterial,
        context: serde_json::Value,
    ) -> Result<Option<brassclaw_secrets::SecretCustodyMetadata>, brassclaw_secrets::SecretStoreError>
    {
        let name = handle.as_str().to_string();
        self.puts.lock().unwrap().push(name.clone());
        if self.fail_refresh_put && name.contains("refresh") {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "refresh write failed".into(),
            });
        }
        self.inner
            .put_original(scope, handle, material, context)
            .await
    }

    async fn custody(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<brassclaw_secrets::SecretCustodyMetadata>, brassclaw_secrets::SecretStoreError>
    {
        self.inner.custody(scope, handle).await
    }

    async fn put_retained_from_original(
        &self,
        scope: ResourceScope,
        handle: SecretHandle,
        source: brassclaw_secrets::SecretCustodySource,
    ) -> Result<Option<brassclaw_secrets::SecretCustodyMetadata>, brassclaw_secrets::SecretStoreError>
    {
        let name = handle.as_str().to_string();
        self.puts.lock().unwrap().push(name.clone());
        if self.fail_refresh_put && name.contains("refresh") {
            return Err(SecretStoreError::StoreUnavailable {
                reason: "refresh write failed".into(),
            });
        }
        self.inner
            .put_retained_from_original(scope, handle, source)
            .await
    }

    async fn metadata(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<Option<SecretMetadata>, SecretStoreError> {
        self.inner.metadata(scope, handle).await
    }

    async fn delete(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<bool, SecretStoreError> {
        self.deleted
            .lock()
            .unwrap()
            .push(handle.as_str().to_string());
        self.inner.delete(scope, handle).await
    }

    async fn lease_once(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
    ) -> Result<SecretLease, SecretStoreError> {
        self.inner.lease_once(scope, handle).await
    }

    async fn lease_once_if_record_reference(
        &self,
        scope: &ResourceScope,
        handle: &SecretHandle,
        expected: &str,
    ) -> Result<Option<brassclaw_secrets::SecretLease>, brassclaw_secrets::SecretStoreError> {
        self.inner
            .lease_once_if_record_reference(scope, handle, expected)
            .await
    }

    async fn consume(
        &self,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretMaterial, SecretStoreError> {
        self.inner.consume(scope, lease_id).await
    }

    async fn revoke(
        &self,
        scope: &ResourceScope,
        lease_id: SecretLeaseId,
    ) -> Result<SecretLease, SecretStoreError> {
        self.inner.revoke(scope, lease_id).await
    }

    async fn leases_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<SecretLease>, SecretStoreError> {
        self.inner.leases_for_scope(scope).await
    }
}

#[derive(Debug)]
struct NoopEgress;

#[async_trait]
impl RuntimeHttpEgress for NoopEgress {
    async fn execute(
        &self,
        _request: RuntimeHttpEgressRequest,
    ) -> Result<
        brassclaw_host_api::RuntimeHttpEgressResponse,
        brassclaw_host_api::RuntimeHttpEgressError,
    > {
        Err(brassclaw_host_api::RuntimeHttpEgressError::Network {
            reason: "not used".to_string(),
            request_bytes: 0,
            response_bytes: 0,
        })
    }
}

#[derive(Debug)]
struct RecordingEgress {
    status: u16,
    response_body: Vec<u8>,
    requests: Mutex<Vec<RuntimeHttpEgressRequest>>,
    response_delay: std::time::Duration,
    response_at: Mutex<Option<brassclaw_auth::Timestamp>>,
}

#[derive(Debug)]
struct RecordingMaterialSource {
    bound_flow_ids: Mutex<Vec<AuthFlowId>>,
    bound_rotations: Mutex<Vec<(SecretHandle, SecretHandle)>>,
    cleaned_refreshes: Mutex<Vec<SecretHandle>>,
    cleaned_flows: Mutex<Vec<AuthFlowId>>,
    fail_initial_binding: bool,
    fail_rotated_binding: bool,
}

impl RecordingMaterialSource {
    fn new() -> Self {
        Self {
            bound_flow_ids: Mutex::new(Vec::new()),
            bound_rotations: Mutex::new(Vec::new()),
            cleaned_refreshes: Mutex::new(Vec::new()),
            cleaned_flows: Mutex::new(Vec::new()),
            fail_initial_binding: false,
            fail_rotated_binding: false,
        }
    }

    fn bound_flow_ids(&self) -> Vec<AuthFlowId> {
        self.bound_flow_ids.lock().unwrap().clone()
    }

    fn material(&self) -> OAuthClientMaterial {
        OAuthClientMaterial {
            client_id: OAuthClientId::new("dcr-client").unwrap(),
            client_secret: None,
            redirect_uri: OAuthRedirectUri::new("https://app.example/callback").unwrap(),
            token_endpoint: "https://issuer.example/token".to_string(),
        }
    }
}

#[async_trait]
impl OAuthClientMaterialSource for RecordingMaterialSource {
    async fn exchange_material(
        &self,
        _scope: &ResourceScope,
        _flow_id: AuthFlowId,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        Ok(self.material())
    }

    async fn refresh_material(
        &self,
        _scope: &ResourceScope,
        _refresh_secret: &SecretHandle,
    ) -> Result<OAuthClientMaterial, AuthProductError> {
        Ok(self.material())
    }

    async fn bind_refresh_material(
        &self,
        _scope: &ResourceScope,
        flow_id: AuthFlowId,
        _refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        self.bound_flow_ids.lock().unwrap().push(flow_id);
        if self.fail_initial_binding {
            return Err(AuthProductError::BackendUnavailable);
        }
        Ok(())
    }

    async fn bind_rotated_refresh_material(
        &self,
        _scope: &ResourceScope,
        previous: &SecretHandle,
        replacement: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        self.bound_rotations
            .lock()
            .unwrap()
            .push((previous.clone(), replacement.clone()));
        if self.fail_rotated_binding {
            return Err(AuthProductError::BackendUnavailable);
        }
        Ok(())
    }

    async fn cleanup_refresh_material(
        &self,
        _scope: &ResourceScope,
        refresh_secret: &SecretHandle,
    ) -> Result<(), AuthProductError> {
        self.cleaned_refreshes
            .lock()
            .unwrap()
            .push(refresh_secret.clone());
        Ok(())
    }

    async fn cleanup_exchange_material(
        &self,
        _scope: &ResourceScope,
        flow_id: AuthFlowId,
    ) -> Result<(), AuthProductError> {
        self.cleaned_flows.lock().unwrap().push(flow_id);
        Ok(())
    }
}

impl RecordingEgress {
    fn ok(response_body: Vec<u8>) -> Self {
        Self::with_status(200, response_body)
    }

    fn with_status(status: u16, response_body: Vec<u8>) -> Self {
        Self {
            status,
            response_body,
            requests: Mutex::new(Vec::new()),
            response_delay: std::time::Duration::ZERO,
            response_at: Mutex::new(None),
        }
    }

    fn single_request(&self) -> RuntimeHttpEgressRequest {
        let requests = self.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        requests[0].clone()
    }
}

#[async_trait]
impl RuntimeHttpEgress for RecordingEgress {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, RuntimeHttpEgressError> {
        self.requests.lock().unwrap().push(request);
        if !self.response_delay.is_zero() {
            tokio::time::sleep(self.response_delay).await;
        }
        *self.response_at.lock().unwrap() = Some(chrono::Utc::now());
        Ok(RuntimeHttpEgressResponse {
            status: self.status,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: self.response_body.clone(),
            saved_body: None,
            request_bytes: 0,
            response_bytes: 0,
            redaction_applied: true,
        })
    }
}

#[derive(Debug)]
struct NoopObligationHandler;

#[async_trait]
impl CapabilityObligationHandler for NoopObligationHandler {
    async fn satisfy(
        &self,
        _request: CapabilityObligationRequest<'_>,
    ) -> Result<(), brassclaw_capabilities::CapabilityObligationError> {
        Ok(())
    }
}

#[cfg(feature = "skills-db")]
mod native_refresh_store {
    use super::*;

    type ConsumeMutation = Arc<Mutex<Option<(ResourceScope, SecretHandle, SecretMaterial)>>>;

    /// Observe actual backend operations; default delegation is unchanged.
    /// A configured one-shot race mutates actual storage before real consume;
    /// no synthetic storage success or production fault hook is introduced.
    struct ObservedStore<S: SecretStore + ?Sized = brassclaw_secrets::PgSecretStore> {
        inner: Arc<S>,
        before_consume: ConsumeMutation,
        // Records successful ordinary put/CAS results; not durable-effect proof.
        // New custody semantics are tested at the actual persistent owner.
        puts: Mutex<Vec<SecretHandle>>,
    }
    #[async_trait]
    impl<S: SecretStore + ?Sized> SecretStore for ObservedStore<S> {
        async fn close_identity(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            expected: brassclaw_secrets::SecretIdentityClosure,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            self.inner.close_identity(scope, handle, expected).await
        }

        async fn put(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
        ) -> Result<SecretMetadata, SecretStoreError> {
            self.put_with_expiry(scope, handle, material, None).await
        }

        async fn put_with_expiry(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
            expires_at: Option<brassclaw_host_api::Timestamp>,
        ) -> Result<SecretMetadata, SecretStoreError> {
            let result = self
                .inner
                .put_with_expiry(scope, handle.clone(), material, expires_at)
                .await?;
            self.puts.lock().unwrap().push(handle);
            Ok(result)
        }
        async fn record_reference(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<Option<String>, brassclaw_secrets::SecretStoreError> {
            self.inner.record_reference(scope, handle).await
        }

        async fn put_if_record_reference(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
            expected: Option<&str>,
        ) -> Result<Option<brassclaw_secrets::SecretMetadata>, brassclaw_secrets::SecretStoreError>
        {
            let written = self
                .inner
                .put_if_record_reference(scope, handle.clone(), material, expected)
                .await?;
            if written.is_some() {
                self.puts.lock().unwrap().push(handle);
            }
            Ok(written)
        }

        async fn matches(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
            candidate: &SecretMaterial,
        ) -> Result<bool, brassclaw_secrets::SecretStoreError> {
            self.inner.matches(scope, handle, candidate).await
        }

        async fn put_original(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            material: SecretMaterial,
            context: serde_json::Value,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            self.inner
                .put_original(scope, handle, material, context)
                .await
        }

        async fn custody(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            self.inner.custody(scope, handle).await
        }

        async fn put_retained_from_original(
            &self,
            scope: ResourceScope,
            handle: SecretHandle,
            source: brassclaw_secrets::SecretCustodySource,
        ) -> Result<
            Option<brassclaw_secrets::SecretCustodyMetadata>,
            brassclaw_secrets::SecretStoreError,
        > {
            self.inner
                .put_retained_from_original(scope, handle, source)
                .await
        }

        async fn metadata(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<Option<SecretMetadata>, SecretStoreError> {
            self.inner.metadata(scope, handle).await
        }
        async fn delete(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<bool, SecretStoreError> {
            self.inner.delete(scope, handle).await
        }
        async fn lease_once(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
        ) -> Result<SecretLease, SecretStoreError> {
            self.inner.lease_once(scope, handle).await
        }

        async fn lease_once_if_record_reference(
            &self,
            scope: &ResourceScope,
            handle: &SecretHandle,
            expected: &str,
        ) -> Result<Option<brassclaw_secrets::SecretLease>, brassclaw_secrets::SecretStoreError>
        {
            self.inner
                .lease_once_if_record_reference(scope, handle, expected)
                .await
        }
        async fn consume(
            &self,
            scope: &ResourceScope,
            id: SecretLeaseId,
        ) -> Result<SecretMaterial, SecretStoreError> {
            let mutation = self.before_consume.lock().unwrap().take();
            if let Some((owner, handle, material)) = mutation {
                self.inner.put(owner, handle, material).await?;
            }
            self.inner.consume(scope, id).await
        }
        async fn revoke(
            &self,
            scope: &ResourceScope,
            id: SecretLeaseId,
        ) -> Result<SecretLease, SecretStoreError> {
            self.inner.revoke(scope, id).await
        }
        async fn leases_for_scope(
            &self,
            scope: &ResourceScope,
        ) -> Result<Vec<SecretLease>, SecretStoreError> {
            self.inner.leases_for_scope(scope).await
        }
    }
    mod generation_selection {
        use super::*;
        use brassclaw_auth::{
            CredentialAccount, CredentialAccountMutation, CredentialAccountService,
            CredentialAccountStatus, CredentialOwnership, CredentialRefreshAction,
            CredentialRefreshAdmission, CredentialRefreshCompletion, CredentialRefreshOutcomeKind,
            CredentialRefreshPrepared, CredentialRefreshRequest, CredentialSetupService,
            NewCredentialAccount, ProviderBackedCredentialAccountService,
        };
        use std::sync::atomic::{AtomicBool, Ordering};

        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        enum Race {
            BeforeAdmission,
            AfterAdmission,
            AfterAdmissionAba,
            BeforeConsume,
            LostBeginAck,
            Valid,
        }

        struct Fixture {
            setup: Arc<dyn CredentialSetupService>,
            accounts: Arc<dyn CredentialAccountService>,
            reopened_setup: Arc<dyn CredentialSetupService>,
            reopened_accounts: Arc<dyn CredentialAccountService>,
            store: Arc<dyn SecretStore>,
            before_consume: ConsumeMutation,
        }
        async fn fixture(pool: &Arc<brassclaw_pg::PgPool>, filesystem: bool) -> Fixture {
            use crate::pg_auth_product_services::{PgAuthProductServices, run_auth_migrations};
            run_auth_migrations(pool).await.unwrap();
            let hook = Arc::new(Mutex::new(None));
            if filesystem {
                use brassclaw_filesystem::{PostgresRootFilesystem, ScopedFilesystem};
                use brassclaw_host_api::{
                    MountAlias, MountGrant, MountPermissions, MountView, VirtualPath,
                };
                use brassclaw_secrets::{FilesystemSecretStore, SecretsCrypto};
                let root = Arc::new(PostgresRootFilesystem::new(pool.as_ref().clone()));
                root.run_migrations().await.unwrap();
                let view = MountView::new(vec![MountGrant::new(
                    MountAlias::new("/secrets").unwrap(),
                    VirtualPath::new("/tenants/test/users/refresh-generation/secrets").unwrap(),
                    MountPermissions::read_write_list_delete(),
                )])
                .unwrap();
                let first_fs = Arc::new(ScopedFilesystem::with_fixed_view(
                    root.clone(),
                    view.clone(),
                ));
                let second_fs = Arc::new(ScopedFilesystem::with_fixed_view(root, view));
                let crypto = Arc::new(
                    SecretsCrypto::new("native refresh generation filesystem master key".into())
                        .unwrap(),
                );
                let raw = Arc::new(FilesystemSecretStore::new(first_fs.clone(), crypto.clone()));
                let reopened = Arc::new(FilesystemSecretStore::new(second_fs.clone(), crypto));
                let first = Arc::new(
                    crate::product_auth_durable::FilesystemAuthProductServices::new(
                        first_fs,
                        raw.clone(),
                    ),
                );
                let second = Arc::new(
                    crate::product_auth_durable::FilesystemAuthProductServices::new(
                        second_fs, reopened,
                    ),
                );
                Fixture {
                    setup: first.clone(),
                    accounts: first,
                    reopened_setup: second.clone(),
                    reopened_accounts: second,
                    store: Arc::new(ObservedStore {
                        inner: raw,
                        puts: Mutex::new(vec![]),
                        before_consume: hook.clone(),
                    }),
                    before_consume: hook,
                }
            } else {
                let raw = Arc::new(
                    brassclaw_secrets::PgSecretStore::new(
                        pool.as_ref().clone(),
                        "native refresh generation postgres master key".into(),
                        "refresh-generation",
                    )
                    .unwrap(),
                );
                let reopened = Arc::new(
                    brassclaw_secrets::PgSecretStore::new(
                        pool.as_ref().clone(),
                        "native refresh generation postgres master key".into(),
                        "refresh-generation",
                    )
                    .unwrap(),
                );
                let first = Arc::new(PgAuthProductServices::new(pool.clone(), raw.clone()));
                let second = Arc::new(PgAuthProductServices::new(pool.clone(), reopened));
                Fixture {
                    setup: first.clone(),
                    accounts: first,
                    reopened_setup: second.clone(),
                    reopened_accounts: second,
                    store: Arc::new(ObservedStore {
                        inner: raw,
                        puts: Mutex::new(vec![]),
                        before_consume: hook.clone(),
                    }),
                    before_consume: hook,
                }
            }
        }

        struct RacingSetup {
            inner: Arc<dyn CredentialSetupService>,
            store: Arc<dyn SecretStore>,
            race: Race,
            once: AtomicBool,
        }
        #[async_trait]
        impl CredentialSetupService for RacingSetup {
            async fn refresh_action(
                &self,
                request: &CredentialRefreshRequest,
            ) -> Result<Option<CredentialRefreshAction>, AuthProductError> {
                self.inner.refresh_action(request).await
            }
            async fn reconcile_refresh(
                &self,
                request: &CredentialRefreshRequest,
            ) -> Result<Option<CredentialRefreshAction>, AuthProductError> {
                self.inner.reconcile_refresh(request).await
            }
            async fn begin_refresh(
                &self,
                prepared: CredentialRefreshPrepared,
            ) -> Result<CredentialRefreshAdmission, AuthProductError> {
                let source = prepared.preparation.as_ref().unwrap().source.clone();
                let first = self.once.swap(false, Ordering::SeqCst);
                if first && self.race == Race::BeforeAdmission {
                    self.store
                        .put(
                            source.scope.clone(),
                            source.handle.clone(),
                            "replacement-token".into(),
                        )
                        .await
                        .unwrap();
                }
                let result = self.inner.begin_refresh(prepared).await?;
                if first && matches!(&result, CredentialRefreshAdmission::Dispatch(_)) {
                    if matches!(self.race, Race::AfterAdmission | Race::AfterAdmissionAba) {
                        self.store
                            .put(
                                source.scope.clone(),
                                source.handle.clone(),
                                "replacement-token".into(),
                            )
                            .await
                            .unwrap();
                        if self.race == Race::AfterAdmissionAba {
                            self.store
                                .put(
                                    source.scope.clone(),
                                    source.handle.clone(),
                                    "original-token".into(),
                                )
                                .await
                                .unwrap();
                        }
                    }
                    if self.race == Race::LostBeginAck {
                        return Err(AuthProductError::BackendUnavailable);
                    }
                }
                Ok(result)
            }
            async fn complete_refresh(
                &self,
                completion: CredentialRefreshCompletion,
            ) -> Result<CredentialRefreshAction, AuthProductError> {
                self.inner.complete_refresh(completion).await
            }
            async fn compare_and_update_account(
                &self,
                expected: &CredentialAccount,
                request: NewCredentialAccount,
            ) -> Result<CredentialAccount, AuthProductError> {
                self.inner
                    .compare_and_update_account(expected, request)
                    .await
            }
            async fn create_or_update_account(
                &self,
                request: CredentialAccountMutation,
            ) -> Result<CredentialAccount, AuthProductError> {
                self.inner.create_or_update_account(request).await
            }
        }

        async fn verify(filesystem: bool) {
            let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
            let f = fixture(&rig.pool, filesystem).await;
            for race in [
                Race::BeforeAdmission,
                Race::AfterAdmission,
                Race::AfterAdmissionAba,
                Race::BeforeConsume,
                Race::LostBeginAck,
                Race::Valid,
            ] {
                let scope = AuthProductScope::new(sample_scope(), AuthSurface::Callback);
                let handle =
                    SecretHandle::new(format!("refresh-generation-{race:?}").to_ascii_lowercase())
                        .unwrap();
                f.store
                    .put(
                        scope.resource.clone(),
                        handle.clone(),
                        "original-token".into(),
                    )
                    .await
                    .unwrap();
                let reference = f
                    .store
                    .record_reference(&scope.resource, &handle)
                    .await
                    .unwrap()
                    .unwrap();
                let account = f
                    .accounts
                    .create_account(NewCredentialAccount {
                        scope: scope.clone(),
                        provider: AuthProviderId::new("notion").unwrap(),
                        label: CredentialAccountLabel::new(format!("generation {race:?}")).unwrap(),
                        status: CredentialAccountStatus::Configured,
                        ownership: CredentialOwnership::UserReusable,
                        owner_extension: None,
                        granted_extensions: vec![],
                        access_secret: None,
                        access_expires_at: None,
                        refresh_secret: Some(handle.clone()),
                        scopes: vec![],
                    })
                    .await
                    .unwrap();
                let request = CredentialRefreshRequest::new(
                    scope.clone(),
                    account.provider.clone(),
                    account.id,
                );
                if race == Race::BeforeConsume {
                    *f.before_consume.lock().unwrap() = Some((
                        scope.resource.clone(),
                        handle.clone(),
                        "replacement-token".into(),
                    ));
                }
                let egress = Arc::new(RecordingEgress::ok(
                    br#"{"access_token":"fresh-access","refresh_token":"rotated-refresh"}"#
                        .to_vec(),
                ));
                let provider = Arc::new(
                    HostOAuthProviderClient::new(
                        notion_spec(),
                        egress.clone(),
                        f.store.clone(),
                        Arc::new(NoopObligationHandler),
                        OAuthClientId::new("original-client").unwrap(),
                        OAuthRedirectUri::new("https://app.example/callback").unwrap(),
                    )
                    .unwrap(),
                );
                let wrapper = Arc::new(RacingSetup {
                    inner: f.setup.clone(),
                    store: f.store.clone(),
                    race,
                    once: AtomicBool::new(true),
                });
                let coordinator = ProviderBackedCredentialAccountService::new(
                    f.accounts.clone(),
                    wrapper,
                    provider,
                );
                let result = coordinator.refresh_account(request.clone()).await;
                if race == Race::BeforeAdmission {
                    assert!(
                        matches!(result, Err(AuthProductError::BackendConflict)),
                        "{race:?}"
                    );
                    assert!(f.setup.refresh_action(&request).await.unwrap().is_none());
                    assert!(egress.requests.lock().unwrap().is_empty());
                    continue;
                }
                if race == Race::LostBeginAck {
                    assert!(matches!(result, Err(AuthProductError::BackendUnavailable)));
                } else {
                    let report = result.unwrap();
                    assert_eq!(report.refreshed, race == Race::Valid, "{race:?}");
                    if race != Race::Valid {
                        assert_eq!(
                            report.original.unwrap().outcome,
                            CredentialRefreshOutcomeKind::Unconfirmed
                        );
                    }
                }
                let original = f.setup.refresh_action(&request).await.unwrap().unwrap();
                assert_eq!(
                    original.source.as_ref().unwrap().record_reference,
                    reference,
                    "{race:?}"
                );
                let count = egress.requests.lock().unwrap().len();
                assert_eq!(count, usize::from(race == Race::Valid), "{race:?}");
                if race == Race::Valid {
                    assert_eq!(
                        form_params(&egress.single_request().body)
                            .get("refresh_token")
                            .map(String::as_str),
                        Some("original-token")
                    );
                }
                // Removing/changing today's source cannot make an original read
                // reprepare or replay. Use independently constructed native owners
                // and a provider that cannot prepare or dispatch successfully.
                if matches!(race, Race::Valid | Race::LostBeginAck) {
                    assert!(f.store.delete(&scope.resource, &handle).await.unwrap());
                } else {
                    assert_ne!(
                        f.store
                            .record_reference(&scope.resource, &handle)
                            .await
                            .unwrap()
                            .as_deref(),
                        Some(reference.as_str())
                    );
                }
                let resumed = ProviderBackedCredentialAccountService::new(
                    f.reopened_accounts.clone(),
                    f.reopened_setup.clone(),
                    Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
                );
                for _ in 0..2 {
                    let report = resumed.refresh_account(request.clone()).await.unwrap();
                    assert_eq!(report.refreshed, race == Race::Valid, "{race:?}");
                    assert_eq!(report.original.unwrap().id, request.id);
                    assert_eq!(
                        egress.requests.lock().unwrap().len(),
                        count,
                        "no repeated issuer submission"
                    );
                }
                assert_eq!(
                    f.reopened_setup.refresh_action(&request).await.unwrap(),
                    Some(original)
                );
            }
        }
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn native_pg_refresh_generation_is_pinned_across_capture_claim_and_consumption() {
            verify(false).await;
        }
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn native_fs_refresh_generation_is_pinned_across_capture_claim_and_consumption() {
            verify(true).await;
        }
    }

    fn store(pool: &Arc<brassclaw_pg::PgPool>, namespace: &str) -> Arc<ObservedStore> {
        Arc::new(ObservedStore {
            inner: Arc::new(
                brassclaw_secrets::PgSecretStore::new(
                    pool.as_ref().clone(),
                    "controlled-oauth-refresh-artifact-master-key".into(),
                    namespace,
                )
                .unwrap(),
            ),
            puts: Mutex::new(Vec::new()),
            before_consume: Arc::new(Mutex::new(None)),
        })
    }
    async fn read(store: &dyn SecretStore, scope: &ResourceScope, handle: &SecretHandle) -> String {
        let lease = store.lease_once(scope, handle).await.unwrap();
        store
            .consume(scope, lease.id)
            .await
            .unwrap()
            .expose_secret()
            .to_string()
    }
    fn request(
        scope: &ResourceScope,
        account: CredentialAccountId,
        handle: &SecretHandle,
    ) -> OAuthProviderRefreshRequest {
        OAuthProviderRefreshRequest {
            refresh_id: brassclaw_auth::CredentialRefreshId::new(),
            provider: AuthProviderId::new("google").unwrap(),
            scope: AuthProductScope::new(scope.clone(), AuthSurface::Callback),
            account_id: account,
            refresh_secret: handle.clone(),
            scopes: vec![],
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_cleanup_rejection_remains_an_error_and_attempts_other_owned_cleanup() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let store = store(&rig.pool, "oauth-cleanup-rejected");
        rig.pool.get().await.unwrap().batch_execute("CREATE FUNCTION reject_unpublished_access_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.name LIKE '%-oauth-access-%' THEN RAISE EXCEPTION 'controlled unpublished cleanup rejection'; END IF; RETURN OLD; END $$; CREATE TRIGGER reject_unpublished_access_delete BEFORE DELETE ON brassclaw_secrets FOR EACH ROW EXECUTE FUNCTION reject_unpublished_access_delete();").await.unwrap();
        let context = exchange_context();
        let scope = context.scope.resource.clone();
        let mut source = RecordingMaterialSource::new();
        source.fail_initial_binding = true;
        let source = Arc::new(source);
        let client = HostOAuthProviderClient::new_with_client_material(
            notion_spec(),
            Arc::new(RecordingEgress::ok(
                br#"{"access_token":"unpublished-access","refresh_token":"unpublished-refresh"}"#
                    .to_vec(),
            )),
            store.clone(),
            Arc::new(NoopObligationHandler),
            source.clone(),
        )
        .unwrap();
        let error = client
            .exchange_callback(context, callback_request("notion", "notion", &[]))
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            brassclaw_auth::AuthErrorCode::BackendUnavailable
        );
        let written = { store.puts.lock().unwrap().clone() };
        assert_eq!(written.len(), 2);
        for handle in &written {
            let retained = store.metadata(&scope, handle).await.unwrap().is_some();
            assert_eq!(retained, handle.as_str().contains("-oauth-access-"));
        }
        assert_eq!(source.cleaned_refreshes.lock().unwrap().len(), 1);
        assert_eq!(source.cleaned_flows.lock().unwrap().len(), 1);
        rig.pool.get().await.unwrap().batch_execute("DROP TRIGGER reject_unpublished_access_delete ON brassclaw_secrets; DROP FUNCTION reject_unpublished_access_delete();").await.unwrap();
        for handle in written {
            store.delete(&scope, &handle).await.unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_refresh_results_do_not_overwrite_another_completion_or_original_secret() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        let store = store(&rig.pool, "oauth-refresh-immutable");
        let scope = sample_scope();
        let original = SecretHandle::new("original-refresh-input").unwrap();
        store
            .put(
                scope.clone(),
                original.clone(),
                SecretMaterial::from("original-refresh-token"),
            )
            .await
            .unwrap();
        let account = CredentialAccountId::new();
        let mut outputs = Vec::new();
        for (access, refresh) in [
            ("first-access", "first-refresh"),
            ("second-access", "second-refresh"),
        ] {
            let egress = Arc::new(RecordingEgress::ok(
                serde_json::to_vec(
                    &serde_json::json!({"access_token":access,"refresh_token":refresh}),
                )
                .unwrap(),
            ));
            let client = HostOAuthProviderClient::new(
                google_spec(),
                egress.clone(),
                store.clone(),
                Arc::new(NoopObligationHandler),
                OAuthClientId::new("google-client").unwrap(),
                OAuthRedirectUri::new("https://app.example/callback").unwrap(),
            )
            .unwrap();
            let result = prepared_refresh(&client, request(&scope, account, &original))
                .await
                .unwrap();
            assert_eq!(egress.requests.lock().unwrap().len(), 1);
            outputs.push(result);
        }
        assert_ne!(outputs[0].access_secret, outputs[1].access_secret);
        assert_ne!(outputs[0].refresh_secret, outputs[1].refresh_secret);
        assert_eq!(
            read(store.as_ref(), &scope, &outputs[0].access_secret).await,
            "first-access"
        );
        assert_eq!(
            read(
                store.as_ref(),
                &scope,
                outputs[0].refresh_secret.as_ref().unwrap()
            )
            .await,
            "first-refresh"
        );
        assert_eq!(
            read(store.as_ref(), &scope, &outputs[1].access_secret).await,
            "second-access"
        );
        assert_eq!(
            read(store.as_ref(), &scope, &original).await,
            "original-refresh-token"
        );
        let source = Arc::new(RecordingMaterialSource::new());
        let client = HostOAuthProviderClient::new_with_client_material(
            notion_spec(),
            Arc::new(RecordingEgress::ok(
                br#"{"access_token":"access-without-rotation"}"#.to_vec(),
            )),
            store.clone(),
            Arc::new(NoopObligationHandler),
            source.clone(),
        )
        .unwrap();
        let without_rotation = prepared_refresh(
            &client,
            OAuthProviderRefreshRequest {
                refresh_id: brassclaw_auth::CredentialRefreshId::new(),
                provider: AuthProviderId::new("notion").unwrap(),
                scope: AuthProductScope::new(scope.clone(), AuthSurface::Callback),
                account_id: account,
                refresh_secret: original.clone(),
                scopes: vec![],
            },
        )
        .await
        .unwrap();
        assert!(without_rotation.refresh_secret.is_none());
        assert!(source.bound_rotations.lock().unwrap().is_empty());
        assert_eq!(
            read(store.as_ref(), &scope, &original).await,
            "original-refresh-token"
        );
        // Controlled issuer replies test artifact custody, not remote replay safety.
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_refresh_omission_clears_old_access_expiry_and_preserves_original_refresh_token()
    {
        use crate::pg_auth_product_services::{PgAuthProductServices, run_auth_migrations};
        use brassclaw_auth::{
            CredentialAccountLookupRequest, CredentialAccountService, CredentialAccountStatus,
            CredentialOwnership, CredentialRefreshRequest, NewCredentialAccount,
            ProviderBackedCredentialAccountService,
        };
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let secrets = store(&rig.pool, "native-refresh-expiry-omission");
        let resource = sample_scope();
        let access = SecretHandle::new("old-expired-access").unwrap();
        let refresh = SecretHandle::new("old-live-refresh").unwrap();
        let old_deadline = chrono::Utc::now() - chrono::Duration::seconds(1);
        secrets
            .put_with_expiry(
                resource.clone(),
                access.clone(),
                "old access material".into(),
                Some(old_deadline),
            )
            .await
            .unwrap();
        secrets
            .put(
                resource.clone(),
                refresh.clone(),
                "live original refresh material".into(),
            )
            .await
            .unwrap();
        let backend = Arc::new(PgAuthProductServices::new(
            rig.pool.clone(),
            secrets.inner.clone(),
        ));
        let account = backend
            .create_account(NewCredentialAccount {
                scope: AuthProductScope::new(resource.clone(), AuthSurface::Callback),
                provider: AuthProviderId::new("google").unwrap(),
                label: CredentialAccountLabel::new("expired access, valid refresh").unwrap(),
                status: CredentialAccountStatus::Configured,
                ownership: CredentialOwnership::UserReusable,
                owner_extension: None,
                granted_extensions: vec![],
                access_secret: Some(access.clone()),
                access_expires_at: Some(old_deadline),
                refresh_secret: Some(refresh.clone()),
                scopes: vec![],
            })
            .await
            .unwrap();
        assert_eq!(
            account.projection().status,
            CredentialAccountStatus::Expired
        );
        let egress = Arc::new(RecordingEgress::ok(
            br#"{"access_token":"renewed access material"}"#.to_vec(),
        ));
        let provider = Arc::new(
            HostOAuthProviderClient::new(
                google_spec(),
                egress.clone(),
                secrets.clone(),
                Arc::new(NoopObligationHandler),
                OAuthClientId::new("google-client").unwrap(),
                OAuthRedirectUri::new("https://app.example/callback").unwrap(),
            )
            .unwrap(),
        );
        let coordinator =
            ProviderBackedCredentialAccountService::new(backend.clone(), backend.clone(), provider);
        let original = CredentialRefreshRequest::new(
            account.scope.clone(),
            account.provider.clone(),
            account.id,
        );
        let report = coordinator.refresh_account(original.clone()).await.unwrap();
        assert!(report.refreshed);
        let current = backend
            .get_account(CredentialAccountLookupRequest::new(
                account.scope.clone(),
                account.id,
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.access_expires_at, None);
        assert_eq!(current.refresh_secret.as_ref(), Some(&refresh));
        assert_eq!(
            current.projection().status,
            CredentialAccountStatus::Configured
        );
        let selected = secrets
            .lease_once(&resource, current.access_secret.as_ref().unwrap())
            .await
            .unwrap();
        assert_eq!(selected.secret_expires_at, None);
        assert_eq!(
            secrets
                .consume(&resource, selected.id)
                .await
                .unwrap()
                .expose_secret(),
            "renewed access material"
        );
        assert!(
            secrets
                .lease_once(&resource, &access)
                .await
                .unwrap_err()
                .is_expired()
        );
        assert!(
            coordinator
                .refresh_account(original)
                .await
                .unwrap()
                .refreshed
        );
        assert_eq!(
            egress.requests.lock().unwrap().len(),
            1,
            "readback must not repeat the issuer refresh"
        );
        assert_eq!(
            read(secrets.as_ref(), &resource, &refresh).await,
            "live original refresh material"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_failed_initial_or_rotated_binding_cleans_all_unpublished_tokens() {
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        for initial in [true, false] {
            let store = store(&rig.pool, &format!("oauth-bind-failure-{initial}"));
            let context = exchange_context();
            let scope = context.scope.resource.clone();
            let original = SecretHandle::new("original-bound-refresh").unwrap();
            store
                .put(
                    scope.clone(),
                    original.clone(),
                    SecretMaterial::from("retained-original-refresh"),
                )
                .await
                .unwrap();
            store.puts.lock().unwrap().clear();
            let mut source = RecordingMaterialSource::new();
            source.fail_initial_binding = initial;
            source.fail_rotated_binding = !initial;
            let source = Arc::new(source);
            let client = HostOAuthProviderClient::new_with_client_material(notion_spec(), Arc::new(RecordingEgress::ok(br#"{"access_token":"unpublished-access","refresh_token":"unpublished-refresh"}"#.to_vec())), store.clone(), Arc::new(NoopObligationHandler), source.clone()).unwrap();
            let error = if initial {
                client
                    .exchange_callback(context, callback_request("notion", "notion", &[]))
                    .await
                    .unwrap_err()
            } else {
                prepared_refresh(
                    &client,
                    OAuthProviderRefreshRequest {
                        refresh_id: brassclaw_auth::CredentialRefreshId::new(),
                        provider: AuthProviderId::new("notion").unwrap(),
                        scope: AuthProductScope::new(scope.clone(), AuthSurface::Callback),
                        account_id: CredentialAccountId::new(),
                        refresh_secret: original.clone(),
                        scopes: vec![],
                    },
                )
                .await
                .unwrap_err()
            };
            assert_eq!(
                error.code(),
                brassclaw_auth::AuthErrorCode::BackendUnavailable
            );
            let written = { store.puts.lock().unwrap().clone() };
            assert_eq!(written.len(), 2);
            for handle in written {
                assert!(store.metadata(&scope, &handle).await.unwrap().is_none());
            }
            assert_eq!(
                read(store.as_ref(), &scope, &original).await,
                "retained-original-refresh"
            );
            assert_eq!(source.cleaned_refreshes.lock().unwrap().len(), 1);
            assert_eq!(
                source.cleaned_flows.lock().unwrap().len(),
                usize::from(initial)
            );
        }
    }

    mod original_refresh_custody {
        use super::*;
        use crate::pg_auth_product_services::{PgAuthProductServices, run_auth_migrations};
        use brassclaw_auth::{
            CredentialAccount, CredentialAccountLookupRequest, CredentialAccountMutation,
            CredentialAccountService, CredentialAccountStatus, CredentialOwnership,
            CredentialRefreshAction, CredentialRefreshAdmission, CredentialRefreshCompletion,
            CredentialRefreshPrepared, CredentialRefreshRequest, CredentialSetupService,
            NewCredentialAccount, ProviderBackedCredentialAccountService,
        };
        use std::sync::atomic::{AtomicBool, Ordering};

        /// Drop one acknowledgement AFTER the real selected backend commits.
        /// No synthetic success and no production fault hook is introduced.
        struct LostRefreshAck {
            inner: Arc<PgAuthProductServices>,
            at_begin: bool,
            lose: AtomicBool,
        }
        #[async_trait]
        impl CredentialSetupService for LostRefreshAck {
            async fn refresh_action(
                &self,
                request: &CredentialRefreshRequest,
            ) -> Result<Option<CredentialRefreshAction>, AuthProductError> {
                self.inner.refresh_action(request).await
            }
            async fn begin_refresh(
                &self,
                prepared: CredentialRefreshPrepared,
            ) -> Result<CredentialRefreshAdmission, AuthProductError> {
                let result = self.inner.begin_refresh(prepared).await?;
                if self.at_begin && self.lose.swap(false, Ordering::SeqCst) {
                    return Err(AuthProductError::BackendUnavailable);
                }
                Ok(result)
            }
            async fn reconcile_refresh(
                &self,
                request: &brassclaw_auth::CredentialRefreshRequest,
            ) -> Result<Option<brassclaw_auth::CredentialRefreshAction>, AuthProductError>
            {
                self.inner.reconcile_refresh(request).await
            }

            async fn complete_refresh(
                &self,
                completion: CredentialRefreshCompletion,
            ) -> Result<CredentialRefreshAction, AuthProductError> {
                let result = self.inner.complete_refresh(completion).await?;
                if !self.at_begin && self.lose.swap(false, Ordering::SeqCst) {
                    return Err(AuthProductError::BackendUnavailable);
                }
                Ok(result)
            }
            async fn compare_and_update_account(
                &self,
                expected: &CredentialAccount,
                request: NewCredentialAccount,
            ) -> Result<CredentialAccount, AuthProductError> {
                self.inner
                    .compare_and_update_account(expected, request)
                    .await
            }
            async fn create_or_update_account(
                &self,
                request: CredentialAccountMutation,
            ) -> Result<CredentialAccount, AuthProductError> {
                self.inner.create_or_update_account(request).await
            }
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn native_coordinator_reconciles_lost_ack_or_failed_recovery_without_another_issuer_call()
         {
            let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
            run_auth_migrations(&rig.pool).await.unwrap();
            for failure in ["claim_ack", "result_ack", "recovery_lookup"] {
                let at_begin = failure == "claim_ack";
                let store = store(&rig.pool, &format!("original-refresh-ack-{failure}"));
                let scope = sample_scope();
                let refresh = SecretHandle::new(format!("original-refresh-ack-{failure}")).unwrap();
                store
                    .put(
                        scope.clone(),
                        refresh.clone(),
                        SecretMaterial::from("retained-original-refresh-input"),
                    )
                    .await
                    .unwrap();
                let backend = Arc::new(PgAuthProductServices::new(
                    rig.pool.clone(),
                    store.inner.clone(),
                ));
                let account = backend
                    .create_account(NewCredentialAccount {
                        scope: AuthProductScope::new(scope.clone(), AuthSurface::Callback),
                        provider: AuthProviderId::new("google").unwrap(),
                        label: CredentialAccountLabel::new("native original refresh").unwrap(),
                        status: CredentialAccountStatus::Expired,
                        ownership: CredentialOwnership::UserReusable,
                        owner_extension: None,
                        granted_extensions: vec![],
                        access_secret: None,
                        access_expires_at: None,
                        refresh_secret: Some(refresh.clone()),
                        scopes: vec![],
                    })
                    .await
                    .unwrap();
                let egress = Arc::new(RecordingEgress::ok(
                    br#"{"access_token":"published-access","refresh_token":"published-rotation"}"#
                        .to_vec(),
                ));
                let provider = Arc::new(
                    HostOAuthProviderClient::new(
                        google_spec(),
                        egress.clone(),
                        store.clone(),
                        Arc::new(NoopObligationHandler),
                        OAuthClientId::new("google-client").unwrap(),
                        OAuthRedirectUri::new("https://app.example/callback").unwrap(),
                    )
                    .unwrap(),
                );
                let corrupt_id = uuid::Uuid::new_v4().to_string();
                if failure == "recovery_lookup" {
                    // An actual bad aggregate row rejects recovery decoding,
                    // while exact original account/intent queries remain valid.
                    let invalid = serde_json::json!({"controlled_invalid_account":true});
                    rig.pool.get().await.unwrap().execute(
                        "INSERT INTO brassclaw_product_auth_accounts (tenant_id,id,user_id,data) VALUES ($1,$2,$3,$4)",
                        &[&scope.tenant_id.as_str(),&corrupt_id,&scope.user_id.as_str(),&invalid],
                    ).await.unwrap();
                }
                let wrapper = ProviderBackedCredentialAccountService::new(
                    backend.clone(),
                    Arc::new(LostRefreshAck {
                        inner: backend.clone(),
                        at_begin,
                        lose: AtomicBool::new(failure != "recovery_lookup"),
                    }),
                    provider.clone(),
                );
                let request = CredentialRefreshRequest::new(
                    account.scope.clone(),
                    account.provider.clone(),
                    account.id,
                );
                assert!(matches!(
                    wrapper.refresh_account(request.clone()).await,
                    Err(AuthProductError::BackendUnavailable)
                ));
                assert_eq!(
                    egress.requests.lock().unwrap().len(),
                    usize::from(!at_begin)
                );
                if failure == "recovery_lookup" {
                    let original = backend.refresh_action(&request).await.unwrap().unwrap();
                    assert!(original.receipt().unwrap().confirmed_refresh());
                    rig.pool.get().await.unwrap().execute(
                        "DELETE FROM brassclaw_product_auth_accounts WHERE tenant_id=$1 AND id=$2",
                        &[&scope.tenant_id.as_str(),&corrupt_id],
                    ).await.unwrap();
                }
                drop(wrapper);
                drop(backend);
                let reopened = Arc::new(PgAuthProductServices::new(
                    rig.pool.clone(),
                    store.inner.clone(),
                ));
                let restarted = ProviderBackedCredentialAccountService::new(
                    reopened.clone(),
                    reopened.clone(),
                    provider,
                );
                let original = restarted.refresh_account(request.clone()).await.unwrap();
                assert_eq!(original.original.as_ref().unwrap().id, request.id);
                assert_eq!(original.refreshed, !at_begin);
                if at_begin {
                    assert_eq!(
                        original.original.as_ref().unwrap().publication,
                        brassclaw_auth::CredentialRefreshPublication::Pending
                    );
                    let replacement = CredentialRefreshRequest::new(
                        account.scope.clone(),
                        account.provider.clone(),
                        account.id,
                    );
                    assert!(matches!(
                        restarted.refresh_account(replacement).await,
                        Err(AuthProductError::BackendConflict)
                    ));
                } else {
                    let published = reopened
                        .get_account(CredentialAccountLookupRequest::new(
                            account.scope.clone(),
                            account.id,
                        ))
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        read(
                            store.as_ref(),
                            &scope,
                            published.access_secret.as_ref().unwrap()
                        )
                        .await,
                        "published-access"
                    );
                    assert_eq!(
                        read(
                            store.as_ref(),
                            &scope,
                            published.refresh_secret.as_ref().unwrap()
                        )
                        .await,
                        "published-rotation"
                    );
                    assert_eq!(
                        original.original.as_ref().unwrap().published_account_ref,
                        Some(published.snapshot_ref().unwrap())
                    );
                }
                assert_eq!(restarted.refresh_account(request).await.unwrap(), original);
                assert_eq!(
                    egress.requests.lock().unwrap().len(),
                    usize::from(!at_begin)
                );
                assert_eq!(
                    read(store.as_ref(), &scope, &refresh).await,
                    "retained-original-refresh-input"
                );
            }
        }
    }

    #[derive(Default)]
    struct CallbackContinuations(Mutex<Vec<brassclaw_auth::AuthContinuationEvent>>);

    #[async_trait]
    impl crate::auth::RebornAuthContinuationDispatcher for CallbackContinuations {
        async fn dispatch_auth_continuation(
            &self,
            event: brassclaw_auth::AuthContinuationEvent,
        ) -> Result<(), AuthProductError> {
            self.0.lock().unwrap().push(event);
            Ok(())
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_callback_facade_retains_encrypted_host_result_and_recovers_without_provider_or_pkce()
     {
        use crate::auth::{
            RebornOAuthCallbackOutcome, RebornOAuthCallbackRequest, RebornProductAuthServices,
        };
        use crate::pg_auth_product_services::{PgAuthProductServices, run_auth_migrations};
        use brassclaw_auth::{
            AuthChallenge, AuthContinuationRef, AuthFlowKind, AuthFlowManager, AuthFlowStatus,
            NewAuthFlow, OAuthAuthorizationUrl, OAuthCallbackOutcome, OAuthCallbackPublication,
        };
        let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
        run_auth_migrations(&rig.pool).await.unwrap();
        let secrets = store(&rig.pool, "native-callback-original");
        let backend = Arc::new(PgAuthProductServices::new(
            rig.pool.clone(),
            secrets.inner.clone(),
        ));
        let scope = AuthProductScope::new(sample_scope(), AuthSurface::Callback);
        let request = callback_request("google", "original callback", &["gmail.readonly"]);
        let code_hash = request.authorization_code_hash.clone();
        let pkce_hash = request.pkce_verifier_hash.clone();
        let opaque_hash =
            brassclaw_auth::opaque_state_hash("native-original-callback-state").unwrap();
        let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
        let flow = backend
            .create_flow(NewAuthFlow {
                requirement_setup: None,
                id: None,
                scope: scope.clone(),
                kind: AuthFlowKind::IntegrationCredential,
                provider: request.provider.clone(),
                challenge: AuthChallenge::OAuthUrl {
                    authorization_url: OAuthAuthorizationUrl::new(
                        "https://issuer.example/authorize",
                    )
                    .unwrap(),
                    expires_at,
                },
                continuation: AuthContinuationRef::SetupOnly,
                update_binding: None,
                opaque_state_hash: Some(opaque_hash.clone()),
                pkce_verifier_hash: Some(pkce_hash),
                expires_at,
            })
            .await
            .unwrap();
        let egress = Arc::new(RecordingEgress::ok(br#"{"access_token":"original-callback-access-token","refresh_token":"original-callback-refresh-token","scope":"gmail.readonly","expires_in":3600}"#.to_vec()));
        let provider = Arc::new(
            HostOAuthProviderClient::new(
                google_spec(),
                egress.clone(),
                secrets.clone(),
                Arc::new(NoopObligationHandler),
                OAuthClientId::new("google-client").unwrap(),
                OAuthRedirectUri::new("https://app.example/callback").unwrap(),
            )
            .unwrap(),
        );
        let dispatcher = Arc::new(CallbackContinuations::default());
        let facade = RebornProductAuthServices::new(
            backend.clone(),
            backend.clone(),
            backend.clone(),
            backend.clone(),
            provider.clone(),
            backend.clone(),
            dispatcher.clone(),
        );
        for field in ["code", "pkce"] {
            for boundary in ["host", "facade"] {
                let mut bad = callback_request("google", "original callback", &["gmail.readonly"]);
                if field == "code" {
                    bad.authorization_code_hash =
                        AuthorizationCodeHash::new("f".repeat(64)).unwrap();
                } else {
                    bad.pkce_verifier_hash = PkceVerifierHash::new("f".repeat(64)).unwrap();
                }
                let code = if boundary == "host" {
                    provider
                        .exchange_callback(
                            OAuthProviderExchangeContext {
                                scope: scope.clone(),
                                flow_id: flow.id,
                            },
                            bad,
                        )
                        .await
                        .unwrap_err()
                        .code()
                } else {
                    facade
                        .handle_oauth_callback(RebornOAuthCallbackRequest {
                            scope: scope.clone(),
                            flow_id: flow.id,
                            opaque_state_hash: opaque_hash.clone(),
                            outcome: RebornOAuthCallbackOutcome::Authorized {
                                provider_request: bad,
                            },
                        })
                        .await
                        .unwrap_err()
                        .code
                };
                assert_eq!(code, brassclaw_auth::AuthErrorCode::MalformedCallback);
                assert!(egress.requests.lock().unwrap().is_empty());
                assert!(secrets.puts.lock().unwrap().is_empty());
                assert_eq!(
                    backend.get_flow(&scope, flow.id).await.unwrap().unwrap(),
                    flow
                );
            }
        }
        let client = rig.pool.get().await.unwrap();
        client.batch_execute("CREATE FUNCTION reject_host_callback_publication() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'controlled actual Host callback publication failure'; END $$; CREATE TRIGGER reject_host_callback_publication BEFORE INSERT ON brassclaw_product_auth_accounts FOR EACH ROW EXECUTE FUNCTION reject_host_callback_publication();").await.unwrap();
        let first = facade
            .handle_oauth_callback(RebornOAuthCallbackRequest {
                scope: scope.clone(),
                flow_id: flow.id,
                opaque_state_hash: opaque_hash.clone(),
                outcome: RebornOAuthCallbackOutcome::Authorized {
                    provider_request: request,
                },
            })
            .await
            .unwrap_err();
        assert_eq!(
            first.code,
            brassclaw_auth::AuthErrorCode::BackendUnavailable
        );
        assert_eq!(egress.requests.lock().unwrap().len(), 1);
        assert_eq!(secrets.puts.lock().unwrap().len(), 2);
        assert!(dispatcher.0.lock().unwrap().is_empty());
        let reference = brassclaw_auth::OAuthCallbackReference {
            scope: scope.clone(),
            flow_id: flow.id,
            provider: AuthProviderId::new("google").unwrap(),
            opaque_state_hash: opaque_hash.clone(),
            authorization_code_hash: code_hash,
            account_label: CredentialAccountLabel::new("original callback").unwrap(),
            requested_scopes: vec![ProviderScope::new("gmail.readonly").unwrap()],
        };
        let retained = backend
            .read_oauth_callback_original(&scope, &reference)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.flow.status, AuthFlowStatus::Completing);
        assert_eq!(
            retained.action.publication,
            OAuthCallbackPublication::Pending
        );
        assert!(matches!(
            retained.action.outcome,
            OAuthCallbackOutcome::Returned(_)
        ));
        let OAuthCallbackOutcome::Returned(original_result) = &retained.action.outcome else {
            panic!("actual retained Host result");
        };
        let original_deadline = original_result.access_expires_at.unwrap();
        let selected = secrets
            .lease_once(&scope.resource, &original_result.access_secret)
            .await
            .unwrap();
        assert_eq!(selected.secret_expires_at, Some(original_deadline));
        secrets.revoke(&scope.resource, selected.id).await.unwrap();
        assert_eq!(
            read(
                secrets.as_ref(),
                &scope.resource,
                &retained.action.destinations.access_secret
            )
            .await,
            "original-callback-access-token"
        );
        assert_eq!(
            read(
                secrets.as_ref(),
                &scope.resource,
                &retained.action.destinations.refresh_secret
            )
            .await,
            "original-callback-refresh-token"
        );
        client.batch_execute("DROP TRIGGER reject_host_callback_publication ON brassclaw_product_auth_accounts; DROP FUNCTION reject_host_callback_publication();").await.unwrap();
        let recovered_secrets = store(&rig.pool, "native-callback-original");
        let recovered = Arc::new(PgAuthProductServices::new(
            rig.pool.clone(),
            recovered_secrets.inner.clone(),
        ));
        let resumed = RebornProductAuthServices::new(
            recovered.clone(),
            recovered.clone(),
            recovered.clone(),
            recovered.clone(),
            Arc::new(crate::product_auth_durable::UnavailableAuthProviderClient),
            recovered.clone(),
            dispatcher.clone(),
        );
        // Reference-only recovery has neither raw code/verifier nor a usable
        // current provider. The exact original result is the only input.
        let response = resumed
            .resume_oauth_callback_original(&reference)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status, AuthFlowStatus::Completed);
        assert_eq!(dispatcher.0.lock().unwrap().len(), 1);
        let repeated = resumed
            .resume_oauth_callback_original(&reference)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(repeated, response);
        assert_eq!(dispatcher.0.lock().unwrap().len(), 1);
        assert!(recovered_secrets.puts.lock().unwrap().is_empty());
        assert_eq!(egress.requests.lock().unwrap().len(), 1);
        let final_state = recovered
            .read_oauth_callback_original(&scope, &reference)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            final_state.action.destinations,
            retained.action.destinations
        );
        assert_eq!(
            final_state
                .action
                .published_account
                .as_ref()
                .unwrap()
                .access_expires_at,
            Some(original_deadline),
            "reference-only recovery must publish the original deadline, not a restarted lifetime"
        );
        assert_eq!(
            read(
                recovered_secrets.as_ref(),
                &scope.resource,
                &retained.action.destinations.access_secret
            )
            .await,
            "original-callback-access-token"
        );
        let mut changed = reference.clone();
        changed.authorization_code_hash = AuthorizationCodeHash::new("f".repeat(64)).unwrap();
        let error = resumed
            .resume_oauth_callback_original(&changed)
            .await
            .unwrap_err();
        assert_eq!(
            error.code,
            brassclaw_auth::AuthErrorCode::BackendUnavailable
        );
        assert!(error.conflict);
        assert!(!error.retryable);
        // Stable sanitized code plus explicit disposition preserves conflict
        // without serializing backend reasons, secrets or the original payload.
        assert_eq!(egress.requests.lock().unwrap().len(), 1);
        assert_eq!(dispatcher.0.lock().unwrap().len(), 1);
        let serialized = serde_json::to_string(&response).unwrap();
        for secret in [
            "raw-auth-code",
            "raw-pkce-verifier",
            "original-callback-access-token",
            "original-callback-refresh-token",
        ] {
            assert!(!serialized.contains(secret));
        }
    }
}

// Existing direct transport tests prepare actual encrypted source metadata. New
// race cases below keep preparation/dispatch apart to exercise the boundary.
async fn prepared_refresh(
    client: &dyn AuthProviderClient,
    request: OAuthProviderRefreshRequest,
) -> Result<OAuthProviderRefresh, AuthProductError> {
    let preparation = client.prepare_refresh(&request).await?;
    client.refresh_token(request, preparation).await
}

#[cfg(feature = "skills-db")]
mod acknowledged_transport {
    use super::*;
    use brassclaw_host_runtime::{BuiltinObligationServices, HostPrimitiveSettingsSource};
    use brassclaw_product_workflow::MontyMemoryMode;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::Notify;

    /// Observe only non-secret bounds, then forward the actual request to the
    /// production host/policy/Reqwest path. No response or Tool success is faked.
    #[derive(Debug, PartialEq, Eq)]
    struct ObservedBounds {
        timeout_millis: Option<u32>,
        body_bytes: Option<u64>,
        policy_bytes: Option<u64>,
    }
    struct ObservedEgress {
        inner: Arc<dyn RuntimeHttpEgress>,
        bounds: Mutex<Vec<ObservedBounds>>,
        pause_once: AtomicBool,
        entered: Notify,
        release: Notify,
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
            self.bounds.lock().unwrap().push(ObservedBounds {
                timeout_millis: request.timeout_ms,
                body_bytes: request.response_body_limit,
                policy_bytes: request.network_policy.max_egress_bytes,
            });
            if self.pause_once.swap(false, Ordering::AcqRel) {
                self.entered.notify_one();
                self.release.notified().await;
            }
            self.inner.execute(request).await
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_oauth_callback_refresh_use_acknowledged_bounds_and_retain_inflight_pair() {
        use crate::{RebornRuntimeIdentity, RebornRuntimeInput, build_reborn_runtime};
        let rig = crate::runtime::test_pg::pg_rig().await;
        let _db = rig.lock_db().await;
        rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
        let directory = tempfile::tempdir().unwrap();
        let runtime = Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                rig.build_input("oauth-limit-owner", directory.path())
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
        let obligations = BuiltinObligationServices::new(
            Arc::new(brassclaw_events::InMemoryAuditSink::new()),
            secrets.clone(),
            Arc::new(brassclaw_resources::InMemoryResourceGovernor::new()),
        );
        let egress = Arc::new(ObservedEgress {
            inner: Arc::new(obligations.host_http_egress(
                brassclaw_network::PolicyNetworkHttpEgress::new(
                    brassclaw_network::ReqwestNetworkTransport::default(),
                ),
            )),
            bounds: Mutex::new(Vec::new()),
            pause_once: AtomicBool::new(true),
            entered: Notify::new(),
            release: Notify::new(),
        });
        let spec = HostOAuthProviderSpec {
            token_endpoint: "https://127.0.0.1:9/token",
            ..notion_spec()
        };
        let client = Arc::new(
            HostOAuthProviderClient::new(
                spec,
                egress.clone(),
                secrets.clone(),
                Arc::new(obligations.obligation_handler()),
                OAuthClientId::new("original-client").unwrap(),
                OAuthRedirectUri::new("https://app.example/callback").unwrap(),
            )
            .unwrap()
            .with_primitive_settings_source(Some(source.clone())),
        );
        let mut first_limits = original.execution_limits;
        first_limits.oauth_token_timeout_millis = 47000;
        first_limits.oauth_token_response_body_bytes = 32769;
        let first=settings.upsert("default","default",&serde_json::from_value(serde_json::json!({"expected_revision":original.revision,"execution_limits":first_limits})).unwrap()).await.unwrap();
        assert_eq!(source.snapshot().await.unwrap().revision, first.revision);
        let pending = {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .exchange_callback(
                        exchange_context(),
                        callback_request("notion", "original", &["workspace"]),
                    )
                    .await
            })
        };
        tokio::time::timeout(std::time::Duration::from_secs(5), egress.entered.notified())
            .await
            .unwrap();
        let mut second_limits = first_limits;
        second_limits.oauth_token_timeout_millis = 51000;
        second_limits.oauth_token_response_body_bytes = 65539;
        let second=settings.upsert("default","default",&serde_json::from_value(serde_json::json!({"expected_revision":first.revision,"execution_limits":second_limits})).unwrap()).await.unwrap();
        let status = settings
            .runtime_status()
            .await
            .unwrap()
            .unwrap()
            .execution_limits
            .unwrap();
        assert_eq!(status.effective_revision, second.revision);
        assert_eq!(status.limits.oauth_token_timeout_millis, 51000);
        assert_eq!(status.limits.oauth_token_response_body_bytes, 65539);
        egress.release.notify_one();
        assert!(
            pending.await.unwrap().is_err(),
            "real policy must deny private issuer endpoint"
        );
        let refresh_secret = SecretHandle::new("actual-original-refresh-for-live-limit").unwrap();
        let scope = sample_scope();
        secrets
            .put(
                scope.clone(),
                refresh_secret.clone(),
                SecretString::from("original-refresh-value"),
            )
            .await
            .unwrap();
        assert!(
            prepared_refresh(
                client.as_ref(),
                OAuthProviderRefreshRequest {
                    refresh_id: brassclaw_auth::CredentialRefreshId::new(),
                    scope: AuthProductScope::new(scope, AuthSurface::Callback),
                    provider: AuthProviderId::new("notion").unwrap(),
                    account_id: CredentialAccountId::new(),
                    refresh_secret,
                    scopes: vec![ProviderScope::new("workspace").unwrap()],
                }
            )
            .await
            .is_err(),
            "real policy must also deny private refresh endpoint"
        );
        assert_eq!(
            *egress.bounds.lock().unwrap(),
            vec![
                ObservedBounds {
                    timeout_millis: Some(47000),
                    body_bytes: Some(32769),
                    policy_bytes: Some(32769)
                },
                ObservedBounds {
                    timeout_millis: Some(51000),
                    body_bytes: Some(65539),
                    policy_bytes: Some(65539)
                }
            ]
        );
        // A present but unbound owner must stop before egress, not use defaults.
        let unavailable = client
            .as_ref()
            .clone()
            .with_primitive_settings_source(Some(Arc::new(
                crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new(),
            )));
        assert!(
            unavailable
                .exchange_callback(
                    exchange_context(),
                    callback_request("notion", "unbound", &["workspace"])
                )
                .await
                .is_err()
        );
        assert_eq!(egress.bounds.lock().unwrap().len(), 2);
        // Invalid updates never alter durable or acknowledged settings.
        for (timeout, bytes) in [(0, 65539), (51000, 0), (51000, u64::MAX)] {
            let mut invalid = second_limits;
            invalid.oauth_token_timeout_millis = timeout;
            invalid.oauth_token_response_body_bytes = bytes;
            let update = serde_json::from_value(
                serde_json::json!({"expected_revision":second.revision,"execution_limits":invalid}),
            )
            .unwrap();
            assert!(
                settings
                    .upsert("default", "default", &update)
                    .await
                    .is_err()
            );
            assert_eq!(
                settings.get("default", "default").await.unwrap().revision,
                second.revision
            );
            assert_eq!(source.snapshot().await.unwrap().limits, second_limits);
        }
        let restored=settings.upsert("default","default",&serde_json::from_value(serde_json::json!({"expected_revision":second.revision,"execution_limits":original.execution_limits})).unwrap()).await.unwrap();
        assert_eq!(source.snapshot().await.unwrap().revision, restored.revision);
        runtime.shutdown().await.unwrap();
    }
}
