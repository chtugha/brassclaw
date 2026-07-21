//! Phase 5 — v1 SKILL.md → v2 MemoryDoc migration.
//!
//! Reads the embedded migration catalog emitted by `build.rs` and
//! persists each entry as a `MemoryDoc` with `DocType::Skill`. The
//! shared-owner project (`__shared__ / migrated-skills-v1`) makes the
//! resulting docs visible through `store.list_skills_global()`, which is
//! the path the Python orchestrator's `__list_skills__` host call uses
//! to assemble skill candidates.
//!
//! The migration is **idempotent**: every doc carries a stable v5
//! `DocId` derived from the skill name, so re-running on a populated
//! store either no-ops (when both content and `content_hash` match) or
//! refreshes the row in place (when the SKILL.md source changed since
//! the prior install). User-edited skills — those whose stored `source`
//! field differs from `"migrated"` — are never overwritten; only
//! migration-sourced entries are eligible for refresh.
//!
//! The migration tool runs once per startup from
//! `crate::factory::build_local_dev` (and the parallel libsql and
//! production paths, gated on the relevant feature) immediately after
//! `MemoryDocLibSqlStore::open`.

#![forbid(unsafe_code)]

use brassclaw_engine::traits::store::Store;
use brassclaw_engine::types::error::EngineError;
use brassclaw_engine::types::memory::{DocId, DocType, MemoryDoc};
use brassclaw_engine::types::project::ProjectId;
use brassclaw_engine::types::shared_owner_id;
use brassclaw_skills::v2::{
    CodeSnippet, SkillMetrics, SkillRepairRecord, SkillRevision, V2SkillMetadata,
    V2SkillSource,
};
use brassclaw_skills::SkillManifest;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Stable v5 namespace for migrated skill IDs.
///
/// Burning this value would rotate every DocId, force-reseeding the
/// table, and silently drop any `replaces_id`/`parent_version` links
/// that point at the rotated IDs — never change it after the first
/// migration runs in production.
const MIGRATED_SKILL_DOC_NAMESPACE: Uuid =
    uuid::uuid!("7f42a83e-59ab-4d04-8df4-44a4e6e9d4f3");

/// Project slug used to scope migrated-skill docs under the shared
/// owner. Combined with the shared owner id (`__shared__`), it produces
/// a stable `ProjectId` for every `list_memory_docs_by_owner` query
/// the migration performs.
const MIGRATED_SKILLS_PROJECT_SLUG: &str = "migrated-skills-v1";

const MIGRATED_SKILLS_CATALOG_JSON: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/migrated_skills_catalog.json"
));

#[derive(Debug, Deserialize)]
struct CatalogEntry {
    name: String,
    manifest: SkillManifest,
    prompt_content: String,
}

/// Result of a migration pass — surfaced at startup so the caller can
/// log a single line summarising how many catalog entries were
/// inserted, refreshed, or left untouched.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MigrationOutcome {
    pub scanned: u32,
    pub inserted: u32,
    pub refreshed: u32,
    pub unchanged: u32,
}

/// Persist the v1 → v2 skill migration into `store`.
///
/// Reads the embedded catalog (built by `build.rs`), produces a v2
/// `MemoryDoc` for each entry, and writes any entries that are missing
/// or whose `content_hash` differs from the embedded prompt body.
/// Existing entries authored by users (`source != Migrated`) are
/// left alone even if their hashes collide — the migration is a
/// one-shot transplant, not an audit pass.
pub(crate) async fn migrate_v1_skills_to_memory_docs(
    store: &dyn Store,
) -> Result<MigrationOutcome, EngineError> {
    let entries: Vec<CatalogEntry> = serde_json::from_str(MIGRATED_SKILLS_CATALOG_JSON)
        .map_err(|error| EngineError::Store {
            reason: format!(
                "failed to parse embedded migrated_skills catalog JSON: {error}"
            ),
        })?;

    let mut outcome = MigrationOutcome::default();
    for entry in &entries {
        outcome.scanned += 1;
        let doc_id = stable_doc_id(&entry.name);
        let project_id =
            ProjectId::from_slug(shared_owner_id(), MIGRATED_SKILLS_PROJECT_SLUG);

        let metadata = build_v2_metadata(entry);
        let expected_hash = metadata.content_hash.clone();

        match store.load_memory_doc(doc_id).await? {
            None => {
                let doc = assemble_memory_doc(
                    doc_id,
                    project_id,
                    entry,
                    metadata,
                );
                store.save_memory_doc(&doc).await?;
                outcome.inserted += 1;
            }
            Some(existing)
                if existing.content == entry.prompt_content
                    && metadata_hash_matches(&existing.metadata, &expected_hash) =>
            {
                outcome.unchanged += 1;
            }
            Some(existing) if is_user_owned(&existing) => {
                outcome.unchanged += 1;
            }
            Some(_) => {
                let doc = assemble_memory_doc(
                    doc_id,
                    project_id,
                    entry,
                    metadata,
                );
                store.save_memory_doc(&doc).await?;
                outcome.refreshed += 1;
            }
        }
    }
    Ok(outcome)
}

