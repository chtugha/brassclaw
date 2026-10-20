//! Retention and inspection of exact packaged encoding usages. These candidates
//! are private and unapproved; retention does not advertise a chat command.
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
    component_revision::{ComponentRevisionDraft, RevisionError},
    global_bootstrap_components::UsageComponentIds,
    model_encoding_components::{
        model_encoding_tool_draft, model_encoding_usage_drafts,
        model_template_encoding_usage_drafts,
    },
    revision_store::PgComponentRevisionStore,
};
use std::{path::Path, sync::Arc};
use uuid::Uuid;

pub(super) fn ids(tool: Uuid, effective: bool) -> UsageComponentIds {
    brassclaw_skills::model_encoding_components::usage_ids(tool, effective, id)
}
pub(super) fn drafts(
    ids: UsageComponentIds,
    effective: bool,
) -> Result<Vec<ComponentRevisionDraft>, RevisionError> {
    if effective {
        model_template_encoding_usage_drafts(ids)
    } else {
        model_encoding_usage_drafts(ids)
    }
}
pub(super) async fn retain(
    pool: Arc<PgPool>,
    kernel: &dyn MontyKernelSnapshot,
    tool: Uuid,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<[Arc<InspectedRetainedProgram>; 2], RebornBuildError> {
    let runtime = kernel
        .builtin(
            &CapabilityId::new(brassclaw_host_runtime::MODEL_ENCODING_CAPABILITY_ID)
                .map_err(invalid)?,
            None,
        )
        .map_err(invalid)?;
    runtime.verify_native_artifact().await.map_err(invalid)?;
    let native = runtime
        .native_identity()
        .ok_or_else(|| invalid("encoding native identity missing"))?;
    let tool_draft =
        model_encoding_tool_draft(tool, serde_json::to_value(native).map_err(invalid)?)
            .map_err(invalid)?;
    let store = PgComponentRevisionStore::new(pool.clone());
    let tool_ref = store
        .retain_packaged_draft(&tool_draft)
        .await
        .map_err(invalid)?;
    let mut candidates = Vec::new();
    for effective in [false, true] {
        let ids = ids(tool, effective);
        let drafts = drafts(ids, effective).map_err(invalid)?;
        let mut references = vec![tool_ref];
        for draft in &drafts {
            references.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
        }
        let graph = Arc::new(
            store
                .read_exact(&[ids.recipe], &references)
                .await
                .map_err(invalid)?,
        );
        for draft in std::iter::once(&tool_draft).chain(drafts.iter()) {
            if graph
                .revisions()
                .get(&draft.uuid())
                .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
            {
                return Err(invalid(
                    "encoding candidate differs from packaged definitions",
                ));
            }
        }
        let program = prepare_retained_tool_program(
            compile_retained_recipe(graph, ids.recipe, "selected", WorkflowClass::Deterministic)
                .map_err(invalid)?,
        )
        .map_err(invalid)?;
        candidates.push(Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(Arc::new(program)),
                worker,
                limits,
            )
            .await
            .map_err(invalid)?,
        ));
    }
    pool.get().await.map_err(invalid)?.execute(
        "INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1) ON CONFLICT(tool_id) DO NOTHING", &[&tool],
    ).await.map_err(invalid)?;
    candidates
        .try_into()
        .map_err(|_| invalid("both encoding candidates are required"))
}
