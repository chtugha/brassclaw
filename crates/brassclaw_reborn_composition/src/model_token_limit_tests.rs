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
    let request = |revision, map| json!({"mutation_id":uuid::Uuid::new_v4(),"id":"native-ornith-limits","expected_revision":revision,"adapter":"open_ai_completions","model_token_limits":map});
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
    let renamed=service.upsert_provider(caller.clone(),serde_json::from_value(json!({"mutation_id":uuid::Uuid::new_v4(),"id":"native-ornith-limits","expected_revision":revision,"adapter":"open_ai_completions","name":"Renamed, same model limits"})).unwrap()).await.unwrap();
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
        r#"{"mutation_id":"579bf426-a7dc-44ca-9ae9-cdc51f381a4c","id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":true,"max_prompt_tokens":1}}}"#,
        r#"{"mutation_id":"579bf426-a7dc-44ca-9ae9-cdc51f381a4c","id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":1}}}"#,
        r#"{"mutation_id":"579bf426-a7dc-44ca-9ae9-cdc51f381a4c","id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":1,"max_prompt_tokens":0}}}"#,
        r#"{"mutation_id":"579bf426-a7dc-44ca-9ae9-cdc51f381a4c","id":"p","adapter":"ollama","model_token_limits":{"m":{"max_prefix_tokens":1,"max_prompt_tokens":1},"m":{"max_prefix_tokens":2,"max_prompt_tokens":2}}}"#,
    ] {
        assert!(
            serde_json::from_str::<brassclaw_product_workflow::UpsertLlmProviderRequest>(invalid)
                .is_err()
        );
    }
}

