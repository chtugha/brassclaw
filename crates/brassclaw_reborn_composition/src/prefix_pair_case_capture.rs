//! Source-owned complete pair cases and integrity checks over their real native
//! journal. Python owns case preparation/comparison and all match decisions.
//! This module confers no workflow approval, admission or model-design acceptance.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_engine::{
    executor::retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, port_answer_checksum_with_limits,
        typed_value_checksum_with_limits,
    },
    executor::retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::process::PortAnswer;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::PrefixPairCasePacket;
use brassclaw_skills::value_contract::{ContractLimits, strict_json};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio_postgres::Transaction;
use uuid::Uuid;

fn check(value: bool, reason: &'static str) -> Result<(), RebornBuildError> {
    if value {
        Ok(())
    } else {
        // Static labels only: never emit DB, provider or input-derived text.
        #[cfg(test)]
        eprintln!("private Pair custody check failed: {reason}");
        Err(invalid(reason))
    }
}
fn read(value: &str, limits: ContractLimits) -> Result<Value, RebornBuildError> {
    strict_json(value, limits).map_err(invalid)
}
fn text(value: &Value) -> Result<&str, RebornBuildError> {
    value
        .as_str()
        .ok_or_else(|| invalid("pair case literal missing"))
}

struct PreparationEvidence<'a> {
    worker_checksum: &'a str,
    expected_revision: Option<u64>,
}

/// Constructed only by re-reading the retained source owner. A submitted JSON
/// packet is never sufficient provenance. No profile/role or token-limit copy.
pub(super) struct CapturedPairCases {
    packet: PrefixPairCasePacket,
}
impl CapturedPairCases {
    pub(super) async fn capture(
        pool: Arc<PgPool>,
        namespace: &str,
        specification: &str,
        proposal: &str,
    ) -> Result<Self, RebornBuildError> {
        let packet = crate::prefix_model_designs::PgPrefixModelDesigns::new(pool, namespace.into())
            .case_packet(specification, proposal)
            .await
            .map_err(invalid)?;
        Ok(Self { packet })
    }
    pub(super) fn input(&self) -> Value {
        json!({"case_packet_bytes":self.packet.case_packet_bytes})
    }

    /// Compare custody and data handoffs, not case semantics. The selected pure
    /// result remains authoritative for match/stability decisions. The caller
    /// must separately establish exact workflow qualification and run claims.
    #[cfg(test)]
    pub(super) async fn verify_receipt(
        &self,
        tx: &Transaction<'_>,
        run: Uuid,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
        receipt: &str,
        preparation: &crate::source_inspection::PreparedUtility,
    ) -> Result<(), RebornBuildError> {
        let worker_checksum = hex::encode(preparation.worker.checksum());
        self.verify_captured_receipt(
            tx,
            run,
            selected,
            execution,
            receipt,
            PreparationEvidence {
                worker_checksum: &worker_checksum,
                expected_revision: Some(preparation.revision),
            },
        )
        .await
    }

    /// Native answers below are verified against the durable invocation and
    /// exact selected Tool before their reported preparation revision is used.
    /// That revision belongs to the operation's acknowledged snapshot. Reading
    /// mutable current settings here could reject an otherwise valid completed
    /// operation after a concurrent live edit, or attest the wrong revision.
    pub(crate) async fn verify_job_receipt(
        &self,
        pool: &PgPool,
        run: Uuid,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
        receipt: &str,
        worker_checksum: &str,
    ) -> Result<(), RebornBuildError> {
        let mut client = pool.get().await.map_err(invalid)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        self.verify_captured_receipt(
            &tx,
            run,
            selected,
            execution,
            receipt,
            PreparationEvidence {
                worker_checksum,
                expected_revision: None,
            },
        )
        .await?;
        tx.commit().await.map_err(invalid)?;
        Ok(())
    }

