//! Closed behavioral observations for the packaged source compiler. Python owns
//! every workflow step. Rust checks fixed fixture facts and durable effects.
//! Development callers cannot turn these observations into installed approval.
use crate::{
    RebornBuildError,
    bootstrap_compiler_fixture::CompilerFixture,
    bootstrap_recipe_flow::{self, Request},
    bootstrap_reply_validation::invalid,
    bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, RecipeCapacity, SelectedMontyRecipe},
    monty_kernel::MontyKernelSnapshot,
};
use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, RetainedToolPort, RetainedTransportEvidence,
        port_answer_checksum, typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::{
    VmFailure,
    global::{GlobalBounds, Lifecycle},
    heap::HeapSettings,
    process::{PortAnswer, ProcessFailure, RecipeCommand, RootBoot, TaskSettings, WorkerCommand},
    service::{PortFailure, TaskOutcome},
    transport_actor::ActorLimits,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio_postgres::Transaction;

const CASES: &[(&str, &str, &str)] = &[
    ("base-literal", "base-prompt", "literal-reference/v1"),
    (
        "base-deduplicated",
        "base-prompt",
        "literal-reference/v2-deduplicated",
    ),
    ("maker-literal", "prefix-maker", "literal-reference/v1"),
    (
        "maker-deduplicated",
        "prefix-maker",
        "literal-reference/v2-deduplicated",
    ),
    ("duplicate", "base-prompt", "literal-reference/v1"),
    ("missing-guide", "base-prompt", "literal-reference/v1"),
    ("denied", "base-prompt", "literal-reference/v1"),
    ("invalid-revision", "base-prompt", "literal-reference/v1"),
];

fn check(ok: bool, message: &str) -> Result<(), RebornBuildError> {
    if ok { Ok(()) } else { Err(invalid(message)) }
}
struct Invocation {
    reference: String,
    inputs: Value,
    fixture: Arc<CompilerFixture>,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    pool: Arc<PgPool>,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "packaged-source-compiler-qualification"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.inputs.clone()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let invalid =
            || PortFailure::new("compiler_receipt_invalid").expect("constant classification");
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.host_answers().len() != 5
            || execution.transport_failure().is_some()
        {
            return Err(invalid());
        }
        let Some((_, PortAnswer::Return { value })) = execution.host_answers().last() else {
            return Err(invalid());
        };
        let profile = self.fixture.snapshot()["profile"]
            .as_str()
            .ok_or_else(invalid)?;
        let generation = value["generation"].as_str().ok_or_else(invalid)?;
        if value["profile"] != profile
            || value["qualification"] != "quarantined"
            || receipt != format!("prefix-generation:{profile}:{generation}")
        {
            return Err(invalid());
        }
        let client = self.pool.get().await.map_err(|_| invalid())?;
        let row = client.query_opt("SELECT generation_bytes,profile_revision FROM reborn_prefix_generations WHERE profile_name=$1 AND generation_id=$2", &[&profile,&generation]).await.map_err(|_| invalid())?.ok_or_else(invalid)?;
        let bytes: String = row.get(0);
        if hex::encode(Sha256::digest(bytes.as_bytes())) != generation
            || row.get::<_, i64>(1).to_string()
                != self.inputs["revision"].as_str().ok_or_else(invalid)?
        {
            return Err(invalid());
        }
        self.fixture.verify(&bytes).map_err(|_| invalid())
    }
}

