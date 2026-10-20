//! Real registered JSON Tool + acknowledged PostgreSQL settings + one global VM.
use super::*;
use brassclaw_authorization::{
    InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot, ToolExecutionRules,
};
use brassclaw_host_api::{
    CapabilitySet, ExecutionContext, ExtensionId, MountView, NetworkPolicy, RuntimeKind, TrustClass,
};
use brassclaw_host_runtime::{
    HostPrimitiveSettingsSource, JSON_CAPABILITY_ID, RuntimeCapabilityOutcome,
    RuntimeCapabilityRequest, RuntimeFailureKind,
};
use brassclaw_product_workflow::MontyVmSettingsStore;
use brassclaw_trust::{AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustProvenance};
use serde_json::json;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_json_uses_acknowledged_limits_preserves_data_and_obeys_retained_policy() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    let _db = rig.lock_db().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let directory = tempfile::tempdir().unwrap();
    let runtime = Box::pin(build_reborn_runtime(
        RebornRuntimeInput::from_services(
            rig.build_input("json-limit-owner", directory.path())
                .with_runtime_policy(local_dev_runtime_policy()),
        )
        .with_identity(RebornRuntimeIdentity {
            tenant_id: "default".into(),
            ..RebornRuntimeIdentity::reborn_cli()
        }),
    ))
    .await
    .unwrap();
    let root = runtime.global_monty_owner.client().root_identity();
    let lifecycle = crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new();
    assert!(lifecycle.snapshot().await.is_err());
    assert!(
        lifecycle
            .bind(runtime.monty_settings_owner.store().effective_view())
            .await
            .is_err()
    );
    let capability = CapabilityId::new(JSON_CAPABILITY_ID).unwrap();
    let tool = crate::installed_monty_catalogue::id("builtin-json:tool");
    let kernel = runtime.services.monty_kernel.as_ref().unwrap();
    let native = kernel.builtin(&capability, None).unwrap();
    native.verify_native_artifact().await.unwrap();
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
        .builtin_with_policy(
            &capability,
            native.native_identity().unwrap(),
            policy.clone(),
        )
        .unwrap();
    let invoke = |input| {
        RuntimeCapabilityRequest::new(
            ExecutionContext::local_default(
                UserId::new("json-limit-owner").unwrap(),
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
    let settings = runtime.monty_settings_owner.store();
    let original = settings.get("default", "default").await.unwrap();
    lifecycle
        .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
            revision: original.revision,
            limits: original.execution_limits,
        })
        .unwrap();
    assert_eq!(
        lifecycle.snapshot().await.unwrap().revision,
        original.revision
    );
    lifecycle.bind(settings.effective_view()).await.unwrap();
    assert!(lifecycle.bind(settings.effective_view()).await.is_err());
    assert!(
        lifecycle
            .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
                revision: original.revision,
                limits: original.execution_limits
            })
            .is_err()
    );
    let source = runtime.services.primitive_settings.as_ref().unwrap();
    assert_eq!(source.snapshot().await.unwrap().revision, original.revision);
    let mut limits = original.execution_limits;
    limits.max_value_bytes = 4 * 1024 * 1024;
    let changed = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                json!({"expected_revision":original.revision,"execution_limits":limits}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let content = "é\\\"\n".repeat(180_000);
    let expected =
        json!({"user_query":"exact history","chat_history":[{"role":"user","content":content}]});
    let input = json!({"operation":"parse","data":serde_json::to_string(&expected).unwrap()});
    assert!(serde_json::to_vec(&input).unwrap().len() > 1_048_576);
    let RuntimeCapabilityOutcome::Completed(result) =
        retained.invoke(invoke(input.clone())).await.unwrap()
    else {
        panic!("complete Unicode/escaped history must parse");
    };
    assert_eq!(result.output, expected);
    assert_eq!(
        result.usage.output_bytes,
        serde_json::to_vec(&expected).unwrap().len() as u64
    );
    limits.max_value_bytes = 1024 * 1024;
    let reduced = settings
        .upsert(
            "default",
            "default",
            &serde_json::from_value(
                json!({"expected_revision":changed.revision,"execution_limits":limits}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let captured = source.snapshot().await.unwrap();
    assert_eq!(captured.revision, reduced.revision);
    assert_eq!(captured.limits, limits);
    let RuntimeCapabilityOutcome::Failed(failure) =
        retained.invoke(invoke(input.clone())).await.unwrap()
    else {
        panic!("acknowledged reduction must reject later input");
    };
    assert_eq!(failure.kind, RuntimeFailureKind::Resource);
    // A failed durable settings update must not leak into the host or VM view.
    let client = rig.pool.get().await.unwrap();
    client.batch_execute("CREATE FUNCTION reject_json_limit_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.execution_limits->'max_value_bytes' IS DISTINCT FROM OLD.execution_limits->'max_value_bytes' THEN RAISE EXCEPTION 'fixture rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_json_limit_fixture BEFORE UPDATE ON reborn_monty_vm_settings FOR EACH ROW EXECUTE FUNCTION reject_json_limit_fixture()").await.unwrap();
    limits.max_value_bytes = 4 * 1024 * 1024;
    let update = serde_json::from_value(
        json!({"expected_revision":reduced.revision,"execution_limits":limits}),
    )
    .unwrap();
    assert!(
        settings
            .upsert("default", "default", &update)
            .await
            .is_err()
    );
    assert_eq!(source.snapshot().await.unwrap().revision, captured.revision);
    assert_eq!(source.snapshot().await.unwrap().limits, captured.limits);
    let persisted = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        rig.pool.clone(),
        "default",
        "default",
    )
    .get("default", "default")
    .await
    .unwrap();
    assert_eq!(persisted.revision, captured.revision);
    assert_eq!(persisted.execution_limits, captured.limits);
    client.batch_execute("DROP TRIGGER reject_json_limit_fixture ON reborn_monty_vm_settings; DROP FUNCTION reject_json_limit_fixture()").await.unwrap();
    drop(client);
    let raised = settings
        .upsert("default", "default", &update)
        .await
        .unwrap();
    assert_eq!(source.snapshot().await.unwrap().revision, raised.revision);
    let RuntimeCapabilityOutcome::Completed(result) = retained.invoke(invoke(input)).await.unwrap()
    else {
        panic!("acknowledged increase must reach the same retained JSON handler");
    };
    assert_eq!(result.output, expected);
    policy.publish(1, snapshot(2, false)).unwrap();
    let RuntimeCapabilityOutcome::Failed(failure) = retained
        .invoke(invoke(json!({"operation":"parse","data":"true"})))
        .await
        .unwrap()
    else {
        panic!("live policy must still block retained JSON");
    };
    assert_eq!(failure.kind, RuntimeFailureKind::Authorization);
    policy.publish(2, snapshot(3, true)).unwrap();
    assert!(
        matches!(retained.invoke(invoke(json!({"operation":"query","data":{"null":true},"path":"null"}))).await.unwrap(), RuntimeCapabilityOutcome::Completed(result) if result.output == true)
    );
    let restored = settings.upsert("default", "default", &serde_json::from_value(json!({"expected_revision":raised.revision,"execution_limits":original.execution_limits})).unwrap()).await.unwrap();
    assert_eq!(source.snapshot().await.unwrap().revision, restored.revision);
    assert_eq!(runtime.global_monty_owner.client().root_identity(), root);
    // Drop borrows of runtime before consuming its ordinary shutdown owner.
    drop(retained);
    runtime.shutdown().await.unwrap();
}
