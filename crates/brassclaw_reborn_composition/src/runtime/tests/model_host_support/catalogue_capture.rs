//! Exact draft selection for these contained tests, never catalogue approval.
use brassclaw_engine::memory::{
    intent_system::{IntentResolution, IntentScope},
    retained_instruction::{
        WorkflowClass, compile_matched_retained_recipe, compile_retained_recipe,
    },
    retained_tools::{RetainedToolProgram, prepare_retained_tool_program},
};
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

pub(super) fn scope(host: &MontyTaskHost) -> IntentScope {
    let context = host.run_context().expect("actual native chat fixture");
    IntentScope {
        tenant_id: context.scope.tenant_id.to_string(),
        user_id: context.actor.as_ref().unwrap().user_id.to_string(),
        agent_id: context.scope.agent_id.as_ref().unwrap().to_string(),
        // Match the installed catalogue's existing matcher namespace policy.
        // This does not rewrite the admitted task's optional project identity.
        project_id: context
            .scope
            .project_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "default".into()),
    }
}

/// The caller's matcher and all graph reads share this real transaction.
/// Existing exact-root selection is reused; no second graph walker/latest read.
pub(super) async fn programs(
    tx: &tokio_postgres::Transaction<'_>,
    originals: &[Arc<RetainedToolProgram>],
    matched: Option<&IntentResolution>,
) -> BTreeMap<Uuid, Arc<RetainedToolProgram>> {
    let mut refs = BTreeMap::new();
    let mut roots = Vec::new();
    for original in originals {
        let instruction = original.inputs().instruction();
        roots.push(instruction.recipe().uuid);
        for revision in instruction.snapshot().revisions().values() {
            if let Some(previous) = refs.insert(revision.reference().uuid, revision.reference()) {
                assert_eq!(
                    previous,
                    revision.reference(),
                    "incoherent draft fixture revisions"
                );
            }
        }
    }
    let snapshot = PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &roots,
        &refs.into_values().collect::<Vec<_>>(),
    )
    .await
    .unwrap();
    originals
        .iter()
        .map(|original| {
            let id = original.inputs().instruction().recipe().uuid;
            let selected = Arc::new(snapshot.select_roots(&[id]).unwrap());
            let instruction = match matched {
                Some(matched @ IntentResolution::Match { component_id, .. })
                    if *component_id == id =>
                {
                    compile_matched_retained_recipe(selected, matched, WorkflowClass::Deterministic)
                }
                _ => {
                    compile_retained_recipe(selected, id, "selected", WorkflowClass::Deterministic)
                }
            }
            .unwrap();
            (
                id,
                Arc::new(prepare_retained_tool_program(instruction).unwrap()),
            )
        })
        .collect()
}
