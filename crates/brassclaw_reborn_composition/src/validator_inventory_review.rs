//! Closed source-review observations. Neither this source nor its contained
//! parser qualification supplies candidate Q1, own approval or activation.
use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{
            RetainedProgram, RetainedRecipeExecution, RetainedToolPort,
            port_answer_checksum_with_limits, typed_value_checksum_with_limits,
        },
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_tools::{RetainedToolBinding, RetainedToolProgram},
        retained_validator::RetainedValidatorCoverage,
    },
};
use brassclaw_host_runtime::PackagedExecutableImage;
use brassclaw_monty_host::{process::PortAnswer, service::PortFailure};
use brassclaw_pg::PgPool;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    validator_components::{python_pure_structure_drafts, python_structure_drafts},
    value_contract::{ContractLimits, strict_json},
};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::pg_review_admission_source::verified_worker;

use crate::{
    RebornBuildError,
    bootstrap_reply_validation::invalid,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    monty_kernel::MontyKernelSnapshot,
    pg_monty_admission::PgMontyAdmission,
};

/// Factual wire form shared by the contained observations. This serializes an
/// already selected revision; it neither parses nor qualifies a revision.
fn revision_reference_value(reference: ComponentRevisionRef) -> Value {
    json!({"uuid":reference.uuid,"class_code":reference.class_code,
        "version":reference.version,"checksum":hex::encode(reference.checksum)})
}

#[path = "installation_inventory_observation.rs"]
mod installation;
pub(crate) use installation::CheckedPureSourceWorkflow;
#[cfg(test)]
pub(crate) use installation::assert_original_installation_readback;
#[cfg(test)]
pub(crate) use installation::assert_original_pure_workflow_readback;
#[cfg(test)]
pub(crate) use installation::tests::before_cancel as installation_test_before_cancel;
#[cfg(test)]
pub(crate) use installation::tests::before_close as installation_test_before_close;
#[cfg(test)]
pub(crate) use installation::tests::before_reservation as installation_test_before_reservation;
#[cfg(test)]
pub(crate) use installation::tests::before_turn_close as installation_test_before_turn_close;
pub(crate) use installation::{observe_installation, qualify_pure_workflow};

#[path = "qualified_pure_source_review.rs"]
mod qualified_source;
pub(crate) use qualified_source::{
    CheckedPureSourceOriginal, QualifiedPureSourceReview, SourceReviewPreparation,
};

#[path = "bootstrap_inventory_qualification.rs"]
mod qualification;
use brassclaw_engine::memory::retained_validator::ObservedSubjectValidatorChecks;
use brassclaw_skills::review_submission_store::{
    RetainedValidatorSubject, selection::RetainedValidatorSelection,
};
#[cfg(all(test, unix))]
pub(crate) use qualification::assert_actual_ack_and_timeout_faults;
#[cfg(test)]
pub(crate) use qualification::qualify;

/// Issued only by the contained installation qualification below. No JSON
/// constructor or historical mechanical approval can mint this handle.
pub(crate) struct QualifiedSourceInventoryUsage {
    combination: Vec<ComponentRevisionRef>,
    association: String,
    worker: Arc<PackagedExecutableImage>,
    proof: qualification::InventoryQualification,
}
impl QualifiedSourceInventoryUsage {
    pub(crate) fn require_program<'a>(
        &self,
        program: &'a RetainedToolProgram,
    ) -> Result<(&'a str, &'a RetainedToolBinding), RebornBuildError> {
        let (step, binding) = require_inventory_binding(program)?;
        if binding.combination() != self.combination.as_slice()
            || binding.association().exact_bytes() != self.association
        {
            return Err(invalid(
                "qualified inventory usage differs from selected graph",
            ));
        }
        Ok((step, binding))
    }
}

/// Locate the single actual binding; its canonical step ID is not an allowlist.
pub(crate) fn require_inventory_binding(
    program: &RetainedToolProgram,
) -> Result<(&str, &RetainedToolBinding), RebornBuildError> {
    let mut bindings = program.bindings().iter();
    let (step, binding) = bindings
        .next()
        .ok_or_else(|| invalid("inventory binding missing"))?;
    if bindings.next().is_some()
        || binding.capability_id() != brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID
        || binding.association().callable() != "host.source_inspection"
    {
        return Err(invalid(
            "review requires exactly its selected inventory usage",
        ));
    }
    Ok((step, binding))
}

