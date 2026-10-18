//! Read-only capture over existing source owners. This is explicitly a core
//! reference snapshot, not complete-library or model-design qualification.
use brassclaw_product_workflow::RecipeStoreError;
use brassclaw_skills::revision_store::PgComponentRevisionStore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::installed_monty_catalogue::InstalledMontyCatalogue;
fn invalid() -> RecipeStoreError {
    RecipeStoreError::Internal("installed prefix source capture failed".into())
}
fn sha(bytes: &str) -> String {
    hex::encode(Sha256::digest(bytes.as_bytes()))
}
fn record(path: String, kind: &str, disposition: &str, reason: &str, text: String) -> Value {
    json!({"path":path,"kind":kind,"disposition":disposition,"reason":reason,"text":text})
}
impl InstalledMontyCatalogue {
    pub(crate) async fn capture_core_prefix_sources(
        &self,
        profile: &str,
    ) -> Result<String, RecipeStoreError> {
        if !matches!(profile, "base-prompt" | "prefix-maker") {
            return Err(RecipeStoreError::Invalid(
                "unsupported core source profile".into(),
            ));
        }
        self.reference_sources.verify().map_err(|_| invalid())?;
        let generation = self.active.read().await.generation.to_string();
        // The catalogue owner checks one coherent active view and exact sources.
        // Immutable packaged documents are pinned independently of editable Docs.
        let catalogue = self.export_sources(&generation).await?;
        let refs = self
            .reference_sources
            .snapshot
            .revisions()
            .values()
            .map(|revision| revision.reference())
            .collect::<Vec<_>>();
        let roots = self
            .reference_sources
            .snapshot
            .roots()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let mut client = self.pool.get().await.map_err(|_| invalid())?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| invalid())?;
        let snapshot = PgComponentRevisionStore::read_exact_in_transaction(&tx, &roots, &refs)
            .await
            .map_err(|_| invalid())?;
        let package = crate::prefix_reference_seeds::Package {
            snapshot: std::sync::Arc::new(snapshot),
        };
        package.verify().map_err(|_| invalid())?;
        tx.commit().await.map_err(|_| invalid())?;
        let design_record = package
            .snapshot
            .revisions()
            .values()
            .find(|revision| {
                revision.draft().document()["path"] == "scripts/prefix/core-reference-designs.json"
            })
            .ok_or_else(invalid)?;
        let designs = brassclaw_skills::value_contract::strict_json(
            design_record.draft().document()["content"]
                .as_str()
                .ok_or_else(invalid)?,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(|_| invalid())?;
        let design = &designs["profiles"][profile];
        if designs["format"] != "brassclaw-core-reference-designs/1" || !design.is_object() {
            return Err(invalid());
        }
        let mut sources = Vec::new();
        let mut documents = Vec::new();
        for revision in package.snapshot.revisions().values() {
            let reference = revision.reference();
            let document = revision.draft().document();
            let path = document["path"].as_str().ok_or_else(invalid)?;
            let text = document["content"].as_str().ok_or_else(invalid)?;
            let selection = &design["sources"][path];
            sources.push(record(
                path.into(),
                "document",
                selection["disposition"].as_str().ok_or_else(invalid)?,
                selection["reason"].as_str().ok_or_else(invalid)?,
                text.into(),
            ));
            documents.push(json!({"uuid":reference.uuid,"class_code":reference.class_code,"version":reference.version,"checksum":hex::encode(reference.checksum),"revision_bytes":revision.draft().exact_bytes()}));
        }
        let reference_manifest = crate::prefix_generations::canonical(&json!({"format":"brassclaw-core-reference-capture/1","coverage":"core-guides-only","components":documents})).map_err(|_| invalid())?;
        for component in &catalogue.components {
            sources.push(record(format!("catalogue/components/{}/v{}.json", component.reference.uuid, component.reference.version), "component", "included", "Exact owner-selected ordinary-workflow/root source; imported integrity alone is not approval", component.revision_bytes.clone()));
        }
        sources.push(record("catalogue/catalogue.json".into(), "document", "included", "Exact coherent routing catalogue, including disabled Recipe metadata; current Tool policy remains independent", catalogue.catalogue_bytes.clone()));
        for selection in &catalogue.selections {
            sources.push(record(
                format!("catalogue/selections/{}.json", sha(selection)),
                "document",
                "included",
                "Exact selected workflow/input/export/approval pins; not Tool permission",
                selection.clone(),
            ));
        }
        let catalogue_manifest = crate::prefix_generations::canonical(
            &serde_json::to_value(&catalogue).map_err(|_| invalid())?,
        )
        .map_err(|_| invalid())?;
        sources.push(record("capture/core-reference-revisions.json".into(), "document", "development-only", "Complete provenance envelope; document originals are separately included in the model view", reference_manifest.clone()));
        sources.push(record("capture/catalogue-export.json".into(), "document", "development-only", "Complete transfer envelope; component/selection originals are separately included in the model view", catalogue_manifest));
        let release = crate::prefix_generations::canonical(&json!({"format":"brassclaw-core-source-release/1","reference_manifest_sha256":sha(&reference_manifest),"catalogue_generation":catalogue.generation,"catalogue_checksum":catalogue.catalogue_checksum,"coverage":"core-guides-and-active-ordinary-workflows-with-retained-root"})).map_err(|_| invalid())?;
        let capture = json!({"format":"brassclaw-prefix-source-snapshot/1","profile":profile,"release":release,"header":design["header"],"evidence_view":design["evidence_view"],"sources":sources,
            "procedures":{"compiler/entry.py":brassclaw_skills::prefix_compiler_components::COMPILER_SOURCE,"compiler/prefix_render.py":brassclaw_skills::prefix_reference_components::REFERENCE_RENDER_SOURCE,"compiler/prefix_units.py":brassclaw_skills::prefix_compiler_components::UNIT_SOURCE}});
        let bytes = crate::prefix_generations::canonical(&capture).map_err(|_| invalid())?;
        crate::prefix_job_execution::validate_snapshot(profile, &bytes).map_err(|_| invalid())?;
        Ok(bytes)
    }
}
