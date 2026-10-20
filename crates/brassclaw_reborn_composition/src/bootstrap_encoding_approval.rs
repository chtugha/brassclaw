//! Closed installation owner for the two exact packaged native encoding usages.
//! Imported model bytes and authored system_seed labels cannot create this proof.
use crate::{
    RebornBuildError, bootstrap_encoding_fixture::Fixture,
    bootstrap_encoding_validation as validation, bootstrap_evidence as evidence,
    bootstrap_reply_validation::invalid, monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::retained_tools::RetainedToolBinding,
};
use brassclaw_host_api::CapabilityId;
use brassclaw_host_runtime::{NativeImplementationRef, PackagedExecutableImage};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::{AssociationApprovalDeclaration, ValidationMode},
    component_revision::REVISION_LIMITS,
    global_bootstrap_components::UsageComponentIds,
    revision_store::PgComponentRevisionStore,
    value_contract::strict_json,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct BootstrapEncodingApproval {
    declaration: AssociationApprovalDeclaration,
    subject: Arc<Subject>,
}
impl BootstrapEncodingApproval {
    pub(super) fn approval_id(&self) -> Uuid {
        self.declaration.approval_id()
    }
    pub(super) fn usage_approval(
        &self,
        binding: &RetainedToolBinding,
    ) -> Option<&AssociationApprovalDeclaration> {
        self.declaration
            .require_selected_combination(binding.association(), binding.combination())
            .ok()
            .map(|()| &self.declaration)
    }
    pub(super) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        let actual = read(tx, &self.subject)
            .await?
            .ok_or_else(|| invalid("encoding qualification missing"))?;
        if actual.approval_id() != self.approval_id() {
            return Err(invalid("encoding qualification differs"));
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Subject {
    inspected: Arc<InspectedRetainedProgram>,
    ids: UsageComponentIds,
    effective: bool,
    header: Value,
    components: Value,
    association: String,
    q1: Uuid,
    behavior: Uuid,
    approval: Uuid,
    fixture: Arc<Fixture>,
    worker: Arc<PackagedExecutableImage>,
}
impl Subject {
    async fn prepare(
        candidate: &InspectedRetainedProgram,
        effective: bool,
        native: NativeImplementationRef,
        worker: Arc<PackagedExecutableImage>,
        fixture: Arc<Fixture>,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                candidate.program().clone(),
                &worker.path(),
                candidate.preparation_limits(),
            )
            .await
            .map_err(invalid)?,
        );
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(invalid("encoding Tool program required"));
        };
        let binding = program
            .bindings()
            .get("0:2")
            .filter(|_| program.bindings().len() == 1)
            .ok_or_else(|| invalid("encoding binding differs"))?;
        let ids = crate::prefix_encoding_candidates::ids(binding.tool().uuid, effective);
        let instruction = program.inputs().instruction();
        if instruction.recipe().uuid != ids.recipe
            || binding.capability_id() != brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID
        {
            return Err(invalid("encoding packaged identity differs"));
        }
        let tool = brassclaw_skills::model_encoding_components::model_encoding_tool_draft(
            ids.tool,
            serde_json::to_value(native).map_err(invalid)?,
        )
        .map_err(invalid)?;
        let drafts = crate::prefix_encoding_candidates::drafts(ids, effective).map_err(invalid)?;
        let mut wanted = BTreeSet::new();
        for draft in std::iter::once(&tool).chain(drafts.iter()) {
            wanted.insert(draft.uuid());
            if instruction
                .snapshot()
                .revisions()
                .get(&draft.uuid())
                .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
            {
                return Err(invalid(
                    "encoding qualification requires exact packaged definitions",
                ));
            }
        }
        if instruction
            .snapshot()
            .revisions()
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != wanted
        {
            return Err(invalid("encoding dependency graph differs"));
        }
        let sources: BTreeMap<_, _> = inspected
            .source_checks()
            .iter()
            .map(|(uuid, source)| (uuid.to_string(), source.observations()))
            .collect();
        let header = json!({"producer":crate::bootstrap_encoding_fixture::PRODUCER,"scope":"packaged-model-encoding-usages/1","effective_template":effective,
            "selection_checksum":hex::encode(instruction.retained_selection().map_err(invalid)?.checksum()),"native":native,"worker_checksum":hex::encode(worker.checksum()),
            "specification":fixture.specification,"artifacts":fixture.artifacts,"bad_tokenizer":fixture.bad_tokenizer,
            "cases":validation::cases(effective),"sources":sources,
            "fixture_checksum":evidence::checksum(include_str!("bootstrap_encoding_fixture.rs")),
            "validator_checksum":evidence::checksum(include_str!("bootstrap_encoding_validation.rs")),
            "owner_checksum":evidence::checksum(include_str!("bootstrap_encoding_approval.rs")),
            "flow_checksum":evidence::checksum(include_str!("bootstrap_recipe_flow.rs")),
            "root_checksum":evidence::checksum(crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED)});
        let identity = Uuid::new_v5(&ids.recipe, header.to_string().as_bytes());
        let components=json!(binding.combination().iter().map(|r|json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})).collect::<Vec<_>>());
        let association = binding.association().exact_bytes().to_owned();
        Ok(Arc::new(Self {
            inspected,
            ids,
            effective,
            header,
            components,
            association,
            q1: Uuid::new_v5(&identity, b"q1"),
            behavior: Uuid::new_v5(&identity, b"behavior"),
            approval: Uuid::new_v5(&identity, b"approval"),
            fixture,
            worker,
        }))
    }
    fn record(&self, id: Uuid, kind: &str, report: Value) -> String {
        json!({"format":"component-review-evidence/1","evidence_id":id,"kind":kind,"validation_mode":"system_seed","association_checksum":evidence::checksum(&self.association),"components":self.components,"succeeded":true,"reviewed_evidence":[],"report":report}).to_string()
    }
    fn q1_bytes(&self) -> String {
        self.record(self.q1,"q1",json!({"header":self.header,"checks":["exact-packaged-definitions-and-dependencies","retained-native-and-worker-integrity","effect-free-preload-and-typed-binding","fixed-independent-token-id-oracle","private-data-only-fixture-provenance"]}))
    }
    fn approval_bytes(&self) -> String {
        json!({"format":"skill-association-approval/1","approval_id":self.approval,"association_checksum":evidence::checksum(&self.association),"components":self.components,"validation_mode":"system_seed","q1_ref":self.q1,"q2_ref":null,"behavioral_refs":[self.behavior]}).to_string()
    }
}
pub(super) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    candidates: &[Arc<InspectedRetainedProgram>; 2],
    worker_path: &Path,
) -> Result<Vec<BootstrapEncodingApproval>, RebornBuildError> {
    let runtime = kernel
        .builtin(
            &CapabilityId::new(brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID)
                .map_err(invalid)?,
            None,
        )
        .map_err(invalid)?;
    runtime.verify_native_artifact().await.map_err(invalid)?;
    let native = runtime
        .native_identity()
        .ok_or_else(|| invalid("encoding native registration missing"))?;
    let worker = Arc::new(
        PackagedExecutableImage::capture(
            worker_path,
            candidates[0].preparation_limits().max_native_artifact_bytes,
        )
        .await
        .map_err(invalid)?,
    );
    let fixture = Arc::new(Fixture::retain(pool.clone()).await?);
    let subjects = [
        Subject::prepare(
            &candidates[0],
            false,
            native,
            worker.clone(),
            fixture.clone(),
        )
        .await?,
        Subject::prepare(&candidates[1], true, native, worker, fixture).await?,
    ];
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    if let Some(approved) = begin(&tx, &subjects).await? {
        tx.commit().await.map_err(invalid)?;
        return Ok(approved);
    }
    let mut approved = Vec::new();
    tx.commit().await.map_err(invalid)?;
    drop(client);
    let mut observations = Vec::new();
    for subject in &subjects {
        let observed = Box::pin(validation::execute(
            pool.clone(),
            kernel.clone(),
            subject.inspected.clone(),
            &subject.worker.path(),
            subject.fixture.clone(),
            subject.effective,
        ))
        .await?;
        observations.push(observed);
    }
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    for (subject, observed) in subjects.iter().zip(observations) {
        validation::verify(
            &tx,
            &subject.inspected,
            &subject.fixture,
            subject.effective,
            &hex::encode(subject.worker.checksum()),
            &observed,
        )
        .await?;
        let behavior = subject.record(
            subject.behavior,
            "behavior",
            json!({"header":subject.header,"observed":observed}),
        );
        evidence::retain_completed(
            &tx,
            subject.behavior,
            &behavior,
            subject.approval,
            &subject.approval_bytes(),
        )
        .await?;
    }
    for subject in &subjects {
        approved.push(
            read(&tx, subject)
                .await?
                .ok_or_else(|| invalid("encoding approval readback missing"))?,
        );
    }
    tx.commit().await.map_err(invalid)?;
    Ok(approved)
}
// Atomic group intent. No Tool dispatch or retry can occur before this commits.
async fn begin(
    tx: &Transaction<'_>,
    subjects: &[Arc<Subject>; 2],
) -> Result<Option<Vec<BootstrapEncodingApproval>>, RebornBuildError> {
    evidence::require_coherent(tx).await?;
    let mut approved = Vec::new();
    for subject in subjects {
        if let Some(value) = read(tx, subject).await? {
            approved.push(value);
        }
    }
    if approved.len() == 2 {
        return Ok(Some(approved));
    }
    if !approved.is_empty() {
        return Err(invalid(
            "encoding qualification group incomplete; reconcile without replay",
        ));
    }
    for subject in subjects {
        if evidence::read_evidence(tx, subject.q1).await?.is_some() {
            return Err(invalid(
                "encoding qualification intent incomplete; reconcile without replay",
            ));
        }
    }
    let enabled: bool = tx
        .query_one(
            "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
            &[&subjects[0].ids.tool],
        )
        .await
        .map_err(invalid)?
        .get(0);
    if !enabled {
        return Ok(Some(Vec::new()));
    }
    for subject in subjects {
        if !evidence::retain_intent(tx, subject.q1, &subject.q1_bytes()).await? {
            return Err(invalid("encoding intent already retained; no replay"));
        }
    }
    Ok(None)
}

