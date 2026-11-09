//! Installation-owned immutable drafts for the already registered parser.
//! Retention supplies neither review evidence nor an active routing selection.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use brassclaw_engine::memory::retained_tools::RetainedToolProgram;
use brassclaw_host_api::CapabilityId;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::ComponentRevisionDraft,
    global_bootstrap_components::{UsageComponentIds, packaged_component_id},
    revision_store::PgComponentRevisionStore,
    validator_components::{source_inspection_drafts, source_inventory_drafts},
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    RebornBuildError, bootstrap_reply_validation::invalid, monty_kernel::MontyKernelSnapshot,
};

/// Exact refs/dependencies returned by the existing package store. This is
/// transient retention metadata, never component approval or latest selection.
#[derive(Default)]
pub(super) struct PackagedDefinitions {
    references: BTreeMap<Uuid, brassclaw_skills::association_contract::ComponentRevisionRef>,
    dependencies: BTreeMap<Uuid, BTreeSet<Uuid>>,
}
impl PackagedDefinitions {
    pub(super) async fn retain(
        &mut self,
        store: &PgComponentRevisionStore,
        draft: &ComponentRevisionDraft,
    ) -> Result<(), RebornBuildError> {
        let reference = store.retain_packaged_draft(draft).await.map_err(invalid)?;
        if self
            .references
            .insert(draft.uuid(), reference)
            .is_some_and(|old| old != reference)
        {
            return Err(invalid("packaged definition conflict"));
        }
        self.dependencies
            .insert(draft.uuid(), draft.dependencies().clone());
        Ok(())
    }
    pub(super) fn extend(&mut self, other: Self) -> Result<(), RebornBuildError> {
        for (id, reference) in other.references {
            if self
                .references
                .insert(id, reference)
                .is_some_and(|old| old != reference)
            {
                return Err(invalid("packaged graph reference conflict"));
            }
        }
        self.dependencies.extend(other.dependencies);
        Ok(())
    }
    pub(super) async fn snapshot(
        &self,
        pool: Arc<PgPool>,
        root: Uuid,
    ) -> Result<
        Arc<brassclaw_skills::component_revision::RetainedComponentSnapshot>,
        RebornBuildError,
    > {
        let mut pending = vec![root];
        let mut selected = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !selected.insert(id) {
                continue;
            }
            pending.extend(
                self.dependencies
                    .get(&id)
                    .ok_or_else(|| invalid("packaged dependency not retained"))?
                    .iter()
                    .copied(),
            );
        }
        let refs = selected
            .into_iter()
            .map(|id| {
                self.references
                    .get(&id)
                    .copied()
                    .ok_or_else(|| invalid("packaged revision not retained"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Arc::new(
            PgComponentRevisionStore::new(pool)
                .read_exact(
                    &[root],
                    &refs,
                    brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                )
                .await
                .map_err(invalid)?,
        ))
    }
}
pub(super) struct RetainedInspectionComponents {
    pub(super) usage: UsageComponentIds,
    pub(super) definitions: PackagedDefinitions,
}

pub(super) fn usage_ids(tool: Uuid, inventory: bool) -> UsageComponentIds {
    let usage = if inventory { "inventory" } else { "source" };
    UsageComponentIds {
        recipe: packaged_component_id(&format!("validator:inspection:{usage}:recipe")),
        python_code: packaged_component_id(&format!("validator:inspection:{usage}:code")),
        skill: packaged_component_id(&format!("validator:inspection:{usage}:skill")),
        // Both usages bind the same callable; their exact argument schemas live
        // in their separate associations and the native primitive contract.
        tool_skill: packaged_component_id("validator:inspection:tool-skill"),
        tool,
        formatter: None,
    }
}

pub(super) fn decoder_id() -> Uuid {
    packaged_component_id("validator:inspection:source-entries")
}

/// Recover the actual registration, verify its native artifact, then retain the
/// exact drafts. Package upgrades preserve old bytes and operator successors;
/// the existing store never changes active pointers or manufactures approval.
pub(super) async fn retain(
    pool: Arc<PgPool>,
    kernel: &dyn MontyKernelSnapshot,
    tenant: &str,
    reply: &RetainedToolProgram,
) -> Result<RetainedInspectionComponents, RebornBuildError> {
    let binding = reply
        .bindings()
        .get("0:2")
        .filter(|binding| {
            reply.bindings().len() == 1 && binding.capability_id() == "host.post_reply"
        })
        .ok_or_else(|| invalid("source inspection requires the existing reply usage"))?;
    let reply_ids = UsageComponentIds {
        recipe: reply.inputs().instruction().recipe().uuid,
        python_code: binding.python().uuid,
        tool_skill: binding.tool_skill().uuid,
        skill: binding.skill().uuid,
        tool: binding.tool().uuid,
        formatter: None,
    };
    let capability = CapabilityId::new(brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID)
        .map_err(invalid)?;
    let native = kernel.builtin(&capability, None).map_err(invalid)?;
    native.verify_native_artifact().await.map_err(invalid)?;
    let identity = native
        .native_identity()
        .ok_or_else(|| invalid("source inspection native identity missing"))?;
    let tool = crate::builtin_bootstrap::seed_source_inspection_tool(pool.clone(), tenant)
        .await
        .map_err(invalid)?;
    // The shared primitive supports either profile. Per-usage contracts require
    // exactly their selected argument; the native schema enforces exactly one.
    let tool_draft = ComponentRevisionDraft::from_json(&json!({
        "format":"component-revision/1","uuid":tool,"class_code":0,
        "document":{"capability_id":capability.as_str(),"callable":"host.source_inspection",
            "implementation":{"identity":identity},"input_contract":{
                "source":{"type":"string","required":false,"checks":[]},
                "sources":{"type":"object","required":false,"checks":[],"fields":{},"allow_extra_fields":true,
                    "extra_fields":{"type":"string","checks":[]}}
            }},"dependencies":[],"association":null
    }).to_string()).map_err(invalid)?;
    brassclaw_skills::value_contract::InputContract::tool_parameters_from_value(
        &tool_draft.document()["input_contract"],
        brassclaw_skills::component_revision::REVISION_LIMITS,
    )
    .map_err(invalid)?;
    let mut drafts = source_inspection_drafts(usage_ids(tool, false)).map_err(invalid)?;
    drafts.extend(
        source_inventory_drafts(usage_ids(tool, true), decoder_id(), reply_ids).map_err(invalid)?,
    );
    let store = PgComponentRevisionStore::new(pool.clone());
    let mut definitions = PackagedDefinitions::default();
    for draft in std::iter::once(&tool_draft).chain(drafts.iter()) {
        definitions.retain(&store, draft).await?;
    }
    // Preserve an existing live block across restarts/upgrades.
    pool.get().await.map_err(invalid)?.execute(
        "INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1) ON CONFLICT(tool_id) DO NOTHING",
        &[&tool],
    ).await.map_err(invalid)?;
    Ok(RetainedInspectionComponents {
        usage: usage_ids(tool, true),
        definitions,
    })
}
