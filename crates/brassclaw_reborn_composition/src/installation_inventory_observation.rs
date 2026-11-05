//! Actual controlled installation consumer. No authored submission/queue,
//! semantic approval, own Q1/Q2, active route or general recovery is created.
use super::*;
use crate::{
    bootstrap_evidence as evidence, bootstrap_evidence::io,
    source_inspection_components::PackagedDefinitions,
};
use brassclaw_engine::memory::{
    retained_instruction::{WorkflowClass, compile_retained_recipe},
    retained_tools::prepare_retained_tool_program,
};
use brassclaw_skills::{
    component_revision::REVISION_LIMITS, global_bootstrap_components::packaged_component_id,
    review_submission_store::installation as store,
};
use brassclaw_turns::{TurnRunId, TurnRunRecord, TurnScope, TurnSpawnTreeStateStore, TurnStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

#[cfg(test)]
#[path = "installation_inventory_fault_tests.rs"]
pub(super) mod tests;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    format: String,
    operation_id: Uuid,
    selection: store::InstallationRecordRef,
    subject: store::InstallationRecordRef,
    parser_completion: store::InstallationRecordRef,
    executor_configuration: String,
    executor_configuration_checksum: String,
    input_bytes: String,
    input_checksum: String,
    worker_checksum: String,
    root_reference: ComponentRevisionRef,
    root_source_checksum: String,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionBinding {
    format: String,
    operation: store::InstallationRecordRef,
    run_id: Uuid,
    scope: TurnScope,
    runner_id: Uuid,
    claim_checksum: String,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeJournal {
    invocation_id: Uuid,
    selection_bytes: String,
    arguments_bytes: String,
    answer_bytes: String,
    data_max_depth: u64,
    data_max_nodes: u64,
    data_max_bytes: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Preterminal {
    format: String,
    operation: store::InstallationRecordRef,
    admission: store::InstallationRecordRef,
    selection: store::InstallationRecordRef,
    task_handle: Uuid,
    receipt: String,
    checks: Value,
    native_inventory: Value,
    native_journal: NativeJournal,
    step_fingerprints: Vec<StepFingerprint>,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepFingerprint {
    step: String,
    input_checksum: String,
    result_checksum: Option<String>,
    arguments_checksum: Option<String>,
    answer_checksum: Option<String>,
    max_depth: usize,
    max_nodes: usize,
    max_bytes: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuiescenceRecord {
    format: String,
    operation: store::InstallationRecordRef,
    admission: store::InstallationRecordRef,
    preterminal: store::InstallationRecordRef,
    actual: Value,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completed {
    format: String,
    operation: store::InstallationRecordRef,
    admission: store::InstallationRecordRef,
    preterminal: store::InstallationRecordRef,
    quiescence: store::InstallationRecordRef,
    turn_record: Value,
    outcome: String,
    q1_qualification: bool,
    semantic_approval: bool,
    catalogue_activation: bool,
}
pub(crate) enum InstallationSourceObservations {
    ObservedPositive(Vec<CheckedProfile>),
    CompletedNegative(Vec<CheckedProfile>),
    UnavailableBeforeFirstExecution { reason: &'static str },
}
pub(crate) struct CheckedProfile {
    context: Arc<Context>,
    completion_bytes: String,
}
impl InstallationSourceObservations {
    /// Uses only the supplied final-boot snapshot, never a nested pool/Turn read.
    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        match self {
            Self::ObservedPositive(profiles) | Self::CompletedNegative(profiles) => {
                if profiles.len() != 2 {
                    return Err(invalid("both installation checking profiles required"));
                }
                let mut positive = true;
                for (profile, expected) in profiles.iter().zip([
                    store::InstallationProfile::Structure,
                    store::InstallationProfile::Pure,
                ]) {
                    if profile.context.selection.profile() != expected {
                        return Err(invalid("installation checking profiles differ"));
                    }
                    let read = profile
                        .context
                        .read_completed(tx)
                        .await?
                        .ok_or_else(|| invalid("installation completion missing"))?;
                    if read.0 != profile.completion_bytes {
                        return Err(invalid("installation original completion changed"));
                    }
                    positive &= read.1;
                }
                if positive != matches!(self, Self::ObservedPositive(_)) {
                    return Err(invalid("installation observation status differs"));
                }
            }
            Self::UnavailableBeforeFirstExecution { reason } => {
                if !matches!(
                    *reason,
                    "instance_tool_blocked" | "fixture_adapter_unavailable"
                ) {
                    return Err(invalid("unrecognized installation unavailability"));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn status(&self) -> &'static str {
        match self {
            Self::ObservedPositive(_) => "source_observed_positive_not_qualified",
            Self::CompletedNegative(_) => "source_completed_negative_not_ready",
            Self::UnavailableBeforeFirstExecution { .. } => "source_observation_unavailable",
        }
    }
}

pub(crate) async fn observe_installation(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    definitions: PackagedDefinitions,
    reply: &RetainedToolProgram,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
) -> Result<InstallationSourceObservations, RebornBuildError> {
    #[cfg(test)]
    let audit_pool = pool.clone();
    let result = observe_installation_inner(
        pool,
        kernel,
        worker,
        definitions,
        reply,
        executor,
        preparation,
        storage,
        root,
    )
    .await;
    #[cfg(test)]
    tests::after_consumer(&audit_pool, &result).await?;
    result
}

async fn observe_installation_inner(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    mut definitions: PackagedDefinitions,
    reply: &RetainedToolProgram,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
) -> Result<InstallationSourceObservations, RebornBuildError> {
    let revisions = brassclaw_skills::revision_store::PgComponentRevisionStore::new(pool.clone());
    for draft in crate::builtin_bootstrap::validator_check_drafts() {
        let draft = draft.map_err(invalid)?;
        io::bounded(storage.as_ref(), definitions.retain(&revisions, &draft)).await?;
    }
    let mut programs = Vec::new();
    for profile in [
        store::InstallationProfile::Structure,
        store::InstallationProfile::Pure,
    ] {
        let id = match profile {
            store::InstallationProfile::Structure => "validator:python-structure:recipe",
            store::InstallationProfile::Pure => "validator:python-pure-structure:recipe",
        };
        let graph = io::bounded(
            storage.as_ref(),
            definitions.snapshot(pool.clone(), packaged_component_id(id)),
        )
        .await?;
        let program = Arc::new(
            prepare_retained_tool_program(
                compile_retained_recipe(
                    graph,
                    packaged_component_id(id),
                    "selected",
                    WorkflowClass::Deterministic,
                )
                .map_err(invalid)?,
            )
            .map_err(invalid)?,
        );
        let (_, binding) = require_inventory_binding(&program)?;
        require_packaged_inventory_profile(
            &program,
            binding,
            match profile {
                store::InstallationProfile::Structure => InventoryReviewProfile::Structure,
                store::InstallationProfile::Pure => InventoryReviewProfile::Pure,
            },
        )?;
        programs.push((profile, program));
    }
    let (_, binding) = require_inventory_binding(&programs[0].1)?;
    let enabled = io::bounded(storage.as_ref(), async {
        pool.get()
            .await
            .map_err(invalid)?
            .query_opt(
                "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                &[&binding.tool().uuid],
            )
            .await
            .map_err(invalid)?
            .map(|row| row.get::<_, bool>(0))
            .ok_or_else(|| invalid("source inspection policy missing"))
    })
    .await?;
    let identity=json!({"components":binding.combination(),"association":binding.association().exact_bytes(),
        "worker_checksum":hex::encode(worker.checksum()),"qualifier":evidence::checksum(include_str!("bootstrap_inventory_qualification.rs")),
        "completion":evidence::checksum(include_str!("bootstrap_inventory_completion.rs")),"primitives":evidence::checksum(include_str!("bootstrap_inventory_primitives.rs")),
        "io":evidence::checksum(include_str!("bootstrap_evidence_io.rs")),"contained_close":evidence::checksum(include_str!("bootstrap_validation.rs")),"executor":evidence::checksum(include_str!("global_monty_startup.rs")),"root_reference":root.reference()}).to_string();
    let qualification_id = Uuid::new_v5(&binding.skill().uuid, identity.as_bytes());
    let cached = qualification::qualify_packaged(
        pool.clone(),
        kernel.clone(),
        worker.clone(),
        &programs[0].1,
        preparation,
        reply,
        qualification_id,
        executor.clone(),
        storage.clone(),
        false,
        root.clone(),
    )
    .await?;
    let parser = match cached {
        Some(parser) => parser,
        None if !enabled => {
            return Ok(
                InstallationSourceObservations::UnavailableBeforeFirstExecution {
                    reason: "instance_tool_blocked",
                },
            );
        }
        None => {
            let native = serde_json::from_value(
                programs[0].1.inputs().instruction().snapshot().revisions()[&binding.tool().uuid]
                    .draft()
                    .document()["implementation"]["identity"]
                    .clone(),
            )
            .map_err(invalid)?;
            match io::bounded(storage.as_ref(), async {
                Ok(kernel.inventory_fixture_preparation(native).await)
            })
            .await?
            {
                Ok(_facts) => {}
                Err(brassclaw_host_runtime::RetainedCapabilityError::FixtureAdapterUnavailable) => {
                    return Ok(
                        InstallationSourceObservations::UnavailableBeforeFirstExecution {
                            reason: "fixture_adapter_unavailable",
                        },
                    );
                }
                Err(error) => return Err(invalid(error)),
            }
            qualification::qualify_packaged(
                pool.clone(),
                kernel.clone(),
                worker.clone(),
                &programs[0].1,
                preparation,
                reply,
                qualification_id,
                executor.clone(),
                storage.clone(),
                true,
                root.clone(),
            )
            .await?
            .ok_or_else(|| invalid("permitted installation parser qualification unavailable"))?
        }
    };
    #[cfg(test)]
    tests::before_subject(&pool).await?;
    let package = store::InstallationPackageContext {
        factory_checksum: evidence::checksum(&json!({
            "format":"installation-validator-owner-code/1",
            "consumer":evidence::checksum(include_str!("installation_inventory_observation.rs")),
            "native_observer":evidence::checksum(include_str!("validator_inventory_review.rs")),
            "receipt":evidence::checksum(include_str!("pg_review_admission_source.rs")),
            "coverage":evidence::checksum(include_str!("../../brassclaw_engine/src/memory/retained_validator.rs")),
            "subject_inputs":evidence::checksum(include_str!("../../brassclaw_engine/src/memory/retained_validator_subject.rs")),
            "subject_store":evidence::checksum(include_str!("../../brassclaw_skills/src/review_installation_subject.rs")),
            "contained_close":evidence::checksum(include_str!("bootstrap_validation.rs")),
        }).to_string()),
        worker_checksum: hex::encode(worker.checksum()),
        association_checksum: evidence::checksum(binding.association().exact_bytes()),
        parser_completion: parser.proof.record_reference(),
    };
    let subject_graph = io::bounded(
        storage.as_ref(),
        definitions.snapshot(
            pool.clone(),
            packaged_component_id("validator:structural-presence"),
        ),
    )
    .await?;
    #[cfg(test)]
    let subject_graph = tests::negative_subject(&pool, subject_graph).await?;
    let candidate = subject_graph.revisions()
        [&packaged_component_id("validator:structural-presence")]
        .reference();
    let expected =
        brassclaw_skills::validator_components::structural_presence_draft(candidate.uuid)
            .map_err(invalid)?;
    #[cfg(test)]
    let expected = tests::expected_subject(expected)?;
    if subject_graph.revisions()[&candidate.uuid]
        .draft()
        .exact_bytes()
        != expected.exact_bytes()
        || subject_graph.revisions().len() != 1
    {
        return Err(invalid(
            "installation subject differs from actual packaged root/closure",
        ));
    }
    // Existing immutable-store transport bound. Invocation uses its original
    // VM profile; today's task-data limits cannot rewrite historical evidence.
    let limits = REVISION_LIMITS;
    let subject_draft =
        store::subject_record(&subject_graph, candidate, package, limits).map_err(invalid)?;
    retain_draft(&pool, storage.as_ref(), &subject_draft).await?;
    let subject = io::bounded(storage.as_ref(), async {
        let mut client = io::EvidenceClient::acquire(&pool).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        let subject = store::read_subject_in_transaction(&tx, subject_draft.reference().id, limits)
            .await
            .map_err(invalid)?
            .ok_or_else(|| invalid("installation subject missing"))?;
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(Arc::new(subject))
    })
    .await?;
    let mut checked = Vec::new();
    let mut positive = true;
    for (profile, program) in programs {
        parser.require_program(&program)?;
        let selection_draft = store::selection_record(
            &subject,
            program
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(invalid)?
                .exact_bytes(),
            profile,
            limits,
        )
        .map_err(invalid)?;
        retain_draft(&pool, storage.as_ref(), &selection_draft).await?;
        let selection = io::bounded(storage.as_ref(), async {
            let mut client = io::EvidenceClient::acquire(&pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            let saved =
                store::read_selection_in_transaction(&tx, selection_draft.reference().id, limits)
                    .await
                    .map_err(invalid)?
                    .ok_or_else(|| invalid("installation selection missing"))?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(Arc::new(saved))
        })
        .await?;
        let context = Arc::new(Context {
            pool: pool.clone(),
            storage: storage.clone(),
            kernel: kernel.clone(),
            worker: worker.clone(),
            parser: parser.clone(),
            subject: subject.clone(),
            selection,
            program,
            root: root.clone(),
            operation: Uuid::new_v5(
                &selection_draft.reference().id,
                b"controlled-installation-observation",
            ),
        });
        let original = context.read_original().await?;
        let (completion_bytes, pass) = match original {
            Some(complete) => complete,
            None if !enabled => {
                return Ok(
                    InstallationSourceObservations::UnavailableBeforeFirstExecution {
                        reason: "instance_tool_blocked",
                    },
                );
            }
            None => context.execute(executor.clone(), preparation).await?,
        };
        positive &= pass;
        checked.push(CheckedProfile {
            context,
            completion_bytes,
        });
    }
    Ok(if positive {
        InstallationSourceObservations::ObservedPositive(checked)
    } else {
        InstallationSourceObservations::CompletedNegative(checked)
    })
}
fn data_limits(
    executor: &crate::global_monty_startup::ControlledExecutorConfiguration,
) -> ContractLimits {
    executor.capacity().data_limits
}
async fn retain_draft(
    pool: &Arc<PgPool>,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    draft: &store::InstallationRecordDraft,
) -> Result<(), RebornBuildError> {
    io::retain_record(pool, storage, draft.reference().id, draft.bytes()).await
}
struct Context {
    pool: Arc<PgPool>,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: Arc<PackagedExecutableImage>,
    parser: Arc<QualifiedSourceInventoryUsage>,
    subject: Arc<store::RetainedInstallationValidatorSubject>,
    selection: Arc<store::RetainedInstallationValidatorSelection>,
    program: Arc<RetainedToolProgram>,
    operation: Uuid,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
}
impl Context {
    fn id(&self, stage: &str) -> Uuid {
        Uuid::new_v5(&self.operation, stage.as_bytes())
    }
    fn record<T: Serialize>(&self, id: Uuid, report: &T) -> Result<String, RebornBuildError> {
        let components=self.program.inputs().instruction().snapshot().revisions().values().map(|r|{let r=r.reference();json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})}).collect::<Vec<_>>();
        Ok(json!({"format":"component-review-evidence/1","evidence_id":id,"kind":"behavior","validation_mode":"system_seed",
            "association_checksum":self.subject.package_context().association_checksum,"components":components,"succeeded":false,"reviewed_evidence":[],"report":report}).to_string())
    }
    async fn read<T: serde::de::DeserializeOwned + Serialize>(
        &self,
        tx: &Transaction<'_>,
        id: Uuid,
    ) -> Result<Option<(String, T)>, RebornBuildError> {
        let Some(bytes) = evidence::read_evidence(tx, id).await? else {
            return Ok(None);
        };
        let value = strict_json(&bytes, REVISION_LIMITS).map_err(invalid)?;
        let report: T = serde_json::from_value(value["report"].clone()).map_err(invalid)?;
        if self.record(id, &report)? != bytes {
            return Err(invalid("installation immutable record fields/bytes differ"));
        }
        Ok(Some((bytes, report)))
    }
    async fn read_original(&self) -> Result<Option<(String, bool)>, RebornBuildError> {
        io::bounded(self.storage.as_ref(), async {
            let mut client = io::EvidenceClient::acquire(&self.pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            let result = self.read_completed(&tx).await?;
            if result.is_none()
                && evidence::read_evidence(&tx, self.operation)
                    .await?
                    .is_some()
            {
                return Err(invalid(
                    "installation original incomplete; reconcile without admission or replay",
                ));
            }
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(result)
        })
        .await
    }
    async fn read_completed(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<Option<(String, bool)>, RebornBuildError> {
        let Some((completed_bytes, completed)) =
            self.read::<Completed>(tx, self.id("completed")).await?
        else {
            return Ok(None);
        };
        let (operation_bytes, operation) = self
            .read::<Operation>(tx, self.operation)
            .await?
            .ok_or_else(|| invalid("installation original intent missing"))?;
        self.verify_operation(&operation)?;
        brassclaw_skills::revision_store::PgComponentRevisionStore::read_revision_in_transaction(
            tx,
            operation.root_reference,
        )
        .await
        .map_err(invalid)?;
        let (admission_bytes, admission) = self
            .read::<AdmissionBinding>(tx, self.id("original-admission"))
            .await?
            .ok_or_else(|| invalid("installation original admission missing"))?;
        let (preterminal_bytes, preterminal) = self
            .read::<Preterminal>(tx, self.id("preterminal"))
            .await?
            .ok_or_else(|| invalid("installation original observation missing"))?;
        let (quiescence_bytes, quiescence) = self
            .read::<QuiescenceRecord>(tx, self.id("quiescence"))
            .await?
            .ok_or_else(|| invalid("installation original quiescence missing"))?;
        let op = reference(self.operation, &operation_bytes);
        let admitted = reference(self.id("original-admission"), &admission_bytes);
        let observed = reference(self.id("preterminal"), &preterminal_bytes);
        if admission.operation != op
            || completed.operation != op
            || preterminal.operation != op
            || completed.admission != admitted
            || preterminal.admission != admitted
            || completed.preterminal != observed
            || preterminal.selection != *self.selection.reference()
        {
            return Err(invalid("installation original evidence graph differs"));
        }
        if completed.quiescence != reference(self.id("quiescence"), &quiescence_bytes)
            || quiescence.operation != op
            || quiescence.admission != admitted
            || quiescence.preterminal != observed
            || quiescence.format != "installation-validator-quiescence/1"
            || quiescence.q1_qualification
            || quiescence.semantic_approval
            || quiescence.catalogue_activation
        {
            return Err(invalid("installation quiescence evidence graph differs"));
        }
        let original_executor =
            crate::global_monty_startup::ControlledExecutorConfiguration::restore_original(
                &operation.executor_configuration,
            )?;
        self.verify_preterminal(&preterminal, data_limits(&original_executor))?;
        self.verify_closed_admission(tx, &admission).await?;
        self.verify_journal(tx, &admission, &preterminal.native_journal)
            .await?;
        let restored = store::read_selection_in_transaction(
            tx,
            self.selection.reference().id,
            REVISION_LIMITS,
        )
        .await
        .map_err(invalid)?
        .ok_or_else(|| invalid("installation selected graph missing"))?;
        if restored.bytes() != self.selection.bytes() {
            return Err(invalid("installation selected graph changed"));
        }
        crate::bootstrap_quiescence_wire::verify(
            &quiescence.actual,
            &self.program,
            &json!(preterminal.task_handle),
            &preterminal.receipt,
        )?;
        if quiescence.actual["execution"]["controlled_executor_configuration"]
            != operation.executor_configuration
            || quiescence.actual["execution"]["controlled_executor_configuration_checksum"]
                != operation.executor_configuration_checksum
        {
            return Err(invalid("installation original executor selection differs"));
        }
        let turn: TurnRunRecord = exact(&completed.turn_record)?;
        verify_turn(&turn, &admission)?;
        let pass = preterminal.checks["aggregation"]["pass"]
            .as_bool()
            .ok_or_else(|| invalid("installation aggregate missing"))?;
        if completed.format != "installation-validator-completed/1"
            || completed.outcome
                != if pass {
                    "observed_positive"
                } else {
                    "completed_negative"
                }
            || completed.q1_qualification
            || completed.semantic_approval
            || completed.catalogue_activation
        {
            return Err(invalid(
                "installation completed outcome or non-grant markers differ",
            ));
        }
        Ok(Some((completed_bytes, pass)))
    }
    fn verify_operation(&self, op: &Operation) -> Result<(), RebornBuildError> {
        let executor =
            crate::global_monty_startup::ControlledExecutorConfiguration::restore_original(
                &op.executor_configuration,
            )?;
        let coverage =
            RetainedValidatorCoverage::from_inputs(self.program.inputs()).map_err(invalid)?;
        let expected = coverage
            .installation_task_inputs(&self.subject, self.program.inputs(), data_limits(&executor))
            .map_err(invalid)?;
        let inputs: Value =
            strict_json(&op.input_bytes, data_limits(&executor)).map_err(invalid)?;
        if op.format != "installation-validator-operation/1"
            || op.operation_id != self.operation
            || op.selection != *self.selection.reference()
            || op.subject != *self.subject.reference()
            || op.parser_completion != self.parser.proof.record_reference()
            || op.executor_configuration_checksum != executor.checksum()
            || op.input_checksum != evidence::checksum(&op.input_bytes)
            || serde_json::to_string(&expected).map_err(invalid)? != op.input_bytes
            || inputs != expected
            || op.worker_checksum != hex::encode(self.worker.checksum())
            || op.root_reference != self.root.reference()
            || op.root_source_checksum != hex::encode(self.root.source_checksum())
            || op.q1_qualification
            || op.semantic_approval
            || op.catalogue_activation
        {
            return Err(invalid(
                "installation original inputs/package/executor differ",
            ));
        }
        Ok(())
    }
    fn verify_preterminal(
        &self,
        pre: &Preterminal,
        limits: ContractLimits,
    ) -> Result<(), RebornBuildError> {
        use brassclaw_engine::memory::retained_validator::ValidatorInputFingerprint;
        let coverage =
            RetainedValidatorCoverage::from_inputs(self.program.inputs()).map_err(invalid)?;
        let order = &self.program.program().steplist;
        let expected_steps = order
            .iter()
            .filter(|step| step.step_id != coverage.receipt_step())
            .map(|step| step.step_id.as_str())
            .collect::<Vec<_>>();
        let results = crate::pg_review_admission_source::recorded_receipt_results(
            crate::pg_review_admission_source::ValidatorReceiptProfile::Inventory,
            &pre.receipt,
            &expected_steps,
            coverage.receipt_step(),
            limits,
        )
        .map_err(invalid)?;
        let mut fingerprints = BTreeMap::new();
        let (native_step, _) = require_inventory_binding(&self.program)?;
        let native_arguments =
            strict_json(&pre.native_journal.arguments_bytes, limits).map_err(invalid)?;
        let native_result = results
            .get(native_step)
            .ok_or_else(|| invalid("installation native result missing"))?;
        if pre.step_fingerprints.len() != order.len() {
            return Err(invalid("installation step evidence incomplete"));
        }
        for step in &pre.step_fingerprints {
            let captured = ContractLimits {
                max_depth: step.max_depth,
                max_nodes: step.max_nodes,
                max_bytes: step.max_bytes,
            };
            // Each observation belongs to this original executor capacity. A
            // later settings reduction cannot reinterpret historical facts.
            if captured.max_depth != limits.max_depth
                || captured.max_nodes != limits.max_nodes
                || captured.max_bytes != limits.max_bytes
            {
                return Err(invalid("installation captured step capacity differs"));
            }
            let input: [u8; 32] = hex::decode(&step.input_checksum)
                .map_err(invalid)?
                .try_into()
                .map_err(|_| invalid("installation input digest length differs"))?;
            if fingerprints
                .insert(
                    step.step.clone(),
                    ValidatorInputFingerprint {
                        checksum: input,
                        limits: captured,
                    },
                )
                .is_some()
            {
                return Err(invalid("installation duplicate step evidence"));
            }
            let result = results
                .get(&step.step)
                .ok_or_else(|| invalid("installation step result missing"))?;
            if step.result_checksum
                != Some(hex::encode(
                    typed_value_checksum_with_limits(result, captured).map_err(invalid)?,
                ))
            {
                return Err(invalid("installation step result fingerprint differs"));
            }
            if step.step == native_step {
                if pre.native_journal.data_max_depth
                    != u64::try_from(captured.max_depth).map_err(invalid)?
                    || pre.native_journal.data_max_nodes
                        != u64::try_from(captured.max_nodes).map_err(invalid)?
                    || pre.native_journal.data_max_bytes
                        != u64::try_from(captured.max_bytes).map_err(invalid)?
                {
                    return Err(invalid("installation native captured capacity differs"));
                }
                let digest = |value: &str| -> Result<[u8; 32], RebornBuildError> {
                    hex::decode(value)
                        .map_err(invalid)?
                        .try_into()
                        .map_err(|_| invalid("installation digest length differs"))
                };
                let report = verify_inventory_fingerprints(
                    native_result,
                    &native_arguments,
                    &pre.native_journal.answer_bytes,
                    InventoryFingerprints {
                        input,
                        arguments: step.arguments_checksum.as_deref().map(digest).transpose()?,
                        answer: step.answer_checksum.as_deref().map(digest).transpose()?,
                        result: step.result_checksum.as_deref().map(digest).transpose()?,
                    },
                    captured,
                    self.worker.checksum(),
                )?;
                if pre.native_inventory != report {
                    return Err(invalid("installation native report differs"));
                }
            } else if step.arguments_checksum.is_some() || step.answer_checksum.is_some() {
                return Err(invalid("installation pure step has native evidence"));
            }
        }
        coverage
            .verify_installation_input_fingerprints(
                &self.subject,
                &self.selection,
                &self.program,
                &results,
                &fingerprints,
                limits,
            )
            .map_err(invalid)?;
        let mut expected = coverage
            .recorded_result_summary(&self.program, &results, limits)
            .map_err(invalid)?;
        let report = expected
            .as_object_mut()
            .ok_or_else(|| invalid("installation summary invalid"))?;
        report.insert(
            "format".into(),
            json!("observed-installation-validator-checks/1"),
        );
        report.insert(
            "scope".into(),
            json!("controlled-installation-source-review"),
        );
        report.insert("subject_id".into(), json!(self.subject.reference().id));
        report.insert(
            "subject_checksum".into(),
            json!(self.subject.reference().checksum),
        );
        report.insert("task_handle".into(), json!(pre.task_handle));
        report.insert("catalogue_activation".into(), json!(false));
        if pre.format != "installation-validator-preterminal/1"
            || pre.task_handle.is_nil()
            || pre.q1_qualification
            || pre.semantic_approval
            || pre.catalogue_activation
            || pre.checks != expected
        {
            return Err(invalid("installation actual rule scope differs"));
        }
        Ok(())
    }
    async fn verify_closed_admission(
        &self,
        tx: &Transaction<'_>,
        binding: &AdmissionBinding,
    ) -> Result<(), RebornBuildError> {
        if binding.format != "installation-validator-admission/1"
            || binding.run_id.is_nil()
            || binding.runner_id.is_nil()
            || binding.q1_qualification
            || binding.semantic_approval
            || binding.catalogue_activation
        {
            return Err(invalid("installation admission shape differs"));
        }
        let row=tx.query_one("SELECT phase,outcome,scope,runner_id,claim_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1",&[&binding.run_id]).await.map_err(invalid)?;
        if row.get::<_, String>(0) != "settled"
            || row.get::<_, Option<Value>>(1)
                != Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
            || row.get::<_, Value>(2) != json!(binding.scope)
            || row.get::<_, Uuid>(3) != binding.runner_id
            || hex::encode(row.get::<_, Vec<u8>>(4)) != binding.claim_checksum
        {
            return Err(invalid("installation original Admission not closed"));
        }
        Ok(())
    }
    async fn verify_journal(
        &self,
        tx: &Transaction<'_>,
        binding: &AdmissionBinding,
        journal: &NativeJournal,
    ) -> Result<(), RebornBuildError> {
        let actual = read_journal(tx, binding.run_id, &self.program).await?;
        if serde_json::to_value(actual).map_err(invalid)?
            != serde_json::to_value(journal).map_err(invalid)?
        {
            return Err(invalid("installation original native ACK differs"));
        }
        Ok(())
    }
    async fn execute(
        self: &Arc<Self>,
        executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
        preparation: brassclaw_monty_host::utility::UtilityLimits,
    ) -> Result<(String, bool), RebornBuildError> {
        self.parser.proof.verify(&self.pool).await?;
        let limits = data_limits(&executor);
        let coverage =
            RetainedValidatorCoverage::from_inputs(self.program.inputs()).map_err(invalid)?;
        let inputs = coverage
            .installation_task_inputs(&self.subject, self.program.inputs(), limits)
            .map_err(invalid)?;
        let input_bytes = serde_json::to_string(&inputs).map_err(invalid)?;
        let operation = Operation {
            format: "installation-validator-operation/1".into(),
            operation_id: self.operation,
            selection: self.selection.reference().clone(),
            subject: self.subject.reference().clone(),
            parser_completion: self.parser.proof.record_reference(),
            executor_configuration: executor.exact_bytes().to_owned(),
            executor_configuration_checksum: executor.checksum(),
            input_checksum: evidence::checksum(&input_bytes),
            input_bytes,
            worker_checksum: hex::encode(self.worker.checksum()),
            root_reference: self.root.reference(),
            root_source_checksum: hex::encode(self.root.source_checksum()),
            q1_qualification: false,
            semantic_approval: false,
            catalogue_activation: false,
        };
        self.verify_operation(&operation)?;
        let operation_bytes = self.record(self.operation, &operation)?;
        #[cfg(test)]
        tests::before_intent(self).await?;
        // First stage is durable before even reserving the contained claim.
        io::retain_new_intent(
            &self.pool,
            self.storage.as_ref(),
            self.operation,
            &operation_bytes,
        )
        .await?;
        let source_reference = format!("installation-source-observation:{}", self.operation);
        let admitted = io::bounded(
            self.storage.as_ref(),
            crate::bootstrap_validation::admit(
                self.pool.clone(),
                "installation-source-observer",
                &source_reference,
                None,
            ),
        )
        .await?;
        let binding = AdmissionBinding {
            format: "installation-validator-admission/1".into(),
            operation: reference(self.operation, &operation_bytes),
            run_id: admitted.attempt.run_id.as_uuid(),
            scope: admitted.chat_context()?.scope.clone(),
            runner_id: admitted.attempt.runner_id.as_uuid(),
            claim_checksum: hex::encode(Sha256::digest(
                serde_json::to_vec(&admitted.attempt.lease_token).map_err(invalid)?,
            )),
            q1_qualification: false,
            semantic_approval: false,
            catalogue_activation: false,
        };
        let admission_bytes = self.record(self.id("original-admission"), &binding)?;
        let quiescence_proof = OnceLock::new();
        let observed = async {
            io::retain_record(
                &self.pool,
                self.storage.as_ref(),
                self.id("original-admission"),
                &admission_bytes,
            )
            .await?;
            let restored = Arc::new(
                RetainedValidatorCoverage::restore_installation_preparation(&self.selection)
                    .map_err(invalid)?,
            );
            self.parser.require_program(&restored)?;
            let worker =
                verified_worker(&self.parser.worker, preparation.max_native_artifact_bytes).await?;
            let inspected = Arc::new(
                InspectedRetainedProgram::inspect_with_limits(
                    RetainedProgram::Tools(restored.clone()),
                    &worker.path(),
                    preparation,
                )
                .await
                .map_err(invalid)?,
            );
            let (_, tool_binding) = self.parser.require_program(&restored)?;
            require_packaged_inventory_profile(
                &restored,
                tool_binding,
                match self.selection.profile() {
                    store::InstallationProfile::Structure => InventoryReviewProfile::Structure,
                    store::InstallationProfile::Pure => InventoryReviewProfile::Pure,
                },
            )?;
            let tools = crate::installed_monty_catalogue::qualified_inventory_tools(
                self.pool.clone(),
                self.kernel.clone(),
                admitted.host.clone(),
                admitted.admission.clone(),
                restored,
                &self.parser,
            )?;
            let source = Arc::new(InstallationSource {
                context: self.clone(),
                reference: source_reference,
                inspected,
                coverage,
                inputs,
                host: admitted.host.clone(),
                tools,
                binding: binding.clone(),
                admission_bytes: admission_bytes.clone(),
                received: OnceLock::new(),
                limits,
            });
            let observation = crate::bootstrap_recipe_flow::execute(
                &admitted,
                crate::bootstrap_recipe_flow::Request::contained(
                    worker.path(),
                    executor,
                    &self.root,
                    source.clone(),
                )?,
            )
            .await?;
            let (actual, proof) = observation.quiescence_proof()?;
            quiescence_proof
                .set(proof)
                .map_err(|_| invalid("original quiescence already observed"))?;
            let preterminal_bytes = source
                .received
                .get()
                .ok_or_else(|| invalid("installation actual observation missing"))?
                .clone();
            let quiescence = QuiescenceRecord {
                format: "installation-validator-quiescence/1".into(),
                operation: binding.operation.clone(),
                admission: reference(self.id("original-admission"), &admission_bytes),
                preterminal: reference(self.id("preterminal"), &preterminal_bytes),
                actual,
                q1_qualification: false,
                semantic_approval: false,
                catalogue_activation: false,
            };
            let quiescence_bytes = self.record(self.id("quiescence"), &quiescence)?;
            io::retain_record(
                &self.pool,
                self.storage.as_ref(),
                self.id("quiescence"),
                &quiescence_bytes,
            )
            .await?;
            #[cfg(test)]
            tests::after_quiescence(self, &admitted).await?;
            Ok::<_, RebornBuildError>((preterminal_bytes, quiescence_bytes))
        }
        .await;
        let close = match quiescence_proof.get() {
            Some(proof) => {
                io::bounded(
                    self.storage.as_ref(),
                    crate::bootstrap_validation::settle_quiescent_closed(
                        &self.pool, &admitted, proof,
                    ),
                )
                .await
            }
            None => io::bounded(
                self.storage.as_ref(),
                crate::bootstrap_validation::settle_closed(&admitted),
            )
            .await
            .map(|_| TurnStatus::Completed),
        };
        let close = crate::bootstrap_validation::read_source_close(
            &self.pool,
            &admitted,
            quiescence_proof.get(),
            close,
            self.storage.as_ref(),
        )
        .await;
        let (preterminal_bytes, quiescence_bytes) = match observed {
            Ok(facts) => facts,
            Err(operation) => {
                return Err(match close {
                    Ok(record) => invalid(format!(
                        "{operation}; original cleanup confirmed {:?}",
                        record.status
                    )),
                    Err(cleanup) => invalid(format!("{operation}; {cleanup}")),
                });
            }
        };
        let turn = close?;
        if turn.status == TurnStatus::Cancelled {
            return Err(invalid(
                "installation observation cancelled; original cleanup confirmed Cancelled",
            ));
        }
        verify_turn(&turn, &binding)?;
        let preterminal: Preterminal = serde_json::from_value(
            strict_json(&preterminal_bytes, REVISION_LIMITS).map_err(invalid)?["report"].clone(),
        )
        .map_err(invalid)?;
        let pass = preterminal.checks["aggregation"]["pass"]
            .as_bool()
            .ok_or_else(|| invalid("installation actual aggregate missing"))?;
        let completed = Completed {
            format: "installation-validator-completed/1".into(),
            operation: binding.operation.clone(),
            admission: reference(self.id("original-admission"), &admission_bytes),
            preterminal: reference(self.id("preterminal"), &preterminal_bytes),
            quiescence: reference(self.id("quiescence"), &quiescence_bytes),
            turn_record: serde_json::to_value(&turn).map_err(invalid)?,
            outcome: if pass {
                "observed_positive"
            } else {
                "completed_negative"
            }
            .into(),
            q1_qualification: false,
            semantic_approval: false,
            catalogue_activation: false,
        };
        // Verify closed Admission/native facts before publishing the trusted
        // immutable past-Turn ACK. No caller JSON can reach this publisher.
        io::bounded(self.storage.as_ref(), async {
            let mut client = io::EvidenceClient::acquire(&self.pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            self.verify_closed_admission(&tx, &binding).await?;
            self.verify_journal(&tx, &binding, &preterminal.native_journal)
                .await?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            Ok(())
        })
        .await?;
        let completed_bytes = self.record(self.id("completed"), &completed)?;
        io::retain_record(
            &self.pool,
            self.storage.as_ref(),
            self.id("completed"),
            &completed_bytes,
        )
        .await?;
        self.read_original()
            .await?
            .ok_or_else(|| invalid("installation complete original readback missing"))
    }
}

struct InstallationSource {
    context: Arc<Context>,
    reference: String,
    inspected: Arc<InspectedRetainedProgram>,
    coverage: RetainedValidatorCoverage,
    inputs: Value,
    host: Arc<MontyTaskHost>,
    tools: Arc<dyn RetainedToolPort>,
    binding: AdmissionBinding,
    admission_bytes: String,
    received: OnceLock<String>,
    limits: ContractLimits,
}
#[async_trait]
impl InternalRecipeSource for InstallationSource {
    fn name(&self) -> &str {
        "contained-packaged-source-observation"
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
        let result = async {
            let (checks, results) = crate::pg_review_admission_source::InstallationReceiptSubject {
                subject: &self.context.subject,
                selection: &self.context.selection,
                inspected: &self.inspected,
                coverage: &self.coverage,
                limits: self.limits,
            }
            .observe(receipt, selected, execution)
            .map_err(invalid)?;
            let (step, _) = self.context.parser.require_program(&self.context.program)?;
            let native = io::bounded(
                self.context.storage.as_ref(),
                NativeInventoryObservation {
                    pool: &self.context.pool,
                    host: &self.host,
                    program: &self.context.program,
                    step,
                    worker: self.context.worker.checksum(),
                    limits: self.limits,
                }
                .verify(execution, &results[step]),
            )
            .await?;
            let journal = io::bounded(self.context.storage.as_ref(), async {
                let mut client = io::EvidenceClient::acquire(&self.context.pool).await?;
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .map_err(invalid)?;
                let record = read_journal(&tx, self.binding.run_id, &self.context.program).await?;
                tx.commit().await.map_err(invalid)?;
                client.recycle_after_commit();
                Ok(record)
            })
            .await?;
            let steps = execution
                .observations()
                .iter()
                .map(|(id, proof)| StepFingerprint {
                    step: id.clone(),
                    input_checksum: hex::encode(proof.input_checksum()),
                    result_checksum: proof.result_checksum().map(hex::encode),
                    arguments_checksum: proof.arguments_checksum().map(hex::encode),
                    answer_checksum: proof.answer_checksum().map(hex::encode),
                    max_depth: proof.data_limits().max_depth,
                    max_nodes: proof.data_limits().max_nodes,
                    max_bytes: proof.data_limits().max_bytes,
                })
                .collect();
            let observed = Preterminal {
                format: "installation-validator-preterminal/1".into(),
                operation: self.binding.operation.clone(),
                admission: reference(self.context.id("original-admission"), &self.admission_bytes),
                selection: self.context.selection.reference().clone(),
                task_handle: serde_json::from_value(
                    serde_json::to_value(execution.task_handle()).map_err(invalid)?,
                )
                .map_err(invalid)?,
                receipt: receipt.to_owned(),
                checks: checks.report().clone(),
                native_inventory: native,
                native_journal: journal,
                step_fingerprints: steps,
                q1_qualification: false,
                semantic_approval: false,
                catalogue_activation: false,
            };
            self.context.verify_preterminal(&observed, self.limits)?;
            let bytes = self
                .context
                .record(self.context.id("preterminal"), &observed)?;
            self.received
                .set(bytes.clone())
                .map_err(|_| invalid("installation observation already received"))?;
            io::retain_record(
                &self.context.pool,
                self.context.storage.as_ref(),
                self.context.id("preterminal"),
                &bytes,
            )
            .await
        }
        .await;
        result.map_err(|_| failure())
    }
}
fn reference(id: Uuid, bytes: &str) -> store::InstallationRecordRef {
    store::InstallationRecordRef {
        id,
        checksum: evidence::checksum(bytes),
    }
}
fn exact<T: serde::de::DeserializeOwned + Serialize>(value: &Value) -> Result<T, RebornBuildError> {
    let decoded: T = serde_json::from_value(value.clone()).map_err(invalid)?;
    if serde_json::to_value(&decoded).map_err(invalid)? != *value {
        return Err(invalid("installation original fields normalized"));
    }
    Ok(decoded)
}
fn verify_turn(turn: &TurnRunRecord, binding: &AdmissionBinding) -> Result<(), RebornBuildError> {
    if turn.run_id.as_uuid() != binding.run_id
        || turn.scope != binding.scope
        || turn.status != TurnStatus::Completed
        || turn.claim_count != 1
        || turn.runner_id.is_some()
        || turn.lease_token.is_some()
        || turn.lease_expires_at.is_some()
        || turn.failure.is_some()
    {
        return Err(invalid("installation original Turn close unacknowledged"));
    }
    Ok(())
}
async fn read_journal(
    tx: &Transaction<'_>,
    run: Uuid,
    program: &RetainedToolProgram,
) -> Result<NativeJournal, RebornBuildError> {
    let (step, _) = require_inventory_binding(program)?;
    let rows=tx.query("SELECT invocation_id,selection_bytes,arguments_bytes,answer_bytes,phase,attempt_count,data_max_depth,data_max_nodes,data_max_bytes,selection_checksum,arguments_checksum,answer_checksum FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND recipe_id=$2 AND step_id=$3",&[&run,&program.inputs().instruction().recipe().uuid,&step]).await.map_err(invalid)?;
    if rows.len() != 1 {
        return Err(invalid(
            "installation original native invocation missing/extra",
        ));
    }
    let count: i64 = tx
        .query_one(
            "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
            &[&run],
        )
        .await
        .map_err(invalid)?
        .get(0);
    let row = &rows[0];
    let selection: String = row.get(1);
    let arguments: String = row.get(2);
    let answer: Option<String> = row.get(3);
    if count != 1
        || selection != PgMontyAdmission::invocation_selection(program, step).map_err(invalid)?
        || evidence::checksum(&selection) != row.get::<_, String>(9)
        || evidence::checksum(&arguments) != row.get::<_, String>(10)
        || answer.as_ref().map(|v| evidence::checksum(v)) != row.get::<_, Option<String>>(11)
        || row.get::<_, String>(4) != "answered"
        || row.get::<_, i16>(5) != 1
    {
        return Err(invalid("installation native answer unresolved"));
    }
    Ok(NativeJournal {
        invocation_id: row.get(0),
        selection_bytes: row.get(1),
        arguments_bytes: row.get(2),
        answer_bytes: row
            .get::<_, Option<String>>(3)
            .ok_or_else(|| invalid("installation native answer absent"))?,
        data_max_depth: u64::try_from(row.get::<_, i64>(6)).map_err(invalid)?,
        data_max_nodes: u64::try_from(row.get::<_, i64>(7)).map_err(invalid)?,
        data_max_bytes: u64::try_from(row.get::<_, i64>(8)).map_err(invalid)?,
    })
}

/// Acceptance source for the real boot consumer, including its historical
/// path. No alternate observer or manufactured successful execution is used.
#[cfg(test)]
pub(crate) async fn assert_original_installation_readback(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    pool: Arc<PgPool>,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
) {
    async fn counts(pool: &PgPool) -> (i64, i64, i64, i64) {
        let client = pool.get().await.unwrap();
        let row=client.query_one("SELECT (SELECT count(*) FROM reborn_component_review_submissions),(SELECT count(*) FROM reborn_component_review_requests),(SELECT count(*) FROM brassclaw_monty_tool_invocations),(SELECT count(*) FROM brassclaw_monty_task_admissions)",&[]).await.unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
    let before = counts(&pool).await;
    let grants_before = tests::Audit::read(&pool).await.unwrap();
    let RetainedProgram::Tools(reply) = catalogue.reply.program() else {
        panic!("actual reply context")
    };
    let mut package = crate::source_inspection_components::retain(
        pool.clone(),
        catalogue.kernel.as_ref(),
        &catalogue.qualification_scope.tenant_id,
        reply,
    )
    .await
    .unwrap();
    package
        .definitions
        .extend(
            crate::builtin_bootstrap::retain_validator_structure_drafts(&pool, package.usage)
                .await
                .unwrap(),
        )
        .unwrap();
    let original = observe_installation(
        pool.clone(),
        catalogue.kernel.clone(),
        catalogue.worker_image.clone(),
        package.definitions,
        reply,
        executor,
        preparation,
        catalogue.resource_store_settings.clone(),
        catalogue.root_definition.clone(),
    )
    .await
    .unwrap();
    let profiles = match &original {
        InstallationSourceObservations::ObservedPositive(p) => p,
        InstallationSourceObservations::CompletedNegative(_) => {
            panic!("unchanged structural-presence package must pass both checking profiles")
        }
        InstallationSourceObservations::UnavailableBeforeFirstExecution { reason } => {
            panic!("actual native installation observations required: {reason}")
        }
    };
    assert_eq!(profiles.len(), 2);
    assert_eq!(
        profiles[0].context.selection.profile(),
        store::InstallationProfile::Structure
    );
    assert_eq!(
        profiles[1].context.selection.profile(),
        store::InstallationProfile::Pure
    );
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .unwrap();
    original.verify_in_transaction(&tx).await.unwrap();
    tx.commit().await.unwrap();
    drop(client);
    // Exercise the normal Turn owner's bounded terminal pruning, rather than
    // deleting audit rows or treating a missing live Turn as completion.
    for profile in profiles {
        use brassclaw_turns::TurnStateStore;
        let mut client = pool.get().await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .unwrap();
        let (_, admission) = profile
            .context
            .read::<AdmissionBinding>(&tx, profile.context.id("original-admission"))
            .await
            .unwrap()
            .unwrap();
        let (_, preterminal) = profile
            .context
            .read::<Preterminal>(&tx, profile.context.id("preterminal"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            preterminal.checks["aggregation"],
            json!({"pass":true,"errors":[]})
        );
        assert!(
            preterminal.checks["rules"]
                .as_array()
                .unwrap()
                .iter()
                .all(|rule| rule["ok"] == true && rule["errors"] == json!([])),
            "each actual selected rule must positively pass"
        );
        assert_eq!(
            preterminal.step_fingerprints.len(),
            profile.context.program.program().steplist.len()
        );
        tx.commit().await.unwrap();
        drop(client);
        let state = brassclaw_turns::PgTurnStateStore::new(
            pool.clone(),
            crate::bootstrap_validation::NAMESPACE,
        )
        .with_limits(brassclaw_turns::InMemoryTurnStateStoreLimits {
            max_terminal_records: 0,
            ..Default::default()
        });
        let run = TurnRunId::from_uuid(admission.run_id);
        state
            .request_cancel_tree(brassclaw_turns::CancelRunRequest {
                scope: admission.scope.clone(),
                actor: brassclaw_turns::TurnActor::new(
                    brassclaw_host_api::UserId::new("installation-owner").unwrap(),
                ),
                run_id: run,
                reason: brassclaw_turns::SanitizedCancelReason::OperatorRequested,
                idempotency_key: brassclaw_turns::IdempotencyKey::new(format!(
                    "prune-completed-{run}"
                ))
                .unwrap(),
            })
            .await
            .unwrap();
        assert!(
            state
                .get_run_record(&admission.scope, run)
                .await
                .unwrap()
                .is_none(),
            "normal Turn pruning must actually remove the original terminal record"
        );
        assert_eq!(
            profile.context.read_original().await.unwrap().unwrap().0,
            profile.completion_bytes,
            "trusted immutable completion survives normal Turn pruning without replay"
        );
    }
    assert_eq!(
        grants_before,
        tests::Audit::read(&pool).await.unwrap(),
        "original consumer cannot grant review/approval, activate or enqueue"
    );
    assert_eq!(
        before,
        counts(&pool).await,
        "completed installation originals must not admit, replay or write authored review rows"
    );
}
