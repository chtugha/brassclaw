//! Original Tool-account sources on the existing catalogue/task/child carriers.
//! Metadata selection and service binding never grant dispatch authority.
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

use async_trait::async_trait;
use brassclaw_auth::{
    AuthProductScope, AuthSurface, CredentialAccountRecordSource, CredentialRequirementOwner,
};
use brassclaw_engine::executor::{
    retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram,
};
use brassclaw_host_api::{
    CapabilityId, CredentialStageError, InvocationId, ResourceScope,
    RuntimeCredentialAccountRequirement, RuntimeCredentialRequirement,
    RuntimeCredentialRequirementSource,
};
use brassclaw_host_runtime::{
    RuntimeCredentialAccessSecret, RuntimeCredentialAccountRequest,
    RuntimeCredentialAccountResolver, RuntimeCredentialUseGuard, RuntimeRawSecretRequest,
    RuntimeRawSecretSource,
};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_secrets::SecretStore;
use brassclaw_turns::run_profile::AgentLoopDriverError;

use crate::{
    monty_kernel::MontyKernelSnapshot,
    product_auth_runtime_credentials::captured::CapturedAccountUsage,
};

pub(crate) struct ToolCredentialSourceOwner {
    accounts: Arc<dyn CredentialAccountRecordSource>,
    secrets: Arc<dyn SecretStore>,
}
impl ToolCredentialSourceOwner {
    pub(crate) fn new(
        accounts: Arc<dyn CredentialAccountRecordSource>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self { accounts, secrets }
    }
}

#[derive(Clone)]
pub(crate) enum CapturedToolCredentials {
    Captured(Arc<ToolCredentialFamily>),
    Fenced,
    Uncaptured,
}

pub(crate) struct ToolCredentialFamily {
    owner: Option<Arc<ToolCredentialSourceOwner>>,
    scope: Option<AuthProductScope>,
    usages: BTreeMap<String, CapturedAccountUsage>,
    raw_usages: BTreeMap<String, BTreeMap<String, CapturedRawUsage>>,
}

struct CapturedRawUsage {
    declarations: Vec<RuntimeCredentialRequirement>,
    source: Option<RuntimeCredentialAccessSecret>,
}
impl CapturedRawUsage {
    async fn capture(
        secrets: &dyn SecretStore,
        scope: &ResourceScope,
        declarations: Vec<RuntimeCredentialRequirement>,
    ) -> Result<Self, AgentLoopDriverError> {
        let handle = declarations
            .first()
            .map(|declaration| &declaration.handle)
            .ok_or_else(failed)?;
        if scope.is_system()
            || declarations.iter().any(|declaration| {
                &declaration.handle != handle
                    || !matches!(
                        declaration.source,
                        RuntimeCredentialRequirementSource::SecretHandle
                    )
            })
        {
            return Err(failed());
        }
        for declaration in &declarations {
            declaration
                .audience
                .validate_declaration()
                .map_err(|_| failed())?;
            declaration
                .target
                .validate_declaration()
                .map_err(|_| failed())?;
        }
        let source = secrets
            .record_reference(scope, handle)
            .await
            .map_err(|_| failed())?
            .map(|record_reference| {
                if record_reference.is_empty() {
                    return Err(failed());
                }
                Ok(RuntimeCredentialAccessSecret {
                    scope: scope.clone(),
                    handle: handle.clone(),
                    record_reference,
                    access_expires_at: None,
                })
            })
            .transpose()?;
        Ok(Self {
            declarations,
            source,
        })
    }
}

pub(crate) struct BoundCredentialServices {
    pub(crate) secrets: Arc<dyn SecretStore>,
    pub(crate) accounts: Arc<dyn RuntimeCredentialAccountResolver>,
    pub(crate) guard: Arc<dyn RuntimeCredentialUseGuard>,
    pub(crate) raw: Arc<dyn RuntimeRawSecretSource>,
}

