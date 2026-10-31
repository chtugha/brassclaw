//! Actual protected route/facade + in-memory owners; not native PG acceptance.
#[cfg(feature = "postgres")]
#[path = "oauth_usage_tests.rs"]
mod usage;
#[cfg(feature = "postgres")]
pub(crate) use usage::verify_dcr_oauth_usage;

#[cfg(feature = "skills-db")]
#[path = "../../tests/support/oauth_original_browser.rs"]
mod native_browser;

use super::tests::{NoopDispatcher, NoopObligationHandler, RouteDcrSetupEgress, test_caller};
use super::*;
use crate::oauth_dcr::{OAuthDcrProvider, OAuthDcrProviderConfig, OAuthDcrProviderRegistry};
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, header};
use brassclaw_auth::{AuthFlowManager, InMemoryAuthProductServices};
use brassclaw_host_api::{
    NetworkMethod, RuntimeHttpEgress, RuntimeHttpEgressRequest, RuntimeHttpEgressResponse,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;

struct ObservedRegistration {
    registrations: AtomicUsize,
    token_exchanges: AtomicUsize,
    deletions: AtomicUsize,
    malformed: bool,
    cleanup_status: u16,
    cleanup_unknown: bool,
}
#[async_trait::async_trait]
impl RuntimeHttpEgress for ObservedRegistration {
    async fn execute(
        &self,
        request: RuntimeHttpEgressRequest,
    ) -> Result<RuntimeHttpEgressResponse, brassclaw_host_api::RuntimeHttpEgressError> {
        let registration = request.method == NetworkMethod::Post
            && request.url.as_str() == "https://oauth.notion.com/register";
        if request.method == NetworkMethod::Post
            && request.url.as_str() == "https://oauth.notion.com/token"
        {
            self.token_exchanges.fetch_add(1, Ordering::SeqCst);
        }
        if registration {
            self.registrations.fetch_add(1, Ordering::SeqCst);
        }
        let deletion = request.method == NetworkMethod::Delete;
        if deletion {
            self.deletions.fetch_add(1, Ordering::SeqCst);
        }
        if deletion && self.cleanup_unknown {
            return Err(brassclaw_host_api::RuntimeHttpEgressError::Network {
                reason: "controlled lost cleanup reply".into(),
                request_bytes: 5,
                response_bytes: 0,
            });
        }
        let mut response = RouteDcrSetupEgress.execute(request).await?;
        if deletion {
            response.status = self.cleanup_status;
            response.body = if self.cleanup_status == 204 {
                Vec::new()
            } else {
                b"controlled provider rejection".to_vec()
            };
            response.response_bytes = response.body.len() as u64;
        }
        if registration && self.malformed {
            response.body = b"unusable-registration-response".to_vec();
            response.response_bytes = response.body.len() as u64;
        }
        Ok(response)
    }
}

fn app(
    malformed: bool,
) -> (
    Router,
    Arc<ObservedRegistration>,
    Arc<InMemoryAuthProductServices>,
) {
    app_with_cleanup(malformed, 204)
}
fn app_with_cleanup(
    malformed: bool,
    cleanup_status: u16,
) -> (
    Router,
    Arc<ObservedRegistration>,
    Arc<InMemoryAuthProductServices>,
) {
    app_with_cleanup_outcome(malformed, cleanup_status, false)
}
fn app_with_cleanup_outcome(
    malformed: bool,
    cleanup_status: u16,
    cleanup_unknown: bool,
) -> (
    Router,
    Arc<ObservedRegistration>,
    Arc<InMemoryAuthProductServices>,
) {
    let observed = Arc::new(ObservedRegistration {
        registrations: AtomicUsize::new(0),
        token_exchanges: AtomicUsize::new(0),
        deletions: AtomicUsize::new(0),
        malformed,
        cleanup_status,
        cleanup_unknown,
    });
    let provider = OAuthDcrProvider::new(
        OAuthDcrProviderConfig {
            spec: crate::notion_oauth::notion_provider_spec(),
            callback_origin: "http://127.0.0.1:3000".into(),
            client_name: "Brassclaw".into(),
            account_label: CredentialAccountLabel::new("notion").unwrap(),
            scopes: vec![],
        },
        observed.clone(),
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
        Arc::new(NoopObligationHandler),
    )
    .unwrap();
    let auth = Arc::new(InMemoryAuthProductServices::new());
    let services = RebornProductAuthServices::from_shared(auth.clone(), Arc::new(NoopDispatcher))
        .with_flow_record_source(auth.clone())
        .with_dcr_oauth_registry(Arc::new(OAuthDcrProviderRegistry::new(vec![Arc::new(
            provider,
        )])));
    let state = ProductAuthRouteState::new(
        Arc::new(services),
        TenantId::new("tenant-alpha").unwrap(),
        None,
        None,
    )
    .with_services(extension_oauth_requirement::test_services("notion"));
    (
        product_auth_route_mount(state)
            .protected
            .layer(axum::Extension(test_caller())),
        observed,
        auth,
    )
}

async fn request() -> serde_json::Value {
    extension_oauth_requirement::test_request("notion").await
}

#[tokio::test]
async fn http_cleanup_rejection_remains_visible_without_a_second_delete_or_local_purge() {
    let (app, observed, _) = app_with_cleanup(false, 405);
    let original = request().await;
    assert_eq!(send(&app, original.clone()).await.0, StatusCode::OK);
    let mut cancel = original;
    cancel["action"] = json!("cancel");
    let (status, receipt) = send(&app, cancel.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["status"], "canceled");
    assert_eq!(receipt["cleanup"]["remote"], "unconfirmed");
    assert_eq!(receipt["cleanup"]["local"], "retained");
    assert_eq!(receipt["cleanup"]["http_status"], 405);
    assert_eq!(receipt["cleanup"]["can_reconcile"], true);
    assert!(!receipt.to_string().contains("registration-token"));
    assert_eq!(send(&app, cancel).await.1, receipt);
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 1);
    assert_eq!(observed.deletions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn http_lost_cleanup_reply_keeps_confirmed_cancellation_and_unknown_effect_visible() {
    let (app, observed, _) = app_with_cleanup_outcome(false, 204, true);
    let original = request().await;
    let (status, first) = send(&app, original.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let mut cancel = original.clone();
    cancel["action"] = json!("cancel");
    let (status, receipt) = send(&app, cancel.clone()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(receipt["flow_id"], original["flow_id"]);
    assert_eq!(receipt["expires_at"], first["expires_at"]);
    assert_eq!(receipt["status"], "canceled");
    assert_eq!(receipt["terminal"], true);
    assert_eq!(receipt["error"]["code"], "backend_unavailable");
    assert_eq!(receipt["cleanup"]["remote"], "unconfirmed");
    assert_eq!(receipt["cleanup"]["local"], "retained");
    assert_eq!(receipt["cleanup"]["error"], "backend_unavailable");
    assert!(receipt["authorization_url"].is_null());
    let (_, again) = send(&app, cancel).await;
    assert_eq!(again["flow_id"], receipt["flow_id"]);
    assert_eq!(again["cleanup"], receipt["cleanup"]);
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 1);
    assert_eq!(observed.deletions.load(Ordering::SeqCst), 1);
}
async fn send(app: &Router, request: serde_json::Value) -> (StatusCode, serde_json::Value) {
    send_on(app, request, "notion").await
}
async fn send_google(app: &Router, request: serde_json::Value) -> (StatusCode, serde_json::Value) {
    send_on(app, request, "google-drive").await
}
async fn send_on(
    app: &Router,
    request: serde_json::Value,
    package: &str,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!(
                    "/api/webchat/v2/extensions/{package}/setup/oauth/start"
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({"non_json_rejection":true}));
    (status, value)
}

#[tokio::test]
async fn http_dcr_start_recover_cancel_and_explicit_successor_preserve_original_disposition() {
    let (app, observed, _auth) = app(false);
    let original = request().await;
    let (status, first) = send(&app, original.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["flow_id"], original["flow_id"]);
    assert_eq!(
        first["callback_scope"]["invocation_id"],
        original["invocation_id"]
    );
    assert_eq!(first["status"], "awaiting_user");
    assert_eq!(first["terminal"], false);
    assert!(
        first["authorization_url"]
            .as_str()
            .unwrap()
            .starts_with("https://oauth.notion.com/")
    );
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 1);
    for action in ["recover", "start"] {
        let mut retry = original.clone();
        retry["action"] = json!(action);
        let (status, again) = send(&app, retry).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(again, first);
    }
    let mut wrong = original.clone();
    wrong["action"] = json!("recover");
    wrong["invocation_id"] = json!(InvocationId::new().to_string());
    assert_eq!(send(&app, wrong).await.0, StatusCode::FORBIDDEN);
    let mut changed = original.clone();
    changed["action"] = json!("recover");
    changed["scopes"] = json!(["changed"]);
    assert_eq!(send(&app, changed).await.0, StatusCode::CONFLICT);
    let mut missing = original.clone();
    missing["action"] = json!("recover");
    missing["flow_id"] = json!(AuthFlowId::new().to_string());
    assert_eq!(send(&app, missing).await.0, StatusCode::NOT_FOUND);
    let mut cancel = original.clone();
    cancel["action"] = json!("cancel");
    let (status, canceled) = send(&app, cancel.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(canceled["status"], "canceled");
    assert_eq!(canceled["terminal"], true);
    assert_eq!(canceled["cleanup"]["remote"], "confirmed");
    assert_eq!(canceled["cleanup"]["local"], "confirmed");
    assert_eq!(canceled["cleanup"]["can_reconcile"], false);
    assert!(canceled["authorization_url"].is_null());
    assert_eq!(canceled["flow_id"], first["flow_id"]);
    assert_eq!(canceled["expires_at"], first["expires_at"]);
    assert_eq!(send(&app, cancel).await.1, canceled);
    assert_eq!(
        send(&app, original.clone()).await.1,
        canceled,
        "start does not manufacture a replacement after cancel"
    );
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 1);
    assert_eq!(
        observed.deletions.load(Ordering::SeqCst),
        1,
        "explicit cancellation owns one claimed DELETE, never retry compensation"
    );
    let mut successor = original;
    successor["flow_id"] = json!(AuthFlowId::new().to_string());
    let (status, next) = send(&app, successor).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(next["flow_id"], first["flow_id"]);
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn http_unusable_registration_is_reported_and_retained_without_issuer_replay() {
    let (app, observed, auth) = app(true);
    let original = request().await;
    let (status, pending) = send(&app, original.clone()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(pending["status"], "pending");
    assert_eq!(pending["terminal"], false);
    assert_eq!(pending["error"]["code"], "backend_unavailable");
    assert!(pending["authorization_url"].is_null());
    let (status, retry) = send(&app, original.clone()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(retry, pending);
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 1);
    assert_eq!(observed.deletions.load(Ordering::SeqCst), 0);
    let mut read = original.clone();
    read["action"] = json!("recover");
    let (status, read) = send(&app, read).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(read["flow_id"], pending["flow_id"]);
    assert!(
        read["error"].is_null(),
        "a read reports the stored phase without inventing another failed provider call"
    );
    let scope = AuthProductScope::new(
        ResourceScope {
            tenant_id: TenantId::new("tenant-alpha").unwrap(),
            user_id: test_caller().user_id,
            agent_id: None,
            project_id: None,
            thread_id: None,
            invocation_id: InvocationId::from_uuid(
                Uuid::parse_str(original["invocation_id"].as_str().unwrap()).unwrap(),
            ),
        },
        AuthSurface::Callback,
    );
    let flow_id =
        AuthFlowId::from_uuid(Uuid::parse_str(original["flow_id"].as_str().unwrap()).unwrap());
    let state = auth
        .read_dcr_registration(&scope, flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        state.action.outcome,
        brassclaw_auth::DcrRegistrationOutcome::Returned { .. }
    ));
    assert!(state.flow.challenge.is_none());
    let mut poisoned = original;
    poisoned["token"] = json!("PRIVATE_FIXTURE_NOT_ACCEPTED");
    assert_eq!(
        send(&app, poisoned).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 1);
}

/// The protected Extension route with actual facade/flow-owner implementations.
/// Shared by in-memory, native PostgreSQL and native record-filesystem cases.
pub(crate) async fn verify_google_original_lifecycle(
    product_auth: Arc<RebornProductAuthServices>,
    secrets: Arc<dyn brassclaw_secrets::SecretStore>,
) {
    let provider = Arc::new(crate::oauth_gate::GoogleOAuthGateProvider::new(
        crate::input::OAuthClientConfig::new(
            "google-client.apps.googleusercontent.com",
            "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
            None,
        )
        .unwrap(),
        secrets,
    ));
    let product_auth = Arc::new((*product_auth).clone().with_oauth_gate_registry(Arc::new(
        crate::oauth_gate::GoogleOAuthGateProviderRegistry::new(vec![provider]),
    )));
    let state = ProductAuthRouteState::new(
        product_auth.clone(),
        TenantId::new("tenant-alpha").unwrap(),
        None,
        None,
    )
    .with_services(extension_oauth_requirement::test_services("google-drive"))
    .with_google_oauth(
        GoogleOAuthRouteConfig::new(
            "google-client.apps.googleusercontent.com",
            "http://127.0.0.1:3000/api/reborn/product-auth/oauth/google/callback",
        )
        .unwrap(),
    );
    let app = product_auth_route_mount(state.clone())
        .protected
        .layer(axum::Extension(test_caller()));
    let original = extension_oauth_requirement::test_request("google-drive").await;
    for invalid in [None, Some("invalid-flow-id")] {
        let mut bad = original.clone();
        if let Some(value) = invalid {
            bad["flow_id"] = json!(value);
        } else {
            bad.as_object_mut().unwrap().remove("flow_id");
        }
        let (status, _) = send_google(&app, bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    for field in ["requirement", "target"] {
        let mut invalid = original.clone();
        invalid["flow_id"] = json!(AuthFlowId::new().to_string());
        invalid.as_object_mut().unwrap().remove(field);
        assert_eq!(send_google(&app, invalid).await.0, StatusCode::BAD_REQUEST);
    }
    let (status, first) = send_google(&app, original.clone()).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["flow_id"], original["flow_id"]);
    assert_eq!(first["terminal"], false);
    assert!(
        first["authorization_url"]
            .as_str()
            .unwrap()
            .contains("code_challenge=")
    );
    let id = AuthFlowId::from_uuid(Uuid::parse_str(first["flow_id"].as_str().unwrap()).unwrap());
    let scope = scope_from_authenticated_caller_parts_requiring_invocation(
        &test_caller(),
        &ScopeFields {
            session_id: None,
            thread_id: None,
            invocation_id: Some(original["invocation_id"].as_str().unwrap().into()),
        },
    )
    .unwrap();
    assert!(
        state.pkce_verifier_for_callback(id).is_err(),
        "Google never enters the route cache"
    );
    let provider_id = AuthProviderId::new(GOOGLE_PROVIDER_ID).unwrap();
    let before = product_auth
        .oauth_pkce_verifier_for_flow(&scope, &provider_id, id)
        .await
        .unwrap()
        .unwrap();
    let (status, again) = send_google(&app, original.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first, again);
    assert_eq!(
        before.expose_secret(),
        product_auth
            .oauth_pkce_verifier_for_flow(&scope, &provider_id, id)
            .await
            .unwrap()
            .unwrap()
            .expose_secret()
    );
    let flow = product_auth
        .flow_manager()
        .get_flow(&scope, id)
        .await
        .unwrap()
        .unwrap();
    let setup = product_auth
        .flow_manager()
        .requirement_setup(&scope, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        setup.requirement(),
        &extension_oauth_requirement::test_requirement("google-drive")
    );
    assert!(setup.previous().is_none());
    assert_eq!(
        flow.pkce_verifier_hash,
        Some(pkce_verifier_hash(before.expose_secret()).unwrap())
    );
    for (field, changed) in [
        ("account_label", json!("changed")),
        (
            "scopes",
            json!(["https://www.googleapis.com/auth/calendar.readonly"]),
        ),
        (
            "expires_at",
            json!((Utc::now() + ChronoDuration::minutes(4)).to_rfc3339()),
        ),
    ] {
        let mut mutation = original.clone();
        mutation[field] = changed;
        let (status, _) = send_google(&app, mutation).await;
        assert_eq!(status, StatusCode::CONFLICT, "changed {field}");
    }
    for (field, changed) in [
        ("requirement", json!("sha256:wrong")),
        ("provider", json!("notion")),
        (
            "target",
            json!({"kind":"existing","account_id":brassclaw_auth::CredentialAccountId::new()}),
        ),
        ("target", serde_json::Value::Null),
    ] {
        let mut mutation = original.clone();
        mutation[field] = changed;
        assert_eq!(send_google(&app, mutation).await.0, StatusCode::CONFLICT);
    }
    // The original remains recoverable even when current provider configuration
    // has disappeared. Recovery is not a new setup or a new PKCE preparation.
    let mut no_config = state.clone();
    no_config.google_oauth = None;
    no_config.services = None;
    let recovered_app = product_auth_route_mount(no_config)
        .protected
        .layer(axum::Extension(test_caller()));
    let mut recover = original.clone();
    recover["action"] = json!("recover");
    let (status, recovered) = send_google(&recovered_app, recover.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered, first);
    for action in ["recover", "cancel"] {
        let mut missing = original.clone();
        missing["action"] = json!(action);
        let missing_id = AuthFlowId::new();
        missing["flow_id"] = json!(missing_id.to_string());
        let (status, _) = send_google(&app, missing).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            product_auth
                .flow_manager()
                .get_flow(&scope, missing_id)
                .await
                .unwrap()
                .is_none()
        );
    }
    let mut foreign = recover.clone();
    foreign["invocation_id"] = json!(InvocationId::new().to_string());
    let (status, _) = send_google(&app, foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let mut cancel = original.clone();
    cancel["action"] = json!("cancel");
    let (status, canceled) = send_google(&app, cancel.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(canceled["status"], "canceled");
    assert_eq!(canceled["terminal"], true);
    assert!(canceled["authorization_url"].is_null());
    assert!(state.pkce_verifier_for_callback(id).is_err());
    let (_, repeated_cancel) = send_google(&app, cancel).await;
    assert_eq!(repeated_cancel, canceled);
    let (_, repeated_start) = send_google(&app, original.clone()).await;
    assert_eq!(repeated_start, canceled);
    let after = product_auth
        .flow_manager()
        .get_flow(&scope, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.pkce_verifier_hash, flow.pkce_verifier_hash);
    assert_eq!(after.opaque_state_hash, flow.opaque_state_hash);
    assert_eq!(after.update_binding, flow.update_binding);
    assert_eq!(after.expires_at, flow.expires_at);
    // Two first requests may race; both must converge on one accepted definition
    // and only its matching verifier may be installed in the callback owner.
    let mut concurrent = original;
    concurrent["flow_id"] = json!(AuthFlowId::new().to_string());
    let ((a, left), (b, right)) = tokio::join!(
        send_google(&app, concurrent.clone()),
        send_google(&app, concurrent)
    );
    assert_eq!(a, StatusCode::OK, "{left}");
    assert_eq!(b, StatusCode::OK, "{right}");
    assert_eq!(left, right);
    let concurrent_id =
        AuthFlowId::from_uuid(Uuid::parse_str(left["flow_id"].as_str().unwrap()).unwrap());
    let concurrent_flow = product_auth
        .flow_manager()
        .get_flow(&scope, concurrent_id)
        .await
        .unwrap()
        .unwrap();
    let retained = product_auth
        .oauth_pkce_verifier_for_flow(&scope, &provider_id, concurrent_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        concurrent_flow.pkce_verifier_hash,
        Some(pkce_verifier_hash(retained.expose_secret()).unwrap())
    );
}

#[tokio::test]
async fn http_google_setup_preserves_original_identity_and_verifier() {
    let shared = Arc::new(InMemoryAuthProductServices::new());
    verify_google_original_lifecycle(
        Arc::new(RebornProductAuthServices::from_shared(
            shared,
            Arc::new(NoopDispatcher),
        )),
        Arc::new(brassclaw_secrets::InMemorySecretStore::new()),
    )
    .await;
}

#[cfg(feature = "postgres")]
pub(crate) use usage::{DcrPreparationBackend, verify_dcr_preparation_handoff};
