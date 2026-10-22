//! Actual provider service/CAS/PostgreSQL path. No model inference or new store.
use brassclaw_product_workflow::{
    LlmConfigService, LlmConfigServiceError, WebUiAuthenticatedCaller,
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn model_input_limits_use_one_provider_revision_and_preserve_unrelated_configuration() {
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let owner = "model-limit-operator";
    let repo = Arc::new(crate::PgProviderRepo::new(rig.pool.as_ref().clone(), owner));
    repo.initialize().await.unwrap();
    let model = "cyankiwi/Ornith-1.5-9B-AWQ-INT4";
    let definition=serde_json::from_value(json!({"id":"native-ornith-limits","aliases":[],"protocol":"open_ai_completions",
        "default_base_url":"http://127.0.0.1:8000/v1","base_url_required":true,"api_key_required":false,
        "model_env":"UNUSED_MODEL_LIMIT_FIXTURE","default_model":model,"description":"Native limits fixture","context_window_tokens":131072})).unwrap();
    let initial = repo.replace_revision(definition, None).await.unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("native-model-limit-test-master-key"),
            owner,
        )
        .unwrap(),
    ));
    let primary = Arc::new(brassclaw_llm::SwappableLlmProvider::new(Arc::new(
        crate::runtime::PlaceholderLlmProvider,
    )));
    let reload = Arc::new(crate::llm_reload::RebornLlmReloadAdapter::new(
        Arc::new(brassclaw_llm::LlmReloadHandle::new(primary, None)),
        Arc::new(brassclaw_llm::SessionManager::new(Default::default())),
        keys.clone(),
        rig.pool.clone(),
        owner,
    ));
    let service = crate::RebornLlmConfigService::new(keys, rig.pool.clone(), repo.clone(), owner)
        .with_reload_trigger(reload);
    let caller = WebUiAuthenticatedCaller::new(
        brassclaw_host_api::TenantId::new(owner).unwrap(),
        brassclaw_host_api::UserId::new("operator").unwrap(),
        None,
        None,
    );
    let limits = json!({model:{"max_prefix_tokens":100000,"max_prompt_tokens":14688}});
    let request = |revision, map| json!({"id":"native-ornith-limits","expected_revision":revision,"adapter":"open_ai_completions","model_token_limits":map});
    let saved = service
        .upsert_provider(
            caller.clone(),
            serde_json::from_value(request(initial.revision, limits.clone())).unwrap(),
        )
        .await
        .unwrap();
    let view = saved
        .providers
        .iter()
        .find(|p| p.id == "native-ornith-limits")
        .unwrap();
    assert_eq!(
        serde_json::to_value(&view.model_token_limits).unwrap(),
        limits
    );
    let revision = view.revision;
    let retained = repo
        .get_revision("native-ornith-limits", false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.definition.context_window_tokens, Some(131072));
    assert_eq!(
        retained.definition.model_token_limits.0[model].total_input_tokens(),
        Some(114688)
    );
    assert!(matches!(
        service
            .upsert_provider(
                caller.clone(),
                serde_json::from_value(request(initial.revision, json!({}))).unwrap()
            )
            .await,
        Err(LlmConfigServiceError::Conflict { .. })
    ));
    let renamed=service.upsert_provider(caller.clone(),serde_json::from_value(json!({"id":"native-ornith-limits","expected_revision":revision,"adapter":"open_ai_completions","name":"Renamed, same model limits"})).unwrap()).await.unwrap();
    let view = renamed
        .providers
        .iter()
        .find(|p| p.id == "native-ornith-limits")
        .unwrap();
    assert_eq!(
        serde_json::to_value(&view.model_token_limits).unwrap(),
        limits,
        "omitted map preserves all model entries"
    );
    let revision = view.revision;
    let bad = json!({model:{"max_prefix_tokens":131072,"max_prompt_tokens":1}});
    assert!(matches!(
        service
            .upsert_provider(
                caller.clone(),
                serde_json::from_value(request(revision, bad)).unwrap()
            )
            .await,
        Err(LlmConfigServiceError::InvalidRequest { .. })
    ));
    assert_eq!(
        repo.get_revision("native-ornith-limits", false)
            .await
            .unwrap()
            .unwrap()
            .revision,
        revision
    );
    let fresh = service.snapshot(caller.clone()).await.unwrap();
    assert_eq!(
        serde_json::to_value(
            &fresh
                .providers
                .iter()
                .find(|p| p.id == "native-ornith-limits")
                .unwrap()
                .model_token_limits
        )
        .unwrap(),
        limits
    );
    let cleared = service
        .upsert_provider(
            caller,
            serde_json::from_value(request(revision, json!({}))).unwrap(),
        )
        .await
        .unwrap();
    assert!(
        cleared
            .providers
            .iter()
            .find(|p| p.id == "native-ornith-limits")
            .unwrap()
            .model_token_limits
            .0
            .is_empty()
    );
    assert!(
        serde_json::from_value::<brassclaw_product_workflow::UpsertLlmProviderRequest>(request(
            revision,
            serde_json::Value::Null
        ))
        .is_err()
    );
    for invalid in [
        r#"{"id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":true,"max_prompt_tokens":1}}}"#,
        r#"{"id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":1}}}"#,
        r#"{"id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":1,"max_prompt_tokens":0}}}"#,
        r#"{"id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":1,"max_prompt_tokens":1},"m":{"max_prefix_tokens":2,"max_prompt_tokens":2}}}"#,
    ] {
        assert!(
            serde_json::from_str::<brassclaw_product_workflow::UpsertLlmProviderRequest>(invalid)
                .is_err()
        );
    }
}
