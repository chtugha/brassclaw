//! Installation-owned literal references, retained in the existing class-17
//! component revision store. No second document catalogue or approval label.
#[path = "prefix_release_documents.rs"]
mod release;
use brassclaw_engine::memory::intent_system::IntentScope;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    component_revision::{ComponentRevisionDraft, RetainedComponentSnapshot},
    revision_store::PgComponentRevisionStore,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::{RebornBuildError, installed_monty_catalogue::id};

const GUIDES: [(&str, &str); 9] = [
    ("skills.md", include_str!("../../../skills.md")),
    ("recipe.md", include_str!("../../../recipe.md")),
    ("tools.md", include_str!("../../../tools.md")),
    ("toolskills.md", include_str!("../../../toolskills.md")),
    ("CLAUDE.md", include_str!("../../../CLAUDE.md")),
    ("AGENTS.md", include_str!("../../../AGENTS.md")),
    (
        "simplified_v3.md",
        include_str!("../../../simplified_v3.md"),
    ),
    (
        "scripts/prefix/prefix-maker-reference.md",
        include_str!("../../../scripts/prefix/prefix-maker-reference.md"),
    ),
    (
        "scripts/prefix/core-reference-designs.json",
        include_str!("../../../scripts/prefix/core-reference-designs.json"),
    ),
];
const DESCRIPTION: &str = "Initial packaged reference projection. Prefix source collection uses exact immutable release revisions, not this editable compatibility row.";
fn invalid() -> RebornBuildError {
    RebornBuildError::InvalidConfig {
        reason: "packaged prefix reference integrity differs".into(),
    }
}
fn sha(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}
fn drafts() -> Result<Vec<ComponentRevisionDraft>, RebornBuildError> {
    let mut drafts = GUIDES.iter().map(|(path, text)| {
        if text.trim().is_empty() { return Err(invalid()); }
        ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":id(&format!("prefix:document:{path}")),"class_code":17,
            "document":{"format":"packaged-prefix-reference/1","path":path,"content":text,"content_sha256":sha(text),"coverage":"core-guides-only","disposition":"included","authority":"packaged-reference; distinguish binding targets from implemented support"},
            "dependencies":[],"association":null}).to_string()).map_err(|_| invalid())
    }).collect::<Result<Vec<_>, _>>()?;
    drafts.extend(release::additional_drafts()?);
    Ok(drafts)
}

