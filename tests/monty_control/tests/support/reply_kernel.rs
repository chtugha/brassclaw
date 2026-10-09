//! Actual transcript and memory implementations behind retained kernel views.
use std::{collections::HashMap, sync::Arc};

use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolRule, LiveStableToolPolicy, StableToolPolicySnapshot,
    ToolExecutionRules,
};
use brassclaw_extensions::{
    CapabilityManifest, CapabilityVisibility, ExtensionManifest, ExtensionPackage,
    ExtensionRegistry, ExtensionRuntime, MANIFEST_SCHEMA_VERSION, ManifestSource,
};
use brassclaw_filesystem::PostgresRootFilesystem;
use brassclaw_host_api::{
    CapabilityId, CapabilityProfileSchemaRef, EffectKind, ExtensionId, MountAlias, MountGrant,
    MountPermissions, MountView, NetworkPolicy, PackageId, PermissionMode, RequestedTrustClass,
    TrustClass, VirtualPath,
};
use brassclaw_host_runtime::{
    CapabilitySurfaceVersion, HostRuntimeServices, RetainedFirstPartyCapability,
    builtin_first_party_handlers, builtin_first_party_package,
};
use brassclaw_reborn::monty_task_host::{MontyTaskHost, POST_REPLY_CAPABILITY_ID};
use brassclaw_resources::InMemoryResourceGovernor;
use brassclaw_trust::{
    AdminConfig, AdminEntry, AuthorityCeiling, EffectiveTrustClass, HostTrustAssignment,
    HostTrustPolicy, TrustDecision, TrustProvenance,
};
use uuid::Uuid;

