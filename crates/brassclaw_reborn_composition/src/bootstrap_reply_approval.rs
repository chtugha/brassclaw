//! Trusted first-party bootstrap owner for one exact packaged public reply
//! combination. No authored input, human-Q2 bypass, activation or Tool grant.
use crate::{
    RebornBuildError,
    bootstrap_reply_validation::{self as validation, hex, invalid},
    monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedStepFailure, port_answer_checksum, typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::{AssociationApprovalDeclaration, ValidationMode},
    component_revision::REVISION_LIMITS,
    global_bootstrap_components::{UsageComponentIds, public_reply_drafts},
    revision_store::PgComponentRevisionStore,
    value_contract::strict_json,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path, sync::Arc};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

/// Only this owner can construct it, after committed exact evidence is read.
/// This is neither a catalogue selection nor permission to invoke a Tool.
pub(crate) struct BootstrapReplyApproval {
    approval: AssociationApprovalDeclaration,
    selection: [u8; 32],
    subject: Arc<Subject>,
}
impl BootstrapReplyApproval {
    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        let actual = read(tx, &self.subject)
            .await?
            .ok_or_else(|| invalid("activated bootstrap approval missing"))?;
        if actual.approval_id() != self.approval_id() || actual.selection != self.selection {
            return Err(invalid("activated bootstrap approval differs"));
        }
        Ok(())
    }
    pub(crate) fn pin_program(
        &self,
        program: brassclaw_engine::memory::retained_tools::RetainedToolProgram,
    ) -> Result<brassclaw_engine::memory::retained_tools::RetainedToolProgram, RebornBuildError>
    {
        if program
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .checksum()
            != self.selection
        {
            return Err(invalid(
                "bootstrap approval subject differs from activation",
            ));
        }
        program
            .with_association_approvals(&std::collections::BTreeMap::from([(
                "0:2".to_owned(),
                &self.approval,
            )]))
            .map_err(invalid)
    }
    pub(crate) async fn activated_program(
        &self,
        pool: Arc<PgPool>,
    ) -> Result<Arc<InspectedRetainedProgram>, RebornBuildError> {
        use brassclaw_engine::memory::{
            retained_instruction::compile_retained_recipe,
            retained_tools::prepare_retained_tool_program,
        };
        let instruction = self.subject.inspected.program().inputs().instruction();
        let refs: Vec<_> = instruction
            .snapshot()
            .revisions()
            .values()
            .map(|r| r.reference())
            .collect();
        let snapshot = PgComponentRevisionStore::new(pool)
            .read_exact(&[instruction.recipe().uuid], &refs)
            .await
            .map_err(invalid)?;
        let compiled = compile_retained_recipe(
            Arc::new(snapshot),
            instruction.recipe().uuid,
            &instruction.variant().variant_key,
            instruction.class(),
        )
        .map_err(invalid)?;
        let program =
            self.pin_program(prepare_retained_tool_program(compiled).map_err(invalid)?)?;
        Ok(Arc::new(
            InspectedRetainedProgram::inspect(
                RetainedProgram::Tools(Arc::new(program)),
                &self.subject.worker_path,
            )
            .await
            .map_err(invalid)?,
        ))
    }
    /// Compare the entire reviewed workflow with its exact approval pins, not
    /// merely a compatible usage association or a Recipe's source label.
    pub(crate) fn require_selected_program(
        &self,
        selected: &InspectedRetainedProgram,
    ) -> Result<(), RebornBuildError> {
        let raw = self
            .subject
            .inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?;
        let mut expected: Value = serde_json::from_str(raw.exact_bytes()).map_err(invalid)?;
        expected["association_approvals"] = json!({"0:2": {
            "approval_id": self.approval.approval_id(),
            "checksum": hex(Sha256::digest(self.approval.exact_bytes().as_bytes()).into())
        }});
        let expected_bytes = serde_json::to_string(&expected).map_err(invalid)?;
        if selected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .exact_bytes()
            != expected_bytes
        {
            return Err(invalid(
                "public command differs from trusted activated workflow",
            ));
        }
        Ok(())
    }
    pub(crate) fn approval_id(&self) -> Uuid {
        self.approval.approval_id()
    }
    pub(crate) fn selection_checksum(&self) -> [u8; 32] {
        self.selection
    }
}
struct Subject {
    inspected: Arc<InspectedRetainedProgram>,
    header: Value,
    components: Value,
    association: String,
    q1: Uuid,
    behavior: Uuid,
    approval: Uuid,
    selection: [u8; 32],
    worker_path: std::path::PathBuf,
    _worker: brassclaw_host_runtime::PackagedExecutableImage,
}
impl Subject {
    async fn prepare(
        inspected: Arc<InspectedRetainedProgram>,
        worker: &Path,
    ) -> Result<Self, RebornBuildError> {
        // Own validation bytes rather than execute a mutable package pathname
        // after hashing. Repeat inspection under that same interpreter.
        let worker_directory =
            brassclaw_host_runtime::PackagedExecutableImage::capture(worker, 256 * 1024 * 1024)
                .await
                .map_err(invalid)?;
        let worker_path = worker_directory.path();
        let worker_checksum = hex(worker_directory.checksum());
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect(inspected.program().clone(), &worker_path)
                .await
                .map_err(invalid)?,
        );
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(invalid("bootstrap requires exact Tool usage"));
        };
        let binding = program
            .bindings()
            .get("0:2")
            .filter(|_| program.bindings().len() == 1)
            .ok_or_else(|| invalid("bootstrap usage differs"))?;
        let instruction = program.inputs().instruction();
        let ids = UsageComponentIds {
            recipe: instruction.recipe().uuid,
            skill: binding.skill().uuid,
            python_code: binding.python().uuid,
            tool_skill: binding.tool_skill().uuid,
            tool: binding.tool().uuid,
            formatter: None,
        };
        for (actual, name) in [
            (ids.recipe, "reply:recipe"),
            (ids.skill, "reply:skill"),
            (ids.python_code, "reply:code"),
            (ids.tool_skill, "reply:binding"),
        ] {
            if actual != crate::installed_monty_catalogue::id(name) {
                return Err(invalid(
                    "bootstrap identity is not the shipped reply component",
                ));
            }
        }
        for expected in public_reply_drafts(ids).map_err(invalid)? {
            if instruction
                .snapshot()
                .revisions()
                .get(&expected.uuid())
                .is_none_or(|actual| actual.draft().exact_bytes() != expected.exact_bytes())
            {
                return Err(invalid("only exact bundled public reply enters bootstrap"));
            }
        }
        let image = brassclaw_host_runtime::NativeExecutableImage::capture(1024 * 1024 * 1024)
            .await
            .map_err(invalid)?;
        let tool = instruction.snapshot().revisions()[&binding.tool().uuid]
            .draft()
            .document();
        if tool["implementation"]["format"] != "packaged-task-reply/1"
            || tool["implementation"]["adapter"] != "TaskReplyCapability/1"
            || tool["implementation"]["artifact_checksum"] != hex(image.checksum())
            || binding.capability_id() != "host.post_reply"
        {
            return Err(invalid("bootstrap reply implementation integrity differs"));
        }
        let selected = instruction.retained_selection().map_err(invalid)?;
        let selection = selected.checksum();
        let components = json!(binding.combination().iter().map(|reference|json!({
            "uuid":reference.uuid,"class_code":reference.class_code,"version":reference.version,"checksum":hex(reference.checksum)
        })).collect::<Vec<_>>());
        let association = binding.association().exact_bytes().to_owned();
        let mut sources = serde_json::Map::new();
        for id in inspected.preload_order() {
            let source = inspected
                .source_checks()
                .get(id)
                .ok_or_else(|| invalid("bootstrap preload inspection missing"))?;
            sources.insert(id.to_string(), json!(source.observations()));
        }
        let header = json!({"producer":validation::PRODUCER,"scope":"packaged-public-literal-reply/1",
            "selection_checksum":hex(selection),"worker_checksum":worker_checksum,
            "application_checksum":hex(image.checksum()),"cases":validation::cases(),"sources":sources,
            "validator_root_checksum":validation::root_checksum()});
        let identity = Uuid::new_v5(&ids.recipe, header.to_string().as_bytes());
        Ok(Self {
            inspected,
            header,
            components,
            association,
            selection,
            q1: Uuid::new_v5(&identity, b"q1"),
            behavior: Uuid::new_v5(&identity, b"behavior"),
            approval: Uuid::new_v5(&identity, b"approval"),
            worker_path,
            _worker: worker_directory,
        })
    }
    fn evidence(&self, id: Uuid, kind: &str, report: Value) -> String {
        json!({"format":"component-review-evidence/1","evidence_id":id,"kind":kind,
            "validation_mode":"system_seed","association_checksum":hex(Sha256::digest(self.association.as_bytes()).into()),
            "components":self.components,"succeeded":true,"reviewed_evidence":[],"report":report}).to_string()
    }
    fn q1_bytes(&self) -> String {
        self.evidence(self.q1,"q1",json!({"header":self.header,"checks":["exact-bundled-definitions","retained-typed-ibs-binding","actual-monty-preload-export-inspection","retained-application-and-worker-integrity"]}))
    }
    fn approval_bytes(&self) -> String {
        json!({"format":"skill-association-approval/1","approval_id":self.approval,
            "association_checksum":hex(Sha256::digest(self.association.as_bytes()).into()),"components":self.components,
            "validation_mode":"system_seed","q1_ref":self.q1,"q2_ref":null,"behavioral_refs":[self.behavior]}).to_string()
    }
}

