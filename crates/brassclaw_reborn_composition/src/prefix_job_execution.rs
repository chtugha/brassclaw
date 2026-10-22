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
use std::sync::Arc;

use crate::global_recipe_ports::{InternalRecipeSource, SelectedMontyRecipe};
use crate::prefix_job_definitions::Definition;

pub(crate) struct Invocation {
    pub(crate) definition: Definition,
    pub(crate) reference: String,
    pub(crate) inspected: Arc<InspectedRetainedProgram>,
    pub(crate) tools: Arc<dyn RetainedToolPort>,
    pub(crate) pool: Arc<PgPool>,
}
fn invalid() -> PortFailure {
    PortFailure::new("prefix_job_receipt_invalid").expect("constant classification")
}
/// Source metadata must describe the actual qualified compiler procedures.
/// References remain data; this check neither trusts their prose nor executes it.
pub(crate) fn validate_snapshot(profile: &str, bytes: &str) -> Result<(), PortFailure> {
    let snapshot = brassclaw_skills::value_contract::strict_json(
        bytes,
        brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
    )
    .map_err(|_| invalid())?;
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
    fn name(&self) -> &str {
        if self.definition.operation == "measure" {
            "prefix-measurement"
        } else {
            "prefix-generation"
        }
    }
    fn reference(&self) -> &str {
        &self.reference
    }
    async fn select(&self) -> Result<SelectedMontyRecipe, PortFailure> {
        Ok(SelectedMontyRecipe {
            normal_match: None,
            inspected: self.inspected.clone(),
            inputs: Some(self.definition.input.clone().into()),
            tools: Some(self.tools.clone()),
        })
    }
    async fn verify_receipt(
        &self,
        receipt: &str,
        selected: &InspectedRetainedProgram,
        execution: &RetainedRecipeExecution,
    ) -> Result<(), PortFailure> {
        if self.definition.operation == "measure" {
            if !std::ptr::eq(selected, self.inspected.as_ref()) {
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
        if !std::ptr::eq(selected, self.inspected.as_ref())
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
        let profile = &self.definition.profile;
        if value["profile"] != *profile
            || value["qualification"] != "quarantined"
            || receipt != format!("prefix-generation:{profile}:{generation}")
        {
            return Err(invalid());
        }
        let client = self.pool.get().await.map_err(|_| invalid())?;
        let row = client.query_opt("SELECT generation_bytes,profile_revision FROM reborn_prefix_generations WHERE profile_name=$1 AND generation_id=$2", &[profile, &generation]).await.map_err(|_| invalid())?.ok_or_else(invalid)?;
        let bytes: String = row.get(0);
        if hex::encode(Sha256::digest(bytes.as_bytes())) != generation
            || u64::try_from(row.get::<_, i64>(1)).ok() != Some(self.definition.revision)
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
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(|_| invalid())?;
        if archived != crate::prefix_generations::canonical(&snapshot).map_err(|_| invalid())? {
            return Err(invalid());
        }
        Ok(())
    }
}
