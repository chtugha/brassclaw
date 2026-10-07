//! Real builtin runtime, filesystem mount and instance-wide kernel policy.
use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolPolicySnapshot, InstanceToolRule, LiveInstanceToolPolicy,
    ToolExecutionRules,
};
use brassclaw_extensions::ExtensionRegistry;
use brassclaw_filesystem::LocalFilesystem;
use brassclaw_host_api::{
    CapabilityId, CapabilitySet, EffectKind, ExecutionContext, ExtensionId, HostPath, MountAlias,
    MountGrant, MountPermissions, MountView, NetworkPolicy, PackageId, RuntimeKind, TrustClass,
    VirtualPath,
};
use brassclaw_host_runtime::{
    CapabilitySurfacePolicy, CapabilitySurfaceVersion, HostRuntimeServices, SurfaceKind,
    VisibleCapabilityRequest, builtin_first_party_handlers, builtin_first_party_package,
};
use brassclaw_loop_support::{HostRuntimeLoopCapabilityPortFactory, PgCapabilityIo};
use brassclaw_resources::InMemoryResourceGovernor;
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
use brassclaw_turns::run_profile::{
    InMemoryLoopHostMilestoneSink, LoopCapabilityPort, LoopRunContext,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::Arc,
};

pub fn port(
    context: &LoopRunContext,
    root: &Path,
    io: Arc<PgCapabilityIo>,
) -> Arc<dyn LoopCapabilityPort> {
    let package = builtin_first_party_package().unwrap();
    let mut effects = Vec::new();
    for descriptor in &package.capabilities {
        for effect in &descriptor.effects {
            if !effects.contains(effect) {
                effects.push(*effect);
            }
        }
    }
    let mut registry = ExtensionRegistry::new();
    registry.insert(package).unwrap();
    let mut filesystem = LocalFilesystem::new();
    filesystem
        .mount_local(
            VirtualPath::new("/projects/native-fixture").unwrap(),
            HostPath::from_path_buf(root.to_owned()),
        )
        .unwrap();
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/workspace").unwrap(),
        VirtualPath::new("/projects/native-fixture").unwrap(),
        MountPermissions::read_only(),
    )])
    .unwrap();
    let policy = Arc::new(
        LiveInstanceToolPolicy::new(InstanceToolPolicySnapshot {
            revision: 1,
            tools: HashMap::from(
                [("builtin.list_dir", vec![EffectKind::ReadFilesystem])].map(|(name, effects)| {
                    (
                        CapabilityId::new(name).unwrap(),
                        InstanceToolRule {
                            enabled: true,
                            revision: 1,
                            execution: ToolExecutionRules {
                                allowed_effects: effects,
                                mounts: mounts.clone(),
                                network: NetworkPolicy::default(),
                                secrets: vec![],
                                resource_ceiling: None,
                            },
                        },
                    )
                }),
            ),
        })
        .unwrap(),
    );
    let trust_policy = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            PackageId::new("builtin").unwrap(),
            "/system/extensions/builtin/manifest.toml".into(),
            None,
            HostTrustAssignment::first_party(),
            effects.clone(),
            None,
        ),
    ]))])
    .unwrap();
    let runtime = HostRuntimeServices::new(
        Arc::new(registry),
        Arc::new(filesystem),
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(InstanceToolAuthorizer::new(policy)),
        brassclaw_processes::ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("native-global-tools-v1").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(
        builtin_first_party_handlers(Arc::new(
            brassclaw_triggers::InMemoryTriggerRepository::default(),
        ))
        .unwrap(),
    ))
    .with_trust_policy(Arc::new(trust_policy))
    .host_runtime_for_local_testing();
    let mut execution = ExecutionContext::local_default(
        context.actor().unwrap().user_id.clone(),
        brassclaw_loop_support::loop_driver_execution_extension_id(context).unwrap(),
        RuntimeKind::FirstParty,
        TrustClass::UserTrusted,
        CapabilitySet::default(),
        MountView::default(),
    )
    .unwrap();
    execution.tenant_id = context.scope.tenant_id.clone();
    execution.agent_id = context.scope.agent_id.clone();
    execution.project_id = context.scope.project_id.clone();
    execution.thread_id = Some(context.thread_id.clone());
    execution.resource_scope.tenant_id = execution.tenant_id.clone();
    execution.resource_scope.agent_id = execution.agent_id.clone();
    execution.resource_scope.project_id = execution.project_id.clone();
    execution.resource_scope.thread_id = execution.thread_id.clone();
    execution.validate().unwrap();
    let visible = VisibleCapabilityRequest::new(execution, SurfaceKind::new("agent_loop").unwrap())
        .with_policy(CapabilitySurfacePolicy::allow_all())
        .with_provider_trust(BTreeMap::from([(
            ExtensionId::new("builtin").unwrap(),
            TrustDecision {
                effective_trust: EffectiveTrustClass::user_trusted(),
                authority_ceiling: AuthorityCeiling {
                    allowed_effects: effects,
                    max_resource_ceiling: None,
                },
                provenance: TrustProvenance::AdminConfig,
                evaluated_at: chrono::Utc::now(),
            },
        )]));
    HostRuntimeLoopCapabilityPortFactory::new(
        Arc::new(runtime),
        visible,
        io.clone(),
        io,
        Arc::new(InMemoryLoopHostMilestoneSink::default()),
    )
    .with_execution_mounts(mounts)
    .for_run_context(context.clone())
}
