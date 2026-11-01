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

/// K3 acceptance: a committed provider change whose reload fails leaves
/// desired_publications diverged from runtime_publications, with the
/// failed target captured in runtime_publication_failures. Calling
/// reconcile_provider_mutation heals the divergence without redoing effects.
///
/// # Why ControllableReload must route through publish_primary_from
///
/// `runtime_publication_failures` is populated from
/// `LlmReloadTrigger::publication_status().failed`, which maps to
/// `SwappableLlmProvider::publication_status().failed`. That `failed` field is
/// set *only* by `SwappableLlmProvider::preparation_failed()`, which is called
/// exclusively inside `LlmReloadHandle::publish_primary_from` when the preparer
/// closure returns `Err(ProviderPreparationFailure)`.
///
/// A trigger that returns bare `Err(String)` from `reload()` without going through
/// `publish_primary_from` never sets `state.failed`, so
/// `runtime_publication_failures` stays empty and the K3 assertions would fail.
///
/// This test therefore wires a real `LlmReloadHandle` and delegates to a real
/// `RebornLlmReloadAdapter` on the success path (so `desired == runtime` holds
/// after a successful reload) and calls `handle.publish_primary_from` with a
/// failing closure on the failure path (so `state.failed` is populated).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_provider_reload_failure_captured_and_reconciled_via_mutation() {
    use brassclaw_product_workflow::LlmConfigService;
    use std::sync::atomic::{AtomicBool, Ordering};

    // ControllableReload wraps a real RebornLlmReloadAdapter (success path) and
    // holds its LlmReloadHandle separately (failure injection path).
    //
    // On success:  delegate to inner.reload() — produces a real Configured
    //              publication from the DB so desired == runtime holds.
    // On failure:  call handle.publish_primary_from with a failing closure so
    //              preparation_failed() sets state.failed on the SwappableLlmProvider
    //              and runtime_publication_failures is populated by build_snapshot.
    struct ControllableReload {
        inner: Arc<crate::llm_reload::RebornLlmReloadAdapter>,
        handle: Arc<brassclaw_llm::LlmReloadHandle>,
        should_fail: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl crate::llm_config_service::LlmReloadTrigger for ControllableReload {
        async fn reload(&self) -> Result<brassclaw_llm::ProviderPublication, String> {
            if self.should_fail.load(Ordering::SeqCst) {
                // Capture the current desired target before failing so the
                // failure record is meaningful (the thing we tried to publish).
                let desired = self.inner.publication();
                self.handle
                    .publish_primary_from(|| {
                        let desired = desired.clone();
                        async move {
                            Err(brassclaw_llm::ProviderPreparationFailure::selected(
                                brassclaw_llm::LlmError::ConfigurationChanged {
                                    provider: "k3-controllable-reload-failure".into(),
                                },
                                desired,
                            ))
                        }
                    })
                    .await
                    .map_err(|_| "deliberate controllable reload failure".to_owned())
            } else {
                // Real reload through the DB — produces a Configured publication
                // that matches what build_snapshot computes as desired.
                self.inner.reload().await
            }
        }

        fn publication(&self) -> brassclaw_llm::ProviderPublication {
            self.inner.publication()
        }

        fn publication_status(&self) -> brassclaw_llm::ProviderPublicationStatus {
            self.inner.publication_status()
        }
    }

    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let tenant = "native-k3-reload-failure";
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        tenant,
    ));

    // Seed a provider and select it as Kohai.
    repo.upsert(
        serde_json::from_value(serde_json::json!({
            "id": "k3-provider",
            "protocol": "open_ai_completions",
            "default_base_url": "http://127.0.0.1:18888/v1",
            "base_url_required": true,
            "api_key_required": false,
            "model_env": "UNUSED_K3_RELOAD_MODEL",
            "default_model": "k3-model-v1",
            "description": "K3 reload failure fixture"
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[("llm.default.provider_id", Some("k3-provider"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();

    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("k3-reload-failure-master-key"),
            "k3-reload-key-owner",
        )
        .unwrap(),
    ));

    // Build the LlmReloadHandle + SwappableLlmProvider first so we can pass
    // the same Arc<LlmReloadHandle> to both the adapter (success path) and the
    // ControllableReload (failure injection path).
    let primary = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let handle = Arc::new(brassclaw_llm::LlmReloadHandle::new(
        Arc::clone(&primary),
        None,
    ));
    let session = Arc::new(brassclaw_llm::SessionManager::new(Default::default()));
    let inner = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        Arc::clone(&handle),
        Arc::clone(&session),
        keys.clone(),
        rig.pool.clone(),
        tenant,
    ));
    let should_fail = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(ControllableReload {
        inner: Arc::clone(&inner),
        handle: Arc::clone(&handle),
        should_fail: Arc::clone(&should_fail),
    });
    let service =
        crate::RebornLlmConfigService::new(keys.clone(), rig.pool.clone(), repo.clone(), tenant)
            .with_reload_trigger(reload as Arc<dyn crate::llm_config_service::LlmReloadTrigger>);

    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("k3-operator").unwrap(),
        None,
        None,
    );

    // --- Baseline: initial snapshot. desired == runtime (both Unconfigured
    //     because no successful reload has run yet). ---
    let initial = service.snapshot(caller.clone()).await.unwrap();
    assert_eq!(
        initial.desired_publications.get("kohai"),
        initial.runtime_publications.get("kohai"),
        "before any selection change, desired and runtime publications must agree"
    );
    assert!(
        !initial.runtime_publication_failures.contains_key("kohai"),
        "no failure before any selection is made"
    );

    // --- Perform a selection with reload succeeding to establish a baseline.
    //     inner.reload() resolves the DB selection and calls publish_primary_from
    //     with the real Configured publication, so runtime matches desired. ---
    let current_revision = repo
        .get_revision("k3-provider", false)
        .await
        .unwrap()
        .unwrap()
        .revision;
    let baseline = service
        .set_active(
            caller.clone(),
            serde_json::from_value(serde_json::json!({
                "mutation_id": uuid::Uuid::new_v4(),
                "provider_id": "k3-provider",
                "model": "k3-model-v1",
                "role": "kohai",
                "expected_revision": current_revision,
                "expected_selection_ref": initial.role_selection_refs.get("kohai").cloned()
                    .unwrap_or_default()
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        baseline.desired_publications["kohai"], baseline.runtime_publications["kohai"],
        "after a successful selection + reload, desired and runtime must agree"
    );
    assert!(
        !baseline.runtime_publication_failures.contains_key("kohai"),
        "no failure after successful selection"
    );
    let working_desired = baseline.desired_publications["kohai"].clone();

    // --- Arm the reload failure, then edit the provider definition.
    //     The upsert commits the DB change but the reload returns Err. ---
    should_fail.store(true, Ordering::SeqCst);
    let current_revision = repo
        .get_revision("k3-provider", false)
        .await
        .unwrap()
        .unwrap()
        .revision;
    let mutation_id = uuid::Uuid::new_v4();
    let upsert_result = service
        .upsert_provider(
            caller.clone(),
            serde_json::from_value(serde_json::json!({
                "mutation_id": mutation_id,
                "id": "k3-provider",
                "expected_revision": current_revision,
                "adapter": "open_ai_completions",
                "name": "K3 description update — reload will fail"
            }))
            .unwrap(),
        )
        .await;
    // The DB commit succeeded but reload failed → service returns Err(Unavailable).
    assert!(
        upsert_result.is_err(),
        "upsert must return an error when the post-commit reload fails"
    );

    // --- Snapshot after failed reload: desired diverges from runtime.
    //     desired_publications["kohai"] is computed from the DB (definition_revision
    //     incremented by upsert). runtime_publications["kohai"] still reflects the
    //     last successful reload. runtime_publication_failures["kohai"] is set
    //     because publish_primary_from called preparation_failed() on the provider. ---
    let after_failure = service.snapshot(caller.clone()).await.unwrap();
    assert_ne!(
        after_failure.desired_publications["kohai"], after_failure.runtime_publications["kohai"],
        "desired and runtime publications must diverge after a failed reload"
    );
    assert_eq!(
        after_failure.runtime_publications["kohai"], working_desired,
        "runtime publication must still reflect the last successful reload"
    );
    assert!(
        after_failure
            .runtime_publication_failures
            .contains_key("kohai"),
        "runtime_publication_failures must capture the failed reload target"
    );

    // --- Reconcile while reload is still failing: must return Err. ---
    let reconcile_failing = service
        .reconcile_provider_mutation(caller.clone(), mutation_id)
        .await;
    assert!(
        reconcile_failing.is_err(),
        "reconcile must also fail when reload is still broken"
    );

    // --- Repair: allow reload to succeed, then reconcile.
    //     reconcile_provider_mutation re-runs finish_provider_mutation, which calls
    //     refresh_running_provider → inner.reload() → publish_primary_from with the
    //     real Configured publication → desired and runtime converge. ---
    should_fail.store(false, Ordering::SeqCst);
    let reconciled = service
        .reconcile_provider_mutation(caller.clone(), mutation_id)
        .await
        .unwrap();
    assert_eq!(
        reconciled.desired_publications["kohai"], reconciled.runtime_publications["kohai"],
        "after successful reconciliation desired and runtime must agree"
    );
    assert!(
        !reconciled
            .runtime_publication_failures
            .contains_key("kohai"),
        "no failure after successful reconciliation"
    );
    assert_eq!(
        reconciled.mutation_receipt.as_ref().map(|r| r.mutation_id),
        Some(mutation_id),
        "the original mutation receipt is returned"
    );

    repo.delete("k3-provider").await.unwrap();
}
