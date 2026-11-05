//! Finite independent workflow oracles over real Recipe execution and durable
//! native observations. Workflow sequencing remains in the packaged Recipe.
use super::fixture::{CASES, Fixture};
use crate::{
    RebornBuildError, bootstrap_recipe_flow,
    bootstrap_reply_validation::invalid,
    bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    monty_kernel::MontyKernelSnapshot,
};
use async_trait::async_trait;
use brassclaw_engine::{
    executor::{
        retained_recipe::{
            RetainedProgram, RetainedRecipeExecution, RetainedStepFailure, RetainedToolPort,
            RetainedTransportEvidence, port_answer_checksum, port_answer_checksum_with_limits,
            typed_value_checksum, typed_value_checksum_with_limits,
        },
        retained_source::InspectedRetainedProgram,
    },
    memory::retained_tools::RetainedToolProgram,
};
use brassclaw_monty_host::{
    VmFailure,
    global::Lifecycle,
    process::{PortAnswer, ProcessFailure, RecipeCommand, WorkerCommand},
    service::{PortFailure, TaskOutcome},
};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

fn check(ok: bool, reason: &str) -> Result<(), RebornBuildError> {
    if ok { Ok(()) } else { Err(invalid(reason)) }
}
fn failed_step(case: &str) -> Option<&'static str> {
    match case {
        "bad-packet" => Some("0:1"),
        "denied" => Some("0:3"),
        "missing-leading-prefix" => Some("0:5"),
        _ => None,
    }
}
fn calls(case: &str) -> usize {
    match case {
        "bad-packet" => 0,
        "denied" => 1,
        _ => 2,
    }
}
fn order(case: &str) -> Vec<String> {
    let all = ["0:1", "0:3", "0:5", "0:6"];
    let length = failed_step(case)
        .map(|step| all.iter().position(|id| *id == step).expect("fixed step") + 1)
        .unwrap_or(all.len());
    all[..length].iter().map(|step| (*step).into()).collect()
}
fn hash(value: &Value) -> Result<String, RebornBuildError> {
    typed_value_checksum(value)
        .map(hex::encode)
        .map_err(invalid)
}
struct NativeCase {
    encoded: Option<String>,
    arguments: BTreeMap<String, Value>,
    answers: BTreeMap<String, PortAnswer>,
}