#[derive(Clone, Copy)]
pub(crate) enum InventoryReviewProfile {
    Structure,
    Pure,
}
fn require_packaged_inventory_profile(
    program: &RetainedToolProgram,
    binding: &RetainedToolBinding,
    profile: InventoryReviewProfile,
) -> Result<(), RebornBuildError> {
    let ids = crate::source_inspection_components::usage_ids(binding.tool().uuid, true);
    let native_profile =
        serde_json::to_value(brassclaw_engine::executor::retained_profile::profile())
            .map_err(invalid)?;
    let drafts = match profile {
        InventoryReviewProfile::Structure => python_structure_drafts(
            ids,
            crate::builtin_bootstrap::VALIDATOR_STRUCTURE_RECEIPT_SEED,
            native_profile,
        ),
        InventoryReviewProfile::Pure => python_pure_structure_drafts(
            ids,
            crate::builtin_bootstrap::VALIDATOR_PURE_STRUCTURE_RECEIPT_SEED,
            native_profile,
        ),
    }
    .map_err(invalid)?;
    // Compare complete factory bytes, including typed native/result edges,
    // variant, rule order and receipt profile. A step name grants nothing.
    let graph = program.inputs().instruction().snapshot().revisions();
    for draft in &drafts {
        if graph
            .get(&draft.uuid())
            .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid("review graph differs from packaged profile"));
        }
    }
    let shared = crate::builtin_bootstrap::validator_check_drafts()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    for revision in graph.values() {
        if binding.combination().contains(&revision.reference()) {
            continue;
        }
        let expected = drafts
            .iter()
            .chain(shared.iter())
            .find(|draft| draft.uuid() == revision.reference().uuid)
            .ok_or_else(|| invalid("review contains an undeclared checking component"))?;
        if expected.exact_bytes() != revision.draft().exact_bytes() {
            return Err(invalid(
                "selected checking definition differs from packaged bytes",
            ));
        }
    }
    if !drafts
        .iter()
        .any(|d| d.class_code() == 21 && d.uuid() == program.inputs().instruction().recipe().uuid)
    {
        return Err(invalid("review selected a different Recipe"));
    }
    Ok(())
}

