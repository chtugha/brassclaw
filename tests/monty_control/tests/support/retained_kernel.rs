//! Actual retained JSON implementation and live UUID policy through the kernel.
use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
    ToolExecutionRules,
};
use brassclaw_extensions::ExtensionRegistry;
use brassclaw_filesystem::LocalFilesystem;
use brassclaw_host_api::{CapabilityId, EffectKind, MountView, NetworkPolicy, PackageId};
use brassclaw_host_runtime::{
    CapabilitySurfaceVersion, FirstPartyCapabilityRegistry, HostRuntime, HostRuntimeServices,
    builtin_first_party_handlers, builtin_first_party_package,
};
use brassclaw_resources::InMemoryResourceGovernor;
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
pub fn snapshot(revision: u64, enabled: bool, tool: Uuid) -> StableToolPolicySnapshot {
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
pub fn trust() -> TrustDecision {
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

pub fn runtime(tool: Uuid) -> (Arc<dyn HostRuntime>, Arc<LiveStableToolPolicy>) {
    let package = builtin_first_party_package().unwrap();
    let mut registry = ExtensionRegistry::new();
    registry.insert(package).unwrap();
    let registrations = builtin_first_party_handlers(Arc::new(
        brassclaw_triggers::InMemoryTriggerRepository::default(),
    ))
    .unwrap();
    let id = CapabilityId::new("builtin.json").unwrap();
    let selected = registrations.retain_binding(&id).unwrap();
    let retained = FirstPartyCapabilityRegistry::new().with_handler(id, Arc::new(selected));
    // The actual selected handler stays alive after the source registry dies.
    drop(registrations);
    let policy = Arc::new(LiveStableToolPolicy::new(snapshot(1, true, tool)).unwrap());
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
    .with_first_party_capabilities(Arc::new(retained))
    .with_trust_policy(Arc::new(trust_policy))
    .host_runtime_for_local_testing();
    (Arc::new(runtime), policy)
}
