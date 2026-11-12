//! Controlled installation qualification of the narrow original-invocation
//! primitive, its usage, independent encoder and private consuming harness.
//! The installed authored-case constructor remains separate from seed fixtures.
use std::sync::Arc;

use crate::{
    bootstrap_evidence::{self as evidence, io},
    bootstrap_validator_approval::{CheckedCaseEncoder, SessionInput},
};
use brassclaw_engine::{
    executor::retained_recipe::RetainedProgram,
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_host_api::CapabilityId;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft, pure_case_components,
    revision_store::PgComponentRevisionStore,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    RebornBuildError, bootstrap_reply_validation::invalid, monty_kernel::MontyKernelSnapshot,
    source_inspection_components::PackagedDefinitions,
};

#[path = "pure_case_seed_fixture.rs"]
mod seed;

#[derive(Clone, Copy, PartialEq, Eq)]
enum HostingState {
    Unknown,
    NotStarted,
    InFlight,
    WorkerJoined,
    ConfirmedClosed,
}
impl HostingState {
    fn diagnostic_name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::NotStarted => "not_started",
            Self::InFlight => "in_flight",
            Self::WorkerJoined => "worker_joined",
            Self::ConfirmedClosed => "confirmed_closed",
        }
    }
}
struct DiagnosticActivity {
    current: &'static str,
    primary_failure: Option<&'static str>,
}
struct QualificationProgress {
    phase: std::sync::Mutex<(&'static str, HostingState)>,
    activity: std::sync::Mutex<DiagnosticActivity>,
    original: std::sync::Mutex<Option<Arc<seed::SeedOperation>>>,
    closed: std::sync::Mutex<Option<Box<crate::bootstrap_recipe_flow::Observation>>>,
    cleanup_pending: std::sync::atomic::AtomicBool,
    cleanup_error: std::sync::Mutex<Option<RebornBuildError>>,
    closed_turn: std::sync::Mutex<Option<brassclaw_turns::TurnRunRecord>>,
    admission_reference: std::sync::Mutex<Option<String>>,
}
impl QualificationProgress {
    fn new() -> Self {
        Self {
            phase: std::sync::Mutex::new(("case_package", HostingState::NotStarted)),
            activity: std::sync::Mutex::new(DiagnosticActivity {
                current: "prepare_package",
                primary_failure: None,
            }),
            original: std::sync::Mutex::new(None),
            closed: std::sync::Mutex::new(None),
            cleanup_pending: std::sync::atomic::AtomicBool::new(false),
            cleanup_error: std::sync::Mutex::new(None),
            closed_turn: std::sync::Mutex::new(None),
            admission_reference: std::sync::Mutex::new(None),
        }
    }
    // Diagnostic-only annotations: they grant no proof, change no gate and
    // never turn an unavailable diagnostic lock into another execution error.
    fn note(&self, activity: &'static str) {
        if let Ok(mut trace) = self.activity.try_lock() {
            trace.current = activity;
        }
    }
    fn note_primary_failure(&self) {
        if let Ok(mut trace) = self.activity.try_lock() {
            trace.primary_failure = Some(trace.current);
        }
    }
    fn checkpoint(&self, phase: &'static str, state: HostingState) -> Result<(), RebornBuildError> {
        *self
            .phase
            .lock()
            .map_err(|_| invalid("case qualification custody unavailable"))? = (phase, state);
        Ok(())
    }
    fn acknowledged_cleanup(
        &self,
        closed: &brassclaw_turns::TurnRunRecord,
    ) -> Result<(), RebornBuildError> {
        *self
            .closed_turn
            .lock()
            .map_err(|_| invalid("case closed Turn custody unavailable"))? = Some(closed.clone());
        self.cleanup_pending
            .store(false, std::sync::atomic::Ordering::Release);
        let (phase, state) = *self
            .phase
            .lock()
            .map_err(|_| invalid("case closure custody unavailable"))?;
        if state == HostingState::WorkerJoined {
            self.checkpoint(phase, HostingState::ConfirmedClosed)?;
        }
        Ok(())
    }
}
/// Optional private support only. The actual cause and current original stay
/// with the catalogue owner; the operator projection exposes phase/kind only.
pub(crate) struct CaseUnavailable {
    phase: &'static str,
    reason: &'static str,
    cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    custody: Arc<QualificationProgress>,
}
impl std::fmt::Debug for CaseUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaseUnavailable")
            .field("phase", &self.phase)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for CaseUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "original Pure case {}: {}", self.phase, self.reason)
    }
}
impl std::error::Error for CaseUnavailable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause.as_deref().map(|e| e as _)
    }
}
impl CaseUnavailable {
    fn diagnostic(&self) -> String {
        let hosting = self
            .custody
            .phase
            .try_lock()
            .map(|guard| guard.1.diagnostic_name())
            .unwrap_or("unavailable");
        let (activity, primary) = self
            .custody
            .activity
            .try_lock()
            .map(|trace| {
                (
                    trace.current,
                    trace.primary_failure.unwrap_or(trace.current),
                )
            })
            .unwrap_or(("unavailable", "unavailable"));
        let cause = self
            .cause
            .as_deref()
            .map(|error| diagnostic_cause(error))
            .unwrap_or_else(|| "not_recorded".into());
        let native = self
            .custody
            .original
            .try_lock()
            .map(|operation| {
                operation
                    .as_ref()
                    .map(|operation| operation.diagnostic())
                    .unwrap_or_else(|| "not_bound".into())
            })
            .unwrap_or_else(|_| "custody_unavailable".into());
        let cleanup = self
            .custody
            .cleanup_error
            .try_lock()
            .map(|error| {
                error
                    .as_ref()
                    .map(|error| diagnostic_cause(error))
                    .unwrap_or_else(|| "not_recorded".into())
            })
            .unwrap_or_else(|_| "custody_unavailable".into());
        let observation = self
            .custody
            .closed
            .try_lock()
            .map(|original| original.is_some());
        let turn = self
            .custody
            .closed_turn
            .try_lock()
            .map(|original| original.is_some());
        format!(
            "phase={} reason={} hosting={hosting} activity={activity} primary_activity={primary} cause={cause} cleanup_pending={} cleanup_cause={cleanup} observation_retained={} closed_turn_retained={} last_native=[{native}]",
            self.phase,
            self.reason,
            self.custody
                .cleanup_pending
                .load(std::sync::atomic::Ordering::Acquire),
            observation
                .map(|value| if value { "yes" } else { "no" })
                .unwrap_or("unknown"),
            turn.map(|value| if value { "yes" } else { "no" })
                .unwrap_or("unknown"),
        )
    }
    pub(crate) fn view(&self) -> brassclaw_product_workflow::OriginalPureCaseReadiness {
        use crate::pure_case_execution::OriginalCaseOperation;
        let native_pending = self
            .custody
            .original
            .lock()
            .map(|original| {
                original
                    .as_ref()
                    .is_some_and(|operation| operation.has_uncommitted_evidence())
            })
            .unwrap_or(true)
            || self
                .custody
                .cleanup_pending
                .load(std::sync::atomic::Ordering::Acquire);
        let cleanup_error = self
            .custody
            .cleanup_error
            .lock()
            .map(|error| error.is_some())
            .unwrap_or(true);
        let admission_present = self
            .custody
            .admission_reference
            .lock()
            .map(|reference| reference.is_some())
            .unwrap_or(true);
        let closed_turn_present = self
            .custody
            .closed_turn
            .lock()
            .map(|original| original.is_some())
            .unwrap_or(false);
        let evidence_pending =
            native_pending || cleanup_error || (admission_present && !closed_turn_present);
        brassclaw_product_workflow::OriginalPureCaseReadiness {
            ready: false,
            evidence_pending,
            phase: Some(self.phase.into()),
            reason_kind: Some(self.reason.into()),
        }
    }
}
fn failure(
    phase: &'static str,
    reason: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
    custody: Arc<QualificationProgress>,
) -> CaseUnavailable {
    CaseUnavailable {
        phase,
        reason,
        cause: Some(Box::new(cause)),
        custody,
    }
}
fn fatal(error: CaseUnavailable) -> RebornBuildError {
    RebornBuildError::OriginalPureCaseQualification {
        diagnostic: error.diagnostic(),
        source: Box::new(error),
    }
}

