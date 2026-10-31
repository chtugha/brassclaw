//! Closed development candidate assembly over existing PG/IBS/Inspector owners.
//! Test-only until the installation-owned qualified caller exists; no production
//! admission, approval, model dispatch or extra storage API is introduced.
//! Consumes an already retained dependency snapshot; a standalone usage never
//! needs a dedicated reference Recipe to be selected here.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid};
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_monty_host::utility::UtilityLimits;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    bootstrap_answer_workflow::{BootstrapAnswerWorkflowIds, bootstrap_answer_workflow_drafts},
    component_revision::RetainedComponentSnapshot,
    global_bootstrap_components::UsageComponentIds,
    revision_store::PgComponentRevisionStore,
    strict_json_components::StrictJsonUsageIds,
};
use std::{path::Path, sync::Arc};
use uuid::Uuid;

/// Selected component addresses, not approval/capability objects. The closed
/// Subject owner separately compares their actual definitions/native artifacts
/// against its packaged usage contracts and reviewer foundation.
pub(super) struct Bindings {
    pub(super) model: UsageComponentIds,
    pub(super) response: UsageComponentIds,
    pub(super) strict: StrictJsonUsageIds,
    pub(super) answer_text: Uuid,
    pub(super) answer_stage: Uuid,
}

pub(super) async fn retain(
    pool: Arc<PgPool>,
    dependencies: &RetainedComponentSnapshot,
    ids: BootstrapAnswerWorkflowIds,
    bindings: &Bindings,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<Arc<InspectedRetainedProgram>, RebornBuildError> {
    let drafts = bootstrap_answer_workflow_drafts(
        ids,
        bindings.model,
        bindings.response,
        bindings.strict,
        bindings.answer_text,
        bindings.answer_stage,
    )
    .map_err(invalid)?;
    // Reuse the existing exact-root graph selector rather than a second graph
    // walker or latest lookup. Usage Skill roots bring their real Tool/binding/
    // code/association dependencies. Separate consuming Recipes are not roots.
    let dependencies = dependencies
        .select_roots(&[
            bindings.model.skill,
            bindings.response.skill,
            bindings.strict.skill,
            bindings.answer_text,
            bindings.answer_stage,
        ])
        .map_err(invalid)?;
    for draft in &drafts {
        if dependencies.revisions().contains_key(&draft.uuid()) {
            return Err(invalid("bootstrap candidate aliases a retained dependency"));
        }
    }
    let store = PgComponentRevisionStore::new(pool);
    let mut references: Vec<_> = dependencies
        .revisions()
        .values()
        .map(|revision| revision.reference())
        .collect();
    for draft in &drafts {
        references.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }
    let snapshot = store
        .read_exact(&[ids.recipe], &references)
        .await
        .map_err(invalid)?;
    for draft in &drafts {
        if snapshot
            .revisions()
            .get(&draft.uuid())
            .is_none_or(|revision| revision.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid("bootstrap candidate differs from packaged graph"));
        }
    }
    for (uuid, original) in dependencies.revisions() {
        if snapshot
            .revisions()
            .get(uuid)
            .is_none_or(|revision| revision.reference() != original.reference())
        {
            return Err(invalid(
                "bootstrap candidate substituted an original dependency",
            ));
        }
    }
    let program = prepare_retained_tool_program(
        compile_retained_recipe(
            Arc::new(snapshot),
            ids.recipe,
            "selected",
            WorkflowClass::RequiresModel,
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