fn failed() -> AgentLoopDriverError {
    AgentLoopDriverError::Failed {
        reason_kind: "monty_credential_source_unavailable".into(),
    }
}

impl CapturedToolCredentials {
    pub(crate) async fn capture(
        host: &Arc<MontyTaskHost>,
        kernel: &dyn MontyKernelSnapshot,
        owner: Option<Arc<ToolCredentialSourceOwner>>,
        programs: &[Arc<InspectedRetainedProgram>],
    ) -> Result<Self, AgentLoopDriverError> {
        if host.check_dispatch_open().is_err() {
            return Ok(Self::Fenced);
        }
        let capture = async {
            let mut requirements = BTreeMap::<String, RuntimeCredentialAccountRequirement>::new();
            let mut raw_requirements =
                BTreeMap::<(String, String), Vec<RuntimeCredentialRequirement>>::new();
            // Task preparation may contain many usages of one primitive. Check
            // its captured native declaration once, rejecting identity conflicts.
            let mut declarations = BTreeMap::<
                String,
                (
                    brassclaw_host_runtime::NativeImplementationRef,
                    brassclaw_host_api::CapabilityDescriptor,
                ),
            >::new();
            for inspected in programs {
                let RetainedProgram::Tools(program) = inspected.program() else {
                    return Err(failed());
                };
                for binding in program.bindings().values() {
                    // These existing task-specific ports own model/transcript
                    // sources separately; they are not native account Tools.
                    if matches!(
                        binding.capability_id(),
                        "host.post_reply" | "host.kohai_complete"
                    ) {
                        continue;
                    }
                    let capability =
                        CapabilityId::new(binding.capability_id()).map_err(|_| failed())?;
                    let document = program.inputs().instruction().snapshot().revisions()
                        [&binding.tool().uuid]
                        .draft()
                        .document();
                    let expected =
                        serde_json::from_value(document["implementation"]["identity"].clone())
                            .map_err(|_| failed())?;
                    let descriptor = match declarations.entry(binding.capability_id().to_owned()) {
                        std::collections::btree_map::Entry::Occupied(entry) => {
                            if entry.get().0 != expected {
                                return Err(failed());
                            }
                            &entry.into_mut().1
                        }
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            let descriptor = kernel
                                .native_descriptor(&capability, expected)
                                .map_err(|_| failed())?;
                            &entry.insert((expected, descriptor)).1
                        }
                    };
                    for declaration in &descriptor.runtime_credentials {
                        if matches!(
                            &declaration.source,
                            RuntimeCredentialRequirementSource::SecretHandle
                        ) {
                            let key = (
                                binding.capability_id().to_owned(),
                                declaration.handle.as_str().to_owned(),
                            );
                            let usages = raw_requirements.entry(key).or_default();
                            if !usages.contains(declaration) {
                                usages.push(declaration.clone());
                            }
                            continue;
                        }
                        let requirement = RuntimeCredentialAccountRequirement::new(
                            descriptor.provider.clone(),
                            declaration.clone(),
                        )
                        .map_err(|_| failed())?;
                        let reference = requirement.reference().map_err(|_| failed())?;
                        if let Some(previous) = requirements.insert(reference, requirement.clone())
                            && previous != requirement
                        {
                            return Err(failed());
                        }
                    }
                }
            }
            let context = host.work_context();
            let scope = context.actor.as_ref().map(|actor| {
                AuthProductScope::new(
                    ResourceScope {
                        tenant_id: context.tenant_id.clone(),
                        user_id: actor.user_id.clone(),
                        agent_id: context.agent_id.clone(),
                        project_id: context.project_id.clone(),
                        thread_id: host.run_context().map(|chat| chat.thread_id.clone()),
                        // Read-only selection-owner scope; not a native dispatch ID,
                        // account source address, lease, claim or admission proof.
                        invocation_id: InvocationId::new(),
                    },
                    AuthSurface::Api,
                )
            });
            let mut usages = BTreeMap::new();
            let mut raw_usages: BTreeMap<String, BTreeMap<String, CapturedRawUsage>> =
                BTreeMap::new();
            if !requirements.is_empty() || !raw_requirements.is_empty() {
                let owner = owner.as_ref().ok_or_else(failed)?;
                let scope = scope.as_ref().ok_or_else(failed)?;
                for (reference, requirement) in requirements {
                    let usage = CapturedAccountUsage::capture(
                        owner.accounts.clone(),
                        owner.secrets.as_ref(),
                        scope.clone(),
                        requirement,
                    )
                    .await
                    .map_err(|_| failed())?;
                    usages.insert(reference, usage);
                }
                for ((capability, handle), declarations) in raw_requirements {
                    // Capture absence as a completed choice too. Resolution and
                    // invocation preparation never consult current material.
                    let usage = CapturedRawUsage::capture(
                        owner.secrets.as_ref(),
                        &scope.resource,
                        declarations,
                    )
                    .await?;
                    raw_usages
                        .entry(capability)
                        .or_default()
                        .insert(handle, usage);
                }
            }
            Ok(Self::Captured(Arc::new(ToolCredentialFamily {
                owner,
                scope,
                usages,
                raw_usages,
            })))
        };
        // A cancelled task must still bind passive ports and reach the existing
        // global service's acknowledgement, without waiting on source backends.
        tokio::select! {
            biased;
            _ = host.cancellation_requested() => Ok(Self::Fenced),
            result = capture => result,
        }
    }

    pub(crate) fn bind_invocation(
        &self,
        host: &Arc<MontyTaskHost>,
        scope: &ResourceScope,
    ) -> Result<BoundCredentialServices, AgentLoopDriverError> {
        let Self::Captured(family) = self else {
            return Err(failed());
        };
        let original = family.scope.as_ref().ok_or_else(failed)?;
        if !CredentialRequirementOwner::from_scope(&original.resource).matches_scope(scope)
            || !scope_matches_host(host, scope)
        {
            return Err(failed());
        }
        host.check_dispatch_open().map_err(|_| failed())?;
        let owner = family.owner.as_ref().ok_or_else(failed)?;
        let accounts = Arc::new(InvocationAccounts {
            family: family.clone(),
            host: Arc::downgrade(host),
            scope: scope.clone(),
        });
        Ok(BoundCredentialServices {
            secrets: owner.secrets.clone(),
            guard: accounts.clone(),
            raw: accounts.clone(),
            accounts,
        })
    }
}

