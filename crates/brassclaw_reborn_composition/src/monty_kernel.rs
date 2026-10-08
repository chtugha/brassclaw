//! Task-bound registrations through the real captured kernel/substrate.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_authorization::{
    InstanceToolAuthorizer, InstanceToolPolicyError, InstanceToolPolicySource, InstanceToolRule,
    ToolExecutionRules,
};
use brassclaw_extensions::{
    CapabilityManifest, CapabilityVisibility, ExtensionManifest, ExtensionPackage,
    ExtensionRuntime, MANIFEST_SCHEMA_VERSION, ManifestSource,
};
use brassclaw_filesystem::RootFilesystem;
use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, CapabilityProfileSchemaRef, EffectKind, ExtensionId,
    MountView, NetworkPolicy, PackageId, PermissionMode, RequestedTrustClass, TrustClass,
    VirtualPath,
};
use brassclaw_host_runtime::{
    FirstPartyCapabilitySnapshot, HostRuntimeServices, NativeImplementationRef,
    RetainedCapabilityError, RetainedFirstPartyCapability,
};
use brassclaw_pg::PgPool;
use brassclaw_processes::{ProcessResultStore, ProcessStore};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_resources::ResourceGovernor;
use brassclaw_trust::{AdminConfig, AdminEntry, HostTrustAssignment, HostTrustPolicy};
use uuid::Uuid;

use crate::RebornBuildError;

pub(crate) trait MontyKernelSnapshot: Send + Sync {
    fn bind_task_budget_settings(
        &self,
        settings: brassclaw_resources::LiveMontyTaskSettings,
    ) -> Result<(), RebornBuildError>;
    fn builtin(
        &self,
        id: &CapabilityId,
        expected: Option<NativeImplementationRef>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError>;
    fn builtin_with_policy(
        &self,
        id: &CapabilityId,
        expected: NativeImplementationRef,
        policy: Arc<dyn InstanceToolPolicySource>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError>;
    fn reply(
        &self,
        host: Arc<MontyTaskHost>,
        policy: Arc<dyn InstanceToolPolicySource>,
    ) -> Result<RetainedFirstPartyCapability, RebornBuildError>;
}

pub(crate) fn capture<F, G, S, R>(
    services: &HostRuntimeServices<F, G, S, R>,
) -> Result<Arc<dyn MontyKernelSnapshot>, RebornBuildError>
where
    F: RootFilesystem + 'static,
    G: ResourceGovernor + 'static,
    S: ProcessStore + 'static,
    R: ProcessResultStore + 'static,
{
    Ok(Arc::new(
        services
            .capture_first_party_capabilities()
            .map_err(|error| invalid(error.to_string()))?,
    ))
}

fn invalid(reason: impl Into<String>) -> RebornBuildError {
    RebornBuildError::InvalidConfig {
        reason: reason.into(),
    }
}

pub(crate) fn reply_package() -> Result<ExtensionPackage, RebornBuildError> {
    let capability = CapabilityId::new("host.post_reply")?;
    ExtensionPackage::from_manifest(
        ExtensionManifest {
            schema_version: MANIFEST_SCHEMA_VERSION.into(),
            id: ExtensionId::new("host")?,
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
                id: capability,
                implements: vec![],
                description: "Publish an answer to this admitted task".into(),
                effects: vec![EffectKind::ExternalWrite],
                default_permission: PermissionMode::Allow,
                visibility: CapabilityVisibility::HostInternal,
                input_schema_ref: CapabilityProfileSchemaRef::new(
                    "schemas/post-reply.input.v1.json",
                )?,
                output_schema_ref: CapabilityProfileSchemaRef::new(
                    "schemas/post-reply.output.v1.json",
                )?,
                prompt_doc_ref: None,
                required_host_ports: vec![],
                runtime_credentials: vec![],
                resource_profile: None,
            }],
        },
        VirtualPath::new("/system/extensions/host")?,
    )
    .map_err(|error| invalid(error.to_string()))
}

impl<F: RootFilesystem + 'static, G: ResourceGovernor + 'static> MontyKernelSnapshot
    for FirstPartyCapabilitySnapshot<F, G>
{
    fn bind_task_budget_settings(
        &self,
        settings: brassclaw_resources::LiveMontyTaskSettings,
    ) -> Result<(), RebornBuildError> {
        FirstPartyCapabilitySnapshot::bind_task_budget_settings(self, settings)
            .map_err(|error| invalid(error.to_string()))
    }

    fn builtin_with_policy(
        &self,
        id: &CapabilityId,
        expected: NativeImplementationRef,
        policy: Arc<dyn InstanceToolPolicySource>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        self.retain_native_with_authorizer(
            id,
            expected,
            Arc::new(InstanceToolAuthorizer::new(policy)),
        )
    }
    fn builtin(
        &self,
        id: &CapabilityId,
        expected: Option<NativeImplementationRef>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        match expected {
            Some(expected) => self.retain_native(id, expected),
            None => self.retain(id),
        }
    }
    fn reply(
        &self,
        host: Arc<MontyTaskHost>,
        policy: Arc<dyn InstanceToolPolicySource>,
    ) -> Result<RetainedFirstPartyCapability, RebornBuildError> {
        let package = reply_package()?;
        let trust = HostTrustPolicy::new(vec![Box::new(AdminConfig::with_entries(vec![
            AdminEntry::for_local_manifest(
                PackageId::new("host")?,
                "/system/extensions/host/manifest.toml".into(),
                None,
                HostTrustAssignment::first_party(),
                vec![EffectKind::ExternalWrite],
                None,
            ),
        ]))])
        .map_err(|error| invalid(error.to_string()))?;
        self.retain_bound_handler(
            package,
            &CapabilityId::new("host.post_reply")?,
            host.reply_capability(),
            Arc::new(InstanceToolAuthorizer::new(policy)),
            Arc::new(trust),
        )
        .map_err(|error| invalid(error.to_string()))
    }
}

/// Only the exact selected Tool UUID/capability is resolved. Scope identifiers
/// address task data and never become permission keys. The kernel rechecks this
/// current row after obligation preparation before each actual invocation.
pub(crate) struct PgSelectedToolPolicy {
    pub(crate) host: Arc<MontyTaskHost>,
    pub(crate) pool: Arc<PgPool>,
    pub(crate) tool: Uuid,
    pub(crate) capability: CapabilityId,
    pub(crate) effects: Vec<EffectKind>,
    pub(crate) mounts: MountView,
}
#[async_trait]
impl InstanceToolPolicySource for PgSelectedToolPolicy {
    async fn current_rule(
        &self,
        descriptor: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        self.host
            .check_dispatch_open()
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        if descriptor.id != self.capability || descriptor.effects != self.effects {
            return Ok(None);
        }
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        let row = client
            .query_opt(
                "SELECT enabled,revision FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                &[&self.tool],
            )
            .await
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        self.host
            .check_dispatch_open()
            .map_err(|_| InstanceToolPolicyError::Unavailable)?;
        row.map(|row| {
            let revision = u64::try_from(row.get::<_, i64>(1))
                .map_err(|_| InstanceToolPolicyError::Invalid)?;
            Ok(InstanceToolRule {
                enabled: row.get(0),
                revision,
                execution: ToolExecutionRules {
                    allowed_effects: self.effects.clone(),
                    mounts: self.mounts.clone(),
                    network: NetworkPolicy::default(),
                    secrets: vec![],
                    resource_ceiling: None,
                },
            })
        })
        .transpose()
    }
}
