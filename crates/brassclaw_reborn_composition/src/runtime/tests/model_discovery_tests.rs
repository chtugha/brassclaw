use super::*;
use brassclaw_product_workflow::LlmConfigService;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_model_discovery_uses_explicit_endpoint_policy_and_atomic_credentials() {
    use axum::{
        Router,
        extract::{OriginalUri, State},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Response},
    };
    use std::sync::atomic::{AtomicU8, Ordering};
    type RequestLog =
        Arc<tokio::sync::Mutex<Vec<(String, Option<String>, Option<String>, Option<String>)>>>;
    #[derive(Clone, Default)]
    struct Endpoint {
        mode: Arc<AtomicU8>,
        requests: RequestLog,
    }
    async fn endpoint(
        State(state): State<Endpoint>,
        OriginalUri(uri): OriginalUri,
        headers: HeaderMap,
    ) -> Response {
        let value = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        state.requests.lock().await.push((
            uri.to_string(),
            value("authorization"),
            value("x-api-key"),
            value("x-goog-api-key"),
        ));
        match state.mode.load(Ordering::SeqCst) {
            1 => (StatusCode::FOUND, [("location", "/redirect-target")], "").into_response(),
            2 => (StatusCode::UNAUTHORIZED, "denied").into_response(),
            3 => (StatusCode::OK, "<html>not a model list</html>").into_response(),
            4 => (StatusCode::OK, "x".repeat(2048)).into_response(),
            5 => {
                tokio::time::sleep(Duration::from_millis(1300)).await;
                (StatusCode::OK, r#"{"data":[{"id":"slow-model"}]}"#).into_response()
            }
            6 => (
                StatusCode::OK,
                r#"{"data":[{"id":"duplicate"},{"id":"duplicate"}]}"#,
            )
                .into_response(),
            7 => {
                if uri.query() == Some("after_id=page-one") {
                    (
                        StatusCode::OK,
                        r#"{"data":[{"id":"page-two"}],"has_more":false,"last_id":"page-two"}"#,
                    )
                        .into_response()
                } else {
                    (
                        StatusCode::OK,
                        r#"{"data":[{"id":"page-one"}],"has_more":true,"last_id":"page-one"}"#,
                    )
                        .into_response()
                }
            }
            8 => {
                let body = if uri.query() == Some("pageToken=next-page") {
                    r#"{"models":[{"name":"models/gemini-two"}]}"#
                } else {
                    r#"{"models":[{"name":"models/gemini-one"}],"nextPageToken":"next-page"}"#
                };
                (StatusCode::OK, body).into_response()
            }
            9 => (
                StatusCode::OK,
                r#"{"models":[{"name":"local-one"},{"name":"local-two"}]}"#,
            )
                .into_response(),
            10 => (
                StatusCode::OK,
                r#"{"models":[],"nextPageToken":"repeated"}"#,
            )
                .into_response(),
            _ => (
                StatusCode::OK,
                r#"{"data":[{"id":"chat-model"},{"id":"embedding-model"}]}"#,
            )
                .into_response(),
        }
    }
    let state = Endpoint::default();
    let listener = native_http::bind_loopback().await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let stop = tokio_util::sync::CancellationToken::new();
    let server_stop = stop.clone();
    let router = Router::new().fallback(endpoint).with_state(state.clone());
    let server = tokio::spawn(async move {
        native_http::serve(listener, router, server_stop.cancelled_owned())
            .await
            .unwrap();
    });
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let tenant = "native-model-discovery";
    let repo = Arc::new(crate::PgProviderRepo::new(
        rig.pool.as_ref().clone(),
        tenant,
    ));
    let definition = serde_json::from_value(json!({
        "id":"openai","protocol":"open_ai_completions","default_base_url":url,
        "base_url_required":true,"api_key_required":true,"model_env":"UNUSED_DISCOVERY_MODEL",
        "default_model":"chat-model","description":"Explicit local model discovery",
        "api_key_env":"UNUSED_DISCOVERY_KEY"
    }))
    .unwrap();
    repo.upsert(definition).await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("native-model-discovery-retained-master-key"),
            "native-discovery-key-owner",
        )
        .unwrap(),
    ));
    keys.put("openai", secrecy::SecretString::from("native-old-key"))
        .await
        .unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[("llm.default.provider_id", Some("openai"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let primary = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let reload_handle = Arc::new(brassclaw_llm::LlmReloadHandle::new(primary.clone(), None));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        reload_handle,
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
        keys.clone(),
        rig.pool.clone(),
        tenant,
    ));
    let service = Arc::new(
        crate::RebornLlmConfigService::new(keys.clone(), rig.pool.clone(), repo.clone(), tenant)
            .with_reload_trigger(reload),
    );
    let caller = brassclaw_product_workflow::WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(tenant).unwrap(),
        brassclaw_host_api::UserId::new("discovery-operator").unwrap(),
        None,
        None,
    );
    let request =
        |adapter: &str, endpoint: &str, policy: Option<serde_json::Value>, key: Option<&str>| {
            let mut value = json!({"provider_id":"openai","adapter":adapter,"base_url":endpoint});
            if let Some(policy) = policy {
                value["model_discovery"] = policy;
            }
            if let Some(key) = key {
                value["api_key"] = json!(key);
            }
            serde_json::from_value(value).unwrap()
        };
    rig.pool.resize(1);
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        service.list_models(
            caller.clone(),
            request("open_ai_completions", &url, None, None),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    rig.pool.resize(16);
    assert!(result.ok);
    assert_eq!(result.models, ["chat-model", "embedding-model"]);
    assert_eq!(
        state.requests.lock().await[0],
        (
            "/v1/models".into(),
            Some("Bearer native-old-key".into()),
            None,
            None
        )
    );
    let anthropic = service
        .list_models(
            caller.clone(),
            request("anthropic", &url, None, Some("draft-anthropic-key")),
        )
        .await
        .unwrap();
    assert!(anthropic.ok);
    assert_eq!(
        state.requests.lock().await[1],
        (
            "/v1/models".into(),
            None,
            Some("draft-anthropic-key".into()),
            None
        )
    );
    let root_url = url.trim_end_matches("/v1");
    state.mode.store(8, Ordering::SeqCst);
    let gemini = service
        .list_models(
            caller.clone(),
            request("gemini", root_url, None, Some("draft-gemini-key")),
        )
        .await
        .unwrap();
    assert!(gemini.ok);
    assert_eq!(gemini.models, ["gemini-one", "gemini-two"]);
    let last = state.requests.lock().await.last().unwrap().clone();
    assert_eq!(last.0, "/v1beta/models?pageToken=next-page");
    assert!(last.1.is_none() && last.2.is_none());
    assert_eq!(last.3.as_deref(), Some("draft-gemini-key"));
    state.mode.store(10, Ordering::SeqCst);
    let before = state.requests.lock().await.len();
    let repeated = service
        .list_models(
            caller.clone(),
            request("gemini", root_url, None, Some("draft-gemini-key")),
        )
        .await
        .unwrap();
    assert!(!repeated.ok && repeated.models.is_empty());
    assert_eq!(
        state.requests.lock().await.len(),
        before + 2,
        "repeated pagination cannot loop until deadline or return partial success"
    );
    state.mode.store(9, Ordering::SeqCst);
    let ollama = service
        .list_models(caller.clone(), request("ollama", root_url, None, None))
        .await
        .unwrap();
    assert!(ollama.ok);
    assert_eq!(ollama.models, ["local-one", "local-two"]);
    assert_eq!(state.requests.lock().await.last().unwrap().0, "/api/tags");
    state.mode.store(0, Ordering::SeqCst);
    let nearai = service
        .list_models(
            caller.clone(),
            request("nearai", root_url, None, Some("draft-near-key")),
        )
        .await
        .unwrap();
    assert!(nearai.ok);
    let last = state.requests.lock().await.last().unwrap().clone();
    assert_eq!(last.0, "/v1/models");
    assert_eq!(last.1.as_deref(), Some("Bearer draft-near-key"));
    for mode in [1, 2, 3, 4, 6] {
        state.mode.store(mode, Ordering::SeqCst);
        let before = state.requests.lock().await.len();
        let result = service
            .list_models(
                caller.clone(),
                request(
                    "open_ai_completions",
                    &url,
                    Some(json!({"timeout_secs":2,"max_response_bytes":1024})),
                    None,
                ),
            )
            .await
            .unwrap();
        assert!(!result.ok && result.models.is_empty());
        assert_eq!(
            state.requests.lock().await.len(),
            before + 1,
            "failure/redirect cannot redispatch or fabricate models"
        );
    }
    state.mode.store(3, Ordering::SeqCst);
    let rejected_probe = service
        .test_connection(
            caller.clone(),
            request("open_ai_completions", &url, None, None),
        )
        .await
        .unwrap();
    assert!(
        !rejected_probe.ok,
        "constructing a provider cannot report a successful connection"
    );
    state.mode.store(0, Ordering::SeqCst);
    let accepted_probe = service
        .test_connection(
            caller.clone(),
            request("open_ai_completions", &url, None, None),
        )
        .await
        .unwrap();
    assert!(accepted_probe.ok && accepted_probe.message.contains("2 models"));
    state.mode.store(7, Ordering::SeqCst);
    let paged = service
        .list_models(
            caller.clone(),
            request(
                "anthropic",
                &url,
                Some(json!({"timeout_secs":2,"max_response_bytes":2048})),
                Some("paged-key"),
            ),
        )
        .await
        .unwrap();
    assert!(paged.ok);
    assert_eq!(paged.models, ["page-one", "page-two"]);
    assert!(
        state
            .requests
            .lock()
            .await
            .last()
            .unwrap()
            .0
            .ends_with("?after_id=page-one")
    );
    let unsupported_page = service
        .list_models(
            caller.clone(),
            request("open_ai_completions", &url, None, None),
        )
        .await
        .unwrap();
    assert!(!unsupported_page.ok && unsupported_page.models.is_empty());
    state.mode.store(5, Ordering::SeqCst);
    let timeout = service
        .list_models(
            caller.clone(),
            request(
                "open_ai_completions",
                &url,
                Some(json!({"timeout_secs":1,"max_response_bytes":1024})),
                None,
            ),
        )
        .await
        .unwrap();
    assert!(!timeout.ok && timeout.message.contains("deadline"));
    let view = service.snapshot(caller.clone()).await.unwrap();
    assert!(
        view.providers
            .iter()
            .find(|provider| provider.id == "openai")
            .unwrap()
            .can_list_models,
        "available model discovery is determined by its implemented adapter, not an absent/stale setup label"
    );
    let before = repo.get_revision("openai", true).await.unwrap().unwrap();
    service
        .upsert_provider(
            caller.clone(),
            serde_json::from_value(json!({
                "id":"openai","expected_revision":before.revision,"adapter":"open_ai_completions",
                "model_discovery":{"timeout_secs":3,"max_response_bytes":4096}
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        service
            .list_models(
                caller.clone(),
                request("open_ai_completions", &url, None, None)
            )
            .await
            .unwrap()
            .ok
    );
    let saved = repo.get_revision("openai", true).await.unwrap().unwrap();
    assert_eq!(saved.definition.model_discovery.timeout_secs, 3);
    assert_eq!(saved.definition.model_discovery.max_response_bytes, 4096);
    assert_eq!(primary.retained_selection().provider_id, "openai");
    state.mode.store(0, Ordering::SeqCst);
    crate::db_config::save_config_changes(
        &rig.pool,
        tenant,
        &[
            ("llm.default.model", Some("generation-zero")),
            ("llm.sempai.provider_id", Some("openai")),
            ("llm.sempai.model", Some("generation-zero")),
            ("embedding.provider_id", Some("openai")),
            ("embedding.model", Some("generation-zero")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let admin = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), tenant);
    let roles = [
        brassclaw_llm::ProviderRole::Kohai,
        brassclaw_llm::ProviderRole::Sempai,
        brassclaw_llm::ProviderRole::Embedding,
    ];
    rig.pool.resize(1);
    for role in roles {
        let captured = tokio::time::timeout(
            Duration::from_secs(5),
            admin.capture_selection(
                crate::provider_admin::ProviderSelectionSource::Role(role),
                &keys,
            ),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        assert_eq!(captured.selection.model.as_deref(), Some("generation-zero"));
        assert_eq!(
            captured.definition.default_base_url.as_deref(),
            Some(url.as_str())
        );
        assert_eq!(
            secrecy::ExposeSecret::expose_secret(captured.credential.as_ref().unwrap()),
            "native-old-key"
        );
    }
    rig.pool.resize(16);
    let mut blocker = rig.pool.get().await.unwrap();
    let tx = blocker.transaction().await.unwrap();
    tx.query("SELECT scope FROM brassclaw_secrets WHERE tenant_id='native-discovery-key-owner' AND name='llm_provider_openai_api_key' FOR UPDATE",&[]).await.unwrap();
    let pending_service = service.clone();
    let pending_caller = caller.clone();
    let old_request = request("open_ai_completions", &url, None, None);
    let pending = tokio::spawn(async move {
        pending_service
            .list_models(pending_caller, old_request)
            .await
    });
    let mut pending_roles = Vec::new();
    for role in roles {
        let admin = admin.clone();
        let keys = keys.clone();
        pending_roles.push(tokio::spawn(async move {
            admin
                .capture_selection(
                    crate::provider_admin::ProviderSelectionSource::Role(role),
                    &keys,
                )
                .await
        }));
    }
    let observer = rig.pool.get().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {
            let waiting: i64 = observer.query_one("SELECT count(*) FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE '%SELECT scope, ciphertext, key_version FROM brassclaw_secrets%'", &[]).await.unwrap().get(0);
            if waiting>=4 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    let mut replacement = saved.definition.clone();
    replacement.default_base_url = Some(url.replace("/v1", "/new/v1"));
    repo.replace_revision_in_transaction(&tx, replacement, Some(saved.revision))
        .await
        .unwrap();
    keys.put_in_transaction(&tx, "openai", secrecy::SecretString::from("native-new-key"))
        .await
        .unwrap();
    crate::db_config::save_config_changes_in_transaction(
        &tx,
        tenant,
        &[
            ("llm.default.model", Some("generation-one")),
            ("llm.sempai.model", Some("generation-one")),
            ("embedding.model", Some("generation-one")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let requests_before = state.requests.lock().await.len();
    tx.commit().await.unwrap();
    assert!(
        matches!(
            pending.await.unwrap(),
            Err(brassclaw_product_workflow::LlmConfigServiceError::Unavailable)
        ),
        "a concurrent endpoint/key change invalidates the old snapshot before I/O"
    );
    assert_eq!(state.requests.lock().await.len(), requests_before);
    for pending in pending_roles {
        assert!(
            pending.await.unwrap().is_err(),
            "a mixed role/definition/credential snapshot must fail before provider construction"
        );
    }
    let new_url = url.replace("/v1", "/new/v1");
    for role in roles {
        let captured = admin
            .capture_selection(
                crate::provider_admin::ProviderSelectionSource::Role(role),
                &keys,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(captured.selection.model.as_deref(), Some("generation-one"));
        assert_eq!(
            captured.definition.default_base_url.as_deref(),
            Some(new_url.as_str())
        );
        assert_eq!(
            secrecy::ExposeSecret::expose_secret(captured.credential.as_ref().unwrap()),
            "native-new-key"
        );
    }
    assert!(
        service
            .list_models(
                caller.clone(),
                request("open_ai_completions", &new_url, None, None)
            )
            .await
            .unwrap()
            .ok
    );
    assert_eq!(
        state.requests.lock().await.last().unwrap().1.as_deref(),
        Some("Bearer native-new-key")
    );
    assert!(
        service
            .list_models(
                caller.clone(),
                request("open_ai_completions", &url, None, None)
            )
            .await
            .unwrap()
            .ok
    );
    assert_eq!(
        state.requests.lock().await.last().unwrap().1,
        None,
        "an old endpoint cannot receive the successor credential"
    );
    assert_eq!(
        observer
            .query_one(
                "SELECT count(*) FROM brassclaw_secrets WHERE scope LIKE 'lease-v2:%'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    stop.cancel();
    server.await.unwrap();
}
