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
    records: &mut BTreeMap<Uuid, CatalogueSourceComponent>,
    revision: &RetainedComponentRevision,
) -> Result<(), RecipeStoreError> {
    let r = revision.reference();
    if let Some(old) = records.get(&r.uuid) {
        if old.reference.version != r.version
            || old.reference.checksum != hex::encode(r.checksum)
            || old.reference.class_code != r.class_code
            || old.revision_bytes != revision.draft().exact_bytes()
        {
            return Err(integrity());
        }
    } else {
        records.insert(
            r.uuid,
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
            collect(&mut records, revision)?;
        }
        let mut selections = BTreeSet::new();
        for selected in std::iter::once(&self.reply)
            .chain(std::iter::once(&self.history))
            .chain(view.programs.iter())
        {
            let instruction = selected.program().inputs().instruction();
            for revision in instruction.snapshot().revisions().values() {
                collect(&mut records, revision)?;
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
            format: "brassclaw-catalogue-source-export/1".into(),
            generation: view.generation.to_string(),
            coverage: "active-ordinary-workflows-and-retained-root".into(),
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
