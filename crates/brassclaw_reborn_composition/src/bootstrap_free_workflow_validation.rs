//! Contained structural verification of the exact packaged B1/B2 answer Recipe.
//! Executes the independent pure oracle and verifies exact graph assembly and
//! protocol behavior. This is not a model call, Free activation or serving claim.
use super::fixture;
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_engine::executor::retained_source::InspectedRetainedProgram;
use brassclaw_monty_host::utility::UtilityLimits;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::REVISION_LIMITS, revision_store::PgComponentRevisionStore,
    value_contract::strict_json,
};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use tokio_postgres::Transaction;
use uuid::Uuid;

/// Structural checks on the assembled B1/B2 program beyond pure-protocol oracle.
pub(super) fn check_graph_structure(
    inspected: &InspectedRetainedProgram,
    ids: &FreeWorkflowIds,
) -> Result<(), RebornBuildError> {
    use brassclaw_engine::executor::retained_recipe::RetainedProgram;
    let RetainedProgram::Tools(program) = inspected.program() else {
        return Err(invalid("free-workflow candidate must be a Tools program"));
    };
    // The B1/B2 answer Recipe uses RequiresModel; bindings are 0:3 (model),
    // 0:5 (response/JSON), 0:8 (strict JSON). The pure components handle the
    // rest without Tool dispatch.
    let bindings = program.bindings();
    if bindings.len() != 3 {
        return Err(invalid("free-workflow candidate binding count differs"));
    }
    if bindings
        .get("0:3")
        .is_none_or(|b| b.capability_id() != "host.kohai_complete")
    {
        return Err(invalid("free-workflow model binding at 0:3 differs"));
    }
    if bindings
        .get("0:5")
        .is_none_or(|b| b.capability_id() != brassclaw_host_runtime::JSON_CAPABILITY_ID)
    {
        return Err(invalid("free-workflow response binding at 0:5 differs"));
    }
    if bindings
        .get("0:8")
        .is_none_or(|b| b.capability_id() != brassclaw_host_runtime::JSON_CAPABILITY_ID)
    {
        return Err(invalid("free-workflow strict binding at 0:8 differs"));
    }
    // The pure protocol component and its dependencies must be present.
    let _ = inspected.pure_component(ids.protocol).map_err(invalid)?;
    // Verify the recipe UUID matches the assembled instruction.
    let instruction = program.inputs().instruction();
    if instruction.recipe().uuid != ids.recipe {
        return Err(invalid("free-workflow candidate recipe UUID differs"));
    }
    Ok(())
}

/// Executes the independent pure oracle and returns structured observations.
pub(super) async fn execute(
    pool: Arc<PgPool>,
    inspected: Arc<InspectedRetainedProgram>,
    ids: &FreeWorkflowIds,
    worker: &Path,
    limits: UtilityLimits,
    task_settings: brassclaw_monty_host::process::TaskSettings,
) -> Result<Value, RebornBuildError> {
    check_graph_structure(&inspected, ids)?;
    // Verify all referenced component revisions are exact packaged definitions.
    let store = PgComponentRevisionStore::new(pool.clone());
    let instruction = inspected.program().inputs().instruction();
    let refs: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect();
    store
        .read_exact(
            &[ids.recipe],
            &refs,
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
        .map_err(invalid)?;
    // Record the serving selection hash before and after to confirm no side effect.
    let before_hash = serving_hash(&pool).await?;
    let observations =
        fixture::execute_pure_oracle(&inspected, ids.protocol, worker, limits, task_settings)
            .await?;
    let after_hash = serving_hash(&pool).await?;
    if before_hash != after_hash {
        return Err(invalid("free-workflow oracle changed serving selection"));
    }
    Ok(json!({
        "format": "free-workflow-behavior-observations/1",
        "recipe": ids.recipe,
        "protocol": ids.protocol,
        "observations": observations,
        "serving_before": before_hash,
        "serving_after": after_hash,
    }))
}

/// Verify stored behavior evidence against the independent fixture oracle.
pub(super) async fn verify(
    tx: &Transaction<'_>,
    inspected: &InspectedRetainedProgram,
    ids: &FreeWorkflowIds,
    observed: &Value,
) -> Result<(), RebornBuildError> {
    crate::bootstrap_evidence::require_coherent(tx).await?;
    if observed.as_object().is_none_or(|v| v.len() != 6)
        || observed["format"] != "free-workflow-behavior-observations/1"
        || observed["recipe"] != ids.recipe.to_string()
        || observed["protocol"] != ids.protocol.to_string()
        || observed["serving_before"]
            .as_str()
            .is_none_or(|s| s.len() != 64)
        || observed["serving_before"] != observed["serving_after"]
    {
        return Err(invalid("free-workflow behavior envelope differs"));
    }
    // Verify graph structure once more against the stored observation.
    check_graph_structure(inspected, ids)?;
    // Verify the stored component revisions are still exact.
    let instruction = inspected.program().inputs().instruction();
    let refs: Vec<_> = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect();
    PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[ids.recipe],
        &refs,
        brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
    )
    .await
    .map_err(invalid)?;
    // Verify oracle cases against the independent fixture constants.
    let oracle_observations = &observed["observations"];
    if oracle_observations["format"] != "free-workflow-pure-oracle/2" {
        return Err(invalid("free-workflow oracle format differs"));
    }
    fixture::verify_oracle_observations(oracle_observations)?;
    // Verify the stored strict-JSON bytes round-trip exactly.
    let stored_bytes = crate::prefix_generations::canonical(observed).map_err(invalid)?;
    let parsed = strict_json(&stored_bytes, REVISION_LIMITS).map_err(invalid)?;
    if &parsed != observed {
        return Err(invalid(
            "free-workflow behavior observation round-trip differs",
        ));
    }
    Ok(())
}

/// Component IDs for the assembled B1/B2 answer Recipe.
pub(super) struct FreeWorkflowIds {
    pub(super) recipe: Uuid,
    pub(super) protocol: Uuid,
}

async fn serving_hash(pool: &PgPool) -> Result<String, RebornBuildError> {
    let bytes: String = pool
        .get()
        .await
        .map_err(invalid)?
        .query_one(
            "SELECT coalesce(jsonb_agg(to_jsonb(s) ORDER BY to_jsonb(s)::text),'[]'::jsonb)::text \
             FROM reborn_basic_prompt_store s",
            &[],
        )
        .await
        .map_err(invalid)?
        .get(0);
    Ok(crate::bootstrap_evidence::checksum(&bytes))
}