/// A closed redaction vocabulary for error reporting only. String matching
/// here selects no routing, approval, retry, cleanup or containment decision.
/// Unknown/private InvalidConfig text stays in the original cause, not output.
fn diagnostic_cause(error: &(dyn std::error::Error + 'static)) -> String {
    if let Some(error) = error.downcast_ref::<RebornBuildError>() {
        return match error {
            RebornBuildError::OriginalPureEncoderQualification { diagnostic, .. } => {
                format!("independent_encoder/{diagnostic}")
            }
            RebornBuildError::InvalidConfig { reason } => {
                for phase in ["admission", "completion", "settlement", "shutdown"] {
                    for kind in [
                        "InvalidInput",
                        "InvalidLimits",
                        "InvalidPortResult",
                        "Backpressure",
                        "Closed",
                        "Transport",
                        "Protocol",
                        "Deadline",
                        "Join",
                        "SettingsConflict",
                        "UnsafeHeapReduction",
                    ] {
                        if reason == &format!("bootstrap Recipe {phase}: {kind}") {
                            return format!("contained_{phase}/{kind}");
                        }
                    }
                }
                match reason.as_str() {
                    "model-free contained receipt or quiescence incomplete" => {
                        "quiescence_incomplete"
                    }
                    "contained expected-stop lifecycle incomplete" => "expected_stop_incomplete",
                    "contained transport unresolved" => "transport_unresolved",
                    "contained worker has not been reaped" => "worker_not_reaped",
                    "contained worker closure unacknowledged" => "worker_close_unacknowledged",
                    "contained native worker failed" => "worker_failed",
                    _ => "invalid_config_private",
                }
                .into()
            }
            RebornBuildError::OriginalPureCaseQualification { .. } => {
                "nested_case_qualification".into()
            }
            _ => "typed_build_error".into(),
        };
    }
    if let Some(error) = error.downcast_ref::<brassclaw_host_runtime::RetainedCapabilityError>() {
        // This is a closed Copy enum, never a provider/source/SQL diagnostic.
        return format!("native_registration/{error:?}");
    }
    "private_typed_cause".into()
}

pub(crate) struct PackagedCaseGraph {
    pub(crate) program: Arc<RetainedToolProgram>,
    definitions: PackagedDefinitions,
    qualification: Arc<RetainedToolProgram>,
}

async fn retain(
    pool: Arc<PgPool>,
    identity: brassclaw_host_runtime::NativeImplementationRef,
    tenant: &str,
) -> Result<PackagedCaseGraph, RebornBuildError> {
    let capability =
        CapabilityId::new(brassclaw_host_runtime::PURE_CASE_CAPABILITY_ID).map_err(invalid)?;
    let tool = crate::builtin_bootstrap::seed_pure_case_tool(pool.clone(), tenant)
        .await
        .map_err(invalid)?;
    if tool != pure_case_components::usage_ids().tool {
        return Err(invalid("original Pure Tool identity differs"));
    }
    let draft=ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":tool,
        "class_code":0,"document":{"capability_id":capability.as_str(),"callable":"host.invoke_original_pure_case",
            "implementation":{"identity":identity},"input_contract":{}},"dependencies":[],"association":null}).to_string())
        .map_err(invalid)?;
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut definitions = PackagedDefinitions::default();
    definitions.retain(&store, &draft).await?;
    for draft in pure_case_components::drafts().map_err(invalid)? {
        definitions.retain(&store, &draft).await?;
    }
    pool.get()
        .await
        .map_err(invalid)?
        .execute(
            "INSERT INTO brassclaw_instance_tool_settings(tool_id)
        VALUES($1) ON CONFLICT(tool_id) DO NOTHING",
            &[&tool],
        )
        .await
        .map_err(invalid)?;
    let qualification_recipe = pure_case_components::qualification_recipe().map_err(invalid)?;
    definitions.retain(&store, &qualification_recipe).await?;
    let qualification_snapshot = definitions
        .snapshot(pool.clone(), qualification_recipe.uuid())
        .await?;
    let qualification = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                qualification_snapshot,
                qualification_recipe.uuid(),
                "selected",
                WorkflowClass::Deterministic,
            )
            .map_err(invalid)?,
        )
        .map_err(invalid)?,
    );
    let snapshot = definitions
        .snapshot(pool, pure_case_components::usage_ids().recipe)
        .await?;
    let program = Arc::new(
        prepare_retained_tool_program(
            compile_retained_recipe(
                snapshot,
                pure_case_components::usage_ids().recipe,
                "selected",
                WorkflowClass::Deterministic,
            )
            .map_err(invalid)?,
        )
        .map_err(invalid)?,
    );
    Ok(PackagedCaseGraph {
        program,
        definitions,
        qualification,
    })
}

