//! Real browser/HTTP/Auth continuation fixture. Only acknowledgements are cut;
//! state, secret custody and continuation use the runtime's actual native owner.
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::{
    http::{HeaderValue, header},
    response::Html,
    routing::get,
};
use brassclaw_auth::{
    AuthChallenge, AuthFlowRecord, AuthFlowStatus, AuthInteractionId, AuthProductError,
    AuthProductScope, CredentialAccountListRequest, ManualTokenSetupIdentity,
    ManualTokenSetupRequest, SecretSubmitRequest, SecretSubmitResult,
};
use brassclaw_host_api::{
    ExtensionId, RuntimeCredentialAccountProviderId, RuntimeCredentialAuthRequirement, UserId,
};
use brassclaw_turns::{GateRef, GetRunStateRequest, TurnActor, TurnScope, TurnStatus};
use secrecy::SecretString;

use super::RebornRuntime;
use crate::{
    RebornManualTokenFlowService, WebuiAuthenticator, WebuiServeConfig, build_webui_services,
    webui_v2_app,
};

const BEARER: &str = "native-manual-browser-fixture-bearer";
const MARKER: &str = "native-manual-original-browser: passed";

struct FixtureBearer(UserId);

#[async_trait]
impl WebuiAuthenticator for FixtureBearer {
    async fn authenticate(&self, token: &str) -> Option<UserId> {
        (token == BEARER).then(|| self.0.clone())
    }
}

#[derive(Default)]
struct Observations {
    first: Option<(ManualTokenSetupIdentity, AuthFlowRecord)>,
    setup_calls: usize,
    raw_calls: usize,
    reference_calls: usize,
    legacy_calls: usize,
    abandon_calls: usize,
}

struct LoseFirstAcknowledgements {
    inner: Arc<dyn RebornManualTokenFlowService>,
    observed: Mutex<Observations>,
}

#[async_trait]
impl RebornManualTokenFlowService for LoseFirstAcknowledgements {
    async fn request_original_manual_token_flow(
        &self,
        identity: ManualTokenSetupIdentity,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let flow = self
            .inner
            .request_original_manual_token_flow(identity, request)
            .await?;
        let mut observed = self.observed.lock().unwrap();
        observed.setup_calls += 1;
        if let Some((original_identity, original_flow)) = &observed.first {
            assert_eq!(*original_identity, identity);
            assert_eq!(original_flow.id, flow.id);
            assert_eq!(original_flow.scope, flow.scope);
            assert_eq!(original_flow.created_at, flow.created_at);
            assert_eq!(original_flow.expires_at, flow.expires_at);
        } else {
            observed.first = Some((identity, flow.clone()));
        }
        if observed.setup_calls == 1 {
            // The real native transaction already committed. Do not compensate.
            Err(AuthProductError::BackendUnavailable)
        } else {
            Ok(flow)
        }
    }

    async fn submit_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
        secret: SecretString,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        let call = {
            let mut observed = self.observed.lock().unwrap();
            observed.raw_calls += 1;
            observed.raw_calls
        };
        let result = self
            .inner
            .submit_original_manual_token_flow(scope, identity, secret)
            .await?;
        if call == 1 {
            assert_eq!(result.1.status, AuthFlowStatus::Completed);
            assert!(result.1.continuation_emitted_at.is_none());
            // Whole publication is real; the facade has not dispatched the gate.
            Err(AuthProductError::BackendUnavailable)
        } else {
            Ok(result)
        }
    }

    async fn continue_original_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: ManualTokenSetupIdentity,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.observed.lock().unwrap().reference_calls += 1;
        self.inner
            .continue_original_manual_token_flow(scope, identity)
            .await
    }

    async fn request_manual_token_flow(
        &self,
        request: ManualTokenSetupRequest,
    ) -> Result<AuthChallenge, AuthProductError> {
        self.observed.lock().unwrap().legacy_calls += 1;
        self.inner.request_manual_token_flow(request).await
    }

    async fn submit_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        request: SecretSubmitRequest,
    ) -> Result<(SecretSubmitResult, AuthFlowRecord), AuthProductError> {
        self.observed.lock().unwrap().legacy_calls += 1;
        self.inner.submit_manual_token_flow(scope, request).await
    }

    async fn abandon_manual_token_flow(
        &self,
        scope: &AuthProductScope,
        identity: AuthInteractionId,
    ) -> Result<bool, AuthProductError> {
        self.observed.lock().unwrap().abandon_calls += 1;
        self.inner.abandon_manual_token_flow(scope, identity).await
    }
}