#[tokio::test]
async fn dispatch_policy_reads_exact_model_limits_from_one_revision_without_context_fallback() {
    use crate::provider_admin::ProviderSelectionSource;
    use brassclaw_llm::{
        ModelRequestTokenCounts, ModelTokenCheckError, ProviderDispatchGuard, ProviderRequestFacts,
        ProviderRole,
    };
    let rig = crate::runtime::test_pg::native_pg::NativePostgres::start().await;
    let owner = "dispatch-policy-owner";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), owner);
    repo.initialize().await.unwrap();
    let define = |id: &str, prefix: u64, prompt: u64| {
        serde_json::from_value(json!({
        "id":id,"protocol":"open_ai_completions","default_base_url":"http://127.0.0.1:1/v1",
        "base_url_required":true,"api_key_required":false,"model_env":"UNUSED_DISPATCH_POLICY_MODEL",
        "default_model":"same-model","context_window_tokens":131072,"description":"Canonical dispatch policy fixture",
        "model_token_limits":{"same-model":{"max_prefix_tokens":prefix,"max_prompt_tokens":prompt},
            "other-model":{"max_prefix_tokens":2000,"max_prompt_tokens":1000}}})).unwrap()
    };
    repo.replace_revision(define("policy-a", 100000, 14688), None)
        .await
        .unwrap();
    repo.replace_revision(define("policy-b", 80000, 20000), None)
        .await
        .unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        owner,
        &[
            ("llm.default.provider_id", Some("policy-a")),
            ("llm.sempai.provider_id", Some("policy-b")),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("native-dispatch-policy-test-master-key"),
            owner,
        )
        .unwrap(),
    ));
    let admin = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), owner);
    let kohai = admin
        .capture_selection(ProviderSelectionSource::Role(ProviderRole::Kohai), &keys)
        .await
        .unwrap()
        .unwrap();
    let sempai = admin
        .capture_selection(ProviderSelectionSource::Role(ProviderRole::Sempai), &keys)
        .await
        .unwrap()
        .unwrap();
    let guard = crate::provider_dispatch::PgProviderDispatchGuard::from_capture(
        rig.pool.as_ref().clone(),
        owner,
        keys.clone(),
        &kohai,
    )
    .unwrap();
    let sempai_guard = crate::provider_dispatch::PgProviderDispatchGuard::from_capture(
        rig.pool.as_ref().clone(),
        owner,
        keys.clone(),
        &sempai,
    )
    .unwrap();
    let facts = ProviderRequestFacts {
        model: "same-model",
        max_output_tokens: Some(16384),
    };
    let state = guard.check(facts).await.unwrap();
    let reviewer = sempai_guard.check(facts).await.unwrap();
    assert_eq!(
        state.definition_revision,
        kohai.revision.definition_revision
    );
    assert_eq!(
        reviewer.definition_revision,
        sempai.revision.definition_revision
    );
    assert_eq!(
        state.model_token_limits,
        Some(kohai.definition.model_token_limits.0["same-model"])
    );
    assert_eq!(
        reviewer.model_token_limits,
        Some(sempai.definition.model_token_limits.0["same-model"])
    );
    assert_eq!(state.configured_context_window_tokens, Some(131072));
    assert_eq!(state.requested_output_tokens, Some(16384));
    let counts = ModelRequestTokenCounts {
        prefix_tokens: 100000,
        prompt_tokens: 14688,
        input_tokens: 114688,
    };
    assert_eq!(
        state.check_configured_token_counts("policy-a", "same-model", counts),
        Ok(())
    );
    assert_eq!(
        reviewer.check_configured_token_counts("policy-b", "same-model", counts),
        Err(ModelTokenCheckError::PrefixExceeded)
    );
    assert_eq!(
        state.check_configured_token_counts("policy-b", "same-model", counts),
        Err(ModelTokenCheckError::TargetMismatch)
    );
    let unknown_context = guard
        .check(ProviderRequestFacts {
            model: "other-model",
            max_output_tokens: Some(1),
        })
        .await
        .unwrap();
    assert_eq!(
        unknown_context.model_token_limits,
        Some(kohai.definition.model_token_limits.0["other-model"])
    );
    assert_eq!(unknown_context.configured_context_window_tokens, None);
    assert_eq!(
        unknown_context.check_configured_token_counts(
            "policy-a",
            "other-model",
            ModelRequestTokenCounts {
                prefix_tokens: 1,
                prompt_tokens: 1,
                input_tokens: 2
            }
        ),
        Err(ModelTokenCheckError::UnknownContext)
    );
    let missing = guard
        .check(ProviderRequestFacts {
            model: "unconfigured-model",
            max_output_tokens: Some(1),
        })
        .await
        .unwrap();
    assert_eq!(missing.model_token_limits, None);
    assert_eq!(
        missing.check_configured_token_counts("policy-a", "unconfigured-model", counts),
        Err(ModelTokenCheckError::MissingInputLimits)
    );
    let latest = repo.get_revision("policy-a", false).await.unwrap().unwrap();
    let mut looser = latest.definition;
    looser
        .model_token_limits
        .0
        .get_mut("same-model")
        .unwrap()
        .max_prefix_tokens = 100001;
    let looser = repo
        .replace_revision(looser, Some(latest.revision))
        .await
        .unwrap();
    let live = guard.check(facts).await.unwrap();
    assert_eq!(live.definition_revision, looser.revision);
    assert_eq!(live.model_token_limits.unwrap().max_prefix_tokens, 100001);
    assert_eq!(
        live.check_configured_token_counts(
            "policy-a",
            "same-model",
            ModelRequestTokenCounts {
                prefix_tokens: 100001,
                prompt_tokens: 14687,
                input_tokens: 114688
            }
        ),
        Ok(())
    );
    crate::db_config::save_config_changes(
        &rig.pool,
        owner,
        &[("llm.default.provider_id", Some("policy-b"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let old_target = guard.check(facts).await.unwrap();
    assert_eq!(old_target.provider_id, "policy-a");
    let new_role = admin
        .capture_selection(ProviderSelectionSource::Role(ProviderRole::Kohai), &keys)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new_role.definition.id, "policy-b");
    crate::db_config::save_config_changes(
        &rig.pool,
        owner,
        &[("llm.default.provider_id", Some("policy-a"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let latest = repo.get_revision("policy-a", false).await.unwrap().unwrap();
    let mut metadata = latest.definition;
    metadata.description = "Metadata successor preserves canonical ceilings".into();
    let renamed = repo
        .replace_revision(metadata, Some(latest.revision))
        .await
        .unwrap();
    let observed = guard.check(facts).await.unwrap();
    assert_eq!(observed.definition_revision, renamed.revision);
    assert_eq!(observed.model_token_limits, live.model_token_limits);
    assert_eq!(
        state.definition_revision, kohai.revision.definition_revision,
        "old observations are immutable, not live grants"
    );
    let mut tightened = renamed.definition;
    tightened
        .model_token_limits
        .0
        .get_mut("same-model")
        .unwrap()
        .max_prefix_tokens = 99999;
    let tightened = repo
        .replace_revision(tightened, Some(renamed.revision))
        .await
        .unwrap();
    assert!(
        matches!(
            guard.check(facts).await,
            Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
        ),
        "keep the existing fail-closed tightening fence"
    );
    let capture = admin
        .capture_selection(ProviderSelectionSource::Role(ProviderRole::Kohai), &keys)
        .await
        .unwrap()
        .unwrap();
    let fresh = crate::provider_dispatch::PgProviderDispatchGuard::from_capture(
        rig.pool.as_ref().clone(),
        owner,
        keys,
        &capture,
    )
    .unwrap();
    let current = fresh.check(facts).await.unwrap();
    assert_eq!(current.definition_revision, tightened.revision);
    assert_eq!(
        current.check_configured_token_counts("policy-a", "same-model", counts),
        Err(ModelTokenCheckError::PrefixExceeded)
    );
    // Removing configured bounds is not permission to dispatch old work
    // without them. Isolate removal from the preceding tightening case.
    let mut removed = kohai.definition.clone();
    removed.model_token_limits.0.remove("same-model");
    let removed = repo
        .replace_revision(removed, Some(tightened.revision))
        .await
        .unwrap();
    assert!(matches!(
        guard.check(facts).await,
        Err(brassclaw_llm::LlmError::ConfigurationChanged { .. })
    ));
    let restored = repo
        .replace_revision(kohai.definition.clone(), Some(removed.revision))
        .await
        .unwrap();
    let checked = guard.check(facts).await.unwrap();
    assert_eq!(checked.definition_revision, restored.revision);
    assert_eq!(checked.model_token_limits, state.model_token_limits);

    // The final wire guard consumes the SAME native canonical owner. These
    // fixture counts qualify policy/transport custody, not model tokenization.
    use brassclaw_llm::final_request_guard::{
        FinalRequestError, FinalRequestGuard, FinalRequestInput, FinalRequestMeasurement,
        FinalRequestMeasurer, FinalRequestSendError, FinalRequestTransport,
    };
    struct DeclaredMeasurement(ModelRequestTokenCounts);
    #[async_trait::async_trait]
    impl FinalRequestMeasurer for DeclaredMeasurement {
        async fn measure(
            &self,
            input: FinalRequestInput<'_>,
        ) -> Result<FinalRequestMeasurement, FinalRequestError> {
            use sha2::Digest;
            let body: serde_json::Value =
                serde_json::from_slice(input.body).map_err(|_| FinalRequestError::Measurement)?;
            if body["model"] != input.model {
                return Err(FinalRequestError::Binding);
            }
            let output = body["max_completion_tokens"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or(FinalRequestError::Measurement)?;
            Ok(FinalRequestMeasurement {
                body_sha256: sha2::Sha256::digest(input.body).into(),
                provider_id: input.provider_id.into(),
                model: input.model.into(),
                reserved_output_tokens: output,
                counts: self.0,
            })
        }
    }
    let endpoint: reqwest::Url = "http://127.0.0.1:1/v1/chat/completions".parse().unwrap();
    let final_guard = FinalRequestGuard::new(
        "policy-a".into(),
        "same-model".into(),
        endpoint.clone(),
        guard.clone(),
        Arc::new(DeclaredMeasurement(counts)),
    )
    .unwrap();
    let request = reqwest::Client::new()
        .post(endpoint)
        .json(&json!({
        "model":"same-model","messages":[],"max_completion_tokens":16384}))
        .build()
        .unwrap();
    let retained_request = final_guard.prepare(request).await.unwrap();
    let mut absent = kohai.definition.clone();
    absent.model_token_limits.0.remove("same-model");
    let absent = repo
        .replace_revision(absent, Some(restored.revision))
        .await
        .unwrap();
    let transport = FinalRequestTransport::new(reqwest::Client::builder().no_proxy()).unwrap();
    assert!(
        matches!(
            retained_request.send(&transport).await,
            Err(FinalRequestSendError::Rejected(FinalRequestError::Policy))
        ),
        "removing native caps after preparation must reject before any HTTP send"
    );
    let original = repo
        .replace_revision(kohai.definition.clone(), Some(absent.revision))
        .await
        .unwrap();
    assert_eq!(
        guard.check(facts).await.unwrap().definition_revision,
        original.revision
    );
}
