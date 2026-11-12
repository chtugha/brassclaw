#![cfg(test)]

//! Loopback SDK/transport evidence; fixture guards are not installation proofs.
use crate::codex_test_helpers::{codex_http_fixture, make_test_jwt};
use crate::final_request_guard::*;
use crate::provider_observation::{ProviderObservationError, ProviderWireObserver};
use crate::*;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
struct Guard {
    valid: Arc<AtomicBool>,
    checks: Arc<AtomicUsize>,
}
#[async_trait]
impl ProviderDispatchGuard for Guard {
    async fn check(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        if !self.valid.load(Ordering::SeqCst) {
            return Err(LlmError::ConfigurationChanged {
                provider: "fixture source changed".into(),
            });
        }
        Ok(ProviderDispatchState {
            definition_revision: 1,
            response_cache: Default::default(),
            provider_id: "codex-fixture".into(),
            model: facts.model.into(),
            model_token_limits: Some(ModelTokenLimits {
                max_prefix_tokens: 10,
                max_prompt_tokens: 10,
            }),
            configured_context_window_tokens: Some(30),
            requested_output_tokens: facts.max_output_tokens,
        })
    }
}
fn guard() -> Arc<Guard> {
    Arc::new(Guard {
        valid: Arc::new(AtomicBool::new(true)),
        checks: Arc::new(AtomicUsize::new(0)),
    })
}
struct Source {
    calls: AtomicUsize,
    guard: Arc<Guard>,
    bound_checks: Arc<Mutex<Vec<usize>>>,
    barrier: Option<Arc<tokio::sync::Barrier>>,
}
impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source").finish_non_exhaustive()
    }
}
#[derive(Debug)]
struct BoundGuard {
    owner: Arc<Guard>,
    ordinal: usize,
    checks: Arc<Mutex<Vec<usize>>>,
    barrier: Option<Arc<tokio::sync::Barrier>>,
}
#[async_trait]
impl ProviderDispatchGuard for BoundGuard {
    async fn check(
        &self,
        facts: ProviderRequestFacts<'_>,
    ) -> Result<ProviderDispatchState, LlmError> {
        self.checks.lock().unwrap().push(self.ordinal);
        if let Some(barrier) = &self.barrier {
            barrier.wait().await;
        }
        self.owner.check(facts).await
    }
}
#[async_trait]
impl CodexHostSessionSource for Source {
    async fn session(&self) -> Result<CodexHostSession, LlmError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CodexHostSession::new(
            make_test_jwt(&format!("account-{n}")).into(),
            Arc::new(BoundGuard {
                owner: self.guard.clone(),
                ordinal: n,
                checks: self.bound_checks.clone(),
                barrier: self.barrier.clone(),
            }),
        ))
    }
}
fn source() -> Arc<Source> {
    Arc::new(Source {
        calls: AtomicUsize::new(0),
        guard: guard(),
        bound_checks: Default::default(),
        barrier: None,
    })
}
fn provider(
    endpoint: &str,
    source: Arc<Source>,
) -> Arc<crate::token_refreshing::TokenRefreshingProvider> {
    Arc::new(crate::token_refreshing::TokenRefreshingProvider::new_host(
        Arc::new(
            OpenAiCodexProvider::new_host_with_response_limit("fixture-model", endpoint, 5, None)
                .unwrap(),
        ),
        source,
    ))
}
fn request() -> CompletionRequest {
    let mut r = CompletionRequest::new(vec![ChatMessage::user("Unicode ü history")]);
    r.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
    r
}
fn reply() -> String {
    r#"data: {"type":"response.output_text.delta","delta":"answer"}

data: {"type":"response.completed","response":{"status":"completed"}}

"#
    .into()
}