async fn native_case(
    tx: &Transaction<'_>,
    program: &RetainedToolProgram,
    fixture: &Fixture,
    case: &str,
    run: Uuid,
    worker: &str,
) -> Result<NativeCase, RebornBuildError> {
    let rows=tx.query("SELECT step_id,recipe_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id",&[&run]).await.map_err(invalid)?;
    check(
        rows.len() == calls(case),
        "pair durable invocation count differs",
    )?;
    let prepared = fixture.prepared(case)?;
    let mut observed = NativeCase {
        encoded: None,
        arguments: BTreeMap::new(),
        answers: BTreeMap::new(),
    };
    for row in rows {
        let step: String = row.get(0);
        check(
            step == if observed.arguments.is_empty() {
                "0:3"
            } else {
                "0:5"
            },
            "pair repeated/foreign native step",
        )?;
        let selection =
            crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, &step)
                .map_err(invalid)?;
        check(
            row.get::<_, Uuid>(1) == program.inputs().instruction().recipe().uuid
                && row.get::<_, String>(2) == selection
                && row.get::<_, String>(3) == crate::bootstrap_evidence::checksum(&selection)
                && row.get::<_, String>(6) == "answered"
                && row.get::<_, i16>(7) == 1,
            "pair native selection/attempt differs",
        )?;
        let arguments = brassclaw_skills::value_contract::strict_json(
            &row.get::<_, String>(4),
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(invalid)?;
        let answer: PortAnswer = serde_json::from_str(
            row.get::<_, Option<String>>(5)
                .as_deref()
                .ok_or_else(|| invalid("pair native answer unresolved"))?,
        )
        .map_err(invalid)?;
        if step == "0:3" {
            let texts = &prepared["texts"];
            check(
                arguments
                    == json!({"operation":"sha256_utf8","data":serde_json::to_string(texts).map_err(invalid)?}),
                "pair digest arguments differ",
            )?;
            let expected = if case == "denied" {
                PortAnswer::TerminalError {
                    reason_kind: format!(
                        "retained_tool_{}",
                        brassclaw_host_runtime::RuntimeFailureKind::Authorization.as_str()
                    ),
                }
            } else {
                PortAnswer::Return{value:json!({"digests":texts.as_array().ok_or_else(||invalid("pair texts missing"))?.iter().map(|text|text.as_str().map(|text|crate::bootstrap_encoding_fixture::checksum(text.as_bytes())).ok_or_else(||invalid("pair text missing"))).collect::<Result<Vec<_>,_>>()?}).into()}
            };
            check(
                port_answer_checksum(&answer).map_err(invalid)?
                    == port_answer_checksum(&expected).map_err(invalid)?,
                "pair digest outcome differs",
            )?;
        } else {
            check(
                arguments
                    == json!({"specification":fixture.specification,"artifact_set":fixture.artifacts,"probes_json":prepared["probes_json"]}),
                "pair original-template arguments differ",
            )?;
            if case == "missing-leading-prefix" {
                // Absent leading bytes make the requested partition invalid.
                // A different expected suffix instead completes with a mismatch.
                check(
                    matches!(&answer,PortAnswer::TerminalError{reason_kind} if reason_kind==&format!("retained_tool_{}",brassclaw_host_runtime::RuntimeFailureKind::InvalidInput.as_str())),
                    "pair leading-prefix InvalidInput classification differs",
                )?;
            } else {
                let PortAnswer::Return { value } = &answer else {
                    return Err(invalid("pair encoding success missing"));
                };
                let bytes = value
                    .as_str()
                    .ok_or_else(|| invalid("pair encoding bytes missing"))?;
                fixture.verify_observation(case, bytes, worker)?;
                observed.encoded = Some(bytes.into());
            }
        }
        observed.arguments.insert(step.clone(), arguments);
        observed.answers.insert(step, answer);
    }
    Ok(observed)
}
struct ExpectedSteps {
    inputs: BTreeMap<String, Value>,
    results: BTreeMap<String, Value>,
}
fn values(
    fixture: &Fixture,
    case: &str,
    native: &NativeCase,
) -> Result<ExpectedSteps, RebornBuildError> {
    let mut inputs = BTreeMap::from([("0:1".into(), fixture.inputs(case)?)]);
    let mut results = BTreeMap::new();
    if case == "bad-packet" {
        return Ok(ExpectedSteps { inputs, results });
    }
    let prepared = fixture.prepared(case)?;
    results.insert("0:1".into(), prepared.clone());
    inputs.insert("0:3".into(), json!({"texts":prepared["texts"]}));
    if case == "denied" {
        return Ok(ExpectedSteps { inputs, results });
    }
    let PortAnswer::Return { value: digests } = &native.answers["0:3"] else {
        return Err(invalid("pair digests missing"));
    };
    results.insert("0:3".into(), digests.as_value().clone());
    inputs.insert("0:5".into(), native.arguments["0:5"].clone());
    if case == "missing-leading-prefix" {
        return Ok(ExpectedSteps { inputs, results });
    }
    let bytes = native
        .encoded
        .as_deref()
        .ok_or_else(|| invalid("pair observation missing"))?;
    results.insert("0:5".into(), json!(bytes));
    inputs.insert("0:6".into(),json!({"bank_bytes":prepared["bank_bytes"],"digests":digests["digests"],"observation_bytes":bytes}));
    let observed: Value = serde_json::from_str(bytes).map_err(invalid)?;
    results.insert(
        "0:6".into(),
        json!(
            crate::prefix_generations::canonical(&fixture.comparison(case, &observed))
                .map_err(invalid)?
        ),
    );
    Ok(ExpectedSteps { inputs, results })
}
struct Invocation {
    reference: String,
    inputs: Value,
    fixture: Arc<Fixture>,
    case: &'static str,
    worker: String,
    run: Uuid,
    pool: Arc<PgPool>,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "packaged-prefix-pair-qualification"
    }
    fn reference(&self) -> &str {
        &self.reference
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
        let bad = || PortFailure::new("pair_receipt_invalid").expect("constant classification");
        if failed_step(self.case).is_some()
            || !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.host_answers().len() != 2
            || execution.transport_failure().is_some()
        {
            return Err(bad());
        }
        let RetainedProgram::Tools(program) = selected.program() else {
            return Err(bad());
        };
        let mut client = self.pool.get().await.map_err(|_| bad())?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(|_| bad())?;
        let native = native_case(
            &tx,
            program,
            &self.fixture,
            self.case,
            self.run,
            &self.worker,
        )
        .await
        .map_err(|_| bad())?;
        let ExpectedSteps { results, .. } =
            values(&self.fixture, self.case, &native).map_err(|_| bad())?;
        if results["0:6"].as_str() != Some(receipt) {
            return Err(bad());
        }
        tx.commit().await.map_err(|_| bad())?;
        Ok(())
    }
}

