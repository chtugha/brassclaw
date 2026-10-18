//! Exact packaged usage oracles using real contained execution and durable rows.
//! Expected results are reconstructed from submitted originals and PostgreSQL,
//! never accepted from a caller-provided success flag or model output.
use crate::{
    RebornBuildError, bootstrap_generation_approval::Fixture, bootstrap_reply_validation::invalid,
    bootstrap_validation, monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedExecutionError, RetainedProgram, RetainedRecipeExecution, RetainedStepFailure,
        RetainedTransportEvidence, port_answer_checksum, typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::{
    VmFailure,
    global::{GlobalBounds, Lifecycle},
    heap::HeapSettings,
    process::{PortAnswer, ProcessFailure, RecipeCommand, RootBoot, TaskSettings, WorkerCommand},
    transport_actor::ActorLimits,
};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio_postgres::Transaction;
use uuid::Uuid;

fn check(condition: bool, reason: &str) -> Result<(), RebornBuildError> {
    if condition {
        Ok(())
    } else {
        Err(invalid(reason))
    }
}
fn hash(value: &Value) -> Result<String, RebornBuildError> {
    typed_value_checksum(value)
        .map(hex::encode)
        .map_err(invalid)
}
fn missing() -> String {
    hex::encode(Sha256::digest(
        b"prefix-generation-bootstrap-intentionally-missing/1",
    ))
}

pub(super) fn case_inventory(retain: bool) -> Value {
    if retain {
        json!([
            {"name":"retain","outcome":"return","calls":1}, {"name":"duplicate","outcome":"return","calls":1},
            {"name":"invalid-package","outcome":"retained_tool_invalid_input","calls":1},
            {"name":"revision-conflict","outcome":"retained_tool_operation_failed","calls":1},
            {"name":"denied","outcome":"retained_tool_authorization","calls":1},
            {"name":"invalid-revision","outcome":"pure_failure","calls":0}
        ])
    } else {
        json!([
            {"name":"read","outcome":"return","calls":1}, {"name":"missing","outcome":"retained_tool_operation_failed","calls":1},
            {"name":"invalid-generation","outcome":"retained_tool_invalid_input","calls":1},
            {"name":"denied","outcome":"retained_tool_authorization","calls":1}
        ])
    }
}
fn inputs(case: &Value, fixture: &Fixture, retain: bool) -> Value {
    if retain {
        let revision = if case["name"] == "invalid-revision" {
            "01".into()
        } else if case["name"] == "revision-conflict" {
            if fixture.revision == 1 {
                "2".into()
            } else {
                "1".into()
            }
        } else {
            fixture.revision.to_string()
        };
        json!({"profile":"base-prompt","revision":revision,"generation_bytes":if case["name"]=="invalid-package"{"{not-a-package}"}else{&fixture.package}})
    } else {
        json!({"profile":"base-prompt","generation":if case["name"]=="missing"{missing()}else if case["name"]=="invalid-generation"{"invalid-hash".into()}else{fixture.generation.clone()}})
    }
}
fn step_inputs(task: &Value, retain: bool) -> Result<BTreeMap<String, Value>, RebornBuildError> {
    if retain {
        let mut result = BTreeMap::from([("0:1".into(), json!({"revision":task["revision"]}))]);
        if task["revision"] != "01" {
            let revision = task["revision"]
                .as_str()
                .ok_or_else(|| invalid("revision missing"))?
                .parse::<u64>()
                .map_err(invalid)?;
            result.insert("0:3".into(),json!({"profile":task["profile"],"expected_profile_revision":revision,"generation_bytes":task["generation_bytes"]}));
        }
        Ok(result)
    } else {
        Ok(BTreeMap::from([("0:2".into(), task.clone())]))
    }
}
async fn stored(tx: &Transaction<'_>, fixture: &Fixture) -> Result<Value, RebornBuildError> {
    let row=tx.query_one("SELECT generation_bytes,profile_revision,generation_kind,created_at FROM reborn_prefix_generations WHERE profile_name='base-prompt' AND generation_id=$1", &[&fixture.generation]).await.map_err(invalid)?;
    check(
        row.get::<_, String>(0) == fixture.package
            && u64::try_from(row.get::<_, i64>(1)).map_err(invalid)? == fixture.revision
            && row.get::<_, String>(2) == "compiler-package",
        "archived generation differs from submitted originals",
    )?;
    Ok(
        json!({"profile":"base-prompt","generation":fixture.generation,"profile_revision":fixture.revision,"kind":"compiler-package","qualification":"quarantined",
        "created_at":row.get::<_,chrono::DateTime<chrono::Utc>>(3).to_rfc3339()}),
    )
}
async fn malformed_absent(tx: &Transaction<'_>) -> Result<String, RebornBuildError> {
    let generation = hex::encode(Sha256::digest(b"{not-a-package}"));
    check(
        tx.query_opt(
            "SELECT 1 FROM reborn_prefix_generations WHERE generation_id=$1",
            &[&generation],
        )
        .await
        .map_err(invalid)?
        .is_none(),
        "malformed generation was retained",
    )?;
    Ok(generation)
}

async fn serving(tx: &Transaction<'_>) -> Result<Value, RebornBuildError> {
    let active = tx
        .query_opt(
            "SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton",
            &[],
        )
        .await
        .map_err(invalid)?
        .map(|row| row.get::<_, Uuid>(0));
    let legacy:Value=tx.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb) FROM reborn_basic_prompt_store s",&[]).await.map_err(invalid)?.get(0);
    Ok(json!({"active_catalogue":active,"legacy_checksum":hash(&legacy)?}))
}