#[derive(Default)]
struct Observer {
    events: Mutex<Vec<&'static str>>,
    fail_persist: bool,
}
#[async_trait]
impl ProviderWireObserver for Observer {
    async fn before_submission(&self) -> Result<(), ProviderObservationError> {
        self.events.lock().unwrap().push("before");
        Ok(())
    }
    fn submission_started(&self) {
        self.events.lock().unwrap().push("started");
    }
    fn submission_rejected(&self) {
        self.events.lock().unwrap().push("rejected");
    }
    fn receive_response(&self, _: u16, bytes: &[u8]) -> Result<(), ProviderObservationError> {
        assert!(!bytes.is_empty());
        self.events.lock().unwrap().push("received");
        Ok(())
    }
    async fn persist_response(&self) -> Result<(), ProviderObservationError> {
        self.events.lock().unwrap().push("persisted");
        if self.fail_persist {
            Err(ProviderObservationError)
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn host_rejects_configured_before_source_and_metadata_has_no_auth_effect() {
    let source = source();
    let p = provider("http://127.0.0.1:1", source.clone());
    assert!(matches!(
        p.complete(CompletionRequest::new(vec![])).await,
        Err(LlmError::ConfigurationChanged { .. })
    ));
    let tools = ToolCompletionRequest::new(vec![], vec![]);
    assert!(matches!(
        p.complete_with_tools(tools).await,
        Err(LlmError::ConfigurationChanged { .. })
    ));
    assert!(p.list_models().await.unwrap().is_empty());
    assert_eq!(p.model_metadata().await.unwrap().id, "fixture-model");
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    assert!(
        !format!(
            "{:?}",
            CodexHostSession::new("private-sentinel".into(), guard())
        )
        .contains("private-sentinel")
    );
}

#[tokio::test]
async fn concurrent_host_requests_send_their_own_token_account_and_guard() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut pairs = Vec::new();
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0; 4096];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_string();
                    let header = |name: &str| {
                        headers
                            .lines()
                            .filter_map(|l| l.split_once(':'))
                            .find(|(k, _)| k.eq_ignore_ascii_case(name))
                            .unwrap()
                            .1
                            .trim()
                            .to_owned()
                    };
                    let length = header("content-length").parse::<usize>().unwrap();
                    if bytes.len() >= end + 4 + length {
                        pairs.push((header("authorization"), header("chatgpt-account-id")));
                        break;
                    }
                }
            }
            let body = reply();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
        pairs
    });
    let mut source = source();
    Arc::get_mut(&mut source).unwrap().barrier = Some(Arc::new(tokio::sync::Barrier::new(2)));
    let p = provider(&endpoint, source.clone());
    let mut tools = ToolCompletionRequest::new(vec![ChatMessage::user("tools ü")], vec![]);
    tools.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
    let (a, b) = tokio::join!(p.complete(request()), p.complete_with_tools(tools));
    assert_eq!(a.unwrap().content, "answer");
    assert_eq!(b.unwrap().content.as_deref(), Some("answer"));
    let mut pairs = server.await.unwrap();
    pairs.sort_by(|a, b| a.1.cmp(&b.1));
    for (n, (token, account)) in pairs.iter().enumerate() {
        assert_eq!(account, &format!("account-{n}"));
        assert_eq!(token, &format!("Bearer {}", make_test_jwt(account)));
    }
    assert_eq!(source.guard.checks.load(Ordering::SeqCst), 2);
    let mut checked = source.bound_checks.lock().unwrap().clone();
    checked.sort();
    assert_eq!(checked, vec![0, 1]);
}

