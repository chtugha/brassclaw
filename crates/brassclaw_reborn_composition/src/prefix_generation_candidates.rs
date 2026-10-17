//! Installation-owned immutable candidate retention. No bootstrap approval,
//! coherent activation, public matching or request admission occurs here.
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
    global_bootstrap_components::UsageComponentIds,
    prefix_generation_components::{
        GenerationUsage, generation_tool_draft, generation_usage_drafts,
    },
    revision_store::PgComponentRevisionStore,
};
use std::{path::Path, sync::Arc};

pub(super) async fn retain(
    pool: Arc<PgPool>,
    kernel: &dyn MontyKernelSnapshot,
    tool_id: uuid::Uuid,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<[Arc<InspectedRetainedProgram>; 2], RebornBuildError> {
    let capability = CapabilityId::new(brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID)
        .map_err(invalid)?;
    let runtime = kernel.builtin(&capability, None).map_err(invalid)?;
    runtime.verify_native_artifact().await.map_err(invalid)?;
    let native = runtime
        .native_identity()
        .ok_or_else(|| invalid("generation primitive has no retained native identity"))?;
    let tool = generation_tool_draft(tool_id, serde_json::to_value(native).map_err(invalid)?)
        .map_err(invalid)?;
    let store = PgComponentRevisionStore::new(pool.clone());
    let tool_ref = store.retain_packaged_draft(&tool).await.map_err(invalid)?;
    let mut result = Vec::new();
    for usage in [GenerationUsage::Read, GenerationUsage::Retain] {
        let retain = matches!(usage, GenerationUsage::Retain);
        let name = if retain { "retain" } else { "read" };
        let ids = UsageComponentIds {
            recipe: id(&format!("prefix-generation:{name}:recipe")),
            python_code: id(&format!("prefix-generation:{name}:code")),
            skill: id(&format!("prefix-generation:{name}:skill")),
            tool_skill: id(&format!("prefix-generation:{name}:binding")),
            tool: tool_ref.uuid,
            formatter: retain.then(|| id("prefix-generation:revision-formatter")),
        };
        let drafts = generation_usage_drafts(ids, usage).map_err(invalid)?;
        let mut references = vec![tool_ref];
        for draft in &drafts {
            references.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
        }
        let snapshot = Arc::new(
            store
                .read_exact(&[ids.recipe], &references)
                .await
                .map_err(invalid)?,
        );
        for draft in std::iter::once(&tool).chain(drafts.iter()) {
            if snapshot
                .revisions()
                .get(&draft.uuid())
                .is_none_or(|r| r.draft().exact_bytes() != draft.exact_bytes())
            {
                return Err(invalid(
                    "generation candidate differs from exact packaged definitions",
                ));
            }
        }
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                snapshot,
                ids.recipe,
                "selected",
                WorkflowClass::Deterministic,
            )
            .map_err(invalid)?,
        )
        .map_err(invalid)?;
        result.push(Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(Arc::new(program)),
                worker,
                limits,
            )
            .await
            .map_err(invalid)?,
        ));
    }
    // Initialize only the shipped identity; preserve operator edits/blocks.
    pool.get().await.map_err(invalid)?.execute(
        "INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1) ON CONFLICT(tool_id) DO NOTHING", &[&tool_ref.uuid],
    ).await.map_err(invalid)?;
    result
        .try_into()
        .map_err(|_| invalid("both generation candidates are required"))
}
