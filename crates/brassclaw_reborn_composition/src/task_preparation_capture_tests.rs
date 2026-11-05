//! Real startup/ACK-live capture plus original-codec corruption cases.
use super::*;
use brassclaw_product_workflow::{MontyMemoryMode, MontyVmSettingsStore};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_original_preparation_capture_survives_acknowledged_changes() {
    let rig = crate::runtime::test_pg::pg_rig().await;
    rig.configure_runtime_memory(MontyMemoryMode::Manual).await;
    let workspace = tempfile::tempdir().unwrap();
    let input = crate::RebornRuntimeInput::from_services(
        rig.build_input("original-preparation-capture", workspace.path())
            .with_runtime_policy(crate::local_dev_runtime_policy().unwrap()),
    );
    let runtime = crate::build_reborn_runtime(input).await.unwrap();
    let root = runtime.global_monty_owner.client().root_identity();
    let startup = runtime.installed_catalogue.initial_preparation.clone();
    assert!(startup.origin() == PreparationOrigin::AcknowledgedStartup);
    assert!(startup.revision() > 0);
    let first = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert!(first.origin() == PreparationOrigin::AcknowledgedLive);
    let settings = runtime.webui_monty_settings_store();
    let before = settings.get("default", "default").await.unwrap();
    assert_eq!(first.revision(), before.revision);
    assert_eq!(first.policy(), before.execution_limits);
    assert_eq!(startup.revision(), first.revision());
    assert_eq!(startup.policy(), first.policy());
    assert_eq!(
        runtime
            .installed_catalogue
            .preparation_policy()
            .await
            .unwrap(),
        first.policy()
    );
    assert_eq!(
        runtime
            .installed_catalogue
            .preparation_limits()
            .await
            .unwrap()
            .bounds
            .max_value_nodes,
        first.limits().bounds.max_value_nodes
    );

    let mut changed_limits = before.execution_limits;
    changed_limits.max_value_nodes = changed_limits.max_value_nodes.checked_add(1).unwrap();
    let update = serde_json::from_value(serde_json::json!({
        "expected_revision": before.revision, "execution_limits": changed_limits,
    }))
    .unwrap();
    let changed = settings
        .upsert("default", "default", &update)
        .await
        .unwrap();
    let second = runtime
        .installed_catalogue
        .capture_task_preparation()
        .await
        .unwrap();
    assert_eq!(second.revision(), changed.revision);
    assert!(second.revision() > first.revision());
    assert_eq!(second.policy(), changed_limits);
    assert_ne!(second.bytes(), first.bytes());
    assert_eq!(runtime.global_monty_owner.client().root_identity(), root);

    // Restore must use the retained original after live uptake, never today's
    // limits or a revision inferred from those limits.
    let restored = PreparationCapture::restore_original(first.bytes(), first.checksum()).unwrap();
    assert_eq!(restored.bytes(), first.bytes());
    assert_eq!(restored.revision(), first.revision());
    assert_eq!(restored.policy(), before.execution_limits);
    assert!(restored.origin() == first.origin());
    assert!(PreparationCapture::restore_original(first.bytes(), second.checksum()).is_err());

    let encode = |wire: &Wire| serde_json::to_string(wire).unwrap();
    let mut wire: Wire = serde_json::from_str(first.bytes()).unwrap();
    wire.revision = 0;
    let invalid = encode(&wire);
    assert!(PreparationCapture::restore_original(&invalid, &checksum(&invalid)).is_err());
    let mut wire: Wire = serde_json::from_str(first.bytes()).unwrap();
    let mut policy: serde_json::Value = serde_json::from_str(&wire.execution_limits_bytes).unwrap();
    policy.as_object_mut().unwrap().remove("max_value_nodes");
    wire.execution_limits_bytes = serde_json::to_string(&policy).unwrap();
    wire.execution_limits_checksum = checksum(&wire.execution_limits_bytes);
    let missing = encode(&wire);
    assert!(PreparationCapture::restore_original(&missing, &checksum(&missing)).is_err());
    let duplicate = first
        .bytes()
        .replacen("\"revision\":", "\"revision\":19,\"revision\":", 1);
    assert!(PreparationCapture::restore_original(&duplicate, &checksum(&duplicate)).is_err());
    let unknown = first.bytes().replacen('{', "{\"approval\":true,", 1);
    assert!(PreparationCapture::restore_original(&unknown, &checksum(&unknown)).is_err());
    let mut wire: Wire = serde_json::from_str(first.bytes()).unwrap();
    wire.execution_limits_bytes =
        wire.execution_limits_bytes
            .replacen('{', "{\"max_value_nodes\":1,", 1);
    wire.execution_limits_checksum = checksum(&wire.execution_limits_bytes);
    let duplicate_limits = encode(&wire);
    assert!(
        PreparationCapture::restore_original(&duplicate_limits, &checksum(&duplicate_limits))
            .is_err()
    );

    drop(settings);
    runtime.shutdown().await.unwrap();
}
