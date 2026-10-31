//! Real Extension dialog / public Product facade / native Auth browser fixture.
//! Only successful transport acknowledgements are cut after the real operation.
use crate::RebornRuntime;
use crate::{RebornWebuiBundle, WebuiAuthenticator, WebuiServeConfig, webui_v2_app};
use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use brassclaw_auth::{
    AuthFlowId, AuthFlowStatus, AuthProductScope, AuthSurface, CredentialAccountListRequest,
};
use brassclaw_host_api::{ResourceScope, UserId};
use brassclaw_product_workflow::{
    ExtensionManualTokenCompletion, ExtensionManualTokenSetupReceipt, RebornServicesError,
    RebornServicesErrorCode,
};
use brassclaw_webui_v2::WebUiV2HttpError;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

const BEARER: &str = "native-extension-original-fixture-bearer";
const BASE: &str = "/api/webchat/v2/extensions/nearai/setup/manual-token/";
const MARKER: &str = "native-extension-original-browser: passed";
struct Operator(UserId);
#[async_trait]
impl WebuiAuthenticator for Operator {
    async fn authenticate(&self, token: &str) -> Option<UserId> {
        (token == BEARER).then(|| self.0.clone())
    }
}
#[derive(Default)]
struct Observed {
    originals: BTreeMap<AuthFlowId, ExtensionManualTokenSetupReceipt>,
    setup: usize,
    raw: usize,
    reference: usize,
    cancel: usize,
    generic: usize,
    raw_completion: Option<ExtensionManualTokenCompletion>,
    reference_completion: Option<ExtensionManualTokenCompletion>,
    statuses: Vec<u16>,
}
async fn cut_acknowledgements(
    State(observed): State<Arc<Mutex<Observed>>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    let mutation = request.method() == axum::http::Method::POST;
    let response = next.run(request).await;
    if !mutation {
        return response;
    }
    if path == "/api/webchat/v2/extensions/nearai/setup" {
        observed.lock().unwrap().generic += 1;
        return response;
    }
    let Some(action) = path.strip_prefix(BASE) else {
        return response;
    };
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 64 * 1024).await.unwrap();
    let mut cut = false;
    {
        let mut state = observed.lock().unwrap();
        match action {
            "setup" => {
                state.setup += 1;
                if parts.status == StatusCode::OK {
                    let receipt: ExtensionManualTokenSetupReceipt =
                        serde_json::from_slice(&bytes).unwrap();
                    if let Some(first) = state.originals.get(&receipt.identity.flow_id) {
                        assert_eq!(first.identity, receipt.identity);
                        assert_eq!(first.invocation_id, receipt.invocation_id);
                        assert_eq!(first.provider, receipt.provider);
                        assert_eq!(first.label, receipt.label);
                        assert_eq!(first.expires_at, receipt.expires_at);
                    } else {
                        state.originals.insert(receipt.identity.flow_id, receipt);
                        cut = true;
                    }
                }
            }
            "secret-submit" => {
                state.raw += 1;
                if parts.status == StatusCode::OK {
                    let completion: ExtensionManualTokenCompletion =
                        serde_json::from_slice(&bytes).unwrap();
                    assert!(state.raw_completion.is_none());
                    state.raw_completion = Some(completion);
                    cut = true;
                }
            }
            "continue" => {
                state.reference += 1;
                if parts.status == StatusCode::OK {
                    state.reference_completion = Some(serde_json::from_slice(&bytes).unwrap());
                }
            }
            "cancel" => {
                state.cancel += 1;
            }
            _ => panic!("unexpected dedicated Extension action"),
        }
        state
            .statuses
            .push(if cut { 503 } else { parts.status.as_u16() });
    }
    if cut {
        // The actual Product future/native operation has already completed;
        // this is a deliberate lost acknowledgement, never fake saved state.
        return WebUiV2HttpError::from(RebornServicesError::from_status(
            RebornServicesErrorCode::Unavailable,
            503,
            true,
        ))
        .into_response();
    }
    Response::from_parts(parts, Body::from(bytes))
}

