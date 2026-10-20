//! Closed native encoding qualification through the real Python Recipe flow.
//! Fixed data oracles check behavior; this module never accepts model quality,
//! cache behavior, a prefix design or caller-supplied approval claims.
use crate::{
    RebornBuildError,
    bootstrap_encoding_fixture::Fixture,
    bootstrap_recipe_flow,
    bootstrap_reply_validation::invalid,
    bootstrap_validation,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    monty_kernel::MontyKernelSnapshot,
};
use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, RetainedToolPort, port_answer_checksum,
        typed_value_checksum,
    },
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::{
    process::PortAnswer,
    service::{PortFailure, TaskOutcome},
};
use brassclaw_pg::PgPool;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path, sync::Arc};
use tokio_postgres::Transaction;
use uuid::Uuid;

fn check(ok: bool, reason: &str) -> Result<(), RebornBuildError> {
    if ok { Ok(()) } else { Err(invalid(reason)) }
}
pub(super) fn cases(effective: bool) -> Vec<&'static str> {
    let mut cases = vec![
        "original",
        "duplicate",
        "denied",
        "missing-assets",
        "invalid-hash",
        "invalid-probes",
        "bad-tokenizer",
    ];
    if effective {
        cases.push("bad-template");
        cases.push("render-fuel");
    }
    cases
}
fn rejection(case: &str) -> Option<&'static str> {
    match case {
        "original" | "duplicate" => None,
        "denied" => Some("retained_tool_authorization"),
        "missing-assets" => Some("retained_tool_operation_failed"),
        "render-fuel" => Some("retained_tool_resource"),
        _ => Some("retained_tool_invalid_input"),
    }
}
struct Invocation {
    reference: String,
    inputs: Value,
    inspected: Arc<InspectedRetainedProgram>,
    tools: Arc<dyn RetainedToolPort>,
    fixture: Arc<Fixture>,
    effective: bool,
}
#[async_trait]
impl InternalRecipeSource for Invocation {
    fn name(&self) -> &str {
        "packaged-model-encoding-qualification"
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
        let bad = || PortFailure::new("encoding_receipt_invalid").expect("constant classification");
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.transport_failure().is_some()
        {
            return Err(bad());
        }
        let [(_, PortAnswer::Return { value })] = execution.host_answers() else {
            return Err(bad());
        };
        if value.as_str() != Some(receipt) {
            return Err(bad());
        }
        self.fixture
            .verify(receipt, self.effective)
            .map_err(|_| bad())?;
        Ok(())
    }
}