fn scope_matches_host(host: &MontyTaskHost, scope: &ResourceScope) -> bool {
    let context = host.work_context();
    context
        .actor
        .as_ref()
        .is_some_and(|actor| scope.user_id == actor.user_id)
        && scope.tenant_id == context.tenant_id
        && scope.agent_id == context.agent_id
        && scope.project_id == context.project_id
        && scope.thread_id.as_ref() == host.run_context().map(|chat| &chat.thread_id)
}

struct InvocationAccounts {
    family: Arc<ToolCredentialFamily>,
    host: Weak<MontyTaskHost>,
    scope: ResourceScope,
}

impl InvocationAccounts {
    fn raw_usage(
        &self,
        request: &RuntimeRawSecretRequest<'_>,
    ) -> Result<&CapturedRawUsage, CredentialStageError> {
        let host = self.host.upgrade().ok_or(CredentialStageError::Backend)?;
        if request.scope != &self.scope || !scope_matches_host(&host, request.scope) {
            return Err(CredentialStageError::Backend);
        }
        host.check_dispatch_open()
            .map_err(|_| CredentialStageError::Backend)?;
        let usage = self
            .family
            .raw_usages
            .get(request.capability_id.as_str())
            .and_then(|usages| usages.get(request.handle.as_str()))
            .ok_or(CredentialStageError::Backend)?;
        if usage.declarations.is_empty()
            || usage.declarations.iter().any(|declaration| {
                declaration.handle != *request.handle
                    || !matches!(
                        declaration.source,
                        RuntimeCredentialRequirementSource::SecretHandle
                    )
            })
        {
            return Err(CredentialStageError::Backend);
        }
        Ok(usage)
    }
}