pub(super) struct Kernel {
    pub handles: HashMap<String, Arc<RetainedFirstPartyCapability>>,
    pub policy: Arc<LiveStableToolPolicy>,
    pub mounts: MountView,
    reply_tool: Uuid,
    memory_tool: Option<Uuid>,
}
impl Kernel {
    pub fn block_memory(&self) {
        assert!(self.memory_tool.is_some());
        self.policy
            .publish(2, self.snapshot(3, false, true))
            .unwrap();
    }
    pub fn block_reply(&self) {
        self.policy
            .publish(2, self.snapshot(3, true, false))
            .unwrap();
    }
    fn snapshot(
        &self,
        revision: u64,
        memory_enabled: bool,
        reply_enabled: bool,
    ) -> StableToolPolicySnapshot {
        let mut tools = HashMap::from([(
            self.reply_tool,
            rule(
                revision,
                reply_enabled,
                vec![EffectKind::ExternalWrite],
                MountView::default(),
            ),
        )]);
        let mut capabilities = HashMap::from([(
            CapabilityId::new(POST_REPLY_CAPABILITY_ID).unwrap(),
            self.reply_tool,
        )]);
        if let Some(memory_tool) = self.memory_tool {
            tools.insert(
                memory_tool,
                rule(
                    revision,
                    memory_enabled,
                    vec![EffectKind::ReadFilesystem, EffectKind::WriteFilesystem],
                    self.mounts.clone(),
                ),
            );
            capabilities.insert(
                CapabilityId::new("builtin.memory_write").unwrap(),
                memory_tool,
            );
        }
        StableToolPolicySnapshot {
            revision,
            tools,
            capabilities,
        }
    }
}
fn rule(
    revision: u64,
    enabled: bool,
    effects: Vec<EffectKind>,
    mounts: MountView,
) -> InstanceToolRule {
    InstanceToolRule {
        enabled,
        revision,
        execution: ToolExecutionRules {
            allowed_effects: effects,
            mounts,
            network: NetworkPolicy::default(),
            secrets: vec![],
            resource_ceiling: None,
        },
    }
}
pub(super) fn trust() -> TrustDecision {
    TrustDecision {
        effective_trust: EffectiveTrustClass::user_trusted(),
        authority_ceiling: AuthorityCeiling {
            allowed_effects: vec![
                EffectKind::ExternalWrite,
                EffectKind::ReadFilesystem,
                EffectKind::WriteFilesystem,
            ],
            max_resource_ceiling: None,
        },
        provenance: TrustProvenance::AdminConfig,
        evaluated_at: chrono::Utc::now(),
    }
}
pub(super) fn kernel(
    host: Arc<MontyTaskHost>,
    reply_tool: Uuid,
    memory_tool: Uuid,
    filesystem: Arc<PostgresRootFilesystem>,
) -> Kernel {
    build_kernel(host, reply_tool, Some(memory_tool), filesystem)
}
fn build_kernel(
    host: Arc<MontyTaskHost>,
    reply_tool: Uuid,
    memory_tool: Option<Uuid>,
    filesystem: Arc<PostgresRootFilesystem>,
) -> Kernel {
    let reply_id = CapabilityId::new(POST_REPLY_CAPABILITY_ID).unwrap();
    let host_package = ExtensionPackage::from_manifest(
        ExtensionManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.into(),
            id: ExtensionId::new("host").unwrap(),
            name: "Admitted transcript host".into(),
            version: "0.1.0".into(),
            description: "Task-bound transcript reply primitive".into(),
            source: ManifestSource::HostBundled,
            requested_trust: RequestedTrustClass::FirstPartyRequested,
            descriptor_trust_default: TrustClass::Sandbox,
            runtime: ExtensionRuntime::FirstParty {
                service: "host".into(),
            },
            host_apis: vec![],
            hooks: vec![],
            capabilities: vec![CapabilityManifest {
                id: reply_id.clone(),
                implements: vec![],
                description: "Publish an answer to this exact admitted task".into(),
                effects: vec![EffectKind::ExternalWrite],
                default_permission: PermissionMode::Allow,
                visibility: CapabilityVisibility::HostInternal,
                input_schema_ref: CapabilityProfileSchemaRef::new(
                    "schemas/post-reply.input.v1.json",
                )
                .unwrap(),
                output_schema_ref: CapabilityProfileSchemaRef::new(
                    "schemas/post-reply.output.v1.json",
                )
                .unwrap(),
                prompt_doc_ref: None,
                required_host_ports: vec![],
                runtime_credentials: vec![],
                resource_profile: None,
            }],
        },
        VirtualPath::new("/system/extensions/host").unwrap(),
    )
    .unwrap();
    let builtin = builtin_first_party_package().unwrap();
    let effects = builtin
        .capabilities
        .iter()
        .flat_map(|d| d.effects.iter().copied())
        .collect();
    let mut registry = ExtensionRegistry::new();
    registry.insert(builtin).unwrap();
    registry.insert(host_package).unwrap();
    let mut handlers = builtin_first_party_handlers(Arc::new(
        brassclaw_triggers::InMemoryTriggerRepository::default(),
    ))
    .unwrap();
    handlers.insert_handler(reply_id.clone(), host.reply_capability());
    let mounts = if memory_tool.is_some() {
        MountView::new(vec![MountGrant::new(
            MountAlias::new("/memory").unwrap(),
            VirtualPath::new("/memory").unwrap(),
            MountPermissions::read_write_list_delete(),
        )])
        .unwrap()
    } else {
        MountView::default()
    };
    let mut kernel = Kernel {
        handles: HashMap::new(),
        policy: Arc::new(
            LiveStableToolPolicy::new(StableToolPolicySnapshot {
                revision: 1,
                tools: HashMap::new(),
                capabilities: HashMap::new(),
            })
            .unwrap(),
        ),
        mounts,
        reply_tool,
        memory_tool,
    };
    kernel
        .policy
        .publish(1, kernel.snapshot(2, true, true))
        .unwrap();
    let trust_policy = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
        AdminEntry::for_local_manifest(
            PackageId::new("builtin").unwrap(),
            "/system/extensions/builtin/manifest.toml".into(),
            None,
            HostTrustAssignment::first_party(),
            effects,
            None,
        ),
        AdminEntry::for_local_manifest(
            PackageId::new("host").unwrap(),
            "/system/extensions/host/manifest.toml".into(),
            None,
            HostTrustAssignment::first_party(),
            vec![EffectKind::ExternalWrite],
            None,
        ),
    ]))])
    .unwrap();
    let runtime = HostRuntimeServices::new(
        Arc::new(registry),
        filesystem,
        Arc::new(InMemoryResourceGovernor::new()),
        Arc::new(InstanceToolAuthorizer::new(kernel.policy.clone())),
        brassclaw_processes::ProcessServices::in_memory(),
        CapabilitySurfaceVersion::new("draft-reply-history-kernel").unwrap(),
    )
    .with_first_party_capabilities(Arc::new(handlers))
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
    let mut retained = vec![reply_id];
    if memory_tool.is_some() {
        retained.push(CapabilityId::new("builtin.memory_write").unwrap());
    }
    for id in retained {
        kernel
            .handles
            .insert(id.to_string(), Arc::new(snapshot.retain(&id).unwrap()));
    }
    kernel
}
