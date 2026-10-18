//! Actual native registration + immutable model store + retained worker boundary.
use super::*;
use crate::runtime::RebornRuntime;
use brassclaw_authorization::{
    InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot, ToolExecutionRules,
};
use brassclaw_host_api::{
    CapabilitySet, ExecutionContext, ExtensionId, MountView, NetworkPolicy, RuntimeKind, TrustClass,
};
use brassclaw_host_runtime::{
    HostRuntimeError, MODEL_ENCODING_CAPABILITY_ID, RuntimeCapabilityOutcome,
    RuntimeCapabilityRequest, RuntimeFailureKind,
};
use brassclaw_product_workflow::MontyVmSettingsStore;
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub(super) async fn assert_registered_encoding(
    runtime: &Arc<RebornRuntime>,
    specification: &str,
    artifact_set: &str,
    prefix_job_id: Uuid,
) {
    let root = runtime.global_monty_owner.client().root_identity();
    let capability = CapabilityId::new(MODEL_ENCODING_CAPABILITY_ID).unwrap();
    let kernel = runtime.services.monty_kernel.as_ref().unwrap();
    let native = kernel.builtin(&capability, None).unwrap();
    let identity = native
        .native_identity()
        .expect("actual native implementation registration");
    native.verify_native_artifact().await.unwrap();
    assert_eq!(
        native.native_input_schema().unwrap()["additionalProperties"],
        false
    );
    let tool = Uuid::new_v4();
    let snapshot = |revision, enabled| StableToolPolicySnapshot {
        revision,
        tools: HashMap::from([(
            tool,
            InstanceToolRule {
                enabled,
                revision,
                execution: ToolExecutionRules {
                    allowed_effects: vec![EffectKind::DispatchCapability],
                    mounts: MountView::default(),
                    network: NetworkPolicy::default(),
                    secrets: vec![],
                    resource_ceiling: None,
                },
            },
        )]),
        capabilities: HashMap::from([(capability.clone(), tool)]),
    };
    let policy = Arc::new(LiveStableToolPolicy::new(snapshot(1, true)).unwrap());
    let retained = kernel
        .builtin_with_policy(&capability, identity, policy.clone())
        .unwrap();
    let invoke = |input| {
        let mut context = ExecutionContext::local_default(
            UserId::new("component-deployment-owner").unwrap(),
            ExtensionId::new("builtin").unwrap(),
            RuntimeKind::FirstParty,
            TrustClass::FirstParty,
            CapabilitySet::default(),
            MountView::default(),
        )
        .unwrap();
        context.tenant_id = TenantId::new(runtime.webui_tenant_id()).unwrap();
        context.resource_scope.tenant_id = context.tenant_id.clone();
        RuntimeCapabilityRequest::new(
            context,
            capability.clone(),
            ResourceEstimate::default(),
            input,
            TrustDecision {
                effective_trust: EffectiveTrustClass::user_trusted(),
                authority_ceiling: AuthorityCeiling {
                    allowed_effects: vec![EffectKind::DispatchCapability],
                    max_resource_ceiling: None,
                },
                provenance: TrustProvenance::AdminConfig,
                evaluated_at: chrono::Utc::now(),
            },
        )
    };
    let settings = runtime.monty_settings_owner.store();
    let before = settings.get("default", "default").await.unwrap();
    let mut limits = before.execution_limits;
    limits.utility_worker_memory_bytes = 1024 * 1024 * 1024;
    limits.response_timeout_millis = 120_000;
    let changed = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                json!({"expected_revision":before.revision,"execution_limits":limits}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let job = runtime
        .inspect_prefix_job("base-prompt", prefix_job_id)
        .await
        .unwrap();
    assert_eq!(job.run_status, Some(brassclaw_turns::TurnStatus::Completed));
    let receipt = job.internal_receipt.as_deref().unwrap();
    let generation = receipt
        .strip_prefix("prefix-generation:base-prompt:")
        .unwrap();
    let source = crate::prefix_generations::PgPrefixGenerations::new(
        runtime.installed_catalogue.pool.clone(),
    )
    .get("base-prompt", generation)
    .await
    .unwrap();
    let package: Value = serde_json::from_str(&source.generation_bytes).unwrap();
    use base64::Engine;
    let prefix = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(package["artifacts"]["context.md"]["data"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(
        prefix.len() > 100_000,
        "use the actual retained source compiler output"
    );
    if let Some(path) = std::env::var_os("BRASSCLAW_TEST_CORE_GENERATION_EXPORT") {
        use std::io::Write;
        let directory = std::path::PathBuf::from(path);
        let path = directory.join(format!("core-generation-{generation}.json"));
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(source.generation_bytes.as_bytes()).unwrap();
                file.sync_all().unwrap();
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => assert_eq!(
                std::fs::read(&path).unwrap(),
                source.generation_bytes.as_bytes()
            ),
            Err(error) => panic!("cannot retain native prefix evidence: {error}"),
        }
    }
    let probes=json!(["Inspect firewall rules","Inspect router rules"].map(|question|json!({"kind":"chat","context_json":json!({"messages":[{"role":"system","content":prefix},{"role":"user","content":question}],"add_generation_prompt":true,"enable_thinking":false}).to_string()}))).to_string();
    let input =
        json!({"specification":specification,"artifact_set":artifact_set,"probes_json":probes});
    inspect_native_usage(runtime, tool, identity, &input, false).await;
    let RuntimeCapabilityOutcome::Completed(result) =
        retained.invoke(invoke(input.clone())).await.unwrap()
    else {
        panic!("native model encoding must complete")
    };
    let observed: Value = serde_json::from_str(result.output.as_str().unwrap()).unwrap();
    assert_eq!(observed["format"], "brassclaw-model-encoding-observation/1");
    assert_eq!(observed["qualification"], "offline-measurement-only");
    assert_eq!(observed["specification"], specification);
    assert_eq!(observed["artifact_set"], artifact_set);
    assert_eq!(observed["preparation_revision"], changed.revision);
    assert!(
        observed["report"]["probes"][0]["token_count"]
            .as_u64()
            .unwrap()
            > 1024
    );
    assert_eq!(observed["report"]["truncation_disabled"], true);
    assert_eq!(observed["report"]["padding_disabled"], true);
    eprintln!(
        "core prefix encoding: context_bytes={} token_counts={} stable_prefix_tokens={}",
        prefix.len(),
        observed["report"]["probes"],
        observed["report"]["common_prefix_tokens"]
    );
    assert_eq!(result.usage.process_count, 1);
    // Measure the existing compiler's actual injection adapter with changing
    // tools/roles/thinking. A system-message-only probe would miss tool preambles.
    let originals =
        std::path::PathBuf::from(std::env::var_os("BRASSCLAW_TEST_MODEL_ARTIFACT_ROOT").unwrap());
    let original_template = std::fs::read_to_string(originals.join("chat_template.jinja")).unwrap();
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let adapter =
        brassclaw_skills::prefix_reference_components::template_adapter_draft(Uuid::new_v4())
            .unwrap();
    let source = adapter.document()["content"].as_str().unwrap();
    let preparation = brassclaw_monty_host::utility::UtilityLimits::from_execution(limits).unwrap();
    inspect_pure_prefix_draft(runtime, &adapter, preparation).await;
    let request = |original: String| brassclaw_monty_host::utility::UtilityRequest::Evaluate {
        source: format!("{source}\nbuild_prefix_template(inputs)\n"),
        inputs: std::collections::BTreeMap::from([(
            "inputs".into(),
            json!({"reference":prefix,"original_template":original}),
        )]),
        bounds: preparation.bounds,
        max_compute_time: Duration::from_secs(5),
    };
    let brassclaw_monty_host::utility::UtilityOutput::Evaluated { value, stdout } =
        brassclaw_monty_host::utility::execute(
            &runtime.installed_catalogue.worker,
            request(original_template.clone()),
            preparation.process,
        )
        .await
        .unwrap()
    else {
        panic!("template adapter must return literal bytes")
    };
    assert!(stdout.is_empty());
    let template = value.as_str().unwrap().to_owned();
    assert!(
        brassclaw_monty_host::utility::execute(
            &runtime.installed_catalogue.worker,
            request("unsupported template".into()),
            preparation.process
        )
        .await
        .is_err(),
        "changed template anchors require a reviewed adapter"
    );
    let mut contexts = Vec::new();
    for thinking in [false, true] {
        for tools in [
            Value::Null,
            json!([{"type":"function","function":{"name":"read_configuration","parameters":{"type":"object","properties":{}}}}]),
            json!([{"type":"function","function":{"name":"list_interfaces","parameters":{"type":"object","properties":{"device":{"type":"string"}}}}}]),
        ] {
            contexts.push(json!({"messages":[{"role":"system","content":"Operator instructions."},{"role":"user","content":"CACHE-PROBE-A"}],"tools":tools,"add_generation_prompt":true,"enable_thinking":thinking}));
        }
        for system in [
            Value::Null,
            json!([{"type":"text","text":"Operator instructions."}]),
        ] {
            contexts.push(json!({"messages":[{"role":"system","content":system},{"role":"user","content":"CACHE-PROBE-B"}],"add_generation_prompt":true,"enable_thinking":thinking}));
        }
        contexts.push(json!({"messages":[{"role":"user","content":"CACHE-PROBE-C"}],"add_generation_prompt":true,"enable_thinking":thinking}));
    }
    let probes: Vec<_> = contexts
        .into_iter()
        .map(|context| json!({"kind":"chat","context_json":context.to_string()}))
        .collect();
    let template_input = json!({"specification":specification,"artifact_set":artifact_set,"probes_json":serde_json::to_string(&probes).unwrap(),"template_utf8":template});
    inspect_native_usage(runtime, tool, identity, &template_input, true).await;
    let measured = match retained
        .invoke(invoke(template_input.clone()))
        .await
        .unwrap()
    {
        RuntimeCapabilityOutcome::Completed(measured) => measured,
        RuntimeCapabilityOutcome::Failed(error) => panic!(
            "effective template measurement failed: kind={:?}, summary={:?}",
            error.kind, error.message
        ),
        other => panic!(
            "effective template measurement did not complete: {}",
            other.kind()
        ),
    };
    let effective: Value = serde_json::from_str(measured.output.as_str().unwrap()).unwrap();
    assert_eq!(effective["template_origin"], "supplied-template");
    assert_ne!(
        effective["report"]["chat_template_sha256"],
        effective["model_chat_template_sha256"]
    );
    assert!(
        effective["report"]["common_prefix_tokens"]
            .as_u64()
            .unwrap()
            > 10_000,
        "different tools/roles must retain a measured leading reference"
    );
    let oracle_request = json!({"format":"brassclaw-model-encoding-worker/1","expected_reference":prefix,"assets":{
        "tokenizer_json":std::fs::read_to_string(originals.join("tokenizer.json")).unwrap(),
        "tokenizer_config_json":std::fs::read_to_string(originals.join("tokenizer_config.json")).unwrap(),"chat_template":template},"probes":probes});
    let mut oracle = Command::new("python3")
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../brassclaw_model_encoding/tests/huggingface_oracle.py"),
        )
        .env("BRASSCLAW_ENCODING_MODEL_ORIGINALS", &originals)
        .env("HF_HUB_OFFLINE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    oracle
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&oracle_request).unwrap().as_bytes())
        .unwrap();
    let oracle = oracle.wait_with_output().unwrap();
    assert!(oracle.status.success());
    assert_eq!(
        effective["report"],
        serde_json::from_slice::<Value>(&oracle.stdout).unwrap()
    );
    eprintln!(
        "effective injected prefix: probe_count={} stable_prefix_tokens={} request_tokens={}",
        effective["report"]["probes"].as_array().unwrap().len(),
        effective["report"]["common_prefix_tokens"],
        effective["report"]["probes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|probe| probe["token_count"].to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_capacity_comparison(runtime, measured.output.as_str().unwrap()).await;
    let mut invalid_template = template_input;
    invalid_template["template_utf8"] = json!("{{ raise_exception('reject supplied template') }}");
    assert!(
        matches!(
            retained.invoke(invoke(invalid_template)).await.unwrap(),
            RuntimeCapabilityOutcome::Failed(_)
        ),
        "supplied-template failure must never fall back to the original"
    );
    policy.publish(1, snapshot(2, false)).unwrap();
    let RuntimeCapabilityOutcome::Failed(blocked) =
        retained.invoke(invoke(input.clone())).await.unwrap()
    else {
        panic!("live Tool block must apply")
    };
    assert_eq!(blocked.kind, RuntimeFailureKind::Authorization);
    policy.publish(2, snapshot(3, true)).unwrap();
    for invalid in [
        json!({"specification":specification,"artifact_set":"invalid","probes_json":"[]"}),
        json!({"specification":specification,"artifact_set":artifact_set,"probes_json":"[]","worker":"/tmp/untrusted"}),
    ] {
        match retained.invoke(invoke(invalid)).await {
            Ok(RuntimeCapabilityOutcome::Failed(error)) => {
                assert_eq!(error.kind, RuntimeFailureKind::InvalidInput)
            }
            Err(HostRuntimeError::InvalidRequest { .. }) => {}
            _ => panic!("invalid native model input must fail closed"),
        }
    }
    let mut missing = input.clone();
    missing["artifact_set"] = json!("0".repeat(64));
    assert!(
        matches!(
            retained.invoke(invoke(missing)).await.unwrap(),
            RuntimeCapabilityOutcome::Failed(_)
        ),
        "missing immutable assets must never read latest or a checkout file"
    );
    settings.upsert("default","default",&serde_json::from_value(json!({"expected_revision":changed.revision,"execution_limits":before.execution_limits})).unwrap()).await.unwrap();
    assert_eq!(runtime.global_monty_owner.client().root_identity(), root);
}

async fn inspect_native_usage(
    runtime: &Arc<RebornRuntime>,
    tool: Uuid,
    identity: brassclaw_host_runtime::NativeImplementationRef,
    input: &Value,
    with_template: bool,
) {
    let ids = brassclaw_skills::global_bootstrap_components::UsageComponentIds {
        recipe: Uuid::new_v4(),
        python_code: Uuid::new_v4(),
        tool_skill: Uuid::new_v4(),
        tool,
        skill: Uuid::new_v4(),
        formatter: None,
    };
    let mut drafts = if with_template {
        brassclaw_skills::model_encoding_components::model_template_encoding_usage_drafts(ids)
    } else {
        brassclaw_skills::model_encoding_components::model_encoding_usage_drafts(ids)
    }
    .unwrap();
    drafts.push(
        brassclaw_skills::model_encoding_components::model_encoding_tool_draft(
            tool,
            serde_json::to_value(identity).unwrap(),
        )
        .unwrap(),
    );
    let store = brassclaw_skills::revision_store::PgComponentRevisionStore::new(
        runtime.installed_catalogue.pool.clone(),
    );
    let mut refs = Vec::new();
    for draft in &drafts {
        refs.push(store.retain_packaged_draft(draft).await.unwrap());
    }
    let graph = Arc::new(store.read_exact(&[ids.recipe], &refs).await.unwrap());
    let selected = brassclaw_engine::memory::retained_instruction::compile_retained_recipe(
        graph,
        ids.recipe,
        "selected",
        brassclaw_engine::memory::retained_instruction::WorkflowClass::Deterministic,
    )
    .unwrap();
    let program =
        brassclaw_engine::memory::retained_tools::prepare_retained_tool_program(selected).unwrap();
    assert_eq!(
        program.bindings()["0:2"].capability_id(),
        MODEL_ENCODING_CAPABILITY_ID
    );
    let mut pattern = "encode model specification=% artifacts=% probes=%".to_owned();
    let mut example = format!(
        "encode model specification={} artifacts={} probes={}",
        input["specification"].as_str().unwrap(),
        input["artifact_set"].as_str().unwrap(),
        input["probes_json"].as_str().unwrap()
    );
    if with_template {
        pattern.push_str(" template=%");
        example.push_str(" template=");
        example.push_str(input["template_utf8"].as_str().unwrap());
    }
    let bound = program
        .inputs()
        .bind_variant_example(&pattern, &example, &json!({}))
        .unwrap();
    assert!(
        bound == *input,
        "typed command binding must preserve complete opaque inputs"
    );
    let current = runtime
        .monty_settings_owner
        .store()
        .effective_view()
        .get("default", "default")
        .await
        .unwrap();
    let inspected =
        brassclaw_engine::executor::retained_source::InspectedRetainedProgram::inspect_with_limits(
            brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(Arc::new(program)),
            &runtime.installed_catalogue.worker,
            brassclaw_monty_host::utility::UtilityLimits::from_execution(current.execution_limits)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        inspected.invocation("0:2"),
        Some(if with_template {
            "result = encode_model_template_probes(inputs=inputs)"
        } else {
            "result = encode_model_probes(inputs=inputs)"
        })
    );
}

// Pure component execution over the actual native observation, with explicit
// fixture limits. A successful calculation grants no activation or cache claim.
async fn assert_capacity_comparison(runtime: &Arc<RebornRuntime>, observation: &str) {
    use brassclaw_monty_host::utility::{self, UtilityLimits, UtilityOutput, UtilityRequest};
    use std::{collections::BTreeMap, time::Duration};
    let draft =
        brassclaw_skills::prefix_compiler_components::capacity_draft(Uuid::new_v4()).unwrap();
    let document = draft.document();
    let observed: Value = serde_json::from_str(observation).unwrap();
    let inputs = json!({
        "observation_bytes":observation,
        "expected_specification":observed["specification"],
        "expected_artifact_set":observed["artifact_set"],
        "expected_template_sha256":observed["report"]["chat_template_sha256"],
        "limits_ref":"fixture:explicit-context-and-allowances",
        "context_window_tokens":131072,
        "reserved_output_tokens":4096,
        "reserved_additional_input_tokens":8192
    });
    let current = runtime
        .monty_settings_owner
        .store()
        .effective_view()
        .get("default", "default")
        .await
        .unwrap();
    let limits = UtilityLimits::from_execution(current.execution_limits).unwrap();
    inspect_pure_prefix_draft(runtime, &draft, limits).await;
    let source = format!(
        "{}\nassess_prefix_capacity(inputs)\n",
        document["content"].as_str().unwrap()
    );
    let evaluate = |inputs| UtilityRequest::Evaluate {
        source: source.clone(),
        inputs: BTreeMap::from([("inputs".into(), inputs)]),
        bounds: limits.bounds,
        max_compute_time: Duration::from_secs(5),
    };
    let mut largest = 0;
    for context in [131072, 262144] {
        let mut candidate = inputs.clone();
        candidate["context_window_tokens"] = json!(context);
        let UtilityOutput::Evaluated { value, stdout } = utility::execute(
            &runtime.installed_catalogue.worker,
            evaluate(candidate),
            limits.process,
        )
        .await
        .unwrap() else {
            panic!("capacity calculation must return data")
        };
        assert!(stdout.is_empty());
        largest = value["largest_probe_tokens"].as_u64().unwrap();
        assert!(largest > 131072, "retain the complete large reference");
        assert_eq!(value["required_tokens"], largest + 4096 + 8192);
        assert_eq!(value["fits"], context == 262144);
        assert_eq!(value["cache_qualification"], "unverified");
        assert_eq!(value["model_quality"], "unmeasured");
    }
    assert!(largest < 262144 - 4096 - 8192);
    for (key, value) in [
        ("expected_template_sha256", json!("0".repeat(64))),
        ("expected_artifact_set", json!("0".repeat(64))),
        ("context_window_tokens", json!(true)),
        ("reserved_output_tokens", Value::Null),
        ("reserved_additional_input_tokens", json!(-1)),
    ] {
        let mut invalid = inputs.clone();
        invalid[key] = value;
        assert!(
            utility::execute(
                &runtime.installed_catalogue.worker,
                evaluate(invalid),
                limits.process
            )
            .await
            .is_err(),
            "invalid capacity input {key} must stop"
        );
    }
}

async fn inspect_pure_prefix_draft(
    runtime: &Arc<RebornRuntime>,
    draft: &brassclaw_skills::component_revision::ComponentRevisionDraft,
    limits: brassclaw_monty_host::utility::UtilityLimits,
) {
    use brassclaw_monty_host::utility::{self, UtilityOutput, UtilityRequest};
    brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
        draft.document(),
        draft.dependencies(),
    )
    .unwrap()
    .unwrap();
    brassclaw_skills::value_contract::InputContract::from_value(
        &draft.document()["input_contract"],
        brassclaw_skills::component_revision::REVISION_LIMITS,
    )
    .unwrap();
    brassclaw_skills::value_contract::ValueContract::result_from_value(
        &draft.document()["result_contract"],
        brassclaw_skills::component_revision::REVISION_LIMITS,
    )
    .unwrap();
    let UtilityOutput::Inspected { structure } = utility::execute(
        &runtime.installed_catalogue.worker,
        UtilityRequest::InspectSource {
            source: draft.document()["content"].as_str().unwrap().into(),
            bounds: limits.bounds,
        },
        limits.process,
    )
    .await
    .unwrap() else {
        panic!("pure preload inspection required")
    };
    assert!(structure.direct_host_calls.is_empty());
    assert_eq!(structure.host_value_references, 0);
    assert!(structure.preload.is_some());
}
