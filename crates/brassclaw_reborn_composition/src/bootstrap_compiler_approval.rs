//! Installation-owned qualification of the exact packaged source compiler.
//! Authored labels/claims cannot construct this type. Approval is not activation,
//! a model-quality result or permission to dispatch either primitive.
use crate::{
    RebornBuildError, bootstrap_compiler_validation as validation, bootstrap_evidence as evidence,
    bootstrap_generation_approval::BootstrapGenerationApproval,
    bootstrap_reply_validation::invalid, monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::retained_tools::RetainedToolBinding,
};
use brassclaw_host_api::CapabilityId;
use brassclaw_host_runtime::PackagedExecutableImage;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::{AssociationApprovalDeclaration, ValidationMode},
    component_revision::{REVISION_LIMITS, RetainedComponentSnapshot},
    prefix_compiler_components::{SourceCompilerIds, source_compiler_drafts},
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

pub(super) struct BootstrapCompilerApproval {
    declaration: AssociationApprovalDeclaration,
    subject: Arc<Subject>,
}
impl BootstrapCompilerApproval {
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
            .ok_or_else(|| invalid("compiler qualification missing"))?;
        if actual.approval_id() != self.approval_id() {
            return Err(invalid("compiler qualification differs"));
        }
        Ok(())
    }
    /// The compiler returns an internal receipt, not a published chat reply.
    /// Prepare its exact private selection without advertising a chat route or
    /// qualifying arbitrary Recipes that reuse the hash Skill.
    pub(super) async fn prepare_private_program(
        &self,
    ) -> Result<Arc<InspectedRetainedProgram>, RebornBuildError> {
        let original = self.subject.inspected.program().inputs().instruction();
        let snapshot = Arc::new(
            original
                .snapshot()
                .select_roots(&[original.recipe().uuid])
                .map_err(invalid)?,
        );
        let compiled = brassclaw_engine::memory::retained_instruction::compile_retained_recipe(
            snapshot,
            original.recipe().uuid,
            "selected",
            brassclaw_engine::memory::retained_instruction::WorkflowClass::Deterministic,
        )
        .map_err(invalid)?;
        let program =
            brassclaw_engine::memory::retained_tools::prepare_retained_tool_program(compiled)
                .map_err(invalid)?;
        let mut approvals = BTreeMap::new();
        for (step, binding) in program.bindings() {
            let declaration =
                if binding.capability_id() == brassclaw_host_runtime::JSON_CAPABILITY_ID {
                    &self.declaration
                } else {
                    self.subject
                        .retain
                        .usage_approval(binding)
                        .ok_or_else(|| invalid("compiler storage qualification differs"))?
                };
            approvals.insert(step.clone(), declaration);
        }
        let program = program
            .with_association_approvals(&approvals)
            .map_err(invalid)?;
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(Arc::new(program)),
                &self.subject.worker.path(),
                self.subject.inspected.preparation_limits(),
            )
            .await
            .map_err(invalid)?,
        );
        self.require_selected_program(&inspected)?;
        Ok(inspected)
    }
    pub(super) fn require_selected_program(
        &self,
        inspected: &InspectedRetainedProgram,
    ) -> Result<(), RebornBuildError> {
        let RetainedProgram::Tools(original) = self.subject.inspected.program() else {
            return Err(invalid("compiler program missing"));
        };
        let mut expected: Value = serde_json::from_str(
            original
                .inputs()
                .instruction()
                .retained_selection()
                .map_err(invalid)?
                .exact_bytes(),
        )
        .map_err(invalid)?;
        let mut approvals = BTreeMap::new();
        for (step, binding) in original.bindings() {
            let declaration =
                if binding.capability_id() == brassclaw_host_runtime::JSON_CAPABILITY_ID {
                    &self.declaration
                } else {
                    self.subject
                        .retain
                        .usage_approval(binding)
                        .ok_or_else(|| invalid("compiler storage qualification differs"))?
                };
            approvals.insert(step,json!({"approval_id":declaration.approval_id(),"checksum":evidence::checksum(declaration.exact_bytes())}));
        }
        expected["association_approvals"] = json!(approvals);
        if inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .exact_bytes()
            != serde_json::to_string(&expected).map_err(invalid)?
        {
            return Err(invalid("qualified compiler selection differs"));
        }
        Ok(())
    }
}
struct Subject {
    inspected: Arc<InspectedRetainedProgram>,
    worker: Arc<PackagedExecutableImage>,
    retain: BootstrapGenerationApproval,
    header: Value,
    association: String,
    components: Value,
    q1: Uuid,
    behavior: Uuid,
    approval: Uuid,
    json_tool: Uuid,
    generation_tool: Uuid,
}
impl Subject {
    async fn prepare(
        pool: Arc<PgPool>,
        kernel: &dyn MontyKernelSnapshot,
        candidate: &InspectedRetainedProgram,
        generation: &[BootstrapGenerationApproval],
        worker: &Path,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let image = Arc::new(
            PackagedExecutableImage::capture(
                worker,
                candidate.preparation_limits().max_native_artifact_bytes,
            )
            .await
            .map_err(invalid)?,
        );
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                candidate.program().clone(),
                &image.path(),
                candidate.preparation_limits(),
            )
            .await
            .map_err(invalid)?,
        );
        let RetainedProgram::Tools(program) = inspected.program() else {
            return Err(invalid("compiler requires Tool program"));
        };
        let hash = program
            .bindings()
            .get("0:4")
            .ok_or_else(|| invalid("compiler hash binding missing"))?;
        let storage = program
            .bindings()
            .get("0:16")
            .ok_or_else(|| invalid("compiler storage binding missing"))?;
        let json_tool = hash.tool().uuid;
        let generation_tool = storage.tool().uuid;
        let ids = SourceCompilerIds::allocate(
            json_tool,
            generation_tool,
            crate::installed_monty_catalogue::id,
        );
        let mut native = Vec::new();
        for capability in [
            brassclaw_host_runtime::JSON_CAPABILITY_ID,
            brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID,
        ] {
            let runtime = kernel
                .builtin(&CapabilityId::new(capability).map_err(invalid)?, None)
                .map_err(invalid)?;
            runtime.verify_native_artifact().await.map_err(invalid)?;
            native.push(
                serde_json::to_value(
                    runtime
                        .native_identity()
                        .ok_or_else(|| invalid("compiler native registration missing"))?,
                )
                .map_err(invalid)?,
            );
        }
        let mut drafts = vec![
            brassclaw_skills::utf8_digest_components::json_text_tool_draft(
                json_tool,
                native[0].clone(),
            )
            .map_err(invalid)?,
            brassclaw_skills::prefix_generation_components::generation_tool_draft(
                generation_tool,
                native[1].clone(),
            )
            .map_err(invalid)?,
        ];
        drafts.extend(source_compiler_drafts(ids).map_err(invalid)?);
        let expected = RetainedComponentSnapshot::new(
            &[ids.recipe, ids.digest.recipe, ids.retain.recipe],
            drafts
                .into_iter()
                .map(|draft| draft.at_version(1))
                .collect::<Result<Vec<_>, _>>()
                .map_err(invalid)?,
        )
        .map_err(invalid)?
        .select_roots(&[ids.recipe])
        .map_err(invalid)?;
        let actual = program.inputs().instruction().snapshot();
        if program.inputs().instruction().recipe().uuid != ids.recipe
            || actual.revisions().keys().copied().collect::<BTreeSet<_>>()
                != expected
                    .revisions()
                    .keys()
                    .copied()
                    .collect::<BTreeSet<_>>()
            || expected.revisions().iter().any(|(id, wanted)| {
                actual.revisions().get(id).is_none_or(|actual| {
                    actual.draft().exact_bytes() != wanted.draft().exact_bytes()
                })
            })
        {
            return Err(invalid(
                "compiler qualification admits only exact packaged definitions",
            ));
        }
        if program.bindings().len() != 5
            || program.bindings().iter().any(|(step, binding)| {
                if step == "0:16" {
                    binding.capability_id()
                        != brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID
                } else {
                    binding.capability_id() != brassclaw_host_runtime::JSON_CAPABILITY_ID
                        || binding.association().exact_bytes() != hash.association().exact_bytes()
                        || binding.combination() != hash.combination()
                }
            })
        {
            return Err(invalid("compiler qualification binding graph differs"));
        }
        let retain = generation
            .iter()
            .find(|owner| owner.usage_approval(storage).is_some())
            .ok_or_else(|| invalid("compiler requires qualified canonical storage usage"))?
            .clone();
        let selection = program
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .checksum();
        let sources: BTreeMap<_, _> = inspected
            .source_checks()
            .iter()
            .map(|(id, check)| (id.to_string(), check.observations()))
            .collect();
        let header = json!({"producer":"brassclaw-source-compiler-bootstrap/1","selection_checksum":hex::encode(selection),"worker_checksum":hex::encode(image.checksum()),"native":native,"sources":sources,"retain_approval":retain.approval_id(),
            "owner_checksum":evidence::checksum(include_str!("bootstrap_compiler_approval.rs")),"validator_checksum":evidence::checksum(include_str!("bootstrap_compiler_validation.rs")),"fixture_checksum":evidence::checksum(include_str!("bootstrap_compiler_fixture.rs")),"observer_checksum":evidence::checksum(include_str!("bootstrap_recipe_flow.rs")),"adapter_checksum":evidence::checksum(include_str!("global_recipe_ports.rs")),"root_checksum":evidence::checksum(crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED),"scope":"source-compilation-only; model/cache/activation unqualified"});
        let identity = Uuid::new_v5(&ids.recipe, header.to_string().as_bytes());
        let association = hash.association().exact_bytes().to_owned();
        let components=json!(hash.combination().iter().map(|r| json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})).collect::<Vec<_>>());
        // Reread the exact retained graph; a syntactically valid draft alone is
        // not proof that immutable storage contains these packaged artifacts.
        let refs: Vec<_> = actual
            .revisions()
            .values()
            .map(|revision| revision.reference())
            .collect();
        PgComponentRevisionStore::new(pool)
            .read_exact(&[ids.recipe], &refs)
            .await
            .map_err(invalid)?;
        Ok(Arc::new(Self {
            inspected,
            worker: image,
            retain,
            header,
            association,
            components,
            q1: Uuid::new_v5(&identity, b"q1"),
            behavior: Uuid::new_v5(&identity, b"behavior"),
            approval: Uuid::new_v5(&identity, b"approval"),
            json_tool,
            generation_tool,
        }))
    }
    fn record(&self, id: Uuid, kind: &str, report: Value) -> String {
        json!({"format":"component-review-evidence/1","evidence_id":id,"kind":kind,"validation_mode":"system_seed","association_checksum":evidence::checksum(&self.association),"components":self.components,"succeeded":true,"reviewed_evidence":[],"report":report}).to_string()
    }
    fn q1_bytes(&self) -> String {
        self.record(self.q1,"q1",json!({"header":self.header,"checks":["exact-packaged-source-workflow-and-dependencies","registered-retained-native-primitives","typed-preload-and-one-Tool-usages","closed-independent-original-render-and-origin-oracles"]}))
    }
    fn approval_bytes(&self) -> String {
        json!({"format":"skill-association-approval/1","approval_id":self.approval,"association_checksum":evidence::checksum(&self.association),"components":self.components,"validation_mode":"system_seed","q1_ref":self.q1,"q2_ref":null,"behavioral_refs":[self.behavior]}).to_string()
    }
}

