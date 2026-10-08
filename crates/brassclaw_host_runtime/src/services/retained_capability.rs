//! Actual retained first-party implementation through the ordinary kernel path.
//! Native builtin registrations additionally retain checked image/adapter data;
//! catalogue and exact component-combination approval remain separate.
use super::{
    DefaultHostRuntime, FirstPartyCapabilityRegistry, FirstPartyRuntimeAdapter,
    HostRuntimeServices, InvocationServicesResolver, ProcessResultStore, ProcessStore,
    ResourceGovernor, RootFilesystem,
};
use crate::{HostRuntime, HostRuntimeError, RuntimeCapabilityOutcome, RuntimeCapabilityRequest};
use brassclaw_dispatcher::RuntimeDispatcher;
use brassclaw_events::EventSink;
use brassclaw_extensions::SharedExtensionRegistry;
use brassclaw_host_api::{
    CapabilityDescriptor, CapabilityId, RuntimeKind, runtime_policy::EffectiveRuntimePolicy,
};
use std::sync::Arc;

/// Technical registration failure, never a routing outcome or a Tool grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RetainedCapabilityError {
    #[error("retained capability requires explicit runtime and trust policies")]
    MissingPolicy,
    #[error("first-party implementation registration is missing")]
    MissingRegistration,
    #[error("selected capability is not declared")]
    MissingCapability,
    #[error("selected capability is not a first-party registration")]
    WrongRuntime,
    #[error("selected capability and package declaration disagree")]
    InconsistentDeclaration,
    #[error("selected native executable or adapter contract does not match")]
    NativeIdentity,
    #[error("selected capability has no verified native registration")]
    MissingNativeRegistration,
}

/// One actual host catalogue snapshot plus immutable handler registrations.
/// Fields stay private: upper composition cannot overwrite the captured view,
/// invoke a raw handler, or reach around kernel mediation. This view does not
/// establish component revision/ABI matching, activation or review approval.
pub struct FirstPartyCapabilitySnapshot<F, G>
where
    F: RootFilesystem + 'static,
    G: ResourceGovernor + 'static,
{
    registry: Arc<SharedExtensionRegistry>,
    registrations: Arc<FirstPartyCapabilityRegistry>,
    kernel: DefaultHostRuntime,
    filesystem: Arc<F>,
    governor: Arc<G>,
    services: Arc<dyn InvocationServicesResolver>,
    policy: EffectiveRuntimePolicy,
    events: Option<Arc<dyn EventSink>>,
}

/// A selected real handler and declaration behind the existing kernel facade.
/// Replacement/removal cannot change them. Current authority and technical
/// enforcement remain independent and are checked before each invocation.
#[derive(Clone)]
pub struct RetainedFirstPartyCapability {
    descriptor: CapabilityDescriptor,
    runtime: Arc<DefaultHostRuntime>,
    native: Option<Arc<crate::native_registration::NativeBuiltinRegistration>>,
}
impl RetainedFirstPartyCapability {
    pub fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    pub fn native_identity(&self) -> Option<crate::NativeImplementationRef> {
        self.native.as_ref().map(|entry| entry.reference())
    }

    /// Exact resolved schema registered with the retained builtin, independent
    /// of later catalogue/schema-reference replacement. No approval inference.
    pub fn native_input_schema(&self) -> Option<&serde_json::Value> {
        self.native.as_ref().map(|entry| entry.input_schema())
    }

    pub async fn verify_native_artifact(&self) -> Result<(), crate::NativeImageError> {
        self.native
            .as_ref()
            .ok_or(crate::NativeImageError::Identity)?
            .image()
            .verify()
            .await
    }

    pub async fn invoke(
        &self,
        request: RuntimeCapabilityRequest,
    ) -> Result<RuntimeCapabilityOutcome, HostRuntimeError> {
        if request.capability_id != self.descriptor.id {
            return Err(HostRuntimeError::invalid_request(
                "retained capability identity mismatch",
            ));
        }
        self.runtime.invoke_capability(request).await
    }
}

