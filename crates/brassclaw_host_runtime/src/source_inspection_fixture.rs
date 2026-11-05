//! A fixed native guard fixture through the original registration and substrate.
//! These immutable settings describe a fixture, never effective instance policy,
//! Tool permission, component qualification or task/VM completion.
use crate::{
    InvocationServices, InvocationServicesError, InvocationServicesResolutionRequest,
    InvocationServicesResolver, NativeImplementationRef, NativePrimitiveSettingsCapture,
    RetainedCapabilityError, RetainedFirstPartyCapability,
};
use brassclaw_host_api::{MontyExecutionLimits, ResourceScope};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

/// Mechanical facts obtained from the actual registered backend, not a grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceInspectionPreparationFacts {
    pub capture: NativePrimitiveSettingsCapture,
    pub worker_checksum: [u8; 32],
    pub backend_binding_id: Uuid,
}
const SOURCE: &str = "def source_budget_probe(inputs):\n    return inputs\n";

/// Only the checked native registration constructs this immutable profile.
/// Its instance ID fences live binding; historical completion readers keep the
/// original record and never require a later process to recreate that instance.
pub struct SourceInspectionFixtureProfile {
    base: SourceInspectionPreparationFacts,
    limits: MontyExecutionLimits,
    arguments: Value,
    bytes: String,
    checksum: [u8; 32],
    identity: String,
}
impl SourceInspectionFixtureProfile {
    pub(crate) fn capture(
        base: SourceInspectionPreparationFacts,
        native: NativeImplementationRef,
    ) -> Result<Self, RetainedCapabilityError> {
        let invalid = || RetainedCapabilityError::FixturePreparation;
        if base.capture.settings.revision == 0
            || base.backend_binding_id.is_nil()
            || base.worker_checksum == [0; 32]
        {
            return Err(invalid());
        }
        base.capture
            .settings
            .limits
            .validate()
            .map_err(|_| invalid())?;
        let mut limits = base.capture.settings.limits;
        limits.max_source_bytes = limits.max_source_bytes.min(
            u64::try_from(SOURCE.len().checked_sub(1).ok_or_else(invalid)?)
                .map_err(|_| invalid())?,
        );
        limits.validate().map_err(|_| invalid())?;
        let arguments = Self::package_arguments();
        let base_bytes =
            serde_json::to_string(&base.capture.settings.limits).map_err(|_| invalid())?;
        let limit_bytes = serde_json::to_string(&limits).map_err(|_| invalid())?;
        let argument_bytes = serde_json::to_string(&arguments).map_err(|_| invalid())?;
        let record = json!({"format":"source-inspection-fixture-identity/1","fixture_revision":1,
            "base_origin":base.capture.origin,"base_revision":base.capture.settings.revision,
            "base_limits_bytes":base_bytes,"base_limits_checksum":hex::encode(Sha256::digest(base_bytes.as_bytes())),
            "fixture_limits_bytes":limit_bytes,"fixture_limits_checksum":hex::encode(Sha256::digest(limit_bytes.as_bytes())),
            "fixed_arguments_bytes":argument_bytes,"fixed_arguments_checksum":hex::encode(Sha256::digest(argument_bytes.as_bytes())),
            "worker_checksum":hex::encode(base.worker_checksum),"native_implementation":native,
            "fixture_definition_checksum":hex::encode(Sha256::digest(SOURCE.as_bytes())),
            "owner_code_checksum":hex::encode(native.artifact_checksum()),
            "adapter_code_checksum":hex::encode(Sha256::digest(include_str!("source_inspection_fixture.rs").as_bytes())),
            "backend_binding_id":base.backend_binding_id});
        let material = serde_json::to_string(&record).map_err(|_| invalid())?;
        let identity = hex::encode(Sha256::digest(material.as_bytes()));
        let bytes = serde_json::to_string(
            &json!({"format":"source-inspection-fixture-profile/1","profile_id":identity,
            "identity_material_bytes":material,"identity_material_checksum":identity}),
        )
        .map_err(|_| invalid())?;
        let checksum = Sha256::digest(bytes.as_bytes()).into();
        Ok(Self {
            base,
            limits,
            arguments,
            bytes,
            checksum,
            identity,
        })
    }
    pub fn package_arguments() -> Value {
        json!({"sources":{"valid":"","over_capacity":SOURCE}})
    }
    pub fn exact_bytes(&self) -> &str {
        &self.bytes
    }
    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub fn arguments(&self) -> &Value {
        &self.arguments
    }
    pub fn base(&self) -> SourceInspectionPreparationFacts {
        self.base
    }
    pub fn limits(&self) -> MontyExecutionLimits {
        self.limits
    }
    pub fn source_bytes(&self) -> usize {
        SOURCE.len()
    }
    pub fn guard_reason(&self) -> String {
        format!(
            "source inventory fixture source byte limit exceeded profile {} fixture_revision 1 base_revision {} limit {} actual {}",
            self.identity,
            self.base.capture.settings.revision,
            self.limits.max_source_bytes,
            SOURCE.len()
        )
    }
}
pub struct RetainedSourceInspectionFixture {
    pub(crate) capability: RetainedFirstPartyCapability,
    pub(crate) profile: Arc<SourceInspectionFixtureProfile>,
}
impl RetainedSourceInspectionFixture {
    pub fn capability(&self) -> &RetainedFirstPartyCapability {
        &self.capability
    }
    pub fn profile(&self) -> &SourceInspectionFixtureProfile {
        &self.profile
    }
}