pub(super) async fn execute(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    inspected: Arc<InspectedRetainedProgram>,
    worker: &Path,
    fixture: &Fixture,
    retain: bool,
) -> Result<Value, RebornBuildError> {
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("generation Tool program missing"));
    };
    let limits = inspected.preparation_limits();
    let adapter = brassclaw_host_api::DEFAULT_MONTY_ADAPTER_RESERVE_BYTES as usize;
    let frame = serde_json::to_vec(&json!(fixture.package))
        .map_err(invalid)?
        .len()
        .checked_add(1024 * 1024)
        .ok_or_else(|| invalid("baseline frame overflow"))?;
    check(
        frame <= limits.process.max_frame_bytes,
        "baseline exceeds qualified transport",
    )?;
    let mut process = limits.process;
    process.max_frame_bytes = frame;
    let reserve = frame
        .checked_mul(2)
        .and_then(|n| n.checked_add(adapter))
        .ok_or_else(|| invalid("baseline reserve overflow"))?;
    let heap = process
        .hard_memory_bytes
        .checked_sub(reserve)
        .filter(|n| *n > frame)
        .ok_or_else(|| invalid("baseline exceeds physical validation capacity"))?;
    let mut bounds = limits.bounds;
    bounds.max_value_bytes = frame;
    check(
        bounds.max_source_bytes <= frame && bounds.max_stdout_bytes <= frame,
        "validation bounds exceed frame",
    )?;
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client.transaction().await.map_err(invalid)?;
    let before = serving(&tx).await?;
    malformed_absent(&tx).await?;
    tx.commit().await.map_err(invalid)?;
    drop(client);
    let inventory = case_inventory(retain);
    let mut observed = Vec::new();
    for case in inventory
        .as_array()
        .ok_or_else(|| invalid("generation cases missing"))?
    {
        let supplied = inputs(case, fixture, retain);
        let task_inputs = program
            .inputs()
            .bind_task_inputs(&supplied)
            .map_err(invalid)?;
        let admitted = Box::pin(bootstrap_validation::admit(
            pool.clone(),
            "bootstrap-prefix-generation",
            "Contained packaged generation qualification",
        ))
        .await?;
        admitted
            .admission
            .retain_recipe_selection(program.inputs().instruction())
            .await
            .map_err(invalid)?;
        let tools = crate::installed_monty_catalogue::bootstrap_generation_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            case["name"] == "denied",
        )?;
        let boot = RootBoot {
            source: crate::bootstrap_reply_validation::ROOT.into(),
            checksum: Sha256::digest(crate::bootstrap_reply_validation::ROOT.as_bytes()).into(),
            aliases: ["await_next_task", "enter_task", "validation_hold"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
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
        };
        let actor = ActorLimits {
            max_unclaimed: 8,
            max_reserved_frame_bytes: frame * 2,
            max_control_unclaimed: 8,
            max_control_reserved_frame_bytes: frame * 2,
        };
        let (mut owner, transport, root, task) =
            bootstrap_validation::start(worker, boot, process, actor, admitted.input.clone())
                .await?;
        let mut execution =
            RetainedRecipeExecution::new_for_behavioral_validation(task, inspected.clone())
                .map_err(invalid)?;
        let prepared = step_inputs(&task_inputs, retain)?;
        let mut last = None;
        let mut results = BTreeMap::<String, Value>::new();
        for (step, input) in &prepared {
            let mut supplied = input.clone();
            if step == "0:3" {
                supplied["expected_profile_revision"] = results
                    .get("0:1")
                    .ok_or_else(|| invalid("actual preparation result missing"))?
                    .clone();
            }
            let input = program
                .inputs()
                .bind_step_inputs(step, &supplied)
                .map_err(invalid)?;
            let result = execution
                .run_step(
                    &transport,
                    step,
                    &input,
                    if step == "0:1" {
                        None
                    } else {
                        Some(tools.as_ref())
                    },
                )
                .await;
            let failed = result.is_err();
            if let Ok(value) = &result {
                results.insert(step.clone(), value.clone());
            }
            last = Some(result);
            if failed {
                break;
            }
        }
        let result = last.ok_or_else(|| invalid("generation has no executable step"))?;
        let success = case["outcome"] == "return";
        check(
            result.is_ok() == success,
            "generation behavior outcome differs",
        )?;
        check(
            execution.host_answers().len()
                == case["calls"]
                    .as_u64()
                    .ok_or_else(|| invalid("case count missing"))? as usize,
            "generation dispatch count differs",
        )?;
        let mut steps = serde_json::Map::new();
        for (step, actual) in execution.observations() {
            check(
                actual.settled() && actual.observation_error().is_none(),
                "generation observation unsettled",
            )?;
            steps.insert(step.clone(),json!({"inputs":hex::encode(actual.input_checksum()),"arguments":actual.arguments_checksum().map(hex::encode),
                "answer":actual.answer_checksum().map(hex::encode),"result":actual.result_checksum().map(hex::encode),"failure":actual.failure().map(RetainedStepFailure::reason_kind)}));
        }
        let mut rejection = Value::Null;
        if success {
            check(
                execution.is_complete() && execution.transport_failure().is_none(),
                "generation success is incomplete",
            )?;
            let mut client = pool.get().await.map_err(invalid)?;
            let tx = client.transaction().await.map_err(invalid)?;
            let receipt = stored(&tx, fixture).await?;
            tx.commit().await.map_err(invalid)?;
            let expected = if retain {
                receipt
            } else {
                json!({"summary":receipt,"generation_bytes":fixture.package})
            };
            check(
                result.map_err(invalid)? == expected,
                "generation returned bytes/receipt differ from SQL",
            )?;
        } else {
            check(
                !execution.is_complete() && execution.failed_step_id().is_some(),
                "generation negative case completed",
            )?;
            let Err(RetainedExecutionError::Process(error)) = &result else {
                return Err(invalid("generation rejection lacks actual child failure"));
            };
            let Some(RetainedTransportEvidence::Process {
                error: retained,
                command,
                ..
            }) = execution.transport_failure()
            else {
                return Err(invalid("generation child failure evidence missing"));
            };
            check(
                Arc::ptr_eq(error, retained)
                    && retained.kind == ProcessFailure::Vm(VmFailure::Python)
                    && retained.snapshot.as_ref().is_some_and(|snapshot| {
                        snapshot.lifecycle == Lifecycle::Ready && snapshot.root == root.root
                    })
                    && command
                        .as_ref()
                        .is_ok_and(|command| match command.as_ref() {
                            WorkerCommand::Recipe {
                                command: RecipeCommand::Start { .. },
                            } => case["calls"] == 0,
                            WorkerCommand::Recipe {
                                command: RecipeCommand::ResumeHost { .. },
                            } => case["calls"] == 1,
                            _ => false,
                        }),
                "generation rejection has a different runtime failure",
            )?;
            rejection = json!({"kind":"python","root":root.root,"lifecycle":"ready","command":if case["calls"] == 0 {"start"} else {"resume-host"}});
            if case["calls"] == 1 {
                let Some((_, PortAnswer::TerminalError { reason_kind })) =
                    execution.host_answers().first()
                else {
                    return Err(invalid("generation terminal answer missing"));
                };
                check(
                    Some(reason_kind.as_str()) == case["outcome"].as_str(),
                    "generation failure classification differs",
                )?;
            }
        }
        check(
            admitted.host.finalized_reply_ref().is_none(),
            "storage qualification published a reply",
        )?;
        let mut client = pool.get().await.map_err(invalid)?;
        let tx = client.transaction().await.map_err(invalid)?;
        let rejected_generation = malformed_absent(&tx).await?;
        tx.commit().await.map_err(invalid)?;
        let record = json!({"rejection":rejection,"rejected_generation":rejected_generation,"case":case,"run_id":admitted.attempt.run_id.as_uuid(),"thread_id":admitted.host.run_context().thread_id,
            "worker_root":root.root,"task_inputs":hash(&task_inputs)?,"order":execution.observation_order(),"steps":steps});
        bootstrap_validation::close(&mut owner, &admitted).await?;
        observed.push(record);
    }
    let mut client = pool.get().await.map_err(invalid)?;
    let tx = client.transaction().await.map_err(invalid)?;
    let after = serving(&tx).await?;
    stored(&tx, fixture).await?;
    tx.commit().await.map_err(invalid)?;
    check(
        before == after,
        "generation qualification changed serving selection",
    )?;
    Ok(json!({"cases":observed,"before":before,"after":after}))
}

