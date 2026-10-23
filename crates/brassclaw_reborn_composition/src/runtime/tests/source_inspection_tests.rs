//! Ordinary startup registration, native identity, current policy and live
//! preparation settings around the actual contained nonexecuting parser.
use super::*;
use brassclaw_authorization::{
    InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot, ToolExecutionRules,
};
use brassclaw_host_api::{
    CapabilitySet, ExecutionContext, ExtensionId, MountView, NetworkPolicy, RuntimeKind, TrustClass,
};
use brassclaw_host_runtime::{
    HostRuntimeError, RuntimeCapabilityOutcome, RuntimeCapabilityRequest, RuntimeFailureKind,
    SOURCE_INSPECTION_CAPABILITY_ID,
};
use brassclaw_product_workflow::MontyVmSettingsStore;
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use serde_json::json;
use sha2::Digest;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_source_inspection_is_registered_nonexecuting_and_uses_live_limits_and_policy() {
    let rig = super::super::test_pg::pg_rig().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let root = tempfile::tempdir().unwrap();
    let runtime = Arc::new(
        Box::pin(build_reborn_runtime(
            RebornRuntimeInput::from_services(
                rig.build_input("inspection-owner", root.path())
                    .with_runtime_policy(local_dev_runtime_policy()),
            )
            .with_model_gateway_override(Arc::new(RecordingGateway {
                reply: "inspection fixture reply".into(),
                requests: Arc::new(StdMutex::new(Vec::new())),
            })),
        ))
        .await
        .unwrap(),
    );
    let vm = runtime.global_monty_owner.client().root_identity();
    let capability = CapabilityId::new(SOURCE_INSPECTION_CAPABILITY_ID).unwrap();
    let kernel = runtime.services.monty_kernel.as_ref().unwrap();
    let native = kernel.builtin(&capability, None).unwrap();
    let identity = native
        .native_identity()
        .expect("actual native registration");
    native.verify_native_artifact().await.unwrap();
    assert_eq!(
        native.descriptor().effects,
        vec![EffectKind::DispatchCapability]
    );
    assert!(native.descriptor().resource_profile.is_none());
    assert_eq!(
        native.native_input_schema().unwrap()["additionalProperties"],
        false
    );
    let tool = uuid::Uuid::new_v4();
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
        RuntimeCapabilityRequest::new(
            ExecutionContext::local_default(
                UserId::new("inspection-owner").unwrap(),
                ExtensionId::new("builtin").unwrap(),
                RuntimeKind::FirstParty,
                TrustClass::FirstParty,
                CapabilitySet::default(),
                MountView::default(),
            )
            .unwrap(),
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
    let source = "raise RuntimeError('source must never execute')\ndef export(inputs):\n    return host.http(url=inputs['url'])\n";
    let RuntimeCapabilityOutcome::Completed(result) = retained
        .invoke(invoke(json!({"source":source})))
        .await
        .unwrap()
    else {
        panic!("actual parser completion required")
    };
    let facts: serde_json::Value = serde_json::from_str(result.output.as_str().unwrap()).unwrap();
    assert_eq!(facts["format"], "python-source-facts/1");
    let inspection_worker = runtime
        .services
        .source_inspection
        .as_ref()
        .unwrap()
        .prepare()
        .await
        .unwrap()
        .worker;
    assert!(Arc::ptr_eq(
        &inspection_worker,
        &runtime.installed_catalogue.worker_image
    ));
    assert!(Arc::ptr_eq(
        &inspection_worker,
        runtime.global_monty_owner.worker_image()
    ));
    assert_eq!(runtime.installed_catalogue.worker, inspection_worker.path());
    assert_ne!(
        runtime.installed_catalogue.worker,
        brassclaw_monty_host::process::installed_worker().unwrap()
    );
    assert_eq!(
        facts["worker_checksum"],
        hex::encode(inspection_worker.checksum())
    );
    assert_eq!(
        facts["facts"]["source_checksum"],
        hex::encode(sha2::Sha256::digest(source.as_bytes()))
    );
    assert_eq!(facts["facts"]["direct_host_calls"][0]["attribute"], "http");
    assert!(facts.get("pass").is_none());
    assert!(facts.get("approved").is_none());
    assert_eq!(result.usage.process_count, 1);
    assert_eq!(
        result.usage.output_bytes,
        serde_json::to_vec(&result.output).unwrap().len() as u64
    );
    // Prepare the actual usage contract against this observed native handler.
    // Retention/inspection remains a constrained draft path, never activation.
    let ids = brassclaw_skills::global_bootstrap_components::UsageComponentIds {
        recipe: uuid::Uuid::new_v4(),
        python_code: uuid::Uuid::new_v4(),
        tool_skill: uuid::Uuid::new_v4(),
        tool,
        skill: uuid::Uuid::new_v4(),
        formatter: None,
    };
    let mut drafts = brassclaw_skills::validator_components::source_inspection_drafts(ids).unwrap();
    drafts.push(brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":tool,"class_code":0,
        "document":{"capability_id":SOURCE_INSPECTION_CAPABILITY_ID,"callable":"host.source_inspection",
            "implementation":{"identity":identity},"input_contract":{"source":{"type":"string","required":true,"checks":[]}}},
        "dependencies":[],"association":null,
    }).to_string()).unwrap());
    let store = brassclaw_skills::revision_store::PgComponentRevisionStore::new(rig.pool.clone());
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
    let prepared =
        brassclaw_engine::memory::retained_tools::prepare_retained_tool_program(selected).unwrap();
    assert_eq!(
        prepared.bindings()["0:2"].capability_id(),
        SOURCE_INSPECTION_CAPABILITY_ID
    );
    let effective = runtime
        .monty_settings_owner
        .store()
        .effective_view()
        .get("default", "default")
        .await
        .unwrap();
    let inspected =
        brassclaw_engine::executor::retained_source::InspectedRetainedProgram::inspect_with_limits(
            brassclaw_engine::executor::retained_recipe::RetainedProgram::Tools(Arc::new(prepared)),
            &runtime.installed_catalogue.worker,
            brassclaw_monty_host::utility::UtilityLimits::from_execution(
                effective.execution_limits,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        inspected.invocation("0:2"),
        Some("result = inspect_python_source(inputs=inputs)")
    );
    let worker_hash = facts["worker_checksum"].clone();
    let inventory = json!({"module Ü":source,"bad":"def private_fragment(:\n"});
    let RuntimeCapabilityOutcome::Completed(result) = retained
        .invoke(invoke(json!({"sources":inventory})))
        .await
        .unwrap()
    else {
        panic!("one real bounded inventory inspection required")
    };
    let report: serde_json::Value = serde_json::from_str(result.output.as_str().unwrap()).unwrap();
    assert_eq!(result.usage.process_count, 1);
    assert_eq!(report["format"], "python-source-inventory/1");
    assert_eq!(report["worker_checksum"], worker_hash);
    assert_eq!(report["sources"].as_object().unwrap().len(), 2);
    assert_eq!(report["sources"]["module Ü"]["kind"], "inspected");
    assert_eq!(
        report["sources"]["module Ü"]["structure"]["source_checksum"],
        facts["facts"]["source_checksum"]
    );
    assert_eq!(report["sources"]["bad"]["kind"], "compile_rejected");
    assert_eq!(report["sources"]["bad"]["category"], "syntax");
    assert!(report.get("pass").is_none());
    assert!(report.get("approved").is_none());
    assert!(!result.output.as_str().unwrap().contains("private_fragment"));
    assert_eq!(
        result.usage.output_bytes,
        serde_json::to_vec(&result.output).unwrap().len() as u64
    );
    for bad in [
        json!({"source":null}),
        json!({"source":source,"worker":"/arbitrary"}),
        json!({"source":"def private_fragment(:"}),
        json!({"sources":null}),
        json!({"sources":{"bad":null}}),
        json!({"sources":{"":"x=1"}}),
        json!({"source":source,"sources":inventory}),
    ] {
        match retained.invoke(invoke(bad)).await {
            Ok(RuntimeCapabilityOutcome::Failed(failure)) => {
                assert_eq!(failure.kind, RuntimeFailureKind::InvalidInput)
            }
            Err(HostRuntimeError::InvalidRequest { .. }) => {}
            _ => panic!("malformed source/input must be rejected as invalid input"),
        }
    }
    policy.publish(1, snapshot(2, false)).unwrap();
    let RuntimeCapabilityOutcome::Failed(failure) = retained
        .invoke(invoke(json!({"source":source})))
        .await
        .unwrap()
    else {
        panic!("disabled source Tool must fail authorization")
    };
    assert_eq!(failure.kind, RuntimeFailureKind::Authorization);
    let RuntimeCapabilityOutcome::Failed(failure) = retained
        .invoke(invoke(json!({"sources":inventory})))
        .await
        .unwrap()
    else {
        panic!("the same current Tool policy covers inventory inspection")
    };
    assert_eq!(failure.kind, RuntimeFailureKind::Authorization);
    policy.publish(2, snapshot(3, true)).unwrap();
    let settings = runtime.monty_settings_owner.store();
    let current = settings.get("default", "default").await.unwrap();
    let mut larger = current.execution_limits;
    larger.max_source_bytes = 2 * 1024 * 1024;
    let request = serde_json::from_value(
        json!({"expected_revision":current.revision,"execution_limits":larger}),
    )
    .unwrap();
    let increased = settings
        .upsert("default", "default", &request)
        .await
        .unwrap();
    let large_source = format!("#{}\n", "a".repeat(1_048_577));
    let RuntimeCapabilityOutcome::Failed(failure) = retained
        .invoke(invoke(
            json!({"sources":{"one":large_source,"two":large_source}}),
        ))
        .await
        .unwrap()
    else {
        panic!("individually fitting sources cannot exceed the total acknowledged ceiling")
    };
    assert_eq!(failure.kind, RuntimeFailureKind::Resource);
    let RuntimeCapabilityOutcome::Completed(result) = retained
        .invoke(invoke(json!({"source":large_source})))
        .await
        .unwrap()
    else {
        panic!("full source above the former shared limit must be inspected")
    };
    let larger_facts: serde_json::Value =
        serde_json::from_str(result.output.as_str().unwrap()).unwrap();
    assert_eq!(larger_facts["preparation_revision"], increased.revision);
    assert_eq!(larger_facts["worker_checksum"], worker_hash);
    assert_eq!(
        larger_facts["facts"]["source_checksum"],
        hex::encode(sha2::Sha256::digest(large_source.as_bytes()))
    );
    let mut smaller = larger;
    smaller.max_source_bytes = 512;
    let unsafe_request = serde_json::from_value(
        json!({"expected_revision":increased.revision,"execution_limits":smaller}),
    )
    .unwrap();
    assert!(
        settings
            .upsert("default", "default", &unsafe_request)
            .await
            .is_err(),
        "a source ceiling smaller than the retained global root cannot be published"
    );
    assert_eq!(
        settings.get("default", "default").await.unwrap().revision,
        increased.revision
    );
    smaller.max_source_bytes = current.execution_limits.max_source_bytes;
    let request = serde_json::from_value(
        json!({"expected_revision":increased.revision,"execution_limits":smaller}),
    )
    .unwrap();
    let reduced = settings
        .upsert("default", "default", &request)
        .await
        .unwrap();
    let RuntimeCapabilityOutcome::Failed(failure) = retained
        .invoke(invoke(json!({"source":large_source})))
        .await
        .unwrap()
    else {
        panic!("oversized source must fail its acknowledged capacity")
    };
    assert_eq!(failure.kind, RuntimeFailureKind::Resource);
    let RuntimeCapabilityOutcome::Completed(result) = retained
        .invoke(invoke(json!({"source":"value = 1"})))
        .await
        .unwrap()
    else {
        panic!("small input remains admitted")
    };
    let smaller_facts: serde_json::Value =
        serde_json::from_str(result.output.as_str().unwrap()).unwrap();
    assert_eq!(smaller_facts["preparation_revision"], reduced.revision);
    let request = serde_json::from_value(
        json!({"expected_revision":reduced.revision,"execution_limits":current.execution_limits}),
    )
    .unwrap();
    let restored = settings
        .upsert("default", "default", &request)
        .await
        .unwrap();
    let mut node_limited = current.execution_limits;
    node_limited.max_value_nodes = 4096;
    let request = serde_json::from_value(
        json!({"expected_revision":restored.revision,"execution_limits":node_limited}),
    )
    .unwrap();
    let node_policy = settings
        .upsert("default", "default", &request)
        .await
        .unwrap();
    let dense_source = "value = 1\n".repeat(10_000);
    let RuntimeCapabilityOutcome::Failed(failure) = retained
        .invoke(invoke(json!({"source":dense_source})))
        .await
        .unwrap()
    else {
        panic!("actual parser observation capacity must reject the whole operation")
    };
    assert_eq!(failure.kind, RuntimeFailureKind::Resource);
    let request = serde_json::from_value(json!({"expected_revision":node_policy.revision,"execution_limits":current.execution_limits})).unwrap();
    settings
        .upsert("default", "default", &request)
        .await
        .unwrap();
    assert_eq!(runtime.global_monty_owner.client().root_identity(), vm);
    shutdown_shared_runtime(runtime).await.unwrap();
}
