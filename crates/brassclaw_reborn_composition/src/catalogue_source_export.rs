//! Read-only projection of the existing catalogue owner's exact sources.
//! No row scanning, latest lookup, authoring verdict, publication or Tool call.
use std::collections::{BTreeMap, BTreeSet};

use brassclaw_product_workflow::{
    CatalogueSourceComponent, CatalogueSourceExport, ComponentRevisionSelection, RecipeStoreError,
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef, component_revision::RetainedComponentRevision,
    revision_store::PgComponentRevisionStore,
};
use sha2::{Digest, Sha256};
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

use crate::installed_monty_catalogue::InstalledMontyCatalogue;

// Supported projection/transport capacity, never a model token budget. Overflow
// rejects the entire export; it never truncates components or source bodies.
const MAX_EXPORT_BYTES: usize = 64 * 1024 * 1024;
fn integrity() -> RecipeStoreError {
    RecipeStoreError::Internal("catalogue source integrity verification failed".into())
}
fn reference(r: ComponentRevisionRef) -> ComponentRevisionSelection {
    ComponentRevisionSelection {
        uuid: r.uuid.to_string(),
        class_code: r.class_code,
        version: r.version,
        checksum: hex::encode(r.checksum),
    }
}
fn collect(
    records: &mut BTreeMap<(Uuid, u64), CatalogueSourceComponent>,
    revision: &RetainedComponentRevision,
    allow_versions: bool,
) -> Result<(), RecipeStoreError> {
    let r = revision.reference();
    if records
        .range((r.uuid, 0)..=(r.uuid, u64::MAX))
        .any(|(_, old)| {
            old.reference.class_code != r.class_code
                || (!allow_versions && old.reference.version != r.version)
        })
    {
        return Err(integrity());
    }
    if let Some(old) = records.get(&(r.uuid, r.version)) {
        if old.reference.version != r.version
            || old.reference.checksum != hex::encode(r.checksum)
            || old.reference.class_code != r.class_code
            || old.revision_bytes != revision.draft().exact_bytes()
        {
            return Err(integrity());
        }
    } else {
        records.insert(
            (r.uuid, r.version),
            CatalogueSourceComponent {
                reference: reference(r),
                revision_bytes: revision.draft().exact_bytes().to_owned(),
            },
        );
    }
    Ok(())
}
impl InstalledMontyCatalogue {
    pub(crate) async fn export_sources(
        &self,
        generation: &str,
    ) -> Result<CatalogueSourceExport, RecipeStoreError> {
        self.export_source_scope(generation, false).await
    }

    /// The two qualified private prefix workflows are source references, not
    /// newly advertised routing entries or execution authority. Different pinned
    /// revisions may coexist across independent approved workflow selections.
    pub(crate) async fn export_prefix_workflow_sources(
        &self,
        generation: &str,
    ) -> Result<CatalogueSourceExport, RecipeStoreError> {
        self.export_source_scope(generation, true).await
    }