pub(super) async fn verify(
    tx: &Transaction<'_>,
    inspected: &InspectedRetainedProgram,
    fixture: &Fixture,
    retain: bool,
    observed: &Value,
) -> Result<(), RebornBuildError> {
    check(
        observed.as_object().is_some_and(|o| o.len() == 3)
            && observed["before"] == observed["after"],
        "generation serving evidence differs",
    )?;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("generation program missing"));
    };
    let inventory = case_inventory(retain);
    let wanted = inventory
        .as_array()
        .ok_or_else(|| invalid("case inventory missing"))?;
    let actual = observed["cases"]
        .as_array()
        .filter(|a| a.len() == wanted.len())
        .ok_or_else(|| invalid("generation case coverage differs"))?;
    let receipt = stored(tx, fixture).await?;
    let rejected_generation = malformed_absent(tx).await?;
    let selected = program
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let mut runs = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut threads = BTreeSet::new();
    for (record, case) in actual.iter().zip(wanted) {
        let run: Uuid = serde_json::from_value(record["run_id"].clone()).map_err(invalid)?;
        let thread = record["thread_id"]
            .as_str()
            .ok_or_else(|| invalid("generation thread missing"))?;
        check(
            record.as_object().is_some_and(|o| o.len() == 9)
                && record["rejected_generation"] == rejected_generation
                && record["case"] == *case
                && !run.is_nil()
                && runs.insert(run)
                && thread.starts_with("bootstrap-prefix-generation-")
                && threads.insert(thread)
                && !record["worker_root"].is_null()
                && roots.insert(record["worker_root"].to_string()),
            "generation case identity differs",
        )?;
        check(
            record["rejection"]
                == if case["outcome"] == "return" {
                    Value::Null
                } else {
                    json!({"kind":"python","root":record["worker_root"],"lifecycle":"ready","command":if case["calls"] == 0 {"start"} else {"resume-host"}})
                },
            "generation rejection evidence differs",
        )?;
        let task = inputs(case, fixture, retain);
        check(
            record["task_inputs"] == hash(&task)?,
            "generation task input evidence differs",
        )?;
        let prepared = step_inputs(&task, retain)?;
        let order: Vec<_> = prepared.keys().cloned().collect();
        check(
            record["order"] == json!(order)
                && record["steps"]
                    .as_object()
                    .is_some_and(|s| s.len() == order.len()),
            "generation executable step order differs",
        )?;
        let row=tx.query_one("SELECT s.selection_bytes,s.selection_checksum,a.phase,a.outcome,a.scope FROM brassclaw_monty_recipe_selections s JOIN brassclaw_monty_task_admissions a USING(run_id) WHERE s.run_id=$1 AND s.recipe_id=$2", &[&run,&program.inputs().instruction().recipe().uuid]).await.map_err(invalid)?;
        let outcome: Option<Value> = row.get(3);
        let scope: Value = row.get(4);
        check(
            row.get::<_, String>(0) == selected.exact_bytes()
                && row.get::<_, String>(1) == hex::encode(selected.checksum())
                && row.get::<_, String>(2) == "settled"
                && outcome
                    == Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
                && scope["tenant_id"] == "bootstrap-validation"
                && scope["agent_id"] == "bootstrap-prefix-generation"
                && scope["thread_id"] == thread,
            "generation durable selection/settlement differs",
        )?;
        let rows=tx.query("SELECT step_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count,recipe_id FROM brassclaw_monty_tool_invocations WHERE run_id=$1", &[&run]).await.map_err(invalid)?;
        check(
            rows.len()
                == case["calls"]
                    .as_u64()
                    .ok_or_else(|| invalid("case count missing"))? as usize,
            "generation durable invocation count differs",
        )?;
        for (step, input) in &prepared {
            let proof = &record["steps"][step];
            check(
                proof.as_object().is_some_and(|p| p.len() == 5) && proof["inputs"] == hash(input)?,
                "generation typed input evidence differs",
            )?;
            if step == "0:1" {
                let failed = case["outcome"] == "pure_failure";
                let expected = if failed {
                    Value::Null
                } else {
                    json!(
                        input["revision"]
                            .as_str()
                            .ok_or_else(|| invalid("revision absent"))?
                            .parse::<u64>()
                            .map_err(invalid)?
                    )
                };
                check(
                    proof["arguments"].is_null()
                        && proof["answer"].is_null()
                        && proof["result"]
                            == if failed {
                                Value::Null
                            } else {
                                json!(hash(&expected)?)
                            }
                        && proof["failure"]
                            == if failed {
                                json!(RetainedStepFailure::Process.reason_kind())
                            } else {
                                Value::Null
                            },
                    "generation pure preparation evidence differs",
                )?;
            }
        }
        for row in rows {
            check(
                row.get::<_, Uuid>(7) == program.inputs().instruction().recipe().uuid,
                "generation dispatched another Recipe",
            )?;
            let step = row.get::<_, String>(0);
            let expected_step = if retain { "0:3" } else { "0:2" };
            check(step == expected_step, "generation dispatched another step")?;
            let mut arguments = prepared[&step].clone();
            arguments
                .as_object_mut()
                .ok_or_else(|| invalid("generation arguments missing"))?
                .insert(
                    "operation".into(),
                    json!(if retain { "retain" } else { "read" }),
                );
            let invocation =
                crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, &step)
                    .map_err(invalid)?;
            check(
                row.get::<_, String>(1) == invocation
                    && row.get::<_, String>(2) == crate::bootstrap_evidence::checksum(&invocation)
                    && brassclaw_skills::value_contract::strict_json(
                        &row.get::<_, String>(3),
                        brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
                    )
                    .map_err(invalid)?
                        == arguments
                    && row.get::<_, String>(5) == "answered"
                    && row.get::<_, i16>(6) == 1,
                "generation durable arguments/attempt differ",
            )?;
            let expected = if case["outcome"] == "return" {
                PortAnswer::Return {
                    value: (if retain {
                        receipt.clone()
                    } else {
                        json!({"summary":receipt,"generation_bytes":fixture.package})
                    })
                    .into(),
                }
            } else {
                PortAnswer::TerminalError {
                    reason_kind: case["outcome"]
                        .as_str()
                        .ok_or_else(|| invalid("outcome absent"))?
                        .into(),
                }
            };
            let answer_bytes = row
                .get::<_, Option<String>>(4)
                .ok_or_else(|| invalid("generation answer unresolved"))?;
            let answer: PortAnswer = serde_json::from_str(&answer_bytes).map_err(invalid)?;
            let proof = &record["steps"][&step];
            let result = if let PortAnswer::Return { value } = &expected {
                json!(hash(value)?)
            } else {
                Value::Null
            };
            check(
                port_answer_checksum(&answer).map_err(invalid)?
                    == port_answer_checksum(&expected).map_err(invalid)?
                    && proof["arguments"] == hash(&arguments)?
                    && proof["answer"]
                        == hex::encode(port_answer_checksum(&expected).map_err(invalid)?)
                    && proof["result"] == result
                    && proof["failure"]
                        == if case["outcome"] == "return" {
                            Value::Null
                        } else {
                            json!(RetainedStepFailure::Process.reason_kind())
                        },
                "generation durable answer/result evidence differs",
            )?;
        }
    }
    Ok(())
}
