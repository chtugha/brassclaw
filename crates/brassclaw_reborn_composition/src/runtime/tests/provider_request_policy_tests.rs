use super::*;
use brassclaw_llm::{CompletionRequest, LlmProvider, ProviderRequestPolicy};
use brassclaw_product_workflow::LlmConfigService;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_live_provider_request_policy_reaches_selected_clients() {
    use axum::{
        Json, Router,
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::post,
    };
    use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
    #[derive(Default)]
    struct Endpoint {
        calls: AtomicUsize,
        mode: AtomicU8,
        failures: AtomicUsize,
        notify_failures: std::sync::atomic::AtomicBool,
        failure_seen: tokio::sync::Notify,
    }
    impl Endpoint {
        fn take_failure(&self) -> bool {
            let mut previous = self.failures.load(Ordering::SeqCst);
            loop {
                let Some(next) = previous.checked_sub(1) else {
                    return false;
                };
                match self.failures.compare_exchange_weak(
                    previous,
                    next,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => return true,
                    Err(current) => previous = current,
                }
            }
        }
    }
    async fn endpoint(
        State(state): State<Arc<Endpoint>>,
        Json(body): Json<serde_json::Value>,
    ) -> Response {
        state.calls.fetch_add(1, Ordering::SeqCst);
        match state.mode.load(Ordering::SeqCst) {
            1 => tokio::time::sleep(Duration::from_millis(1300)).await,
            2 if state.take_failure() => {
                if state.notify_failures.load(Ordering::SeqCst) {
                    state.failure_seen.notify_one();
                }
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [("retry-after", "2")],
                    "deliberate temporary rejection",
                )
                    .into_response();
            }
            3 => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "deliberate circuit failure",
                )
                    .into_response();
            }
            4 => {
                return (StatusCode::UNAUTHORIZED, "deliberate credential rejection")
                    .into_response();
            }
            5 => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(axum::body::Body::from(
                        "context length exceeded".repeat(100),
                    ))
                    .unwrap();
            }
            6 => {
                let stream = futures::stream::iter([Ok::<_, std::convert::Infallible>(
                    "chunked-response".repeat(200),
                )]);
                return Response::builder()
                    .body(axum::body::Body::from_stream(stream))
                    .unwrap();
            }
            _ => {}
        }
        Json(serde_json::json!({"id":"native-request-answer","object":"chat.completion","created":1,"model":body["model"],"choices":[{"index":0,"message":{"role":"assistant","content":"request policy answer"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}})).into_response()
    }
    let state = Arc::new(Endpoint::default());
    let listener = native_http::bind_loopback().await.unwrap();
    let address = listener.local_addr().unwrap();
    let stop = tokio_util::sync::CancellationToken::new();
    let server_stop = stop.clone();
    let app = Router::new()
        .route("/v1/chat/completions", post(endpoint))
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        native_http::serve(listener, app, server_stop.cancelled_owned())
            .await
            .unwrap();
    });
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let tenant = "native-request-policy";
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        tenant,
    ));
    let initial = ProviderRequestPolicy {
        timeout_secs: 2,
        max_retries: 0,
        circuit_breaker_threshold: None,
        circuit_breaker_recovery_secs: 1,
        retry_timing: Default::default(),
        max_response_bytes: None,
    };
    repo.upsert(serde_json::from_value(serde_json::json!({"id":"request-provider","protocol":"open_ai_completions","default_base_url":format!("http://{address}/v1"),"base_url_required":true,"api_key_required":false,"model_env":"UNUSED_REQUEST_POLICY_MODEL","default_model":"request-model","description":"Native request policy fixture","request_policy":initial})).unwrap()).await.unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.provider_id", Some("request-provider")),
            ("llm.sempai.provider_id", Some("request-provider")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("native-request-policy-retained-master-key"),
            "native-request-key-owner",
        )
        .unwrap(),
    ));
    let primary = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let sempai = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        Arc::new(brassclaw_llm::LlmReloadHandle::new(primary.clone(), None)),
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
        keys.clone(),
        rig.pool.clone(),
        tenant,
    ));
    let service =
        crate::RebornLlmConfigService::new(keys.clone(), rig.pool.clone(), repo.clone(), tenant)
            .with_reload_trigger(reload)
            .with_sempai_swappable(sempai.clone());
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("request-operator").unwrap(),
        None,
        None,
    );
    async fn publish(
        service: &crate::RebornLlmConfigService,
        repo: &crate::PgProviderRepo,
        caller: brassclaw_product_workflow::WebUiAuthenticatedCaller,
        policy: ProviderRequestPolicy,
    ) -> brassclaw_product_workflow::LlmConfigSnapshot {
        let revision = repo
            .get_revision("request-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision;
        service.upsert_provider(caller,serde_json::from_value(serde_json::json!({"mutation_id":uuid::Uuid::new_v4(),"id":"request-provider","expected_revision":revision,"adapter":"open_ai_completions","request_policy":policy})).unwrap()).await.unwrap()
    }
    let snapshot = publish(&service, &repo, caller.clone(), initial).await;
    assert_eq!(
        snapshot
            .providers
            .iter()
            .find(|row| row.id == "request-provider")
            .unwrap()
            .request_policy,
        initial
    );
    let retained = primary.retained_selection();
    let publication = primary.publication();
    assert!(primary.set_model("mutable-client-retarget").is_err());
    assert_eq!(primary.publication(), publication);
    assert_eq!(retained.provider.active_model_name(), "request-model");
    let request = || {
        CompletionRequest::new(vec![brassclaw_llm::ChatMessage::user(
            "native request policy",
        )])
        .with_max_tokens(16)
    };
    state.mode.store(1, Ordering::SeqCst);
    assert!(primary.complete(request()).await.is_ok());
    let short = ProviderRequestPolicy {
        timeout_secs: 1,
        ..initial
    };
    publish(&service, &repo, caller.clone(), short).await;
    assert!(primary.complete(request()).await.is_err());
    assert!(sempai.complete(request()).await.is_err());
    let before = state.calls.load(Ordering::SeqCst);
    assert!(
        matches!(
            retained.provider.complete(request()).await,
            Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
        ),
        "retained clients cannot ignore a newly tightened transport deadline"
    );
    assert_eq!(state.calls.load(Ordering::SeqCst), before);
    state.mode.store(2, Ordering::SeqCst);
    let retry = ProviderRequestPolicy {
        max_retries: 1,
        retry_timing: brassclaw_llm::RetryTimingPolicy {
            base_delay_ms: 5,
            max_backoff_ms: 10,
            jitter_percent: 0,
            max_retry_after_secs: 1,
        },
        ..initial
    };
    publish(&service, &repo, caller.clone(), retry).await;
    state.failures.store(1, Ordering::SeqCst);
    let before = state.calls.load(Ordering::SeqCst);
    let retry_started = std::time::Instant::now();
    assert!(primary.complete(request()).await.is_ok());
    assert!(
        retry_started.elapsed() >= Duration::from_millis(900)
            && retry_started.elapsed() < Duration::from_millis(1800),
        "the two-second provider hint must reach the configured one-second wait ceiling"
    );
    assert_eq!(
        state.calls.load(Ordering::SeqCst),
        before + 2,
        "one retry means initial request plus one additional attempt"
    );
    state.mode.store(4, Ordering::SeqCst);
    let before = state.calls.load(Ordering::SeqCst);
    assert!(matches!(
        primary.complete(request()).await,
        Err(brassclaw_llm::LlmError::AuthFailed { .. })
    ));
    assert_eq!(
        state.calls.load(Ordering::SeqCst),
        before + 1,
        "credential rejection is not transient and must not be retried"
    );
    let circuit = ProviderRequestPolicy {
        circuit_breaker_threshold: Some(1),
        ..initial
    };
    publish(&service, &repo, caller.clone(), circuit).await;
    state.mode.store(3, Ordering::SeqCst);
    assert!(primary.complete(request()).await.is_err());
    state.mode.store(0, Ordering::SeqCst);
    let before = state.calls.load(Ordering::SeqCst);
    assert!(primary.complete(request()).await.is_err());
    assert_eq!(
        state.calls.load(Ordering::SeqCst),
        before,
        "an open circuit rejects before network dispatch"
    );
    assert!(
        sempai.complete(request()).await.is_ok(),
        "role connections keep independent failure state"
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(
        primary.complete(request()).await.is_ok(),
        "configured recovery wait admits a trial request"
    );
    // Both selected roles take the finite body limit. Retained clients cannot
    // bypass it; capacity failure must not trigger retry.
    let bounded = ProviderRequestPolicy {
        max_response_bytes: Some(128),
        max_retries: 2,
        ..initial
    };
    publish(&service, &repo, caller.clone(), bounded).await;
    for mode in [0, 5, 6] {
        state.mode.store(mode, Ordering::SeqCst);
        for role in [&primary, &sempai] {
            let before = state.calls.load(Ordering::SeqCst);
            assert!(matches!(
                role.complete(request()).await,
                Err(brassclaw_llm::LlmError::ResponseTooLarge { limit: 128, .. })
            ));
            assert_eq!(
                state.calls.load(Ordering::SeqCst),
                before + 1,
                "response capacity does not authorize another provider effect"
            );
        }
    }
    state.mode.store(0, Ordering::SeqCst);
    let before = state.calls.load(Ordering::SeqCst);
    assert!(matches!(
        retained.provider.complete(request()).await,
        Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
    ));
    assert_eq!(state.calls.load(Ordering::SeqCst), before);
    publish(
        &service,
        &repo,
        caller.clone(),
        ProviderRequestPolicy {
            max_response_bytes: Some(4096),
            ..initial
        },
    )
    .await;
    assert!(primary.complete(request()).await.is_ok());
    assert!(sempai.complete(request()).await.is_ok());
    publish(&service, &repo, caller.clone(), circuit).await;
    let before = repo
        .get_revision("request-provider", false)
        .await
        .unwrap()
        .unwrap();
    let invalid = ProviderRequestPolicy {
        timeout_secs: 0,
        ..initial
    };
    assert!(service.upsert_provider(caller.clone(),serde_json::from_value(serde_json::json!({"mutation_id":uuid::Uuid::new_v4(),"id":"request-provider","expected_revision":before.revision,"adapter":"open_ai_completions","request_policy":invalid})).unwrap()).await.is_err());
    assert_eq!(
        repo.get_revision("request-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        before.revision
    );
    let preserved = service.upsert_provider(caller.clone(),serde_json::from_value(serde_json::json!({"mutation_id":uuid::Uuid::new_v4(),"id":"request-provider","expected_revision":before.revision,"adapter":"open_ai_completions","name":"policy retained without override"})).unwrap()).await.unwrap();
    assert_eq!(
        preserved
            .providers
            .iter()
            .find(|row| row.id == "request-provider")
            .unwrap()
            .request_policy,
        circuit
    );
    assert!(serde_json::from_value::<brassclaw_product_workflow::UpsertLlmProviderRequest>(serde_json::json!({"mutation_id":uuid::Uuid::new_v4(),"id":"request-provider","adapter":"open_ai_completions","request_policy":null})).is_err());
    // External writers do not call the settings reload path. The real
    // retained factory still checks compatibility before any HTTP/cache use.
    publish(&service, &repo, caller.clone(), initial).await;
    state.mode.store(0, Ordering::SeqCst);
    let pinned = primary.retained_selection();
    let mut current = repo
        .get_revision("request-provider", false)
        .await
        .unwrap()
        .unwrap()
        .definition;
    current.description = "external description-only successor".into();
    repo.upsert(current.clone()).await.unwrap();
    assert!(
        pinned.provider.complete(request()).await.is_ok(),
        "compatible descriptive edits preserve old targets"
    );
    let before = state.calls.load(Ordering::SeqCst);
    let mut tightened = current.clone();
    tightened.context_window_tokens = Some(8);
    repo.upsert(tightened).await.unwrap();
    assert!(matches!(
        pinned.provider.complete(request()).await,
        Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
    ));
    assert_eq!(
        state.calls.load(Ordering::SeqCst),
        before,
        "tightening blocks before sending even the output bound"
    );
    let mut moved = current.clone();
    moved.default_base_url = Some(format!("http://{address}/moved/v1"));
    repo.upsert(moved).await.unwrap();
    assert!(matches!(
        pinned.provider.complete(request()).await,
        Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
    ));
    assert_eq!(
        state.calls.load(Ordering::SeqCst),
        before,
        "no endpoint substitution under a retained target"
    );
    repo.upsert(current).await.unwrap();
    keys.put(
        "request-provider",
        secrecy::SecretString::from("actual-native-successor-key"),
    )
    .await
    .unwrap();
    assert!(matches!(
        pinned.provider.complete(request()).await,
        Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
    ));
    assert_eq!(
        state.calls.load(Ordering::SeqCst),
        before,
        "key-only rotation fences the old credential"
    );
    publish(&service, &repo, caller.clone(), retry).await;
    assert!(
        primary.complete(request()).await.is_ok(),
        "new work uses the authenticated successor"
    );
    state.mode.store(2, Ordering::SeqCst);
    state.failures.store(1, Ordering::SeqCst);
    state.notify_failures.store(true, Ordering::SeqCst);
    let before = state.calls.load(Ordering::SeqCst);
    let waiting = {
        let primary = primary.clone();
        let request = request();
        tokio::spawn(async move { primary.complete(request).await })
    };
    tokio::time::timeout(Duration::from_secs(5), state.failure_seen.notified())
        .await
        .unwrap();
    repo.delete("request-provider").await.unwrap();
    assert!(matches!(
        waiting.await.unwrap(),
        Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
    ));
    assert_eq!(
        state.calls.load(Ordering::SeqCst),
        before + 1,
        "deactivation during retry wait prevents the second dispatch"
    );
    stop.cancel();
    server.await.unwrap();
}
