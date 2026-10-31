//! Durable definition -> retained internal Recipe selection. Workflow decisions
//! remain in the packaged Python compiler and global Recipe runner.
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_engine::executor::{
    retained_recipe::{RetainedRecipeExecution, RetainedToolPort},
    retained_source::InspectedRetainedProgram,
};
use brassclaw_monty_host::{process::PortAnswer, service::PortFailure};
use brassclaw_pg::PgPool;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};

use crate::global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe};
use crate::prefix_job_definitions::Definition;

pub(crate) enum ReceiptContract {
    Generation,
    ModelSetup,
    Measurement,
    PairCases {
        source: Arc<crate::prefix_pair_case_capture::CapturedPairCases>,
        worker_checksum: String,
    },
}

pub(crate) type NativeRuntimeBinder =
    Box<dyn Fn() -> Result<Arc<dyn RetainedToolPort>, crate::RebornBuildError> + Send + Sync>;

pub(crate) struct Invocation {
    pub(crate) receipt_contract: ReceiptContract,
    pub(crate) definition: Definition,
    pub(crate) reference: String,
    pub(crate) inspected: Arc<InspectedRetainedProgram>,
    pub(crate) tools: OnceLock<Result<Arc<dyn RetainedToolPort>, ()>>,
    pub(crate) bind_tools: NativeRuntimeBinder,
    pub(crate) pool: Arc<PgPool>,
}
fn invalid() -> PortFailure {
    PortFailure::new("prefix_job_receipt_invalid").expect("constant classification")
}
/// Source metadata must describe the actual qualified compiler procedures.
/// References remain data; this check neither trusts their prose nor executes it.
pub(crate) fn validate_snapshot(
    profile: &str,
    bytes: &str,
    limits: brassclaw_skills::value_contract::ContractLimits,
) -> Result<(), PortFailure> {
    let snapshot =
        brassclaw_skills::value_contract::strict_json(bytes, limits).map_err(|_| invalid())?;
    let procedures = serde_json::json!({
        "compiler/entry.py":brassclaw_skills::prefix_compiler_components::COMPILER_SOURCE,
        "compiler/prefix_render.py":brassclaw_skills::prefix_reference_components::REFERENCE_RENDER_SOURCE,
        "compiler/prefix_units.py":brassclaw_skills::prefix_compiler_components::UNIT_SOURCE
    });
    if snapshot["profile"] != profile
        || snapshot["format"] != "brassclaw-prefix-source-snapshot/1"
        || snapshot["procedures"] != procedures
        || crate::prefix_generations::canonical(&snapshot).map_err(|_| invalid())? != bytes
    {
        return Err(invalid());
    }
    Ok(())
}

#[async_trait]
impl InternalRecipeSource for Invocation {
    fn captured_program(&self) -> Option<Arc<InspectedRetainedProgram>> {
        Some(self.inspected.clone())
    }
    fn bind_native_runtimes(&self) -> Result<(), PortFailure> {
        self.tools
            .get_or_init(|| (self.bind_tools)().map_err(|_| ()))
            .as_ref()
            .map(|_| ())
            .map_err(|_| invalid())
    }
    fn name(&self) -> &str {
        match &self.receipt_contract {
            ReceiptContract::Generation => "prefix-generation",
            ReceiptContract::ModelSetup => "provider-model-setup",
            ReceiptContract::Measurement => "prefix-measurement",
            ReceiptContract::PairCases { .. } => "prefix-pair-cases",
        }
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    fn requires_step_observations(&self) -> bool {
        matches!(
            &self.receipt_contract,
            ReceiptContract::PairCases { .. }
                | ReceiptContract::ModelSetup
                | ReceiptContract::Measurement
        )
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.definition.input.clone().into()),
            tools: Some(
                self.tools
                    .get()
                    .and_then(|result| result.as_ref().ok())
                    .ok_or_else(invalid)?
                    .clone(),
            ),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if !std::ptr::eq(selected, self.inspected.as_ref()) {
            return Err(invalid());
        }
        if matches!(&self.receipt_contract, ReceiptContract::ModelSetup) {
            if self.definition.operation != "bootstrap_setup" {
                return Err(invalid());
            }
            return crate::prefix_bootstrap_setup::verify_execution(
                &self.definition.input,
                receipt,
                execution,
            )
            .map(|_| ())
            .map_err(|_| invalid());
        }
        if let ReceiptContract::PairCases {
            source,
            worker_checksum,
        } = &self.receipt_contract
        {
            if self.definition.operation != "check_cases" {
                return Err(invalid());
            }
            return source
                .verify_job_receipt(
                    &self.pool,
                    self.definition.id,
                    selected,
                    execution,
                    receipt,
                    worker_checksum,
                )
                .await
                .map_err(|_| invalid());
        }
        if matches!(&self.receipt_contract, ReceiptContract::Measurement) {
            if self.definition.operation != "measure" {
                return Err(invalid());
            }
            return crate::prefix_measurement_capture::verify_receipt(
                &self.definition,
                receipt,
                selected,
                execution,
                &self.pool,
            )
            .await;
        }
        if self.definition.operation != "generate"
            || !execution.is_complete()
            || execution.transport_failure().is_some()
            || execution.host_answers().len() != 5
        {
            return Err(invalid());
        }
        let Some((_, PortAnswer::Return { value })) = execution.host_answers().last() else {
            return Err(invalid());
        };
        let generation = value["generation"].as_str().ok_or_else(invalid)?;
        let (profile, revision) = self.definition.target.profile().ok_or_else(invalid)?;
        if value["profile"] != profile
            || value["qualification"] != "quarantined"
            || receipt != format!("prefix-generation:{profile}:{generation}")
        {
            return Err(invalid());
        }
        let client = self.pool.get().await.map_err(|_| invalid())?;
        let row = client.query_opt("SELECT generation_bytes,profile_revision FROM reborn_prefix_generations WHERE profile_name=$1 AND generation_id=$2", &[&profile, &generation]).await.map_err(|_| invalid())?.ok_or_else(invalid)?;
        let bytes: String = row.get(0);
        if hex::encode(Sha256::digest(bytes.as_bytes())) != generation
            || u64::try_from(row.get::<_, i64>(1)).ok() != Some(revision)
        {
            return Err(invalid());
        }
        crate::prefix_generations::validate_package(profile, &bytes).map_err(|_| invalid())?;
        let package: Value = brassclaw_skills::value_contract::strict_json(
            &bytes,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(|_| invalid())?;
        let archived = STANDARD
            .decode(
                package["artifacts"]["source_snapshot.json"]["data"]
                    .as_str()
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| invalid())?;
        let archived = std::str::from_utf8(&archived).map_err(|_| invalid())?;
        let expected = self.definition.input["snapshot_bytes"]
            .as_str()
            .ok_or_else(invalid)?;
        // Compare the exact canonical source object passed to the compiler.
        // The package validator separately checks originals, manifests and maps.
        let snapshot = brassclaw_skills::value_contract::strict_json(
            expected,
            self.definition.original_input_limits,
        )
        .map_err(|_| invalid())?;
        if archived != crate::prefix_generations::canonical(&snapshot).map_err(|_| invalid())? {
            return Err(invalid());
        }
        Ok(())
    }
}
