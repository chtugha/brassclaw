//! Closed installation qualification of the exact packaged generation usages.
//! Stored proof bytes are not trusted merely for declaring system_seed mode.
use crate::{
    RebornBuildError, bootstrap_evidence as evidence, bootstrap_reply_validation::invalid,
    installed_monty_catalogue::id, monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::retained_tools::{RetainedToolBinding, RetainedToolProgram},
};
use brassclaw_host_api::CapabilityId;
use brassclaw_host_runtime::{NativeImplementationRef, PackagedExecutableImage};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::{AssociationApprovalDeclaration, ValidationMode},
    component_revision::REVISION_LIMITS,
    global_bootstrap_components::UsageComponentIds,
    prefix_generation_components::{
        GenerationUsage, generation_tool_draft, generation_usage_drafts,
    },
    revision_store::PgComponentRevisionStore,
    value_contract::strict_json,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

pub(super) struct BootstrapGenerationApproval {
    declaration: AssociationApprovalDeclaration,
    subject: Arc<Subject>,
}
impl BootstrapGenerationApproval {
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
            .ok_or_else(|| invalid("generation qualification is missing"))?;
        if actual.approval_id() != self.approval_id() {
            return Err(invalid("generation qualification differs"));
        }
        Ok(())
    }
    /// Only the exact canonical workflow receives its packaged pure-logic
    /// coverage. Reusing a usage elsewhere does not qualify that Recipe's code.
    pub(super) fn canonical_coverage(
        &self,
        program: &RetainedToolProgram,
    ) -> Result<Option<Uuid>, RebornBuildError> {
        let selected = program
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?;
        if selected.checksum() != self.subject.selection {
            return Ok(None);
        }
        Ok(self.subject.ids.formatter)
    }
    pub(super) fn require_selected_program(
        &self,
        inspected: &InspectedRetainedProgram,
    ) -> Result<(), RebornBuildError> {
        let source = self
            .subject
            .inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?;
        let mut expected: Value = serde_json::from_str(source.exact_bytes()).map_err(invalid)?;
        expected["association_approvals"] = json!({self.subject.step:{"approval_id":self.approval_id(),"checksum":evidence::checksum(self.declaration.exact_bytes())}});
        let expected_bytes = serde_json::to_string(&expected).map_err(invalid)?;
        if inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?
            .exact_bytes()
            != expected_bytes
        {
            return Err(invalid(
                "generation canonical workflow differs from its qualification",
            ));
        }
        Ok(())
    }
}