fn assemble_memory_doc(
    doc_id: DocId,
    project_id: ProjectId,
    entry: &CatalogEntry,
    metadata: V2SkillMetadata,
) -> MemoryDoc {
    // Capture now once so created_at and updated_at are identical for
    // newly-inserted docs (avoids a spurious microsecond-level divergence
    // caused by two separate Utc::now() calls).
    let now = chrono::Utc::now();
    // V2SkillMetadata only contains String, u32, Vec, and Option types —
    // serde_json serialization cannot fail in practice. The fallback is a
    // defensive guard against future additions of a custom Serialize impl.
    let metadata_json = serde_json::to_value(&metadata).unwrap_or_else(|error| {
        serde_json::Value::String(format!("<metadata encode failed: {error}>"))
    });
    MemoryDoc {
        id: doc_id,
        project_id,
        user_id: shared_owner_id().to_string(),
        doc_type: DocType::Skill,
        title: entry.name.clone(),
        content: entry.prompt_content.clone(),
        source_thread_id: None,
        tags: entry.manifest.activation.tags.clone(),
        metadata: metadata_json,
        created_at: now,
        updated_at: now,
    }
}

fn stable_doc_id(skill_name: &str) -> DocId {
    DocId(Uuid::new_v5(
        &MIGRATED_SKILL_DOC_NAMESPACE,
        skill_name.as_bytes(),
    ))
}