    async fn verify_captured_receipt(
        &self,
        tx: &Transaction<'_>,
        run: Uuid,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
        receipt: &str,
        preparation: PreparationEvidence<'_>,
    ) -> Result<(), RebornBuildError> {
        let worker_checksum = preparation.worker_checksum;
        let RetainedProgram::Tools(program) = selected.program() else {
            return Err(invalid("pair case Tool program required"));
        };
        let order = ["0:1", "0:3", "0:5", "0:6"];
        check(
            !run.is_nil()
                && execution.is_complete()
                && execution.transport_failure().is_none()
                && execution.observations().len() == order.len()
                && execution
                    .observation_order()
                    .is_some_and(|actual| actual.iter().map(String::as_str).eq(order))
                && execution.host_answers().len() == 2,
            "pair case completion/order differs",
        )?;
        let packet = read(
            &self.packet.case_packet_bytes,
            execution.observations()["0:1"].data_limits(),
        )?;
        let cases = packet["cases"]
            .as_array()
            .filter(|cases| !cases.is_empty())
            .ok_or_else(|| invalid("complete source cases missing"))?;
        // Reconstruct only the declared physical inputs for integrity matching.
        // No template rendering, selection, fit or match decision occurs here.
        let mut texts = vec![json!(self.packet.case_packet_bytes)];
        let mut probes = Vec::new();
        for case in cases {
            texts.extend([
                case["expected_render"].clone(),
                case["context_json"].clone(),
                case["expected_prefix"].clone(),
            ]);
            probes.push(json!({"kind":"leading_prefix_chat","context_json":case["context_json"],"leading_prefix":case["expected_prefix"]}));
            probes.push(json!({"kind":"text","text":case["expected_render"]}));
        }
        let probes = crate::prefix_generations::canonical(&json!(probes)).map_err(invalid)?;
        let texts = json!(texts);
        let digest_arguments = json!({"operation":"sha256_utf8","data":serde_json::to_string(&texts).map_err(invalid)?});
        let encoding_arguments = json!({"specification":self.packet.specification,"artifact_set":self.packet.artifact_set,"probes_json":probes});
        let rows=tx.query("SELECT step_id,recipe_id,selection_bytes,selection_checksum,arguments_bytes,answer_bytes,phase,attempt_count FROM brassclaw_monty_tool_invocations WHERE run_id=$1 ORDER BY step_id",&[&run]).await.map_err(invalid)?;
        check(rows.len() == 2, "pair case native journal incomplete")?;
        let mut outputs = Vec::new();
        for ((row, (step, answer)), (expected_step, arguments)) in
            rows.into_iter().zip(execution.host_answers()).zip([
                ("0:3", digest_arguments),
                ("0:5", encoding_arguments.clone()),
            ])
        {
            check(step == expected_step, "pair case native step differs")?;
            let proof = &execution.observations()[step];
            let limits = proof.data_limits();
            let selection =
                crate::pg_monty_admission::PgMontyAdmission::invocation_selection(program, step)
                    .map_err(invalid)?;
            check(
                row.get::<_, String>(0) == *step
                    && row.get::<_, Uuid>(1) == program.inputs().instruction().recipe().uuid
                    && row.get::<_, String>(2) == selection
                    && row.get::<_, String>(3) == crate::bootstrap_evidence::checksum(&selection)
                    && row.get::<_, String>(6) == "answered"
                    && row.get::<_, i16>(7) == 1
                    && read(&row.get::<_, String>(4), limits)? == arguments,
                "pair case native identity/arguments/attempt differs",
            )?;
            let recorded: PortAnswer = serde_json::from_str(
                row.get::<_, Option<String>>(5)
                    .as_deref()
                    .ok_or_else(|| invalid("pair case native answer unresolved"))?,
            )
            .map_err(invalid)?;
            let checksum = port_answer_checksum_with_limits(answer, limits).map_err(invalid)?;
            check(
                port_answer_checksum_with_limits(&recorded, limits).map_err(invalid)? == checksum
                    && proof.answer_checksum() == Some(checksum)
                    && proof.arguments_checksum()
                        == Some(
                            typed_value_checksum_with_limits(&arguments, limits)
                                .map_err(invalid)?,
                        ),
                "pair case native answer custody differs",
            )?;
            let PortAnswer::Return { value } = answer else {
                return Err(invalid("pair case native success required"));
            };
            outputs.push(value.as_value().clone());
        }
        let digests = json!({"digests":texts.as_array().ok_or_else(||invalid("pair case texts missing"))?.iter()
            .map(|value|text(value).map(|value|brassclaw_model_encoding::checksum(value.as_bytes())))
            .collect::<Result<Vec<_>,_>>()?});
        check(outputs[0] == digests, "pair case native digest differs")?;
        let observed = read(
            text(&outputs[1])?,
            execution.observations()["0:5"].data_limits(),
        )?;
        let preparation_revision = observed["preparation_revision"]
            .as_u64()
            .filter(|revision| *revision > 0)
            .ok_or_else(|| invalid("pair operation acknowledgement missing"))?;
        check(
            preparation
                .expected_revision
                .is_none_or(|expected| expected == preparation_revision),
            "pair operation acknowledgement differs",
        )?;
        check(
            observed["specification"] == self.packet.specification
                && observed["artifact_set"] == self.packet.artifact_set
                && observed["template_origin"] == "model-artifact-template"
                && observed["model_chat_template_sha256"] == packet["template_sha256"]
                && observed["report"]["chat_template_sha256"] == packet["template_sha256"]
                && observed["worker_checksum"] == worker_checksum
                && observed["preparation_revision"] == preparation_revision,
            "pair case native encoding source/preparation differs",
        )?;
        let prepared = json!({"bank_bytes":self.packet.case_packet_bytes,"texts":texts,"probes_json":probes,"specification":self.packet.specification,"artifact_set":self.packet.artifact_set});
        let inputs = [
            self.input(),
            json!({"texts":texts}),
            encoding_arguments,
            json!({"bank_bytes":self.packet.case_packet_bytes,"digests":outputs[0]["digests"],"observation_bytes":outputs[1]}),
        ];
        let results = [
            prepared,
            outputs[0].clone(),
            outputs[1].clone(),
            json!(receipt),
        ];
        for ((step, inputs), result) in order.into_iter().zip(inputs).zip(results) {
            let proof = &execution.observations()[step];
            let limits = proof.data_limits();
            check(
                proof.settled()
                    && proof.observation_error().is_none()
                    && proof.failure().is_none()
                    && proof.input_checksum()
                        == typed_value_checksum_with_limits(&inputs, limits).map_err(invalid)?
                    && proof.result_checksum()
                        == Some(
                            typed_value_checksum_with_limits(&result, limits).map_err(invalid)?,
                        )
                    && (matches!(step, "0:3" | "0:5")
                        || (proof.arguments_checksum().is_none()
                            && proof.answer_checksum().is_none())),
                "pair case exact step data handoff differs",
            )?;
        }
        let result = read(receipt, execution.observations()["0:6"].data_limits())?;
        check(
            crate::prefix_generations::canonical(&result).map_err(invalid)? == receipt
                && result["format"] == "brassclaw-prefix-pair-case-comparison/1"
                && result["case_packet_sha256"]
                    == brassclaw_model_encoding::checksum(self.packet.case_packet_bytes.as_bytes())
                && result["proposal"] == self.packet.proposal
                && result["specification"] == self.packet.specification
                && result["artifact_set"] == self.packet.artifact_set
                && result["template_sha256"] == packet["template_sha256"]
                && result["worker_checksum"] == worker_checksum
                && result["preparation_revision"] == preparation_revision
                && result["qualification"] == "retained-template-cases-only"
                && result["provider_wire_qualified"] == false
                && result.get("context_fit").is_some_and(Value::is_null)
                && result["cache_quality"] == "unmeasured"
                && result["model_quality"] == "unmeasured",
            "pair case receipt source/scope differs",
        )
    }
}