/// The capability remains private; no public active catalogue is manufactured.
/// Only this installation owner's completed exact proof can construct the view.
pub(crate) struct QualifiedCaseHarness {
    program: Arc<RetainedToolProgram>,
    proof_id: uuid::Uuid,
    proof_bytes: String,
    context: Arc<Context>,
    encoder: CheckedCaseEncoder,
    prerequisites: Vec<(Uuid, String)>,
}
impl QualifiedCaseHarness {
    pub(crate) fn program(&self) -> Arc<RetainedToolProgram> {
        self.program.clone()
    }
    pub(crate) async fn require(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        let actual = crate::bootstrap_evidence::read_evidence(tx, self.proof_id)
            .await?
            .ok_or_else(|| invalid("original Pure harness qualification missing"))?;
        if actual != self.proof_bytes {
            return Err(invalid("original Pure harness qualification differs"));
        }
        for (id, bytes) in &self.prerequisites {
            if evidence::read_evidence(tx, *id).await?.as_deref() != Some(bytes.as_str()) {
                return Err(invalid("original Pure harness prerequisite differs"));
            }
        }
        let binding = self
            .prerequisites
            .iter()
            .find_map(|(_, bytes)| {
                serde_json::from_str::<Value>(bytes).ok().filter(|value| {
                    value["report"]["format"] == "original-pure-tool-skill-qualification/1"
                })
            })
            .ok_or_else(|| invalid("original Pure binding qualification absent"))?;
        if binding["report"]["argument_checks"] != self.context.binding_checks()? {
            return Err(invalid("original Pure binding negative checks differ"));
        }
        let encoder = self.encoder.verify(tx, &self.program).await?;
        let final_record: Value = serde_json::from_str(&self.proof_bytes).map_err(invalid)?;
        if final_record["report"]["encoder"] != serde_json::to_value(encoder).map_err(invalid)? {
            return Err(invalid("original Pure harness encoder proof differs"));
        }
        for spec in specs() {
            let bytes =
                evidence::read_evidence(tx, self.context.id(&format!("{}:complete", spec.name)))
                    .await?
                    .ok_or_else(|| invalid("original Pure harness stage missing"))?;
            self.context
                .verify_stage_bytes_in_transaction(tx, &spec, &bytes)
                .await?;
            let expected =
                brassclaw_skills::review_submission_store::selection::SourceProofReference {
                    id: self.context.id(&format!("{}:complete", spec.name)),
                    checksum: evidence::checksum(&bytes),
                };
            if !final_record["report"]["completed"]
                .as_array()
                .is_some_and(|refs| refs.contains(&json!(expected)))
            {
                return Err(invalid(
                    "original Pure harness final stage reference differs",
                ));
            }
        }
        // Exact retained graph selection remains immutable for this private use.
        for revision in self
            .program
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .values()
        {
            PgComponentRevisionStore::read_revision_in_transaction(tx, revision.reference())
                .await
                .map_err(invalid)?;
        }
        Ok(())
    }
}

struct Spec {
    name: &'static str,
    export: &'static str,
    expected: Value,
    raw: bool,
}
fn specs() -> Vec<Spec> {
    let exception = |class, message| {
        json!({"kind":"python_exception","exception":{
        "format":"native-python-exception/1","class":class,"message":message}})
    };
    vec![
        Spec {
            name: "primitive-return",
            export: "native_case_return",
            expected: json!({"kind":"return","value":"closed original Ü"}),
            raw: true,
        },
        Spec {
            name: "primitive-exception",
            export: "native_case_exception",
            expected: exception("ValueError", "closed original exception"),
            raw: true,
        },
        Spec {
            name: "primitive-unexpected",
            export: "native_case_unexpected",
            expected: exception("TypeError", "closed unexpected exception"),
            raw: true,
        },
        Spec {
            name: "primitive-output-contract",
            export: "native_case_output_defect",
            expected: json!({"kind":"result_contract_violation","value":7}),
            raw: true,
        },
        Spec {
            name: "skill-code-function",
            export: "native_case_return",
            expected: json!({"kind":"return","value":"closed original Ü"}),
            raw: true,
        },
        Spec {
            name: "private-harness-exception",
            export: "native_case_exception",
            expected: exception("ValueError", "closed original exception"),
            raw: false,
        },
    ]
}

