//! Postgres-backed engine [`Store`] implementation (MemoryDoc surface only).
//!
//! Implements the three `MemoryDoc` operations (`save_memory_doc`,
//! `load_memory_doc`, `list_memory_docs`) using `brassclaw_memory_docs` (V016).
//!
//! All other `Store` methods return `EngineError::Store` — same contract as
//! `MemoryDocLibSqlStore`. Phase 5 factory wiring substitutes this for
//! `MemoryDocLibSqlStore` without changing any other code path.
//!
//! Tags are stored as a Postgres `TEXT[]` array column (not JSON-encoded TEXT),
//! which is a schema improvement over the libSQL variant.

#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::traits::store::Store;
use brassclaw_engine::types::capability::{CapabilityLease, LeaseId};
use brassclaw_engine::types::error::EngineError;
use brassclaw_engine::types::event::ThreadEvent;
use brassclaw_engine::types::memory::{DocId, DocType, MemoryDoc};
use brassclaw_engine::types::mission::{Mission, MissionId, MissionStatus};
use brassclaw_engine::types::project::{Project, ProjectId};
use brassclaw_engine::types::step::Step;
use brassclaw_engine::types::thread::{Thread, ThreadId as EngineThreadId, ThreadState};
use brassclaw_engine::types::{is_shared_owner, shared_owner_candidates};
use brassclaw_pg::PgPool;

