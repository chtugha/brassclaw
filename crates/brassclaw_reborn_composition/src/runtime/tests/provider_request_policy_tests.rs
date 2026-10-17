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
    }
    async fn endpoint(
        State(state): State<Arc<Endpoint>>,
        Json(body): Json<serde_json::Value>,
    ) -> Response {
        state.calls.fetch_add(1, Ordering::SeqCst);
        match state.mode.load(Ordering::SeqCst) {
            1 => tokio::time::sleep(Duration::from_millis(1300)).await,
            2 if state
                .failures
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                    count.checked_sub(1)
                })
                .is_ok() =>
            {
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
    let service = crate::RebornLlmConfigService::new(keys, rig.pool.clone(), repo.clone(), tenant)
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
        service.upsert_provider(caller,serde_json::from_value(serde_json::json!({"id":"request-provider","expected_revision":revision,"adapter":"open_ai_completions","request_policy":policy})).unwrap()).await.unwrap()
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
    assert!(
        retained.provider.complete(request()).await.is_ok(),
        "already retained clients keep their admitted deadline"
    );
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
    // A complete old client remains usable while both newly selected role
    // chains take the finite body limit. Capacity failure must not trigger retry.
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
    assert!(retained.provider.complete(request()).await.is_ok());
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
    assert!(service.upsert_provider(caller.clone(),serde_json::from_value(serde_json::json!({"id":"request-provider","expected_revision":before.revision,"adapter":"open_ai_completions","request_policy":invalid})).unwrap()).await.is_err());
    assert_eq!(
        repo.get_revision("request-provider", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        before.revision
    );
    let preserved = service.upsert_provider(caller.clone(),serde_json::from_value(serde_json::json!({"id":"request-provider","expected_revision":before.revision,"adapter":"open_ai_completions","name":"policy retained without override"})).unwrap()).await.unwrap();
    assert_eq!(
        preserved
            .providers
            .iter()
            .find(|row| row.id == "request-provider")
            .unwrap()
            .request_policy,
        circuit
    );
    assert!(serde_json::from_value::<brassclaw_product_workflow::UpsertLlmProviderRequest>(serde_json::json!({"id":"request-provider","adapter":"open_ai_completions","request_policy":null})).is_err());
    stop.cancel();
    server.await.unwrap();
}
