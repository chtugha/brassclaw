//! Closed first-party qualification of one exact packaged pair workflow.
//! Reuses existing leaf approvals and workflow evidence storage. This neither
//! approves a target/model design nor activates a prefix or a catalogue route.
use crate::bootstrap_workflow_evidence as evidence;
#[path = "bootstrap_pair_fixture.rs"]
mod fixture;
#[path = "bootstrap_pair_validation.rs"]
mod validation;
use crate::{
    RebornBuildError, bootstrap_compiler_approval::BootstrapCompilerApproval,
    bootstrap_encoding_approval::BootstrapEncodingApproval, bootstrap_reply_validation::invalid,
    monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_host_api::CapabilityId;
use brassclaw_host_runtime::PackagedExecutableImage;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::AssociationApprovalDeclaration, component_revision::REVISION_LIMITS,
    revision_store::PgComponentRevisionStore, value_contract::strict_json,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

pub(super) struct BootstrapPairQualification {
    subject: Arc<Subject>,
}
impl BootstrapPairQualification {
    pub(super) fn program(&self) -> Arc<InspectedRetainedProgram> {
        self.subject.inspected.clone()
    }
    pub(super) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        read(tx, &self.subject)
            .await?
            .ok_or_else(|| invalid("pair qualification missing"))?;
        Ok(())
    }
}
#[derive(Clone)]
struct Subject {
    inspected: Arc<InspectedRetainedProgram>,
    worker: Arc<PackagedExecutableImage>,
    fixture: Arc<fixture::Fixture>,
    digest: BootstrapCompilerApproval,
    encoding: BootstrapEncodingApproval,
    header: Value,
    q1: Uuid,
    behavior: Uuid,
}
impl Subject {
    async fn prepare(
        kernel: &dyn MontyKernelSnapshot,
        candidate: &InspectedRetainedProgram,
        worker: Arc<PackagedExecutableImage>,
        fixture: Arc<fixture::Fixture>,
        digest: BootstrapCompilerApproval,
        encoding: BootstrapEncodingApproval,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let RetainedProgram::Tools(original) = candidate.program() else {
            return Err(invalid("pair Tool program required"));
        };
        let json_tool = original
            .bindings()
            .get("0:3")
            .ok_or_else(|| invalid("pair digest binding missing"))?
            .tool()
            .uuid;
        let model_tool = original
            .bindings()
            .get("0:5")
            .filter(|_| original.bindings().len() == 2)
            .ok_or_else(|| invalid("pair encoding binding missing"))?
            .tool()
            .uuid;
        let (ids, drafts) =
            crate::prefix_pair_candidates::definitions(kernel, json_tool, model_tool).await?;
        let mut native = Vec::new();
        for capability in [
            brassclaw_host_runtime::JSON_CAPABILITY_ID,
            brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID,
        ] {
            let runtime = kernel
                .builtin(&CapabilityId::new(capability).map_err(invalid)?, None)
                .map_err(invalid)?;
            native.push(
                serde_json::to_value(
                    runtime
                        .native_identity()
                        .ok_or_else(|| invalid("pair native identity missing"))?,
                )
                .map_err(invalid)?,
            );
        }
        let wanted: BTreeMap<_, _> = drafts.iter().map(|d| (d.uuid(), d)).collect();
        let instruction = original.inputs().instruction();
        if instruction.recipe().uuid != ids.recipe
            || instruction.snapshot().roots().len() != 1
            || !instruction.association_approvals().is_empty()
        {
            return Err(invalid("pair candidate root differs"));
        }
        for (uuid, actual) in instruction.snapshot().revisions() {
            if wanted
                .get(uuid)
                .is_none_or(|draft| draft.exact_bytes() != actual.draft().exact_bytes())
            {
                return Err(invalid("pair requires exact packaged definitions"));
            }
        }
        // Reassemble only these exact roots/revisions, never allocation heads.
        let snapshot = Arc::new(
            instruction
                .snapshot()
                .select_roots(&[ids.recipe])
                .map_err(invalid)?,
        );
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                snapshot,
                ids.recipe,
                "selected",
                WorkflowClass::Deterministic,
            )
            .map_err(invalid)?,
        )
        .map_err(invalid)?;
        let approvals: BTreeMap<String, &AssociationApprovalDeclaration> = BTreeMap::from([
            (
                "0:3".into(),
                digest
                    .usage_approval(&program.bindings()["0:3"])
                    .ok_or_else(|| invalid("pair digest qualification differs"))?,
            ),
            (
                "0:5".into(),
                encoding
                    .usage_approval(&program.bindings()["0:5"])
                    .ok_or_else(|| invalid("pair encoding qualification differs"))?,
            ),
        ]);
        let approval_refs:BTreeMap<_,_>=approvals.iter().map(|(step,approval)|(step.clone(),json!({"approval_id":approval.approval_id(),"checksum":crate::bootstrap_evidence::checksum(approval.exact_bytes())}))).collect();
        let program = program
            .with_association_approvals(&approvals)
            .map_err(invalid)?;
        drop(approvals);
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(Arc::new(program)),
                &worker.path(),
                candidate.preparation_limits(),
            )
            .await
            .map_err(invalid)?,
        );
        let selection = inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?;
        let sources: BTreeMap<_, _> = inspected
            .source_checks()
            .iter()
            .map(|(uuid, source)| (uuid.to_string(), source.observations()))
            .collect();
        let header = json!({"producer":"brassclaw-prefix-pair-case-bootstrap/1","scope":"exact-packaged-pair-case-workflow/1",
            "selection_checksum":hex::encode(selection.checksum()),"worker_checksum":hex::encode(worker.checksum()),"native":native,"leaf_approvals":approval_refs,"sources":sources,
            "specification":fixture.specification,"artifact_set":fixture.artifacts,"cases":fixture::CASES,
            "owner_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_pair_approval.rs")),
            "definitions_checksum":crate::bootstrap_evidence::checksum(include_str!("prefix_pair_candidates.rs")),
            "fixture_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_pair_fixture.rs")),
            "validator_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_pair_validation.rs")),
            "storage_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_workflow_evidence.rs")),
            "flow_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_recipe_flow.rs")),
            "root_checksum":crate::bootstrap_evidence::checksum(crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED)});
        let identity = Uuid::new_v5(&ids.recipe, header.to_string().as_bytes());
        Ok(Arc::new(Self {
            inspected,
            worker,
            fixture,
            digest,
            encoding,
            header,
            q1: Uuid::new_v5(&identity, b"q1"),
            behavior: Uuid::new_v5(&identity, b"behavior"),
        }))
    }
    fn record(&self, id: Uuid, kind: &str, report: Value) -> String {
        json!({"format":"workflow-review-evidence/1","evidence_id":id,"kind":kind,"validation_mode":"system_seed","selection_checksum":self.header["selection_checksum"],"succeeded":true,"report":report}).to_string()
    }
    fn q1_bytes(&self) -> String {
        self.record(self.q1,"q1",json!({"header":self.header,"checks":["exact-packaged-root-and-recursive-dependencies","both-immutable-leaf-qualifications-pinned-before-effects","retained-native-worker-and-source-integrity","typed-inputs-effect-free-preload-and-export-resolution","independent-original-render-token-and-comparison-oracles"],"model_quality":"unmeasured","cache_quality":"unmeasured"}))
    }
}