struct Context {
    progress: Arc<QualificationProgress>,
    base: SessionInput,
    kernel: Arc<dyn MontyKernelSnapshot>,
    graph: PackagedCaseGraph,
    instrument: Arc<brassclaw_engine::executor::retained_source::InspectedPureSelection>,
    manifest: Value,
    manifest_checksum: String,
    operation: Uuid,
    components: Vec<Value>,
    association: String,
}
impl Context {
    fn id(&self, name: &str) -> Uuid {
        Uuid::new_v5(&self.operation, name.as_bytes())
    }
    fn record(&self, id: Uuid, kind: &str, complete: bool, report: Value) -> String {
        evidence::system_record(
            id,
            kind,
            complete,
            &self.association,
            &self.components,
            report,
        )
    }
    fn program(&self, spec: &Spec) -> Arc<RetainedToolProgram> {
        if spec.raw {
            self.graph.qualification.clone()
        } else {
            self.graph.program.clone()
        }
    }
    fn binding_checks(&self) -> Result<Value, RebornBuildError> {
        let binding = &self.graph.program.bindings()["0:2"];
        let bounds = self.base.preparation.limits().bounds;
        let limits = brassclaw_skills::value_contract::ContractLimits {
            max_depth: bounds.max_value_depth,
            max_nodes: bounds.max_value_nodes,
            max_bytes: bounds.max_value_bytes,
        };
        let empty = binding
            .bind_tool_arguments_with_limits(&json!({}), &json!({}), limits)
            .map_err(invalid)?;
        if empty != json!({}) {
            return Err(invalid("original Pure binding changed empty arguments"));
        }
        let mut checks = Vec::new();
        for (name, arguments) in [
            ("null", Value::Null),
            ("array", json!([])),
            ("source", json!({"source":"caller code"})),
            ("export", json!({"export":"caller export","inputs":{}})),
            ("authority", json!({"authorized":true})),
        ] {
            if binding
                .bind_tool_arguments_with_limits(&json!({}), &arguments, limits)
                .is_ok()
            {
                return Err(invalid(
                    "original Pure binding accepted a forbidden caller argument",
                ));
            }
            checks.push(json!({"case":name,"arguments":arguments,"rejected":true}));
        }
        Ok(json!({"accepted_empty":empty,"rejected":checks,"limits":{
            "max_depth":limits.max_depth,"max_nodes":limits.max_nodes,"max_bytes":limits.max_bytes}}))
    }
    fn stage_intent(&self, spec: &Spec) -> Result<String, RebornBuildError> {
        let id = self.id(&format!("{}:intent", spec.name));
        Ok(self.record(id,"behavior",false,json!({"format":"original-pure-seed-stage-intent/1",
            "operation":self.operation,"manifest_checksum":self.manifest_checksum,"stage":spec.name,
            "export":spec.export,"expected":spec.expected,"raw":spec.raw,
            "selection_bytes":self.program(spec).inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes()})))
    }
    async fn run_stage(&self, spec: &Spec) -> Result<(), RebornBuildError> {
        self.progress.note("read_stage_completion");
        self.progress.checkpoint(spec.name, HostingState::Unknown)?;
        let id = self.id(&format!("{}:intent", spec.name));
        let done = self.id(&format!("{}:complete", spec.name));
        if io::read_record(&self.base.pool, self.base.storage.as_ref(), done)
            .await?
            .is_some()
        {
            self.progress.note("verify_existing_stage");
            self.verify_stage(spec).await?;
            self.progress
                .checkpoint(spec.name, HostingState::ConfirmedClosed)?;
            return Ok(());
        }
        self.progress.note("read_stage_intent");
        if io::read_record(&self.base.pool, self.base.storage.as_ref(), id)
            .await?
            .is_some()
        {
            return Err(invalid(
                "original Pure seed stage is unresolved; no fixture replay",
            ));
        }
        self.progress
            .checkpoint(spec.name, HostingState::NotStarted)?;
        let intent = self.stage_intent(spec)?;
        self.progress.note("retain_stage_intent");
        io::retain_new_intent(&self.base.pool, self.base.storage.as_ref(), id, &intent).await?;
        let reference = format!("original-pure-seed:{}:{}", self.operation, spec.name);
        *self
            .progress
            .admission_reference
            .lock()
            .map_err(|_| invalid("case admission custody unavailable"))? = Some(reference.clone());
        *self
            .progress
            .closed_turn
            .lock()
            .map_err(|_| invalid("case closed Turn custody unavailable"))? = None;
        self.progress
            .cleanup_pending
            .store(true, std::sync::atomic::Ordering::Release);
        self.progress.note("admit_stage");
        let mut original_admission = crate::bootstrap_validation::PreparedAdmission::new(
            self.base.pool.clone(),
            "original-pure-seed-qualification",
            &reference,
        )
        .await?;
        let retained = original_admission
            .retain(
                self.base.storage.as_ref(),
                self.id(&format!("{}:admission-original", spec.name)),
                json!({"id":id,"checksum":evidence::checksum(&intent)}),
            )
            .await;
        *self
            .progress
            .admission_reference
            .lock()
            .map_err(|_| invalid("case admission custody unavailable"))? =
            Some(original_admission.reference().to_string());
        retained?;
        let admitted =
            io::bounded(self.base.storage.as_ref(), original_admission.admit(None)).await;
        let admitted = match admitted {
            Ok(admitted) => admitted,
            Err(primary) => {
                self.progress.note_primary_failure();
                let error = original_admission
                    .failure(self.base.storage.as_ref(), primary)
                    .await;
                if let Some(closed) = original_admission.closed_turn() {
                    self.progress.acknowledged_cleanup(closed)?;
                }
                return Err(error);
            }
        };
        let proof = std::sync::OnceLock::new();
        let run=async {
            self.progress.note("prepare_stage_operation");
            let stage=self.id(spec.name);
            let operation=seed::SeedOperation::from_reserved_stage(self.operation,stage,intent.clone(),self.base.clone(),
                admitted.host.clone(),self.instrument.clone(),spec.export)?;
            let program=self.program(spec);
            self.progress.note("inspect_stage_program");
            let inspected=Arc::new(brassclaw_engine::executor::retained_source::InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(program.clone()),&self.base.worker.path(),self.base.preparation.limits())
                .await.map_err(|error| {
                    if !matches!(&error,brassclaw_engine::executor::retained_source::RetainedSourceError::Invalid {..}) {
                        let _=self.progress.checkpoint(spec.name,HostingState::InFlight);
                    }
                    invalid(error)
                })?);
            self.progress.checkpoint(spec.name,HostingState::InFlight)?;
            self.progress.note("prepare_stage_native_tools");
            let tools=crate::installed_monty_catalogue::original_pure_case_tools(self.base.pool.clone(),self.kernel.clone(),
                admitted.host.clone(),admitted.admission.clone(),program,operation.clone(),false)?;
            let source=Arc::new(seed::SeedSource {reference,inspected,operation:operation.clone(),tools,
                raw:spec.raw,expected:spec.expected.clone(),received_receipt:std::sync::OnceLock::new(),
                receipt_failure:brassclaw_monty_host::service::PortFailure::new("original_pure_seed_receipt_invalid")
                    .map_err(|_|invalid("original Pure seed receipt failure classification is invalid"))?});
            *self.progress.original.lock().map_err(|_|invalid("case original custody unavailable"))?=Some(operation.clone());
            self.progress.note("execute_contained_stage");
            let observation=crate::bootstrap_recipe_flow::execute(&admitted,
                crate::bootstrap_recipe_flow::Request::contained(self.base.worker.path(),self.base.executor.clone(),
                    &self.base.root,source.clone())?).await?;
            self.progress.note("verify_stage_root");
            if observation.receipt.root.source_checksum()!=self.base.root.source_checksum() {
                *self.progress.closed.lock().map_err(|_|invalid("case closed custody unavailable"))?=Some(Box::new(observation));
                return Err(invalid("case fixture changed the protected root"));
            }
            self.progress.note("verify_stage_quiescence");
            let (actual,quiescence)=match observation.quiescence_proof() {
                Ok(proof)=>proof,
                Err(error)=>{
                    let safe=observation.ended_failure_projection().is_ok();
                    *self.progress.closed.lock().map_err(|_|invalid("case closed custody unavailable"))?=Some(Box::new(observation));
                    if safe {self.progress.checkpoint(spec.name,HostingState::WorkerJoined)?;}
                    return Err(error);
                }
            };
            *self.progress.closed.lock().map_err(|_|invalid("case closed custody unavailable"))?=Some(Box::new(observation));
            self.progress.checkpoint(spec.name,HostingState::WorkerJoined)?;
            proof.set(quiescence).map_err(|_|invalid("duplicate seed quiescence"))?;
            self.progress.note("read_stage_native_observation");
            let received=operation.received()?;
            let receipt=source.received_receipt.get().ok_or_else(||invalid("closed seed receipt missing"))?;
            let header=json!({"format":"original-pure-seed-stage-observed/1","operation":self.operation,
                "manifest_checksum":self.manifest_checksum,"stage":spec.name,"run_id":admitted.attempt.run_id.as_uuid(),
                "scope":admitted.chat_context()?.scope,"runner_id":admitted.attempt.runner_id.as_uuid(),
                "claim_checksum":evidence::checksum(&serde_json::to_string(&admitted.attempt.lease_token).map_err(invalid)?),
                "native_invocation_id":received.invocation_id,"native_output":received.output,
                "receipt":receipt,"quiescence":actual});
            Ok::<_,RebornBuildError>(header)
        }.await;
        if run.is_err() {
            self.progress.note_primary_failure();
        }
        self.progress.note("settle_stage_close");
        let close = match proof.get() {
            Some(proof) => {
                crate::bootstrap_validation::settle_quiescent_closed(
                    &self.base.pool,
                    &admitted,
                    proof,
                )
                .await
            }
            None => crate::bootstrap_validation::settle_closed(&admitted)
                .await
                .map(|_| brassclaw_turns::TurnStatus::Completed),
        };
        self.progress.note("read_stage_close");
        let closed = crate::bootstrap_validation::read_source_close(
            &self.base.pool,
            &admitted,
            proof.get(),
            close,
            self.base.storage.as_ref(),
        )
        .await;
        let (header, closed) = match (run, closed) {
            (Ok(header), Ok(closed)) => {
                self.progress.acknowledged_cleanup(&closed)?;
                (header, closed)
            }
            (Err(primary), Ok(closed)) => {
                self.progress.acknowledged_cleanup(&closed)?;
                return Err(primary);
            }
            (Ok(_), Err(primary)) => return Err(primary),
            (Err(primary), Err(secondary)) => {
                *self
                    .progress
                    .cleanup_error
                    .lock()
                    .map_err(|_| invalid("case cleanup cause custody unavailable"))? =
                    Some(secondary);
                return Err(primary);
            }
        };
        let report = json!({"format":"original-pure-seed-stage-complete/1","manifest_checksum":self.manifest_checksum,
            "stage":spec.name,"intent":{"id":id,"checksum":evidence::checksum(&intent)},
            "observed":header,"closed_turn":closed});
        let bytes = self.record(done, "behavior", true, report);
        // Original native journals, joined worker and acknowledged closed Turn
        // are checked before any qualifying completion is published.
        self.progress.note("verify_new_stage_completion");
        self.verify_stage_bytes(spec, &bytes).await?;
        self.progress.note("retain_stage_completion");
        io::retain_record(&self.base.pool, self.base.storage.as_ref(), done, &bytes).await?;
        self.progress.note("verify_retained_stage_completion");
        self.verify_stage(spec).await.map(|_| ())
    }
    async fn verify_stage(
        &self,
        spec: &Spec,
    ) -> Result<
        brassclaw_skills::review_submission_store::selection::SourceProofReference,
        RebornBuildError,
    > {
        let id = self.id(&format!("{}:complete", spec.name));
        let bytes = io::read_record(&self.base.pool, self.base.storage.as_ref(), id)
            .await?
            .ok_or_else(|| invalid("original Pure seed completion missing"))?;
        self.verify_stage_bytes(spec, &bytes).await?;
        Ok(
            brassclaw_skills::review_submission_store::selection::SourceProofReference {
                id,
                checksum: evidence::checksum(&bytes),
            },
        )
    }

    async fn verify_stage_bytes(&self, spec: &Spec, bytes: &str) -> Result<(), RebornBuildError> {
        io::bounded(self.base.storage.as_ref(), async {
            let mut client = crate::pg_transaction_client::TransactionClient::from_client(
                self.base.pool.get().await.map_err(invalid)?,
            );
            let tx = client
                .build_transaction()
                .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            self.verify_stage_bytes_in_transaction(&tx, spec, bytes)
                .await?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(())
        })
        .await
    }
    async fn verify_stage_bytes_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        spec: &Spec,
        bytes: &str,
    ) -> Result<(), RebornBuildError> {
        let envelope: Value = serde_json::from_str(bytes).map_err(invalid)?;
        let report = &envelope["report"];
        let id = self.id(&format!("{}:complete", spec.name));
        if bytes != self.record(id, "behavior", true, report.clone())
            || report["format"] != "original-pure-seed-stage-complete/1"
            || report["manifest_checksum"] != self.manifest_checksum
            || report["stage"] != spec.name
        {
            return Err(invalid("original Pure seed completion scope differs"));
        }
        if report.as_object().is_none_or(|r| r.len() != 6)
            || report["observed"].as_object().is_none_or(|r| r.len() != 12)
        {
            return Err(invalid("original Pure seed stage fields differ"));
        }
        let observed = &report["observed"];
        let run: Uuid = serde_json::from_value(observed["run_id"].clone()).map_err(invalid)?;
        let invocation: Uuid =
            serde_json::from_value(observed["native_invocation_id"].clone()).map_err(invalid)?;
        let output = observed["native_output"]
            .as_str()
            .ok_or_else(|| invalid("seed native output absent"))?;
        let native: Value = serde_json::from_str(output).map_err(invalid)?;
        let receipt = observed["receipt"]
            .as_str()
            .ok_or_else(|| invalid("seed receipt absent"))?;
        let q = &observed["quiescence"];
        if observed["format"] != "original-pure-seed-stage-observed/1"
            || observed["operation"] != json!(self.operation)
            || observed["manifest_checksum"] != self.manifest_checksum
            || observed["stage"] != spec.name
            || native["format"] != "original-pure-invocation/1"
            || native["outcome"] != spec.expected
            || native["export"] != spec.export
            || native["component"]
                != serde_json::to_value(self.instrument.inspected().map_err(invalid)?.component())
                    .map_err(invalid)?
            || q["receipt_root"]["source_checksum"] != json!(self.base.root.source_checksum())
            || q["accounting"]["task"] != native["task"]
            || q["accounting"]["failure"] != Value::Null
            || !q["accounting"]["compute_time"].is_object()
            || q["execution"]["root_completed"] != true
            || q["execution"]["all_selected_recipes_complete"] != true
            || q["execution"]["controlled_executor_configuration_checksum"]
                != self.base.executor.checksum()
            || q["execution"]["model_exchange_count"] != 0
            // This existing counter covers the legacy capability port. The
            // retained native dispatch is proved by its exact journal below.
            || q["execution"]["capability_dispatch_count"] != 0
            || q["execution"]["withheld_root_answers"] != 0
            || q["execution"]["withheld_host_answers"] != false
            || q["execution"]["internal_receipt"] != receipt
            || q["execution"]["root"]["vm_id"] != q["receipt_root"]["vm_id"]
            || q["execution"]["root"]["source_checksum"]
                != hex::encode(self.base.root.source_checksum())
            || q["execution"]["reply"] != Value::Null
            || q["execution"]["semantic_approval"] != false
            || q["execution"]["catalogue_activation"] != false
            || q["worker_exit"]["root"] != q["receipt_root"]
            || q["worker_exit"]["status"]["success"] != true
            || q["worker_exit"]["retained_tasks"] != 0
            || q["worker_exit"]["pending_admissions"] != 0
            || q["worker_exit"]["queued_admissions"] != 0
            || q["worker_exit"]["failed_exchanges"] != 0
            || q["worker_exit"]["rejected_commands"] != 0
        {
            return Err(invalid(
                "seed actual outcome, account or joined lifecycle differs",
            ));
        }
        if (spec.raw && receipt != output)
            || (!spec.raw
                && serde_json::from_str::<Value>(receipt).map_err(invalid)?
                    != json!({"format":"original-pure-case-receipt/1","observation":native}))
        {
            return Err(invalid("seed actual harness relation differs"));
        }
        let closed: brassclaw_turns::TurnRunRecord =
            serde_json::from_value(report["closed_turn"].clone()).map_err(invalid)?;
        if closed.run_id.as_uuid() != run
            || serde_json::to_value(&closed.scope).map_err(invalid)? != observed["scope"]
            || closed.claim_count != 1
            || closed.runner_id.is_some()
            || closed.lease_token.is_some()
            || closed.lease_expires_at.is_some()
            || closed.failure.is_some()
            || closed.native_auth_wait.is_some()
            || !matches!(
                closed.status,
                brassclaw_turns::TurnStatus::Completed | brassclaw_turns::TurnStatus::Cancelled
            )
        {
            return Err(invalid(
                "seed original closed Turn is not a settled single claim",
            ));
        }
        let actual_turn = brassclaw_turns::PgTurnStateStore::read_run_record_in_transaction(
            tx,
            &closed.scope,
            closed.run_id,
        )
        .await
        .map_err(invalid)?
        .ok_or_else(|| invalid("seed actual closed Turn missing"))?;
        if serde_json::to_value(&actual_turn).map_err(invalid)? != report["closed_turn"] {
            return Err(invalid("seed actual closed Turn differs"));
        }
        seed::verify_originals(tx, self, spec, observed, &native).await?;
        let intent = self.stage_intent(spec)?;
        let actual = evidence::read_evidence(tx, self.id(&format!("{}:intent", spec.name)))
            .await?
            .ok_or_else(|| invalid("original seed stage intent missing"))?;
        if actual != intent
            || report["intent"]
                != json!({"id":self.id(&format!("{}:intent",spec.name)),"checksum":evidence::checksum(&intent)})
        {
            return Err(invalid("original seed stage intent differs"));
        }
        let row=tx.query_one("SELECT i.selection_bytes,i.arguments_bytes,i.answer_bytes,i.phase,i.attempt_count,
                a.phase,a.outcome,a.scope,a.runner_id,a.claim_checksum,a.started_at IS NOT NULL,
                (SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1)
                FROM brassclaw_monty_tool_invocations i JOIN brassclaw_monty_task_admissions a USING(run_id)
                WHERE i.run_id=$1 AND i.invocation_id=$2",&[&run,&invocation]).await.map_err(invalid)?;
        let selected = crate::pg_monty_admission::PgMontyAdmission::invocation_selection(
            &self.program(spec),
            "0:2",
        )
        .map_err(invalid)?;
        let answer: Value = serde_json::from_str(row.get::<_, &str>(2)).map_err(invalid)?;
        if row.get::<_, &str>(0) != selected
            || row.get::<_, &str>(1) != "{}"
            || answer != json!({"kind":"return","value":output})
            || row.get::<_, &str>(3) != "answered"
            || row.get::<_, i16>(4) != 1
            || row.get::<_, &str>(5) != "settled"
            || row.get::<_, Value>(6)
                != json!({"status":"failed","reason_kind":"bootstrap_validation_closed"})
            || row.get::<_, Value>(7) != observed["scope"]
            || json!(row.get::<_, Uuid>(8)) != observed["runner_id"]
            || hex::encode(row.get::<_, Vec<u8>>(9))
                != observed["claim_checksum"].as_str().unwrap_or("")
            || !row.get::<_, bool>(10)
            || row.get::<_, i64>(11) != 1
        {
            return Err(invalid("seed original native journal/admission differs"));
        }
        Ok(())
    }
}

