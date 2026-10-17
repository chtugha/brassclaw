use brassclaw_host_api::MountPermissions;

use super::*;

#[tokio::test]
async fn local_yolo_policy_mounts_confirmed_host_home_as_host() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage_root = dir.path().join("local-dev");
    let host_home = dir.path().join("home");
    std::fs::create_dir_all(&host_home).expect("host home root");

    let services = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-yolo-host-owner", storage_root)
            .with_runtime_policy(local_yolo_policy())
            .with_local_dev_confirmed_host_home_root(host_home.clone()),
    )
    .await
    .expect("local-dev-yolo services build");
    let local_runtime = services
        .local_runtime
        .as_ref()
        .expect("local-dev runtime substrate");

    let host_mount = local_runtime
        .workspace_mounts
        .mounts
        .iter()
        .find(|mount| mount.alias.as_str() == "/host")
        .expect("host mount exists");
    assert_eq!(host_mount.target.as_str(), "/projects/host");
    assert_eq!(host_mount.permissions, MountPermissions::read_write());

    let raw_host_home_alias = host_home
        .canonicalize()
        .expect("canonical host home")
        .to_string_lossy()
        .into_owned();
    let raw_host_home_mount = local_runtime
        .workspace_mounts
        .mounts
        .iter()
        .find(|mount| mount.alias.as_str() == raw_host_home_alias)
        .expect("raw host home mount exists");
    assert_eq!(raw_host_home_mount.target.as_str(), "/projects/host");
    assert_eq!(
        raw_host_home_mount.permissions,
        MountPermissions::read_write()
    );
}

#[tokio::test]
async fn local_yolo_policy_allows_workspace_under_confirmed_host_home() {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage_root = dir.path().join("local-dev");
    let host_home = dir.path().join("home");
    let workspace_root = host_home.join("repo");
    std::fs::create_dir_all(&workspace_root).expect("workspace root");

    let services = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-yolo-host-owner", storage_root)
            .with_runtime_policy(local_yolo_policy())
            .with_local_dev_workspace_root(workspace_root)
            .with_local_dev_confirmed_host_home_root(host_home),
    )
    .await
    .expect("local-dev-yolo services build");
    let local_runtime = services
        .local_runtime
        .as_ref()
        .expect("local-dev runtime substrate");

    let workspace_mount = local_runtime
        .workspace_mounts
        .mounts
        .iter()
        .find(|mount| mount.alias.as_str() == "/workspace")
        .expect("workspace mount exists");
    assert_eq!(workspace_mount.target.as_str(), "/projects/workspace");
    assert_eq!(workspace_mount.permissions, MountPermissions::read_write());

    let host_mount = local_runtime
        .workspace_mounts
        .mounts
        .iter()
        .find(|mount| mount.alias.as_str() == "/host")
        .expect("host mount exists");
    assert_eq!(host_mount.target.as_str(), "/projects/host");
    assert_eq!(host_mount.permissions, MountPermissions::read_write());
}

#[cfg(unix)]
#[tokio::test]
async fn local_yolo_policy_keeps_symlinked_host_home_raw_alias() {
    let dir = tempfile::tempdir().expect("tempdir"); // safety: test-only setup in #[cfg(test)] module.
    let storage_root = dir.path().join("local-dev");
    let host_home = dir.path().join("home");
    let host_home_link = dir.path().join("home-link");
    std::fs::create_dir_all(&host_home).expect("host home root"); // safety: test-only setup in #[cfg(test)] module.
    std::os::unix::fs::symlink(&host_home, &host_home_link).expect("host home symlink"); // safety: test-only setup in #[cfg(test)] module.

    let services = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-yolo-host-owner", storage_root)
            .with_runtime_policy(local_yolo_policy())
            .with_local_dev_confirmed_host_home_root(host_home_link.clone()),
    )
    .await
    .expect("local-dev-yolo services build"); // safety: test-only assertion in #[cfg(test)] module.
    let local_runtime = services
        .local_runtime
        .as_ref()
        .expect("local-dev runtime substrate"); // safety: test-only assertion in #[cfg(test)] module.

    let raw_aliases = local_runtime
        .workspace_mounts
        .mounts
        .iter()
        .map(|mount| mount.alias.as_str())
        .collect::<Vec<_>>();
    let raw_alias_includes_original =
        raw_aliases.contains(&host_home_link.to_str().expect("utf-8 link path")); // safety: temp paths are test-owned.
    assert!(raw_alias_includes_original); // safety: test-only assertion in #[cfg(test)] module.
    let canonical_host_home = host_home
        .canonicalize()
        .expect("canonical home") // safety: test setup created this path.
        .to_str()
        .expect("utf-8 canonical path") // safety: temp paths are test-owned.
        .to_string();
    let raw_alias_includes_canonical = raw_aliases.contains(&canonical_host_home.as_str());
    assert!(raw_alias_includes_canonical); // safety: test-only assertion in #[cfg(test)] module.
}

