//! Exact packaged case-comparison graph. Retention/inspection is not approval,
//! a public route, a formatter or model-design acceptance.
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
    component_revision::ComponentRevisionDraft,
    prefix_pair_case_components::{PairCaseIds, pair_case_drafts},
    revision_store::PgComponentRevisionStore,
};
use std::{path::Path, sync::Arc};
use uuid::Uuid;

pub(super) async fn definitions(
    kernel: &dyn MontyKernelSnapshot,
    json_tool: Uuid,
    model_tool: Uuid,
) -> Result<(PairCaseIds, Vec<ComponentRevisionDraft>), RebornBuildError> {
    let mut native = Vec::new();
    for capability in [
        brassclaw_host_runtime::JSON_CAPABILITY_ID,
        brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID,
    ] {
        let runtime = kernel
            .builtin(&CapabilityId::new(capability).map_err(invalid)?, None)
            .map_err(invalid)?;
        runtime.verify_native_artifact().await.map_err(invalid)?;
        native.push(
            serde_json::to_value(
                runtime
                    .native_identity()
                    .ok_or_else(|| invalid("pair native identity missing"))?,
            )
            .map_err(invalid)?,
        );
    }
    let ids = PairCaseIds::allocate(id);
    let digest = brassclaw_skills::utf8_digest_components::usage_ids(json_tool, id);
    let encoding = brassclaw_skills::model_encoding_components::usage_ids(model_tool, false, id);
    let mut drafts = pair_case_drafts(ids, digest, encoding).map_err(invalid)?;
    drafts.extend(
        brassclaw_skills::utf8_digest_components::utf8_digest_usage_drafts(digest)
            .map_err(invalid)?,
    );
    drafts.extend(
        brassclaw_skills::model_encoding_components::model_encoding_usage_drafts(encoding)
            .map_err(invalid)?,
    );
    drafts.push(
        brassclaw_skills::utf8_digest_components::json_text_tool_draft(
            json_tool,
            native[0].clone(),
        )
        .map_err(invalid)?,
    );
    drafts.push(
        brassclaw_skills::model_encoding_components::model_encoding_tool_draft(
            model_tool,
            native[1].clone(),
        )
        .map_err(invalid)?,
    );
    Ok((ids, drafts))
}

pub(super) async fn retain(
    pool: Arc<PgPool>,
    kernel: &dyn MontyKernelSnapshot,
    json_tool: Uuid,
    model_tool: Uuid,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<Arc<InspectedRetainedProgram>, RebornBuildError> {
    let (ids, drafts) = definitions(kernel, json_tool, model_tool).await?;
    let store = PgComponentRevisionStore::new(pool);
    let mut references = Vec::new();
    for draft in &drafts {
        references.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }
    let digest = brassclaw_skills::utf8_digest_components::usage_ids(json_tool, id);
    let encoding = brassclaw_skills::model_encoding_components::usage_ids(model_tool, false, id);
    let snapshot = store
        .read_exact(&[ids.recipe, digest.recipe, encoding.recipe], &references)
        .await
        .map_err(invalid)?;
    for draft in &drafts {
        if snapshot
            .revisions()
            .get(&draft.uuid())
            .is_none_or(|actual| actual.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid("pair packaged graph differs"));
        }
    }
    let program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(snapshot.select_roots(&[ids.recipe]).map_err(invalid)?),
            ids.recipe,
            "selected",
            WorkflowClass::Deterministic,
        )
        .map_err(invalid)?,
    )
    .map_err(invalid)?;
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