pub(crate) async fn select(
    catalogue: &crate::installed_monty_catalogue::InstalledMontyCatalogue,
    host: Arc<brassclaw_reborn::monty_task_host::MontyTaskHost>,
    admission: Arc<crate::pg_monty_admission::PgMontyAdmission>,
    definition: crate::prefix_job_definitions::Definition,
    reference: String,
) -> Result<
    Arc<dyn crate::global_recipe_ports::InternalRecipeSource>,
    brassclaw_monty_host::service::PortFailure,
> {
    let bad = || {
        brassclaw_monty_host::service::PortFailure::new("prefix_pair_case_selection_invalid")
            .expect("constant classification")
    };
    let crate::prefix_job_definitions::Target::Pair {
        specification,
        proposal,
    } = &definition.target
    else {
        return Err(bad());
    };
    if definition.operation != "check_cases"
        || definition.mode != "target"
        || definition.target.pair_request().as_ref() != Some(&definition.request)
    {
        return Err(bad());
    }
    let source = Arc::new(
        CapturedPairCases::capture(
            catalogue.pool.clone(),
            definition.origin.tenant_id().as_str(),
            specification,
            proposal,
        )
        .await
        .map_err(|_| bad())?,
    );
    if source.input() != definition.input {
        return Err(bad());
    }
    let qualification = catalogue.pair_case_qualification.as_ref().ok_or_else(bad)?;
    let inspected = qualification.program();
    if inspected
        .program()
        .inputs()
        .instruction()
        .retained_selection()
        .map_err(|_| bad())?
        .exact_bytes()
        != definition.program_bytes
    {
        return Err(bad());
    }
    let mut client = catalogue.pool.get().await.map_err(|_| bad())?;
    let tx = client
        .build_transaction()
        .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(|_| bad())?;
    qualification
        .verify_in_transaction(&tx)
        .await
        .map_err(|_| bad())?;
    tx.commit().await.map_err(|_| bad())?;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(bad());
    };
    let program = program.clone();
    let binding_pool = catalogue.pool.clone();
    let binding_kernel = catalogue.kernel.clone();
    let bind_tools = Box::new(move || {
        crate::installed_monty_catalogue::bootstrap_pair_tools(
            binding_pool.clone(),
            binding_kernel.clone(),
            host.clone(),
            admission.clone(),
            program.clone(),
            false,
        )
    });
    Ok(Arc::new(crate::prefix_job_execution::Invocation {
        receipt_contract: crate::prefix_job_execution::ReceiptContract::PairCases {
            source,
            worker_checksum: hex::encode(catalogue.worker_image.checksum()),
        },
        definition,
        reference,
        inspected,
        tools: Default::default(),
        bind_tools,
        pool: catalogue.pool.clone(),
    }))
}