pub(crate) async fn qualify(
    base: SessionInput,
    kernel: Arc<dyn MontyKernelSnapshot>,
    tenant: &str,
) -> Result<
    (
        Option<Arc<QualifiedCaseHarness>>,
        Option<Arc<CaseUnavailable>>,
    ),
    RebornBuildError,
> {
    use brassclaw_host_runtime::RetainedCapabilityError as NativeError;
    let progress = Arc::new(QualificationProgress::new());
    progress.note("resolve_native_registration");
    let capability =
        CapabilityId::new(brassclaw_host_runtime::PURE_CASE_CAPABILITY_ID).map_err(invalid)?;
    let native = match kernel.builtin(&capability, None) {
        Ok(native) => native,
        Err(
            error @ (NativeError::MissingCapability
            | NativeError::MissingRegistration
            | NativeError::MissingNativeRegistration),
        ) => {
            return Ok((
                None,
                Some(Arc::new(failure(
                    "native_registration",
                    "missing_optional_case_registration",
                    error,
                    progress,
                ))),
            ));
        }
        Err(error) => {
            return Err(fatal(failure(
                "native_registration",
                "shared_registration_integrity",
                error,
                progress,
            )));
        }
    };
    progress.note("verify_native_artifact");
    if let Err(error) = native.verify_native_artifact().await {
        return Err(fatal(failure(
            "native_artifact",
            "shared_native_artifact_integrity",
            error,
            progress,
        )));
    }
    let identity = native.native_identity().ok_or_else(|| {
        fatal(failure(
            "native_artifact",
            "missing_native_identity",
            NativeError::MissingNativeRegistration,
            progress.clone(),
        ))
    })?;
    match qualify_inner(base, kernel, tenant, identity, progress.clone()).await {
        Ok(Some(ready)) => {
            *progress
                .original
                .lock()
                .map_err(|_| invalid("case original custody unavailable"))? = None;
            *progress
                .closed
                .lock()
                .map_err(|_| invalid("case closed custody unavailable"))? = None;
            Ok((Some(ready), None))
        }
        Ok(None) => Ok((
            None,
            Some(Arc::new(CaseUnavailable {
                phase: "initial_tool_policy",
                reason: "case_tool_blocked",
                cause: None,
                custody: progress,
            })),
        )),
        Err(cause) => {
            let (phase, state) = *progress
                .phase
                .lock()
                .map_err(|_| invalid("case readiness custody unavailable"))?;
            let reason = match state {
                HostingState::NotStarted => "case_preparation_unavailable",
                HostingState::ConfirmedClosed => "case_fixture_closed_unqualified",
                HostingState::WorkerJoined => "case_worker_joined_cleanup_pending",
                _ => "case_containment_unconfirmed",
            };
            let error = failure(phase, reason, cause, progress);
            if matches!(
                state,
                HostingState::NotStarted
                    | HostingState::WorkerJoined
                    | HostingState::ConfirmedClosed
            ) {
                Ok((None, Some(Arc::new(error))))
            } else {
                Err(fatal(error))
            }
        }
    }
}
async fn qualify_inner(
    base: SessionInput,
    kernel: Arc<dyn MontyKernelSnapshot>,
    tenant: &str,
    identity: brassclaw_host_runtime::NativeImplementationRef,
    progress: Arc<QualificationProgress>,
) -> Result<Option<Arc<QualifiedCaseHarness>>, RebornBuildError> {
    progress.note("retain_case_package");
    let mut graph = io::bounded(
        base.storage.as_ref(),
        retain(base.pool.clone(), identity, tenant),
    )
    .await?;
    let draft = seed::fixture_draft()?;
    let store = PgComponentRevisionStore::new(base.pool.clone());
    progress.note("retain_seed_fixture");
    io::bounded(
        base.storage.as_ref(),
        graph.definitions.retain(&store, &draft),
    )
    .await?;
    progress.note("snapshot_seed_fixture");
    let instrument_snapshot = io::bounded(
        base.storage.as_ref(),
        graph.definitions.snapshot(base.pool.clone(), draft.uuid()),
    )
    .await?;
    progress.note("inspect_seed_fixture");
    let instrument = Arc::new(
        brassclaw_engine::executor::retained_source::InspectedPureSelection::inspect(
            instrument_snapshot,
            draft.uuid(),
            &base.worker.path(),
            base.preparation.limits(),
        )
        .await
        .map_err(|error| {
            if !matches!(
                &error,
                brassclaw_engine::executor::retained_source::RetainedSourceError::Invalid { .. }
            ) {
                let _ = progress.checkpoint("fixture_inspection", HostingState::InFlight);
            }
            invalid(error)
        })?,
    );
    let manifest = json!({"format":"original-pure-seed-manifest/1",
        "program":graph.program.inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes(),
        "qualification_program":graph.qualification.inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes(),
        "instrument":instrument.inspected().map_err(invalid)?.component(),"worker_checksum":hex::encode(base.worker.checksum()),
        "root":base.root.reference(),"root_checksum":hex::encode(base.root.source_checksum()),
        "executor_bytes":base.executor.exact_bytes(),"executor_checksum":base.executor.checksum(),
        "preparation_bytes":base.preparation.bytes(),"application_checksum":base.application_checksum,
        "owner_checksum":crate::bootstrap_validation::qualification_owner_checksum(include_str!("pure_case_qualification.rs")),
        "fixture_owner_checksum":evidence::checksum(include_str!("pure_case_seed_fixture.rs")),
        "native_handler_checksum":evidence::checksum(include_str!("../../brassclaw_host_runtime/src/first_party_tools/pure_case.rs")),
        "carrier_checksum":evidence::checksum(include_str!("../../brassclaw_engine/src/executor/retained_pure_invocation.rs")),
        "specs":specs().iter().map(|s|json!({"stage":s.name,"export":s.export,"expected":s.expected,"raw":s.raw})).collect::<Vec<_>>()});
    let manifest_checksum = evidence::checksum(&manifest.to_string());
    let slot = Uuid::new_v5(
        &pure_case_components::usage_ids().recipe,
        format!("original-pure-seed-reservation/1:{manifest_checksum}").as_bytes(),
    );
    let components = graph
        .program
        .inputs()
        .instruction()
        .snapshot()
        .revisions()
        .values()
        .map(|r| crate::installed_monty_catalogue::reference_value(r.reference()))
        .collect::<Vec<_>>();
    let association =
        evidence::checksum(graph.program.bindings()["0:2"].association().exact_bytes());
    progress.note("read_seed_reservation");
    let existing = io::read_record(&base.pool, base.storage.as_ref(), slot).await?;
    let bytes = match existing {
        Some(bytes) => bytes,
        None => {
            progress.note("read_initial_tool_policy");
            let enabled = io::bounded(base.storage.as_ref(), async {
                Ok(base
                    .pool
                    .get()
                    .await
                    .map_err(invalid)?
                    .query_one(
                        "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                        &[&pure_case_components::usage_ids().tool],
                    )
                    .await
                    .map_err(invalid)?
                    .get::<_, bool>(0))
            })
            .await?;
            if !enabled {
                tracing::info!(
                    "original Pure case feature unready: its separate native Tool is blocked before first qualification"
                );
                return Ok(None);
            }
            let offered = evidence::system_record(
                slot,
                "behavior",
                false,
                &association,
                &components,
                json!({"format":"original-pure-seed-reservation/1","manifest":manifest,"operation_id":Uuid::new_v4()}),
            );
            progress.note("retain_seed_reservation");
            let _write = io::retain_record(&base.pool, base.storage.as_ref(), slot, &offered).await;
            progress.note("read_seed_reservation_acknowledgement");
            io::read_record(&base.pool, base.storage.as_ref(), slot)
                .await?
                .ok_or_else(|| invalid("original Pure seed reservation remains unresolved"))?
        }
    };
    progress.note("verify_seed_reservation");
    let offered: Value = serde_json::from_str(&bytes).map_err(invalid)?;
    let report = &offered["report"];
    let operation: Uuid =
        serde_json::from_value(report["operation_id"].clone()).map_err(invalid)?;
    if operation.is_nil()
        || report["format"] != "original-pure-seed-reservation/1"
        || report["manifest"] != manifest
        || bytes
            != evidence::system_record(
                slot,
                "behavior",
                false,
                &association,
                &components,
                report.clone(),
            )
    {
        return Err(invalid("original Pure seed reservation differs"));
    }
    let context = Context {
        progress,
        base,
        kernel,
        graph,
        instrument,
        manifest,
        manifest_checksum,
        operation,
        components,
        association,
    };
    for spec in specs().iter().filter(|s| s.name.starts_with("primitive-")) {
        context.run_stage(spec).await?;
    }
    let tool = context.graph.program.bindings()["0:2"].tool();
    let tool_skill = context.graph.program.bindings()["0:2"].tool_skill();
    let binding_record = context.id("tool-skill-compatible");
    let binding=context.record(binding_record,"behavior",true,json!({"format":"original-pure-tool-skill-qualification/1",
        "manifest_checksum":context.manifest_checksum,"tool":tool,"tool_skill":tool_skill,"argument_checks":context.binding_checks()?,
        "prerequisites":specs().iter().filter(|s|s.name.starts_with("primitive-")).map(|s|context.id(&format!("{}:complete",s.name))).collect::<Vec<_>>() }));
    context.progress.note("retain_tool_skill_qualification");
    io::retain_record(
        &context.base.pool,
        context.base.storage.as_ref(),
        binding_record,
        &binding,
    )
    .await?;
    context
        .run_stage(
            &specs()
                .into_iter()
                .find(|s| s.name == "skill-code-function")
                .ok_or_else(|| invalid("closed Skill/code qualification Spec missing"))?,
        )
        .await?;
    context
        .progress
        .checkpoint("independent_encoder", HostingState::InFlight)?;
    context.progress.note("qualify_independent_encoder");
    let encoder =
        crate::bootstrap_validator_approval::qualify_case_encoder(context.base.clone()).await?;
    context
        .progress
        .checkpoint("independent_encoder", HostingState::ConfirmedClosed)?;
    context.progress.note("verify_independent_encoder");
    let encoder_ref = verify_encoder(&context, &encoder).await?;
    context
        .run_stage(
            &specs()
                .into_iter()
                .find(|s| !s.raw)
                .ok_or_else(|| invalid("closed consuming harness qualification Spec missing"))?,
        )
        .await?;
    context.progress.note("verify_all_stage_completions");
    let mut completed = Vec::new();
    for spec in specs() {
        completed.push(context.verify_stage(&spec).await?);
    }
    let proof_id = context.id("qualified-private-harness");
    let proof=context.record(proof_id,"behavior",true,json!({"format":"qualified-original-pure-harness/1",
        "manifest":context.manifest,"reservation":{"id":slot,"checksum":evidence::checksum(&bytes)},
        "tool_skill":{"id":binding_record,"checksum":evidence::checksum(&binding)},"encoder":encoder_ref,
        "completed":completed,"scope":"private-single-case-observation-only","authored_q1":false,"human_q2":false,"activation":false}));
    context.progress.note("retain_harness_qualification");
    io::retain_record(
        &context.base.pool,
        context.base.storage.as_ref(),
        proof_id,
        &proof,
    )
    .await?;
    Ok(Some(Arc::new(QualifiedCaseHarness {
        program: context.graph.program.clone(),
        proof_id,
        proof_bytes: proof,
        prerequisites: vec![(slot, bytes), (binding_record, binding)],
        context: Arc::new(context),
        encoder,
    })))
}

