//! Counter integration fixture, NOT acceptance of a real provider/model design.
use super::*;
use brassclaw_host_api::{InvocationId, TenantId, UserId};
use brassclaw_llm::final_request_guard::FinalRequestGuard;
use brassclaw_llm::{ModelTokenCheckError, ProviderRole};
use brassclaw_resources::{InMemoryResourceGovernor, ResourceAccount};
use serde_json::{Value, json};

struct FixtureBinding {
    scope: ResourceScope,
    pair: Uuid,
    profile: Uuid,
    generation: Uuid,
}
#[async_trait]
impl FinalEncodingBindingVerifier for FixtureBinding {
    async fn verify(
        &self,
        s: &FinalEncodingSelection,
        scope: &ResourceScope,
        headers: &reqwest::header::HeaderMap,
        _body: &[u8],
    ) -> Result<(), FinalRequestError> {
        if headers.keys().any(|name| name.as_str() != "content-type")
            || scope != &self.scope
            || s.pair_acceptance != self.pair
            || s.profile_acceptance != self.profile
            || s.generation_qualification != self.generation
        {
            return Err(FinalRequestError::Binding);
        }
        Ok(())
    }
}
// Physical counter fixture only: not a production workflow-qualification owner.
struct WorkflowFixtureBinding {
    scope: ResourceScope,
    workflow: String,
}
#[async_trait]
impl FinalEncodingBindingVerifier for WorkflowFixtureBinding {
    async fn verify(
        &self,
        _: &FinalEncodingSelection,
        _: &ResourceScope,
        _: &reqwest::header::HeaderMap,
        _: &[u8],
    ) -> Result<(), FinalRequestError> {
        Err(FinalRequestError::Binding)
    }
    async fn verify_workflow(
        &self,
        s: &WorkflowFinalEncodingSelection,
        scope: &ResourceScope,
        headers: &reqwest::header::HeaderMap,
        _: &[u8],
    ) -> Result<(), FinalRequestError> {
        if s.workflow_reference != self.workflow
            || scope != &self.scope
            || headers.keys().any(|name| name.as_str() != "content-type")
        {
            return Err(FinalRequestError::Binding);
        }
        Ok(())
    }
}
fn mapping() -> String {
    json!({"format":brassclaw_model_encoding::wire::WIRE_MAPPING,
        "render_defaults_json":r#"{"add_generation_prompt":true,"continue_final_message":false}"#,
        "missing_tools":"null","null_assistant_content":"preserve","tool_arguments":"preserve","text_parts":{"mode":"join","separator":"\n"},
        "forward_tool_choice":true,"forward_response_format":true,
        "allowed_tool_choices":["auto","none","required"],"allowed_chat_kwargs":[],"allowed_message_fields":["role","content"]}).to_string()
}
fn input<'a>(body: &'a [u8], model: &'a str) -> FinalRequestInput<'a> {
    static HEADERS: std::sync::LazyLock<reqwest::header::HeaderMap> =
        std::sync::LazyLock::new(reqwest::header::HeaderMap::new);
    FinalRequestInput {
        body,
        headers: &HEADERS,
        model,
        provider_id: "counter-fixture",
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_counter_uses_retained_assets_original_worker_and_actual_guard_counts() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let owner = "final-counter-fixture";
    let model = "installation-wordlevel-data-fixture";
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), owner);
    repo.initialize().await.unwrap();
    let definition=serde_json::from_value(json!({"id":"counter-fixture","protocol":"open_ai_completions","default_base_url":"http://127.0.0.1:9/v1","base_url_required":true,"api_key_required":false,"model_env":"UNUSED_FINAL_COUNTER_MODEL","default_model":model,"context_window_tokens":4,"description":"Final counter data fixture","model_token_limits":{model:{"max_prefix_tokens":1,"max_prompt_tokens":1}}})).unwrap();
    let mut registered = repo.replace_revision(definition, None).await.unwrap();
    let specification =
        crate::prefix_model_designs::PgPrefixModelDesigns::new(rig.pool.clone(), owner.into())
            .capture(
                brassclaw_product_workflow::CapturePrefixModelRequest {
                    provider_id: "counter-fixture".into(),
                    expected_provider_revision: registered.revision,
                    model: model.into(),
                },
                vec![brassclaw_product_workflow::PrefixModelTarget {
                    provider_id: "counter-fixture".into(),
                    provider_revision: registered.revision,
                    model: model.into(),
                    consumer: "kohai".into(),
                    configured_context_window: Some(4),
                }],
            )
            .await
            .unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        owner,
        &[("llm.default.provider_id", Some("counter-fixture"))],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            secrecy::SecretString::from("final-counter-data-fixture-master-key"),
            owner,
        )
        .unwrap(),
    ));
    let captured = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), owner)
        .capture_selection(
            crate::provider_admin::ProviderSelectionSource::Role(ProviderRole::Kohai),
            &keys,
        )
        .await
        .unwrap()
        .unwrap();
    let policy = crate::provider_dispatch::PgProviderDispatchGuard::from_capture(
        rig.pool.as_ref().clone(),
        owner,
        keys.clone(),
        &captured,
    )
    .unwrap();
    use base64::{Engine, engine::general_purpose::STANDARD};
    const TEMPLATE: &str = "{% for message in messages %}{{ message.content }} {% endfor %}{% if tools %}{{ tools|tojson }}{% endif %}";
    let config = json!({"chat_template":TEMPLATE}).to_string();
    let files: serde_json::Map<String, Value> = [
        ("config.json", "{}".to_owned()),
        (
            "tokenizer.json",
            crate::bootstrap_encoding_fixture::tokenizer(),
        ),
        ("tokenizer_config.json", config.clone()),
    ]
    .into_iter()
    .map(|(name, bytes)| {
        (
            name.into(),
            json!({"encoding":"base64","data":STANDARD.encode(bytes.as_bytes())}),
        )
    })
    .collect();
    let assets = crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(
        rig.pool.clone(),
        owner.into(),
    )
    .retain_with_origin(
        brassclaw_product_workflow::RetainPrefixModelArtifactsRequest {
            specification: specification.id.clone(),
            artifact_set_bytes:
                json!({"format":"brassclaw-prefix-model-artifact-import/1","artifacts":files})
                    .to_string(),
        },
        crate::prefix_model_artifacts::ArtifactImportOrigin::Operator {
            actor: "final-counter-test-data".into(),
        },
    )
    .await
    .unwrap();
    let settings = Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
    settings
        .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
            revision: 1,
            limits: Default::default(),
        })
        .unwrap();
    let worker = Arc::new(crate::source_inspection::ContainedSourceInspection::new());
    worker.bind(settings).await.unwrap();
    let prepared = worker.prepare().await.unwrap();
    let selection = Arc::new(FinalEncodingSelection {
        specification: specification.id.clone(),
        artifact_set: assets.id.clone(),
        pair_acceptance: Uuid::new_v4(),
        profile_acceptance: Uuid::new_v4(),
        generation_qualification: Uuid::new_v4(),
        profile: "base-prompt".into(),
        generation: "a".repeat(64),
        provider_id: "counter-fixture".into(),
        model: model.into(),
        prefix: Arc::from("hello"),
        prefix_token_ids_sha256: checksum(&1_u32.to_le_bytes()),
        mapping_json: Arc::from(mapping()),
        tokenizer_sha256: checksum(crate::bootstrap_encoding_fixture::tokenizer().as_bytes()),
        tokenizer_config_sha256: checksum(config.as_bytes()),
        template_sha256: checksum(TEMPLATE.as_bytes()),
        worker_sha256: hex::encode(prepared.worker.checksum()),
        preparation_revision: prepared.revision,
    });
    let scope = ResourceScope {
        tenant_id: TenantId::new(owner).unwrap(),
        user_id: UserId::new("final-counter-fixture").unwrap(),
        agent_id: None,
        project_id: None,
        thread_id: None,
        invocation_id: InvocationId::new(),
    };
    let backend = Arc::new(crate::model_encoding::ContainedModelEncoding::new(
        rig.pool.clone(),
        worker,
    ));
    let governor = Arc::new(InMemoryResourceGovernor::new());
    let verifier = Arc::new(FixtureBinding {
        scope: scope.clone(),
        pair: selection.pair_acceptance,
        profile: selection.profile_acceptance,
        generation: selection.generation_qualification,
    });
    let counter = Arc::new(
        ContainedFinalRequestMeasurer::new(
            selection.clone(),
            scope.clone(),
            backend.clone(),
            verifier.clone(),
            governor.clone(),
        )
        .unwrap(),
    );
    let body=json!({"model":selection.model,"messages":[{"role":"system","content":"hello"},{"role":"user","content":"world"}],"max_tokens":2}).to_string();
    let counts = counter
        .measure(input(body.as_bytes(), &selection.model))
        .await
        .unwrap();
    assert_eq!(
        counts.counts,
        ModelRequestTokenCounts {
            prefix_tokens: 1,
            prompt_tokens: 1,
            input_tokens: 2
        }
    );
    assert_eq!(
        counts.body_sha256,
        <[u8; 32]>::from(Sha256::digest(body.as_bytes()))
    );
    let account = ResourceAccount::user(scope.tenant_id.clone(), scope.user_id.clone());
    assert_eq!(governor.usage_for(&account).process_count, 1);
    // Exercise the actual transport guard with the REAL contained counter. No
    // test-supplied token counts and no optional task budgets enter this path.
    let endpoint = "http://127.0.0.1:9/v1/chat/completions".parse().unwrap();
    let guard = Arc::new(
        FinalRequestGuard::new(
            "counter-fixture".into(),
            selection.model.clone(),
            endpoint,
            policy.clone(),
            counter.clone(),
        )
        .unwrap(),
    );
    let request = |bytes: String| {
        reqwest::Client::new()
            .post("http://127.0.0.1:9/v1/chat/completions")
            .body(bytes)
            .build()
            .unwrap()
    };
    guard.prepare(request(body.clone())).await.unwrap();
    let oversized=json!({"model":selection.model,"messages":[{"role":"system","content":"hello"},{"role":"user","content":"world world"}],"max_tokens":2}).to_string();
    assert!(matches!(
        guard.prepare(request(oversized)).await,
        Err(FinalRequestError::Tokens(
            ModelTokenCheckError::PromptExceeded
        ))
    ));
    let tools=json!({"model":selection.model,"messages":[{"role":"system","content":"hello"},{"role":"user","content":"world"}],"max_tokens":2,"tools":[{"type":"function","function":{"name":"inspect","description":"world hello","parameters":{"type":"object"}}}]}).to_string();
    let measured_tools = counter
        .measure(input(tools.as_bytes(), &selection.model))
        .await
        .unwrap();
    assert_eq!(measured_tools.counts.prefix_tokens, 1);
    assert!(
        measured_tools.counts.prompt_tokens > 1,
        "actual default template must include dynamic tool definitions"
    );
    assert!(matches!(
        guard.prepare(request(tools)).await,
        Err(FinalRequestError::Tokens(
            ModelTokenCheckError::PromptExceeded
        ))
    ));
    let provider = brassclaw_llm::create_registry_provider(
        &brassclaw_llm::RegistryProviderConfig::generic(
            brassclaw_llm::registry::ProviderProtocol::OpenAiCompletions,
            "counter-fixture",
            None,
            "http://127.0.0.1:9/v1",
            model,
        ),
        5,
    )
    .unwrap();
    let mut sdk_request = brassclaw_llm::CompletionRequest::new(vec![
        brassclaw_llm::ChatMessage::system("hello"),
        brassclaw_llm::ChatMessage::user("world world"),
    ])
    .with_model(model)
    .with_max_tokens(2);
    sdk_request.dispatch_policy = brassclaw_llm::ProviderDispatchPolicy::SingleSubmission;
    let sdk_result = guard.clone().complete(provider.as_ref(), sdk_request).await;
    assert!(
        matches!(
            &sdk_result,
            Err(brassclaw_llm::LlmError::FinalRequestRejected {
                reason: FinalRequestError::Tokens(ModelTokenCheckError::PromptExceeded)
            })
        ),
        "actual SDK text-part arrays must reach the real native counter and canonical hard-limit guard: {sdk_result:?}"
    );
    let before = governor.usage_for(&account).process_count;
    let mut request_with_header = request(body.clone());
    request_with_header.headers_mut().insert(
        "x-unqualified-template",
        reqwest::header::HeaderValue::from_static("different"),
    );
    assert!(matches!(
        guard.prepare(request_with_header).await,
        Err(FinalRequestError::Binding)
    ));
    assert_eq!(
        governor.usage_for(&account).process_count,
        before,
        "unqualified header must block before starting a worker"
    );

    assert!(matches!(
        counter.measure(input(body.as_bytes(), "other")).await,
        Err(FinalRequestError::Target)
    ));
    assert_eq!(governor.usage_for(&account).process_count, before);
    let invalid=json!({"model":selection.model,"messages":[{"role":"system","content":"hello"},{"role":"user","content":"world"}],"max_tokens":2,"unqualified_extension":"x"}).to_string();
    assert!(matches!(
        counter
            .measure(input(invalid.as_bytes(), &selection.model))
            .await,
        Err(FinalRequestError::Measurement)
    ));
    assert_eq!(governor.usage_for(&account).process_count, before + 1);
    // Actual observation from the native worker supplies the mutation subject.
    let probes=json!([{"kind":"wire_chat","request_json":body.clone(),"mapping_json":selection.mapping_json.to_string(),"expected_model":selection.model,"leading_prefix":"hello"}]).to_string();
    let actual = backend
        .encode(
            &selection.specification,
            &selection.artifact_set,
            &probes,
            None,
            &scope,
        )
        .await
        .unwrap();
    let bytes = actual.output.as_str().unwrap();
    let prefix_binding = BoundFinalEncoding::Prefix(selection.clone());
    measurement(
        &prefix_binding,
        input(body.as_bytes(), &selection.model),
        bytes,
    )
    .unwrap();
    let original: Value = serde_json::from_str(bytes).unwrap();
    for (path, value) in [
        ("/template_origin", json!("supplied-template")),
        (
            "/preparation_revision",
            json!(selection.preparation_revision + 1),
        ),
        ("/worker_checksum", json!("0".repeat(64))),
        ("/artifact_set", json!("0".repeat(64))),
        ("/report/tokenizer_sha256", json!("0".repeat(64))),
        ("/report/chat_template_sha256", json!("0".repeat(64))),
        (
            "/report/probes/0/wire_input/request_sha256",
            json!("0".repeat(64)),
        ),
        (
            "/report/probes/0/wire_input/mapping_sha256",
            json!("0".repeat(64)),
        ),
        ("/report/probes/0/wire_input/model", json!("other")),
        (
            "/report/probes/0/wire_input/reserved_output_tokens",
            json!(0),
        ),
        (
            "/report/probes/0/partition/prefix_bytes_sha256",
            json!("0".repeat(64)),
        ),
        ("/report/probes/0/partition/prompt_tokens", json!(3)),
        (
            "/report/probes/0/partition/render_context_sha256",
            json!("0".repeat(64)),
        ),
        ("/report/common_prefix_tokens", json!(1)),
        ("/report/truncation_disabled", json!(false)),
        ("/report/padding_disabled", json!(false)),
    ] {
        let mut forged = original.clone();
        *forged.pointer_mut(path).unwrap() = value;
        assert!(
            measurement(
                &prefix_binding,
                input(body.as_bytes(), &selection.model),
                &forged.to_string()
            )
            .is_err(),
            "accepted {path}"
        );
    }
    let workflow = Arc::new(WorkflowFinalEncodingSelection {
        workflow_reference: "retained-workflow:counter-fixture".into(),
        specification: selection.specification.clone(),
        artifact_set: selection.artifact_set.clone(),
        provider_id: selection.provider_id.clone(),
        model: selection.model.clone(),
        mapping_json: selection.mapping_json.clone(),
        tokenizer_sha256: selection.tokenizer_sha256.clone(),
        tokenizer_config_sha256: selection.tokenizer_config_sha256.clone(),
        template_sha256: selection.template_sha256.clone(),
        worker_sha256: selection.worker_sha256.clone(),
        preparation_revision: selection.preparation_revision,
    });
    let unsupported = ContainedFinalRequestMeasurer::for_workflow(
        workflow.clone(),
        scope.clone(),
        backend.clone(),
        verifier,
        governor.clone(),
    )
    .unwrap();
    let before = governor.usage_for(&account).process_count;
    assert!(matches!(
        unsupported.measure(input(body.as_bytes(), model)).await,
        Err(FinalRequestError::Binding)
    ));
    assert_eq!(
        governor.usage_for(&account).process_count,
        before,
        "prefix-only verifier must reject before worker execution"
    );
    let free = Arc::new(
        ContainedFinalRequestMeasurer::for_workflow(
            workflow.clone(),
            scope.clone(),
            backend.clone(),
            Arc::new(WorkflowFixtureBinding {
                scope: scope.clone(),
                workflow: workflow.workflow_reference.clone(),
            }),
            governor.clone(),
        )
        .unwrap(),
    );
    let full = free.measure(input(body.as_bytes(), model)).await.unwrap();
    assert_eq!(
        full.counts,
        ModelRequestTokenCounts {
            prefix_tokens: 0,
            prompt_tokens: 2,
            input_tokens: 2
        }
    );
    assert_eq!(full.body_sha256, counts.body_sha256);
    let free_tools = free.measure(input(json!({"model":model,"messages":[{"role":"system","content":"hello"},{"role":"user","content":"world"}],"max_tokens":2,"tools":[{"type":"function","function":{"name":"inspect","description":"world hello","parameters":{"type":"object"}}}]}).to_string().as_bytes(), model)).await.unwrap();
    assert_eq!(free_tools.counts.prefix_tokens, 0);
    assert!(free_tools.counts.prompt_tokens > full.counts.prompt_tokens);
    let mut zero_prefix = registered.definition.clone();
    zero_prefix
        .model_token_limits
        .0
        .get_mut(model)
        .unwrap()
        .max_prefix_tokens = 0;
    registered = repo
        .replace_revision(zero_prefix, Some(registered.revision))
        .await
        .unwrap();
    // A NEW workflow captures the zero-prefix policy. The earlier policy's
    // unmeasured tightening fence must not be disabled merely for this direct
    // guard fixture; ordinary selected SDK dispatch has its separate custody.
    let zero_capture = crate::RebornProviderAdmin::new(rig.pool.as_ref().clone(), owner)
        .capture_selection(
            crate::provider_admin::ProviderSelectionSource::Role(ProviderRole::Kohai),
            &keys,
        )
        .await
        .unwrap()
        .unwrap();
    let zero_policy = crate::provider_dispatch::PgProviderDispatchGuard::from_capture(
        rig.pool.as_ref().clone(),
        owner,
        keys,
        &zero_capture,
    )
    .unwrap();
    let free_guard = FinalRequestGuard::new(
        "counter-fixture".into(),
        model.into(),
        "http://127.0.0.1:9/v1/chat/completions".parse().unwrap(),
        zero_policy,
        free,
    )
    .unwrap();
    let short =
        json!({"model":model,"messages":[{"role":"system","content":"hello"}],"max_tokens":2})
            .to_string();
    free_guard.prepare(request(short)).await.unwrap();
    assert!(
        matches!(
            free_guard.prepare(request(body.clone())).await,
            Err(FinalRequestError::Tokens(
                ModelTokenCheckError::PromptExceeded
            ))
        ),
        "fixed instructions and user content must both consume the canonical prompt ceiling even with prefix cap zero"
    );
    let mut without_limits = registered.definition.clone();
    without_limits.model_token_limits.0.clear();
    repo.replace_revision(without_limits, Some(registered.revision))
        .await
        .unwrap();
    assert!(
        matches!(
            guard.prepare(request(body)).await,
            Err(FinalRequestError::Policy)
        ),
        "removing canonical hard limits must block even though discretionary token budgets are absent"
    );
}