pub(super) async fn run_if_requested(runtime: Arc<RebornRuntime>) {
    let Some(script) = std::env::var_os("BRASSCLAW_MANUAL_TOKEN_BROWSER_ACCEPTANCE") else {
        return;
    };
    let conversation = runtime.new_conversation().await.unwrap();
    let scope = TurnScope::new_with_owner(
        runtime.thread_scope.tenant_id.clone(),
        Some(runtime.thread_scope.agent_id.clone()),
        runtime.thread_scope.project_id.clone(),
        conversation.0,
        Some(runtime.actor_user_id.clone()),
    );
    let gate = GateRef::new("gate:native-manual-original-browser").unwrap();
    let run = super::submit_and_block_auth_run(
        &runtime,
        scope.clone(),
        TurnActor::new(runtime.actor_user_id.clone()),
        &gate,
        vec![RuntimeCredentialAuthRequirement {
            requirement: None,
            provider: RuntimeCredentialAccountProviderId::new("manual-browser-provider").unwrap(),
            requester_extension: ExtensionId::new("native-manual-browser").unwrap(),
            provider_scopes: Vec::new(),
        }],
    )
    .await;
    let native = runtime.services.product_auth.as_ref().unwrap();
    let owner = Arc::new(LoseFirstAcknowledgements {
        inner: native.manual_token_flow_service_for_tests(),
        observed: Mutex::new(Observations::default()),
    });
    let mut bundle = build_webui_services(runtime.clone(), None).await.unwrap();
    bundle.product_auth = Some(Arc::new(
        (**native)
            .clone()
            .with_manual_token_flow_service(owner.clone()),
    ));
    let listener = super::super::tests::native_http::bind_loopback()
        .await
        .unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let csp = format!(
        "default-src 'self'; script-src 'self' 'nonce-{nonce}'; style-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"
    );
    let mut config = WebuiServeConfig::new(
        runtime.thread_scope.tenant_id.clone(),
        Arc::new(FixtureBearer(runtime.actor_user_id.clone())),
        vec![HeaderValue::from_str(&origin).unwrap()],
    )
    .with_default_agent_id(runtime.thread_scope.agent_id.clone())
    .with_csp_header_str(&csp)
    .unwrap();
    if let Some(project) = &runtime.thread_scope.project_id {
        config = config.with_default_project_id(project.clone());
    }
    let original_index = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../brassclaw_webui_v2_static/static/index.html"
    ));
    let import_start = original_index.find("<script type=\"importmap\"").unwrap();
    let map = original_index[import_start..]
        .split_once('>')
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    let page = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/browser/manual-token-original.html"
    ))
    .replace("__IMPORT_MAP__", map)
    .replace("__FIXTURE_NONCE__", &nonce)
    .replace("__FIXTURE_BEARER__", BEARER)
    .replace("__FIXTURE_THREAD__", scope.thread_id.as_str());
    let page_csp = HeaderValue::from_str(&csp).unwrap();
    let router = webui_v2_app(bundle, config)
        .expect("protected native browser router")
        .route(
            "/__native_manual_original",
            get(move || {
                let page = page.clone();
                let csp = page_csp.clone();
                async move { ([(header::CONTENT_SECURITY_POLICY, csp)], Html(page)) }
            }),
        );
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(super::super::tests::native_http::serve(
        listener,
        router,
        async {
            let _ = stopped.await;
        },
    ));
    let thread = scope.thread_id.to_string();
    let run_text = run.to_string();
    let gate_text = gate.as_str().to_owned();
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("node")
            .arg(script)
            .arg(origin)
            .arg(thread)
            .arg(run_text)
            .arg(gate_text)
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
        "browser must actually finish its acceptance assertions"
    );

    let (identity, first) = {
        let observed = owner.observed.lock().unwrap();
        assert_eq!(observed.setup_calls, 4);
        assert_eq!(observed.raw_calls, 2);
        assert_eq!(observed.reference_calls, 1);
        assert_eq!(observed.legacy_calls, 0);
        assert_eq!(observed.abandon_calls, 0);
        observed.first.clone().unwrap()
    };
    let actual = native
        .flow_manager()
        .get_flow(&first.scope, identity.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(actual.status, AuthFlowStatus::Completed);
    assert_eq!(actual.created_at, first.created_at);
    assert_eq!(actual.expires_at, first.expires_at);
    assert!(actual.continuation_emitted_at.is_some());
    let accounts = native
        .credential_account_service()
        .list_accounts(CredentialAccountListRequest {
            scope: first.scope,
            provider: first.provider,
            requester_extension: None,
            cursor: None,
            limit: CredentialAccountListRequest::DEFAULT_LIMIT,
        })
        .await
        .unwrap();
    assert_eq!(accounts.accounts.len(), 1);
    assert_eq!(Some(accounts.accounts[0].id), actual.credential_account_id);
    let resumed = runtime
        .webui_turn_coordinator()
        .get_run_state(GetRunStateRequest { scope, run_id: run })
        .await
        .unwrap();
    assert_eq!(resumed.status, TurnStatus::Queued);
    assert!(resumed.gate_ref.is_none());
}
