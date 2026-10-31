//! Real storage, contained tokenizer and ordinary task-role caller regression.
//! Synthetic reference/WordLevel model + loopback Chat protocol peer, never an
//! expert-model/cache/deployment acceptance or installation approval bypass.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_host_api::{CapabilityId, InvocationId, ResourceScope};
use brassclaw_llm::{ProviderRole, SessionManager};
use brassclaw_loop_support::{
    HostManagedModelGateway, HostManagedModelMessage, HostManagedModelMessageRole,
    HostManagedModelRequest,
};
use brassclaw_product_workflow::{
    CapturePrefixModelRequest, ImportPrefixGenerationRequest, PrefixAcceptanceDecision,
    PrefixAcceptanceSelection, PrefixModelTarget, RetainPrefixDesignProposalRequest,
    RetainPrefixModelArtifactsRequest,
};
use brassclaw_reborn::model_gateway::{LlmModelProfilePolicy, LlmProviderModelGateway};
use brassclaw_resources::InMemoryResourceGovernor;
use brassclaw_turns::{LoopMessageRef, run_profile::*};
use serde_json::{Value, json};

const TEMPLATE: &str = "{% for message in messages %}{% if message.role == 'system' %}SYSTEM\n{{ message.content }}\nEND\n{% elif message.role == 'user' %}USER\n{{ message.content }}\nEND\n{% else %}ASSISTANT\n{{ message.content }}\nEND\n{% endif %}{% endfor %}{% if tools %}{% for tool in tools %}TOOL {{ tool.function.name }} {{ tool.function.description }} {{ tool.function.parameters|tojson }}\n{% endfor %}{% endif %}ASSISTANT\n";
const MODEL: &str = "prefix-binding-wordlevel-fixture";
const PROVIDER: &str = "prefix-binding-fixture";