pub(super) async fn execute(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    inspected: Arc<InspectedRetainedProgram>,
    worker: &Path,
    scope: &str,
) -> Result<Value, RebornBuildError> {
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("compiler requires Tool program"));
    };
    let root = crate::global_root_seed::retain_packaged_global_root(
        pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .map_err(invalid)?;
    let limits = inspected.preparation_limits();
    let mut process = limits.process;
    let frame = 8 * 1024 * 1024;
    check(
        frame <= process.max_frame_bytes
            && limits.bounds.max_source_bytes <= frame
            && limits.bounds.max_stdout_bytes <= frame,
        "closed compiler fixtures exceed configured frame",
    )?;
    process.max_frame_bytes = frame;
    let adapter = brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize;
    let heap = process
        .hard_memory_bytes
        .checked_sub(2 * frame + adapter)
        .filter(|heap| *heap > frame)
        .ok_or_else(|| invalid("compiler fixture containment exceeds configured memory"))?;
    let mut bounds = limits.bounds;
    bounds.max_value_bytes = frame;
    let client = pool.get().await.map_err(invalid)?;
    let serving: Value = client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb) FROM reborn_basic_prompt_store s",&[]).await.map_err(invalid)?.get(0);
    drop(client);
    let mut reports = Vec::new();
    let mut original = None;
    for &(name, profile, view) in CASES {
        let fixture = Arc::new(CompilerFixture::scoped(profile, view, scope)?);
        let client = pool.get().await.map_err(invalid)?;
        let revision: i64 = client
            .query_one(
                "SELECT revision FROM reborn_prefix_profiles WHERE name=$1",
                &[&profile],
            )
            .await
            .map_err(invalid)?
            .get(0);
        drop(client);
        let mut snapshot = fixture.snapshot().clone();
        if name == "missing-guide" {
            snapshot["sources"]
                .as_array_mut()
                .ok_or_else(|| invalid("fixture sources missing"))?
                .remove(0);
        }
        let inputs = json!({"profile":profile,"revision":if name == "invalid-revision" {"01".to_owned()} else {revision.to_string()},"snapshot_bytes":crate::prefix_generations::canonical(&snapshot).map_err(invalid)?});
        let reference = format!("closed-prefix-compiler:{name}:{}", uuid::Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            pool.clone(),
            "bootstrap-source-compiler",
            &reference,
        ))
        .await?;
        let tools = crate::installed_monty_catalogue::bootstrap_compiler_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            name == "denied",
        )?;
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            Request {
                worker: worker.to_owned(),
                process,
                actor: ActorLimits {
                    max_unclaimed: 8,
                    max_reserved_frame_bytes: 2 * frame,
                    max_control_unclaimed: 8,
                    max_control_reserved_frame_bytes: 2 * frame,
                },
                capacity: RecipeCapacity {
                    revision: 1,
                    max_recipes: 8,
                    data_limits: brassclaw_skills::value_contract::ContractLimits {
                        max_bytes: frame,
                        ..EXECUTION_DATA_LIMITS
                    },
                },
                boot: RootBoot {
                    source: root.source().to_owned(),
                    checksum: root.source_checksum(),
                    aliases: root.ports().iter().cloned().collect(),
                    bounds: GlobalBounds {
                        values: bounds,
                        workers: 1,
                        max_pending_calls: 8,
                    },
                    heap_settings: Some(HeapSettings {
                        revision: 1,
                        max_vm_bytes: heap,
                    }),
                    adapter_reserve_bytes: adapter,
                    startup_timeout: Duration::from_secs(10),
                    task_settings: TaskSettings {
                        revision: 1,
                        max_compute_time: Duration::from_secs(30),
                        token_budgets_enabled: false,
                    },
                    max_recipe_contexts: 8,
                },
                source: Arc::new(Invocation {
                    reference,
                    inputs: inputs.clone(),
                    fixture: fixture.clone(),
                    inspected: inspected.clone(),
                    tools,
                    pool: pool.clone(),
                }),
            },
        )
        .await?;
        let success = !matches!(name, "missing-guide" | "denied" | "invalid-revision");
        check(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ) == success,
            "compiler fixture outcome differs",
        )?;
        check(
            observed.report["root_completed"] == success
                && observed.report["model_exchange_count"] == 0
                && observed.report["reply"].is_null()
                && observed.report["semantic_approval"] == false,
            "compiler fixture root evidence differs",
        )?;
        let (fingerprints, rejection) = observed.ports.observe_internal_execution(|execution| -> Result<_,RebornBuildError> {
            for observation in execution.observations().values() { check(observation.settled() && observation.observation_error().is_none(),"compiler observation incomplete")?; }
            let mut rejection = Value::Null;
            if success {
                check(execution.is_complete() && execution.host_answers().len() == 5 && execution.transport_failure().is_none(),"compiler positive execution incomplete")?;
            } else {
                let step = if name == "invalid-revision" {"0:1"} else if name == "missing-guide" {"0:2"} else {"0:4"};
                check(execution.failed_step_id() == Some(step),"compiler rejection step differs")?;
                let Some(RetainedTransportEvidence::Process {error,command,..}) = execution.transport_failure() else { return Err(invalid("compiler rejection lacks actual process evidence")); };
                check(error.kind == ProcessFailure::Vm(VmFailure::Python) && error.snapshot.as_ref().is_some_and(|snapshot| snapshot.lifecycle == Lifecycle::Ready && snapshot.root.as_ref().is_some_and(|root| root.vm_id() == observed.receipt.root.vm_id())),"compiler rejection has unrelated runtime failure")?;
                let command = command.as_ref().map_err(invalid)?;
                rejection = json!({"step":step,"root_vm":observed.receipt.root.vm_id(),"command":match command.as_ref() {
                    WorkerCommand::Recipe {command:RecipeCommand::Start {..}} => "start",
                    WorkerCommand::Recipe {command:RecipeCommand::ResumeControl {..}} => "resume-control",
                    WorkerCommand::Recipe {command:RecipeCommand::ResumeHost {..}} => "resume-host",
                    _ => "unexpected",
                }});
                if name == "denied" {
                    check(matches!(execution.host_answers(), [(_,PortAnswer::TerminalError {reason_kind})] if reason_kind == "retained_tool_authorization") && matches!(command.as_ref(),WorkerCommand::Recipe {command:RecipeCommand::ResumeHost {..}}),"compiler policy rejection evidence differs")?;
                } else {
                    check(execution.host_answers().is_empty() && matches!(command.as_ref(),WorkerCommand::Recipe {command:RecipeCommand::Start {..}|RecipeCommand::ResumeControl {..}}),"compiler pure rejection evidence differs")?;
                }
            }
            Ok((execution.observations().iter().map(|(step,entry)| (step.clone(),(entry.arguments_checksum(),entry.answer_checksum()))).collect::<BTreeMap<_,_>>(), rejection))
        }).await.map_err(|error| invalid(format!("compiler observation: {error:?}")))??;
        check(
            admitted.host.finalized_reply_ref().is_none()
                && observed.exit.failure.is_none()
                && observed.exit.pending_admission.is_none()
                && observed.exit.queued_admissions.is_empty(),
            "compiler worker settlement incomplete",
        )?;
        let exit = observed
            .exit
            .transport
            .map_err(|error| invalid(format!("compiler containment: {error:?}")))?;
        check(
            exit.exit_status.is_some()
                && exit.containment_error.is_none()
                && exit.reap_error.is_none()
                && exit.shutdown_failure.is_none(),
            "compiler worker containment unresolved",
        )?;
        bootstrap_validation::settle_closed(&admitted).await?;
        let client = pool.get().await.map_err(invalid)?;
        let rows = client.query("SELECT step_id,recipe_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1",&[&admitted.attempt.run_id.as_uuid()]).await.map_err(invalid)?;
        let calls = if success {
            5
        } else if name == "denied" {
            1
        } else {
            0
        };
        check(
            rows.len() == calls,
            "compiler run has missing/additional invocations",
        )?;
        let mut invocations = BTreeMap::new();
        for row in rows {
            let step: String = row.get(0);
            let wanted =
                crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, &step)
                    .map_err(invalid)?;
            let arguments: Value =
                serde_json::from_str(&row.get::<_, String>(4)).map_err(invalid)?;
            let answer: PortAnswer = serde_json::from_str(
                &row.get::<_, Option<String>>(5)
                    .ok_or_else(|| invalid("compiler answer unresolved"))?,
            )
            .map_err(invalid)?;
            let args_hash = typed_value_checksum(&arguments).map_err(invalid)?;
            let answer_hash = port_answer_checksum(&answer).map_err(invalid)?;
            check(
                row.get::<_, uuid::Uuid>(1) == program.inputs().instruction().recipe().uuid
                    && row.get::<_, String>(2) == wanted
                    && row.get::<_, String>(3) == hex::encode(Sha256::digest(wanted.as_bytes()))
                    && row.get::<_, String>(6) == "answered"
                    && row.get::<_, i16>(7) == 1
                    && fingerprints.get(&step) == Some(&(Some(args_hash), Some(answer_hash))),
                "compiler durable invocation differs from observation",
            )?;
            invocations.insert(step,json!({"arguments":hex::encode(args_hash),"answer":hex::encode(answer_hash),"selection":row.get::<_,String>(3)}));
        }
        let after: Value = client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb) FROM reborn_basic_prompt_store s",&[]).await.map_err(invalid)?.get(0);
        check(
            after == serving,
            "compiler qualification changed serving state",
        )?;
        if name == "base-literal" {
            original = Some(observed.report["internal_receipt"].clone());
        }
        if name == "duplicate" {
            check(
                original.as_ref() == Some(&observed.report["internal_receipt"]),
                "compiler duplicate changed immutable output",
            )?;
        }
        reports.push(json!({"case":name,"profile":profile,"view":view,"run_id":admitted.attempt.run_id.as_uuid(),"revision":inputs["revision"],"thread_id":admitted.host.run_context().thread_id,"rejection":rejection,"inputs_checksum":hex::encode(typed_value_checksum(&inputs).map_err(invalid)?),"execution":observed.report,"invocations":invocations}));
    }
    Ok(
        json!({"format":"source-compiler-behavior-observations/2","qualification_scope":scope,"cases":reports,"catalogue_activation":false,"model_quality":"unmeasured"}),
    )
}