pub(super) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    candidate: &InspectedRetainedProgram,
    digest: Option<&BootstrapCompilerApproval>,
    encodings: &[BootstrapEncodingApproval],
    worker_path: &Path,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
) -> Result<Option<BootstrapPairQualification>, RebornBuildError> {
    let Some(digest) = digest else {
        return Ok(None);
    };
    if encodings.is_empty() {
        return Ok(None);
    }
    let RetainedProgram::Tools(program) = candidate.program() else {
        return Err(invalid("pair candidate Tool program missing"));
    };
    let binding = program
        .bindings()
        .get("0:5")
        .ok_or_else(|| invalid("pair candidate binding missing"))?;
    let encoding = encodings
        .iter()
        .find(|a| a.usage_approval(binding).is_some())
        .ok_or_else(|| invalid("pair candidate encoding combination unqualified"))?;
    let worker = Arc::new(
        PackagedExecutableImage::capture(
            worker_path,
            candidate.preparation_limits().max_native_artifact_bytes,
        )
        .await
        .map_err(invalid)?,
    );
    let fixture = fixture::Fixture::retain(pool.clone()).await?;
    let subject = Subject::prepare(
        kernel.as_ref(),
        candidate,
        worker,
        fixture,
        digest.clone(),
        encoding.clone(),
    )
    .await?;
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    if let Some(approved) = read(&tx, &subject).await? {
        tx.commit().await.map_err(invalid)?;
        return Ok(Some(approved));
    }
    let tool_ids: Vec<_> = subject
        .inspected
        .program()
        .inputs()
        .instruction()
        .snapshot()
        .revisions()
        .values()
        .filter(|r| r.reference().class_code == 0)
        .map(|r| r.reference().uuid)
        .collect();
    let policies = tx
        .query(
            "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=ANY($1)",
            &[&tool_ids],
        )
        .await
        .map_err(invalid)?;
    if tool_ids.len() != 2 || policies.len() != 2 {
        return Err(invalid("pair Tool policies missing or ambiguous"));
    }
    if policies.iter().any(|row| !row.get::<_, bool>(0)) {
        tx.commit().await.map_err(invalid)?;
        return Ok(None);
    }
    begin(&tx, &subject).await?;
    tx.commit().await.map_err(invalid)?;
    drop(client);
    let observed = Box::pin(validation::execute(
        executor.clone(),
        pool.clone(),
        kernel,
        subject.inspected.clone(),
        &subject.worker.path(),
        &hex::encode(subject.worker.checksum()),
        subject.fixture.clone(),
    ))
    .await?;
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    validation::verify(
        &tx,
        &subject.inspected,
        &subject.fixture,
        &hex::encode(subject.worker.checksum()),
        &observed,
    )
    .await?;
    let behavior = subject.record(
        subject.behavior,
        "behavior",
        json!({"header":subject.header,"observed":observed}),
    );
    evidence::retain_workflow(
        &tx,
        &subject.inspected,
        subject.behavior,
        evidence::WorkflowKind::Behavior,
        &behavior,
    )
    .await?;
    let approved = read(&tx, &subject)
        .await?
        .ok_or_else(|| invalid("pair qualification readback missing"))?;
    tx.commit().await.map_err(invalid)?;
    Ok(Some(approved))
}
async fn begin(tx: &Transaction<'_>, subject: &Arc<Subject>) -> Result<(), RebornBuildError> {
    subject.digest.verify_in_transaction(tx).await?;
    subject.encoding.verify_in_transaction(tx).await?;
    if evidence::read_workflow(
        tx,
        &subject.inspected,
        subject.q1,
        evidence::WorkflowKind::Q1,
    )
    .await?
    .is_some()
        || !evidence::retain_workflow(
            tx,
            &subject.inspected,
            subject.q1,
            evidence::WorkflowKind::Q1,
            &subject.q1_bytes(),
        )
        .await?
    {
        return Err(invalid(
            "pair qualification intent incomplete; reconcile without replay",
        ));
    }
    Ok(())
}
async fn read(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
) -> Result<Option<BootstrapPairQualification>, RebornBuildError> {
    let Some(bytes) = evidence::read_workflow(
        tx,
        &subject.inspected,
        subject.behavior,
        evidence::WorkflowKind::Behavior,
    )
    .await?
    else {
        return Ok(None);
    };
    if evidence::read_workflow(
        tx,
        &subject.inspected,
        subject.q1,
        evidence::WorkflowKind::Q1,
    )
    .await?
    .as_deref()
        != Some(subject.q1_bytes().as_str())
    {
        return Err(invalid("pair Q1 provenance differs"));
    }
    subject.digest.verify_in_transaction(tx).await?;
    subject.encoding.verify_in_transaction(tx).await?;
    let instruction = subject.inspected.program().inputs().instruction();
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
    let record = strict_json(&bytes, REVISION_LIMITS).map_err(invalid)?;
    let report = &record["report"];
    if report.as_object().is_none_or(|v| v.len() != 2)
        || report["header"] != subject.header
        || bytes != subject.record(subject.behavior, "behavior", report.clone())
    {
        return Err(invalid("pair behavior provenance differs"));
    }
    validation::verify(
        tx,
        &subject.inspected,
        &subject.fixture,
        &hex::encode(subject.worker.checksum()),
        &report["observed"],
    )
    .await?;
    Ok(Some(BootstrapPairQualification {
        subject: subject.clone(),
    }))
}

