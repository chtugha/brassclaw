//! Closed, exact first-party qualification of the model-free setup Recipe.
//! General pure approval and model/free workflows are not enabled by this owner.
use crate::{
    RebornBuildError, bootstrap_recipe_flow,
    bootstrap_reply_validation::invalid,
    bootstrap_validation, bootstrap_workflow_evidence as evidence,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    monty_kernel::MontyKernelSnapshot,
};
use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{
            RetainedProgram, RetainedRecipeExecution, RetainedToolPort,
            typed_value_checksum_with_limits,
        },
        retained_source::InspectedRetainedProgram,
    },
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_host_runtime::PackagedExecutableImage;
use brassclaw_monty_host::{
    service::{PortFailure, TaskOutcome},
    utility::UtilityLimits,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS,
    revision_store::PgComponentRevisionStore,
    value_contract::{ContractLimits, strict_json},
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use tokio_postgres::Transaction;
use uuid::Uuid;

pub(crate) struct Qualification {
    subject: Arc<Subject>,
}
/// Exact native qualification retained before a provider mutation starts. No
/// Deserialize/flag, setting, new catalogue view or authority from caller data.
pub(crate) struct Prepared {
    subject: Arc<Subject>,
    original_input_limits: ContractLimits,
}
impl Prepared {
    pub(crate) fn original_input_limits(&self) -> ContractLimits {
        self.original_input_limits
    }
    pub(crate) fn require_owner(&self, owner: &Qualification) -> Result<(), RebornBuildError> {
        if !Arc::ptr_eq(&self.subject, &owner.subject) {
            return Err(invalid(
                "bootstrap preparation belongs to another qualification",
            ));
        }
        Ok(())
    }
    pub(crate) fn program(&self) -> Arc<InspectedRetainedProgram> {
        self.subject.program.clone()
    }
}
struct Subject {
    program: Arc<InspectedRetainedProgram>,
    worker: Arc<PackagedExecutableImage>,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    header: Value,
    q1: Uuid,
    behavior: Uuid,
}
impl Qualification {
    /// Read the effective input limits and verify the qualification evidence in
    /// the same coherent RR snapshot. The caller must not pre-read the policy
    /// independently; this is the only authoritative capture path.
    pub(crate) async fn prepare(&self, pool: &PgPool) -> Result<Prepared, RebornBuildError> {
        let mut client = pool.get().await.map_err(invalid)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        self.verify_in_transaction(&tx).await?;
        // Deref to the inner tokio_postgres::Transaction required by the
        // generic GenericClient + Sync bound; deadpool wraps but does not
        // directly implement the tokio_postgres trait.
        let original_input_limits =
            crate::prefix_job_definitions::read_effective_limits_in_transaction(&*tx)
                .await
                .map_err(|_| invalid("effective input limits unavailable in qualification read"))?;
        tx.commit().await.map_err(invalid)?;
        Ok(Prepared {
            subject: self.subject.clone(),
            original_input_limits,
        })
    }
    pub(crate) fn program(&self) -> Arc<InspectedRetainedProgram> {
        self.subject.program.clone()
    }
    pub(crate) async fn verify_in_transaction(
        &self,
        tx: &Transaction<'_>,
    ) -> Result<(), RebornBuildError> {
        read(tx, &self.subject)
            .await?
            .ok_or_else(|| invalid("setup qualification missing"))?;
        Ok(())
    }
}
fn ids() -> (Uuid, Uuid) {
    (
        crate::installed_monty_catalogue::id("bootstrap-setup:recipe"),
        crate::installed_monty_catalogue::id("bootstrap-setup:code"),
    )
}
impl Subject {
    fn record(&self, id: Uuid, kind: &str, report: Value) -> String {
        json!({"format":"workflow-review-evidence/1","evidence_id":id,"kind":kind,"validation_mode":"system_seed","selection_checksum":self.header["selection_checksum"],"succeeded":true,"report":report}).to_string()
    }
    fn q1(&self) -> String {
        self.record(self.q1,"q1",json!({"header":self.header,"checks":["exact-packaged-two-component-closure","effect-free-preload-and-zero-host-bindings","declared-export-and-recursive-typed-contracts"]}))
    }
}
pub(crate) async fn qualify(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    worker: &Path,
    limits: UtilityLimits,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
) -> Result<Qualification, RebornBuildError> {
    let (recipe, code) = ids();
    let drafts =
        brassclaw_skills::bootstrap_setup_components::drafts(recipe, code).map_err(invalid)?;
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut refs = Vec::new();
    for draft in &drafts {
        refs.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }
    let snapshot = store.read_exact(&[recipe], &refs).await.map_err(invalid)?;
    if snapshot.revisions().len() != drafts.len()
        || drafts.iter().any(|draft| {
            snapshot
                .revisions()
                .get(&draft.uuid())
                .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
        })
    {
        return Err(invalid("setup packaged closure differs"));
    }
    let program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(snapshot),
            recipe,
            "selected",
            WorkflowClass::Deterministic,
        )
        .map_err(invalid)?,
    )
    .map_err(invalid)?;
    if !program.bindings().is_empty() {
        return Err(invalid("setup pure workflow has Tool bindings"));
    }
    let image = Arc::new(
        PackagedExecutableImage::capture(worker, limits.max_native_artifact_bytes)
            .await
            .map_err(invalid)?,
    );
    let program = Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(Arc::new(program)),
            &image.path(),
            limits,
        )
        .await
        .map_err(invalid)?,
    );
    program.pure_component(code).map_err(invalid)?;
    let selection = program
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let header = json!({"producer":"brassclaw-bootstrap-setup-qualification/1","selection_checksum":hex::encode(selection.checksum()),"worker_checksum":hex::encode(image.checksum()),"owner_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_setup_approval.rs")),"evidence_storage_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_workflow_evidence.rs")),"coherence_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_evidence.rs")),"result_consumer_checksum":crate::bootstrap_evidence::checksum(include_str!("prefix_bootstrap_setup.rs")),"packet_checksum":crate::bootstrap_evidence::checksum(include_str!("prefix_bootstrap_packet.rs")),"source_checksum":crate::bootstrap_evidence::checksum(brassclaw_skills::bootstrap_setup_components::SOURCE),"flow_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_recipe_flow.rs")),"root_checksum":crate::bootstrap_evidence::checksum(crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED)});
    let identity = Uuid::new_v5(&recipe, header.to_string().as_bytes());
    let subject = Arc::new(Subject {
        program,
        worker: image,
        executor,
        header,
        q1: Uuid::new_v5(&identity, b"q1"),
        behavior: Uuid::new_v5(&identity, b"behavior"),
    });
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    if read(&tx, &subject).await?.is_some() {
        tx.commit().await.map_err(invalid)?;
        return Ok(Qualification { subject });
    }
    if !evidence::retain_workflow(
        &tx,
        &subject.program,
        subject.q1,
        evidence::WorkflowKind::Q1,
        &subject.q1(),
    )
    .await?
    {
        return Err(invalid(
            "setup qualification intent incomplete; reconcile without replay",
        ));
    }
    tx.commit().await.map_err(invalid)?;
    drop(client);
    let report = execute(pool.clone(), kernel, subject.clone()).await?;
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .start()
        .await
        .map_err(invalid)?;
    verify(&tx, &subject, &report).await?;
    let record = subject.record(
        subject.behavior,
        "behavior",
        json!({"header":subject.header,"observed":report}),
    );
    evidence::retain_workflow(
        &tx,
        &subject.program,
        subject.behavior,
        evidence::WorkflowKind::Behavior,
        &record,
    )
    .await?;
    read(&tx, &subject)
        .await?
        .ok_or_else(|| invalid("setup qualification readback missing"))?;
    tx.commit().await.map_err(invalid)?;
    Ok(Qualification { subject })
}
const CASES: &[&str] = &[
    "initial",
    "manual-null-zero",
    "configured",
    "foreign-executor",
];
fn fixture(case: &str, limits: ContractLimits) -> Result<Value, RebornBuildError> {
    use brassclaw_product_workflow::{
        PrefixModelSetupDraft, PrefixModelSetupField as F, PrefixModelSetupOrigin as O,
        PrefixModelSetupValue as V, PrefixModelSpecification,
    };
    let mut original = json!({"format":"brassclaw-prefix-model-specification/1","provider_namespace":"bootstrap-validation","provider_id":"bootstrap-setup","provider_revision":1,"model":"fixture-model","protocol":"unknown","configured_context_window":null,"context_evidence":"provider-configuration-only","model_token_limits":null,"qualification":"configured-model-contract-only"});
    if case == "configured" {
        original["model_token_limits"] = json!({"max_prefix_tokens":0,"max_prompt_tokens":2048});
        original["model_token_limits_owner"] = json!("provider-definition-revision");
    }
    let bytes = crate::prefix_generations::canonical(&original).map_err(invalid)?;
    let id = crate::bootstrap_evidence::checksum(&bytes);
    let specification = PrefixModelSpecification {
        id: id.clone(),
        provider_id: "bootstrap-setup".into(),
        provider_revision: 1,
        model: "fixture-model".into(),
        specification_bytes: bytes,
        qualification: "configured-model-contract-only".into(),
    };
    let mut setup = PrefixModelSetupDraft {
        specification: id,
        revision: 0,
        operation_id: None,
        fields: F::ALL
            .into_iter()
            .map(|f| {
                (
                    f,
                    V {
                        value: None,
                        origin: O::Unset,
                    },
                )
            })
            .collect(),
    };
    if case == "manual-null-zero" {
        setup.revision = 1;
        setup.operation_id = Some(Uuid::new_v5(&ids().0, b"manual"));
        setup.fields.insert(
            F::MaxPrefixTokens,
            V {
                value: Some(0),
                origin: O::Manual,
            },
        );
        setup.fields.insert(
            F::MaxPromptTokens,
            V {
                value: None,
                origin: O::Manual,
            },
        );
    }
    if case == "configured" {
        setup.fields.insert(
            F::MaxPrefixTokens,
            V {
                value: Some(0),
                origin: O::ProviderConfiguration,
            },
        );
        setup.fields.insert(
            F::MaxPromptTokens,
            V {
                value: Some(2048),
                origin: O::ProviderConfiguration,
            },
        );
    }
    let input = crate::prefix_bootstrap_setup::inputs(
        Uuid::new_v5(&ids().0, b"job"),
        Uuid::new_v5(&ids().0, b"mutation"),
        &specification,
        &setup,
        limits,
    )?;
    if case != "foreign-executor" {
        return Ok(input);
    }
    let mut capture: Value = strict_json(
        input["capture_bytes"]
            .as_str()
            .ok_or_else(|| invalid("setup fixture missing"))?,
        limits,
    )
    .map_err(invalid)?;
    capture["packet"]["payload"]["executing_pair"]["model"] = json!("foreign-model");
    Ok(json!({"capture_bytes":crate::prefix_generations::canonical(&capture).map_err(invalid)?}))
}
struct Invocation {
    reference: String,
    input: Value,
    program: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "packaged-bootstrap-setup-qualification"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn requires_step_observations(&self) -> bool {
        true
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.program.clone(),
            inputs: Some(self.input.clone().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if !std::ptr::eq(selected, self.program.as_ref()) {
            return Err(PortFailure::new("bootstrap_setup_selection_invalid")
                .expect("constant classification"));
        }
        crate::prefix_bootstrap_setup::verify_execution(&self.input, receipt, execution)
            .map(|_| ())
            .map_err(|_| {
                PortFailure::new("bootstrap_setup_receipt_invalid")
                    .expect("constant classification")
            })
    }
}
async fn execute(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    subject: Arc<Subject>,
) -> Result<Value, RebornBuildError> {
    let root = crate::global_root_seed::retain_packaged_global_root(
        pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .map_err(invalid)?;
    let RetainedProgram::Tools(program) = subject.program.program() else {
        return Err(invalid("setup pure Tool program missing"));
    };
    let mut records = Vec::new();
    let mut captured_profile: Option<Value> = None;
    for case in CASES {
        let reference = format!("closed-bootstrap-setup:{case}:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            pool.clone(),
            "bootstrap-setup",
            &reference,
            None,
        ))
        .await?;
        let selected_bounds = subject.program.preparation_limits().bounds;
        let input = fixture(
            case,
            ContractLimits {
                max_depth: selected_bounds.max_value_depth,
                max_nodes: selected_bounds.max_value_nodes,
                max_bytes: selected_bounds.max_value_bytes,
            },
        )?;
        let tools = crate::installed_monty_catalogue::bootstrap_setup_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
        )?;
        let request = bootstrap_recipe_flow::Request::contained(
            subject.worker.path(),
            subject.executor.clone(),
            &root,
            Arc::new(Invocation {
                reference,
                input: input.clone(),
                program: subject.program.clone(),
                tools,
            }),
        )?;
        // The actual selected executor profile, subsequently acknowledged by
        // the VM, is the oracle boundary; never substitute a utility/default cap.
        let bounds = request.boot.bounds.values;
        let limits = ContractLimits {
            max_depth: bounds.max_value_depth,
            max_nodes: bounds.max_value_nodes,
            max_bytes: bounds.max_value_bytes,
        };
        let selected_limits = json!({"max_depth":limits.max_depth,"max_nodes":limits.max_nodes,"max_bytes":limits.max_bytes});
        if captured_profile
            .as_ref()
            .is_some_and(|previous| previous != &selected_limits)
        {
            return Err(invalid("setup qualification profile changed across cases"));
        }
        captured_profile = Some(selected_limits.clone());
        brassclaw_skills::value_contract::validate_data_bounds(&input, limits).map_err(invalid)?;
        let observed = bootstrap_recipe_flow::execute(&admitted, request).await?;
        let success = *case != "foreign-executor";
        if matches!(
            observed.receipt.outcome,
            TaskOutcome::InternalCompleted { .. }
        ) != success
            || observed.report["model_exchange_count"] != 0
            || !observed.report["reply"].is_null()
            || observed.report["semantic_approval"] != false
            || observed.exit.failure.is_some()
            || observed.exit.pending_admission.is_some()
        {
            return Err(invalid("setup contained outcome differs"));
        }
        let proof=observed.ports.observe_internal_execution(|execution|->Result<Value,RebornBuildError>{
            if !execution.host_answers().is_empty() || execution.observations().len()!=1 || execution.is_complete()!=success{return Err(invalid("setup native execution differs"));}
            if !success {
                use brassclaw_engine::executor::retained_recipe::RetainedTransportEvidence;
                use brassclaw_monty_host::{process::ProcessFailure,VmFailure,global::Lifecycle};
                let Some(RetainedTransportEvidence::Process{error,..})=execution.transport_failure() else{return Err(invalid("setup negative lacks process observation"));};
                if error.kind!=ProcessFailure::Vm(VmFailure::Python) || error.snapshot.as_ref().is_none_or(|snapshot|snapshot.lifecycle!=Lifecycle::Ready || snapshot.root.as_ref().is_none_or(|root|root.vm_id()!=observed.receipt.root.vm_id())) {return Err(invalid("setup negative escaped its contained Python step"));}
            }
            let p=execution.observations().get("0:1").ok_or_else(||invalid("setup step observation missing"))?;
            if p.data_limits().max_depth!=limits.max_depth || p.data_limits().max_nodes!=limits.max_nodes || p.data_limits().max_bytes!=limits.max_bytes {return Err(invalid("setup VM did not use its selected profile"));}
            if !p.settled() || p.observation_error().is_some() || p.arguments_checksum().is_some() || p.answer_checksum().is_some() || p.failure().is_some()==success{return Err(invalid("setup observation incomplete"));}
            Ok(json!({"inputs":hex::encode(p.input_checksum()),"result":p.result_checksum().map(hex::encode),"failure":p.failure().map(|f|f.reason_kind()),"limits":{"max_depth":p.data_limits().max_depth,"max_nodes":p.data_limits().max_nodes,"max_bytes":p.data_limits().max_bytes}}))
        }).await.map_err(|e|invalid(format!("setup observation: {e:?}")))??;
        if admitted.host.finalized_reply_ref().is_some() {
            return Err(invalid("setup fixture posted reply"));
        }
        bootstrap_validation::settle_closed(&admitted).await?;
        records.push(json!({"case":case,"run_id":admitted.attempt.run_id.as_uuid(),"proof":proof,"execution":observed.report}));
    }
    Ok(
        json!({"format":"bootstrap-setup-behavior/1","checks":["original-data-result-and-native-step-oracle","four-contained-recipe-outcomes-with-zero-host-calls","durable-original-selection-and-settlement"],"data_limits":captured_profile.ok_or_else(||invalid("setup captured profile missing"))?,"cases":records}),
    )
}
async fn verify(
    tx: &Transaction<'_>,
    subject: &Subject,
    report: &Value,
) -> Result<(), RebornBuildError> {
    crate::bootstrap_evidence::require_coherent(tx).await?;
    let records = report["cases"]
        .as_array()
        .filter(|v| v.len() == CASES.len())
        .ok_or_else(|| invalid("setup cases missing"))?;
    if report.as_object().is_none_or(|v| v.len() != 4)
        || report["format"] != "bootstrap-setup-behavior/1"
        || report["checks"]
            != json!([
                "original-data-result-and-native-step-oracle",
                "four-contained-recipe-outcomes-with-zero-host-calls",
                "durable-original-selection-and-settlement"
            ])
    {
        return Err(invalid("setup behavior envelope differs"));
    }
    let selection = subject
        .program
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let mut runs = std::collections::BTreeSet::new();
    for (record, case) in records.iter().zip(CASES) {
        let run: Uuid = serde_json::from_value(record["run_id"].clone()).map_err(invalid)?;
        if run.is_nil() || !runs.insert(run) || record["case"] != *case {
            return Err(invalid("setup behavior identity differs"));
        }
        let row=tx.query_one("SELECT s.selection_bytes,s.selection_checksum,a.phase,a.outcome,a.scope FROM brassclaw_monty_recipe_selections s JOIN brassclaw_monty_task_admissions a USING(run_id) WHERE s.run_id=$1 AND s.recipe_id=$2",&[&run,&ids().0]).await.map_err(invalid)?;
        let scope: Value = row.get(4);
        if row.get::<_, String>(0) != selection.exact_bytes()
            || row.get::<_, String>(1) != hex::encode(selection.checksum())
            || row.get::<_, String>(2) != "settled"
            || row.get::<_, Option<Value>>(3)
                != Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
            || scope["tenant_id"] != bootstrap_validation::NAMESPACE
            || scope["agent_id"] != "bootstrap-setup"
        {
            return Err(invalid("setup durable original differs"));
        }
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&run],
            )
            .await
            .map_err(invalid)?
            .get(0);
        if count != 0 {
            return Err(invalid("setup qualification dispatched a Tool"));
        }
        let proof = &record["proof"];
        let limits: ContractLimits = ContractLimits {
            max_depth: proof["limits"]["max_depth"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| invalid("setup bounds missing"))?,
            max_nodes: proof["limits"]["max_nodes"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| invalid("setup bounds missing"))?,
            max_bytes: proof["limits"]["max_bytes"]
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| invalid("setup bounds missing"))?,
        };
        if proof["limits"] != report["data_limits"]
            || proof["limits"].as_object().is_none_or(|v| v.len() != 3)
        {
            return Err(invalid(
                "setup cases have different original captured limits",
            ));
        }
        let input = fixture(case, limits)?;
        let success = *case != "foreign-executor";
        if proof["inputs"]
            != hex::encode(typed_value_checksum_with_limits(&input, limits).map_err(invalid)?)
            || record["execution"]["root_completed"] != success
            || record["execution"]["model_exchange_count"] != 0
            || !record["execution"]["reply"].is_null()
            || record["execution"]["semantic_approval"] != false
        {
            return Err(invalid("setup behavior input/outcome differs"));
        }
        if success {
            let receipt = record["execution"]["internal_receipt"]
                .as_str()
                .ok_or_else(|| invalid("setup receipt missing"))?;
            crate::prefix_bootstrap_setup::verify_result(&input, receipt, limits)?;
            if !proof["failure"].is_null()
                || proof["result"]
                    != hex::encode(
                        typed_value_checksum_with_limits(&json!(receipt), limits)
                            .map_err(invalid)?,
                    )
            {
                return Err(invalid("setup actual result proof differs"));
            }
        } else if !record["execution"]["internal_receipt"].is_null()
            || !proof["result"].is_null()
            || proof["failure"]
                != brassclaw_engine::executor::retained_recipe::RetainedStepFailure::Process
                    .reason_kind()
        {
            return Err(invalid("setup negative proof missing"));
        }
    }
    Ok(())
}
async fn read(
    tx: &Transaction<'_>,
    subject: &Arc<Subject>,
) -> Result<Option<()>, RebornBuildError> {
    let Some(bytes) = evidence::read_workflow(
        tx,
        &subject.program,
        subject.behavior,
        evidence::WorkflowKind::Behavior,
    )
    .await?
    else {
        return Ok(None);
    };
    if evidence::read_workflow(tx, &subject.program, subject.q1, evidence::WorkflowKind::Q1)
        .await?
        .as_deref()
        != Some(subject.q1().as_str())
    {
        return Err(invalid("setup Q1 differs"));
    }
    let instruction = subject.program.program().inputs().instruction();
    let refs: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect();
    PgComponentRevisionStore::read_exact_in_transaction(tx, &[ids().0], &refs)
        .await
        .map_err(invalid)?;
    let record = strict_json(&bytes, REVISION_LIMITS).map_err(invalid)?;
    let report = &record["report"];
    if report.as_object().is_none_or(|v| v.len() != 2)
        || report["header"] != subject.header
        || bytes != subject.record(subject.behavior, "behavior", report.clone())
    {
        return Err(invalid("setup behavior provenance differs"));
    }
    verify(tx, subject, &report["observed"]).await?;
    Ok(Some(()))
}