async fn verify_encoder(
    context: &Context,
    encoder: &CheckedCaseEncoder,
) -> Result<
    brassclaw_skills::review_submission_store::selection::SourceProofReference,
    RebornBuildError,
> {
    io::bounded(context.base.storage.as_ref(), async {
        let mut client = crate::pg_transaction_client::TransactionClient::from_client(
            context.base.pool.get().await.map_err(invalid)?,
        );
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        let reference = encoder.verify(&tx, &context.graph.program).await?;
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(reference)
    })
    .await
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn fatal_diagnostic_redacts_private_text_and_retains_original_cause() {
        let private = "private SQL/source/receipt marker";
        let progress = Arc::new(QualificationProgress::new());
        progress.note("execute_contained_stage");
        progress.note_primary_failure();
        progress.note("read_stage_close");
        let error = fatal(failure(
            "primitive-return",
            "case_containment_unconfirmed",
            invalid(private),
            progress,
        ));
        let display = error.to_string();
        assert!(display.contains("activity=read_stage_close"));
        assert!(display.contains("primary_activity=execute_contained_stage"));
        assert!(display.contains("cause=invalid_config_private"));
        assert!(!display.contains(private));
        assert!(!format!("{error:?}").contains(private));
        let original = error
            .source()
            .unwrap()
            .downcast_ref::<CaseUnavailable>()
            .unwrap();
        let original = original
            .source()
            .unwrap()
            .downcast_ref::<RebornBuildError>()
            .unwrap();
        assert!(
            matches!(original, RebornBuildError::InvalidConfig { reason } if reason == private)
        );
    }

    #[test]
    fn contained_diagnostic_uses_only_exact_closed_service_reason() {
        assert_eq!(
            diagnostic_cause(&invalid("bootstrap Recipe completion: Deadline")),
            "contained_completion/Deadline"
        );
        assert_eq!(
            diagnostic_cause(&invalid(
                "bootstrap Recipe completion: Deadline private payload"
            )),
            "invalid_config_private"
        );
    }
}