#[tokio::test]
async fn stale_guard_rejects_before_original_observer_and_network() {
    let source = source();
    source.guard.valid.store(false, Ordering::SeqCst);
    let p = provider("http://127.0.0.1:1", source.clone());
    let observer = Arc::new(Observer::default());
    assert!(
        crate::provider_observation::observe_chat_completion(
            observer.clone(),
            p.complete(request())
        )
        .await
        .is_err()
    );
    assert_eq!(*observer.events.lock().unwrap(), vec!["rejected"]);
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn host_failures_never_repeat_model_or_source_and_preserve_response_custody() {
    for (status, body, fail_persist) in [
        (401, "private-auth-error".to_string(), false),
        (
            200,
            r#"data: {"type":"error","code":"fixture","message":"parse failure"}"#.to_string(),
            false,
        ),
        (200, reply(), true),
    ] {
        let server = codex_http_fixture(move |_, _| (status, body.clone())).await;
        let source = source();
        let p = provider(&server.endpoint, source.clone());
        let observer = Arc::new(Observer {
            fail_persist,
            ..Default::default()
        });
        let retry = crate::RetryProvider::new(p, crate::RetryConfig { max_retries: 3 });
        let result = crate::provider_observation::observe_chat_completion(
            observer.clone(),
            retry.complete(request()),
        )
        .await;
        assert!(result.is_err());
        if fail_persist {
            assert!(
                matches!(&result, Err(LlmError::InvalidResponse { reason, .. }) if reason == "provider response evidence unavailable")
            );
        }
        assert_eq!(server.requests.lock().await.len(), 1);
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            *observer.events.lock().unwrap(),
            vec!["before", "started", "received", "persisted"]
        );
        server.finish().await;
    }
}

struct Measure {
    stale: Option<Arc<AtomicBool>>,
    seen: Mutex<Vec<Vec<u8>>>,
}
#[async_trait]
impl FinalRequestMeasurer for Measure {
    async fn measure(
        &self,
        input: FinalRequestInput<'_>,
    ) -> Result<FinalRequestMeasurement, FinalRequestError> {
        assert_eq!(
            input.headers.get("chatgpt-account-id").unwrap(),
            "account-0"
        );
        self.seen.lock().unwrap().push(input.body.to_vec());
        if let Some(stale) = &self.stale {
            stale.store(false, Ordering::SeqCst);
        }
        Ok(FinalRequestMeasurement {
            body_sha256: Sha256::digest(input.body).into(),
            provider_id: input.provider_id.into(),
            model: input.model.into(),
            reserved_output_tokens: 2,
            counts: ModelRequestTokenCounts {
                prefix_tokens: 1,
                prompt_tokens: 2,
                input_tokens: 3,
            },
        })
    }
}
#[tokio::test]
async fn actual_codex_final_guard_handoff_uses_immutable_request_and_bound_auth_barrier() {
    for stale in [false, true] {
        let server = codex_http_fixture(|_, _| (200, reply())).await;
        let source = source();
        let p = provider(&server.endpoint, source.clone());
        let m = Arc::new(Measure {
            stale: stale.then(|| source.guard.valid.clone()),
            seen: Mutex::new(vec![]),
        });
        let final_guard = Arc::new(
            FinalRequestGuard::new(
                "codex-fixture".into(),
                "fixture-model".into(),
                format!("{}/responses", server.endpoint).parse().unwrap(),
                guard(),
                m.clone(),
            )
            .unwrap(),
        );
        let result = final_guard.complete(p.as_ref(), request()).await;
        assert_eq!(result.is_err(), stale);
        let requests = server.requests.lock().await;
        assert_eq!(requests.len(), if stale { 0 } else { 1 });
        if !stale {
            assert_eq!(
                requests[0].1.as_bytes(),
                m.seen.lock().unwrap()[0].as_slice()
            );
        }
        drop(requests);
        server.finish().await;
    }
}

#[tokio::test]
async fn guarded_db_codex_requires_host_without_reading_corrupt_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    std::fs::write(&path, "corrupt-original-file").unwrap();
    let mut config = crate::tests::test_llm_config();
    config.backend = "openai_codex".into();
    config.resolved_protocol = Some(crate::registry::ProviderProtocol::OpenAiCodex);
    config.openai_codex = Some(crate::codex_test_helpers::test_codex_config(path.clone()));
    let session = crate::create_session_manager(config.session.clone()).await;
    assert!(matches!(
        build_static_provider_chain_with_guard(&config, session.clone(), guard()).await,
        Err(LlmError::ConfigurationChanged { .. })
    ));
    let source = source();
    let _built = build_static_provider_chain_with_codex_host_session(
        &config,
        session,
        guard(),
        source.clone(),
    )
    .await
    .unwrap();
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "corrupt-original-file"
    );
}

