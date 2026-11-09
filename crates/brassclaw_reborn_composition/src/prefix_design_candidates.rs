//! Exact installation-owned design-selection candidates. Retention/inspection
//! supplies no workflow approval, job admission, accepted design or serving.
use crate::{RebornBuildError, bootstrap_reply_validation::invalid, installed_monty_catalogue::id};
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
    component_revision::ComponentRevisionDraft,
    prefix_compiler_components::SourceCompilerIds,
    prefix_design_components::{
        BudgetedDesignIds, CapturedDesignSelectionIds, DesignSelectionIds,
        budgeted_design_selection_drafts,
    },
    revision_store::PgComponentRevisionStore,
};
use std::{collections::BTreeMap, path::Path, sync::Arc};
use uuid::Uuid;

pub(super) async fn retain(
    pool: Arc<PgPool>,
    compiler: &InspectedRetainedProgram,
    encoding: &InspectedRetainedProgram,
    tools: [Uuid; 3],
    worker: &Path,
    limits: UtilityLimits,
) -> Result<[Arc<InspectedRetainedProgram>; 3], RebornBuildError> {
    let [json_tool, generation_tool, model_tool] = tools;
    let source = SourceCompilerIds::allocate(json_tool, generation_tool, id);
    let digest = source.digest;
    let encoding_ids = crate::prefix_encoding_candidates::ids(model_tool, false);
    let design = DesignSelectionIds::allocate(id);
    let captured = CapturedDesignSelectionIds {
        compiler_library: source.library,
        source_prepare: source.stages[0],
        design_inputs: id("prefix-design-selection:captured:design-inputs"),
        coverage: id("prefix-design-selection:captured:coverage"),
        select: id("prefix-design-selection:captured:select"),
    };
    // Existing native/code/association definitions are reused at their exact
    // retained identities; a shared UUID with different bytes fails closed.
    let mut definitions = BTreeMap::<Uuid, ComponentRevisionDraft>::new();
    for inspected in [compiler, encoding] {
        for (uuid, revision) in inspected
            .program()
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
        {
            if definitions
                .get(uuid)
                .is_some_and(|previous| previous.exact_bytes() != revision.draft().exact_bytes())
            {
                return Err(invalid("design candidate dependency revisions conflict"));
            }
            definitions.insert(*uuid, revision.draft().clone());
        }
    }
    for draft in budgeted_design_selection_drafts(
        design,
        captured,
        BudgetedDesignIds {
            budget: id("prefix-design-selection:model-budget"),
            receipt: id("prefix-design-selection:budgeted:receipt"),
        },
        source.units,
        source.renderer,
        digest,
        encoding_ids,
    )
    .map_err(invalid)?
    {
        if definitions
            .get(&draft.uuid())
            .is_some_and(|previous| previous.exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid(
                "packaged design candidate conflicts with retained dependency",
            ));
        }
        definitions.insert(draft.uuid(), draft);
    }
    // Source/compiler snapshots also contain their own Recipe/retention roots.
    // This candidate needs only its complete dependency closure. In particular,
    // a usage's canonical execution Recipe is not an implicit dependency of
    // every Recipe that reuses that usage.
    let mut required = std::collections::BTreeSet::new();
    let mut pending = vec![design.recipe];
    while let Some(uuid) = pending.pop() {
        if !required.insert(uuid) {
            continue;
        }
        let draft = definitions
            .get(&uuid)
            .ok_or_else(|| invalid("design candidate dependency is not retained"))?;
        pending.extend(draft.dependencies());
    }
    definitions.retain(|uuid, _| required.contains(uuid));
    let store = PgComponentRevisionStore::new(pool);
    let mut references = Vec::new();
    for draft in definitions.values() {
        references.push(store.retain_packaged_draft(draft).await.map_err(invalid)?);
    }
    let snapshot = store
        .read_exact(
            &[design.recipe],
            &references,
            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
        )
        .await
        .map_err(invalid)?;
    for (uuid, draft) in &definitions {
        if snapshot
            .revisions()
            .get(uuid)
            .is_none_or(|revision| revision.draft().exact_bytes() != draft.exact_bytes())
        {
            return Err(invalid(
                "design candidate differs from exact packaged definitions",
            ));
        }
    }
    let mut candidates = Vec::new();
    for variant in ["selected", "captured", "budgeted"] {
        let selected = Arc::new(snapshot.select_roots(&[design.recipe]).map_err(invalid)?);
        let program = prepare_retained_tool_program(
            compile_retained_recipe(
                selected,
                design.recipe,
                variant,
                WorkflowClass::Deterministic,
            )
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
    candidates
        .try_into()
        .map_err(|_| invalid("design candidate inventory differs"))
}