/// Retain Q1 before effects as a no-replay bootstrap intent. Existing incomplete
/// evidence requires supervised recovery; no restart or concurrent boot replays.
/// A globally blocked Tool leaves an unqualified candidate, without overriding it.
pub(crate) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    candidate: Arc<InspectedRetainedProgram>,
    worker: &Path,
) -> Result<Option<BootstrapReplyApproval>, RebornBuildError> {
    let subject = Arc::new(Subject::prepare(candidate, worker).await?);
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    if let Some(approval) = reuse_or_require_unstarted(&tx, &subject).await? {
        tx.commit().await.map_err(invalid)?;
        return Ok(Some(approval));
    }
    let RetainedProgram::Tools(program) = subject.inspected.program() else {
        unreachable!()
    };
    let tool = program.bindings()["0:2"].tool().uuid;
    let row = tx
        .query_one(
            "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
            &[&tool],
        )
        .await
        .map_err(invalid)?;
    if !row.get::<_, bool>(0) {
        tx.commit().await.map_err(invalid)?;
        return Ok(None);
    }
    let q1 = subject.q1_bytes();
    let inserted = tx
        .execute(
            "INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum)
        VALUES($1,$2,$3) ON CONFLICT(evidence_id) DO NOTHING",
            &[&subject.q1, &q1, &digest(&q1)],
        )
        .await
        .map_err(invalid)?;
    tx.commit().await.map_err(invalid)?;
    drop(client);
    if inserted != 1 {
        return Err(invalid(
            "bootstrap evidence is incomplete; supervised reconciliation required, no automatic replay",
        ));
    }
    // Keep the contained execution state off the startup future's stack.
    let observations = Box::pin(validation::execute_cases(
        pool.clone(),
        kernel,
        subject.inspected.clone(),
        &subject.worker_path,
    ))
    .await?;
    let behavior = subject.evidence(
        subject.behavior,
        "behavior",
        json!({"header":subject.header,"observed":observations}),
    );
    let approval = subject.approval_bytes();
    // Private producers own these bytes. No authored/client success record enters.
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    tx.execute("INSERT INTO reborn_component_review_evidence(evidence_id,evidence_bytes,checksum) VALUES($1,$2,$3)",&[&subject.behavior,&behavior,&digest(&behavior)]).await.map_err(invalid)?;
    tx.execute("INSERT INTO reborn_skill_association_approvals(approval_id,approval_bytes,checksum) VALUES($1,$2,$3)",&[&subject.approval,&approval,&digest(&approval)]).await.map_err(invalid)?;
    let retained = read(&tx, &subject)
        .await?
        .ok_or_else(|| invalid("bootstrap approval readback missing"))?;
    tx.commit().await.map_err(invalid)?;
    Ok(Some(retained))
}
fn digest(value: &str) -> String {
    hex(Sha256::digest(value.as_bytes()).into())
}
async fn reuse_or_require_unstarted(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
) -> Result<Option<BootstrapReplyApproval>, RebornBuildError> {
    let approval = read(tx, subject).await?;
    if approval.is_none() && evidence(tx, subject.q1).await?.is_some() {
        return Err(invalid(
            "bootstrap evidence is incomplete; supervised reconciliation required, no automatic replay",
        ));
    }
    Ok(approval)
}
async fn evidence(tx: &Transaction<'_>, id: Uuid) -> Result<Option<String>, RebornBuildError> {
    let row=tx.query_opt("SELECT evidence_bytes,checksum FROM reborn_component_review_evidence WHERE evidence_id=$1",&[&id]).await.map_err(invalid)?;
    row.map(|row| {
        let bytes: String = row.get(0);
        if digest(&bytes) != row.get::<_, String>(1) {
            Err(invalid("bootstrap evidence integrity failed"))
        } else {
            Ok(bytes)
        }
    })
    .transpose()
}
async fn read(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
) -> Result<Option<BootstrapReplyApproval>, RebornBuildError> {
    let coherent: bool = tx
        .query_one(
            "SELECT current_setting('transaction_isolation') IN ('repeatable read','serializable')",
            &[],
        )
        .await
        .map_err(invalid)?
        .get(0);
    if !coherent {
        return Err(invalid(
            "bootstrap approval requires coherent database view",
        ));
    }
    let row=tx.query_opt("SELECT approval_bytes,checksum FROM reborn_skill_association_approvals WHERE approval_id=$1",&[&subject.approval]).await.map_err(invalid)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let bytes: String = row.get(0);
    if bytes != subject.approval_bytes() || digest(&bytes) != row.get::<_, String>(1) {
        return Err(invalid("bootstrap approval integrity differs"));
    }
    let declaration =
        AssociationApprovalDeclaration::from_json(&bytes, REVISION_LIMITS).map_err(invalid)?;
    let RetainedProgram::Tools(program) = subject.inspected.program() else {
        return Err(invalid("bootstrap usage missing"));
    };
    let binding = &program.bindings()["0:2"];
    declaration
        .require_selected_combination(binding.association(), binding.combination())
        .map_err(invalid)?;
    if declaration.mode() != ValidationMode::SystemSeed || declaration.q2_reference().is_some() {
        return Err(invalid("bootstrap provenance mode differs"));
    }
    let instruction = program.inputs().instruction();
    let references: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|revision| revision.reference())
        .collect();
    PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[instruction.recipe().uuid],
        &references,
    )
    .await
    .map_err(invalid)?;
    if evidence(tx, subject.q1).await?.as_deref() != Some(subject.q1_bytes().as_str()) {
        return Err(invalid("bootstrap Q1 provenance differs"));
    }
    let behavior = evidence(tx, subject.behavior)
        .await?
        .ok_or_else(|| invalid("bootstrap behavior missing"))?;
    let record = strict_json(&behavior, REVISION_LIMITS).map_err(invalid)?;
    let report = &record["report"];
    if report.as_object().is_none_or(|report| report.len() != 2)
        || report["header"] != subject.header
        || behavior != subject.evidence(subject.behavior, "behavior", report.clone())
    {
        return Err(invalid("bootstrap behavior provenance differs"));
    }
    verify_observations(tx, subject, &report["observed"]).await?;
    Ok(Some(BootstrapReplyApproval {
        approval: declaration,
        selection: subject.selection,
        subject: subject.clone(),
    }))
}
async fn verify_observations(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
    observed: &Value,
) -> Result<(), RebornBuildError> {
    let RetainedProgram::Tools(program) = subject.inspected.program() else {
        return Err(invalid("bootstrap usage missing"));
    };
    let invocation =
        super::pg_monty_admission::PgMontyAdmission::invocation_selection(program, "0:2")
            .map_err(invalid)?;
    let selection = program
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    if observed.as_object().is_none_or(|object| object.len() != 3)
        || observed["invalid_inputs_rejected"] != true
        || observed["root_checksum"] != validation::root_checksum()
    {
        return Err(invalid("bootstrap behavior report differs"));
    }
    let cases = validation::cases();
    let wanted = cases
        .as_array()
        .ok_or_else(|| invalid("bootstrap cases missing"))?;
    let actual = observed["cases"]
        .as_array()
        .filter(|actual| actual.len() == wanted.len())
        .ok_or_else(|| invalid("bootstrap case coverage differs"))?;
    let mut runs = BTreeSet::new();
    let mut threads = BTreeSet::new();
    let mut roots = BTreeSet::new();
    for (case, expected) in actual.iter().zip(wanted) {
        let run: Uuid = serde_json::from_value(case["run_id"].clone()).map_err(invalid)?;
        let thread = case["thread_id"]
            .as_str()
            .ok_or_else(|| invalid("bootstrap thread missing"))?;
        if case.as_object().is_none_or(|value| value.len() != 8)
            || case["case"] != *expected
            || run.is_nil()
            || !runs.insert(run)
            || !thread.starts_with("bootstrap-reply-")
            || !threads.insert(thread)
            || case["worker_root"].is_null()
            || !roots.insert(case["worker_root"].to_string())
        {
            return Err(invalid("bootstrap cases are duplicate or incomplete"));
        }
        let row=tx.query_one("SELECT s.selection_checksum,i.arguments_bytes,i.answer_bytes,i.phase,i.attempt_count,
            a.phase,a.outcome,a.scope,i.selection_bytes,i.selection_checksum,s.selection_bytes
            FROM brassclaw_monty_tool_invocations i JOIN brassclaw_monty_task_admissions a USING(run_id)
            JOIN brassclaw_monty_recipe_selections s ON s.run_id=i.run_id AND s.recipe_id=i.recipe_id
            WHERE i.run_id=$1 AND i.recipe_id=$2 AND i.step_id='0:2'",&[&run,&subject.inspected.program().inputs().instruction().recipe().uuid]).await.map_err(invalid)?;
        let arguments: Value = strict_json(row.get(1), REVISION_LIMITS).map_err(invalid)?;
        let answer_bytes = row
            .get::<_, Option<String>>(2)
            .ok_or_else(|| invalid("bootstrap invocation answer is unresolved"))?;
        let answer: brassclaw_monty_host::process::PortAnswer =
            serde_json::from_str(&answer_bytes).map_err(invalid)?;
        let expected_answer = if expected["outcome"] == "reply" {
            if case["reply_ref"].as_str().is_none() || !case["failure"].is_null() {
                return Err(invalid("bootstrap positive reply missing"));
            }
            brassclaw_monty_host::process::PortAnswer::Return {
                value: case["reply_ref"].clone(),
            }
        } else {
            if !case["reply_ref"].is_null()
                || case["failure"] != RetainedStepFailure::Process.reason_kind()
            {
                return Err(invalid(
                    "bootstrap negative effect or classification differs",
                ));
            }
            brassclaw_monty_host::process::PortAnswer::TerminalError {
                reason_kind: expected["outcome"]
                    .as_str()
                    .ok_or_else(|| invalid("bootstrap outcome missing"))?
                    .into(),
            }
        };
        let outcome: Option<Value> = row.get(6);
        let scope: Value = row.get(7);
        if row.get::<_, String>(0) != hex(subject.selection)
            || row.get::<_, &str>(8) != invocation
            || row.get::<_, &str>(9) != digest(&invocation)
            || row.get::<_, &str>(10) != selection.exact_bytes()
            || arguments != json!({"answer":expected["answer"]})
            || case["arguments_checksum"] != hex(typed_value_checksum(&arguments).map_err(invalid)?)
            || case["answer_checksum"] != hex(port_answer_checksum(&answer).map_err(invalid)?)
            || port_answer_checksum(&answer).map_err(invalid)?
                != port_answer_checksum(&expected_answer).map_err(invalid)?
            || row.get::<_, &str>(3) != "answered"
            || row.get::<_, i16>(4) != 1
            || row.get::<_, &str>(5) != "settled"
            || scope["tenant_id"] != "bootstrap-validation"
            || scope["agent_id"] != "bootstrap-reply"
            || scope["project_id"] != "contained"
            || scope["thread_id"] != thread
            || outcome
                != Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
        {
            return Err(invalid("bootstrap durable execution evidence differs"));
        }
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&run],
            )
            .await
            .map_err(invalid)?
            .get(0);
        if count != 1 {
            return Err(invalid("bootstrap attempted extra effects"));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) async fn assert_bootstrap_readback(
    pool: Arc<PgPool>,
    candidate: Arc<InspectedRetainedProgram>,
) {
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    let subject = Arc::new(Subject::prepare(candidate.clone(), &worker).await.unwrap());
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let actual = read(&tx, &subject).await.unwrap().unwrap();
    assert_eq!(actual.approval_id(), subject.approval);
    assert_eq!(actual.selection_checksum(), subject.selection);
    assert_eq!(actual.approval.mode(), ValidationMode::SystemSeed);
    assert!(actual.approval.q2_reference().is_none());
    let count_before: i64 = tx
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    // An internally altered provenance subject cannot accept an existing record.
    let mut wrong = Subject::prepare(candidate.clone(), &worker).await.unwrap();
    wrong.header["producer"] = json!("untrusted-authored-label");
    assert!(read(&tx, &Arc::new(wrong)).await.is_err());
    // An unavailable approval with an actual retained Q1 intent is incomplete,
    // even when successful behavior exists. The producer cannot replay effects.
    let mut incomplete = Subject::prepare(candidate.clone(), &worker).await.unwrap();
    incomplete.approval = Uuid::new_v4();
    assert!(
        reuse_or_require_unstarted(&tx, &Arc::new(incomplete))
            .await
            .is_err()
    );
    // Successful records are immutable through the actual database trigger.
    tx.batch_execute("SAVEPOINT immutable_bootstrap")
        .await
        .unwrap();
    assert!(tx.execute("UPDATE reborn_component_review_evidence SET evidence_bytes=evidence_bytes WHERE evidence_id=$1",&[&subject.behavior]).await.is_err());
    tx.batch_execute("ROLLBACK TO SAVEPOINT immutable_bootstrap")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(client);
    // Reuse under a current global block is approval retention, never dispatch.
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let reused = read(&tx, &subject).await.unwrap().unwrap();
    assert_eq!(reused.approval_id(), actual.approval_id());
    let count_after: i64 = tx
        .query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        count_before, count_after,
        "restart/readback must not replay fixtures"
    );
    tx.commit().await.unwrap();
}

