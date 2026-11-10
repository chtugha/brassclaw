//! Normal three-step private ResultOnly consumer. The candidate invocation is
//! the one native operation; Rust never enumerates or selects candidate cases.
use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::executor::{
    retained_recipe::{
        RetainedProgram, RetainedRecipeExecution, RetainedToolPort,
        port_answer_checksum_with_limits, typed_value_checksum_with_limits,
    },
    retained_source::{InspectedPureSelection, InspectedRetainedProgram},
};
use brassclaw_monty_host::{process::PortAnswer, service::PortFailure};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use serde_json::json;

use crate::{
    RebornBuildError,
    bootstrap_reply_validation::invalid,
    global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe},
    installed_monty_catalogue::PreparationCapture,
    monty_kernel::MontyKernelSnapshot,
    pg_monty_admission::PgMontyAdmission,
    pure_case_evidence::EvidenceStore,
    pure_case_execution::AuthoredCaseOperation,
    pure_case_original::CheckedOriginalPureCase,
    pure_case_qualification::QualifiedCaseHarness,
};

pub(crate) struct CaseSource {
    reference: String,
    inspected: Arc<InspectedRetainedProgram>,
    operation: Arc<AuthoredCaseOperation>,
    tools: Arc<dyn RetainedToolPort>,
    receipt_failure: PortFailure,
}
impl CaseSource {
    pub(crate) async fn new(
        case: Arc<CheckedOriginalPureCase>,
        qualified: &QualifiedCaseHarness,
        worker: &brassclaw_host_runtime::PackagedExecutableImage,
        preparation: &PreparationCapture,
        kernel: Arc<dyn MontyKernelSnapshot>,
        host: Arc<MontyTaskHost>,
        admission: Arc<PgMontyAdmission>,
        evidence: Arc<EvidenceStore>,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let receipt_failure =
            PortFailure::new("original_pure_case_receipt_invalid").map_err(invalid)?;
        if case.worker_checksum() != worker.checksum() {
            return Err(invalid("original Pure case worker differs"));
        }
        let worker = crate::pg_review_admission_source::verified_worker(
            worker,
            preparation.limits().max_native_artifact_bytes,
        )
        .await?;
        let original = Arc::new(
            InspectedPureSelection::inspect(
                case.snapshot_ref(),
                case.candidate().uuid,
                &worker.path(),
                preparation.limits(),
            )
            .await
            .map_err(invalid)?,
        );
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(qualified.program()),
                &worker.path(),
                preparation.limits(),
            )
            .await
            .map_err(invalid)?,
        );
        let operation =
            AuthoredCaseOperation::new(case.clone(), original, evidence.clone(), host.clone())?;
        let tools = crate::installed_monty_catalogue::original_pure_case_tools(
            evidence.pool.clone(),
            kernel,
            host,
            admission,
            qualified.program(),
            operation.clone(),
            false,
        )?;
        Ok(Arc::new(Self {
            reference: format!(
                "original-pure-case:{}:{}",
                case.id(),
                hex::encode(case.checksum())
            ),
            inspected,
            operation,
            tools,
            receipt_failure,
        }))
    }
}
#[async_trait]
impl InternalRecipeSource for CaseSource {
    fn name(&self) -> &str {
        "original-pure-case"
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        Some(self.inspected.clone())
    }
    fn requires_step_observations(&self) -> bool {
        true
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(json!({}).into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        let failed = || self.receipt_failure.clone();
        if !std::ptr::eq(selected, self.inspected.as_ref())
            || !execution.is_complete()
            || execution.transport_failure().is_some()
            || execution.completed_step_ids().collect::<Vec<_>>() != ["0:2", "0:3"]
            || execution.host_answers().len() != 1
            || execution.host_answers()[0].0 != "0:2"
        {
            return Err(failed());
        }
        let native = self.operation.observed_output().map_err(|_| failed())?;
        if !matches!(&execution.host_answers()[0].1,PortAnswer::Return {value} if value.as_value().as_str()==Some(native))
        {
            return Err(failed());
        }
        let call = execution.observations().get("0:2").ok_or_else(failed)?;
        let encode = execution.observations().get("0:3").ok_or_else(failed)?;
        let expected_answer = PortAnswer::Return {
            value: json!(native).into(),
        };
        if call.arguments_checksum()
            != Some(
                typed_value_checksum_with_limits(&json!({}), call.data_limits())
                    .map_err(|_| failed())?,
            )
            || call.answer_checksum()
                != Some(
                    port_answer_checksum_with_limits(&expected_answer, call.data_limits())
                        .map_err(|_| failed())?,
                )
            || encode.input_checksum()
                != typed_value_checksum_with_limits(
                    &json!({"case_outcome_json":native}),
                    encode.data_limits(),
                )
                .map_err(|_| failed())?
            || encode.result_checksum()
                != Some(
                    typed_value_checksum_with_limits(&json!(receipt), encode.data_limits())
                        .map_err(|_| failed())?,
                )
            || call.failure().is_some()
            || encode.failure().is_some()
        {
            return Err(failed());
        }
        // The separately qualified pure encoder owns its byte/data formatting.
        // These fingerprints prove the actual whole native input and returned
        // receipt passed through that exact selected code. Settlement later
        // checks the complete JSON relation against the original native answer.
        Ok(())
    }
}