#[tokio::test]
async fn local_yolo_policy_requires_confirmed_host_home_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-yolo-host-owner", dir.path().join("local-dev"))
            .with_runtime_policy(local_yolo_policy()),
    )
    .await
    .expect_err("host home policy needs confirmed root");

    assert!(format!("{error}").contains("confirmed host home root"));
}

#[tokio::test]
async fn confirmed_host_home_root_is_rejected_without_matching_policy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let host_home = dir.path().join("home");
    std::fs::create_dir_all(&host_home).expect("host home root");

    let error = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-host-owner", dir.path().join("local-dev"))
            .with_runtime_policy(local_dev_policy())
            .with_local_dev_confirmed_host_home_root(host_home),
    )
    .await
    .expect_err("host home root needs matching policy");

    assert!(format!("{error}").contains("does not allow host home access"));
}

#[tokio::test]
async fn local_yolo_policy_rejects_confirmed_host_home_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let host_home_file = dir.path().join("home-file");
    std::fs::write(&host_home_file, "not a directory").expect("host home file");

    let error = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-yolo-host-owner", dir.path().join("local-dev"))
            .with_runtime_policy(local_yolo_policy())
            .with_local_dev_confirmed_host_home_root(host_home_file),
    )
    .await
    .expect_err("host home root must be a directory");

    assert!(format!("{error}").contains("must be an existing directory"));
}

#[tokio::test]
async fn local_yolo_policy_rejects_confirmed_host_home_filesystem_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = build_reborn_services(
        RebornBuildInput::local_dev("local-dev-yolo-host-owner", dir.path().join("local-dev"))
            .with_runtime_policy(local_yolo_policy())
            .with_local_dev_confirmed_host_home_root(filesystem_root()),
    )
    .await
    .expect_err("host home root must not be a filesystem root");

    assert!(format!("{error}").contains("must not be a filesystem root"));
}

fn local_yolo_policy() -> brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy {
    crate::local_dev_yolo_runtime_policy(true).expect("local-yolo policy resolves") // safety: test-only helper in #[cfg(test)] module.
}

fn local_dev_policy() -> brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy {
    crate::local_dev_runtime_policy().expect("local-dev policy resolves") // safety: test-only helper in #[cfg(test)] module.
}

fn filesystem_root() -> std::path::PathBuf {
    let mut path = std::env::current_dir().expect("current dir"); // safety: test-only helper in #[cfg(test)] module.
    while let Some(parent) = path.parent() {
        path = parent.to_path_buf();
    }
    path
}