#[cfg(test)]
pub(crate) async fn assert_rejects_unapproved_recipe_successor(
    pool: Arc<PgPool>,
    candidate: Arc<InspectedRetainedProgram>,
    successor: brassclaw_skills::association_contract::ComponentRevisionRef,
) {
    use brassclaw_engine::memory::{
        retained_instruction::compile_retained_recipe,
        retained_tools::prepare_retained_tool_program,
    };
    let worker = brassclaw_monty_host::process::installed_worker().unwrap();
    let subject = Arc::new(Subject::prepare(candidate, &worker).await.unwrap());
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    let approved = read(&tx, &subject).await.unwrap().unwrap();
    let instruction = subject.inspected.program().inputs().instruction();
    let refs: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|r| {
            if r.reference().uuid == successor.uuid {
                successor
            } else {
                r.reference()
            }
        })
        .collect();
    let snapshot =
        PgComponentRevisionStore::read_exact_in_transaction(&tx, &[successor.uuid], &refs)
            .await
            .unwrap();
    let compiled = compile_retained_recipe(
        Arc::new(snapshot),
        successor.uuid,
        "selected",
        instruction.class(),
    )
    .unwrap();
    let program = prepare_retained_tool_program(compiled).unwrap();
    assert!(
        approved.pin_program(program).is_err(),
        "an unchanged usage combination cannot approve a changed Recipe subject"
    );
    tx.commit().await.unwrap();
}