impl<F, G> FirstPartyCapabilitySnapshot<F, G>
where
    F: RootFilesystem + 'static,
    G: ResourceGovernor + 'static,
{
    /// Host-owned authority composition for a pinned registration. Replaces no
    /// handler, metadata, artifact or substrate service; grants are not issued.
    pub fn retain_native_with_authorizer(
        &self,
        capability: &CapabilityId,
        expected: crate::NativeImplementationRef,
        authorizer: Arc<dyn brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        let bound = Self {
            registry: self.registry.clone(),
            registrations: self.registrations.clone(),
            kernel: self.kernel.retain_authorizer(authorizer),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: self.services.clone(),
            policy: self.policy.clone(),
            events: self.events.clone(),
        };
        bound.retain_native(capability, expected)
    }

    /// Trusted host composition of an admitted task's implementation. The
    /// package and handler are retained together; this neither qualifies native
    /// provenance nor approves a component. The supplied live authorizer and
    /// trust policy mediate the new registration through the original substrate.
    pub fn retain_bound_handler<T: crate::FirstPartyCapabilityHandler + 'static>(
        &self,
        package: brassclaw_extensions::ExtensionPackage,
        capability: &CapabilityId,
        handler: Arc<T>,
        authorizer: Arc<dyn brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer>,
        trust: Arc<dyn brassclaw_trust::TrustPolicy>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        if !package
            .capabilities
            .iter()
            .any(|entry| &entry.id == capability)
            || self
                .registry
                .snapshot()
                .get_capability(capability)
                .is_some()
        {
            return Err(RetainedCapabilityError::InconsistentDeclaration);
        }
        let mut registry = (*self.registry.snapshot()).clone();
        registry
            .insert(package)
            .map_err(|_| RetainedCapabilityError::InconsistentDeclaration)?;
        let registry = Arc::new(SharedExtensionRegistry::new(registry));
        let mut registrations = (*self.registrations).clone();
        registrations.insert_handler(capability.clone(), handler);
        let bound = Self {
            registry: registry.clone(),
            registrations: Arc::new(registrations),
            kernel: self
                .kernel
                .retain_authorizer(authorizer)
                .with_trust_policy_dyn(trust),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: self.services.clone(),
            policy: self.policy.clone(),
            events: self.events.clone(),
        };
        bound.retain(capability)
    }

    pub fn retain(
        &self,
        capability: &CapabilityId,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        let registry = self.registry.snapshot();
        let descriptor = registry
            .get_capability(capability)
            .ok_or(RetainedCapabilityError::MissingCapability)?;
        let package = registry
            .get_extension(&descriptor.provider)
            .ok_or(RetainedCapabilityError::InconsistentDeclaration)?;
        if descriptor.runtime != RuntimeKind::FirstParty
            || package.manifest.runtime_kind() != RuntimeKind::FirstParty
        {
            return Err(RetainedCapabilityError::WrongRuntime);
        }
        if package
            .capabilities
            .iter()
            .find(|entry| entry.id == *capability)
            != Some(descriptor)
        {
            return Err(RetainedCapabilityError::InconsistentDeclaration);
        }
        let selected = self
            .registrations
            .retain_binding(capability)
            .ok_or(RetainedCapabilityError::MissingRegistration)?;
        let native = selected.native_registration();
        if native
            .as_ref()
            .is_some_and(|entry| entry.descriptor() != descriptor)
        {
            return Err(RetainedCapabilityError::NativeIdentity);
        }
        let handlers = Arc::new(
            FirstPartyCapabilityRegistry::new()
                .with_handler(capability.clone(), Arc::new(selected)),
        );
        let mut dispatcher = RuntimeDispatcher::from_shared_registry(
            self.registry.clone(),
            self.filesystem.clone(),
            self.governor.clone(),
        )
        .with_runtime_policy(self.policy.clone())
        .with_runtime_adapter_arc(
            RuntimeKind::FirstParty,
            Arc::new(FirstPartyRuntimeAdapter::from_registry(
                handlers,
                self.services.clone(),
            )),
        );
        if let Some(events) = &self.events {
            dispatcher = dispatcher.with_event_sink_arc(events.clone());
        }
        Ok(RetainedFirstPartyCapability {
            descriptor: descriptor.clone(),
            native,
            runtime: Arc::new(
                self.kernel
                    .retain_dispatch(self.registry.clone(), Arc::new(dispatcher)),
            ),
        })
    }

    /// Select the exact registered native implementation expected by immutable
    /// Tool metadata. This does not read latest or create an approval/grant.
    pub fn retain_native(
        &self,
        capability: &CapabilityId,
        expected: crate::NativeImplementationRef,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        let selected = self.retain(capability)?;
        let actual = selected
            .native_identity()
            .ok_or(RetainedCapabilityError::MissingNativeRegistration)?;
        if actual != expected {
            return Err(RetainedCapabilityError::NativeIdentity);
        }
        Ok(selected)
    }
}

impl<F, G, S, R> HostRuntimeServices<F, G, S, R>
where
    F: RootFilesystem + 'static,
    G: ResourceGovernor + 'static,
    S: ProcessStore + 'static,
    R: ProcessResultStore + 'static,
{
    /// Capture once for task/catalogue preparation, before selecting usages.
    /// Capturing executes nothing and creates no authorization or approval.
    pub fn capture_first_party_capabilities(
        &self,
    ) -> Result<FirstPartyCapabilitySnapshot<F, G>, RetainedCapabilityError> {
        let policy = self
            .runtime_policy
            .clone()
            .filter(|_| self.trust_policy_configured)
            .ok_or(RetainedCapabilityError::MissingPolicy)?;
        let registrations = self
            .first_party_runtime
            .clone()
            .ok_or(RetainedCapabilityError::MissingRegistration)?;
        let registry = Arc::new(SharedExtensionRegistry::from_snapshot(
            self.registry.snapshot(),
        ));
        Ok(FirstPartyCapabilitySnapshot {
            registry,
            registrations,
            kernel: self.build_host_runtime(),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: self.invocation_services_resolver(),
            policy,
            events: self.event_sink.clone(),
        })
    }
}