#[cfg(test)]
pub(super) async fn assert_retained_proof_checks_journal(
    pool: Arc<PgPool>,
    qualified: &BootstrapPairQualification,
    kernel: &dyn MontyKernelSnapshot,
) {
    let subject = &qualified.subject;
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let before: i64 = tx
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    qualified.verify_in_transaction(&tx).await.unwrap();
    let bytes = evidence::read_workflow(
        &tx,
        &subject.inspected,
        subject.behavior,
        evidence::WorkflowKind::Behavior,
    )
    .await
    .unwrap()
    .unwrap();
    let record: Value = serde_json::from_str(&bytes).unwrap();
    let observed = &record["report"]["observed"];
    for change in [
        "missing-case",
        "wrong-step-name",
        "wrong-answer",
        "wrong-input",
        "false-receipt",
        "quality-claim",
        "captured-limits",
        "vm-profile-anchor",
    ] {
        let mut changed = observed.clone();
        match change {
            "missing-case" => {
                changed["cases"].as_array_mut().unwrap().pop();
            }
            "wrong-step-name" => {
                let steps = changed["cases"][0]["steps"].as_object_mut().unwrap();
                let proof = steps.remove("0:6").unwrap();
                steps.insert("0:99".into(), proof);
            }
            "wrong-answer" => changed["cases"][0]["steps"]["0:5"]["answer"] = json!("0".repeat(64)),
            "wrong-input" => changed["cases"][0]["inputs_checksum"] = json!("0".repeat(64)),
            "false-receipt" => changed["cases"][0]["execution"]["internal_receipt"] = json!("{}"),
            "quality-claim" => changed["model_quality"] = json!("accepted"),
            "vm-profile-anchor" => {
                let previous = changed["cases"][0]["data_limits"]["max_nodes"]
                    .as_u64()
                    .unwrap();
                changed["cases"][0]["data_limits"]["max_nodes"] = json!(previous + 1);
            }
            "captured-limits" => {
                let previous = changed["cases"][0]["steps"]["0:1"]["limits"]["max_bytes"]
                    .as_u64()
                    .unwrap();
                changed["cases"][0]["steps"]["0:1"]["limits"]["max_bytes"] = json!(previous + 1)
            }
            _ => unreachable!(),
        }
        assert!(
            validation::verify(
                &tx,
                &subject.inspected,
                &subject.fixture,
                &hex::encode(subject.worker.checksum()),
                &changed
            )
            .await
            .is_err(),
            "{change}"
        );
    }
    for change in ["authored", "failure", "fake-q2"] {
        let id = Uuid::new_v4();
        let mut changed: Value = serde_json::from_str(&subject.q1_bytes()).unwrap();
        changed["evidence_id"] = json!(id);
        match change {
            "authored" => changed["validation_mode"] = json!("authored"),
            "failure" => changed["succeeded"] = json!(false),
            "fake-q2" => changed["q2_ref"] = json!(Uuid::new_v4()),
            _ => unreachable!(),
        }
        assert!(
            evidence::retain_workflow(
                &tx,
                &subject.inspected,
                id,
                evidence::WorkflowKind::Q1,
                &changed.to_string()
            )
            .await
            .is_err()
        );
        assert!(
            evidence::read_workflow(&tx, &subject.inspected, id, evidence::WorkflowKind::Q1)
                .await
                .unwrap()
                .is_none()
        );
    }
    read(&tx, subject).await.unwrap().unwrap();
    assert_eq!(
        before,
        tx.query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
    );
    tx.commit().await.unwrap();
    // Even an inspectable, structurally identical successor with changed prose
    // is not this packaged subject. No draft/label may borrow its qualification.
    let instruction = subject.inspected.program().inputs().instruction();
    let root = instruction.recipe().uuid;
    let revisions = instruction
        .snapshot()
        .revisions()
        .iter()
        .map(|(uuid, revision)| {
            if *uuid != root {
                return revision.clone();
            }
            let mut value: Value = serde_json::from_str(revision.draft().exact_bytes()).unwrap();
            value["document"]["description"] =
                json!("Unpackaged candidate must not inherit installed proof");
            brassclaw_skills::component_revision::ComponentRevisionDraft::from_json(
                &value.to_string(),
            )
            .unwrap()
            .at_version(revision.reference().version + 1)
            .unwrap()
        })
        .collect();
    let graph =
        brassclaw_skills::component_revision::RetainedComponentSnapshot::new(&[root], revisions)
            .unwrap();
    let program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(graph),
            root,
            "selected",
            WorkflowClass::Deterministic,
        )
        .unwrap(),
    )
    .unwrap();
    let altered = InspectedRetainedProgram::inspect_with_limits(
        RetainedProgram::Tools(Arc::new(program)),
        &subject.worker.path(),
        subject.inspected.preparation_limits(),
    )
    .await
    .unwrap();
    assert!(
        Subject::prepare(
            kernel,
            &altered,
            subject.worker.clone(),
            subject.fixture.clone(),
            subject.digest.clone(),
            subject.encoding.clone()
        )
        .await
        .is_err()
    );
    // A distinct rollback-only pending subject cannot replay the workflow or
    // replace immutable completed qualification records.
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let mut pending = (**subject).clone();
    let identity = Uuid::new_v5(&pending.behavior, b"rollback-only-incomplete-pair/1");
    pending.header["test_scope"] = json!("rollback-only-incomplete-pair/1");
    pending.q1 = Uuid::new_v5(&identity, b"q1");
    pending.behavior = Uuid::new_v5(&identity, b"behavior");
    let pending = Arc::new(pending);
    assert!(
        evidence::retain_workflow(
            &tx,
            &pending.inspected,
            pending.q1,
            evidence::WorkflowKind::Q1,
            &pending.q1_bytes()
        )
        .await
        .unwrap()
    );
    assert!(begin(&tx, &pending).await.is_err());
    assert!(read(&tx, &pending).await.unwrap().is_none());
    assert_eq!(
        before,
        tx.query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0)
    );
    tx.rollback().await.unwrap();
}