pub(super) struct Fixture {
    pub(super) package: String,
    pub(super) generation: String,
    pub(super) revision: u64,
}
struct Subject {
    inspected: Arc<InspectedRetainedProgram>,
    ids: UsageComponentIds,
    step: &'static str,
    retain: bool,
    selection: [u8; 32],
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
        usage: GenerationUsage,
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
            return Err(invalid("generation requires a retained Tool program"));
        };
        let retain = matches!(usage, GenerationUsage::Retain);
        let name = if retain { "retain" } else { "read" };
        let step = if retain { "0:3" } else { "0:2" };
        let binding = program
            .bindings()
            .get(step)
            .filter(|_| program.bindings().len() == 1)
            .ok_or_else(|| invalid("generation packaged binding differs"))?;
        let ids = UsageComponentIds {
            recipe: id(&format!("prefix-generation:{name}:recipe")),
            python_code: id(&format!("prefix-generation:{name}:code")),
            skill: id(&format!("prefix-generation:{name}:skill")),
            tool_skill: id(&format!("prefix-generation:{name}:binding")),
            tool: binding.tool().uuid,
            formatter: retain.then(|| id("prefix-generation:revision-formatter")),
        };
        let instruction = program.inputs().instruction();
        if instruction.recipe().uuid != ids.recipe
            || binding.capability_id() != brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID
        {
            return Err(invalid("generation packaged identity differs"));
        }
        let tool = generation_tool_draft(ids.tool, serde_json::to_value(native).map_err(invalid)?)
            .map_err(invalid)?;
        let drafts = generation_usage_drafts(ids, usage).map_err(invalid)?;
        let mut wanted = BTreeSet::new();
        for expected in std::iter::once(&tool).chain(drafts.iter()) {
            wanted.insert(expected.uuid());
            if instruction
                .snapshot()
                .revisions()
                .get(&expected.uuid())
                .is_none_or(|r| r.draft().exact_bytes() != expected.exact_bytes())
            {
                return Err(invalid(
                    "generation qualification admits only exact packaged definitions",
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
            return Err(invalid("generation packaged dependency graph differs"));
        }
        let selection = instruction
            .retained_selection()
            .map_err(invalid)?
            .checksum();
        let sources: BTreeMap<_, _> = inspected
            .source_checks()
            .iter()
            .map(|(uuid, source)| (uuid.to_string(), source.observations()))
            .collect();
        let header = json!({"producer":"brassclaw-prefix-generation-bootstrap/1","scope":format!("packaged-prefix-generation-{name}/1"),
            "selection_checksum":hex::encode(selection),"worker_checksum":hex::encode(worker.checksum()),"native":native,
            "baseline_checksum":fixture.generation,"profile_revision":fixture.revision,"baseline_coverage":"documentation-only",
            "cases":crate::bootstrap_generation_validation::case_inventory(retain),"sources":sources,
            "validator_checksum":hex::encode(Sha256::digest(include_str!("bootstrap_generation_validation.rs").as_bytes())),
            "owner_checksum":hex::encode(Sha256::digest(include_str!("bootstrap_generation_approval.rs").as_bytes())),
            "root_checksum":crate::bootstrap_reply_validation::root_checksum()});
        let identity = Uuid::new_v5(&ids.recipe, header.to_string().as_bytes());
        let components = json!(binding.combination().iter().map(|r| json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})).collect::<Vec<_>>());
        let association = binding.association().exact_bytes().to_owned();
        Ok(Arc::new(Self {
            inspected,
            ids,
            step,
            retain,
            selection,
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
    fn record(&self, identity: Uuid, kind: &str, report: Value) -> String {
        json!({"format":"component-review-evidence/1","evidence_id":identity,"kind":kind,"validation_mode":"system_seed",
            "association_checksum":evidence::checksum(&self.association),"components":self.components,"succeeded":true,"reviewed_evidence":[],"report":report}).to_string()
    }
    fn q1_bytes(&self) -> String {
        self.record(self.q1,"q1",json!({"header":self.header,"checks":["exact-packaged-definitions-and-workflow","native-registration-and-source-integrity","typed-preload-and-binding","truthful-original-guide-baseline"]}))
    }
    fn approval_bytes(&self) -> String {
        json!({"format":"skill-association-approval/1","approval_id":self.approval,"association_checksum":evidence::checksum(&self.association),"components":self.components,
            "validation_mode":"system_seed","q1_ref":self.q1,"q2_ref":null,"behavioral_refs":[self.behavior]}).to_string()
    }
}

pub(super) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    candidates: &[Arc<InspectedRetainedProgram>; 2],
    worker_path: &Path,
) -> Result<Vec<BootstrapGenerationApproval>, RebornBuildError> {
    let runtime = kernel
        .builtin(
            &CapabilityId::new(brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID)
                .map_err(invalid)?,
            None,
        )
        .map_err(invalid)?;
    runtime.verify_native_artifact().await.map_err(invalid)?;
    let native = runtime
        .native_identity()
        .ok_or_else(|| invalid("generation native registration missing"))?;
    let worker = Arc::new(
        PackagedExecutableImage::capture(
            worker_path,
            candidates[0].preparation_limits().max_native_artifact_bytes,
        )
        .await
        .map_err(invalid)?,
    );
    let package = crate::prefix_installation_baseline::assemble(
        &worker.path(),
        candidates[0].preparation_limits(),
        Duration::from_secs(30),
    )
    .await
    .map_err(invalid)?;
    let generation = evidence::checksum(&package);
    let client = pool.get().await.map_err(invalid)?;
    let revision:i64 = client.query_one("SELECT coalesce((SELECT profile_revision FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1), revision) FROM reborn_prefix_profiles WHERE name='base-prompt'", &[&generation]).await.map_err(invalid)?.get(0);
    drop(client);
    if revision <= 0 || revision > 9_007_199_254_740_991 {
        return Err(invalid(
            "baseline profile revision exceeds the qualified portable contract",
        ));
    }
    let fixture = Arc::new(Fixture {
        package,
        generation,
        revision: u64::try_from(revision).map_err(invalid)?,
    });
    let subjects = [
        Subject::prepare(
            &candidates[0],
            GenerationUsage::Read,
            native,
            worker.clone(),
            fixture.clone(),
        )
        .await?,
        Subject::prepare(
            &candidates[1],
            GenerationUsage::Retain,
            native,
            worker,
            fixture,
        )
        .await?,
    ];
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    let mut approved = Vec::new();
    for subject in &subjects {
        if let Some(value) = read(&tx, subject).await? {
            approved.push(value);
        }
    }
    if approved.len() == 2 {
        tx.commit().await.map_err(invalid)?;
        return Ok(approved);
    }
    if !approved.is_empty() {
        return Err(invalid(
            "generation qualification group is incomplete; reconcile without replay",
        ));
    }
    for subject in &subjects {
        if evidence::read_evidence(&tx, subject.q1).await?.is_some() {
            return Err(invalid(
                "generation qualification intent is incomplete; reconcile without replay",
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
        tx.commit().await.map_err(invalid)?;
        return Ok(Vec::new());
    }
    for subject in &subjects {
        if !evidence::retain_intent(&tx, subject.q1, &subject.q1_bytes()).await? {
            return Err(invalid(
                "generation qualification intent already exists; no replay",
            ));
        }
    }
    tx.commit().await.map_err(invalid)?;
    drop(client);
    let mut observations = Vec::new();
    // Retain establishes the exact archived object required by Read qualification.
    for index in [1, 0] {
        let subject = &subjects[index];
        let observed = Box::pin(crate::bootstrap_generation_validation::execute(
            pool.clone(),
            kernel.clone(),
            subject.inspected.clone(),
            &subject.worker.path(),
            &subject.fixture,
            subject.retain,
        ))
        .await?;
        observations.push((index, observed));
    }
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    for (index, observed) in observations {
        let subject = &subjects[index];
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
                .ok_or_else(|| invalid("generation qualification readback missing"))?,
        );
    }
    tx.commit().await.map_err(invalid)?;
    Ok(approved)
}

async fn read(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
) -> Result<Option<BootstrapGenerationApproval>, RebornBuildError> {
    evidence::require_coherent(tx).await?;
    let Some(bytes) = evidence::read_association(tx, subject.approval).await? else {
        return Ok(None);
    };
    if bytes != subject.approval_bytes()
        || evidence::read_evidence(tx, subject.q1).await?.as_deref()
            != Some(subject.q1_bytes().as_str())
    {
        return Err(invalid("generation qualification provenance differs"));
    }
    let declaration =
        AssociationApprovalDeclaration::from_json(&bytes, REVISION_LIMITS).map_err(invalid)?;
    let RetainedProgram::Tools(program) = subject.inspected.program() else {
        return Err(invalid("generation binding missing"));
    };
    declaration
        .require_selected_combination(
            program.bindings()[subject.step].association(),
            program.bindings()[subject.step].combination(),
        )
        .map_err(invalid)?;
    if declaration.mode() != ValidationMode::SystemSeed || declaration.q2_reference().is_some() {
        return Err(invalid("generation qualification mode differs"));
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
        .ok_or_else(|| invalid("generation behavior missing"))?;
    let record = strict_json(&behavior, REVISION_LIMITS).map_err(invalid)?;
    let report = &record["report"];
    if report.as_object().is_none_or(|r| r.len() != 2)
        || report["header"] != subject.header
        || behavior != subject.record(subject.behavior, "behavior", report.clone())
    {
        return Err(invalid("generation behavior producer differs"));
    }
    crate::bootstrap_generation_validation::verify(
        tx,
        &subject.inspected,
        &subject.fixture,
        subject.retain,
        &report["observed"],
    )
    .await?;
    Ok(Some(BootstrapGenerationApproval {
        declaration,
        subject: subject.clone(),
    }))
}