pub(crate) struct Package {
    pub(crate) snapshot: Arc<RetainedComponentSnapshot>,
}
impl Package {
    pub(crate) fn core_snapshot(&self) -> Result<Arc<RetainedComponentSnapshot>, RebornBuildError> {
        self.snapshot
            .select_roots(
                &GUIDES
                    .iter()
                    .map(|(path, _)| id(&format!("prefix:document:{path}")))
                    .collect::<Vec<_>>(),
            )
            .map(Arc::new)
            .map_err(|_| invalid())
    }
    /// The supplied graph must match this executable's complete packaged set.
    /// Reading a source/status label or an allocation head is insufficient.
    pub(crate) fn verify(&self) -> Result<(), RebornBuildError> {
        let expected = drafts()?;
        if self.snapshot.revisions().len() != expected.len()
            || self.snapshot.roots().len() != expected.len()
        {
            return Err(invalid());
        }
        for draft in expected {
            let retained = self
                .snapshot
                .revisions()
                .get(&draft.uuid())
                .ok_or_else(invalid)?;
            if retained.draft().exact_bytes() != draft.exact_bytes()
                || retained.reference().checksum != draft.checksum()
                || retained.reference().class_code != 17
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

pub(crate) async fn retain(
    pool: Arc<PgPool>,
    scope: &IntentScope,
) -> Result<Package, RebornBuildError> {
    let store = PgComponentRevisionStore::new(pool.clone());
    let documents = drafts()?;
    let mut refs = Vec::with_capacity(documents.len());
    for document in &documents {
        refs.push(
            store
                .retain_packaged_draft(document)
                .await
                .map_err(|_| invalid())?,
        );
    }
    let roots = documents
        .iter()
        .map(ComponentRevisionDraft::uuid)
        .collect::<Vec<_>>();
    let snapshot = Arc::new(
        store
            .read_exact(&roots, &refs)
            .await
            .map_err(|_| invalid())?,
    );
    let package = Package { snapshot };
    package.verify()?;
    // Compatibility projection only. Never overwrite an operator edit or an
    // older projection on upgrade. The exact current package above is the
    // source owner; unchanged/older revisions remain independently readable.
    let mut client = pool.get().await.map_err(|_| invalid())?;
    let tx = client.transaction().await.map_err(|_| invalid())?;
    for document in &documents {
        let path = document.document()["path"].as_str().ok_or_else(invalid)?;
        if !GUIDES.iter().any(|(core, _)| *core == path) {
            continue;
        }
        let text = document.document()["content"]
            .as_str()
            .ok_or_else(invalid)?;
        let hash = sha(text);
        tx.execute("INSERT INTO reborn_docus(id,tenant_id,user_id,agent_id,project_id,name,description,content,content_hash,source,validation_status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'system_seed:prefix-reference','validated') ON CONFLICT DO NOTHING",
            &[&document.uuid(),&scope.tenant_id,&scope.user_id,&scope.agent_id,&scope.project_id,&path,&DESCRIPTION,&text,&hash]).await.map_err(|_| invalid())?;
    }
    tx.commit().await.map_err(|_| invalid())?;
    Ok(package)
}

#[cfg(test)]
pub(crate) async fn assert_database_sources(
    catalogue: &Arc<crate::installed_monty_catalogue::InstalledMontyCatalogue>,
) {
    use brassclaw_skills::component_revision::ComponentRevisionDraft;
    catalogue.reference_sources.verify().unwrap();
    let before = catalogue
        .capture_core_prefix_sources("base-prompt")
        .await
        .unwrap();
    assert_eq!(
        before,
        catalogue
            .capture_core_prefix_sources("base-prompt")
            .await
            .unwrap()
    );
    let capture: serde_json::Value = serde_json::from_str(&before).unwrap();
    let release_bytes = catalogue
        .capture_declared_release_prefix_sources("base-prompt")
        .await
        .unwrap();
    assert_eq!(
        release_bytes,
        catalogue
            .capture_declared_release_prefix_sources("base-prompt")
            .await
            .unwrap()
    );
    let release_capture: serde_json::Value = serde_json::from_str(&release_bytes).unwrap();
    assert_ne!(release_capture["header"], capture["header"]);
    let generation = catalogue.active.read().await.generation.to_string();
    let workflow_sources = catalogue
        .export_prefix_workflow_sources(&generation)
        .await
        .unwrap();
    assert_eq!(
        workflow_sources.format,
        "brassclaw-catalogue-source-export/2"
    );
    for selected in [
        catalogue._compiler_program.as_ref().unwrap().clone(),
        catalogue
            .measurement_qualification
            .as_ref()
            .unwrap()
            .program(),
    ] {
        let instruction = selected.program().inputs().instruction();
        assert!(
            workflow_sources
                .selections
                .iter()
                .any(|source| source == instruction.retained_selection().unwrap().exact_bytes())
        );
        for revision in instruction.snapshot().revisions().values() {
            assert!(
                workflow_sources
                    .components
                    .iter()
                    .any(
                        |source| source.reference.uuid == revision.reference().uuid.to_string()
                            && source.reference.version == revision.reference().version
                            && source.revision_bytes == revision.draft().exact_bytes()
                    )
            );
        }
    }
    if let Some(path) = std::env::var_os("BRASSCLAW_TEST_CATALOGUE_EXPORT") {
        use std::io::Write;
        let path = std::path::PathBuf::from(path).with_extension("prefix-workflows.json");
        let bytes = serde_json::to_vec(&workflow_sources).unwrap();
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(mut output) => {
                output.write_all(&bytes).unwrap();
                output.sync_all().unwrap();
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                assert_eq!(std::fs::read(path).unwrap(), bytes)
            }
            Err(error) => panic!("cannot retain exact prefix workflow source export: {error}"),
        }
    }
    let additions = release::additional_drafts().unwrap();
    let archived_ids: Vec<_> = additions.iter().map(|draft| draft.uuid()).collect();
    for draft in additions {
        let document = draft.document();
        let source = release_capture["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["path"] == document["path"])
            .unwrap();
        for (source_field, document_field) in [
            ("text", "content"),
            ("disposition", "disposition"),
            ("reason", "reason"),
        ] {
            assert_eq!(source[source_field], document[document_field]);
        }
        assert!(
            !capture["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source["path"] == document["path"])
        );
    }
    assert_eq!(
        catalogue
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT count(*) FROM reborn_docus WHERE id=ANY($1)",
                &[&archived_ids],
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0,
        "archived/review-pending release sources must not become validated legacy retrieval rows"
    );
    for (path, text) in GUIDES {
        let source = capture["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["path"] == path)
            .unwrap();
        assert_eq!(source["text"], text);
    }
    let store = PgComponentRevisionStore::new(catalogue.pool.clone());
    let original = catalogue
        .reference_sources
        .snapshot
        .revisions()
        .values()
        .find(|revision| revision.draft().document()["path"] == "skills.md")
        .unwrap();
    let reference = original.reference();
    let mut authored: serde_json::Value =
        serde_json::from_str(original.draft().exact_bytes()).unwrap();
    authored["document"]["content"] =
        json!("Operator draft is not packaged reference authority.\n");
    authored["document"]["content_sha256"] =
        json!(sha(authored["document"]["content"].as_str().unwrap()));
    let authored = ComponentRevisionDraft::from_json(&authored.to_string()).unwrap();
    let latest = store.stage(&authored, reference.version).await.unwrap();
    let client = catalogue.pool.get().await.unwrap();
    let projection: String = client
        .query_one(
            "SELECT content FROM reborn_docus WHERE id=$1",
            &[&reference.uuid],
        )
        .await
        .unwrap()
        .get(0);
    client.execute("UPDATE reborn_docus SET content='operator compatibility edit',validation_status='pending' WHERE id=$1", &[&reference.uuid]).await.unwrap();
    let retained = retain(catalogue.pool.clone(), &catalogue.qualification_scope)
        .await
        .unwrap();
    assert_eq!(
        retained.snapshot.revisions()[&reference.uuid].reference(),
        reference
    );
    assert_eq!(
        store.read_allocation_head(reference.uuid).await.unwrap(),
        Some((17, latest.version))
    );
    assert_eq!(
        client
            .query_one(
                "SELECT content FROM reborn_docus WHERE id=$1",
                &[&reference.uuid]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "operator compatibility edit"
    );
    assert_eq!(
        before,
        catalogue
            .capture_core_prefix_sources("base-prompt")
            .await
            .unwrap()
    );
    let mut refs = catalogue
        .reference_sources
        .snapshot
        .revisions()
        .values()
        .map(|revision| revision.reference())
        .collect::<Vec<_>>();
    *refs
        .iter_mut()
        .find(|candidate| candidate.uuid == reference.uuid)
        .unwrap() = latest;
    let roots = catalogue
        .reference_sources
        .snapshot
        .roots()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let changed = Package {
        snapshot: Arc::new(store.read_exact(&roots, &refs).await.unwrap()),
    };
    assert!(
        changed.verify().is_err(),
        "a source label cannot qualify different reference bytes"
    );
    // Restore this test's compatibility projection; immutable authored/package
    // revisions stay retained. No revision or accepted evidence is deleted.
    client
        .execute(
            "UPDATE reborn_docus SET content=$2,validation_status='validated' WHERE id=$1",
            &[&reference.uuid, &projection],
        )
        .await
        .unwrap();
}
