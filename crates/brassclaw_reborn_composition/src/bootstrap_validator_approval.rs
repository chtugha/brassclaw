//! Closed installation qualification of checking functions, not candidate-class
//! Q1, general Pure publication or permission to run the structural class routes.
use crate::{
    RebornBuildError, bootstrap_evidence as evidence, bootstrap_evidence::io,
    bootstrap_reply_validation::invalid,
};
use brassclaw_engine::{
    executor::{
        retained_recipe::{
            RetainedProgram, RetainedRecipeExecution, RetainedStepFailure,
            RetainedTransportEvidence, typed_value_checksum_with_limits,
        },
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
    },
};
use brassclaw_host_runtime::PackagedExecutableImage;
use brassclaw_monty_host::{
    VmFailure,
    process::{ProcessFailure, RecipeCommand, WorkerCommand},
    service::PortFailure,
    source_structure::SourceStructure,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::REVISION_LIMITS,
    global_bootstrap_components::packaged_component_id,
    pure_component_contract::PureComponentApprovalDeclaration,
    revision_store::PgComponentRevisionStore,
    value_contract::{ContractLimits, strict_json},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;
#[path = "bootstrap_checker_execution.rs"]
mod execution;
#[path = "bootstrap_checker_fixtures.rs"]
mod fixtures;
#[cfg(test)]
#[path = "bootstrap_checker_tests.rs"]
mod tests;
use fixtures::Suite;
#[path = "bootstrap_pure_checker.rs"]
mod pure;
pub(crate) use pure::{CheckedPureCheckerClosure, pure_source_cases, qualify as qualify_pure};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    uuid: Uuid,
    class_code: i32,
    version: u64,
    checksum: String,
}
impl From<ComponentRevisionRef> for Reference {
    fn from(r: ComponentRevisionRef) -> Self {
        Self {
            uuid: r.uuid,
            class_code: r.class_code,
            version: r.version,
            checksum: hex::encode(r.checksum),
        }
    }
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordRef {
    id: Uuid,
    checksum: String,
}
fn reference(id: Uuid, bytes: &str) -> RecordRef {
    RecordRef {
        id,
        checksum: evidence::checksum(bytes),
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Claim {
    format: String,
    scope: String,
    root: Reference,
    components: Vec<Reference>,
    recipe: Reference,
    selection_bytes: BTreeMap<String, String>,
    suites: Vec<Suite>,
    support: Option<RecordRef>,
    worker_checksum: String,
    application_checksum: String,
    global_root: Reference,
    global_source_checksum: String,
    owner_checksum: String,
    fixture_execution_checksum: String,
    factory_checksum: String,
    receipt_checksum: String,
    inspector_checksum: String,
    executor_checksum: String,
    quiescence_checksum: String,
    io_checksum: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reservation {
    format: String,
    claim: Value,
    execution_request_id: Uuid,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    format: String,
    reservation: RecordRef,
    executor_bytes: String,
    executor_checksum: String,
    preparation_bytes: String,
    preparation_checksum: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceProof {
    format: String,
    operation: RecordRef,
    root: Reference,
    closure: Vec<Reference>,
    exports: BTreeMap<String, String>,
    sources: BTreeMap<Uuid, Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    format: String,
    operation: RecordRef,
    variant: String,
    run_id: Uuid,
    scope: brassclaw_turns::TurnScope,
    runner_id: Uuid,
    claim_checksum: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepFact {
    id: String,
    input_checksum: String,
    result_checksum: Option<String>,
    max_depth: usize,
    max_nodes: usize,
    max_bytes: usize,
    python_failure_command: Option<String>,
    python_diagnostic: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observed {
    format: String,
    operation: RecordRef,
    admission: RecordRef,
    source: RecordRef,
    variant: String,
    task: Uuid,
    receipt: Option<String>,
    steps: Vec<StepFact>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Quiescence {
    format: String,
    operation: RecordRef,
    admission: RecordRef,
    observed: RecordRef,
    actual: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completed {
    format: String,
    operation: RecordRef,
    admission: RecordRef,
    observed: RecordRef,
    quiescence: RecordRef,
    turn: Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualified {
    format: String,
    scope: String,
    operation: RecordRef,
    source: RecordRef,
    completed: Vec<RecordRef>,
    root: Reference,
    exports: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: String,
    evidence_id: Uuid,
    kind: String,
    validation_mode: String,
    association_checksum: String,
    components: Vec<Reference>,
    succeeded: bool,
    reviewed_evidence: Vec<Value>,
    report: Value,
}

struct Context {
    pool: Arc<PgPool>,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    worker: Arc<PackagedExecutableImage>,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
    claim: Claim,
    // Derived once from the finalized immutable claim/profile, never a cached
    // validation result or a substitute for original database readback.
    claim_checksum: String,
    envelope_components: Vec<Value>,
    profile: Option<Arc<pure::Profile>>,
    reservation: RecordRef,
    operation_id: Uuid,
    operation: Operation,
    operation_bytes: String,
    programs: BTreeMap<String, Arc<RetainedToolProgram>>,
}
impl Context {
    fn claim_bytes(&self) -> Result<String, RebornBuildError> {
        match &self.profile {
            Some(profile) => profile.claim_bytes(&self.claim),
            None => serde_json::to_string(&self.claim).map_err(invalid),
        }
    }
    fn evidence_components(&self) -> Result<Vec<Value>, RebornBuildError> {
        match &self.profile {
            Some(profile) => profile
                .components(&self.claim)?
                .iter()
                .map(|r| serde_json::to_value(r).map_err(invalid))
                .collect(),
            None => Ok(vec![
                serde_json::to_value(&self.claim.root).map_err(invalid)?,
            ]),
        }
    }
    fn format(&self, stage: &str) -> String {
        let prefix = if self.profile.is_some() {
            "pure-checker"
        } else {
            "structural-checker"
        };
        format!("{prefix}-{stage}/1")
    }
    fn id(&self, stage: &str) -> Uuid {
        Uuid::new_v5(&self.operation_id, stage.as_bytes())
    }
    fn op(&self) -> RecordRef {
        reference(self.operation_id, &self.operation_bytes)
    }
    fn encode<T: Serialize>(
        &self,
        id: Uuid,
        kind: &str,
        success: bool,
        report: &T,
    ) -> Result<String, RebornBuildError> {
        Ok(evidence::system_record(
            id,
            kind,
            success,
            &self.claim_checksum,
            &self.envelope_components,
            serde_json::to_value(report).map_err(invalid)?,
        ))
    }
    async fn read<T: serde::de::DeserializeOwned + Serialize>(
        &self,
        tx: &Transaction<'_>,
        id: Uuid,
        kind: &str,
        success: bool,
    ) -> Result<Option<(String, T)>, RebornBuildError> {
        let Some(bytes) = evidence::read_evidence(tx, id).await? else {
            return Ok(None);
        };
        let envelope: Envelope =
            serde_json::from_value(strict_json(&bytes, REVISION_LIMITS).map_err(invalid)?)
                .map_err(invalid)?;
        let report: T = serde_json::from_value(envelope.report).map_err(invalid)?;
        if self.encode(id, kind, success, &report)? != bytes {
            return Err(invalid("checker original envelope differs"));
        }
        Ok(Some((bytes, report)))
    }
    async fn retain<T: Serialize>(&self, id: Uuid, report: &T) -> Result<String, RebornBuildError> {
        let bytes = self.encode(id, "behavior", false, report)?;
        io::retain_record(&self.pool, self.storage.as_ref(), id, &bytes).await?;
        Ok(bytes)
    }
    fn executor(
        &self,
    ) -> Result<Arc<crate::global_monty_startup::ControlledExecutorConfiguration>, RebornBuildError>
    {
        let executor =
            crate::global_monty_startup::ControlledExecutorConfiguration::restore_original(
                &self.operation.executor_bytes,
            )?;
        if executor.checksum() != self.operation.executor_checksum {
            return Err(invalid("checker original executor checksum differs"));
        }
        Ok(executor)
    }
    fn preparation(
        &self,
    ) -> Result<crate::installed_monty_catalogue::PreparationCapture, RebornBuildError> {
        crate::installed_monty_catalogue::PreparationCapture::restore_original(
            &self.operation.preparation_bytes,
            &self.operation.preparation_checksum,
        )
        .map_err(invalid)
    }
    fn program(&self, variant: &str) -> Result<&Arc<RetainedToolProgram>, RebornBuildError> {
        self.programs
            .get(variant)
            .ok_or_else(|| invalid("checker program missing"))
    }
    fn limits(&self) -> Result<ContractLimits, RebornBuildError> {
        Ok(self.executor()?.capacity().data_limits)
    }
    async fn verify_source(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(RecordRef, BTreeMap<String, String>), RebornBuildError> {
        let (bytes, source) = self
            .read::<SourceProof>(tx, self.id("source"), "behavior", false)
            .await?
            .ok_or_else(|| invalid("checker source original missing"))?;
        if let Some(profile) = &self.profile {
            return profile.verify_source(self, &source, &bytes);
        }
        if source.format != "structural-checker-source/1"
            || source.operation != self.op()
            || source.root != self.claim.root
            || source.closure != [self.claim.root.clone()]
            || source.exports.len() != 1
        {
            return Err(invalid("checker source subject/closure differs"));
        }
        let program = self.program(&self.claim.suites[0].variant)?;
        let graph = program.inputs().instruction().snapshot().revisions();
        let expected_ids = program
            .inputs()
            .components()
            .values()
            .map(|r| r.uuid)
            .collect::<BTreeSet<_>>();
        if source.sources.keys().copied().collect::<BTreeSet<_>>() != expected_ids {
            return Err(invalid("checker source inventory incomplete"));
        }
        for (id, facts) in &source.sources {
            let facts: SourceStructure = serde_json::from_value(facts.clone()).map_err(invalid)?;
            let revision = &graph[id];
            let doc = revision.draft().document();
            let declaration =
                brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
                    doc,
                    revision.draft().dependencies(),
                )
                .map_err(invalid)?
                .ok_or_else(|| invalid("checker has no preload declaration"))?;
            let preload = facts
                .preload
                .as_ref()
                .ok_or_else(|| invalid("checker preload observation missing"))?;
            let functions = declaration
                .exports
                .values()
                .map(|e| (e.symbol.clone(), e.parameters.clone()))
                .chain(declaration.private_functions.clone())
                .collect::<BTreeMap<_, _>>();
            if !revision.draft().dependencies().is_empty()
                || !facts.direct_host_calls.is_empty()
                || facts.relative_imports
                || facts.host_value_references != 0
                || !facts.reserved_name_references.is_empty()
                || facts.source_checksum
                    != evidence::checksum(
                        doc["content"]
                            .as_str()
                            .ok_or_else(|| invalid("checker source missing"))?,
                    )
                || preload.functions != functions
                || preload.constants != declaration.constants.iter().cloned().collect()
                || facts.imports != declaration.imports.iter().cloned().collect()
            {
                return Err(invalid(
                    "checker original pure/native definition facts differ",
                ));
            }
        }
        let revision = &graph[&self.claim.root.uuid];
        let doc = revision.draft().document();
        brassclaw_skills::value_contract::InputContract::from_value(
            &doc["input_contract"],
            REVISION_LIMITS,
        )
        .map_err(invalid)?;
        brassclaw_skills::value_contract::ValueContract::result_from_value(
            &doc["result_contract"],
            REVISION_LIMITS,
        )
        .map_err(invalid)?;
        let declaration = brassclaw_engine::executor::retained_preload::PreloadDeclaration::parse(
            doc,
            revision.draft().dependencies(),
        )
        .map_err(invalid)?
        .ok_or_else(|| invalid("checker export missing"))?;
        let exports = declaration
            .exports
            .keys()
            .map(|name| {
                Ok((
                    name.clone(),
                    declaration
                        .invocation(name, &doc["input_contract"])
                        .map_err(invalid)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, RebornBuildError>>()?;
        if source.exports != exports {
            return Err(invalid("checker entire export interface changed"));
        }
        Ok((reference(self.id("source"), &bytes), exports))
    }
    async fn verify_completed(
        &self,
        tx: &Transaction<'_>,
        suite: &Suite,
        source: &RecordRef,
    ) -> Result<RecordRef, RebornBuildError> {
        let stage = |name: &str| self.id(&format!("{}:{name}", suite.variant));
        let (admission_bytes, admission) = self
            .read::<Admission>(tx, stage("admission"), "behavior", false)
            .await?
            .ok_or_else(|| invalid("checker original admission missing"))?;
        let (observed_bytes, observed) = self
            .read::<Observed>(tx, stage("observed"), "behavior", false)
            .await?
            .ok_or_else(|| invalid("checker original result missing"))?;
        let (quiescence_bytes, quiescence) = self
            .read::<Quiescence>(tx, stage("quiescence"), "behavior", false)
            .await?
            .ok_or_else(|| invalid("checker original quiescence missing"))?;
        let (completed_bytes, completed) = self
            .read::<Completed>(tx, stage("completed"), "behavior", false)
            .await?
            .ok_or_else(|| invalid("checker original completion missing; do not replay"))?;
        let admission_ref = reference(stage("admission"), &admission_bytes);
        let observed_ref = reference(stage("observed"), &observed_bytes);
        if admission.format != self.format("admission")
            || admission.operation != self.op()
            || admission.variant != suite.variant
            || admission.run_id.is_nil()
            || admission.runner_id.is_nil()
            || observed.operation != self.op()
            || observed.admission != admission_ref
            || observed.source != *source
            || observed.variant != suite.variant
            || quiescence.format != self.format("quiescence")
            || quiescence.operation != self.op()
            || quiescence.admission != admission_ref
            || quiescence.observed != observed_ref
            || completed.format != self.format("completed")
            || completed.operation != self.op()
            || completed.admission != admission_ref
            || completed.observed != observed_ref
            || completed.quiescence != reference(stage("quiescence"), &quiescence_bytes)
        {
            return Err(invalid("checker original custody graph differs"));
        }
        self.verify_observed(suite, &observed)?;
        let program = self.program(&suite.variant)?;
        if suite.python_stop {
            crate::bootstrap_quiescence_wire::verify_expected_stop(
                &quiescence.actual,
                program,
                &json!(observed.task),
                &suite.steps[0].id,
            )?;
        } else {
            crate::bootstrap_quiescence_wire::verify(
                &quiescence.actual,
                program,
                &json!(observed.task),
                observed
                    .receipt
                    .as_deref()
                    .ok_or_else(|| invalid("checker receipt missing"))?,
            )?;
        }
        if quiescence.actual["execution"]["controlled_executor_configuration"]
            != self.operation.executor_bytes
            || quiescence.actual["execution"]["controlled_executor_configuration_checksum"]
                != self.operation.executor_checksum
        {
            return Err(invalid("checker historical executor differs"));
        }
        let turn: brassclaw_turns::TurnRunRecord =
            serde_json::from_value(completed.turn).map_err(invalid)?;
        if turn.run_id.as_uuid() != admission.run_id
            || turn.scope != admission.scope
            || turn.status != brassclaw_turns::TurnStatus::Completed
            || turn.claim_count != 1
            || turn.runner_id.is_some()
            || turn.lease_token.is_some()
            || turn.lease_expires_at.is_some()
            || turn.failure.is_some()
            || turn.native_auth_wait.is_some()
        {
            return Err(invalid("checker terminal original differs"));
        }
        let row=tx.query_one("SELECT phase,outcome,scope,runner_id,claim_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1",&[&admission.run_id]).await.map_err(invalid)?;
        if row.get::<_, String>(0) != "settled"
            || row.get::<_, Option<Value>>(1)
                != Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
            || row.get::<_, Value>(2) != json!(admission.scope)
            || row.get::<_, Uuid>(3) != admission.runner_id
            || hex::encode(row.get::<_, Vec<u8>>(4)) != admission.claim_checksum
        {
            return Err(invalid("checker durable native close differs"));
        }
        Ok(reference(stage("completed"), &completed_bytes))
    }
    async fn qualified(&self, tx: &Transaction<'_>) -> Result<Qualified, RebornBuildError> {
        self.qualified_traced(tx, None).await
    }
    async fn qualified_traced(
        &self,
        tx: &Transaction<'_>,
        trace: Option<&pure::case_encoder::QualificationTrace>,
    ) -> Result<Qualified, RebornBuildError> {
        #[cfg(test)]
        tests::record_original_walk(self.operation_id);
        if let Some(profile) = &self.profile {
            profile.verify_native_inputs(tx).await?;
        }
        let reservation_bytes = evidence::read_evidence(tx, self.reservation.id)
            .await?
            .ok_or_else(|| invalid("checker original reservation missing"))?;
        let envelope: Envelope = serde_json::from_value(
            strict_json(&reservation_bytes, REVISION_LIMITS).map_err(invalid)?,
        )
        .map_err(invalid)?;
        let reservation: Reservation = serde_json::from_value(envelope.report).map_err(invalid)?;
        let claim_bytes = self.claim_bytes()?;
        let claim_checksum = evidence::checksum(&claim_bytes);
        if claim_checksum != self.claim_checksum
            || self.evidence_components()? != self.envelope_components
        {
            return Err(invalid("checker immutable claim/envelope identity differs"));
        }
        if reservation.format != self.format("reservation")
            || reference(self.reservation.id, &reservation_bytes) != self.reservation
            || reservation.claim != strict_json(&claim_bytes, REVISION_LIMITS).map_err(invalid)?
            || reservation.execution_request_id.is_nil()
            || Uuid::new_v5(
                &self.reservation.id,
                reservation.execution_request_id.as_bytes(),
            ) != self.operation_id
            || evidence::system_record(
                self.reservation.id,
                "behavior",
                false,
                &claim_checksum,
                &[serde_json::to_value(&self.claim.root).map_err(invalid)?],
                serde_json::to_value(&reservation).map_err(invalid)?,
            ) != reservation_bytes
        {
            return Err(invalid("checker qualification reservation changed"));
        }
        let (operation_bytes, operation) = self
            .read::<Operation>(tx, self.operation_id, "behavior", false)
            .await?
            .ok_or_else(|| invalid("checker original intent missing"))?;
        if operation_bytes != self.operation_bytes
            || operation.format != self.format("operation")
            || operation.reservation != self.reservation
        {
            return Err(invalid("checker original intent differs"));
        }
        self.executor()?;
        self.preparation()?;
        let graph = self
            .program(&self.claim.suites[0].variant)?
            .inputs()
            .instruction()
            .snapshot();
        let refs = graph
            .revisions()
            .values()
            .map(|r| r.reference())
            .collect::<Vec<_>>();
        PgComponentRevisionStore::read_exact_in_transaction(
            tx,
            &[self.claim.recipe.uuid],
            &refs,
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
        .map_err(invalid)?;
        PgComponentRevisionStore::read_revision_in_transaction(tx, self.root.reference())
            .await
            .map_err(invalid)?;
        if let Some(trace) = trace {
            trace.note("verify_encoder_source_original", None);
        }
        let (source, exports) = self.verify_source(tx).await?;
        let mut completed = Vec::new();
        for suite in &self.claim.suites {
            if let Some(trace) = trace {
                trace.note("verify_encoder_completed_original", Some(&suite.variant));
            }
            completed.push(self.verify_completed(tx, suite, &source).await?);
        }
        let qualified = Qualified {
            format: self.format("qualified-own-function"),
            scope: "installation-own-checker-only".into(),
            operation: self.op(),
            source,
            completed,
            root: self.claim.root.clone(),
            exports,
        };
        Ok(qualified)
    }
    fn qualification_value(&self, qualified: &Qualified) -> Result<Value, RebornBuildError> {
        match &self.profile {
            Some(profile) => profile.report(self, qualified),
            None => serde_json::to_value(qualified).map_err(invalid),
        }
    }
    async fn verify_final(&self, tx: &Transaction<'_>) -> Result<RecordRef, RebornBuildError> {
        if self.profile.as_ref().is_none_or(|p| p.approves_function()) {
            return self.verify_approval(tx).await;
        }
        let report = self.qualification_value(&self.qualified(tx).await?)?;
        let mut behavior = None;
        for kind in ["q1", "behavior"] {
            let (bytes, actual) = self
                .read::<Value>(tx, self.id(kind), kind, true)
                .await?
                .ok_or_else(|| invalid("private fixture original qualification missing"))?;
            if actual != report || bytes != self.encode(self.id(kind), kind, true, &report)? {
                return Err(invalid("private fixture original qualification differs"));
            }
            if kind == "behavior" {
                behavior = Some(reference(self.id(kind), &bytes));
            }
        }
        behavior.ok_or_else(|| invalid("private fixture behavior missing"))
    }
    fn approval_bytes(&self) -> Result<String, RebornBuildError> {
        if let Some(profile) = &self.profile {
            if !profile.approves_function() {
                return Err(invalid("private support has no public Pure approval"));
            }
            return Ok(json!({"format":"pure-component-approval/1","approval_id":self.id("approval"),
                "root":profile.subject(&self.claim)?,"components":profile.components(&self.claim)?,
                "validation_mode":"system_seed","q1_ref":self.id("q1"),"q2_ref":null,"behavioral_refs":[self.id("behavior")]}).to_string());
        }
        Ok(json!({"format":"pure-component-approval/1","approval_id":self.id("approval"),"root":self.claim.root,"components":[self.claim.root],"validation_mode":"system_seed","q1_ref":self.id("q1"),"q2_ref":null,"behavioral_refs":[self.id("behavior")]}).to_string())
    }
    async fn verify_approval(&self, tx: &Transaction<'_>) -> Result<RecordRef, RebornBuildError> {
        let qualified = self.qualification_value(&self.qualified(tx).await?)?;
        for (kind, id) in [("q1", self.id("q1")), ("behavior", self.id("behavior"))] {
            let (bytes, _) = self
                .read::<Value>(tx, id, kind, true)
                .await?
                .ok_or_else(|| invalid("checker successful original evidence missing"))?;
            if bytes != self.encode(id, kind, true, &qualified)? {
                return Err(invalid("checker qualification evidence differs"));
            }
        }
        let bytes = self.approval_bytes()?;
        let row=tx.query_opt("SELECT approval_bytes,checksum,root_id,root_version,root_checksum,validation_mode,q1_ref,q2_ref FROM reborn_pure_component_approvals WHERE approval_id=$1",&[&self.id("approval")]).await.map_err(invalid)?.ok_or_else(||invalid("checker original approval missing"))?;
        if row.get::<_, String>(0) != bytes
            || row.get::<_, String>(1) != evidence::checksum(&bytes)
            || row.get::<_, Uuid>(2) != self.claim.root.uuid
            || row.get::<_, i64>(3) != i64::try_from(self.claim.root.version).map_err(invalid)?
            || row.get::<_, String>(4) != self.claim.root.checksum
            || row.get::<_, String>(5) != "system_seed"
            || row.get::<_, Uuid>(6) != self.id("q1")
            || row.get::<_, Option<Uuid>>(7).is_some()
        {
            return Err(invalid("checker approval original differs"));
        }
        let rows=tx.query("SELECT component_id,component_version,component_checksum FROM reborn_pure_component_approval_closure WHERE approval_id=$1",&[&self.id("approval")]).await.map_err(invalid)?;
        if let Some(profile) = &self.profile {
            let expected = profile.components(&self.claim)?;
            if rows.len() != expected.len()
                || rows.iter().any(|row| {
                    expected
                        .iter()
                        .find(|r| r.uuid == row.get::<_, Uuid>(0))
                        .is_none_or(|r| {
                            i64::try_from(r.version).ok() != Some(row.get::<_, i64>(1))
                                || r.checksum != row.get::<_, String>(2)
                        })
                })
            {
                return Err(invalid("pure own approval subject closure differs"));
            }
            return Ok(reference(self.id("approval"), &bytes));
        }
        if rows.len() != 1
            || rows[0].get::<_, Uuid>(0) != self.claim.root.uuid
            || rows[0].get::<_, i64>(1)
                != i64::try_from(self.claim.root.version).map_err(invalid)?
            || rows[0].get::<_, String>(2) != self.claim.root.checksum
        {
            return Err(invalid("checker approval borrowed a different closure"));
        }
        Ok(reference(self.id("approval"), &bytes))
    }
}

pub(crate) struct CheckedStructuralHelpers {
    contexts: Vec<Arc<Context>>,
}
impl CheckedStructuralHelpers {
    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        self.verify_with_receipt(tx).await.map(|_| ())
    }
    async fn verify_with_receipt(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<RecordRef, RebornBuildError> {
        let expected = std::iter::once(fixtures::receipt_id())
            .chain(
                brassclaw_skills::validator_components::VALIDATOR_CLASS_CODES
                    .iter()
                    .map(|c| {
                        brassclaw_skills::validator_components::structural_validator_pc_id(*c)
                    }),
            )
            .collect::<BTreeSet<_>>();
        if self.contexts.len() != expected.len()
            || self
                .contexts
                .iter()
                .map(|c| c.claim.root.uuid)
                .collect::<BTreeSet<_>>()
                != expected
        {
            return Err(invalid(
                "all eight structural helpers and their receipt prerequisite required",
            ));
        }
        evidence::require_coherent(tx).await?;
        let mut support = None;
        for context in &self.contexts {
            if context.claim.support != support {
                return Err(invalid("checker receipt prerequisite differs"));
            }
            let proof = context.verify_approval(tx).await?;
            if support.is_none() {
                support = Some(proof);
            }
        }
        support.ok_or_else(|| invalid("structural receipt prerequisite missing"))
    }
}

pub(crate) async fn qualify(
    pool: Arc<PgPool>,
    worker: Arc<PackagedExecutableImage>,
    root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    preparation: crate::installed_monty_catalogue::PreparationCapture,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    application_checksum: String,
) -> Result<CheckedStructuralHelpers, RebornBuildError> {
    let receipt = fixtures::receipt_draft()?;
    let mut specs = vec![(receipt.clone(), None, fixtures::receipt_suites())];
    for &class in brassclaw_skills::validator_components::VALIDATOR_CLASS_CODES {
        let root_id = brassclaw_skills::validator_components::structural_validator_pc_id(class);
        let root = crate::builtin_bootstrap::validator_recipe_drafts_for_class(class)
            .map_err(invalid)?
            .into_iter()
            .find(|d| d.uuid() == root_id)
            .ok_or_else(|| invalid("canonical checker root missing"))?;
        specs.push((root, Some(class), fixtures::checker_suites(class, root_id)));
    }
    let specs = specs
        .into_iter()
        .map(|(candidate, class, suites)| {
            let recipe = fixtures::recipe(candidate.uuid(), class, &suites)?;
            Ok(Spec {
                candidate,
                recipe,
                suites,
                dependencies: vec![receipt.clone()],
                profile: None,
            })
        })
        .collect::<Result<Vec<_>, RebornBuildError>>()?;
    Ok(CheckedStructuralHelpers {
        contexts: qualify_specs(
            SessionInput {
                pool,
                worker,
                root,
                executor,
                preparation,
                storage,
                application_checksum,
            },
            specs,
        )
        .await?,
    })
}

struct Spec {
    candidate: brassclaw_skills::component_revision::ComponentRevisionDraft,
    recipe: brassclaw_skills::component_revision::ComponentRevisionDraft,
    suites: Vec<Suite>,
    dependencies: Vec<brassclaw_skills::component_revision::ComponentRevisionDraft>,
    profile: Option<Arc<pure::Profile>>,
}

pub(crate) use pure::case_encoder::{CheckedCaseEncoder, qualify as qualify_case_encoder};

#[derive(Clone)]
pub(crate) struct SessionInput {
    pub(crate) pool: Arc<PgPool>,
    pub(crate) worker: Arc<PackagedExecutableImage>,
    pub(crate) root: Arc<brassclaw_skills::orchestrator_contract::GlobalRootDefinition>,
    pub(crate) executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    pub(crate) preparation: crate::installed_monty_catalogue::PreparationCapture,
    pub(crate) storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    pub(crate) application_checksum: String,
}

async fn qualify_specs(
    base: SessionInput,
    specs: Vec<Spec>,
) -> Result<Vec<Arc<Context>>, RebornBuildError> {
    qualify_specs_traced(base, specs, None).await
}

async fn qualify_specs_traced(
    base: SessionInput,
    specs: Vec<Spec>,
    trace: Option<&pure::case_encoder::QualificationTrace>,
) -> Result<Vec<Arc<Context>>, RebornBuildError> {
    let note = |phase| {
        if let Some(trace) = trace {
            trace.note(phase, None);
        }
    };
    let SessionInput {
        pool,
        worker,
        root,
        executor,
        preparation,
        storage,
        application_checksum,
    } = base;
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut contexts: Vec<Arc<Context>> = Vec::new();
    let mut support = None;
    for Spec {
        candidate,
        recipe,
        suites,
        dependencies,
        mut profile,
    } in specs
    {
        if let Some(selected) = &mut profile {
            note("capture_profile_prerequisites");
            let mut next = (**selected).clone();
            let mut client = io::acquire(&pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            for previous in &contexts {
                let prerequisite = previous
                    .profile
                    .as_ref()
                    .ok_or_else(|| invalid("pure prerequisite scope missing"))?;
                if next.requires_subject(prerequisite.subject_id()) {
                    next.prerequisites.push(previous.verify_final(&tx).await?);
                }
            }
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            *selected = Arc::new(next);
        }
        let mut refs = Vec::new();
        note("retain_fixture_components");
        for draft in std::iter::once(&candidate)
            .chain(dependencies.iter())
            .chain(std::iter::once(&recipe))
        {
            if refs
                .iter()
                .any(|r: &ComponentRevisionRef| r.uuid == draft.uuid())
            {
                continue;
            }
            refs.push(
                io::bounded_for(
                    storage.as_ref(),
                    "checker_component_retain",
                    draft.uuid(),
                    async { store.retain_packaged_draft(draft).await.map_err(invalid) },
                )
                .await?,
            );
        }
        // A support-only graph must not retain an unrelated candidate identity.
        note("read_fixture_snapshot");
        let snapshot = Arc::new(
            io::bounded_for(
                storage.as_ref(),
                "checker_fixture_snapshot",
                recipe.uuid(),
                async {
                    store
                        .read_exact(
                            &[recipe.uuid()],
                            &refs,
                            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                        )
                        .await
                        .map_err(invalid)
                },
            )
            .await?,
        );
        let mut programs = BTreeMap::new();
        let mut selections = BTreeMap::new();
        let mut normalized = Vec::new();
        note("prepare_fixture_programs");
        for (desc, suite) in suites.into_iter().enumerate() {
            if let Some(trace) = trace {
                trace.note("prepare_fixture_programs", Some(&suite.variant));
            }
            let suite = fixtures::normalize_suite(desc, &suite)?;
            let program = Arc::new(
                prepare_retained_tool_program(
                    compile_retained_recipe(
                        snapshot.clone(),
                        recipe.uuid(),
                        &suite.variant,
                        WorkflowClass::Deterministic,
                    )
                    .map_err(invalid)?,
                )
                .map_err(invalid)?,
            );
            if !program.bindings().is_empty() {
                return Err(invalid("pure checker fixture acquired a Tool binding"));
            }
            selections.insert(
                suite.variant.clone(),
                program
                    .inputs()
                    .instruction()
                    .retained_selection()
                    .map_err(invalid)?
                    .exact_bytes()
                    .to_owned(),
            );
            programs.insert(suite.variant.clone(), program);
            normalized.push(suite);
        }
        note("construct_fixture_claim");
        let claim = Claim {
            format: if profile.is_some() {
                "pure-checker-execution-context/1"
            } else {
                "structural-checker-claim/1"
            }
            .into(),
            scope: if profile.is_some() {
                "installation-closed-pure-fixture"
            } else {
                "installation-own-checker-only"
            }
            .into(),
            root: snapshot.revisions()[&candidate.uuid()].reference().into(),
            components: refs.iter().copied().map(Into::into).collect(),
            recipe: snapshot.revisions()[&recipe.uuid()].reference().into(),
            selection_bytes: selections,
            suites: normalized,
            support: support.clone(),
            worker_checksum: hex::encode(worker.checksum()),
            application_checksum: application_checksum.clone(),
            global_root: root.reference().into(),
            global_source_checksum: hex::encode(root.source_checksum()),
            owner_checksum: evidence::checksum(include_str!("bootstrap_validator_approval.rs")),
            fixture_execution_checksum: crate::bootstrap_validation::qualification_owner_checksum(
                include_str!("bootstrap_checker_execution.rs"),
            ),
            factory_checksum: evidence::checksum(include_str!("bootstrap_checker_fixtures.rs")),
            receipt_checksum: evidence::checksum(fixtures::RECEIPT_SOURCE),
            inspector_checksum: evidence::checksum(include_str!(
                "../../brassclaw_engine/src/executor/retained_source.rs"
            )),
            executor_checksum: evidence::checksum(include_str!("bootstrap_recipe_flow.rs")),
            quiescence_checksum: evidence::checksum(include_str!("bootstrap_quiescence_wire.rs")),
            io_checksum: evidence::checksum(include_str!("bootstrap_evidence_io.rs")),
        };
        let claim_bytes = match &profile {
            Some(profile) => profile.claim_bytes(&claim)?,
            None => serde_json::to_string(&claim).map_err(invalid)?,
        };
        let claim_checksum = evidence::checksum(&claim_bytes);
        let envelope_components = match &profile {
            Some(profile) => profile
                .components(&claim)?
                .iter()
                .map(|r| serde_json::to_value(r).map_err(invalid))
                .collect::<Result<Vec<_>, _>>()?,
            None => vec![serde_json::to_value(&claim.root).map_err(invalid)?],
        };
        let domain = if profile.is_some() {
            "pure-checker"
        } else {
            "structural-checker"
        };
        let claim_id = Uuid::new_v5(
            &candidate.uuid(),
            format!("{domain}-reservation/1:{}", claim_checksum).as_bytes(),
        );
        let offered = Reservation {
            format: format!("{domain}-reservation/1"),
            claim: strict_json(&claim_bytes, REVISION_LIMITS).map_err(invalid)?,
            execution_request_id: Uuid::new_v4(),
        };
        let components = [serde_json::to_value(&claim.root).map_err(invalid)?];
        let offered_bytes = evidence::system_record(
            claim_id,
            "behavior",
            false,
            &claim_checksum,
            &components,
            serde_json::to_value(&offered).map_err(invalid)?,
        );
        note("read_fixture_reservation");
        let reservation_bytes = match io::read_record(&pool, storage.as_ref(), claim_id).await? {
            Some(bytes) => bytes,
            None => {
                note("retain_fixture_reservation");
                // A losing first-claim proposal reads the winner's complete
                // canonical original. Its offered request ID is never admitted.
                let _write =
                    io::retain_record(&pool, storage.as_ref(), claim_id, &offered_bytes).await;
                io::read_record(&pool, storage.as_ref(), claim_id)
                    .await?
                    .ok_or_else(|| invalid("checker reservation unresolved; no admission"))?
            }
        };
        note("verify_fixture_reservation");
        let envelope: Envelope = serde_json::from_value(
            strict_json(&reservation_bytes, REVISION_LIMITS).map_err(invalid)?,
        )
        .map_err(invalid)?;
        let reservation: Reservation = serde_json::from_value(envelope.report).map_err(invalid)?;
        if reservation.format != format!("{domain}-reservation/1")
            || reservation.execution_request_id.is_nil()
            || reservation.claim != strict_json(&claim_bytes, REVISION_LIMITS).map_err(invalid)?
            || evidence::system_record(
                claim_id,
                "behavior",
                false,
                &claim_checksum,
                &components,
                serde_json::to_value(&reservation).map_err(invalid)?,
            ) != reservation_bytes
        {
            return Err(invalid(
                "checker reservation does not identify canonical package",
            ));
        }
        let reservation_ref = reference(claim_id, &reservation_bytes);
        let operation_id = Uuid::new_v5(&claim_id, reservation.execution_request_id.as_bytes());
        note("read_fixture_operation");
        let existing = io::read_record(&pool, storage.as_ref(), operation_id).await?;
        let offered_operation = Operation {
            format: format!("{domain}-operation/1"),
            reservation: reservation_ref.clone(),
            executor_bytes: executor.exact_bytes().into(),
            executor_checksum: executor.checksum(),
            preparation_bytes: preparation.bytes().into(),
            preparation_checksum: preparation.checksum().into(),
        };
        let operation = match &existing {
            Some(bytes) => {
                let envelope: Envelope =
                    serde_json::from_value(strict_json(bytes, REVISION_LIMITS).map_err(invalid)?)
                        .map_err(invalid)?;
                serde_json::from_value(envelope.report).map_err(invalid)?
            }
            None => offered_operation,
        };
        drop(claim_bytes);
        let mut context = Context {
            pool: pool.clone(),
            storage: storage.clone(),
            worker: worker.clone(),
            root: root.clone(),
            claim,
            claim_checksum,
            envelope_components,
            profile,
            reservation: reservation_ref,
            operation_id,
            operation,
            operation_bytes: String::new(),
            programs,
        };
        context.operation_bytes =
            context.encode(operation_id, "behavior", false, &context.operation)?;
        if existing
            .as_ref()
            .is_some_and(|bytes| bytes != &context.operation_bytes)
        {
            return Err(invalid("checker original operation bytes differ"));
        }
        let context = Arc::new(context);
        if existing.is_some() {
            note("verify_published_original");
            let original = io::bounded_for(storage.as_ref(), "checker_published_original", context.claim.root.uuid, async {
                let mut client = io::acquire(&pool).await?;
                let tx = client.build_transaction().isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true).start().await.map_err(invalid)?;
                let present = tx.query_one("SELECT EXISTS(SELECT 1 FROM reborn_pure_component_approvals WHERE approval_id=$1)", &[&context.id("approval")]).await.map_err(invalid)?.get::<_,bool>(0);
                let proof = if present { Some(context.verify_final(&tx).await?) } else { None };
                tx.commit().await.map_err(invalid)?;client.recycle_after_commit();Ok(proof)
            }).await?;
            if let Some(proof) = original {
                if context.profile.is_none() && support.is_none() {
                    support = Some(proof);
                }
                contexts.push(context);
                continue;
            }
        }
        if existing.is_none() {
            #[cfg(test)]
            tests::before_new_intent(&context).await;
            note("retain_fixture_operation_intent");
            io::retain_new_intent(
                &pool,
                storage.as_ref(),
                operation_id,
                &context.operation_bytes,
            )
            .await?;
            context.execute(trace).await?;
        }
        // Existing complete native originals can finish publication without VM
        // execution; incomplete intent fails here and cannot restart fixtures.
        note("verify_complete_original");
        let qualified = io::bounded_for(
            storage.as_ref(),
            "checker_complete_original",
            context.claim.root.uuid,
            async {
                let mut client = io::acquire(&pool).await?;
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .map_err(invalid)?;
                let qualified = context.qualified_traced(&tx, trace).await?;
                tx.commit().await.map_err(invalid)?;
                client.recycle_after_commit();
                Ok(qualified)
            },
        )
        .await?;
        note("construct_qualification_report");
        let qualified = context.qualification_value(&qualified)?;
        note("retain_qualification_records");
        for (kind, id) in [
            ("q1", context.id("q1")),
            ("behavior", context.id("behavior")),
        ] {
            io::retain_record(
                &pool,
                storage.as_ref(),
                id,
                &context.encode(id, kind, true, &qualified)?,
            )
            .await?;
        }
        if context
            .profile
            .as_ref()
            .is_some_and(|p| !p.approves_function())
        {
            note("verify_final_instrument");
            let mut client = io::acquire(&pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)?;
            context.verify_final(&tx).await?;
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            contexts.push(context);
            continue;
        }
        let approval = context.approval_bytes()?;
        let declaration = PureComponentApprovalDeclaration::from_json(&approval, REVISION_LIMITS)
            .map_err(invalid)?;
        let write = io::bounded_for(
            storage.as_ref(),
            "checker_approval_write",
            context.claim.root.uuid,
            async {
                crate::pg_pure_component_approval::record_pure_component_approval(
                    &pool,
                    &declaration,
                )
                .await
                .map_err(invalid)
            },
        )
        .await;
        let readback = io::bounded_for(
            storage.as_ref(),
            "checker_approval_readback",
            context.claim.root.uuid,
            async {
                let mut client = io::acquire(&pool).await?;
                let tx = client
                    .build_transaction()
                    .isolation_level(IsolationLevel::RepeatableRead)
                    .read_only(true)
                    .start()
                    .await
                    .map_err(invalid)?;
                let proof = context.verify_approval(&tx).await?;
                tx.commit().await.map_err(invalid)?;
                client.recycle_after_commit();
                Ok(proof)
            },
        )
        .await;
        let proof = match readback {
            Ok(proof) => proof,
            Err(error) => {
                return Err(match write {
                    Ok(_) => error,
                    Err(write) => invalid(format!("checker approval unresolved: {write}; {error}")),
                });
            }
        };
        if context.profile.is_none() && support.is_none() {
            support = Some(proof);
        }
        contexts.push(context);
    }
    Ok(contexts)
}