async fn read(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
) -> Result<Option<BootstrapEncodingApproval>, RebornBuildError> {
    evidence::require_coherent(tx).await?;
    let Some(bytes) = evidence::read_association(tx, subject.approval).await? else {
        return Ok(None);
    };
    if bytes != subject.approval_bytes()
        || evidence::read_evidence(tx, subject.q1).await?.as_deref()
            != Some(subject.q1_bytes().as_str())
    {
        return Err(invalid("encoding qualification provenance differs"));
    }
    let declaration =
        AssociationApprovalDeclaration::from_json(&bytes, REVISION_LIMITS).map_err(invalid)?;
    let RetainedProgram::Tools(program) = subject.inspected.program() else {
        return Err(invalid("encoding binding missing"));
    };
    declaration
        .require_selected_combination(
            program.bindings()["0:2"].association(),
            program.bindings()["0:2"].combination(),
        )
        .map_err(invalid)?;
    if declaration.mode() != ValidationMode::SystemSeed || declaration.q2_reference().is_some() {
        return Err(invalid("encoding qualification mode differs"));
    }
    let instruction = program.inputs().instruction();
    let references: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect();
    PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[instruction.recipe().uuid],
        &references,
    )
    .await
    .map_err(invalid)?;
    let behavior = evidence::read_evidence(tx, subject.behavior)
        .await?
        .ok_or_else(|| invalid("encoding behavior missing"))?;
    let record = strict_json(&behavior, REVISION_LIMITS).map_err(invalid)?;
    let report = &record["report"];
    if report.as_object().is_none_or(|v| v.len() != 2)
        || report["header"] != subject.header
        || behavior != subject.record(subject.behavior, "behavior", report.clone())
    {
        return Err(invalid("encoding behavior producer differs"));
    }
    validation::verify(
        tx,
        &subject.inspected,
        &subject.fixture,
        subject.effective,
        &hex::encode(subject.worker.checksum()),
        &report["observed"],
    )
    .await?;
    Ok(Some(BootstrapEncodingApproval {
        declaration,
        subject: subject.clone(),
    }))
}