pub(super) async fn execute(
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    inspected: Arc<InspectedRetainedProgram>,
    worker: &Path,
    worker_checksum: &str,
    fixture: Arc<Fixture>,
) -> Result<Value, RebornBuildError> {
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("pair Tool program missing"));
    };
    let root = crate::global_root_seed::retain_packaged_global_root(
        pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .map_err(invalid)?;
    let before = serving(&pool).await?;
    let mut reports = Vec::new();
    for &case in CASES {
        let reference = format!("closed-prefix-pair:{case}:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            pool.clone(),
            "bootstrap-prefix-pair",
            &reference,
            None,
        ))
        .await?;
        let tools = crate::installed_monty_catalogue::bootstrap_pair_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            case == "denied",
        )?;
        let inputs = fixture.inputs(case)?;
        let request = bootstrap_recipe_flow::Request::contained(
            worker.to_owned(),
            executor.clone(),
            &root,
            Arc::new(Invocation {
                reference,
                inputs: inputs.clone(),
                fixture: fixture.clone(),
                case,
                worker: worker_checksum.into(),
                run: admitted.attempt.run_id.as_uuid(),
                pool: pool.clone(),
                inspected: inspected.clone(),
                tools,
            }),
        )?;
        // Admission capacity and the acknowledged VM execution profile are
        // distinct contracts. The executor captures the latter at each step.
        let bounds = request.boot.bounds.values;
        let data_limits = json!({"max_depth":bounds.max_value_depth,"max_nodes":bounds.max_value_nodes,"max_bytes":bounds.max_value_bytes});
        let observed = bootstrap_recipe_flow::execute(&admitted, request).await?;
        let success = failed_step(case).is_none();
        check(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ) == success,
            "pair root outcome differs",
        )?;
        check(
            observed.report["root_completed"] == success
                && observed.report["model_exchange_count"] == 0
                && observed.report["reply"].is_null()
                && observed.report["semantic_approval"] == false,
            "pair root report differs",
        )?;
        let (steps,rejection)=observed.ports.observe_internal_execution(|execution|->Result<_,RebornBuildError> {
            check(execution.observation_order()==Some(order(case).as_slice()) && execution.observations().len()==order(case).len() && execution.host_answers().len()==calls(case),"pair step/call order differs")?;
            let mut steps=serde_json::Map::new();
            for (step,proof) in execution.observations() {
                check(proof.settled() && proof.observation_error().is_none(),"pair observation unresolved")?;
                steps.insert(step.clone(),json!({"limits":{"max_depth":proof.data_limits().max_depth,"max_nodes":proof.data_limits().max_nodes,"max_bytes":proof.data_limits().max_bytes},"inputs":hex::encode(proof.input_checksum()),"arguments":proof.arguments_checksum().map(hex::encode),"answer":proof.answer_checksum().map(hex::encode),"result":proof.result_checksum().map(hex::encode),"failure":proof.failure().map(RetainedStepFailure::reason_kind)}));
            }
            let rejection=if let Some(step)=failed_step(case) {
                check(!execution.is_complete() && execution.failed_step_id()==Some(step),"pair negative completed")?;
                let Some(RetainedTransportEvidence::Process {error,command,..})=execution.transport_failure() else {return Err(invalid("pair rejection lacks process proof"));};
                check(error.kind==ProcessFailure::Vm(VmFailure::Python) && error.snapshot.as_ref().is_some_and(|s|s.lifecycle==Lifecycle::Ready && s.root.as_ref().is_some_and(|r|r.vm_id()==observed.receipt.root.vm_id())),"pair failed outside contained Python step")?;
                let command=match command.as_ref().map_err(invalid)?.as_ref() {
                    WorkerCommand::Recipe {command:RecipeCommand::Start {..}}=>"start",
                    WorkerCommand::Recipe {command:RecipeCommand::ResumeControl {..}}=>"resume-control",
                    WorkerCommand::Recipe {command:RecipeCommand::ResumeHost {..}}=>"resume-host",
                    _=>return Err(invalid("pair rejection command differs")),
                };
                json!({"step":step,"command":command,"root_vm":observed.receipt.root.vm_id()})
            } else {check(execution.is_complete() && execution.transport_failure().is_none(),"pair success incomplete")?;Value::Null};
            Ok((steps,rejection))
        }).await.map_err(|e|invalid(format!("pair observation: {e:?}")))??;
        check(
            admitted.host.finalized_reply_ref().is_none()
                && observed.exit.failure.is_none()
                && observed.exit.pending_admission.is_none()
                && observed.exit.queued_admissions.is_empty(),
            "pair worker settlement incomplete",
        )?;
        let exit = observed
            .exit
            .transport
            .map_err(|e| invalid(format!("pair containment: {e:?}")))?;
        check(
            exit.exit_status.is_some()
                && exit.containment_error.is_none()
                && exit.reap_error.is_none()
                && exit.shutdown_failure.is_none(),
            "pair worker containment unresolved",
        )?;
        bootstrap_validation::settle_closed(&admitted).await?;
        reports.push(json!({"case":case,"run_id":admitted.attempt.run_id.as_uuid(),"thread_id":admitted.host.run_context().thread_id,"root_vm":observed.receipt.root.vm_id(),"order":order(case),"steps":steps,"rejection":rejection,"execution":observed.report,"inputs_checksum":hash(&inputs)?,"data_limits":data_limits}));
    }
    let after = serving(&pool).await?;
    check(
        before == after,
        "pair qualification changed serving selection",
    )?;
    Ok(
        json!({"format":"prefix-pair-behavior-observations/1","cases":reports,"serving_before":before,"serving_after":after,"cache_quality":"unmeasured","model_quality":"unmeasured"}),
    )
}
async fn serving(pool: &PgPool) -> Result<String, RebornBuildError> {
    let bytes:String=pool.get().await.map_err(invalid)?.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb)::text FROM reborn_basic_prompt_store s",&[]).await.map_err(invalid)?.get(0);
    Ok(crate::bootstrap_evidence::checksum(&bytes))
}
pub(super) async fn verify(
    tx: &Transaction<'_>,
    inspected: &InspectedRetainedProgram,
    fixture: &Fixture,
    worker: &str,
    observed: &Value,
) -> Result<(), RebornBuildError> {
    crate::bootstrap_evidence::require_coherent(tx).await?;
    check(
        observed.as_object().is_some_and(|v| v.len() == 6)
            && observed["format"] == "prefix-pair-behavior-observations/1"
            && observed["cache_quality"] == "unmeasured"
            && observed["model_quality"] == "unmeasured"
            && observed["serving_before"]
                .as_str()
                .is_some_and(|s| s.len() == 64)
            && observed["serving_before"] == observed["serving_after"],
        "pair behavior envelope differs",
    )?;
    let records = observed["cases"]
        .as_array()
        .filter(|v| v.len() == CASES.len())
        .ok_or_else(|| invalid("pair behavior cases incomplete"))?;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("pair Tool program missing"));
    };
    let selected = program
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let mut runs = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut threads = BTreeSet::new();
    let mut duplicate = None;
    let mut preparation: Option<Value> = None;
    for (record, &case) in records.iter().zip(CASES) {
        let run: Uuid = serde_json::from_value(record["run_id"].clone()).map_err(invalid)?;
        let thread = record["thread_id"]
            .as_str()
            .ok_or_else(|| invalid("pair thread missing"))?;
        check(
            record.as_object().is_some_and(|v| v.len() == 10)
                && record["case"] == case
                && !run.is_nil()
                && runs.insert(run)
                && !record["root_vm"].is_null()
                && roots.insert(record["root_vm"].to_string())
                && thread.starts_with("bootstrap-prefix-pair-")
                && threads.insert(thread)
                && record["inputs_checksum"] == hash(&fixture.inputs(case)?)?
                && record["order"] == json!(order(case)),
            "pair case identity/order differs",
        )?;
        let row=tx.query_one("SELECT s.selection_bytes,s.selection_checksum,a.phase,a.outcome,a.scope FROM brassclaw_monty_recipe_selections s JOIN brassclaw_monty_task_admissions a USING(run_id) WHERE s.run_id=$1 AND s.recipe_id=$2",&[&run,&program.inputs().instruction().recipe().uuid]).await.map_err(invalid)?;
        let scope: Value = row.get(4);
        check(
            row.get::<_, String>(0) == selected.exact_bytes()
                && row.get::<_, String>(1) == hex::encode(selected.checksum())
                && row.get::<_, String>(2) == "settled"
                && row.get::<_, Option<Value>>(3)
                    == Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
                && scope["tenant_id"] == crate::bootstrap_validation::NAMESPACE
                && scope["agent_id"] == "bootstrap-prefix-pair"
                && scope["thread_id"] == thread,
            "pair durable admission differs",
        )?;
        let native = native_case(tx, program, fixture, case, run, worker).await?;
        let ExpectedSteps { inputs, results } = values(fixture, case, &native)?;
        let step_records = record["steps"]
            .as_object()
            .filter(|s| s.len() == order(case).len())
            .ok_or_else(|| invalid("pair step records incomplete"))?;
        for step in order(case) {
            let proof = step_records
                .get(&step)
                .ok_or_else(|| invalid("pair step identity missing"))?;
            let expected_failure = failed_step(case) == Some(step.as_str());
            let limit = |name| {
                proof["limits"][name]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|n| *n > 0)
                    .ok_or_else(|| invalid("pair captured limits missing"))
            };
            let limits = brassclaw_skills::value_contract::ContractLimits {
                max_depth: limit("max_depth")?,
                max_nodes: limit("max_nodes")?,
                max_bytes: limit("max_bytes")?,
            };
            check(
                proof["limits"] == record["data_limits"]
                    && proof["limits"].as_object().is_some_and(|l| l.len() == 3),
                "pair captured bounds differ",
            )?;
            let hash_step = |value| {
                typed_value_checksum_with_limits(value, limits)
                    .map(hex::encode)
                    .map_err(invalid)
            };

            let argument = native.arguments.get(&step).map(hash_step).transpose()?;
            let answer = native
                .answers
                .get(&step)
                .map(|answer| port_answer_checksum_with_limits(answer, limits))
                .transpose()
                .map_err(invalid)?
                .map(hex::encode);
            check(
                proof.as_object().is_some_and(|p| p.len() == 6)
                    && proof["inputs"] == hash_step(&inputs[&step])?
                    && proof["arguments"] == json!(argument)
                    && proof["answer"] == json!(answer)
                    && proof["result"]
                        == if expected_failure {
                            Value::Null
                        } else {
                            json!(hash_step(&results[&step])?)
                        }
                    && proof["failure"]
                        == if expected_failure {
                            json!(RetainedStepFailure::Process.reason_kind())
                        } else {
                            Value::Null
                        },
                "pair actual step input/result differs from oracle",
            )?;
        }
        let success = failed_step(case).is_none();
        check(
            record["execution"]["root_completed"] == success
                && record["execution"]["model_exchange_count"] == 0
                && record["execution"]["reply"].is_null()
                && record["execution"]["semantic_approval"] == false,
            "pair root evidence differs",
        )?;
        if success {
            check(
                record["rejection"].is_null()
                    && record["execution"]["internal_receipt"] == results["0:6"],
                "pair exact receipt differs",
            )?;
            if case == "matches" {
                duplicate = Some(results["0:6"].clone());
            }
            if case == "duplicate" {
                check(
                    duplicate.as_ref() == Some(&results["0:6"]),
                    "pair duplicate is nondeterministic",
                )?;
            }
            let comparison: Value = serde_json::from_str(
                results["0:6"]
                    .as_str()
                    .ok_or_else(|| invalid("pair result missing"))?,
            )
            .map_err(invalid)?;
            check(
                comparison["all_cases_match"] == (case != "changed-suffix")
                    && comparison["stable_prefix_encoding"] == true,
                "pair independent comparison differs",
            )?;
        } else {
            check(
                record["rejection"]
                    .as_object()
                    .is_some_and(|r| r.len() == 3)
                    && record["rejection"]["step"] == failed_step(case).expect("negative step")
                    && record["rejection"]["root_vm"] == record["root_vm"]
                    && matches!(
                        record["rejection"]["command"].as_str(),
                        Some("start" | "resume-control" | "resume-host")
                    ),
                "pair rejection evidence differs",
            )?;
        }
        if let Some(bytes) = native.encoded {
            let value: Value = serde_json::from_str(&bytes).map_err(invalid)?;
            if let Some(revision) = preparation.as_ref() {
                check(
                    &value["preparation_revision"] == revision,
                    "pair preparation changed during qualification",
                )?;
            } else {
                preparation = Some(value["preparation_revision"].clone());
            }
        }
    }
    Ok(())
}