/// Reread the closed producer's complete run evidence. This performs no Tool
/// invocation, chooses no workflow and never accepts a caller's success flag.
pub(super) async fn verify(
    tx: &Transaction<'_>,
    inspected: &InspectedRetainedProgram,
    observed: &Value,
    scope: &str,
) -> Result<(), RebornBuildError> {
    crate::bootstrap_evidence::require_coherent(tx).await?;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("compiler program missing"));
    };
    check(
        observed.as_object().is_some_and(|value| value.len() == 5)
            && observed["qualification_scope"] == scope
            && observed["format"] == "source-compiler-behavior-observations/2"
            && observed["catalogue_activation"] == false
            && observed["model_quality"] == "unmeasured",
        "compiler observation format differs",
    )?;
    let records = observed["cases"]
        .as_array()
        .filter(|records| records.len() == CASES.len())
        .ok_or_else(|| invalid("compiler case coverage differs"))?;
    let selected = program
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let mut runs = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut threads = BTreeSet::new();
    let mut first_receipt = None;
    for (record, &(name, profile, view)) in records.iter().zip(CASES) {
        let success = !matches!(name, "missing-guide" | "denied" | "invalid-revision");
        check(
            record.as_object().is_some_and(|value| value.len() == 10)
                && record["case"] == name
                && record["profile"] == profile
                && record["view"] == view,
            "compiler case identity differs",
        )?;
        let run: uuid::Uuid = serde_json::from_value(record["run_id"].clone()).map_err(invalid)?;
        let root: uuid::Uuid = serde_json::from_value(record["execution"]["root"]["vm_id"].clone())
            .map_err(invalid)?;
        let thread = record["thread_id"]
            .as_str()
            .filter(|thread| thread.starts_with("bootstrap-source-compiler-"))
            .ok_or_else(|| invalid("compiler thread identity differs"))?;
        check(
            !run.is_nil()
                && runs.insert(run)
                && !root.is_nil()
                && roots.insert(root)
                && threads.insert(thread),
            "compiler case reused an execution identity",
        )?;
        let revision = record["revision"]
            .as_str()
            .ok_or_else(|| invalid("compiler revision missing"))?;
        check(
            if name == "invalid-revision" {
                revision == "01"
            } else {
                revision.parse::<u64>().is_ok_and(|value| {
                    value > 0 && value <= 9_007_199_254_740_991 && value.to_string() == revision
                })
            },
            "compiler case revision differs",
        )?;
        let fixture = CompilerFixture::scoped(profile, view, scope)?;
        let mut snapshot = fixture.snapshot().clone();
        if name == "missing-guide" {
            snapshot["sources"]
                .as_array_mut()
                .ok_or_else(|| invalid("fixture sources missing"))?
                .remove(0);
        }
        let inputs = json!({"profile":profile,"revision":revision,"snapshot_bytes":crate::prefix_generations::canonical(&snapshot).map_err(invalid)?});
        check(
            record["inputs_checksum"]
                == hex::encode(typed_value_checksum(&inputs).map_err(invalid)?),
            "compiler input evidence differs",
        )?;
        let report = &record["execution"];
        let failed_step = if success {
            None
        } else {
            Some(if name == "invalid-revision" {
                "0:1"
            } else if name == "missing-guide" {
                "0:2"
            } else {
                "0:4"
            })
        };
        let recipes = report["recipes"]
            .as_array()
            .filter(|recipes| recipes.len() == 1)
            .ok_or_else(|| invalid("compiler selected another workflow"))?;
        let recipe = &recipes[0];
        check(
            report["format"] == "monty-internal-task-execution/1"
                && report["root_completed"] == success
                && report["all_selected_recipes_complete"] == success
                && report["model_exchange_count"] == 0
                && report["reply"].is_null()
                && report["semantic_approval"] == false
                && report["catalogue_activation"] == false
                && report["intent_outcome"] == "internal_recipe"
                && report["withheld_root_answers"] == 0
                && report["withheld_host_answers"] == false
                && report["root"]["source_checksum"]
                    == hex::encode(Sha256::digest(
                        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED.as_bytes(),
                    ))
                && report["root"]["workers"] == 1
                && report["accounting"].is_object()
                && report["accounting"]["failure"].is_null()
                && !report["accounting"]["compute_time"].is_null()
                && recipe["recipe_id"] == json!(program.inputs().instruction().recipe().uuid)
                && recipe["selection_checksum"] == hex::encode(selected.checksum())
                && recipe["complete"] == success
                && recipe["failed_step"] == json!(failed_step),
            "compiler root/selection evidence differs",
        )?;
        let completed: BTreeSet<_> = recipe["completed_steps"]
            .as_array()
            .ok_or_else(|| invalid("compiler completed steps missing"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| invalid("compiler completed step invalid"))
            })
            .collect::<Result<_, _>>()?;
        let wanted: BTreeSet<_> = program
            .program()
            .steplist
            .iter()
            .take_while(|step| failed_step != Some(step.step_id.as_str()))
            .map(|step| step.step_id.as_str())
            .collect();
        check(completed == wanted, "compiler step completion differs")?;
        if !success {
            let rejection = &record["rejection"];
            check(
                rejection.as_object().is_some_and(|value| value.len() == 3)
                    && rejection["step"] == json!(failed_step)
                    && rejection["root_vm"] == json!(root)
                    && if name == "denied" {
                        rejection["command"] == "resume-host"
                    } else {
                        matches!(
                            rejection["command"].as_str(),
                            Some("start" | "resume-control")
                        )
                    },
                "compiler rejection provenance differs",
            )?;
        } else {
            check(
                record["rejection"].is_null(),
                "positive compiler case claims rejection",
            )?;
        }
        let admission = tx.query_one("SELECT s.selection_bytes,s.selection_checksum,a.phase,a.outcome,a.scope FROM brassclaw_monty_recipe_selections s JOIN brassclaw_monty_task_admissions a USING(run_id) WHERE s.run_id=$1 AND s.recipe_id=$2",&[&run,&program.inputs().instruction().recipe().uuid]).await.map_err(invalid)?;
        let scope: Value = admission.get(4);
        check(
            admission.get::<_, String>(0) == selected.exact_bytes()
                && admission.get::<_, String>(1) == hex::encode(selected.checksum())
                && admission.get::<_, String>(2) == "settled"
                && admission.get::<_, Option<Value>>(3)
                    == Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
                && scope["tenant_id"] == "bootstrap-validation"
                && scope["agent_id"] == "bootstrap-source-compiler"
                && scope["thread_id"] == thread,
            "compiler durable admission differs",
        )?;
        let rows = tx.query("SELECT step_id,recipe_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1",&[&run]).await.map_err(invalid)?;
        let expected_calls = if success {
            5
        } else if name == "denied" {
            1
        } else {
            0
        };
        check(
            rows.len() == expected_calls
                && record["invocations"]
                    .as_object()
                    .is_some_and(|calls| calls.len() == expected_calls),
            "compiler run-wide invocation evidence differs",
        )?;
        let mut receipt = None;
        for row in rows {
            let step: String = row.get(0);
            let binding = program
                .bindings()
                .get(&step)
                .ok_or_else(|| invalid("compiler dispatched unrelated step"))?;
            let selected =
                crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, &step)
                    .map_err(invalid)?;
            let arguments: Value = brassclaw_skills::value_contract::strict_json(
                &row.get::<_, String>(4),
                EXECUTION_DATA_LIMITS,
            )
            .map_err(invalid)?;
            let answer: PortAnswer = serde_json::from_str(
                &row.get::<_, Option<String>>(5)
                    .ok_or_else(|| invalid("compiler invocation remains unresolved"))?,
            )
            .map_err(invalid)?;
            let proof = &record["invocations"][&step];
            check(
                row.get::<_, uuid::Uuid>(1) == program.inputs().instruction().recipe().uuid
                    && row.get::<_, String>(2) == selected
                    && row.get::<_, String>(3) == hex::encode(Sha256::digest(selected.as_bytes()))
                    && row.get::<_, String>(6) == "answered"
                    && row.get::<_, i16>(7) == 1
                    && proof.as_object().is_some_and(|proof| proof.len() == 3)
                    && proof["selection"] == row.get::<_, String>(3)
                    && proof["arguments"]
                        == hex::encode(typed_value_checksum(&arguments).map_err(invalid)?)
                    && proof["answer"]
                        == hex::encode(port_answer_checksum(&answer).map_err(invalid)?),
                "compiler invocation integrity differs",
            )?;
            if name == "denied" {
                check(
                    step == "0:4"
                        && matches!(answer,PortAnswer::TerminalError {ref reason_kind} if reason_kind == "retained_tool_authorization"),
                    "compiler rejection has another Tool result",
                )?;
                continue;
            }
            let PortAnswer::Return { value } = answer else {
                return Err(invalid("positive compiler invocation did not return"));
            };
            if binding.capability_id() == brassclaw_host_runtime::JSON_CAPABILITY_ID {
                check(
                    arguments.as_object().is_some_and(|args| args.len() == 2)
                        && arguments["operation"] == "sha256_utf8",
                    "compiler hash operation differs",
                )?;
                let values: Vec<String> = serde_json::from_str(
                    arguments["data"]
                        .as_str()
                        .ok_or_else(|| invalid("compiler digest input is not JSON text"))?,
                )
                .map_err(invalid)?;
                let digests: Vec<_> = values
                    .iter()
                    .map(|text| hex::encode(Sha256::digest(text.as_bytes())))
                    .collect();
                check(
                    value == json!({"digests":digests}),
                    "compiler native digest differs from original UTF-8 bytes",
                )?;
            } else {
                check(
                    binding.capability_id()
                        == brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID
                        && arguments.as_object().is_some_and(|args| args.len() == 4)
                        && arguments["operation"] == "retain"
                        && arguments["profile"] == profile
                        && arguments["expected_profile_revision"]
                            == revision.parse::<u64>().map_err(invalid)?,
                    "compiler retention arguments differ",
                )?;
                let bytes = arguments["generation_bytes"]
                    .as_str()
                    .ok_or_else(|| invalid("compiler package missing"))?;
                fixture.verify(bytes)?;
                let generation = hex::encode(Sha256::digest(bytes.as_bytes()));
                let stored = tx.query_one("SELECT generation_bytes,profile_revision,generation_kind,created_at FROM reborn_prefix_generations WHERE profile_name=$1 AND generation_id=$2",&[&profile,&generation]).await.map_err(invalid)?;
                check(
                    stored.get::<_, String>(0) == bytes
                        && stored.get::<_, i64>(1).to_string() == revision
                        && stored.get::<_, String>(2) == "compiler-package"
                        && value
                            == json!({"profile":profile,"generation":generation,"profile_revision":revision.parse::<u64>().map_err(invalid)?,"kind":"compiler-package","qualification":"quarantined","created_at":stored.get::<_,chrono::DateTime<chrono::Utc>>(3).to_rfc3339()}),
                    "compiler durable original/receipt differs",
                )?;
                receipt = Some(format!("prefix-generation:{profile}:{generation}"));
            }
        }
        check(
            report["internal_receipt"] == json!(receipt),
            "compiler root receipt differs from native retention",
        )?;
        if name == "base-literal" {
            first_receipt = receipt.clone();
        }
        if name == "duplicate" {
            check(
                first_receipt == receipt,
                "duplicate compiler changed its generation",
            )?;
        }
    }
    Ok(())
}