/// Actual native-handler result before outer accounting/host acknowledgement.
/// Retaining it alone establishes no successful primitive or qualification.
pub struct SourceInspectionDispatchObservation {
    bytes: String,
}
impl SourceInspectionDispatchObservation {
    pub fn exact_bytes(&self) -> &str {
        &self.bytes
    }
    pub(crate) fn capture(
        scope: &ResourceScope,
        input: &Value,
        outcome: &Result<crate::FirstPartyCapabilityResult, crate::FirstPartyCapabilityError>,
    ) -> Result<Self, crate::FirstPartyCapabilityError> {
        let result = match outcome {
            Ok(result) => json!({"kind":"returned","output":result.output,"usage":result.usage}),
            Err(error) => {
                json!({"kind":"rejected","dispatch_kind":error.kind().map(|kind|kind.as_str()),"safe_summary":error.safe_summary(),"usage":error.usage()})
            }
        };
        let bytes=serde_json::to_string(&json!({"format":"source-inspection-dispatch-observation/1","scope":scope,"arguments":input,"outcome":result}))
            .map_err(|_|crate::FirstPartyCapabilityError::new(brassclaw_host_api::RuntimeDispatchErrorKind::OperationFailed))?;
        Ok(Self { bytes })
    }
}
#[async_trait::async_trait]
pub trait SourceInspectionObservationSink: Send + Sync {
    /// Retain the received payload before awaiting I/O. A failed/unknown write
    /// must remain unresolved in the qualification owner; it must never replace
    /// the actual native outcome with an invented Tool failure.
    /// Immutable native facts only. An ACK grants neither dispatch nor completion.
    async fn retain(
        &self,
        observation: &SourceInspectionDispatchObservation,
    ) -> Result<(), crate::FirstPartyCapabilityError>;
}
pub(crate) struct SourceInspectionInvocationView {
    scope: ResourceScope,
    arguments: Value,
    profile: Option<Arc<SourceInspectionFixtureProfile>>,
    sink: Arc<dyn SourceInspectionObservationSink>,
}
impl SourceInspectionInvocationView {
    pub(crate) fn new(
        scope: ResourceScope,
        arguments: Value,
        profile: Option<Arc<SourceInspectionFixtureProfile>>,
        sink: Arc<dyn SourceInspectionObservationSink>,
    ) -> Self {
        Self {
            scope,
            arguments,
            profile,
            sink,
        }
    }
    pub(crate) fn profile(&self) -> Option<&SourceInspectionFixtureProfile> {
        self.profile.as_deref()
    }
    pub(crate) fn matches(&self, scope: &ResourceScope, input: &Value) -> bool {
        scope == &self.scope && input == &self.arguments
    }
    pub(crate) async fn observe(
        &self,
        scope: &ResourceScope,
        input: &Value,
        outcome: &Result<crate::FirstPartyCapabilityResult, crate::FirstPartyCapabilityError>,
    ) {
        if !self.matches(scope, input) {
            return;
        }
        if let Ok(observation) = SourceInspectionDispatchObservation::capture(scope, input, outcome)
        {
            // The owner independently requires exact durable readback after
            // the outer host ACK. Its missing ACK cannot erase native success.
            let _acknowledged = self.sink.retain(&observation).await;
        }
    }
}

pub(crate) struct FixtureBoundServices {
    inner: Arc<dyn InvocationServicesResolver>,
    scope: ResourceScope,
    view: Arc<SourceInspectionInvocationView>,
}
impl FixtureBoundServices {
    pub(crate) fn new(
        inner: Arc<dyn InvocationServicesResolver>,
        scope: ResourceScope,
        view: Arc<SourceInspectionInvocationView>,
    ) -> Self {
        Self { inner, scope, view }
    }
}
impl InvocationServicesResolver for FixtureBoundServices {
    fn resolve(
        &self,
        request: InvocationServicesResolutionRequest<'_>,
    ) -> Result<InvocationServices, InvocationServicesError> {
        let selected = request.plan.capability.as_str() == crate::SOURCE_INSPECTION_CAPABILITY_ID
            && request.scope == &self.scope;
        let mut services = self.inner.resolve(request)?;
        if !selected || services.source_inspection_fixture.is_some() {
            return Err(InvocationServicesError::ScopedPreparationMismatch);
        }
        services.source_inspection_fixture = Some(self.view.clone());
        Ok(services)
    }
}