pub(super) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    candidate: &InspectedRetainedProgram,
    generation: &[BootstrapGenerationApproval],
    worker: &Path,
) -> Result<Option<BootstrapCompilerApproval>, RebornBuildError> {
    // An operator-blocked, previously unqualified storage primitive cannot
    // bootstrap a compiler; leave that capability unavailable without effects.
    let RetainedProgram::Tools(program) = candidate.program() else {
        return Err(invalid("compiler program missing"));
    };
    let storage = program
        .bindings()
        .get("0:16")
        .ok_or_else(|| invalid("compiler storage binding missing"))?;
    if !generation
        .iter()
        .any(|owner| owner.usage_approval(storage).is_some())
    {
        return Ok(None);
    }
    let subject =
        Subject::prepare(pool.clone(), kernel.as_ref(), candidate, generation, worker).await?;
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    match begin(&tx, &subject).await? {
        Begin::Reuse(approved) => {
            tx.commit().await.map_err(invalid)?;
            return Ok(Some(*approved));
        }
        Begin::Blocked => {
            tx.commit().await.map_err(invalid)?;
            return Ok(None);
        }
        Begin::Run => {}
    }
    tx.commit().await.map_err(invalid)?;
    drop(client);
    let observed = Box::pin(validation::execute(
        pool.clone(),
        kernel,
        subject.inspected.clone(),
        &subject.worker.path(),
        &subject.q1.to_string(),
    ))
    .await?;
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    validation::verify(&tx, &subject.inspected, &observed, &subject.q1.to_string()).await?;
    subject.retain.verify_in_transaction(&tx).await?;
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
    let approved = read(&tx, &subject)
        .await?
        .ok_or_else(|| invalid("compiler qualification readback missing"))?;
    tx.commit().await.map_err(invalid)?;
    Ok(Some(approved))
}
enum Begin {
    Reuse(Box<BootstrapCompilerApproval>),
    Blocked,
    Run,
}
async fn begin(tx: &Transaction<'_>, subject: &Arc<Subject>) -> Result<Begin, RebornBuildError> {
    evidence::require_coherent(tx).await?;
    if let Some(approved) = read(tx, subject).await? {
        return Ok(Begin::Reuse(Box::new(approved)));
    }
    if evidence::read_evidence(tx, subject.q1).await?.is_some() {
        return Err(invalid(
            "compiler qualification intent incomplete; reconcile without replay",
        ));
    }
    for tool in [subject.json_tool, subject.generation_tool] {
        let enabled: bool = tx
            .query_one(
                "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
                &[&tool],
            )
            .await
            .map_err(invalid)?
            .get(0);
        if !enabled {
            return Ok(Begin::Blocked);
        }
    }
    if !evidence::retain_intent(tx, subject.q1, &subject.q1_bytes()).await? {
        return Err(invalid(
            "compiler qualification intent already exists; no replay",
        ));
    }
    Ok(Begin::Run)
}

