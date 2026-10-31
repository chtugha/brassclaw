//! Actual PostgreSQL, encrypted key store and Chat Completions transport.
//! The controlled HTTP peer verifies transport/selection, not model quality.
use super::*;
use serde_json::{Value, json};

pub(crate) fn keys(pool: &PgPool, tenant: &str) -> LlmKeyStore {
    LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            pool.clone(),
            "task-model-provider-authenticated-master-fixture".into(),
            tenant,
        )
        .unwrap(),
    ))
}

pub(crate) async fn provider(
    pool: &PgPool,
    tenant: &str,
    id: &str,
    endpoint: &str,
    expected: Option<u64>,
) {
    let definition = serde_json::from_value(json!({
        "id":id,"protocol":"open_ai_completions","default_base_url":endpoint,
        "model_env":"BRASSCLAW_TASK_SELECTION_TEST_MODEL","default_model":"default-model",
        "description":"Task selection transport fixture", "api_key_required":false,
        "request_policy":brassclaw_llm::ProviderRequestPolicy { max_retries:0, ..Default::default() }
    }))
    .unwrap();
    crate::PgProviderRepo::new(pool.clone(), tenant)
        .replace_revision(definition, expected)
        .await
        .unwrap();
}

pub(crate) async fn select(pool: &PgPool, tenant: &str, role: &str, provider: &str, model: &str) {
    let slot = if role == "kohai" { "default" } else { role };
    let prefix = format!("llm.{slot}");
    crate::db_config::save_config_changes(
        pool,
        tenant,
        &[
            (format!("{prefix}.provider_id").as_str(), Some(provider)),
            (format!("{prefix}.model").as_str(), Some(model)),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
}

async fn complete(
    provider: &RetainedLlmProvider,
    text: &str,
) -> Result<brassclaw_llm::CompletionResponse, brassclaw_llm::LlmError> {
    let mut request =
        brassclaw_llm::CompletionRequest::new(vec![brassclaw_llm::ChatMessage::user(text)]);
    request.model = Some(provider.model.clone());
    request.dispatch_policy = brassclaw_llm::ProviderDispatchPolicy::SingleSubmission;
    provider.provider.complete(request).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_task_selection_retains_original_and_child_before_first_model_after_role_edit() {
    use axum::{
        Json,
        extract::{Path, State},
        routing::post,
    };
    type Calls = Arc<tokio::sync::Mutex<Vec<(String, String)>>>;
    async fn receive(
        Path(target): Path<String>,
        State(calls): State<Calls>,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        let model = body["model"].as_str().unwrap().to_owned();
        calls.lock().await.push((target, model.clone()));
        Json(
            json!({"id":"task-selection-response","object":"chat.completion","created":1,
            "model":model,"choices":[{"index":0,"message":{"role":"assistant","content":"selected transport response"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":8,"completion_tokens":3,"total_tokens":11}}),
        )
    }
    let calls: Calls = Default::default();
    let router = axum::Router::new()
        .route("/{target}/v1/chat/completions", post(receive))
        .with_state(calls.clone());
    let (address, stop, server) = crate::test_loopback_http::spawn(router).await;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let pool = rig.pool.as_ref();
    let tenant = "task-selection-original";
    let keys = keys(pool, tenant);
    provider(
        pool,
        tenant,
        "original",
        &format!("http://{address}/original/v1"),
        None,
    )
    .await;
    provider(
        pool,
        tenant,
        "replacement",
        &format!("http://{address}/replacement/v1"),
        None,
    )
    .await;
    keys.put("original", "original-task-credential-value".into())
        .await
        .unwrap();
    select(pool, tenant, "kohai", "original", "original-kohai").await;
    select(pool, tenant, "sempai", "original", "original-sempai").await;
    let sources = TaskProviderConfiguration::new(pool.clone(), tenant, keys.clone())
        .with_session(Arc::new(SessionManager::new(Default::default())));
    let primary = sources.role(ProviderRole::Kohai);
    let sempai = sources.role(ProviderRole::Sempai);
    let original = primary.capture(None, None).await.unwrap();
    let metadata = original.retained_metadata().unwrap();
    assert!(
        !metadata
            .to_string()
            .contains("original-task-credential-value")
    );
    assert_eq!(original.model_route().unwrap().model_id, "original-kohai");
    assert_eq!(
        original.role_model_route("sempai").unwrap().model_id,
        "original-sempai"
    );
    // This is a real external DB writer, before the original's first SDK build.
    select(pool, tenant, "kohai", "replacement", "new-kohai").await;
    select(pool, tenant, "sempai", "replacement", "new-sempai").await;
    pool.get().await.unwrap().execute(
        "UPDATE brassclaw_llm_providers SET definition=jsonb_set(definition,'{description}',to_jsonb('compatible successor definition'::text)) WHERE tenant_id=$1 AND id='original'",
        &[&tenant],
    ).await.unwrap();
    let child = primary.capture(Some(original.clone()), None).await.unwrap();
    assert!(Arc::ptr_eq(&child, &original));
    assert_eq!(child.retained_metadata().unwrap(), metadata);
    assert!(
        calls.lock().await.is_empty(),
        "capture has no inference effects"
    );
    let old_primary = primary.prepare(&original).await.unwrap();
    let old_sempai = sempai.prepare(&child).await.unwrap();
    assert_eq!(old_primary.provider_id, "original");
    let brassclaw_llm::ProviderPublication::Configured {
        selection: revision,
    } = &old_primary.publication
    else {
        panic!("retained publication required")
    };
    assert_eq!(
        revision.definition_revision, 1,
        "read original immutable history, not latest compatible definition"
    );
    assert_eq!(old_primary.model, "original-kohai");
    assert_eq!(old_sempai.model, "original-sempai");
    complete(&old_primary, "original primary operation")
        .await
        .unwrap();
    complete(&old_sempai, "original sempai operation")
        .await
        .unwrap();
    let unavailable: Arc<dyn LoopTaskModelSelection> =
        Arc::new(brassclaw_turns::run_profile::InheritedUnavailableModelSelection);
    let inherited_absence = primary
        .capture(Some(unavailable.clone()), None)
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&inherited_absence, &unavailable));
    assert!(!inherited_absence.role_configured("kohai"));
    assert!(inherited_absence.model_route().is_err());
    assert!(
        primary.prepare(&inherited_absence).await.is_err(),
        "inherited absence never adopts the now-configured role"
    );
    let fresh = primary.capture(None, None).await.unwrap();
    let new_primary = primary.prepare(&fresh).await.unwrap();
    complete(&new_primary, "new task operation").await.unwrap();
    assert_eq!(
        *calls.lock().await,
        [
            ("original".into(), "original-kohai".into()),
            ("original".into(), "original-sempai".into()),
            ("replacement".into(), "new-kohai".into())
        ]
    );
    // Current credential rotation fences even the already built original client.
    keys.put("original", "rotated-task-credential-value".into())
        .await
        .unwrap();
    assert!(
        complete(&old_primary, "rotation must reject before HTTP")
            .await
            .is_err()
    );
    assert_eq!(calls.lock().await.len(), 3);
    let other_owner = TaskProviderConfiguration::new(pool.clone(), tenant, keys)
        .with_session(Arc::new(SessionManager::new(Default::default())));
    assert!(
        other_owner
            .role(ProviderRole::Kohai)
            .capture(Some(original), None)
            .await
            .is_err()
    );
    stop.cancel();
    server.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_task_selection_keeps_absence_invalid_optional_roles_and_preparation_fences() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let pool = rig.pool.as_ref();
    let tenant = "task-selection-absence";
    let keys = keys(pool, tenant);
    let sources = TaskProviderConfiguration::new(pool.clone(), tenant, keys.clone())
        .with_session(Arc::new(SessionManager::new(Default::default())));
    let primary = sources.role(ProviderRole::Kohai);
    let absent = primary.capture(None, None).await.unwrap();
    assert!(!absent.role_configured("kohai"));
    assert!(!absent.role_configured("sempai"));
    provider(pool, tenant, "available", "http://127.0.0.1:1/v1", None).await;
    select(pool, tenant, "kohai", "available", "chosen-model").await;
    select(
        pool,
        tenant,
        "sempai",
        "missing-provider",
        "invalid-optional-model",
    )
    .await;
    assert!(
        primary.prepare(&absent).await.is_err(),
        "never repair original absence from latest"
    );
    let current = primary.capture(None, None).await.unwrap();
    assert_eq!(current.model_route().unwrap().model_id, "chosen-model");
    assert!(
        current.role_configured("sempai"),
        "invalid is not an instruction to skip Sempai"
    );
    assert!(
        sources
            .role(ProviderRole::Sempai)
            .prepare(&current)
            .await
            .is_err()
    );
    keys.put("available", "first-selected-task-credential".into())
        .await
        .unwrap();
    let selected = primary.capture(None, None).await.unwrap();
    keys.put("available", "replacement-task-credential".into())
        .await
        .unwrap();
    assert!(
        primary.prepare(&selected).await.is_err(),
        "rotation before initial preparation cannot select a new key"
    );
    let deactivated = primary.capture(None, None).await.unwrap();
    crate::PgProviderRepo::new(pool.clone(), tenant)
        .delete("available")
        .await
        .unwrap();
    assert!(primary.prepare(&deactivated).await.is_err());
    provider(pool, tenant, "healthy", "http://127.0.0.1:1/v1", None).await;
    select(pool, tenant, "kohai", "healthy", "healthy-model").await;
    pool.get().await.unwrap().execute(
        "UPDATE brassclaw_llm_providers SET definition=definition-'protocol' WHERE tenant_id=$1 AND id='available' AND deleted_at IS NOT NULL",
        &[&tenant]).await.unwrap();
    let healthy = primary.capture(None, None).await.unwrap();
    assert_eq!(
        healthy.model_route().unwrap().model_id,
        "healthy-model",
        "inactive corrupt rows cannot invalidate the unrelated active task selection"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_task_model_selection_is_persisted_with_original_admission_and_cannot_be_rebound() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let (admitted, _) = crate::native_admission_fixture::reserve_unstarted_with_claim(
        rig.pool.clone(),
        "provider-free selection identity",
    )
    .await;
    let tenant = admitted.context.scope.tenant_id.as_str();
    let sources = TaskProviderConfiguration::new(
        rig.pool.as_ref().clone(),
        tenant,
        keys(rig.pool.as_ref(), tenant),
    )
    .with_session(Arc::new(SessionManager::new(Default::default())));
    let selection = sources
        .role(ProviderRole::Kohai)
        .capture(None, None)
        .await
        .unwrap();
    let metadata = selection.retained_metadata().unwrap();
    let mut corrupt = metadata.clone();
    corrupt["selection"]["namespace"] = json!("not-original");
    assert!(admitted.admission.bind_task_models(Some(corrupt)).is_err());
    admitted
        .admission
        .bind_task_models(Some(metadata.clone()))
        .unwrap();
    admitted.admission.check_and_start().await.unwrap();
    let client = rig.pool.get().await.unwrap();
    let id = admitted.context.run_id.as_uuid();
    let stored: Value = client
        .query_one(
            "SELECT task_model_selection FROM brassclaw_monty_task_admissions WHERE run_id=$1",
            &[&id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, metadata);
    assert!(admitted.admission.bind_task_models(None).is_err());
    assert!(client.execute("UPDATE brassclaw_monty_task_admissions SET task_model_selection=NULL WHERE run_id=$1",&[&id]).await.is_err());
    assert!(client.execute("UPDATE brassclaw_monty_task_admissions SET task_model_selection_version=0 WHERE run_id=$1",&[&id]).await.is_err());
    admitted.admission.check_and_start().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_task_embedding_family_retains_client_space_and_credentials_and_releases_cache() {
    use axum::{
        Json,
        extract::{Path, State},
        http::HeaderMap,
        routing::post,
    };
    type Calls = Arc<tokio::sync::Mutex<Vec<(String, String, String)>>>;
    async fn embed(
        Path(target): Path<String>,
        State(calls): State<Calls>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        calls.lock().await.push((
            target,
            body["model"].as_str().unwrap().to_owned(),
            headers
                .get("authorization")
                .map(|value| value.to_str().unwrap())
                .unwrap_or("")
                .to_owned(),
        ));
        Json(
            json!({"data":body["input"].as_array().unwrap().iter().enumerate()
            .map(|(index,_)|json!({"index":index,"embedding":[1.0,0.0,0.0]})).collect::<Vec<_>>()}),
        )
    }
    let calls: Calls = Default::default();
    let (address, stop, server) = crate::test_loopback_http::spawn(
        axum::Router::new()
            .route("/{target}/v1/embeddings", post(embed))
            .with_state(calls.clone()),
    )
    .await;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let pool = rig.pool.as_ref();
    let tenant = "task-embedding-family";
    let keys = keys(pool, tenant);
    let repo = crate::PgProviderRepo::new(pool.clone(), tenant);
    for id in ["original", "replacement"] {
        repo.replace_revision(serde_json::from_value(json!({"id":id,"protocol":"open_ai_completions",
            "default_base_url":format!("http://{address}/{id}/v1"),"model_env":"BRASSCLAW_TASK_EMBEDDING_TEST_MODEL",
            "default_model":"space-a","description":"Task embedding family transport fixture","api_key_required":false,
            "request_policy":brassclaw_llm::ProviderRequestPolicy {max_retries:0,..Default::default()},
            "embedding_models":{"space-a":{"dimension":3,"cache_max_entries":8},"space-b":{"dimension":3,"cache_max_entries":8}}
        })).unwrap(),None).await.unwrap();
    }
    keys.put("original", "original-task-embedding-credential".into())
        .await
        .unwrap();
    async fn select_embedding(
        pool: &PgPool,
        tenant: &str,
        provider: Option<&str>,
        model: Option<&str>,
    ) {
        crate::db_config::save_config_changes(
            pool,
            tenant,
            &[
                ("embedding.provider_id", provider),
                ("embedding.model", model),
            ],
            crate::db_config::ConfigWriteContext::Operator,
        )
        .await
        .unwrap();
    }
    select_embedding(pool, tenant, Some("original"), Some("space-a")).await;
    let sources = TaskProviderConfiguration::new(pool.clone(), tenant, keys.clone())
        .with_session(Arc::new(SessionManager::new(Default::default())));
    let source = sources.role(ProviderRole::Kohai);
    let selected = source.capture(None, None).await.unwrap();
    let child = source.capture(Some(selected.clone()), None).await.unwrap();
    assert!(Arc::ptr_eq(&selected, &child));
    select_embedding(pool, tenant, Some("replacement"), Some("space-b")).await;
    let original = prepare_embedding(selected.as_any().downcast_ref::<TaskSelection>().unwrap())
        .await
        .unwrap()
        .unwrap();
    let inherited = prepare_embedding(child.as_any().downcast_ref::<TaskSelection>().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&original, &inherited));
    assert_eq!(original.model_name(), "space-a");
    assert_eq!(
        original.embed("same cache input").await.unwrap(),
        vec![1.0, 0.0, 0.0]
    );
    inherited.embed("same cache input").await.unwrap();
    assert_eq!(
        calls.lock().await.as_slice(),
        &[(
            "original".into(),
            "space-a".into(),
            "Bearer original-task-embedding-credential".into()
        )]
    );
    let next = source.capture(None, None).await.unwrap();
    let replacement = prepare_embedding(next.as_any().downcast_ref::<TaskSelection>().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replacement.model_name(), "space-b");
    assert_ne!(original.space_id(), replacement.space_id());
    replacement.embed("new task input").await.unwrap();
    assert_eq!(calls.lock().await[1].0, "replacement");
    assert_eq!(calls.lock().await[1].1, "space-b");
    // A later live credential change fences an already prepared provider even
    // for the exact text that previously hit its cache; it must send no request.
    keys.put("original", "changed-task-embedding-credential".into())
        .await
        .unwrap();
    assert!(original.embed("same cache input").await.is_err());
    assert_eq!(calls.lock().await.len(), 2);
    let private_cache = selected
        .as_any()
        .downcast_ref::<TaskSelection>()
        .unwrap()
        .prepared
        .lock()
        .await
        .embedding
        .as_ref()
        .unwrap()
        .private_cache
        .clone()
        .unwrap();
    assert_eq!(private_cache.len(), 1);
    drop(selected);
    assert_eq!(
        private_cache.len(),
        1,
        "a live child retains the original family cache"
    );
    drop(child);
    assert_eq!(
        private_cache.len(),
        0,
        "last task-family selection releases private cache even if an invocation handle remains"
    );
    assert_eq!(private_cache.max_entries(), 0);
    select_embedding(pool, tenant, None, None).await;
    let absent = source.capture(None, None).await.unwrap();
    select_embedding(pool, tenant, Some("original"), Some("undeclared-space")).await;
    let invalid = source.capture(None, None).await.unwrap();
    select_embedding(pool, tenant, Some("replacement"), Some("space-b")).await;
    assert!(
        prepare_embedding(absent.as_any().downcast_ref::<TaskSelection>().unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        prepare_embedding(invalid.as_any().downcast_ref::<TaskSelection>().unwrap())
            .await
            .is_err()
    );
    assert_eq!(
        calls.lock().await.len(),
        2,
        "absent/invalid task roles do not acquire the repaired latest role"
    );
    // Exact globally published SDKs keep their source-owned cache when a task
    // finishes; task-family release must retire only private clients.
    use brassclaw_memory::EmbeddingProviderSource as _;
    let published_source = Arc::new(
        crate::memory_embedding_source::PgMemoryEmbeddingSource::new(
            pool.clone(),
            tenant.into(),
            keys.clone(),
            Arc::new(SessionManager::new(Default::default())),
        ),
    );
    let slot = Arc::new(brassclaw_memory::EmbeddingProviderSourceSlot::default());
    slot.attach(published_source).unwrap();
    let published = slot.snapshot().await.unwrap().unwrap();
    let shared_sources = TaskProviderConfiguration::new(pool.clone(), tenant, keys)
        .with_memory_source(Some(slot))
        .with_session(Arc::new(SessionManager::new(Default::default())));
    let selected_shared = shared_sources
        .role(ProviderRole::Kohai)
        .capture(None, None)
        .await
        .unwrap();
    let shared = prepare_embedding(
        selected_shared
            .as_any()
            .downcast_ref::<TaskSelection>()
            .unwrap(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        Arc::ptr_eq(&published, &shared),
        "exact published SDK/cache is reused"
    );
    shared
        .embed("shared source cache survives task release")
        .await
        .unwrap();
    let shared_calls = calls.lock().await.len();
    assert_eq!(shared_calls, 3);
    drop(selected_shared);
    published
        .embed("shared source cache survives task release")
        .await
        .unwrap();
    assert_eq!(
        calls.lock().await.len(),
        shared_calls,
        "task release cannot retire a source-owned published cache"
    );
    stop.cancel();
    server.await.unwrap();
}
