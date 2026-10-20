//! One private retained measurement graph, reusing the shipped digest and
//! effective-template encoding usages. Inspection is not workflow approval.
use crate::{
    RebornBuildError, bootstrap_reply_validation::invalid, installed_monty_catalogue::id,
    monty_kernel::MontyKernelSnapshot,
};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_host_api::CapabilityId;
use brassclaw_monty_host::utility::UtilityLimits;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    prefix_measurement_components::{MeasurementIds, measurement_drafts},
    revision_store::PgComponentRevisionStore,
};
use std::{path::Path, sync::Arc};
use uuid::Uuid;

pub(super) async fn retain(
    pool: Arc<PgPool>,
    kernel: &dyn MontyKernelSnapshot,
    json_tool: Uuid,
    model_tool: Uuid,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<Arc<InspectedRetainedProgram>, RebornBuildError> {
    let mut identities = Vec::new();
    for capability in [
        brassclaw_host_runtime::JSON_CAPABILITY_ID,
        brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID,
    ] {
        let runtime = kernel
            .builtin(&CapabilityId::new(capability).map_err(invalid)?, None)
            .map_err(invalid)?;
        runtime.verify_native_artifact().await.map_err(invalid)?;
        identities.push(
            serde_json::to_value(
                runtime
                    .native_identity()
                    .ok_or_else(|| invalid("measurement native identity missing"))?,
            )
            .map_err(invalid)?,
        );
    }
    let ids = MeasurementIds::allocate(
        brassclaw_skills::utf8_digest_components::usage_ids(json_tool, id),
        brassclaw_skills::model_encoding_components::usage_ids(model_tool, true, id),
        id,
    );
    let mut drafts = measurement_drafts(ids).map_err(invalid)?;
    drafts.push(
        brassclaw_skills::utf8_digest_components::json_text_tool_draft(
            json_tool,
            identities[0].clone(),
        )
        .map_err(invalid)?,
    );
    drafts.push(
        brassclaw_skills::model_encoding_components::model_encoding_tool_draft(
            model_tool,
            identities[1].clone(),
        )
        .map_err(invalid)?,
    );
    let store = PgComponentRevisionStore::new(pool);
    let mut references = Vec::new();
    for draft in &drafts {
        references.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }
    let all = store
        .read_exact(
            &[ids.recipe, ids.digest.recipe, ids.encoding.recipe],
            &references,
        )
        .await
        .map_err(invalid)?;
    for draft in &drafts {
        if all
            .revisions()
            .get(&draft.uuid())
            .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid(
                "measurement graph differs from packaged definitions",
            ));
        }
    }
    let snapshot = Arc::new(all.select_roots(&[ids.recipe]).map_err(invalid)?);
    let selected = compile_retained_recipe(
        snapshot,
        ids.recipe,
        "selected",
        WorkflowClass::Deterministic,
    )
    .map_err(invalid)?;
    let program = prepare_retained_tool_program(selected).map_err(invalid)?;
    if program.bindings().len() != 2
        || program
            .bindings()
            .get("0:5")
            .is_none_or(|b| b.tool().uuid != json_tool || b.skill().uuid != ids.digest.skill)
        || program
            .bindings()
            .get("0:7")
            .is_none_or(|b| b.tool().uuid != model_tool || b.skill().uuid != ids.encoding.skill)
    {
        return Err(invalid("measurement must reuse exact canonical usages"));
    }
    Ok(Arc::new(
        InspectedRetainedProgram::inspect_with_limits(
            RetainedProgram::Tools(Arc::new(program)),
            worker,
            limits,
        )
        .await
        .map_err(invalid)?,
    ))
}