/// Original prepared graph/subject plus real native ports. Complete observations
/// may survive later cancellation; this type never claims terminal settlement.
pub(crate) struct InventoryReviewSource {
    reference: String,
    subject: Arc<RetainedValidatorSubject>,
    saved: Arc<RetainedValidatorSelection>,
    inspected: Arc<InspectedRetainedProgram>,
    coverage: RetainedValidatorCoverage,
    parser: Arc<QualifiedSourceInventoryUsage>,
    #[cfg(test)]
    worker: Arc<PackagedExecutableImage>,
    persist: bool,
    tools: Arc<dyn RetainedToolPort>,
    host: Arc<MontyTaskHost>,
    pool: Arc<PgPool>,
    inputs: Value,
    limits: ContractLimits,
    observation: Mutex<Option<ObservedSubjectValidatorChecks>>,
}
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct InventoryReviewRequest {
    pub(crate) pool: Arc<PgPool>,
    pub(crate) kernel: Arc<dyn MontyKernelSnapshot>,
    pub(crate) host: Arc<MontyTaskHost>,
    pub(crate) admission: Arc<PgMontyAdmission>,
    pub(crate) subject: Arc<RetainedValidatorSubject>,
    pub(crate) saved: Arc<RetainedValidatorSelection>,
    pub(crate) inspected: Arc<InspectedRetainedProgram>,
    pub(crate) parser: Arc<QualifiedSourceInventoryUsage>,
    pub(crate) profile: InventoryReviewProfile,
}
#[cfg(test)]
impl InventoryReviewSource {
    pub(crate) async fn prepare(
        request: InventoryReviewRequest,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let InventoryReviewRequest {
            pool,
            kernel,
            host,
            admission,
            subject,
            saved,
            inspected,
            parser,
            profile,
        } = request;
        let limits = captured_limits(&inspected);
        parser.proof.verify(&pool).await?;
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(invalid("inventory review needs retained bindings"));
        };
        let (_, binding) = parser.require_program(program)?;
        require_packaged_inventory_profile(program, binding, profile)?;
        let restored = RetainedValidatorCoverage::restore_preparation(&saved).map_err(invalid)?;
        if restored
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .exact_bytes()
            != program
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(invalid)?
                .exact_bytes()
        {
            return Err(invalid("review differs from retained preparation"));
        }
        let worker = verified_worker(
            &parser.worker,
            inspected.preparation_limits().max_native_artifact_bytes,
        )
        .await?;
        // Reinspect using the retained qualified worker, never an installation
        // pathname supplied by the review caller.
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(Arc::new(restored)),
                &worker.path(),
                inspected.preparation_limits(),
            )
            .await
            .map_err(invalid)?,
        );
        let coverage = RetainedValidatorCoverage::from_inputs(inspected.program().inputs())
            .map_err(invalid)?;
        let inputs = coverage
            .subject_task_inputs(&subject, inspected.program().inputs(), limits)
            .map_err(invalid)?;
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(invalid("review bindings missing"));
        };
        let tools = crate::installed_monty_catalogue::qualified_inventory_tools(
            pool.clone(),
            kernel,
            host.clone(),
            admission,
            program.clone(),
            &parser,
        )?;
        Ok(Arc::new(Self {
            reference: crate::pg_review_admission_source::validator_review_reference(
                subject.submission_id(),
            ),
            subject,
            saved,
            inspected,
            coverage,
            parser,
            worker,
            persist: false,
            tools,
            host,
            pool,
            inputs,
            limits,
            observation: Mutex::new(None),
        }))
    }

    pub(crate) fn worker_path(&self) -> std::path::PathBuf {
        self.worker.path()
    }
    pub(crate) async fn take_observation(&self) -> Option<ObservedSubjectValidatorChecks> {
        self.observation.lock().await.take()
    }
}
impl InventoryReviewSource {
    /// Production entry accepts only an original /2 preparation already checked
    /// by the qualified owner in the same captured admission snapshot.
    pub(crate) fn from_qualified(
        prepared: Arc<crate::pg_review_admission_source::PreparedValidatorReview>,
        parser: Arc<QualifiedSourceInventoryUsage>,
        catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
    ) -> Result<Self, RebornBuildError> {
        if prepared.selection().source_qualification().is_none() {
            return Err(invalid("qualified Source preparation required"));
        }
        let inspected = prepared.inspected();
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(invalid("Source bindings required"));
        };
        parser.require_program(program)?;
        let tools = crate::installed_monty_catalogue::qualified_inventory_tools(
            catalogue.pool.clone(),
            catalogue.kernel.clone(),
            host.clone(),
            admission,
            program.clone(),
            &parser,
        )?;
        let limits = captured_limits(&inspected);
        Ok(Self {
            reference: crate::pg_review_admission_source::validator_review_reference(
                prepared.subject().submission_id(),
            ),
            subject: prepared.retained_subject(),
            saved: prepared.selection(),
            inspected: inspected.clone(),
            coverage: RetainedValidatorCoverage::from_inputs(inspected.program().inputs())
                .map_err(invalid)?,
            #[cfg(test)]
            worker: catalogue.worker_image.clone(),
            parser,
            tools,
            host,
            pool: catalogue.pool.clone(),
            inputs: prepared.input_capture().inputs().as_value().clone(),
            limits,
            observation: Mutex::new(None),
            persist: true,
        })
    }
}
fn failure() -> PortFailure {
    PortFailure::new("validator_inventory_observation_invalid").expect("static classification")
}

