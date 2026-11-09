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
    #[error("native fixture preparation is unavailable or inconsistent")]
    FixturePreparation,
    #[error("registered source backend has no controlled fixture adapter")]
    FixtureAdapterUnavailable,
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
    builtin_obligations: crate::BuiltinObligationHandler,
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
    /// Trusted host composition of already captured credential service views.
    /// The supplied views must retain/fence the original task/invocation and
    /// selected source generations themselves. Binding captures/executes no
    /// credential, proves no lineage and grants no authority. The immutable
    /// registration, native artifact, policy, staging and substrate stay fixed.
    /// Service placement/access still resolves through the original resolver.
    pub fn with_bound_credential_services(
        &self,
        secret_store: Arc<dyn brassclaw_secrets::SecretStore>,
        accounts: Arc<dyn crate::RuntimeCredentialAccountResolver>,
        guard: Arc<dyn crate::RuntimeCredentialUseGuard>,
    ) -> Self {
        let builtin_obligations =
            self.builtin_obligations
                .retain_credential_views(secret_store.clone(), accounts, guard);
        Self {
            registry: self.registry.clone(),
            registrations: self.registrations.clone(),
            kernel: self
                .kernel
                .clone()
                .with_obligation_handler(Arc::new(builtin_obligations.clone())),
            builtin_obligations,
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: Arc::new(super::retained_credentials::CredentialBoundServices::new(
                self.services.clone(),
                secret_store,
            )),
            policy: self.policy.clone(),
            events: self.events.clone(),
        }
    }

    /// Bind the original raw-handle source/fence without changing registration,
    /// policy, placement or staging. Missing original secret ports stay missing.
    pub fn with_bound_raw_secret_source(
        &self,
        source: Arc<dyn crate::RuntimeRawSecretSource>,
    ) -> Self {
        let builtin_obligations = self.builtin_obligations.retain_raw_secret_source(source);
        Self {
            registry: self.registry.clone(),
            registrations: self.registrations.clone(),
            kernel: self
                .kernel
                .clone()
                .with_obligation_handler(Arc::new(builtin_obligations.clone())),
            builtin_obligations,
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: self.services.clone(),
            policy: self.policy.clone(),
            events: self.events.clone(),
        }
    }

    /// Bind acknowledged task settings without exposing the captured governor
    /// or changing a retained implementation/authority grant.
    pub fn bind_task_budget_settings(
        &self,
        settings: brassclaw_resources::LiveMontyTaskSettings,
    ) -> Result<(), brassclaw_resources::ResourceError> {
        self.governor.bind_task_budget_settings(settings)
    }

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
            builtin_obligations: self.builtin_obligations.clone(),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: self.services.clone(),
            policy: self.policy.clone(),
            events: self.events.clone(),
        };
        bound.retain_native(capability, expected)
    }

    /// Bind a finite native guard fixture through the exact registered handler.
    /// Inputs cannot select this view. It changes neither native registration nor
    /// current authority; the original resolver must still approve placement.
    pub async fn retain_native_inventory_capacity_fixture(
        &self,
        expected: crate::NativeImplementationRef,
        scope: &brassclaw_host_api::ResourceScope,
        sink: Arc<dyn crate::SourceInspectionObservationSink>,
        authorizer: Arc<dyn brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer>,
    ) -> Result<crate::RetainedSourceInspectionFixture, RetainedCapabilityError> {
        let facts = self
            .capture_native_inventory_fixture_preparation(expected)
            .await?;
        let profile = Arc::new(crate::SourceInspectionFixtureProfile::capture(
            facts, expected,
        )?);
        let capability = CapabilityId::new(crate::SOURCE_INSPECTION_CAPABILITY_ID)
            .map_err(|_| RetainedCapabilityError::InconsistentDeclaration)?;
        let bound = Self {
            registry: self.registry.clone(),
            registrations: self.registrations.clone(),
            kernel: self.kernel.retain_authorizer(authorizer),
            builtin_obligations: self.builtin_obligations.clone(),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: Arc::new(crate::source_inspection_fixture::FixtureBoundServices::new(
                self.services.clone(),
                scope.clone(),
                Arc::new(
                    crate::source_inspection_fixture::SourceInspectionInvocationView::new(
                        scope.clone(),
                        profile.arguments().clone(),
                        Some(profile.clone()),
                        sink,
                    ),
                ),
            )),
            policy: self.policy.clone(),
            events: self.events.clone(),
        };
        Ok(crate::RetainedSourceInspectionFixture {
            capability: bound.retain_native(&capability, expected)?,
            profile,
        })
    }
    /// Metadata-only adapter availability on the actual native registration.
    /// No Tool invocation, binding permission or substitute settings source.
    pub async fn capture_native_inventory_fixture_preparation(
        &self,
        expected: crate::NativeImplementationRef,
    ) -> Result<crate::SourceInspectionPreparationFacts, RetainedCapabilityError> {
        let capability = CapabilityId::new(crate::SOURCE_INSPECTION_CAPABILITY_ID)
            .map_err(|_| RetainedCapabilityError::InconsistentDeclaration)?;
        let (_, selected) = self.declared_binding(&capability)?;
        let native = selected
            .native_registration()
            .filter(|native| native.reference() == expected)
            .ok_or(RetainedCapabilityError::NativeIdentity)?;
        native
            .image()
            .verify()
            .await
            .map_err(|_| RetainedCapabilityError::NativeIdentity)?;
        let backend = native
            .source_inspection_preparation()
            .ok_or(RetainedCapabilityError::FixtureAdapterUnavailable)?;
        let facts = backend
            .capture_fixture_preparation()
            .await
            .map_err(|error| {
                if error.kind()
                    == Some(brassclaw_host_api::RuntimeDispatchErrorKind::UnsupportedRunner)
                {
                    RetainedCapabilityError::FixtureAdapterUnavailable
                } else {
                    RetainedCapabilityError::FixturePreparation
                }
            })?;
        Ok(facts)
    }

    /// Observe one normal SourceInspection call through its original native
    /// binding. The sink receives facts before outer ACK/fencing, not permission.
    pub fn retain_native_source_inspection_observed(
        &self,
        expected: crate::NativeImplementationRef,
        scope: &brassclaw_host_api::ResourceScope,
        arguments: serde_json::Value,
        sink: Arc<dyn crate::SourceInspectionObservationSink>,
        authorizer: Arc<dyn brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        let capability = CapabilityId::new(crate::SOURCE_INSPECTION_CAPABILITY_ID)
            .map_err(|_| RetainedCapabilityError::InconsistentDeclaration)?;
        self.native_descriptor(&capability, expected)?;
        let view = Arc::new(
            crate::source_inspection_fixture::SourceInspectionInvocationView::new(
                scope.clone(),
                arguments,
                None,
                sink,
            ),
        );
        let bound = Self {
            registry: self.registry.clone(),
            registrations: self.registrations.clone(),
            kernel: self.kernel.retain_authorizer(authorizer),
            builtin_obligations: self.builtin_obligations.clone(),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: Arc::new(crate::source_inspection_fixture::FixtureBoundServices::new(
                self.services.clone(),
                scope.clone(),
                view,
            )),
            policy: self.policy.clone(),
            events: self.events.clone(),
        };
        bound.retain_native(&capability, expected)
    }

    /// Bind the checked single original operation to the actual registered native
    /// handler and exact invocation scope. Input JSON cannot create this service.
    pub fn retain_native_with_pure_case(
        &self,
        expected: crate::NativeImplementationRef,
        scope: &brassclaw_host_api::ResourceScope,
        operation: Arc<dyn crate::RuntimePureCasePort>,
        authorizer: Arc<dyn brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        let capability = CapabilityId::new(crate::PURE_CASE_CAPABILITY_ID)
            .map_err(|_| RetainedCapabilityError::InconsistentDeclaration)?;
        self.native_descriptor(&capability, expected)?;
        let bound = Self {
            registry: self.registry.clone(),
            registrations: self.registrations.clone(),
            kernel: self.kernel.retain_authorizer(authorizer),
            builtin_obligations: self.builtin_obligations.clone(),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: Arc::new(crate::pure_case_port::PureCaseBoundServices::new(
                self.services.clone(),
                scope.clone(),
                operation,
            )),
            policy: self.policy.clone(),
            events: self.events.clone(),
        };
        bound.retain_native(&capability, expected)
    }

    /// Bind one host-owned child operation to the exact native subagent Tool.
    /// The original handler, declaration, artifact, planner and substrate stay
    /// retained; the supplied authorizer still checks every actual invocation.
    /// This does not start a child, validate its provenance or issue a grant.
    pub fn retain_native_with_child_tasks(
        &self,
        expected: crate::NativeImplementationRef,
        child: Arc<dyn crate::RuntimeChildTaskPort>,
        authorizer: Arc<dyn brassclaw_authorization::TrustAwareCapabilityDispatchAuthorizer>,
    ) -> Result<RetainedFirstPartyCapability, RetainedCapabilityError> {
        let capability = CapabilityId::new(crate::SPAWN_SUBAGENT_CAPABILITY_ID)
            .map_err(|_| RetainedCapabilityError::InconsistentDeclaration)?;
        let bound = Self {
            registry: self.registry.clone(),
            registrations: self.registrations.clone(),
            kernel: self.kernel.retain_authorizer(authorizer),
            builtin_obligations: self.builtin_obligations.clone(),
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: Arc::new(crate::child_task_port::ChildTaskBoundServices::new(
                self.services.clone(),
                capability.clone(),
                child,
            )),
            policy: self.policy.clone(),
            events: self.events.clone(),
        };
        bound.retain_native(&capability, expected)
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
            builtin_obligations: self.builtin_obligations.clone(),
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
        let (descriptor, selected) = self.declared_binding(capability)?;
        let native = selected.native_registration();
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
            descriptor,
            native,
            runtime: Arc::new(
                self.kernel
                    .retain_dispatch(self.registry.clone(), Arc::new(dispatcher)),
            ),
        })
    }

    /// Inspect the exact registered declaration before capturing its credential
    /// sources. Does not construct a consumer runtime or resolve any service.
    pub fn native_descriptor(
        &self,
        capability: &CapabilityId,
        expected: crate::NativeImplementationRef,
    ) -> Result<CapabilityDescriptor, RetainedCapabilityError> {
        let (descriptor, selected) = self.declared_binding(capability)?;
        let native = selected
            .native_registration()
            .ok_or(RetainedCapabilityError::MissingNativeRegistration)?;
        if native.reference() != expected {
            return Err(RetainedCapabilityError::NativeIdentity);
        }
        Ok(descriptor)
    }

    fn declared_binding(
        &self,
        capability: &CapabilityId,
    ) -> Result<(CapabilityDescriptor, crate::RetainedFirstPartyBinding), RetainedCapabilityError>
    {
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
        Ok((descriptor.clone(), selected))
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
        let builtin_obligations = self.builtin_obligation_handler();
        Ok(FirstPartyCapabilitySnapshot {
            registry,
            registrations,
            kernel: self.build_host_runtime_with_obligations(builtin_obligations.clone()),
            builtin_obligations,
            filesystem: self.filesystem.clone(),
            governor: self.governor.clone(),
            services: self.invocation_services_resolver(),
            policy,
            events: self.event_sink.clone(),
        })
    }
}
