//! Original primitive intents, actual handler observations and outer host ACKs.
//! Stored facts remain provisional until the separate real terminal readback.
use super::*;
use crate::bootstrap_evidence::io::require_record;
pub(super) use crate::bootstrap_evidence::io::{
    EvidenceClient, bounded, bounded_for, retain_new_intent, retain_record,
};
use brassclaw_authorization::{
    InstanceToolPolicyError, InstanceToolPolicySource, InstanceToolRule,
};
use brassclaw_host_api::{CapabilityDescriptor, ResourceScope};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, SourceInspectionDispatchObservation, SourceInspectionObservationSink,
};
use brassclaw_turns::run_profile::AgentLoopHostError;
use std::sync::OnceLock;

pub(super) struct EvidenceSubject {
    pub(super) pool: Arc<PgPool>,
    pub(super) storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    pub(super) intent: Uuid,
    pub(super) association: String,
    pub(super) components: Vec<Value>,
}
impl EvidenceSubject {
    fn record(&self, id: Uuid, report: Value) -> String {
        evidence_record(
            id,
            "behavior",
            false,
            &self.association,
            &self.components,
            report,
        )
    }
}
struct OriginalCase {
    scope: ResourceScope,
    arguments: Value,
    bytes: String,
}
pub(super) struct PrimitiveEvidence {
    subject: Arc<EvidenceSubject>,
    case_id: Uuid,
    native_id: Uuid,
    ack_id: Uuid,
    original: OnceLock<OriginalCase>,
    received: OnceLock<String>,
    expects_native: bool,
}
impl PrimitiveEvidence {
    pub(super) fn new(subject: Arc<EvidenceSubject>, case: &str) -> Arc<Self> {
        let case_id = Uuid::new_v5(&subject.intent, case.as_bytes());
        Arc::new(Self {
            subject,
            case_id,
            native_id: Uuid::new_v5(&case_id, b"native-return"),
            ack_id: Uuid::new_v5(&case_id, b"outer-host-ack"),
            original: OnceLock::new(),
            received: OnceLock::new(),
            expects_native: case != "primitive-denial",
        })
    }
    pub(super) async fn prepare(
        &self,
        request: &RuntimeCapabilityRequest,
        profile: Option<&str>,
    ) -> Result<(), RebornBuildError> {
        let bytes=self.subject.record(self.case_id,json!({"format":"inventory-primitive-intent/1","qualification_intent":self.subject.intent,
            "scope":request.context.resource_scope,"invocation_id":request.context.invocation_id,"capability_id":request.capability_id,
            "arguments":request.input,"estimate":request.estimate,"fixture_profile":profile}));
        self.original
            .set(OriginalCase {
                scope: request.context.resource_scope.clone(),
                arguments: request.input.clone(),
                bytes: bytes.clone(),
            })
            .map_err(|_| invalid("primitive original intent already captured"))?;
        retain_new_intent(
            &self.subject.pool,
            self.subject.storage.as_ref(),
            self.case_id,
            &bytes,
        )
        .await
    }
    pub(super) async fn acknowledge(
        &self,
        outcome: &Result<RuntimeCapabilityOutcome, AgentLoopHostError>,
        policy_reads: Value,
    ) -> Result<Value, RebornBuildError> {
        let original = self
            .original
            .get()
            .ok_or_else(|| invalid("primitive original intent missing"))?;
        let native = self.received.get().cloned();
        let result = match outcome {
            Ok(RuntimeCapabilityOutcome::Completed(value)) => {
                json!({"kind":"completed","capability_id":value.capability_id,"output":value.output,"usage":value.usage})
            }
            Ok(RuntimeCapabilityOutcome::Failed(value)) => {
                json!({"kind":"failed","capability_id":value.capability_id,"failure_kind":value.kind.as_str(),"message":value.message})
            }
            Ok(RuntimeCapabilityOutcome::ApprovalRequired(gate)) => {
                json!({"kind":"approval_required","capability_id":gate.capability_id,"approval_request_id":gate.approval_request_id,"reason":blocked_reason(gate.reason)})
            }
            Ok(RuntimeCapabilityOutcome::AuthRequired(gate)) => {
                json!({"kind":"auth_required","capability_id":gate.capability_id,"gate_id":gate.gate_id.as_str(),"boundary":gate.boundary,"reason":blocked_reason(gate.reason)})
            }
            Ok(RuntimeCapabilityOutcome::ResourceBlocked(gate)) => {
                json!({"kind":"resource_blocked","capability_id":gate.capability_id,"gate_id":gate.gate_id.as_str(),"reason":blocked_reason(gate.reason)})
            }
            Ok(RuntimeCapabilityOutcome::SpawnedProcess(process)) => {
                json!({"kind":"spawned_process","capability_id":process.capability_id,"process_id":process.process_id})
            }
            Ok(RuntimeCapabilityOutcome::Unknown(value)) => {
                json!({"kind":"unknown","capability_id":value.capability_id})
            }
            Err(error) => {
                json!({"kind":"host_error","error_kind":error.kind.as_str(),"reason_kind":error.reason_kind.map(|reason|reason.as_str()),"diagnostic_ref":error.diagnostic_ref})
            }
        };
        let bytes=self.subject.record(self.ack_id,json!({"format":"inventory-primitive-host-observation/1",
            "intent_bytes":original.bytes,"intent_checksum":crate::bootstrap_evidence::checksum(&original.bytes),
            "native_observation_bytes":native,"host_outcome":result,"policy_reads":policy_reads}));
        retain_record(
            &self.subject.pool,
            self.subject.storage.as_ref(),
            self.ack_id,
            &bytes,
        )
        .await?;
        if native.is_some() != self.expects_native {
            return Err(invalid(
                "native observation acknowledgement missing or unexpected; qualification unresolved",
            ));
        }
        if let Some(native) = &native {
            require_record(
                &self.subject.pool,
                self.subject.storage.as_ref(),
                self.native_id,
                native,
            )
            .await?;
        }
        Ok(
            json!({"intent_id":self.case_id,"intent_checksum":crate::bootstrap_evidence::checksum(&original.bytes),
            "native_id":native.as_ref().map(|_|self.native_id),"native_checksum":native.as_deref().map(crate::bootstrap_evidence::checksum),
            "ack_id":self.ack_id,"ack_checksum":crate::bootstrap_evidence::checksum(&bytes)}),
        )
    }
}
#[async_trait]
impl SourceInspectionObservationSink for PrimitiveEvidence {
    async fn retain(
        &self,
        observation: &SourceInspectionDispatchObservation,
    ) -> Result<(), FirstPartyCapabilityError> {
        let operation=async {
            let original=self.original.get().ok_or_else(||invalid("primitive observation precedes original intent"))?;
            let facts=strict_json(observation.exact_bytes(),brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS).map_err(invalid)?;
            if facts["scope"]!=serde_json::to_value(&original.scope).map_err(invalid)? || facts["arguments"]!=original.arguments {
                return Err(invalid("native observation differs from original primitive"));
            }
            let bytes=self.subject.record(self.native_id,json!({"format":"inventory-primitive-native-observation/1",
                "intent_checksum":crate::bootstrap_evidence::checksum(&original.bytes),"native_bytes":observation.exact_bytes(),
                "native_checksum":crate::bootstrap_evidence::checksum(observation.exact_bytes())}));
            self.received.set(bytes.clone()).map_err(|_|invalid("native original observation already received"))?;
            retain_record(&self.subject.pool,self.subject.storage.as_ref(),self.native_id,&bytes).await
        }.await;
        operation.map_err(|_| {
            FirstPartyCapabilityError::new(
                brassclaw_host_api::RuntimeDispatchErrorKind::OperationFailed,
            )
        })
    }
}
/// Observe the genuine rule read by the actual authorizer. Only the fixture's
/// own restriction changes enabled; a real instance block never passes denial.
pub(super) struct ObservedPolicy {
    pub(super) live: crate::monty_kernel::PgSelectedToolPolicy,
    pub(super) restrict: bool,
    reads: std::sync::Mutex<Vec<InstanceToolRule>>,
}
impl ObservedPolicy {
    pub(super) fn new(
        live: crate::monty_kernel::PgSelectedToolPolicy,
        restrict: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            live,
            restrict,
            reads: std::sync::Mutex::new(Vec::new()),
        })
    }
    pub(super) fn facts(&self) -> Result<Value, RebornBuildError> {
        let reads = self
            .reads
            .lock()
            .map_err(|_| invalid("primitive policy observation unavailable"))?;
        Ok(json!(reads.iter().map(|rule|json!({"enabled":rule.enabled,"revision":rule.revision,"restricted_by_fixture":self.restrict && rule.enabled})).collect::<Vec<_>>()))
    }
    pub(super) fn genuine_restriction(&self) -> Result<bool, RebornBuildError> {
        let reads = self
            .reads
            .lock()
            .map_err(|_| invalid("primitive policy observation unavailable"))?;
        Ok(self.restrict && !reads.is_empty() && reads.iter().all(|rule| rule.enabled))
    }
}
#[async_trait]
impl InstanceToolPolicySource for ObservedPolicy {
    async fn current_rule(
        &self,
        descriptor: &CapabilityDescriptor,
    ) -> Result<Option<InstanceToolRule>, InstanceToolPolicyError> {
        let rule = self.live.current_rule(descriptor).await?;
        if let Some(rule) = &rule {
            self.reads
                .lock()
                .map_err(|_| InstanceToolPolicyError::Unavailable)?
                .push(rule.clone());
        }
        Ok(rule.map(|mut rule| {
            if self.restrict {
                rule.enabled = false;
            }
            rule
        }))
    }
}

fn blocked_reason(reason: brassclaw_host_runtime::RuntimeBlockedReason) -> &'static str {
    match reason {
        brassclaw_host_runtime::RuntimeBlockedReason::ApprovalRequired => "approval_required",
        brassclaw_host_runtime::RuntimeBlockedReason::AuthRequired => "auth_required",
        brassclaw_host_runtime::RuntimeBlockedReason::ResourceLimit => "resource_limit",
        brassclaw_host_runtime::RuntimeBlockedReason::ResourceUnavailable => "resource_unavailable",
    }
}
