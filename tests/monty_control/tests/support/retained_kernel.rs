//! Actual retained JSON implementation and live UUID policy through the kernel.
use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
    ToolExecutionRules,
};
use brassclaw_extensions::ExtensionRegistry;
use brassclaw_filesystem::LocalFilesystem;
use brassclaw_host_api::{CapabilityId, EffectKind, MountView, NetworkPolicy, PackageId};
use brassclaw_host_runtime::{
    BuiltinFirstPartyTools, CapabilitySurfaceVersion, HostRuntimeServices, RetainedCapabilityError,
    RetainedFirstPartyCapability, builtin_first_party_package, builtin_native_first_party_handlers,
};
use brassclaw_resources::InMemoryResourceGovernor;
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
pub(super) fn snapshot(revision: u64, enabled: bool, tool: Uuid) -> StableToolPolicySnapshot {
    StableToolPolicySnapshot {
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
        capabilities: HashMap::from([(CapabilityId::new("builtin.json").unwrap(), tool)]),
    }
}
pub(super) fn trust() -> TrustDecision {
    TrustDecision {
        effective_trust: EffectiveTrustClass::user_trusted(),
        authority_ceiling: AuthorityCeiling {
            allowed_effects: vec![EffectKind::DispatchCapability],
            max_resource_ceiling: None,
        },
        provenance: TrustProvenance::AdminConfig,
        evaluated_at: chrono::Utc::now(),
    }
}

pub(super) async fn runtime(
    program: &brassclaw_engine::memory::retained_tools::RetainedToolProgram,
    step: &str,
) -> (Arc<RetainedFirstPartyCapability>, Arc<LiveStableToolPolicy>) {
    let tool = program.bindings()[step].tool();
    let definition = program.inputs().instruction().snapshot().revisions()[&tool.uuid]
        .draft()
        .document();
    let expected: brassclaw_host_runtime::NativeImplementationRef =
        serde_json::from_value(definition["native_implementation"].clone()).unwrap();
    let package = builtin_first_party_package().unwrap();
    let mut registry = ExtensionRegistry::new();
    registry.insert(package).unwrap();
    let mut registrations =
        builtin_native_first_party_handlers(BuiltinFirstPartyTools::default(), 1_073_741_824)
            .await
            .unwrap();
    let id = CapabilityId::new("builtin.json").unwrap();
    assert_eq!(registrations.native_identity(&id), Some(expected));
    let wrong_contract = registrations
        .native_identity(&CapabilityId::new("builtin.echo").unwrap())
        .unwrap();
    assert_eq!(
        expected.artifact_checksum(),
        wrong_contract.artifact_checksum()
    );
    assert_ne!(
        expected.contract_checksum(),
        wrong_contract.contract_checksum()
    );
    let original_binding = registrations.retain_binding(&id).unwrap();
    let policy = Arc::new(LiveStableToolPolicy::new(snapshot(1, true, tool.uuid)).unwrap());
    let trust_policy = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            PackageId::new("builtin").unwrap(),
            "/system/extensions/builtin/manifest.toml".into(),
            None,
            HostTrustAssignment::first_party(),
            vec![EffectKind::DispatchCapability],
            None,
        ),
    ]))])
    .unwrap();
    let runtime = HostRuntimeServices::new(
        Arc::new(registry),
        Arc::new(LocalFilesystem::new()),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(InstanceToolAuthorizer::new(policy.clone())),
        brassclaw_processes::ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("retained-draft-kernel").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(registrations.clone()))
    .with_trust_policy(Arc::new(trust_policy))
    .with_runtime_policy(brassclaw_host_api::runtime_policy::EffectiveRuntimePolicy {
        deployment: brassclaw_host_api::runtime_policy::DeploymentMode::LocalSingleUser,
        requested_profile: brassclaw_host_api::runtime_policy::RuntimeProfile::LocalDev,
        resolved_profile: brassclaw_host_api::runtime_policy::RuntimeProfile::LocalDev,
        filesystem_backend:
            brassclaw_host_api::runtime_policy::FilesystemBackendKind::HostWorkspace,
        process_backend: brassclaw_host_api::runtime_policy::ProcessBackendKind::LocalHost,
        network_mode: brassclaw_host_api::runtime_policy::NetworkMode::DirectLogged,
        secret_mode: brassclaw_host_api::runtime_policy::SecretMode::ScrubbedEnv,
        approval_policy: brassclaw_host_api::runtime_policy::ApprovalPolicy::AskDestructive,
        audit_mode: brassclaw_host_api::runtime_policy::AuditMode::LocalMinimal,
    });
    let snapshot = runtime.capture_first_party_capabilities().unwrap();
    assert!(matches!(
        snapshot.retain_native(&id, wrong_contract),
        Err(RetainedCapabilityError::NativeIdentity)
    ));
    let mut forged = serde_json::to_value(expected).unwrap();
    forged["artifact_checksum"] = serde_json::to_value([0u8; 32]).unwrap();
    let forged = serde_json::from_value(forged).unwrap();
    assert!(matches!(
        snapshot.retain_native(&id, forged),
        Err(RetainedCapabilityError::NativeIdentity)
    ));
    let selected = snapshot.retain_native(&id, expected).unwrap();
    assert_eq!(selected.native_identity(), Some(expected));
    assert!(selected.native_input_schema().unwrap().is_object());
    selected.verify_native_artifact().await.unwrap();
    // An arbitrary re-registration cannot inherit a verified artifact stamp,
    // even when it wraps the same old handler. The existing selection survives.
    registrations.insert_handler(id.clone(), Arc::new(original_binding));
    assert!(registrations.native_identity(&id).is_none());
    assert_eq!(selected.native_identity(), Some(expected));
    let runtime = runtime.with_first_party_capabilities(Arc::new(registrations));
    let replaced = runtime.capture_first_party_capabilities().unwrap();
    assert!(matches!(
        replaced.retain_native(&id, expected),
        Err(RetainedCapabilityError::MissingNativeRegistration)
    ));
    drop(replaced);
    // The actual handler and declaration survive source registry replacement.
    runtime
        .shared_extension_registry()
        .remove(&brassclaw_host_api::ExtensionId::new("builtin").unwrap())
        .unwrap();
    drop(runtime);
    drop(snapshot);
    (Arc::new(selected), policy)
}