fn map_pool(e: deadpool_postgres::PoolError) -> EngineError {
    EngineError::Store {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> EngineError {
    EngineError::Store {
        reason: e.to_string(),
    }
}

fn stub(method: &'static str) -> EngineError {
    EngineError::Store {
        reason: format!("{method} is not implemented (PgMemoryDocStore — MemoryDoc-only adapter)"),
    }
}

fn doc_type_to_str(dt: DocType) -> &'static str {
    match dt {
        DocType::Summary => "Summary",
        DocType::Lesson => "Lesson",
        DocType::Issue => "Issue",
        DocType::Spec => "Spec",
        DocType::Note => "Note",
        DocType::Skill => "Skill",
        DocType::Plan => "Plan",
        DocType::Recipe => "Recipe",
        DocType::ToolSkill => "ToolSkill",
    }
}

fn parse_doc_type(raw: &str) -> Result<DocType, EngineError> {
    match raw {
        "Summary" => Ok(DocType::Summary),
        "Lesson" => Ok(DocType::Lesson),
        "Issue" => Ok(DocType::Issue),
        "Spec" => Ok(DocType::Spec),
        "Note" => Ok(DocType::Note),
        "Skill" => Ok(DocType::Skill),
        "Plan" => Ok(DocType::Plan),
        "Recipe" => Ok(DocType::Recipe),
        "ToolSkill" => Ok(DocType::ToolSkill),
        other => Err(EngineError::Store {
            reason: format!("brassclaw_memory_docs.doc_type '{other}' is not recognised"),
        }),
    }
}

/// Intermediate row struct to avoid a too-many-arguments helper.
struct MemoryDocRow {
    id_str: String,
    project_id_str: String,
    user_id: String,
    doc_type_str: String,
    title: String,
    content: String,
    source_thread_id: Option<String>,
    tags: Vec<String>,
    metadata_json: String,
    created_at_str: String,
    updated_at_str: String,
}

impl MemoryDocRow {
    fn from_pg_row(r: &tokio_postgres::Row) -> Self {
        Self {
            id_str: r.get(0),
            user_id: r.get(1),
            project_id_str: r.get(2),
            doc_type_str: r.get(3),
            title: r.get(4),
            content: r.get(5),
            source_thread_id: r.get(6),
            tags: r.get(7),
            metadata_json: r.get(8),
            created_at_str: r.get(9),
            updated_at_str: r.get(10),
        }
    }
}

/// Postgres-backed engine `Store` (MemoryDoc surface only).
pub(crate) struct PgMemoryDocStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgMemoryDocStore {
    pub(crate) fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    fn row_to_doc(&self, raw: MemoryDocRow) -> Result<MemoryDoc, EngineError> {
        use chrono::DateTime;
        let id = DocId(
            uuid::Uuid::parse_str(&raw.id_str).map_err(|e| EngineError::Store {
                reason: format!("brassclaw_memory_docs.id is not UUID: {e}"),
            })?,
        );
        let source_thread_id = raw
            .source_thread_id
            .map(|s| {
                uuid::Uuid::parse_str(&s)
                    .map(brassclaw_engine::types::thread::ThreadId)
                    .map_err(|e| EngineError::Store {
                        reason: format!("brassclaw_memory_docs.source_thread_id is not UUID: {e}"),
                    })
            })
            .transpose()?;
        let metadata: serde_json::Value =
            serde_json::from_str(&raw.metadata_json).map_err(|e| EngineError::Store {
                reason: format!("brassclaw_memory_docs.metadata malformed: {e}"),
            })?;
        let created_at = DateTime::parse_from_rfc3339(&raw.created_at_str)
            .map_err(|e| EngineError::Store {
                reason: format!("created_at parse: {e}"),
            })?
            .with_timezone(&chrono::Utc);
        let updated_at = DateTime::parse_from_rfc3339(&raw.updated_at_str)
            .map_err(|e| EngineError::Store {
                reason: format!("updated_at parse: {e}"),
            })?
            .with_timezone(&chrono::Utc);
        Ok(MemoryDoc {
            id,
            project_id: ProjectId::from_slug("brassclaw-memory-store", &raw.project_id_str),
            user_id: raw.user_id,
            doc_type: parse_doc_type(&raw.doc_type_str)?,
            title: raw.title,
            content: raw.content,
            source_thread_id,
            tags: raw.tags,
            metadata,
            created_at,
            updated_at,
        })
    }
}

#[async_trait]
impl Store for PgMemoryDocStore {
    // ── Non-MemoryDoc methods return stub errors (same as MemoryDocLibSqlStore) ──

    async fn save_thread(&self, _: &Thread) -> Result<(), EngineError> {
        Err(stub("save_thread"))
    }
    async fn load_thread(&self, _: EngineThreadId) -> Result<Option<Thread>, EngineError> {
        Err(stub("load_thread"))
    }
    async fn list_threads(&self, _: ProjectId, _: &str) -> Result<Vec<Thread>, EngineError> {
        Err(stub("list_threads"))
    }
    async fn update_thread_state(
        &self,
        _: EngineThreadId,
        _: ThreadState,
    ) -> Result<(), EngineError> {
        Err(stub("update_thread_state"))
    }
    async fn save_step(&self, _: &Step) -> Result<(), EngineError> {
        Err(stub("save_step"))
    }
    async fn load_steps(&self, _: EngineThreadId) -> Result<Vec<Step>, EngineError> {
        Err(stub("load_steps"))
    }
    async fn append_events(&self, _: &[ThreadEvent]) -> Result<(), EngineError> {
        Err(stub("append_events"))
    }
    async fn load_events(&self, _: EngineThreadId) -> Result<Vec<ThreadEvent>, EngineError> {
        Err(stub("load_events"))
    }
    async fn save_project(&self, _: &Project) -> Result<(), EngineError> {
        Err(stub("save_project"))
    }
    async fn load_project(&self, _: ProjectId) -> Result<Option<Project>, EngineError> {
        Err(stub("load_project"))
    }
    async fn save_lease(&self, _: &CapabilityLease) -> Result<(), EngineError> {
        Err(stub("save_lease"))
    }
    async fn load_active_leases(
        &self,
        _: EngineThreadId,
    ) -> Result<Vec<CapabilityLease>, EngineError> {
        Err(stub("load_active_leases"))
    }
    async fn revoke_lease(&self, _: LeaseId, _: &str) -> Result<(), EngineError> {
        Err(stub("revoke_lease"))
    }
    async fn save_mission(&self, _: &Mission) -> Result<(), EngineError> {
        Err(stub("save_mission"))
    }
    async fn load_mission(&self, _: MissionId) -> Result<Option<Mission>, EngineError> {
        Err(stub("load_mission"))
    }
    async fn list_missions(&self, _: ProjectId, _: &str) -> Result<Vec<Mission>, EngineError> {
        Err(stub("list_missions"))
    }
    async fn update_mission_status(
        &self,
        _: MissionId,
        _: MissionStatus,
    ) -> Result<(), EngineError> {
        Err(stub("update_mission_status"))
    }

    // ── MemoryDoc operations (functional) ──

    async fn save_memory_doc(&self, doc: &MemoryDoc) -> Result<(), EngineError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let id_str = doc.id.0.to_string();
        let project_id_str = doc.project_id.to_string();
        let doc_type_str = doc_type_to_str(doc.doc_type);
        let source_thread_id = doc.source_thread_id.map(|t| t.0.to_string());
        let metadata_json =
            serde_json::to_string(&doc.metadata).unwrap_or_else(|_| "{}".to_string());
        let created_at_str = doc.created_at.to_rfc3339();
        let tags: Vec<String> = doc.tags.clone();
        client
            .execute(
                "INSERT INTO brassclaw_memory_docs \
                 (id, tenant_id, user_id, project_id, doc_type, title, content, \
                  source_thread_id, tags, metadata, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb, $11, now()) \
                 ON CONFLICT (tenant_id, user_id, project_id, id) DO UPDATE \
                 SET doc_type = excluded.doc_type, \
                     title = excluded.title, \
                     content = excluded.content, \
                     source_thread_id = excluded.source_thread_id, \
                     tags = excluded.tags, \
                     metadata = excluded.metadata, \
                     updated_at = now()",
                &[
                    &id_str,
                    &self.tenant_id,
                    &doc.user_id,
                    &project_id_str,
                    &doc_type_str,
                    &doc.title,
                    &doc.content,
                    &source_thread_id,
                    &tags,
                    &metadata_json,
                    &created_at_str,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn load_memory_doc(&self, id: DocId) -> Result<Option<MemoryDoc>, EngineError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let id_str = id.0.to_string();
        let row = client
            .query_opt(
                "SELECT id, user_id, project_id, doc_type, title, content, \
                        source_thread_id, tags, metadata::text, \
                        created_at::text, updated_at::text \
                 FROM brassclaw_memory_docs \
                 WHERE tenant_id = $1 AND id = $2",
                &[&self.tenant_id, &id_str],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let doc = self.row_to_doc(MemoryDocRow::from_pg_row(&r))?;
                Ok(Some(doc))
            }
        }
    }

    async fn list_memory_docs(
        &self,
        project_id: ProjectId,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, EngineError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let project_id_str = project_id.to_string();
        let rows = client
            .query(
                "SELECT id, user_id, project_id, doc_type, title, content, \
                        source_thread_id, tags, metadata::text, \
                        created_at::text, updated_at::text \
                 FROM brassclaw_memory_docs \
                 WHERE tenant_id = $1 AND user_id = $2 AND project_id = $3 \
                 ORDER BY created_at ASC",
                &[&self.tenant_id, &user_id, &project_id_str],
            )
            .await
            .map_err(map_pg)?;
        rows.into_iter()
            .map(|r| self.row_to_doc(MemoryDocRow::from_pg_row(&r)))
            .collect()
    }

    async fn list_memory_docs_with_shared(
        &self,
        project_id: ProjectId,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, EngineError> {
        if is_shared_owner(user_id) {
            return self.list_shared_memory_docs(project_id).await;
        }
        let mut docs = self.list_memory_docs(project_id, user_id).await?;
        docs.extend(self.list_shared_memory_docs(project_id).await?);
        Ok(docs)
    }

    async fn list_shared_memory_docs(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<MemoryDoc>, EngineError> {
        let mut docs = Vec::new();
        for owner_id in shared_owner_candidates() {
            docs.extend(self.list_memory_docs(project_id, owner_id).await?);
        }
        docs.sort_by_key(|d| d.id.0);
        docs.dedup_by_key(|d| d.id);
        Ok(docs)
    }

    async fn list_memory_docs_by_owner(
        &self,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, EngineError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT id, user_id, project_id, doc_type, title, content, \
                        source_thread_id, tags, metadata::text, \
                        created_at::text, updated_at::text \
                 FROM brassclaw_memory_docs \
                 WHERE tenant_id = $1 AND user_id = $2 \
                 ORDER BY created_at ASC",
                &[&self.tenant_id, &user_id],
            )
            .await
            .map_err(map_pg)?;
        rows.into_iter()
            .map(|r| self.row_to_doc(MemoryDocRow::from_pg_row(&r)))
            .collect()
    }

    async fn list_skills_global(&self) -> Result<Vec<MemoryDoc>, EngineError> {
        let mut docs = Vec::new();
        for owner_id in shared_owner_candidates() {
            docs.extend(self.list_memory_docs_by_owner(owner_id).await?);
        }
        docs.retain(|d| d.doc_type == DocType::Skill);
        let mut seen = std::collections::HashSet::new();
        docs.retain(|d| seen.insert(d.id));
        Ok(docs)
    }
}