pub(super) async fn execute(
    pool: Arc<PgPool>,
    kernel: Arc<dyn MontyKernelSnapshot>,
    inspected: Arc<InspectedRetainedProgram>,
    worker: &Path,
    fixture: Arc<Fixture>,
    effective: bool,
) -> Result<Value, RebornBuildError> {
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("encoding Tool program missing"));
    };
    let root = crate::global_root_seed::retain_packaged_global_root(
        pool.clone(),
        crate::builtin_bootstrap::GLOBAL_ORCHESTRATOR_SEED,
    )
    .await
    .map_err(invalid)?;
    let before = serving_checksum(&pool).await?;
    let mut reports = Vec::new();
    for case in cases(effective) {
        let inputs = fixture.inputs(case, effective);
        let reference = format!("closed-model-encoding:{case}:{}", Uuid::new_v4());
        let admitted = Box::pin(bootstrap_validation::admit(
            pool.clone(),
            "bootstrap-model-encoding",
            &reference,
        ))
        .await?;
        let tools = crate::installed_monty_catalogue::bootstrap_encoding_tools(
            pool.clone(),
            kernel.clone(),
            admitted.host.clone(),
            admitted.admission.clone(),
            program.clone(),
            case == "denied",
        )?;
        let observed = bootstrap_recipe_flow::execute(
            &admitted,
            bootstrap_recipe_flow::Request::contained(
                worker.to_owned(),
                inspected.preparation_limits(),
                &root,
                Arc::new(Invocation {
                    reference,
                    inputs: inputs.clone(),
                    inspected: inspected.clone(),
                    tools,
                    fixture: fixture.clone(),
                    effective,
                }),
            )?,
        )
        .await?;
        let success = rejection(case).is_none();
        check(
            matches!(
                observed.receipt.outcome,
                TaskOutcome::InternalCompleted { .. }
            ) == success,
            "encoding root outcome differs",
        )?;
        check(
            observed.report["root_completed"] == success
                && observed.report["model_exchange_count"] == 0
                && observed.report["reply"].is_null()
                && observed.report["semantic_approval"] == false,
            "encoding root evidence differs",
        )?;
        let fingerprints=observed.ports.observe_internal_execution(|execution|->Result<_,RebornBuildError> {
            check(execution.host_answers().len()==1 && execution.observations().len()==1 && execution.observation_order().is_some_and(|order| order == ["0:2"]),"encoding observation count differs")?;
            let proof=&execution.observations()["0:2"];
            check(proof.settled() && proof.observation_error().is_none(),"encoding observation unresolved")?;
            if success {
                check(execution.is_complete() && execution.transport_failure().is_none(),"encoding positive incomplete")?;
            } else {
                check(!execution.is_complete() && execution.failed_step_id()==Some("0:2") && execution.transport_failure().is_some(),"encoding negative completed")?;
                check(matches!(execution.host_answers(),[(_,PortAnswer::TerminalError {reason_kind})] if Some(reason_kind.as_str())==rejection(case)),"encoding failure category differs")?;
            }
            Ok((proof.arguments_checksum(),proof.answer_checksum(),proof.result_checksum(),proof.failure().map(|f|f.reason_kind().to_owned())))
        }).await.map_err(|error|invalid(format!("encoding observation: {error:?}")))??;
        check(
            admitted.host.finalized_reply_ref().is_none()
                && observed.exit.failure.is_none()
                && observed.exit.pending_admission.is_none()
                && observed.exit.queued_admissions.is_empty(),
            "encoding worker settlement incomplete",
        )?;
        let exit = observed
            .exit
            .transport
            .map_err(|error| invalid(format!("encoding containment: {error:?}")))?;
        check(
            exit.exit_status.is_some()
                && exit.containment_error.is_none()
                && exit.reap_error.is_none()
                && exit.shutdown_failure.is_none(),
            "encoding worker containment unresolved",
        )?;
        bootstrap_validation::settle_closed(&admitted).await?;
        let client = pool.get().await.map_err(invalid)?;
        let row=client.query_one("SELECT arguments_bytes,answer_bytes FROM brassclaw_monty_tool_invocations WHERE run_id=$1 AND step_id='0:2'",&[&admitted.attempt.run_id.as_uuid()]).await.map_err(invalid)?;
        let args: Value = serde_json::from_str(&row.get::<_, String>(0)).map_err(invalid)?;
        let answer: PortAnswer = serde_json::from_str(
            &row.get::<_, Option<String>>(1)
                .ok_or_else(|| invalid("encoding answer unresolved"))?,
        )
        .map_err(invalid)?;
        check(
            fingerprints.0 == Some(typed_value_checksum(&args).map_err(invalid)?)
                && fingerprints.1 == Some(port_answer_checksum(&answer).map_err(invalid)?),
            "encoding journal differs from native observation",
        )?;
        reports.push(json!({"case":case,"run_id":admitted.attempt.run_id.as_uuid(),"thread_id":admitted.host.run_context().thread_id,
            "root_vm":observed.receipt.root.vm_id(),"inputs_checksum":hex::encode(typed_value_checksum(&inputs).map_err(invalid)?),
            "arguments_checksum":fingerprints.0.map(hex::encode),"answer_checksum":fingerprints.1.map(hex::encode),
            "result_checksum":fingerprints.2.map(hex::encode),"failure":fingerprints.3,"execution":observed.report}));
    }
    let after = serving_checksum(&pool).await?;
    check(
        before == after,
        "encoding qualification changed serving selection",
    )?;
    Ok(
        json!({"format":"model-encoding-behavior-observations/1","effective_template":effective,"cases":reports,"cache_quality":"unmeasured","model_quality":"unmeasured","serving_before":before,"serving_after":after}),
    )
}

async fn serving_checksum(pool: &PgPool) -> Result<String, RebornBuildError> {
    let client = pool.get().await.map_err(invalid)?;
    let snapshot: String = client.query_one("SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb)::text FROM reborn_basic_prompt_store s", &[]).await.map_err(invalid)?.get(0);
    Ok(crate::bootstrap_evidence::checksum(&snapshot))
}

