//! Closed installation qualification of the exact packaged generation usages.
//! Stored proof bytes are not trusted merely for declaring system_seed mode.
use crate::{
    RebornBuildError, bootstrap_evidence as evidence, bootstrap_reply_validation::invalid,
    global_monty_startup::ControlledExecutorConfiguration, installed_monty_catalogue::id,
    monty_kernel::MontyKernelSnapshot,
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

const PRODUCER: &str = "brassclaw-prefix-generation-bootstrap/2";
const PROFILE: &str = "configured-hold-executor/1";
#[derive(Clone)]
pub(super) struct BootstrapGenerationApproval {
    declaration: AssociationApprovalDeclaration,
    subject: Arc<Subject>,
    original: Arc<OriginalContext>,
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
        if actual.approval_id() != self.approval_id()
            || actual.original.value() != self.original.value()
        {
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
impl Fixture {
    fn value(&self) -> Value {
        json!({"format":"prefix-generation-fixture/1","profile":"base-prompt","package_bytes":self.package,
            "package_checksum":self.generation,"profile_revision":self.revision})
    }
    fn parse(value: &Value) -> Result<Arc<Self>, RebornBuildError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("generation original fixture object missing"))?;
        let package = value["package_bytes"]
            .as_str()
            .ok_or_else(|| invalid("generation original package bytes missing"))?;
        let revision = value["profile_revision"]
            .as_u64()
            .filter(|v| (1..=9_007_199_254_740_991).contains(v))
            .ok_or_else(|| invalid("generation original profile revision invalid"))?;
        let fixture = Arc::new(Self {
            package: package.to_owned(),
            generation: evidence::checksum(package),
            revision,
        });
        if object.len() != 5 || *value != fixture.value() {
            return Err(invalid("generation original fixture differs"));
        }
        Ok(fixture)
    }
}
struct OriginalContext {
    fixture: Arc<Fixture>,
    executor: Arc<ControlledExecutorConfiguration>,
}
impl OriginalContext {
    fn value(&self) -> Value {
        json!({"fixture":self.fixture.value(),"executor_bytes":self.executor.exact_bytes(),"executor_checksum":self.executor.checksum()})
    }
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
    worker: Arc<PackagedExecutableImage>,
}
impl Subject {
    async fn prepare(
        candidate: &InspectedRetainedProgram,
        usage: GenerationUsage,
        native: NativeImplementationRef,
        worker: Arc<PackagedExecutableImage>,
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
        let header = json!({"producer":PRODUCER,"behavior_profile":PROFILE,"behavior_format":crate::bootstrap_generation_validation::BEHAVIOR_FORMAT,"scope":format!("packaged-prefix-generation-{name}/1"),
            "selection_checksum":hex::encode(selection),"worker_checksum":hex::encode(worker.checksum()),"native":native,
            "fixture_contract":"prefix-generation-fixture/1","baseline_coverage":"documentation-only",
            "cases":crate::bootstrap_generation_validation::case_inventory(retain),"sources":sources,
            "validator_checksum":hex::encode(Sha256::digest(include_str!("bootstrap_generation_validation.rs").as_bytes())),
            "owner_checksum":hex::encode(Sha256::digest(include_str!("bootstrap_generation_approval.rs").as_bytes())),
            "root_checksum":evidence::checksum(crate::bootstrap_validation::CONFIGURED_HOLD_ROOT),
            "executor_owner_checksum":evidence::checksum(include_str!("global_monty_startup.rs")),
            "hold_owner_checksum":crate::bootstrap_validation::owner_checksum()});
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
            worker,
        }))
    }
    fn record(&self, identity: Uuid, kind: &str, report: Value) -> String {
        json!({"format":"component-review-evidence/1","evidence_id":identity,"kind":kind,"validation_mode":"system_seed",
            "association_checksum":evidence::checksum(&self.association),"components":self.components,"succeeded":true,"reviewed_evidence":[],"report":report}).to_string()
    }
    fn checks(&self) -> Value {
        json!([
            "exact-packaged-definitions-and-workflow",
            "native-registration-and-source-integrity",
            "typed-preload-and-binding",
            "truthful-original-guide-baseline"
        ])
    }
    fn q1_bytes(&self, original: &OriginalContext) -> String {
        self.record(self.q1, "q1", json!({"format":"prefix-generation-intent/2","header":self.header,
            "checks":self.checks(),"executor_bytes":original.executor.exact_bytes(),"executor_checksum":original.executor.checksum(),
            "fixture":original.fixture.value()}))
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
    executor: Arc<ControlledExecutorConfiguration>,
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
    let subjects = [
        Subject::prepare(
            &candidates[0],
            GenerationUsage::Read,
            native,
            worker.clone(),
        )
        .await?,
        Subject::prepare(
            &candidates[1],
            GenerationUsage::Retain,
            native,
            worker.clone(),
        )
        .await?,
    ];
    // First consult the stable functional subject. Completed and unresolved
    // originals never regenerate today's mutable fixture inputs.
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(invalid)?;
    if let Some(approved) = read_group(&tx, &subjects).await? {
        tx.commit().await.map_err(invalid)?;
        return Ok(approved);
    }
    let allowed = enabled(&tx, subjects[0].ids.tool).await?;
    tx.commit().await.map_err(invalid)?;
    drop(client);
    if !allowed {
        return Ok(Vec::new());
    }
    #[cfg(test)]
    if FORBID_FRESH_FIXTURE.try_with(|_| ()).is_ok() {
        return Err(invalid(
            "generation unexpectedly regenerated fixture inputs",
        ));
    }
    let package = crate::prefix_installation_baseline::assemble(
        &worker.path(),
        candidates[0].preparation_limits(),
        Duration::from_secs(30),
    )
    .await
    .map_err(invalid)?;
    let generation = evidence::checksum(&package);
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    // A concurrent winner may have committed while pure fixture preparation
    // ran. Only these exact IDs compete; unrelated subjects/writers are free.
    if let Some(approved) = read_group(&tx, &subjects).await? {
        tx.commit().await.map_err(invalid)?;
        return Ok(approved);
    }
    if !enabled(&tx, subjects[0].ids.tool).await? {
        tx.commit().await.map_err(invalid)?;
        return Ok(Vec::new());
    }
    let revision:i64 = tx.query_one("SELECT coalesce((SELECT profile_revision FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1), revision) FROM reborn_prefix_profiles WHERE name='base-prompt'", &[&generation]).await.map_err(invalid)?.get(0);
    let original = Arc::new(OriginalContext {
        fixture: Fixture::parse(
            &json!({"format":"prefix-generation-fixture/1","profile":"base-prompt","package_bytes":package,"package_checksum":generation,"profile_revision":revision}),
        )?,
        executor,
    });
    for subject in &subjects {
        let bytes = subject.q1_bytes(&original);
        // Bound the complete original carrier before INSERT or any effect.
        parse_original_context(subject, &bytes)?;
        if !evidence::retain_intent(&tx, subject.q1, &bytes).await? {
            return Err(invalid(
                "generation qualification intent already exists; no replay",
            ));
        }
    }
    tx.commit().await.map_err(invalid)?;
    drop(client);
    #[cfg(test)]
    if INTERRUPT_AFTER_INTENT.try_with(|_| ()).is_ok() {
        return Err(invalid(
            "generation test interrupted after original intents",
        ));
    }
    let mut observations = Vec::new();
    // Retain establishes the exact archived object required by Read qualification.
    for index in [1, 0] {
        let subject = &subjects[index];
        let observed = Box::pin(crate::bootstrap_generation_validation::execute(
            pool.clone(),
            kernel.clone(),
            subject.inspected.clone(),
            &subject.worker.path(),
            &original.fixture,
            subject.retain,
            original.executor.clone(),
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
    let mut approved = Vec::new();
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
    let original = read_original_context(tx, subject)
        .await?
        .ok_or_else(|| invalid("generation original intent missing"))?;
    if bytes != subject.approval_bytes() {
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
        brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
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
    verify_original_behavior(tx, subject, &original, &report["observed"]).await?;
    Ok(Some(BootstrapGenerationApproval {
        declaration,
        subject: subject.clone(),
        original,
    }))
}

async fn read_original_context(
    tx: &Transaction<'_>,
    subject: &Subject,
) -> Result<Option<Arc<OriginalContext>>, RebornBuildError> {
    evidence::read_evidence(tx, subject.q1)
        .await?
        .map(|bytes| parse_original_context(subject, &bytes))
        .transpose()
}
fn parse_original_context(
    subject: &Subject,
    bytes: &str,
) -> Result<Arc<OriginalContext>, RebornBuildError> {
    let record = strict_json(bytes, REVISION_LIMITS).map_err(invalid)?;
    let report = &record["report"];
    let original = Arc::new(OriginalContext {
        fixture: Fixture::parse(&report["fixture"])?,
        executor: ControlledExecutorConfiguration::restore_original(
            report["executor_bytes"]
                .as_str()
                .ok_or_else(|| invalid("generation original executor bytes missing"))?,
        )?,
    });
    if report["executor_checksum"] != original.executor.checksum()
        || bytes != subject.q1_bytes(&original)
    {
        return Err(invalid(
            "generation original fixture/executor intent differs",
        ));
    }
    Ok(original)
}
async fn verify_original_behavior(
    tx: &Transaction<'_>,
    subject: &Subject,
    original: &OriginalContext,
    observed: &Value,
) -> Result<(), RebornBuildError> {
    if observed["executor_bytes"] != original.executor.exact_bytes()
        || observed["executor_checksum"] != original.executor.checksum()
    {
        return Err(invalid(
            "generation behavior differs from original executor intent",
        ));
    }
    crate::bootstrap_generation_validation::verify(
        tx,
        &subject.inspected,
        &original.fixture,
        subject.retain,
        observed,
    )
    .await
}
async fn enabled(tx: &Transaction<'_>, tool: Uuid) -> Result<bool, RebornBuildError> {
    Ok(tx
        .query_one(
            "SELECT enabled FROM brassclaw_instance_tool_settings WHERE tool_id=$1",
            &[&tool],
        )
        .await
        .map_err(invalid)?
        .get(0))
}
/// Group consistency for this exact immutable functional subject only. Mutable
/// fixture/settings data never makes new IDs. Different real reviewed shipped
/// combinations/artifacts are independent subjects, not repairs of old work.
async fn read_group(
    tx: &Transaction<'_>,
    subjects: &[Arc<Subject>; 2],
) -> Result<Option<Vec<BootstrapGenerationApproval>>, RebornBuildError> {
    evidence::require_coherent(tx).await?;
    let mut approved = Vec::new();
    for subject in subjects {
        if let Some(value) = read(tx, subject).await? {
            approved.push(value);
        }
    }
    if approved.len() == 2 {
        if approved[0].original.value() != approved[1].original.value() {
            return Err(invalid(
                "generation original group fixture/executor differs",
            ));
        }
        return Ok(Some(approved));
    }
    if !approved.is_empty() {
        return Err(invalid(
            "generation qualification group is incomplete; reconcile without replay",
        ));
    }
    for subject in subjects {
        if evidence::read_evidence(tx, subject.q1).await?.is_some() {
            return Err(invalid(
                "generation qualification intent is incomplete; reconcile without replay",
            ));
        }
    }
    Ok(None)
}
/// Test-only readback of the production writer/reader after real profile and
/// settings edits. Mutated projections stay in memory; originals stay intact.
#[cfg(test)]
pub(crate) async fn assert_original_executor_custody(
    pool: Arc<PgPool>,
    approvals: &[BootstrapGenerationApproval],
    smaller: Arc<ControlledExecutorConfiguration>,
) {
    assert_eq!(approvals.len(), 2);
    // A genuine later fixture input: use the existing operator generation
    // import owner with different full package bytes and the edited profile.
    // Whitespace changes the outer archive checksum, not guide/provenance facts.
    let client = pool.get().await.unwrap();
    let revision: i64 = client
        .query_one(
            "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    drop(client);
    let later_package = format!("{}\n", approvals[0].original.fixture.package);
    crate::prefix_generations::validate_package("base-prompt", &later_package).unwrap();
    let later = crate::prefix_generations::PgPrefixGenerations::new(pool.clone())
        .import(
            "base-prompt",
            brassclaw_product_workflow::ImportPrefixGenerationRequest {
                expected_profile_revision: revision as u64,
                generation_bytes: later_package.clone(),
            },
        )
        .await
        .unwrap();
    assert_ne!(
        later.summary.generation,
        approvals[0].original.fixture.generation
    );
    assert_eq!(later.summary.profile_revision, revision as u64);
    assert_eq!(later.summary.qualification, "quarantined");
    assert_eq!(later.generation_bytes, later_package);
    let subjects = [approvals[0].subject.clone(), approvals[1].subject.clone()];
    let mut client = pool.get().await.unwrap();
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .start()
        .await
        .unwrap();
    assert!(read_group(&tx, &subjects).await.unwrap().is_some());
    for subject in &subjects {
        let original = read_original_context(&tx, subject).await.unwrap().unwrap();
        assert_ne!(original.executor.exact_bytes(), smaller.exact_bytes());
        let original_bytes = evidence::read_evidence(&tx, subject.q1)
            .await
            .unwrap()
            .unwrap();
        let original_json = strict_json(&original_bytes, REVISION_LIMITS).unwrap();
        let behavior = evidence::read_evidence(&tx, subject.behavior)
            .await
            .unwrap()
            .unwrap();
        let record = strict_json(&behavior, REVISION_LIMITS).unwrap();
        let observed = &record["report"]["observed"];
        verify_original_behavior(&tx, subject, &original, observed)
            .await
            .unwrap();
        assert_eq!(observed["executor_bytes"], original.executor.exact_bytes());
        assert_eq!(
            observed["format"],
            crate::bootstrap_generation_validation::BEHAVIOR_FORMAT
        );
        for case in observed["cases"].as_array().unwrap() {
            assert_eq!(case["creation"]["workers"], 2);
            assert_eq!(case["creation"]["waiting_slots"], json!([1]));
            assert_eq!(
                case["creation"]["bounds"]["max_source_bytes"],
                16 * 1024 * 1024
            );
            assert_eq!(
                case["creation"]["bounds"]["max_stdout_bytes"],
                16 * 1024 * 1024
            );
            assert_eq!(
                case["creation"]["frame"]["configured_frame_bytes"],
                64 * 1024 * 1024
            );
            assert_eq!(
                case["creation"]["task"]["revision"],
                original.executor.task().revision
            );
        }
        for field in ["executor_bytes", "executor_checksum", "format"] {
            let mut missing = original_json.clone();
            missing["report"].as_object_mut().unwrap().remove(field);
            assert!(parse_original_context(subject, &missing.to_string()).is_err());
            let mut missing = observed.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                verify_original_behavior(&tx, subject, &original, &missing)
                    .await
                    .is_err()
            );
        }
        for (field, value) in [
            ("format", json!("prefix-generation-behavior/1")),
            ("executor_checksum", json!("0".repeat(64))),
            ("executor_bytes", json!(smaller.exact_bytes())),
            ("unknown", json!(true)),
        ] {
            let mut changed = observed.clone();
            changed[field] = value;
            assert!(
                verify_original_behavior(&tx, subject, &original, &changed)
                    .await
                    .is_err()
            );
        }
        for (field, value) in [
            ("workers", json!(1)),
            ("waiting_slots", json!([])),
            ("frame", json!({})),
            ("heap", json!({})),
            ("task", json!({})),
            ("source_checksum", json!("0".repeat(64))),
            ("bounds", json!({})),
        ] {
            let mut changed = observed.clone();
            changed["cases"][0]["creation"][field] = value;
            assert!(
                verify_original_behavior(&tx, subject, &original, &changed)
                    .await
                    .is_err()
            );
        }
        let current_revision: i64 = tx
            .query_one(
                "SELECT revision FROM reborn_prefix_profiles WHERE name='base-prompt'",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        assert!(current_revision as u64 > original.fixture.revision);
        let mut different_fixture = original.fixture.value();
        different_fixture["profile_revision"] = json!(current_revision);
        let changed = OriginalContext {
            fixture: Fixture::parse(&different_fixture).unwrap(),
            executor: original.executor.clone(),
        };
        assert!(
            verify_original_behavior(&tx, subject, &changed, observed)
                .await
                .is_err(),
            "today's profile revision is not the original native fixture"
        );
        let mut bad_fixture = original.fixture.value();
        bad_fixture["package_checksum"] = json!("0".repeat(64));
        assert!(Fixture::parse(&bad_fixture).is_err());
        let mut bad_fixture = original.fixture.value();
        bad_fixture["profile_revision"] = json!(0);
        assert!(Fixture::parse(&bad_fixture).is_err());
        let mut bad_fixture = original.fixture.value();
        bad_fixture.as_object_mut().unwrap().remove("package_bytes");
        assert!(Fixture::parse(&bad_fixture).is_err());
        let mut replaced = observed.clone();
        replaced["executor_bytes"] = json!(smaller.exact_bytes());
        replaced["executor_checksum"] = json!(smaller.checksum());
        assert!(
            verify_original_behavior(&tx, subject, &original, &replaced)
                .await
                .is_err(),
            "a valid current executor is not the original intent"
        );
        let mut changed_intent = original_json.clone();
        changed_intent["report"]["executor_checksum"] = json!(smaller.checksum());
        assert!(parse_original_context(subject, &changed_intent.to_string()).is_err());
        assert_eq!(
            evidence::read_evidence(&tx, subject.q1)
                .await
                .unwrap()
                .unwrap(),
            original_bytes
        );
        assert_eq!(
            evidence::read_evidence(&tx, subject.behavior)
                .await
                .unwrap()
                .unwrap(),
            behavior
        );
        assert_eq!(
            read(&tx, subject).await.unwrap().unwrap().approval_id(),
            subject.approval
        );
    }
    tx.rollback().await.unwrap();
}

#[cfg(test)]
tokio::task_local! {
    static INTERRUPT_AFTER_INTENT: ();
    static FORBID_FRESH_FIXTURE: ();
}
#[cfg(test)]
pub(crate) async fn interrupt_after_original_intents<F: std::future::Future>(
    future: F,
) -> F::Output {
    INTERRUPT_AFTER_INTENT.scope((), future).await
}

#[cfg(test)]
pub(crate) async fn forbid_fresh_fixture<F: std::future::Future>(future: F) -> F::Output {
    FORBID_FRESH_FIXTURE.scope((), future).await
}