    async fn export_source_scope(
        &self,
        generation: &str,
        prefix_workflows: bool,
    ) -> Result<CatalogueSourceExport, RecipeStoreError> {
        let expected = Uuid::parse_str(generation)
            .ok()
            .filter(|g| !g.is_nil() && g.to_string() == generation)
            .ok_or_else(|| {
                RecipeStoreError::Invalid("canonical non-nil catalogue generation required".into())
            })?;
        let _serial = self.activation.lock().await;
        let view = self.active.read().await.clone();
        if expected != view.generation {
            return Err(RecipeStoreError::Conflict(
                "catalogue generation changed; select and export again".into(),
            ));
        }
        let mut client =
            self.pool.get().await.map_err(|_| {
                RecipeStoreError::Unavailable("catalogue database unavailable".into())
            })?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| integrity())?;
        self.verify_source_selection(&tx, &view)
            .await
            .map_err(|_| integrity())?;
        // The root is retained separately from usage approvals. Exporting its
        // pinned source never promotes its pending review into an approval.
        let root = PgComponentRevisionStore::read_exact_in_transaction(
            &tx,
            &[self.root_reference.uuid],
            &[self.root_reference],
        )
        .await
        .map_err(|_| integrity())?;
        let mut records = BTreeMap::new();
        for revision in root.revisions().values() {
            collect(&mut records, revision, prefix_workflows)?;
        }
        let mut selections = BTreeSet::new();
        let mut private = Vec::new();
        if prefix_workflows {
            let compiler = self._compiler_program.as_ref().ok_or_else(|| {
                RecipeStoreError::Unavailable(
                    "source compiler qualification unavailable for export".into(),
                )
            })?;
            self.compiler_approval
                .as_ref()
                .ok_or_else(integrity)?
                .require_selected_program(compiler)
                .map_err(|_| integrity())?;
            private.push(compiler.clone());
            private.push(
                self.measurement_qualification
                    .as_ref()
                    .ok_or_else(|| {
                        RecipeStoreError::Unavailable(
                            "measurement qualification unavailable for export".into(),
                        )
                    })?
                    .program(),
            );
        }
        for selected in std::iter::once(&self.reply)
            .chain(std::iter::once(&self.history))
            .chain(view.programs.iter())
            .chain(private.iter())
        {
            let instruction = selected.program().inputs().instruction();
            for revision in instruction.snapshot().revisions().values() {
                collect(&mut records, revision, prefix_workflows)?;
            }
            selections.insert(
                instruction
                    .retained_selection()
                    .map_err(|_| integrity())?
                    .exact_bytes()
                    .to_owned(),
            );
        }
        let export = CatalogueSourceExport {
            format: if prefix_workflows {
                "brassclaw-catalogue-source-export/2"
            } else {
                "brassclaw-catalogue-source-export/1"
            }
            .into(),
            generation: view.generation.to_string(),
            coverage: if prefix_workflows {
                "active-ordinary-plus-source-compiler-and-measurement-with-retained-root"
            } else {
                "active-ordinary-workflows-and-retained-root"
            }
            .into(),
            catalogue_bytes: view.bytes.clone(),
            catalogue_checksum: hex::encode(Sha256::digest(view.bytes.as_bytes())),
            root: reference(self.root_reference),
            components: records.into_values().collect(),
            selections: selections.into_iter().collect(),
        };
        // Serde escaping is part of the actual transfer size, unlike raw source
        // sums. This limit is deliberately checked on the complete projection.
        if serde_json::to_vec(&export).map_err(|_| integrity())?.len() > MAX_EXPORT_BYTES {
            return Err(RecipeStoreError::Unavailable(
                "complete catalogue export exceeds supported transport capacity".into(),
            ));
        }
        tx.commit().await.map_err(|_| integrity())?;
        Ok(export)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_skills::component_revision::ComponentRevisionDraft;
    use serde_json::json;

    #[test]
    fn versioned_source_records_preserve_each_revision_and_reject_conflicts() {
        // Pure source-transfer records, not qualification or execution evidence.
        let uuid = Uuid::from_u128(911);
        let record = |class, version, text| {
            ComponentRevisionDraft::from_json(
                &json!({"format":"component-revision/1",
                "uuid":uuid,"class_code":class,"document":{"content":text},
                "dependencies":[],"association":null})
                .to_string(),
            )
            .unwrap()
            .at_version(version)
            .unwrap()
        };
        let first = record(17, 1, "first exact source\n");
        let next = record(17, 2, "second exact source\n");
        let mut expanded = BTreeMap::new();
        collect(&mut expanded, &first, true).unwrap();
        collect(&mut expanded, &next, true).unwrap();
        collect(&mut expanded, &first, true).unwrap();
        assert_eq!(expanded.len(), 2);
        assert_eq!(
            expanded[&(uuid, 1)].revision_bytes,
            first.draft().exact_bytes()
        );
        assert_eq!(
            expanded[&(uuid, 2)].revision_bytes,
            next.draft().exact_bytes()
        );
        assert!(
            collect(
                &mut expanded,
                &record(17, 1, "conflicting first source"),
                true
            )
            .is_err()
        );
        assert!(collect(&mut expanded, &record(18, 3, "class changed"), true).is_err());
        let mut ordinary = BTreeMap::new();
        collect(&mut ordinary, &first, false).unwrap();
        assert!(collect(&mut ordinary, &next, false).is_err());
        assert_eq!(ordinary.len(), 1);
    }
}
