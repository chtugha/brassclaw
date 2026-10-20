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
            RetainedTransportEvidence, port_answer_checksum, typed_value_checksum,
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
        "wrong-original" => Some("0:8"),
        "unsupported-template" => Some("0:2"),
        "denied" => Some("0:5"),
        "bad-assets" => Some("0:7"),
        "bad-input" => Some("0:1"),
        _ => None,
    }
}
fn calls(case: &str) -> usize {
    match case {
        "unsupported-template" | "bad-input" => 0,
        "denied" => 1,
        _ => 2,
    }
}
fn order(case: &str) -> Vec<String> {
    let all = ["0:1", "0:2", "0:3", "0:5", "0:7", "0:8", "0:9", "0:10"];
    let length = failed_step(case)
        .map(|step| all.iter().position(|id| *id == step).expect("fixed step") + 1)
        .unwrap_or(all.len());
    all[..length].iter().map(|s| (*s).to_owned()).collect()
}
fn hash(value: &Value) -> Result<String, RebornBuildError> {
    typed_value_checksum(value)
        .map(hex::encode)
        .map_err(invalid)
}
struct NativeCase {
    template: Option<String>,
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
        "measurement durable invocation count differs",
    )?;
    let mut observed = NativeCase {
        template: None,
        encoded: None,
        arguments: BTreeMap::new(),
        answers: BTreeMap::new(),
    };
    let request = fixture.request(case);
    for row in rows {
        let step: String = row.get(0);
        check(
            step == if observed.arguments.is_empty() {
                "0:5"
            } else {
                "0:7"
            },
            "measurement dispatched another/repeated step",
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
            "measurement durable selection/attempt differs",
        )?;
        let arguments: Value = brassclaw_skills::value_contract::strict_json(
            &row.get::<_, String>(4),
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(invalid)?;
        let answer: PortAnswer = serde_json::from_str(
            &row.get::<_, Option<String>>(5)
                .ok_or_else(|| invalid("measurement native answer unresolved"))?,
        )
        .map_err(invalid)?;
        if step == "0:5" {
            let texts: Value = brassclaw_skills::value_contract::strict_json(
                arguments["data"]
                    .as_str()
                    .ok_or_else(|| invalid("measurement digest data missing"))?,
                brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
            )
            .map_err(invalid)?;
            let list = texts
                .as_array()
                .filter(|a| a.len() == 2)
                .ok_or_else(|| invalid("measurement template pair differs"))?;
            let template = list[0]
                .as_str()
                .ok_or_else(|| invalid("measurement effective template missing"))?;
            check(
                list[1] == request["original_template"]
                    && arguments
                        == json!({"operation":"sha256_utf8","data":crate::prefix_generations::canonical(&texts).map_err(invalid)?}),
                "measurement exact digest arguments differ",
            )?;
            let expected = if case == "denied" {
                PortAnswer::TerminalError {
                    reason_kind: "retained_tool_authorization".into(),
                }
            } else {
                PortAnswer::Return {value:json!({"digests":[crate::bootstrap_encoding_fixture::checksum(template.as_bytes()),crate::bootstrap_encoding_fixture::checksum(list[1].as_str().ok_or_else(||invalid("measurement original template missing"))?.as_bytes())]}).into()}
            };
            check(
                port_answer_checksum(&answer).map_err(invalid)?
                    == port_answer_checksum(&expected).map_err(invalid)?,
                "measurement digest outcome differs",
            )?;
            observed.template = Some(template.to_owned());
        } else {
            let template = observed
                .template
                .as_deref()
                .ok_or_else(|| invalid("measurement model ran before digest"))?;
            check(
                arguments
                    == json!({"specification":request["specification"],"artifact_set":request["artifact_set"],"probes_json":request["probes_json"],"template_utf8":template}),
                "measurement exact model arguments differ",
            )?;
            if case == "bad-assets" {
                check(
                    matches!(&answer,PortAnswer::TerminalError {reason_kind} if reason_kind=="retained_tool_operation_failed"),
                    "measurement asset rejection differs",
                )?;
            } else {
                let PortAnswer::Return { value } = &answer else {
                    return Err(invalid("measurement model result missing"));
                };
                let bytes = value
                    .as_str()
                    .ok_or_else(|| invalid("measurement model JSON bytes missing"))?;
                fixture.verify_observation(bytes, template, worker)?;
                observed.encoded = Some(bytes.to_owned());
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
    let request = fixture.request(case);
    let mut inputs = BTreeMap::from([("0:1".into(), fixture.inputs(case)?)]);
    let mut results = BTreeMap::new();
    if case == "bad-input" {
        return Ok(ExpectedSteps { inputs, results });
    }
    results.insert("0:1".into(), request.clone());
    inputs.insert(
        "0:2".into(),
        json!({"reference":request["reference"],"original_template":request["original_template"]}),
    );
    if case == "unsupported-template" {
        return Ok(ExpectedSteps { inputs, results });
    }
    let template = native
        .template
        .as_deref()
        .ok_or_else(|| invalid("measurement template result missing"))?;
    results.insert("0:2".into(), json!(template));
    inputs.insert(
        "0:3".into(),
        json!({"effective_template":template,"original_template":request["original_template"]}),
    );
    let texts = json!([template, request["original_template"]]);
    results.insert("0:3".into(), texts.clone());
    inputs.insert("0:5".into(), json!({"texts":texts}));
    if case == "denied" {
        return Ok(ExpectedSteps { inputs, results });
    }
    let PortAnswer::Return { value: digests } = &native.answers["0:5"] else {
        return Err(invalid("measurement digests missing"));
    };
    results.insert("0:5".into(), digests.as_value().clone());
    inputs.insert("0:7".into(), native.arguments["0:7"].clone());
    if case == "bad-assets" {
        return Ok(ExpectedSteps { inputs, results });
    }
    let bytes = native
        .encoded
        .as_deref()
        .ok_or_else(|| invalid("measurement observation missing"))?;
    results.insert("0:7".into(), json!(bytes));
    inputs.insert(
        "0:8".into(),
        json!({"digests":digests["digests"],"observation_bytes":bytes}),
    );
    if case == "wrong-original" {
        return Ok(ExpectedSteps { inputs, results });
    }
    let template_hash = crate::bootstrap_encoding_fixture::checksum(template.as_bytes());
    results.insert(
        "0:8".into(),
        json!({"effective_template_sha256":template_hash,"observation_bytes":bytes}),
    );
    inputs.insert("0:9".into(),json!({"observation_bytes":bytes,"expected_specification":request["specification"],"expected_artifact_set":request["artifact_set"],"expected_template_sha256":template_hash,"limits_ref":request["limits_ref"],"context_window_tokens":request["context_window_tokens"],"reserved_output_tokens":request["reserved_output_tokens"],"reserved_additional_input_tokens":request["reserved_additional_input_tokens"]}));
    let observation: Value = serde_json::from_str(bytes).map_err(invalid)?;
    let comparison = fixture.comparison(case, &observation, template);
    results.insert("0:9".into(), comparison.clone());
    inputs.insert("0:10".into(), json!({"comparison":comparison}));
    results.insert(
        "0:10".into(),
        json!(crate::prefix_generations::canonical(&comparison).map_err(invalid)?),
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
        "packaged-prefix-measurement-qualification"
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
        let bad =
            || PortFailure::new("measurement_receipt_invalid").expect("constant classification");
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
        if results["0:10"].as_str() != Some(receipt) {
            return Err(bad());
        }
        tx.commit().await.map_err(|_| bad())?;
        Ok(())
    }
}

pub(super) async fn execute(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    inspected: Arc<InspectedRetainedProgram>,
    worker: &Path,
    worker_checksum: &str,
    fixture: Arc<Fixture>,
) -> Result<Value, RebornBuildError> {
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("measurement Tool program missing"));
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
        let reference = format!("closed-prefix-measurement:{case}:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            pool.clone(),
            "bootstrap-prefix-measurement",
            &reference,
        ))
        .await?;
        let tools = crate::installed_monty_catalogue::bootstrap_measurement_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            case == "denied",
        )?;
        let inputs = fixture.inputs(case)?;
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            bootstrap_recipe_flow::Request::contained(
                worker.to_owned(),
                inspected.preparation_limits(),
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
            )?,
        )
        .await?;
        let success = failed_step(case).is_none();
        check(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ) == success,
            "measurement root outcome differs",
        )?;
        check(
            observed.report["root_completed"] == success
                && observed.report["model_exchange_count"] == 0
                && observed.report["reply"].is_null()
                && observed.report["semantic_approval"] == false,
            "measurement root report differs",
        )?;
        let (steps,rejection)=observed.ports.observe_internal_execution(|execution|->Result<_,RebornBuildError> {
            check(execution.observation_order()==Some(order(case).as_slice()) && execution.observations().len()==order(case).len() && execution.host_answers().len()==calls(case),"measurement step/call order differs")?;
            let mut steps=serde_json::Map::new();
            for (step,proof) in execution.observations() {
                check(proof.settled() && proof.observation_error().is_none(),"measurement observation unresolved")?;
                steps.insert(step.clone(),json!({"inputs":hex::encode(proof.input_checksum()),"arguments":proof.arguments_checksum().map(hex::encode),"answer":proof.answer_checksum().map(hex::encode),"result":proof.result_checksum().map(hex::encode),"failure":proof.failure().map(RetainedStepFailure::reason_kind)}));
            }
            let rejection=if let Some(step)=failed_step(case) {
                check(!execution.is_complete() && execution.failed_step_id()==Some(step),"measurement negative completed")?;
                let Some(RetainedTransportEvidence::Process {error,command,..})=execution.transport_failure() else {return Err(invalid("measurement rejection lacks process proof"));};
                check(error.kind==ProcessFailure::Vm(VmFailure::Python) && error.snapshot.as_ref().is_some_and(|s|s.lifecycle==Lifecycle::Ready && s.root.as_ref().is_some_and(|r|r.vm_id()==observed.receipt.root.vm_id())),"measurement failed outside contained Python step")?;
                let command=match command.as_ref().map_err(invalid)?.as_ref() {
                    WorkerCommand::Recipe {command:RecipeCommand::Start {..}}=>"start",
                    WorkerCommand::Recipe {command:RecipeCommand::ResumeControl {..}}=>"resume-control",
                    WorkerCommand::Recipe {command:RecipeCommand::ResumeHost {..}}=>"resume-host",
                    _=>return Err(invalid("measurement rejection command differs")),
                };
                json!({"step":step,"command":command,"root_vm":observed.receipt.root.vm_id()})
            } else {check(execution.is_complete() && execution.transport_failure().is_none(),"measurement success incomplete")?;Value::Null};
            Ok((steps,rejection))
        }).await.map_err(|e|invalid(format!("measurement observation: {e:?}")))??;
        check(
            admitted.host.finalized_reply_ref().is_none()
                && observed.exit.failure.is_none()
                && observed.exit.pending_admission.is_none()
                && observed.exit.queued_admissions.is_empty(),
            "measurement worker settlement incomplete",
        )?;
        let exit = observed
            .exit
            .transport
            .map_err(|e| invalid(format!("measurement containment: {e:?}")))?;
        check(
            exit.exit_status.is_some()
                && exit.containment_error.is_none()
                && exit.reap_error.is_none()
                && exit.shutdown_failure.is_none(),
            "measurement worker containment unresolved",
        )?;
        bootstrap_validation::settle_closed(&admitted).await?;
        reports.push(json!({"case":case,"run_id":admitted.attempt.run_id.as_uuid(),"thread_id":admitted.host.run_context().thread_id,"root_vm":observed.receipt.root.vm_id(),"order":order(case),"steps":steps,"rejection":rejection,"execution":observed.report,"inputs_checksum":hash(&inputs)?}));
    }
    let after = serving(&pool).await?;
    check(
        before == after,
        "measurement qualification changed serving selection",
    )?;
    Ok(
        json!({"format":"prefix-measurement-behavior-observations/1","cases":reports,"serving_before":before,"serving_after":after,"cache_quality":"unmeasured","model_quality":"unmeasured"}),
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
            && observed["format"] == "prefix-measurement-behavior-observations/1"
            && observed["cache_quality"] == "unmeasured"
            && observed["model_quality"] == "unmeasured"
            && observed["serving_before"]
                .as_str()
                .is_some_and(|s| s.len() == 64)
            && observed["serving_before"] == observed["serving_after"],
        "measurement behavior envelope differs",
    )?;
    let records = observed["cases"]
        .as_array()
        .filter(|v| v.len() == CASES.len())
        .ok_or_else(|| invalid("measurement behavior cases incomplete"))?;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("measurement Tool program missing"));
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
            .ok_or_else(|| invalid("measurement thread missing"))?;
        check(
            record.as_object().is_some_and(|v| v.len() == 9)
                && record["case"] == case
                && !run.is_nil()
                && runs.insert(run)
                && !record["root_vm"].is_null()
                && roots.insert(record["root_vm"].to_string())
                && thread.starts_with("bootstrap-prefix-measurement-")
                && threads.insert(thread)
                && record["inputs_checksum"] == hash(&fixture.inputs(case)?)?
                && record["order"] == json!(order(case)),
            "measurement case identity/order differs",
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
                && scope["agent_id"] == "bootstrap-prefix-measurement"
                && scope["thread_id"] == thread,
            "measurement durable admission differs",
        )?;
        let native = native_case(tx, program, fixture, case, run, worker).await?;
        let ExpectedSteps { inputs, results } = values(fixture, case, &native)?;
        let step_records = record["steps"]
            .as_object()
            .filter(|s| s.len() == order(case).len())
            .ok_or_else(|| invalid("measurement step records incomplete"))?;
        for step in order(case) {
            let proof = step_records
                .get(&step)
                .ok_or_else(|| invalid("measurement step identity missing"))?;
            let expected_failure = failed_step(case) == Some(step.as_str());
            let argument = native.arguments.get(&step).map(hash).transpose()?;
            let answer = native
                .answers
                .get(&step)
                .map(port_answer_checksum)
                .transpose()
                .map_err(invalid)?
                .map(hex::encode);
            check(
                proof.as_object().is_some_and(|p| p.len() == 5)
                    && proof["inputs"] == hash(&inputs[&step])?
                    && proof["arguments"] == json!(argument)
                    && proof["answer"] == json!(answer)
                    && proof["result"]
                        == if expected_failure {
                            Value::Null
                        } else {
                            json!(hash(&results[&step])?)
                        }
                    && proof["failure"]
                        == if expected_failure {
                            json!(RetainedStepFailure::Process.reason_kind())
                        } else {
                            Value::Null
                        },
                "measurement actual step input/result differs from oracle",
            )?;
        }
        let success = failed_step(case).is_none();
        check(
            record["execution"]["root_completed"] == success
                && record["execution"]["model_exchange_count"] == 0
                && record["execution"]["reply"].is_null()
                && record["execution"]["semantic_approval"] == false,
            "measurement root evidence differs",
        )?;
        if success {
            check(
                record["rejection"].is_null()
                    && record["execution"]["internal_receipt"] == results["0:10"],
                "measurement exact receipt differs",
            )?;
            if case == "fits" {
                duplicate = Some(results["0:10"].clone());
            }
            if case == "duplicate" {
                check(
                    duplicate.as_ref() == Some(&results["0:10"]),
                    "measurement duplicate is nondeterministic",
                )?;
            }
            check(
                results["0:9"]["fits"] == (case != "does-not-fit"),
                "measurement capacity decision differs",
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
                "measurement rejection evidence differs",
            )?;
        }
        if let Some(bytes) = native.encoded {
            let value: Value = serde_json::from_str(&bytes).map_err(invalid)?;
            if let Some(revision) = preparation.as_ref() {
                check(
                    &value["preparation_revision"] == revision,
                    "measurement preparation changed during qualification",
                )?;
            } else {
                preparation = Some(value["preparation_revision"].clone());
            }
        }
    }
    Ok(())
}