#[async_trait]
impl RuntimeRawSecretSource for InvocationAccounts {
    async fn resolve_source(
        &self,
        request: RuntimeRawSecretRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, CredentialStageError> {
        self.raw_usage(&request)?
            .source
            .clone()
            .ok_or(CredentialStageError::AuthRequired)
    }
    fn check_source_use(
        &self,
        request: &RuntimeRawSecretRequest<'_>,
        source: &RuntimeCredentialAccessSecret,
    ) -> Result<(), CredentialStageError> {
        if self.raw_usage(request)?.source.as_ref() != Some(source) {
            return Err(CredentialStageError::Backend);
        }
        Ok(())
    }
}

impl RuntimeCredentialUseGuard for InvocationAccounts {
    fn check_source_use(
        &self,
        request: &RuntimeCredentialAccountRequest<'_>,
        source: &RuntimeCredentialAccessSecret,
    ) -> Result<(), CredentialStageError> {
        let host = self.host.upgrade().ok_or(CredentialStageError::Backend)?;
        if request.scope != &self.scope || !scope_matches_host(&host, request.scope) {
            return Err(CredentialStageError::Backend);
        }
        let requirement = request.requirement.ok_or(CredentialStageError::Backend)?;
        if !requirement.matches_dispatch(
            &requirement.requirement().handle,
            request.provider,
            request.provider_scopes,
            request.requester_extension,
        ) {
            return Err(CredentialStageError::Backend);
        }
        let reference = requirement
            .reference()
            .map_err(|_| CredentialStageError::Backend)?;
        if !self
            .family
            .usages
            .get(&reference)
            .is_some_and(|usage| usage.owns_source(source))
        {
            return Err(CredentialStageError::Backend);
        }
        host.check_dispatch_open()
            .map_err(|_| CredentialStageError::Backend)
    }
}
impl std::fmt::Debug for InvocationAccounts {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InvocationAccounts")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}
#[async_trait]
impl RuntimeCredentialAccountResolver for InvocationAccounts {
    async fn resolve_access_secret(
        &self,
        request: RuntimeCredentialAccountRequest<'_>,
    ) -> Result<RuntimeCredentialAccessSecret, CredentialStageError> {
        let host = self.host.upgrade().ok_or(CredentialStageError::Backend)?;
        if request.scope != &self.scope || !scope_matches_host(&host, request.scope) {
            return Err(CredentialStageError::Backend);
        }
        host.check_dispatch_open()
            .map_err(|_| CredentialStageError::Backend)?;
        let requirement = request.requirement.ok_or(CredentialStageError::Backend)?;
        let reference = requirement
            .reference()
            .map_err(|_| CredentialStageError::Backend)?;
        let usage = self
            .family
            .usages
            .get(&reference)
            .ok_or(CredentialStageError::Backend)?;
        let original = self
            .family
            .scope
            .as_ref()
            .ok_or(CredentialStageError::Backend)?;
        // A verified child inherits the actual parent's family object. Keep
        // its original logical account scope, while the exact child's native
        // invocation/target scope is checked above. Never widen the global
        // account visibility rule or rewrite a SecretStore lease address.
        // This operation reads metadata only. Cancellation may drop the read
        // without waiting for an unavailable backend or retaining the task host.
        let result = tokio::select! {
            biased;
            _ = host.cancellation_requested() => Err(CredentialStageError::Backend),
            result = usage.resolve(RuntimeCredentialAccountRequest {
                requirement: request.requirement,
                scope: &original.resource,
                provider: request.provider,
                provider_scopes: request.provider_scopes,
                requester_extension: request.requester_extension,
            }) => result,
        };
        host.check_dispatch_open()
            .map_err(|_| CredentialStageError::Backend)?;
        result
    }
}

#[cfg(test)]
#[path = "task_tool_credentials_tests.rs"]
mod tests;