#[cfg(test)]
pub(super) async fn assert_incomplete_intent_is_not_replayed(
    pool: Arc<PgPool>,
    approved: &BootstrapCompilerApproval,
) {
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
    // Preserve immutable completed records. A distinct rollback-only subject
    // models a committed Q1 intent whose behavior/approval has not completed.
    let original = &approved.subject;
    let identity = Uuid::new_v5(
        &original.approval,
        b"rollback-only-incomplete-compiler-test/1",
    );
    let mut header = original.header.clone();
    header["test_scope"] = json!("rollback-only-incomplete-compiler-test/1");
    let pending = Arc::new(Subject {
        inspected: original.inspected.clone(),
        worker: original.worker.clone(),
        retain: original.retain.clone(),
        header,
        association: original.association.clone(),
        components: original.components.clone(),
        q1: Uuid::new_v5(&identity, b"q1"),
        behavior: Uuid::new_v5(&identity, b"behavior"),
        approval: Uuid::new_v5(&identity, b"approval"),
        json_tool: original.json_tool,
        generation_tool: original.generation_tool,
    });
    assert!(
        evidence::retain_intent(&tx, pending.q1, &pending.q1_bytes())
            .await
            .unwrap()
    );
    assert!(begin(&tx, &pending).await.is_err());
    assert_eq!(
        tx.query_one("SELECT count(*) FROM brassclaw_monty_tool_invocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        before
    );
    tx.rollback().await.unwrap();
}

async fn read(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
) -> Result<Option<BootstrapCompilerApproval>, RebornBuildError> {
    evidence::require_coherent(tx).await?;
    let Some(bytes) = evidence::read_association(tx, subject.approval).await? else {
        return Ok(None);
    };
    if bytes != subject.approval_bytes()
        || evidence::read_evidence(tx, subject.q1).await?.as_deref()
            != Some(subject.q1_bytes().as_str())
    {
        return Err(invalid("compiler bootstrap provenance differs"));
    }
    let declaration =
        AssociationApprovalDeclaration::from_json(&bytes, REVISION_LIMITS).map_err(invalid)?;
    let RetainedProgram::Tools(program) = subject.inspected.program() else {
        return Err(invalid("compiler program missing"));
    };
    let binding = &program.bindings()["0:4"];
    declaration
        .require_selected_combination(binding.association(), binding.combination())
        .map_err(invalid)?;
    if declaration.mode() != ValidationMode::SystemSeed || declaration.q2_reference().is_some() {
        return Err(invalid("compiler qualification mode differs"));
    }
    subject.retain.verify_in_transaction(tx).await?;
    let instruction = program.inputs().instruction();
    let refs: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|revision| revision.reference())
        .collect();
    PgComponentRevisionStore::read_exact_in_transaction(tx, &[instruction.recipe().uuid], &refs)
        .await
        .map_err(invalid)?;
    let behavior = evidence::read_evidence(tx, subject.behavior)
        .await?
        .ok_or_else(|| invalid("compiler behavior proof missing"))?;
    let record = strict_json(&behavior, REVISION_LIMITS).map_err(invalid)?;
    let report = &record["report"];
    if report.as_object().is_none_or(|report| report.len() != 2)
        || report["header"] != subject.header
        || behavior != subject.record(subject.behavior, "behavior", report.clone())
    {
        return Err(invalid("compiler behavior producer differs"));
    }
    validation::verify(
        tx,
        &subject.inspected,
        &report["observed"],
        &subject.q1.to_string(),
    )
    .await?;
    Ok(Some(BootstrapCompilerApproval {
        declaration,
        subject: subject.clone(),
    }))
}