fn mapping() -> String {
    json!({"format":brassclaw_model_encoding::wire::WIRE_MAPPING,
        "render_defaults_json":r#"{"add_generation_prompt":true,"continue_final_message":false}"#,
        "missing_tools":"null","null_assistant_content":"preserve","tool_arguments":"preserve",
        "text_parts":{"mode":"join","separator":"\n"},"forward_tool_choice":true,
        "forward_response_format":true,"allowed_tool_choices":["auto","none","required"],
        "allowed_chat_kwargs":[],"allowed_message_fields":["role","content","tool_calls","tool_call_id","name"]})
    .to_string()
}
struct Fixture {
    pool: Arc<PgPool>,
    owner: String,
    specification: String,
    pair: String,
    artifacts: String,
    worker: Arc<crate::source_inspection::ContainedSourceInspection>,
    governor: Arc<InMemoryResourceGovernor>,
    scope: ResourceScope,
}
impl Fixture {
    async fn new(
        pool: Arc<PgPool>,
        scope: ResourceScope,
        provider_id: &str,
        model: &str,
        revision: u64,
        context: u32,
    ) -> Self {
        let namespace = scope.tenant_id.to_string();
        let designs =
            crate::prefix_model_designs::PgPrefixModelDesigns::new(pool.clone(), namespace.clone());
        let specification = designs
            .capture(
                CapturePrefixModelRequest {
                    provider_id: provider_id.into(),
                    expected_provider_revision: revision,
                    model: model.into(),
                },
                vec![PrefixModelTarget {
                    provider_id: provider_id.into(),
                    provider_revision: revision,
                    model: model.into(),
                    consumer: "kohai".into(),
                    configured_context_window: Some(context),
                }],
            )
            .await
            .unwrap();
        let config = json!({"chat_template":TEMPLATE}).to_string();
        let assets=crate::prefix_model_artifacts::PgPrefixModelArtifacts::new(pool.clone(),namespace.clone())
        .retain_with_origin(RetainPrefixModelArtifactsRequest {specification:specification.id.clone(),artifact_set_bytes:json!({"format":"brassclaw-prefix-model-artifact-import/1",
            "artifacts":{"config.json":{"encoding":"base64","data":STANDARD.encode("{}")},
                "tokenizer.json":{"encoding":"base64","data":STANDARD.encode(crate::bootstrap_encoding_fixture::tokenizer())},
                "tokenizer_config.json":{"encoding":"base64","data":STANDARD.encode(&config)}}}).to_string()},
            crate::prefix_model_artifacts::ArtifactImportOrigin::Operator {actor:"native-binding-operator".into()}).await.unwrap();
        let mut pair = crate::prefix_pair_construction::tests::draft(
            &specification.id,
            &assets.id,
            "tokenizer_config.json",
            &checksum(config.as_bytes()),
            json!("/chat_template"),
            &checksum(TEMPLATE.as_bytes()),
            &checksum(b"{}"),
        );
        pair["format"] = json!(crate::prefix_pair_construction::WIRE_FORMAT);
        pair["construction"]["prefix"]["leading_literal"] = json!("SYSTEM\n");
        pair["construction"]["prefix"]["trailing_literal"] = json!("");
        pair["construction"]["reference_normalization"] = json!("preserve");
        pair["construction"]["request_output_tokens"] = json!(16);
        pair["construction"]["wire_mapping_json"] = json!(mapping());
        pair["construction"]["supported_modes"] = json!(["plain", "tools"]);
        pair["construction"]["unsupported_modes"] = json!({});
        pair["sample_reference"] = json!("Reference");
        let case = |id: &str, tools: bool| {
            let mut body = json!({"model":model,"messages":[{"role":"system","content":"Reference"},{"role":"user","content":"Question"}],"max_tokens":16});
            if tools {
                body["tools"] = json!([{"type":"function","function":{"name":"inspect","description":"read data","parameters":{}}}]);
            }
            let dynamic = format!(
                "\nEND\nUSER\nQuestion\nEND\n{}ASSISTANT",
                if tools {
                    "TOOL inspect read data {}\n"
                } else {
                    ""
                }
            );
            json!({"id":id,"mode":if tools {"tools"} else {"plain"},
            "render_context_json":json!({"messages":body["messages"],"tools":body.get("tools"),"add_generation_prompt":true,"continue_final_message":false}).to_string(),
            "expected_fixed_prefix":"SYSTEM\nReference","formatted_prompt":dynamic,
            "expected_request":format!("SYSTEM\nReference{dynamic}"),"evidence":["template"],"wire_request_json":body.to_string()})
        };
        pair["worked_cases"] = json!([case("plain", false), case("tools", true)]);
        let pair = designs
            .retain_pair_proposal(RetainPrefixDesignProposalRequest {
                specification: specification.id.clone(),
                proposal_bytes: pair.to_string(),
            })
            .await
            .unwrap();
        let settings =
            Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
        settings
            .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
                revision: 1,
                limits: Default::default(),
            })
            .unwrap();
        let worker = Arc::new(crate::source_inspection::ContainedSourceInspection::new());
        worker.bind(settings).await.unwrap();
        Fixture {
            pool: pool.clone(),
            owner: namespace.clone(),
            specification: specification.id,
            pair: pair.id,
            artifacts: assets.id,
            worker: worker.clone(),
            governor: Arc::new(InMemoryResourceGovernor::new()),
            scope,
        }
    }
    fn store(&self) -> PgPrefixAcceptances {
        PgPrefixAcceptances::new(self.pool.clone(), self.owner.clone())
    }
    async fn prepare(
        &self,
        profile: &str,
        generation: &str,
    ) -> Result<PreparedPrefixReview, Error> {
        let backend = crate::model_encoding::ContainedModelEncoding::new(
            self.pool.clone(),
            self.worker.clone(),
        );
        self.store()
            .prepare(
                profile,
                generation,
                &self.scope,
                &backend,
                self.governor.clone(),
            )
            .await
    }
    async fn package(&self, profile: &str, revision_name: &str) -> (String, String, String) {
        let source = crate::bootstrap_compiler_fixture::CompilerFixture::scoped(
            profile,
            "literal-reference/v5-contextual-sharing",
            revision_name,
        )
        .unwrap();
        let original = source.compile_for_test();
        source.verify(&original).unwrap();
        let generations = crate::prefix_generations::PgPrefixGenerations::new(self.pool.clone());
        let retained = generations
            .import(
                profile,
                ImportPrefixGenerationRequest {
                    expected_profile_revision: 1,
                    generation_bytes: original.clone(),
                },
            )
            .await
            .unwrap();
        assert_eq!(retained.summary.kind, "compiler-package");
        let content = json!({"format":"brassclaw-prefix-design-proposal/2","profile":profile,
            "specification":self.specification,"rationale":"Synthetic complete-reference fixture, not domain knowledge",
            "design":{"adapter":"whole-unit-reference/1","generation_rules":{
                "format":"brassclaw-prefix-generation-rules/1","profile":profile,
                "header":"Preserve every original unit and provenance.",
                "required_coverage":[{"path":"","match":"prefix","first_lines":[]}],
                "bands":[{"minimum_tokens":0,"maximum_tokens":100000,"groups":[{
                    "id":"complete-reference","required":true,"priority":0,"dependencies":[],
                    "selectors":[{"path":"","match":"prefix","first_lines":[]}]}]}]}}});
        let proposal = crate::prefix_model_designs::PgPrefixModelDesigns::new(
            self.pool.clone(),
            self.owner.clone(),
        )
        .retain_proposal(
            profile,
            RetainPrefixDesignProposalRequest {
                specification: self.specification.clone(),
                proposal_bytes: content.to_string(),
            },
        )
        .await
        .unwrap();
        let package: Value = serde_json::from_str(&original).unwrap();
        let context = String::from_utf8(
            STANDARD
                .decode(package["artifacts"]["context.md"]["data"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        let prefix = format!("SYSTEM\n{context}");
        let binding = json!({"format":"brassclaw-prefix-model-input-binding/1","specification":self.specification,
            "artifact_set":self.artifacts,"pair_proposal":self.pair,"profile_proposal":proposal.id,
            "mapping_sha256":checksum(mapping().as_bytes()),"prefix_file":"prefix.utf8","prefix_sha256":checksum(prefix.as_bytes())});
        let target = json!({"format":crate::prefix_generation_binding::TARGET_FORMAT,"profile":profile,
            "source_generation":retained.summary.generation,"source_package_bytes":original,
            "artifacts":{"prefix.utf8":{"encoding":"base64","data":STANDARD.encode(prefix.as_bytes())},
                "model_input_binding.json":{"encoding":"base64","data":STANDARD.encode(binding.to_string())}}});
        let target = generations
            .import(
                profile,
                ImportPrefixGenerationRequest {
                    expected_profile_revision: 1,
                    generation_bytes: target.to_string(),
                },
            )
            .await
            .unwrap();
        assert_eq!(target.summary.kind, "model-target");
        assert_eq!(target.summary.qualification, "quarantined");
        (
            target.summary.generation,
            retained.summary.generation,
            context,
        )
    }
    async fn closed_frame_candidate(&self, profile: &str, generation: &str) -> String {
        let designs = crate::prefix_model_designs::PgPrefixModelDesigns::new(
            self.pool.clone(),
            self.owner.clone(),
        );
        let pair = designs
            .get_pair_proposal(&self.specification, &self.pair)
            .await
            .unwrap();
        let mut document: Value = serde_json::from_str(&pair.proposal_bytes).unwrap();
        document["construction"]["prefix"]["trailing_literal"] = json!("\nEND\n");
        for case in document["worked_cases"].as_array_mut().unwrap() {
            case["expected_fixed_prefix"] = json!("SYSTEM\nReference\nEND\n");
            case["formatted_prompt"] = json!(
                case["formatted_prompt"]
                    .as_str()
                    .unwrap()
                    .strip_prefix("\nEND\n")
                    .unwrap()
            );
        }
        let closed = designs
            .retain_pair_proposal(RetainPrefixDesignProposalRequest {
                specification: self.specification.clone(),
                proposal_bytes: document.to_string(),
            })
            .await
            .unwrap();
        let generations = crate::prefix_generations::PgPrefixGenerations::new(self.pool.clone());
        let target = generations.get(profile, generation).await.unwrap();
        let mut package: Value = serde_json::from_str(&target.generation_bytes).unwrap();
        let mut prefix = String::from_utf8(
            STANDARD
                .decode(
                    package["artifacts"]["prefix.utf8"]["data"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap(),
        )
        .unwrap();
        prefix.push_str("\nEND\n");
        let mut binding: Value = serde_json::from_slice(
            &STANDARD
                .decode(
                    package["artifacts"]["model_input_binding.json"]["data"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap(),
        )
        .unwrap();
        binding["pair_proposal"] = json!(closed.id);
        binding["prefix_sha256"] = json!(checksum(prefix.as_bytes()));
        package["artifacts"]["prefix.utf8"]["data"] = json!(STANDARD.encode(prefix.as_bytes()));
        package["artifacts"]["model_input_binding.json"]["data"] =
            json!(STANDARD.encode(binding.to_string()));
        generations
            .import(
                profile,
                ImportPrefixGenerationRequest {
                    expected_profile_revision: 1,
                    generation_bytes: package.to_string(),
                },
            )
            .await
            .unwrap()
            .summary
            .generation
    }
    async fn approve(&self, profile: &str, generation: &str) -> Uuid {
        let review = self.prepare(profile, generation).await.unwrap();
        let view = review.view();
        let id = Uuid::new_v4();
        self.store().approve(review,id,view["review_checksum"].as_str().unwrap(),self.scope.user_id.as_str(),
            "Human fixture review of these exact synthetic originals, reference quality and explicit loopback default-template/mapping contract; no real-model expertise claim").await.unwrap();
        id
    }
}

struct Schemas {
    description: String,
}
#[async_trait]
impl LoopCapabilityPort for Schemas {
    fn tool_definitions(&self) -> Result<Vec<ProviderToolDefinition>, AgentLoopHostError> {
        Ok(vec![ProviderToolDefinition {
            capability_id: CapabilityId::new("fixture.inspect").unwrap(),
            name: "inspect".into(),
            description: self.description.clone(),
            parameters: json!({"type":"object","properties":{}}),
        }])
    }
    async fn visible_capabilities(
        &self,
        _: VisibleCapabilityRequest,
    ) -> Result<VisibleCapabilitySurface, AgentLoopHostError> {
        Err(denied())
    }
    async fn invoke_capability(
        &self,
        _: CapabilityInvocation,
    ) -> Result<CapabilityOutcome, AgentLoopHostError> {
        Err(denied())
    }
    async fn invoke_capability_batch(
        &self,
        _: CapabilityBatchInvocation,
    ) -> Result<CapabilityBatchOutcome, AgentLoopHostError> {
        Err(denied())
    }
}
fn denied() -> AgentLoopHostError {
    AgentLoopHostError::new(
        AgentLoopHostErrorKind::PolicyDenied,
        "metadata fixture grants no Tool dispatch",
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_scoped_acceptance_retains_sources_and_guards_both_task_roles() {
    use axum::{Json, Router, extract::State, routing::post};
    type Calls = Arc<tokio::sync::Mutex<Vec<Value>>>;
    async fn receive(State(calls): State<Calls>, Json(body): Json<Value>) -> Json<Value> {
        calls.lock().await.push(body.clone());
        Json(
            json!({"id":"prefix-binding-response","object":"chat.completion","created":1,"model":MODEL,
            "choices":[{"index":0,"message":{"role":"assistant","content":"native transport result"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
        )
    }
    let calls: Calls = Default::default();
    let app = Router::new()
        .route("/v1/chat/completions", post(receive))
        .with_state(calls.clone());
    let (address, stop, server) = crate::test_loopback_http::spawn(app).await;
    let rig = crate::runtime::test_pg::pg_rig().await;
    let (admitted, _claim) = crate::native_admission_fixture::reserve_unstarted_with_claim(
        rig.pool.clone(),
        "native final model request",
    )
    .await;
    let namespace = admitted.context.scope.tenant_id.to_string();
    let repo = crate::PgProviderRepo::new(rig.pool.as_ref().clone(), &namespace);
    repo.initialize().await.unwrap();
    let definition=serde_json::from_value(json!({"id":PROVIDER,"protocol":"open_ai_completions",
        "default_base_url":format!("http://{address}/v1"),"base_url_required":true,"api_key_required":false,
        "model_env":"UNUSED_PREFIX_BINDING_MODEL","default_model":MODEL,"description":"Synthetic prefix binding transport fixture",
        "context_window_tokens":200000,"model_token_limits":{MODEL:{"max_prefix_tokens":100000,"max_prompt_tokens":100000}},
        "request_policy":brassclaw_llm::ProviderRequestPolicy {max_retries:0,..Default::default()}})).unwrap();
    let provider = repo.replace_revision(definition, None).await.unwrap();
    crate::db_config::save_config_changes(
        &rig.pool,
        &namespace,
        &[
            ("llm.default.provider_id", Some(PROVIDER)),
            ("llm.sempai.provider_id", Some(PROVIDER)),
        ],
        crate::db_config::ConfigWriteContext::Operator,
    )
    .await
    .unwrap();
    let fixture = Fixture::new(
        rig.pool.clone(),
        ResourceScope {
            tenant_id: admitted.context.scope.tenant_id.clone(),
            user_id: admitted.context.actor.as_ref().unwrap().user_id.clone(),
            agent_id: admitted.context.scope.agent_id.clone(),
            project_id: admitted.context.scope.project_id.clone(),
            thread_id: Some(admitted.context.scope.thread_id.clone()),
            invocation_id: InvocationId::new(),
        },
        PROVIDER,
        MODEL,
        provider.revision,
        200000,
    )
    .await;
    let worker = fixture.worker.clone();
    let (base, source_base, reference_base) = fixture.package("base-prompt", "first").await;
    assert!(
        fixture.prepare("base-prompt", &source_base).await.is_err(),
        "source-only is never a serving generation"
    );
    let closed = fixture.closed_frame_candidate("base-prompt", &base).await;
    assert!(
        fixture.prepare("base-prompt", &closed).await.is_err(),
        "authored plain-case declarations cannot approve an SDK-incompatible closed system boundary"
    );
    assert!(
        calls.lock().await.is_empty(),
        "draft SDK qualification never sends a provider request"
    );
    let review = fixture.prepare("base-prompt", &base).await.unwrap();
    assert!(
        fixture
            .store()
            .approve(review, Uuid::new_v4(), &"0".repeat(64), "human", "review")
            .await
            .is_err()
    );
    assert!(fixture.store().bindings().await.unwrap().is_empty());
    let base_id = fixture.approve("base-prompt", &base).await;
    assert!(
        fixture.store().bindings().await.unwrap().is_empty(),
        "approval cannot activate"
    );
    let prepared = worker.prepare().await.unwrap();
    assert_eq!(
        fixture
            .store()
            .activate(base_id, 0, prepared.revision)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        fixture
            .store()
            .activate(base_id, 0, prepared.revision)
            .await
            .unwrap(),
        1,
        "lost reply reads same publication"
    );
    let (sempai, _, reference_sempai) = fixture.package("sempai", "first").await;
    let sempai_id = fixture.approve("sempai", &sempai).await;
    fixture
        .store()
        .activate(sempai_id, 0, prepared.revision)
        .await
        .unwrap();
    let b = fixture
        .store()
        .resolve(base_id, prepared.revision)
        .await
        .unwrap();
    let s = fixture
        .store()
        .resolve(sempai_id, prepared.revision)
        .await
        .unwrap();
    assert_eq!(
        b.pair_acceptance, s.pair_acceptance,
        "same pair shares one accepted design"
    );
    assert_ne!(b.profile_acceptance, s.profile_acceptance);
    let legacy = Uuid::new_v4();
    rig.pool.get().await.unwrap().execute("INSERT INTO reborn_prefix_design_acceptances(acceptance_id,proposal_id,candidate_profile,candidate_generation,evidence_bytes,checksum) VALUES($1,$2,'base-prompt',$3,'{}',$4)",
        &[&legacy,&fixture.store().record(base_id).await.unwrap().review.profile_proposal,&source_base,&checksum(b"{}")]).await.unwrap();
    assert!(
        fixture
            .store()
            .resolve(legacy, prepared.revision)
            .await
            .is_err(),
        "historical records stay unqualified"
    );
    let keys = crate::LlmKeyStore::new_postgres(Arc::new(
        brassclaw_secrets::PgSecretStore::new(
            rig.pool.as_ref().clone(),
            "prefix-binding-native-master-fixture".into(),
            &namespace,
        )
        .unwrap(),
    ));
    let sources = crate::task_model_provider::TaskProviderConfiguration::new(
        rig.pool.as_ref().clone(),
        &namespace,
        keys,
    )
    .with_final_encoding(Some(worker), fixture.governor.clone())
    .with_session(Arc::new(SessionManager::new(Default::default())));
    let make_gateway = |role, profile: &str| {
        LlmProviderModelGateway::new(
            Arc::new(crate::runtime::PlaceholderLlmProvider),
            LlmModelProfilePolicy::new()
                .allow_model_profile(ModelProfileId::new(profile).unwrap(), None),
        )
        .with_task_source(sources.role(role))
    };
    let primary = make_gateway(ProviderRole::Kohai, "interactive_model")
        .capture_task_model_selection(None, None)
        .await
        .unwrap()
        .unwrap();
    let reviewer = make_gateway(ProviderRole::Sempai, "sempai_model")
        .capture_task_model_selection(Some(primary.binding.clone()), None)
        .await
        .unwrap()
        .unwrap();
    let mut corrupt_binding = primary.binding.retained_metadata().unwrap();
    corrupt_binding["prefix_bindings"][0]["qualification"] = json!(Uuid::new_v4());
    assert!(
        admitted
            .admission
            .bind_task_models(Some(corrupt_binding))
            .is_err(),
        "prefix references are included in original native metadata integrity"
    );
    assert!(rig.pool.get().await.unwrap().execute(
        "UPDATE brassclaw_monty_task_admissions SET native_actor='null'::jsonb WHERE run_id=$1",
        &[&admitted.context.run_id.as_uuid()]).await.is_err(), "native actor is immutable admission identity");
    admitted
        .admission
        .bind_task_models(Some(primary.binding.retained_metadata().unwrap()))
        .unwrap();
    admitted.admission.check_and_start().await.unwrap();
    let request = |profile: &str, text: &str| HostManagedModelRequest {
        dispatch_policy: LoopModelDispatchPolicy::SingleSubmission,
        model_profile_id: ModelProfileId::new(profile).unwrap(),
        messages: vec![HostManagedModelMessage {
            role: HostManagedModelMessageRole::User,
            content: text.into(),
            content_ref: LoopMessageRef::new("msg:binding-user").unwrap(),
            tool_result_provider_call: None,
            tool_result_content: None,
        }],
        surface_version: None,
        resolved_model_route: None,
        run_id: admitted.context.run_id,
        turn_id: admitted.context.turn_id,
    };
    primary
        .gateway
        .stream_model(request("interactive_model", "Question"))
        .await
        .unwrap();
    reviewer
        .gateway
        .stream_model(request("sempai_model", "Question"))
        .await
        .unwrap();
    let sent = calls.lock().await.clone();
    assert_eq!(sent.len(), 2);
    for (body, reference) in sent.iter().zip([reference_base, reference_sempai]) {
        assert_eq!(
            body["max_tokens"]
                .as_u64()
                .or_else(|| body["max_completion_tokens"].as_u64()),
            Some(16)
        );
        assert_eq!(
            body["messages"].as_array().unwrap().len(),
            2,
            "one immutable reference plus dynamic input"
        );
        assert!(
            body["messages"][0]["content"]
                .to_string()
                .contains(reference.lines().next().unwrap())
        );
    }
    primary
        .gateway
        .stream_model_with_capabilities(
            request("interactive_model", "Question"),
            Arc::new(Schemas {
                description: "read data".into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        calls.lock().await.len(),
        3,
        "actual Tool-SDK branch also guarded"
    );
    let mut changed = provider.definition.clone();
    changed
        .model_token_limits
        .0
        .get_mut(MODEL)
        .unwrap()
        .max_prompt_tokens = 64;
    let schema_limit = repo
        .replace_revision(changed, Some(provider.revision))
        .await
        .unwrap();
    primary
        .gateway
        .preflight_task_model_usage(None)
        .await
        .unwrap();
    reviewer
        .gateway
        .preflight_task_model_usage(None)
        .await
        .unwrap();
    primary
        .gateway
        .stream_model(request("interactive_model", "Question"))
        .await
        .unwrap();
    assert!(
        primary
            .gateway
            .stream_model_with_capabilities(
                request("interactive_model", "Question"),
                Arc::new(Schemas {
                    description: "data ".repeat(512)
                })
            )
            .await
            .is_err()
    );
    assert_eq!(
        calls.lock().await.len(),
        4,
        "Tool schema expansion is counted after the SDK transformation"
    );
    let mut changed = schema_limit.definition.clone();
    changed
        .model_token_limits
        .0
        .get_mut(MODEL)
        .unwrap()
        .max_prompt_tokens = 100000;
    let provider = repo
        .replace_revision(changed, Some(schema_limit.revision))
        .await
        .unwrap();
    let (successor, _, _) = fixture.package("base-prompt", "successor").await;
    let successor_id = fixture.approve("base-prompt", &successor).await;
    fixture
        .store()
        .activate(successor_id, 1, prepared.revision)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store()
            .resolve(base_id, prepared.revision)
            .await
            .unwrap()
            .generation,
        base
    );
    let pinned = primary.binding.retained_metadata().unwrap();
    assert!(
        pinned["prefix_bindings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["profile"] == "base-prompt" && p["qualification"] == json!(base_id))
    );
    let mut changed = provider.definition.clone();
    changed
        .model_token_limits
        .0
        .get_mut(MODEL)
        .unwrap()
        .max_prompt_tokens = 2;
    let tighter = repo
        .replace_revision(changed, Some(provider.revision))
        .await
        .unwrap();
    assert!(
        primary
            .gateway
            .stream_model(request("interactive_model", "large prompt data data"))
            .await
            .is_err()
    );
    assert!(
        reviewer
            .gateway
            .stream_model(request("sempai_model", "large prompt data data"))
            .await
            .is_err()
    );
    assert_eq!(
        calls.lock().await.len(),
        4,
        "hard prompt caps reject both roles without HTTP"
    );
    let mut changed = tighter.definition.clone();
    changed
        .model_token_limits
        .0
        .get_mut(MODEL)
        .unwrap()
        .max_prompt_tokens = 100000;
    changed
        .model_token_limits
        .0
        .get_mut(MODEL)
        .unwrap()
        .max_prefix_tokens = 1;
    let prefix_tight = repo
        .replace_revision(changed, Some(tighter.revision))
        .await
        .unwrap();
    assert!(
        primary
            .gateway
            .stream_model(request("interactive_model", "Question"))
            .await
            .is_err()
    );
    assert_eq!(
        calls.lock().await.len(),
        4,
        "hard prefix cap cannot be disabled"
    );
    let mut changed = prefix_tight.definition.clone();
    changed
        .model_token_limits
        .0
        .get_mut(MODEL)
        .unwrap()
        .max_prefix_tokens = 100000;
    changed.context_window_tokens = Some(16);
    repo.replace_revision(changed, Some(prefix_tight.revision))
        .await
        .unwrap();
    assert!(
        reviewer
            .gateway
            .stream_model(request("sempai_model", "Question"))
            .await
            .is_err()
    );
    assert_eq!(
        calls.lock().await.len(),
        4,
        "full input plus explicit output must fit context"
    );
    assert!(
        serde_json::from_value::<PrefixAcceptanceDecision>(
            json!({"selection":{"profile":"base-prompt","generation":base},"acceptance_id":base_id,
        "review_checksum":"0".repeat(64),"semantic_review":"review","approved":true})
        )
        .is_err()
    );
    let retry=PrefixAcceptanceDecision {selection:PrefixAcceptanceSelection {profile:"base-prompt".into(),generation:base},acceptance_id:base_id,
        review_checksum:fixture.store().record(base_id).await.unwrap().review_checksum,
        semantic_review:"Human fixture review of these exact synthetic originals, reference quality and explicit loopback default-template/mapping contract; no real-model expertise claim".into()};
    assert!(
        fixture
            .store()
            .approved_retry(&retry, fixture.scope.user_id.as_str())
            .await
            .unwrap()
    );
    stop.cancel();
    server.await.unwrap();
}

/// Actual native data qualification reused by the ordinary task-host test.
/// The WordLevel peer remains a protocol fixture, never model-quality evidence.
pub(crate) struct NativePrefixFixture {
    pub(crate) worker: Arc<crate::source_inspection::ContainedSourceInspection>,
    pub(crate) governor: Arc<InMemoryResourceGovernor>,
    pub(crate) reference: String,
}
pub(crate) async fn qualify_for_existing_provider(
    pool: Arc<PgPool>,
    scope: ResourceScope,
    target: PrefixModelTarget,
    profile: &str,
) -> NativePrefixFixture {
    let fixture = Fixture::new(
        pool,
        scope,
        &target.provider_id,
        &target.model,
        target.provider_revision,
        target
            .configured_context_window
            .expect("actual exact-model context fixture"),
    )
    .await;
    let (generation, _, reference) = fixture.package(profile, "native-host").await;
    let id = fixture.approve(profile, &generation).await;
    let prepared = fixture.worker.prepare().await.unwrap();
    fixture
        .store()
        .activate(id, 0, prepared.revision)
        .await
        .unwrap();
    NativePrefixFixture {
        worker: fixture.worker,
        governor: fixture.governor,
        reference,
    }
}