#[tokio::test]
async fn host_transport_failure_is_one_effect_with_no_source_or_model_replay() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut buf = [0; 4096];
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                break;
            }
        }
        // Deliberate disconnect after actual request; no HTTP response.
    });
    let source = source();
    let p = provider(&endpoint, source.clone());
    let retry = RetryProvider::new(p, RetryConfig { max_retries: 3 });
    let observer = Arc::new(Observer::default());
    assert!(
        crate::provider_observation::observe_chat_completion(
            observer.clone(),
            retry.complete(request())
        )
        .await
        .is_err()
    );
    server.await.unwrap();
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(*observer.events.lock().unwrap(), vec!["before", "started"]);
}

#[tokio::test]
async fn host_request_never_installs_token_into_shared_file_authstate() {
    let server = codex_http_fixture(|_, _| (200, reply())).await;
    let raw = Arc::new(
        OpenAiCodexProvider::new_host_with_response_limit(
            "fixture-model",
            &server.endpoint,
            5,
            None,
        )
        .unwrap(),
    );
    let p = crate::token_refreshing::TokenRefreshingProvider::new_host(raw.clone(), source());
    assert_eq!(p.complete(request()).await.unwrap().content, "answer");
    assert!(matches!(
        raw.complete(request()).await,
        Err(LlmError::AuthFailed { .. })
    ));
    assert_eq!(server.requests.lock().await.len(), 1);
    server.finish().await;
}

#[tokio::test]
async fn host_connect_error_preserves_reqwest_cause_without_endpoint_url() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let endpoint = format!("http://{address}/private-endpoint-sentinel");
    let p = provider(&endpoint, source());
    match p.complete(request()).await {
        Err(LlmError::Http(error)) => {
            assert!(error.is_connect());
            assert!(!error.is_builder());
            assert!(error.url().is_none());
            assert!(std::error::Error::source(&error).is_some());
            assert!(!format!("{error:?}").contains("private-endpoint-sentinel"));
        }
        other => panic!("expected actual safe connect error, got {other:?}"),
    }
}

#[test]
fn invalid_codex_endpoint_is_rejected_before_constructing_host_provider() {
    // The explicit endpoint resolver now rejects this configuration before an
    // operation exists. Do not fabricate a Host source or claim a later HTTP
    // builder/observer event for an object that cannot be constructed.
    assert!(matches!(
        OpenAiCodexProvider::new_host_with_response_limit(
            "fixture-model",
            "http://[invalid-host",
            5,
            None
        ),
        Err(LlmError::FinalRequestRejected {
            reason: FinalRequestError::Target
        })
    ));
}

/// Successor of old File+HostManaged refresh-failure case: the actual Host
/// source owns expiry/refresh errors. The SDK never repairs them via File.
struct ExpiredHostSource {
    calls: AtomicUsize,
}
impl std::fmt::Debug for ExpiredHostSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExpiredHostSource").finish_non_exhaustive()
    }
}
#[async_trait]
impl CodexHostSessionSource for ExpiredHostSource {
    async fn session(&self) -> Result<CodexHostSession, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(LlmError::SessionExpired {
            provider: "openai_codex".into(),
        })
    }
}
#[tokio::test]
async fn host_source_expiry_stops_both_model_entry_points_without_file_repair() {
    let server = codex_http_fixture(|path, _| {
        panic!("Host expiry cannot trigger model/device/file refresh I/O: {path}")
    })
    .await;
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("legacy-session.json");
    std::fs::write(&file, "original standalone file bytes").unwrap();
    let source = Arc::new(ExpiredHostSource {
        calls: AtomicUsize::new(0),
    });
    let raw = Arc::new(
        OpenAiCodexProvider::new_host_with_response_limit(
            "fixture-model",
            &server.endpoint,
            5,
            None,
        )
        .unwrap(),
    );
    let p = crate::token_refreshing::TokenRefreshingProvider::new_host(raw, source.clone());
    let mut tools = ToolCompletionRequest::new(vec![], vec![]);
    tools.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
    assert!(matches!(
        p.complete(request()).await,
        Err(LlmError::SessionExpired { .. })
    ));
    assert!(matches!(
        p.complete_with_tools(tools).await,
        Err(LlmError::SessionExpired { .. })
    ));
    assert!(p.list_models().await.unwrap().is_empty());
    assert_eq!(p.model_metadata().await.unwrap().id, "fixture-model");
    assert_eq!(source.calls.load(Ordering::SeqCst), 2);
    assert!(server.requests.lock().await.is_empty());
    assert_eq!(
        std::fs::read_to_string(file).unwrap(),
        "original standalone file bytes"
    );
    server.finish().await;
}