pub(super) async fn run_if_requested(runtime: Arc<RebornRuntime>, bundle: RebornWebuiBundle) {
    let Some(script) = std::env::var_os("BRASSCLAW_EXTENSION_MANUAL_BROWSER_ACCEPTANCE") else {
        return;
    };
    let native = runtime.services.product_auth.as_ref().unwrap().clone();
    let observed = Arc::new(Mutex::new(Observed::default()));
    let listener = super::native_http::bind_loopback().await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let csp = format!(
        "default-src 'self'; script-src 'self' 'nonce-{nonce}'; style-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"
    );
    let mut config = WebuiServeConfig::new(
        runtime.thread_scope.tenant_id.clone(),
        Arc::new(Operator(runtime.actor_user_id.clone())),
        vec![HeaderValue::from_str(&origin).unwrap()],
    )
    .with_default_agent_id(runtime.thread_scope.agent_id.clone())
    .with_csp_header_str(&csp)
    .unwrap();
    if let Some(project) = &runtime.thread_scope.project_id {
        config = config.with_default_project_id(project.clone());
    }
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
    let page = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/browser/extension-manual-original.html"
    ))
    .replace("__IMPORT_MAP__", import_map)
    .replace("__FIXTURE_NONCE__", &nonce)
    .replace("__FIXTURE_BEARER__", BEARER);
    let page_csp = HeaderValue::from_str(&csp).unwrap();
    let router = webui_v2_app(bundle, config)
        .unwrap()
        .route(
            "/__native_extension_original",
            get(move || {
                let page = page.clone();
                let csp = page_csp.clone();
                async move { ([(header::CONTENT_SECURITY_POLICY, csp)], Html(page)) }
            }),
        )
        .layer(middleware::from_fn_with_state(
            observed.clone(),
            cut_acknowledgements,
        ));
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(super::native_http::serve(listener, router, async {
        let _ = stopped.await;
    }));
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
    eprintln!("{stdout}");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.lines().any(|line| line == MARKER),
        "the real browser must complete its assertions"
    );
    let (originals, published) = {
        let state = observed.lock().unwrap();
        assert_eq!(
            (
                state.setup,
                state.raw,
                state.reference,
                state.cancel,
                state.generic
            ),
            (5, 2, 1, 1, 0)
        );
        assert_eq!(
            state.statuses,
            vec![503, 200, 503, 200, 409, 200, 200, 503, 200]
        );
        assert_eq!(state.originals.len(), 2);
        assert_eq!(state.raw_completion, state.reference_completion);
        (
            state.originals.values().cloned().collect::<Vec<_>>(),
            state.reference_completion.clone().unwrap(),
        )
    };
    for receipt in originals {
        let scope = AuthProductScope::new(
            ResourceScope {
                tenant_id: runtime.thread_scope.tenant_id.clone(),
                user_id: runtime.actor_user_id.clone(),
                agent_id: Some(runtime.thread_scope.agent_id.clone()),
                project_id: runtime.thread_scope.project_id.clone(),
                thread_id: None,
                invocation_id: receipt.invocation_id,
            },
            AuthSurface::Web,
        );
        let flow = native
            .recover_original_manual_token_setup(&scope, receipt.identity)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(flow.expires_at, receipt.expires_at);
        let binding = flow
            .update_binding
            .as_ref()
            .expect("actual pre-existing account was explicitly bound");
        assert_eq!(binding.account_id, published.credential_ref);
        if receipt.identity == published.identity {
            assert_eq!(flow.status, AuthFlowStatus::Completed);
            assert_eq!(flow.credential_account_id, Some(published.credential_ref));
            assert!(flow.continuation_emitted_at.is_some());
        } else {
            assert_eq!(flow.status, AuthFlowStatus::Canceled);
            assert!(flow.credential_account_id.is_none());
            assert!(flow.continuation_emitted_at.is_none());
        }
        let accounts = native
            .credential_account_service()
            .list_accounts(CredentialAccountListRequest::new(scope, receipt.provider))
            .await
            .unwrap();
        assert_eq!(accounts.accounts.len(), 1);
        assert_eq!(accounts.accounts[0].id, published.credential_ref);
    }
}