#[async_trait]
impl InternalRecipeSource for InventoryReviewSource {
    fn name(&self) -> &str {
        "contained-original-source-review"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        Some(self.inspected.clone())
    }
    fn requires_step_observations(&self) -> bool {
        true
    }
    fn bind_native_runtimes(&self) -> Result<(), PortFailure> {
        Ok(())
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let (checks, results) = crate::pg_review_admission_source::ValidatorReceiptSubject {
            subject: &self.subject,
            selection: &self.saved,
            inspected: &self.inspected,
            coverage: &self.coverage,
            limits: self.limits,
        }
        .observe(
            crate::pg_review_admission_source::ValidatorReceiptProfile::Inventory,
            receipt,
            selected,
            execution,
        )
        .map_err(|_| failure())?;
        let RetainedProgram::Tools(program) = selected.program() else {
            return Err(failure());
        };
        let (step, _) = self
            .parser
            .require_program(program)
            .map_err(|_| failure())?;
        NativeInventoryObservation {
            pool: &self.pool,
            host: &self.host,
            program,
            step,
            worker: self.parser.worker.checksum(),
            limits: self.limits,
        }
        .verify(execution, &results[step])
        .await
        .map_err(|_| failure())?;
        if self.persist {
            let task =
                Uuid::parse_str(&execution.task_handle().to_string()).map_err(|_| failure())?;
            crate::pg_validator_q1_evidence::persist_validator_source_evidence(
                &self.pool,
                &self.subject,
                &self.saved,
                &checks,
                task,
                self.limits,
            )
            .await
            .map_err(|_| failure())?;
        }
        let mut retained = self.observation.lock().await;
        if retained.is_some() {
            return Err(failure());
        }
        *retained = Some(checks);
        Ok(())
    }
}

fn captured_limits(inspected: &InspectedRetainedProgram) -> ContractLimits {
    let bounds = inspected.preparation_limits().bounds;
    ContractLimits {
        max_depth: bounds.max_value_depth,
        max_nodes: bounds.max_value_nodes,
        max_bytes: bounds.max_value_bytes,
    }
}
/// Native return, executor fingerprints and the original PG invocation must all
/// agree. This authenticates facts, not the candidate's semantics or settlement.
struct NativeInventoryObservation<'a> {
    pool: &'a PgPool,
    host: &'a MontyTaskHost,
    program: &'a RetainedToolProgram,
    step: &'a str,
    worker: [u8; 32],
    limits: ContractLimits,
}
impl NativeInventoryObservation<'_> {
    async fn verify(
        self,
        execution: &RetainedRecipeExecution,
        result: &Value,
    ) -> Result<Value, RebornBuildError> {
        let Self {
            pool,
            host,
            program,
            step,
            worker,
            limits,
        } = self;
        let [(actual_step, answer)] = execution.host_answers() else {
            return Err(invalid("inventory requires one actual native answer"));
        };
        let PortAnswer::Return { value } = answer else {
            return Err(invalid("inventory native answer failed"));
        };
        let observed = execution
            .observations()
            .get(step)
            .ok_or_else(|| invalid("inventory observation missing"))?;
        if actual_step != step
            || value.as_value() != result
            || observed.answer_checksum()
                != Some(
                    port_answer_checksum_with_limits(answer, observed.data_limits())
                        .map_err(invalid)?,
                )
            || observed.result_checksum()
                != Some(
                    typed_value_checksum_with_limits(result, observed.data_limits())
                        .map_err(invalid)?,
                )
        {
            return Err(invalid("inventory answer differs from original executor"));
        }
        let client = pool.get().await.map_err(invalid)?;
        let rows = client.query("SELECT selection_bytes,selection_checksum,arguments_bytes,arguments_checksum,answer_bytes,answer_checksum,attempt_count,phase FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND recipe_id=$2 AND step_id=$3", &[&host.attempt().run_id.as_uuid(), &program.inputs().instruction().recipe().uuid, &step]).await.map_err(invalid)?;
        let [row] = rows.as_slice() else {
            return Err(invalid("original native inventory invocation missing"));
        };
        let bytes: &str = row.get(0);
        let arguments: &str = row.get(2);
        let answer_bytes: &str = row
            .get::<_, Option<&str>>(4)
            .ok_or_else(|| invalid("inventory answer remains unresolved"))?;
        if bytes != PgMontyAdmission::invocation_selection(program, step).map_err(invalid)?
            || crate::bootstrap_evidence::checksum(bytes) != row.get::<_, &str>(1)
            || crate::bootstrap_evidence::checksum(arguments) != row.get::<_, &str>(3)
            || crate::bootstrap_evidence::checksum(answer_bytes)
                != row
                    .get::<_, Option<&str>>(5)
                    .ok_or_else(|| invalid("inventory answer digest missing"))?
            || row.get::<_, i16>(6) != 1
            || row.get::<_, &str>(7) != "answered"
        {
            return Err(invalid("inventory original journal differs"));
        }
        let arguments = strict_json(arguments, limits).map_err(invalid)?;
        verify_inventory_fingerprints(
            result,
            &arguments,
            answer_bytes,
            InventoryFingerprints {
                input: observed.input_checksum(),
                arguments: observed.arguments_checksum(),
                answer: observed.answer_checksum(),
                result: observed.result_checksum(),
            },
            observed.data_limits(),
            worker,
        )
    }
}