#[cfg(all(feature = "postgres", feature = "skills-db"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_startup_registration_obeys_persisted_artifact_capacity() {
    use brassclaw_product_workflow::MontyVmSettingsStore;
    use serde_json::json;
    let rig = crate::runtime::test_pg::pg_rig().await;
    rig.configure_runtime_memory(brassclaw_product_workflow::MontyMemoryMode::Manual)
        .await;
    let home = tempfile::tempdir().unwrap();
    let store = crate::pg_monty_vm_settings::PgMontyVmSettingsStore::new(
        rig.pool.clone(),
        "default",
        "default",
    );
    let before = store.get("default", "default").await.unwrap();
    let mut limits = before.execution_limits;
    limits.max_native_artifact_bytes = 1;
    let update = serde_json::from_value(
        json!({"expected_revision":before.revision,"execution_limits":limits}),
    )
    .unwrap();
    let small = store.upsert("default", "default", &update).await.unwrap();
    let input = || {
        rig.build_input("native-capacity-owner", home.path())
            .with_runtime_policy(local_dev_policy())
    };
    let rejected = Box::pin(build_reborn_services(input())).await;
    assert!(
        matches!(rejected, Err(RebornBuildError::InvalidConfig { ref reason })
        if reason == "native builtin registration failed: native executable exceeds artifact capacity")
    );
    assert_eq!(
        store.get("default", "default").await.unwrap().revision,
        small.revision
    );
    // The actual retained native image comes from the real factory once the
    // operator restores an adequate value. It is not a fabricated registration.
    limits.max_native_artifact_bytes = before.execution_limits.max_native_artifact_bytes;
    // macOS invokes an actual identity verifier. Its captured output must use
    // the same settings snapshot, rather than the constructor's default bound.
    #[cfg(target_os = "macos")]
    let restore_revision = {
        limits.max_stdout_bytes = 1;
        let update = serde_json::from_value(json!({
            "expected_revision":small.revision,"execution_limits":limits,
        }))
        .unwrap();
        let too_small_output = store.upsert("default", "default", &update).await.unwrap();
        assert!(matches!(Box::pin(build_reborn_services(input())).await,
            Err(RebornBuildError::InvalidConfig { ref reason })
            if reason == "native builtin registration failed: native executable verification did not complete"));
        limits.max_stdout_bytes = before.execution_limits.max_stdout_bytes;
        too_small_output.revision
    };
    #[cfg(not(target_os = "macos"))]
    let restore_revision = small.revision;
    let update = serde_json::from_value(
        json!({"expected_revision":restore_revision,"execution_limits":limits}),
    )
    .unwrap();
    let restored = store.upsert("default", "default", &update).await.unwrap();
    let services = Box::pin(build_reborn_services(input())).await.unwrap();
    assert!(!services.readiness.workers.turn_runner);
    assert!(!services.readiness.workers.trigger_poller);
    let selected = services
        .monty_kernel
        .as_ref()
        .unwrap()
        .builtin(
            &brassclaw_host_api::CapabilityId::new(brassclaw_host_runtime::JSON_CAPABILITY_ID)
                .unwrap(),
            None,
        )
        .unwrap();
    let identity = selected
        .native_identity()
        .expect("actual native registration");
    selected.verify_native_artifact().await.unwrap();
    limits.max_native_artifact_bytes = 1;
    let update = serde_json::from_value(
        json!({"expected_revision":restored.revision,"execution_limits":limits}),
    )
    .unwrap();
    let lowered = store.upsert("default", "default", &update).await.unwrap();
    assert!(matches!(Box::pin(build_reborn_services(input())).await,
        Err(RebornBuildError::InvalidConfig { ref reason })
        if reason == "native builtin registration failed: native executable exceeds artifact capacity"));
    assert_eq!(selected.native_identity(), Some(identity));
    selected.verify_native_artifact().await.unwrap();
    // Deliberate persisted corruption exercises a failed source read. Startup
    // must report it before retention, rather than silently using a default.
    let client = rig.pool.get().await.unwrap();
    client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=execution_limits||'{\"unexpected_native_setting\":1}'::jsonb,revision=revision+1 WHERE tenant_id='default' AND user_id='default' AND agent_id='default' AND project_id='default'", &[]).await.unwrap();
    assert!(matches!(Box::pin(build_reborn_services(input())).await,
        Err(RebornBuildError::InvalidConfig { ref reason })
        if reason == "native registration settings are unavailable or invalid"));
    client.execute("UPDATE reborn_monty_vm_settings SET execution_limits=$1,revision=revision+1 WHERE tenant_id='default' AND user_id='default' AND agent_id='default' AND project_id='default'", &[&serde_json::to_value(limits).unwrap()]).await.unwrap();
    assert_eq!(
        store.get("default", "default").await.unwrap().revision,
        lowered.revision + 2
    );
    assert_eq!(selected.native_identity(), Some(identity));
    selected.verify_native_artifact().await.unwrap();
}
