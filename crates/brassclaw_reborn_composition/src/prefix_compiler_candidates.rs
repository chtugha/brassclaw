//! Retain/inspect one complete packaged compiler graph. No usage qualification,
//! activation, source collection, compilation effect or job admission occurs here.
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
    prefix_compiler_components::{SourceCompilerIds, source_compiler_drafts},
    revision_store::PgComponentRevisionStore,
};
use std::{path::Path, sync::Arc};

pub(super) struct CompilerCandidates {
    pub(super) source: Arc<InspectedRetainedProgram>,
    pub(super) digest: Arc<InspectedRetainedProgram>,
}

pub(super) async fn retain(
    pool: Arc<PgPool>,
    kernel: &dyn MontyKernelSnapshot,
    json_tool: uuid::Uuid,
    generation_tool: uuid::Uuid,
    worker: &Path,
    limits: UtilityLimits,
) -> Result<CompilerCandidates, RebornBuildError> {
    let mut native = Vec::new();
    for capability in [
        brassclaw_host_runtime::JSON_CAPABILITY_ID,
        brassclaw_host_runtime::PREFIX_GENERATION_CAPABILITY_ID,
    ] {
        let runtime = kernel
            .builtin(&CapabilityId::new(capability).map_err(invalid)?, None)
            .map_err(invalid)?;
        runtime.verify_native_artifact().await.map_err(invalid)?;
        native.push(
            serde_json::to_value(
                runtime
                    .native_identity()
                    .ok_or_else(|| invalid("compiler primitive has no retained native identity"))?,
            )
            .map_err(invalid)?,
        );
    }
    let ids = SourceCompilerIds::allocate(json_tool, generation_tool, id);
    let mut drafts = vec![
        brassclaw_skills::utf8_digest_components::json_text_tool_draft(
            json_tool,
            native[0].clone(),
        )
        .map_err(invalid)?,
        brassclaw_skills::prefix_generation_components::generation_tool_draft(
            generation_tool,
            native[1].clone(),
        )
        .map_err(invalid)?,
    ];
    drafts.extend(source_compiler_drafts(ids).map_err(invalid)?);
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut references = Vec::new();
    for draft in &drafts {
        references.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }
    let snapshot = Arc::new(
        store
            .read_exact(
                &[ids.recipe, ids.digest.recipe, ids.retain.recipe],
                &references,
            )
            .await
            .map_err(invalid)?,
    );
    for draft in &drafts {
        if snapshot
            .revisions()
            .get(&draft.uuid())
            .is_none_or(|actual| actual.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid(
                "compiler candidate differs from exact packaged definitions",
            ));
        }
    }
    let mut programs = Vec::new();
    for root in [ids.recipe, ids.digest.recipe] {
        let selected = Arc::new(snapshot.select_roots(&[root]).map_err(invalid)?);
        let program = prepare_retained_tool_program(
            compile_retained_recipe(selected, root, "selected", WorkflowClass::Deterministic)
                .map_err(invalid)?,
        )
        .map_err(invalid)?;
        programs.push(Arc::new(
            InspectedRetainedProgram::inspect_with_limits(
                RetainedProgram::Tools(Arc::new(program)),
                worker,
                limits,
            )
            .await
            .map_err(invalid)?,
        ));
    }
    let [source, digest]: [Arc<InspectedRetainedProgram>; 2] = programs
        .try_into()
        .map_err(|_| invalid("compiler candidate inventory differs"))?;
    // Binding metadata grants nothing; preserve an operator's existing block.
    pool.get().await.map_err(invalid)?.execute("INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1) ON CONFLICT(tool_id) DO NOTHING",&[&json_tool]).await.map_err(invalid)?;
    let candidates = CompilerCandidates { source, digest };
    tracing::debug!(source_recipe=%candidates.source.program().inputs().instruction().recipe().uuid,
        digest_recipe=%candidates.digest.program().inputs().instruction().recipe().uuid,
        "Retained unqualified prefix compiler candidates");
    Ok(candidates)
}