#[cfg(test)]
pub(super) async fn assert_retained_proof_checks_actual_journal(
    pool: Arc<PgPool>,
    approvals: &[BootstrapEncodingApproval],
) {
    assert_eq!(approvals.len(), 2);
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    for approval in approvals {
        approval.verify_in_transaction(&tx).await.unwrap();
        let subject = &approval.subject;
        let bytes = evidence::read_evidence(&tx, subject.behavior)
            .await
            .unwrap()
            .unwrap();
        let record: Value = serde_json::from_str(&bytes).unwrap();
        let observed = &record["report"]["observed"];
        for change in [
            "missing-case",
            "wrong-run",
            "wrong-answer",
            "wrong-result",
            "duplicate-case",
            "quality-claim",
        ] {
            let mut changed = observed.clone();
            match change {
                "missing-case" => {
                    changed["cases"].as_array_mut().unwrap().pop();
                }
                "wrong-run" => changed["cases"][0]["run_id"] = json!(Uuid::new_v4()),
                "wrong-answer" => changed["cases"][0]["answer_checksum"] = json!("0".repeat(64)),
                "wrong-result" => changed["cases"][0]["result_checksum"] = json!("0".repeat(64)),
                "duplicate-case" => changed["cases"][1] = changed["cases"][0].clone(),
                "quality-claim" => changed["model_quality"] = json!("accepted"),
                _ => unreachable!(),
            }
            assert!(
                validation::verify(
                    &tx,
                    &subject.inspected,
                    &subject.fixture,
                    subject.effective,
                    &hex::encode(subject.worker.checksum()),
                    &changed
                )
                .await
                .is_err(),
                "{change}"
            );
        }
        // Re-reading proof must be observation-only, never replay an invocation.
        let before: i64 = tx
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get(0);
        read(&tx, subject).await.unwrap().unwrap();
        let after: i64 = tx
            .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(before, after);
    }
    let subjects: [Arc<Subject>; 2] = approvals
        .iter()
        .map(|a| a.subject.clone())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|_| panic!("two subjects required"));
    assert_eq!(begin(&tx, &subjects).await.unwrap().unwrap().len(), 2);
    tx.commit().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let mut pending = Vec::new();
    for subject in subjects {
        let mut altered = (*subject).clone();
        let identity = Uuid::new_v5(
            &altered.approval,
            b"rollback-only-incomplete-encoding-test/1",
        );
        altered.header["test_scope"] = json!("rollback-only-incomplete-encoding-test/1");
        altered.q1 = Uuid::new_v5(&identity, b"q1");
        altered.behavior = Uuid::new_v5(&identity, b"behavior");
        altered.approval = Uuid::new_v5(&identity, b"approval");
        pending.push(Arc::new(altered));
    }
    let pending: [Arc<Subject>; 2] = pending
        .try_into()
        .unwrap_or_else(|_| panic!("two pending subjects required"));
    let before: i64 = tx
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    assert!(
        evidence::retain_intent(&tx, pending[0].q1, &pending[0].q1_bytes())
            .await
            .unwrap()
    );
    assert!(begin(&tx, &pending).await.is_err());
    assert!(
        evidence::read_evidence(&tx, pending[1].q1)
            .await
            .unwrap()
            .is_none()
    );
    let after: i64 = tx
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(before, after);
    tx.rollback().await.unwrap();
}
