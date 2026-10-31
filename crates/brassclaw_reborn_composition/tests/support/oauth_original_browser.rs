//! Native Auth/Secret HTTP caller plus opt-in actual Chromium component acceptance.
//! This does not qualify global startup, the full Extensions page or a live issuer.
//! The listener is test infrastructure outside production `src` ingress owners.
use super::*;
use axum::{
    extract::Request as AxumRequest,
    middleware::{self, Next},
    response::Html,
};
use std::sync::atomic::AtomicBool;

const BEARER: &str = "native-dcr-original-browser-test-bearer";
const MARKER: &str = "native-dcr-original-browser: passed";

async fn browser_transport(
    axum::extract::State(cut): axum::extract::State<Arc<AtomicBool>>,
    request: AxumRequest,
    next: Next,
) -> Response {
    let api = request.uri().path().starts_with("/api/");
    if api
        && request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            != Some(concat!(
                "Bearer ",
                "native-dcr-original-browser-test-bearer"
            ))
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let oauth = request.uri().path() == "/api/webchat/v2/extensions/notion/setup/oauth/start";
    let response = next.run(request).await;
    if !oauth || response.status() != StatusCode::OK {
        return response;
    }
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 64 * 1024).await.unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    if reply["status"] == "awaiting_user" && cut.swap(false, Ordering::SeqCst) {
        // The real native operation has returned after commit. Cut only its
        // transport acknowledgement, never synthesize its saved state.
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"controlled lost acknowledgement"})),
        )
            .into_response();
    }
    Response::from_parts(parts, Body::from(bytes))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_dcr_original_http_retains_one_registration_with_optional_browser_acceptance() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    crate::pg_auth_product_services::run_auth_migrations(rig.pool.as_ref())
        .await
        .unwrap();
    let secrets = Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "native-dcr-browser-secret-owner-key".into(),
            "native-dcr-browser",
        )
        .unwrap(),
    );
    let native = Arc::new(crate::pg_auth_product_services::PgAuthProductServices::new(
        rig.pool.clone(),
        secrets.clone(),
    ));
    let observed = Arc::new(ObservedRegistration {
        registrations: AtomicUsize::new(0),
        token_exchanges: AtomicUsize::new(0),
        deletions: AtomicUsize::new(0),
        malformed: false,
        cleanup_status: 204,
        cleanup_unknown: true,
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
        secrets.clone(),
        Arc::new(NoopObligationHandler),
    )
    .unwrap();
    let provider = Arc::new(provider);
    let client = crate::oauth_provider_client::HostOAuthProviderClient::new_with_client_material(
        crate::notion_oauth::notion_provider_spec(),
        observed.clone(),
        secrets,
        Arc::new(NoopObligationHandler),
        provider.clone(),
    )
    .unwrap();
    // Native persistence and provider egress are separate ports, as in the
    // production factory. The DCR provider supplies the actual retained client.
    let services = crate::auth::RebornProductAuthServicePorts::from_shared_with_provider(
        native.clone(),
        Arc::new(client),
    )
    .into_services(Arc::new(NoopDispatcher))
    .with_flow_record_source(native.clone())
    .with_dcr_oauth_registry(Arc::new(OAuthDcrProviderRegistry::new(vec![provider])));
    let state = ProductAuthRouteState::new(
        Arc::new(services),
        TenantId::new("tenant-alpha").unwrap(),
        None,
        None,
    );
    let state = state.with_services(extension_oauth_requirement::test_services("notion"));
    let app = product_auth_route_mount(state)
        .protected
        .layer(axum::Extension(test_caller()));
    let original = request().await;
    let (status, first) = send(&app, original.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let mut recover = original.clone();
    recover["action"] = json!("recover");
    assert_eq!(send(&app, recover).await.1, first);
    let mut cancel = original;
    cancel["action"] = json!("cancel");
    let (status, closed) = send(&app, cancel.clone()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(closed["status"], "canceled");
    assert_eq!(closed["cleanup"]["remote"], "unconfirmed");
    assert_eq!(send(&app, cancel).await.1["cleanup"], closed["cleanup"]);
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 1);
    assert_eq!(observed.deletions.load(Ordering::SeqCst), 1);

    let Some(script) = std::env::var_os("BRASSCLAW_DCR_BROWSER_ACCEPTANCE") else {
        eprintln!("Native HTTP assertions completed; Chromium acceptance was not requested.");
        return;
    };
    let index = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../brassclaw_webui_v2_static/static/index.html"
    ));
    let start = index.find(r#"<script type="importmap""#).unwrap();
    let import_map = index[start..]
        .split_once('>')
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    let nonce = Uuid::new_v4().simple().to_string();
    let csp = format!(
        "default-src 'self'; script-src 'self' 'nonce-{nonce}'; style-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"
    );
    let browser_definition = extension_oauth_requirement::test_request("notion").await;
    let secret = json!({"name":browser_definition["requirement"],"provider":browser_definition["provider"],
        "setup":{"account_label":browser_definition["account_label"],"scopes":browser_definition["scopes"],"invocation_id":browser_definition["invocation_id"]}});
    let page = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/browser/oauth-original.html"
    ))
    .replace(
        "__FIXTURE_SECRET__",
        &serde_json::to_string(&secret).unwrap(),
    )
    .replace("__IMPORT_MAP__", import_map)
    .replace("__FIXTURE_NONCE__", &nonce)
    .replace("__FIXTURE_BEARER__", BEARER);
    let app = app
        .merge(brassclaw_webui_v2_static::static_router())
        .route(
            "/__native_dcr_original",
            get(move || {
                let page = page.clone();
                let csp = csp.clone();
                async move { ([(header::CONTENT_SECURITY_POLICY, csp)], Html(page)) }
            }),
        )
        .layer(middleware::from_fn_with_state(
            Arc::new(AtomicBool::new(true)),
            browser_transport,
        ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
    });
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(script)
            .arg(origin)
            .output()
    })
    .await
    .unwrap()
    .unwrap();
    let _ = shutdown.send(());
    server.await.unwrap().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.lines().any(|line| line == MARKER),
        "actual browser must finish its assertions"
    );
    eprintln!("{stdout}");
    assert_eq!(observed.registrations.load(Ordering::SeqCst), 2);
    assert_eq!(observed.deletions.load(Ordering::SeqCst), 2);
    let count: i64 = rig
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT count(*) FROM brassclaw_product_auth_flows WHERE data->>'status'='canceled'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count, 2,
        "both actual originals remain canceled in PostgreSQL"
    );
}