/// Passive recorded hashes, not an executor/approval seal. Live and historical
/// consumers share this exact native data/fingerprint check.
struct InventoryFingerprints {
    input: [u8; 32],
    arguments: Option<[u8; 32]>,
    answer: Option<[u8; 32]>,
    result: Option<[u8; 32]>,
}
fn verify_inventory_fingerprints(
    result: &Value,
    arguments: &Value,
    answer_bytes: &str,
    fingerprints: InventoryFingerprints,
    limits: ContractLimits,
    worker: [u8; 32],
) -> Result<Value, RebornBuildError> {
    let answer = PortAnswer::Return {
        value: brassclaw_host_api::JsonValue::new(result.clone()),
    };
    let input = typed_value_checksum_with_limits(arguments, limits).map_err(invalid)?;
    if fingerprints.input != input
        || fingerprints.arguments != Some(input)
        || fingerprints.answer
            != Some(port_answer_checksum_with_limits(&answer, limits).map_err(invalid)?)
        || fingerprints.result
            != Some(typed_value_checksum_with_limits(result, limits).map_err(invalid)?)
        || strict_json(answer_bytes, limits).map_err(invalid)?
            != serde_json::to_value(&answer).map_err(invalid)?
    {
        return Err(invalid(
            "inventory journal data differs from original dispatch",
        ));
    }
    verify_inventory_report(result, arguments, worker, limits)
}

fn verify_inventory_report(
    result: &Value,
    arguments: &Value,
    worker: [u8; 32],
    limits: ContractLimits,
) -> Result<Value, RebornBuildError> {
    let report = strict_json(
        result
            .as_str()
            .ok_or_else(|| invalid("inventory result must remain JSON string"))?,
        limits,
    )
    .map_err(invalid)?;
    if report.as_object().is_none_or(|o| o.len() != 4)
        || report["format"] != "python-source-inventory/1"
        || report["worker_checksum"] != hex::encode(worker)
        || report["preparation_revision"]
            .as_u64()
            .is_none_or(|r| r == 0)
    {
        return Err(invalid(
            "inventory interpreter or preparation identity differs",
        ));
    }
    let sources = arguments["sources"]
        .as_object()
        .ok_or_else(|| invalid("inventory sources missing"))?;
    let facts = report["sources"]
        .as_object()
        .ok_or_else(|| invalid("inventory facts missing"))?;
    if arguments.as_object().is_none_or(|o| o.len() != 1) || sources.len() != facts.len() {
        return Err(invalid("inventory source membership differs"));
    }
    let typed_facts: BTreeMap<String, brassclaw_monty_host::source_inventory::SourceInspection> =
        serde_json::from_value(report["sources"].clone()).map_err(invalid)?;
    for (id, source) in sources {
        let source = source
            .as_str()
            .ok_or_else(|| invalid("inventory source is not text"))?;
        let fact = typed_facts
            .get(id)
            .ok_or_else(|| invalid("inventory original source omitted"))?;
        let checksum = match fact {
            brassclaw_monty_host::source_inventory::SourceInspection::Inspected { structure } => {
                &structure.source_checksum
            }
            brassclaw_monty_host::source_inventory::SourceInspection::CompileRejected {
                source_checksum,
                ..
            } => source_checksum,
        };
        if checksum != &crate::bootstrap_evidence::checksum(source) {
            return Err(invalid("inventory source checksum differs"));
        }
    }
    Ok(report)
}

#[tokio::test]
async fn retained_review_worker_rejects_replacement_before_inspection() {
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), b"packaged fixture identity").unwrap();
    let image = PackagedExecutableImage::capture(source.path(), 128)
        .await
        .unwrap();
    verified_worker(&image, 128).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(image.path(), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    std::fs::write(image.path(), b"different bytes").unwrap();
    assert!(verified_worker(&image, 128).await.is_err());
}