/// Reconstruct fixture inputs, oracle answers and exact durable dispatches.
/// Evidence rows alone are never sufficient to approve the combination.
pub(super) async fn verify(
    tx: &Transaction<'_>,
    inspected: &InspectedRetainedProgram,
    fixture: &Fixture,
    effective: bool,
    worker_checksum: &str,
    observed: &Value,
) -> Result<(), RebornBuildError> {
    crate::bootstrap_evidence::require_coherent(tx).await?;
    check(
        observed.as_object().is_some_and(|v| v.len() == 7)
            && observed["format"] == "model-encoding-behavior-observations/1"
            && observed["effective_template"] == effective
            && observed["cache_quality"] == "unmeasured"
            && observed["model_quality"] == "unmeasured",
        "encoding report contract differs",
    )?;
    check(
        observed["serving_before"]
            .as_str()
            .is_some_and(|s| s.len() == 64)
            && observed["serving_before"] == observed["serving_after"],
        "encoding serving evidence differs",
    )?;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("encoding program missing"));
    };
    let inventory = cases(effective);
    let actual = observed["cases"]
        .as_array()
        .filter(|v| v.len() == inventory.len())
        .ok_or_else(|| invalid("encoding cases incomplete"))?;
    let selected = program
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(invalid)?;
    let invocation =
        crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, "0:2")
            .map_err(invalid)?;
    let mut runs = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut threads = BTreeSet::new();
    let mut original = None;
    for (record, case) in actual.iter().zip(inventory) {
        let run: Uuid = serde_json::from_value(record["run_id"].clone()).map_err(invalid)?;
        let thread = record["thread_id"]
            .as_str()
            .ok_or_else(|| invalid("encoding thread missing"))?;
        let inputs = fixture.inputs(case, effective);
        let success = rejection(case).is_none();
        check(
            record.as_object().is_some_and(|v| v.len() == 10)
                && record["case"] == case
                && !run.is_nil()
                && runs.insert(run)
                && thread.starts_with("bootstrap-model-encoding-")
                && threads.insert(thread)
                && !record["root_vm"].is_null()
                && roots.insert(record["root_vm"].to_string())
                && record["inputs_checksum"]
                    == hex::encode(typed_value_checksum(&inputs).map_err(invalid)?),
            "encoding case identity differs",
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
                && scope["agent_id"] == "bootstrap-model-encoding"
                && scope["thread_id"] == thread,
            "encoding durable admission differs",
        )?;
        let rows=tx.query("SELECT step_id,recipe_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1",&[&run]).await.map_err(invalid)?;
        check(rows.len() == 1, "encoding invocation count differs")?;
        let row = &rows[0];
        let args: Value = serde_json::from_str(&row.get::<_, String>(4)).map_err(invalid)?;
        let answer: PortAnswer = serde_json::from_str(
            &row.get::<_, Option<String>>(5)
                .ok_or_else(|| invalid("encoding answer unresolved"))?,
        )
        .map_err(invalid)?;
        check(
            row.get::<_, String>(0) == "0:2"
                && row.get::<_, Uuid>(1) == program.inputs().instruction().recipe().uuid
                && row.get::<_, String>(2) == invocation
                && row.get::<_, String>(3) == crate::bootstrap_evidence::checksum(&invocation)
                && args == inputs
                && row.get::<_, String>(6) == "answered"
                && row.get::<_, i16>(7) == 1
                && record["arguments_checksum"]
                    == hex::encode(typed_value_checksum(&args).map_err(invalid)?)
                && record["answer_checksum"]
                    == hex::encode(port_answer_checksum(&answer).map_err(invalid)?),
            "encoding journal integrity differs",
        )?;
        if success {
            let PortAnswer::Return { value } = &answer else {
                return Err(invalid("encoding success lacks native result"));
            };
            let bytes = value
                .as_str()
                .ok_or_else(|| invalid("encoding result is not JSON bytes"))?;
            let result = fixture.verify(bytes, effective)?;
            check(
                result["worker_checksum"] == worker_checksum
                    && record["result_checksum"]
                        == hex::encode(typed_value_checksum(value).map_err(invalid)?)
                    && record["failure"].is_null()
                    && record["execution"]["internal_receipt"] == bytes,
                "encoding result differs from retained worker",
            )?;
            if case == "original" {
                original = Some(bytes.to_owned());
            } else {
                check(
                    original.as_deref() == Some(bytes),
                    "encoding duplicate is nondeterministic",
                )?;
            }
        } else {
            check(matches!(&answer,PortAnswer::TerminalError {reason_kind} if Some(reason_kind.as_str())==rejection(case)) && record["result_checksum"].is_null() && record["failure"]==brassclaw_engine::executor::retained_recipe::RetainedStepFailure::Process.reason_kind(),"encoding rejection differs")?;
        }
        check(
            record["execution"]["root_completed"] == success
                && record["execution"]["model_exchange_count"] == 0
                && record["execution"]["reply"].is_null()
                && record["execution"]["semantic_approval"] == false,
            "encoding root report differs",
        )?;
    }
    Ok(())
}