/// Successor of old File+HostManaged auth-rejection case: only actual Host
/// snapshots are accepted, and neither model entry point repairs/replays 401.
#[tokio::test]
async fn host_auth_rejection_never_replays_or_starts_file_or_device_login() {
    let server = codex_http_fixture(|path, _| {
        assert_eq!(path, "/responses");
        (401, r#"{"error":{"message":"expired"}}"#.into())
    })
    .await;
    let source = source();
    let p = provider(&server.endpoint, source.clone());
    let mut tools = ToolCompletionRequest::new(vec![], vec![]);
    tools.dispatch_policy = ProviderDispatchPolicy::SingleSubmission;
    assert!(matches!(
        p.complete(request()).await,
        Err(LlmError::AuthFailed { .. })
    ));
    assert!(matches!(
        p.complete_with_tools(tools).await,
        Err(LlmError::AuthFailed { .. })
    ));
    assert_eq!(source.calls.load(Ordering::SeqCst), 2);
    assert_eq!(server.requests.lock().await.len(), 2);
    server.finish().await;
}

#[tokio::test]
async fn explicit_standalone_file_refresh_and_single_submission_compatibility() {
    use secrecy::ExposeSecret;
    for expired in [false, true] {
        let new_token = make_test_jwt("standalone-refreshed");
        let returned_token = new_token.clone();
        let server = codex_http_fixture(move |path,_| match path {
            "/oauth/token" => {
                assert!(expired,"SingleSubmission auth rejection must not repair File credentials");
                (200,serde_json::json!({"access_token":returned_token,"refresh_token":"rotated-file-refresh","expires_in":3600,"token_type":"Bearer"}).to_string())
            }
            "/responses" => if expired {(200,reply())} else {(401,"unauthorized".into())},
            _ => panic!("unexpected standalone device login: {path}"),
        }).await;
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("standalone.json");
        let mut config = crate::codex_test_helpers::test_codex_config(path.clone());
        config.auth_endpoint = server.endpoint.clone();
        config.api_base_url = server.endpoint.clone();
        let old_token = make_test_jwt("standalone-original");
        let session = Arc::new(OpenAiCodexSessionManager::new(config.clone()).unwrap());
        let original = OpenAiCodexSession {
            access_token: old_token.clone(),
            refresh_token: "original-file-refresh".into(),
            expires_at: chrono::Utc::now()
                + chrono::Duration::seconds(if expired { -1 } else { 3600 }),
            created_at: chrono::Utc::now(),
        };
        session.save_session(&original).await.unwrap();
        session.set_session(original).await;
        let before = std::fs::read(&path).unwrap();
        let inner = Arc::new(
            OpenAiCodexProvider::new(&config.model, &config.api_base_url, &old_token, 5).unwrap(),
        );
        let p = crate::token_refreshing::TokenRefreshingProvider::new(inner, session.clone());
        let mut r = request();
        if expired {
            r.dispatch_policy = ProviderDispatchPolicy::Configured;
        }
        let result = p.complete(r).await;
        let requests = server.requests.lock().await;
        if expired {
            assert_eq!(result.unwrap().content, "answer");
            assert_eq!(
                requests.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
                vec!["/oauth/token", "/responses"]
            );
            assert_eq!(
                session.get_access_token().await.unwrap().expose_secret(),
                &new_token
            );
            assert_ne!(std::fs::read(&path).unwrap(), before);
        } else {
            assert!(matches!(result, Err(LlmError::AuthFailed { .. })));
            assert_eq!(requests.len(), 1);
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        drop(requests);
        server.finish().await;
    }
}