fn build_v2_metadata(entry: &CatalogEntry) -> V2SkillMetadata {
    let content_hash = format!("sha256:{}", sha256_hex(&entry.prompt_content));
    V2SkillMetadata {
        name: entry.name.clone(),
        version: 1,
        description: entry.manifest.description.clone(),
        activation: entry.manifest.activation.clone(),
        source: V2SkillSource::Migrated,
        requires: entry.manifest.requires.clone(),
        code_snippets: Vec::<CodeSnippet>::new(),
        metrics: SkillMetrics::default(),
        parent_version: None,
        revisions: Vec::<SkillRevision>::new(),
        repairs: Vec::<SkillRepairRecord>::new(),
        content_hash,
        bundle_path: None,
        source_url: None,
    }
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn metadata_hash_matches(
    existing: &serde_json::Value,
    expected_hash: &str,
) -> bool {
    existing
        .get("content_hash")
        .and_then(|h| h.as_str())
        == Some(expected_hash)
}

fn is_user_owned(doc: &MemoryDoc) -> bool {
    doc.metadata
        .get("source")
        .and_then(|s| s.as_str())
        .map(|s| s != "migrated")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory `Store` impl suitable for unit-testing the migration
    /// catalog without touching libsql. Tracks inserted vs refreshed
    /// records so the round-trip / idempotency assertions can verify
    /// the migration only does the work the outcome counts.
    #[derive(Default)]
    struct InMemoryStore {
        docs: std::sync::Mutex<Vec<MemoryDoc>>,
    }

    #[async_trait::async_trait]
    impl Store for InMemoryStore {
        async fn save_thread(
            &self,
            _: &brassclaw_engine::types::thread::Thread,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn load_thread(
            &self,
            _: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Option<brassclaw_engine::types::thread::Thread>, EngineError>
        {
            unimplemented!()
        }
        async fn list_threads(
            &self,
            _: ProjectId,
            _: &str,
        ) -> Result<Vec<brassclaw_engine::types::thread::Thread>, EngineError>
        {
            unimplemented!()
        }
        async fn update_thread_state(
            &self,
            _: brassclaw_engine::types::thread::ThreadId,
            _: brassclaw_engine::types::thread::ThreadState,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn save_step(
            &self,
            _: &brassclaw_engine::types::step::Step,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn load_steps(
            &self,
            _: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Vec<brassclaw_engine::types::step::Step>, EngineError> {
            unimplemented!()
        }
        async fn append_events(
            &self,
            _: &[brassclaw_engine::types::event::ThreadEvent],
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn load_events(
            &self,
            _: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Vec<brassclaw_engine::types::event::ThreadEvent>, EngineError>
        {
            unimplemented!()
        }
        async fn save_project(
            &self,
            _: &brassclaw_engine::types::project::Project,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn load_project(
            &self,
            _: ProjectId,
        ) -> Result<Option<brassclaw_engine::types::project::Project>, EngineError>
        {
            unimplemented!()
        }
        async fn save_memory_doc(
            &self,
            doc: &MemoryDoc,
        ) -> Result<(), EngineError> {
            self.docs.lock().expect("poisoned").push(doc.clone());
            Ok(())
        }
        async fn load_memory_doc(
            &self,
            id: DocId,
        ) -> Result<Option<MemoryDoc>, EngineError> {
            Ok(self
                .docs
                .lock()
                .expect("poisoned")
                .iter()
                .find(|d| d.id == id)
                .cloned())
        }
        async fn list_memory_docs(
            &self,
            project_id: ProjectId,
            user_id: &str,
        ) -> Result<Vec<MemoryDoc>, EngineError> {
            Ok(self
                .docs
                .lock()
                .expect("poisoned")
                .iter()
                .filter(|d| d.project_id == project_id && d.user_id == user_id)
                .cloned()
                .collect())
        }
        async fn list_memory_docs_by_owner(
            &self,
            user_id: &str,
        ) -> Result<Vec<MemoryDoc>, EngineError> {
            Ok(self
                .docs
                .lock()
                .expect("poisoned")
                .iter()
                .filter(|d| d.user_id == user_id)
                .cloned()
                .collect())
        }
        async fn save_lease(
            &self,
            _: &brassclaw_engine::types::capability::CapabilityLease,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn load_active_leases(
            &self,
            _: brassclaw_engine::types::thread::ThreadId,
        ) -> Result<Vec<brassclaw_engine::types::capability::CapabilityLease>, EngineError>
        {
            unimplemented!()
        }
        async fn revoke_lease(
            &self,
            _: brassclaw_engine::types::capability::LeaseId,
            _: &str,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn save_mission(
            &self,
            _: &brassclaw_engine::types::mission::Mission,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
        async fn load_mission(
            &self,
            _: brassclaw_engine::types::mission::MissionId,
        ) -> Result<Option<brassclaw_engine::types::mission::Mission>, EngineError> {
            unimplemented!()
        }
        async fn list_missions(
            &self,
            _: ProjectId,
            _: &str,
        ) -> Result<Vec<brassclaw_engine::types::mission::Mission>, EngineError> {
            unimplemented!()
        }
        async fn update_mission_status(
            &self,
            _: brassclaw_engine::types::mission::MissionId,
            _: brassclaw_engine::types::mission::MissionStatus,
        ) -> Result<(), EngineError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn migration_inserts_all_catalog_entries_on_empty_store() {
        let store = InMemoryStore::default();
        let outcome =
            migrate_v1_skills_to_memory_docs(&store).await.expect("migration");
        assert!(
            outcome.inserted > 0,
            "expected the embedded catalog to produce inserts, got {outcome:?}"
        );
        assert_eq!(outcome.refreshed, 0);
        assert_eq!(outcome.unchanged, 0);
        assert_eq!(
            outcome.inserted + outcome.unchanged + outcome.refreshed,
            outcome.scanned
        );

        let docs = store
            .list_memory_docs_by_owner(shared_owner_id())
            .await
            .expect("global list");
        for doc in &docs {
            assert_eq!(doc.doc_type, DocType::Skill);
            assert_eq!(doc.user_id, shared_owner_id());
            assert_eq!(
                doc.metadata.get("source").and_then(|v| v.as_str()),
                Some("migrated")
            );
        }
    }

    #[tokio::test]
    async fn migration_is_idempotent_on_unmodified_catalog() {
        let store = InMemoryStore::default();
        let first =
            migrate_v1_skills_to_memory_docs(&store).await.expect("first run");
        assert!(first.inserted > 0);
        let first_count = first.inserted;

        let second =
            migrate_v1_skills_to_memory_docs(&store).await.expect("second run");
        assert_eq!(second.inserted, 0);
        assert_eq!(second.refreshed, 0);
        assert_eq!(second.unchanged, first_count);
    }

    #[test]
    fn sha256_hex_is_deterministic_and_64_chars() {
        let h1 = sha256_hex("coding");
        let h2 = sha256_hex("coding");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn stable_doc_id_is_repeatable_per_name() {
        let a = stable_doc_id("coding");
        let b = stable_doc_id("coding");
        let c = stable_doc_id("commit");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn metadata_hash_matches_handles_missing_field() {
        assert!(!metadata_hash_matches(
            &serde_json::json!({}),
            "sha256:abc"
        ));
        assert!(metadata_hash_matches(
            &serde_json::json!({"content_hash": "sha256:abc"}),
            "sha256:abc"
        ));
        assert!(!metadata_hash_matches(
            &serde_json::json!({"content_hash": "sha256:zzz"}),
            "sha256:abc"
        ));
    }
}
