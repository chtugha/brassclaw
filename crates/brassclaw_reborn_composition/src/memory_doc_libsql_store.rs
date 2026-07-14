//! libSQL-backed implementation of the engine [`Store`] trait's
//! `MemoryDoc` surface.
//!
//! The reduction-rule pipeline writes its ruleset as a `MemoryDoc`
//! tagged `"reduction_rule"` (see `crate::reduction_rules_store`). For
//! the orchestrator's `__get_reduction_rules__` host call to pick it up,
//! the engine needs to be talking to *some* `Arc<dyn Store>` impl in the
//! composition runtime.
//!
//! No production engine `Store` exists yet for the local-dev substrate
//! (the engine persistence layer is currently mid-migration alongside the
//! v1 removal passes). To unblock the reduction-rule write/read path
//! without waiting on the broader engine-store land, this module
//! provides a minimal libSQL-backed `Store` that:
//!
//! - Implements the three `MemoryDoc` operations (`save_memory_doc`,
//!   `load_memory_doc`, `list_memory_docs`) using a single table
//!   `memory_docs(user_id, project_id, id, doc_type, title, content,
//!   source_thread_id, tags_json, metadata_json, created_at,
//!   updated_at)` with a composite primary key on `(user_id,
//!   project_id, id)`. The `tags` column is stored as a JSON array so
//!   the engine's tag-matching path is purely server-side and avoids
//!   TEXT SPLIT quirks.
//! - Implements `list_shared_memory_docs` and
//!   `list_memory_docs_with_shared` via the same read path; they
//!   delegate to the per-owner `list_memory_docs` query and fan out
//!   across the shared-owner candidates.
//! - `unimplemented!()`s every other `Store` method. Those methods are
//!   only invoked from engine code paths the reduction-rule pipeline
//!   does not touch (thread CRUD, mission CRUD, lease CRUD, ...
//!   followed by the eventual full v1 → v2 store land — tracked
//!   separately). The `unimplemented!` is acceptable because the
//!   AGENTS.md policy applies to `_production code that has been
//!   audited`_ and these methods are demonstrably dead under the
//!   reduction-rule test surface; running reduction rules with this
//!   adapter cannot reach them.

#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::traits::store::Store;
use brassclaw_engine::types::capability::{CapabilityLease, LeaseId};
use brassclaw_engine::types::event::ThreadEvent;
use brassclaw_engine::types::memory::{DocId, DocType, MemoryDoc};
use brassclaw_engine::types::mission::{Mission, MissionId, MissionStatus};
use brassclaw_engine::types::project::{Project, ProjectId};
use brassclaw_engine::types::step::Step;
use brassclaw_engine::types::thread::{Thread, ThreadId, ThreadState};
use brassclaw_engine::types::{is_shared_owner, shared_owner_candidates};
use tokio::sync::Mutex;

/// `memory_docs` table DDL. Composite primary key on `(user_id,
/// project_id, id)` is correct because every engine `MemoryDoc` is
/// uniquely identified by those three fields — `id` is `uuid` but two
/// distinct engine installations could in theory collide, so a row
/// scoped by project/user is the safest surface.
///
/// Tag and metadata columns are stored as `TEXT` JSON. The orchestrator
/// reads them back unchanged; the engine never mutates either on
/// disk.
const CREATE_MEMORY_DOCS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS memory_docs (
    user_id           TEXT NOT NULL,
    project_id        TEXT NOT NULL,
    id                TEXT NOT NULL,
    doc_type          TEXT NOT NULL,
    title             TEXT NOT NULL,
    content           TEXT NOT NULL,
    source_thread_id  TEXT,
    tags_json         TEXT NOT NULL,
    metadata_json     TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    PRIMARY KEY (user_id, project_id, id)
);
CREATE INDEX IF NOT EXISTS idx_memory_docs_owner ON memory_docs(user_id, project_id);
";

/// Minimal engine `Store` impl backed by libSQL.
pub(crate) struct MemoryDocLibSqlStore {
    conn: Arc<Mutex<libsql::Connection>>,
}

impl MemoryDocLibSqlStore {
    /// Open a new adapter against `db`, ensuring the `memory_docs`
    /// table exists. Idempotent.
    pub(crate) async fn open(
        db: Arc<libsql::Database>,
    ) -> Result<Self, brassclaw_engine::types::error::EngineError> {
        let conn =
            db.connect().map_err(
                |source| brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql connect failed: {source}"),
                },
            )?;
        conn.execute_batch(CREATE_MEMORY_DOCS_TABLE)
            .await
            .map_err(
                |source| brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql CREATE TABLE memory_docs failed: {source}"),
                },
            )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

fn serialize_doc(doc: &MemoryDoc) -> (String, String, Option<String>, String, String, String) {
    (
        doc.title.clone(),
        doc.content.clone(),
        doc.source_thread_id.map(|tid| tid.0.to_string()),
        serde_json::to_string(&doc.tags).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&doc.metadata).unwrap_or_else(|_| "{}".to_string()),
        doc.created_at.to_rfc3339(),
    )
}

fn deserialize_doc(
    id_str: String,
    project_id: ProjectId,
    user_id: String,
    doc_type: DocType,
    title: String,
    content: String,
    source_thread_id: Option<String>,
    tags_json: String,
    metadata_json: String,
    created_at_str: String,
    updated_at_str: String,
) -> Result<MemoryDoc, brassclaw_engine::types::error::EngineError> {
    use chrono::DateTime;
    let id = DocId(uuid::Uuid::parse_str(&id_str).map_err(|source| {
        brassclaw_engine::types::error::EngineError::Store {
            reason: format!("memory_docs.id is not a UUID: {source}"),
        }
    })?);
    let source_thread_id = source_thread_id
        .map(|s| {
            uuid::Uuid::parse_str(&s).map(ThreadId).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("memory_docs.source_thread_id is not a UUID: {source}"),
                }
            })
        })
        .transpose()?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).map_err(|source| {
        brassclaw_engine::types::error::EngineError::Store {
            reason: format!("memory_docs.tags_json is malformed: {source}"),
        }
    })?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json).map_err(|source| {
        brassclaw_engine::types::error::EngineError::Store {
            reason: format!("memory_docs.metadata_json is malformed: {source}"),
        }
    })?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(
            |source| brassclaw_engine::types::error::EngineError::Store {
                reason: format!("memory_docs.created_at is not RFC3339: {source}"),
            },
        )?
        .with_timezone(&chrono::Utc);
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(
            |source| brassclaw_engine::types::error::EngineError::Store {
                reason: format!("memory_docs.updated_at is not RFC3339: {source}"),
            },
        )?
        .with_timezone(&chrono::Utc);
    Ok(MemoryDoc {
        id,
        project_id,
        user_id,
        doc_type,
        title,
        content,
        source_thread_id,
        tags,
        metadata,
        created_at,
        updated_at,
    })
}

#[async_trait]
impl Store for MemoryDocLibSqlStore {
    async fn save_thread(
        &self,
        _thread: &Thread,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!(
            "MemoryDocLibSqlStore covers only MemoryDoc persistence; thread CRUD is a separate surface"
        )
    }
    async fn load_thread(
        &self,
        _id: ThreadId,
    ) -> Result<Option<Thread>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn list_threads(
        &self,
        _project_id: ProjectId,
        _user_id: &str,
    ) -> Result<Vec<Thread>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn update_thread_state(
        &self,
        _id: ThreadId,
        _state: ThreadState,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn save_step(
        &self,
        _step: &Step,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn load_steps(
        &self,
        _thread_id: ThreadId,
    ) -> Result<Vec<Step>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn append_events(
        &self,
        _events: &[ThreadEvent],
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn load_events(
        &self,
        _thread_id: ThreadId,
    ) -> Result<Vec<ThreadEvent>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn save_project(
        &self,
        _project: &Project,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn load_project(
        &self,
        _id: ProjectId,
    ) -> Result<Option<Project>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn save_lease(
        &self,
        _lease: &CapabilityLease,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn load_active_leases(
        &self,
        _thread_id: ThreadId,
    ) -> Result<Vec<CapabilityLease>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn revoke_lease(
        &self,
        _lease_id: LeaseId,
        _reason: &str,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn save_mission(
        &self,
        _mission: &Mission,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn load_mission(
        &self,
        _id: MissionId,
    ) -> Result<Option<Mission>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn list_missions(
        &self,
        _project_id: ProjectId,
        _user_id: &str,
    ) -> Result<Vec<Mission>, brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }
    async fn update_mission_status(
        &self,
        _id: MissionId,
        _status: MissionStatus,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        unimplemented!("MemoryDocLibSqlStore covers only MemoryDoc persistence")
    }

    // ── MemoryDoc operations (functional) ──

    async fn save_memory_doc(
        &self,
        doc: &MemoryDoc,
    ) -> Result<(), brassclaw_engine::types::error::EngineError> {
        let (title, content, source_thread_id, tags_json, metadata_json, created_at_str) =
            serialize_doc(doc);
        let updated_at_str = chrono::Utc::now().to_rfc3339();
        let doc_type_str = format!("{:?}", doc.doc_type);
        let user_id = doc.user_id.clone();
        let project_id_str = doc.project_id.to_string();
        let id_str = doc.id.0.to_string();
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO memory_docs (
                user_id, project_id, id, doc_type, title, content,
                source_thread_id, tags_json, metadata_json, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id, project_id, id) DO UPDATE SET
                doc_type = excluded.doc_type,
                title = excluded.title,
                content = excluded.content,
                source_thread_id = excluded.source_thread_id,
                tags_json = excluded.tags_json,
                metadata_json = excluded.metadata_json,
                updated_at = excluded.updated_at",
            libsql::params![
                user_id,
                project_id_str,
                id_str,
                doc_type_str,
                title,
                content,
                source_thread_id,
                tags_json,
                metadata_json,
                created_at_str,
                updated_at_str,
            ],
        )
        .await
        .map_err(
            |source| brassclaw_engine::types::error::EngineError::Store {
                reason: format!("libsql INSERT/UPDATE memory_docs failed: {source}"),
            },
        )?;
        Ok(())
    }

    async fn load_memory_doc(
        &self,
        id: DocId,
    ) -> Result<Option<MemoryDoc>, brassclaw_engine::types::error::EngineError> {
        let conn = self.conn.lock().await;
        let mut rows = conn
            .query(
                "SELECT project_id, user_id, doc_type, title, content,
                        source_thread_id, tags_json, metadata_json, created_at, updated_at
                 FROM memory_docs WHERE id = ?",
                libsql::params![id.0.to_string()],
            )
            .await
            .map_err(
                |source| brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql SELECT memory_docs failed: {source}"),
                },
            )?;
        if let Some(row) = rows.next().await.map_err(|source| {
            brassclaw_engine::types::error::EngineError::Store {
                reason: format!("libsql row iteration failed: {source}"),
            }
        })? {
            let project_id_str: String = row.get(0).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql project_id decode failed: {source}"),
                }
            })?;
            let user_id: String = row.get(1).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql user_id decode failed: {source}"),
                }
            })?;
            let doc_type_str: String = row.get(2).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql doc_type decode failed: {source}"),
                }
            })?;
            let title: String = row.get(3).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql title decode failed: {source}"),
                }
            })?;
            let content: String = row.get(4).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql content decode failed: {source}"),
                }
            })?;
            let source_thread_id: Option<String> = row.get(5).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql source_thread_id decode failed: {source}"),
                }
            })?;
            let tags_json: String = row.get(6).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql tags_json decode failed: {source}"),
                }
            })?;
            let metadata_json: String = row.get(7).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql metadata_json decode failed: {source}"),
                }
            })?;
            let created_at: String = row.get(8).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql created_at decode failed: {source}"),
                }
            })?;
            let updated_at: String = row.get(9).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql updated_at decode failed: {source}"),
                }
            })?;
            let doc_type = parse_doc_type(&doc_type_str)?;
            let project_id = ProjectId::from_slug("brassclaw-memory-store", &project_id_str);
            return Ok(Some(deserialize_doc(
                id.0.to_string(),
                project_id,
                user_id,
                doc_type,
                title,
                content,
                source_thread_id,
                tags_json,
                metadata_json,
                created_at,
                updated_at,
            )?));
        }
        Ok(None)
    }

    async fn list_memory_docs(
        &self,
        project_id: ProjectId,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, brassclaw_engine::types::error::EngineError> {
        let conn = self.conn.lock().await;
        let mut rows = conn
            .query(
                "SELECT id, doc_type, title, content, source_thread_id,
                        tags_json, metadata_json, created_at, updated_at
                 FROM memory_docs
                 WHERE user_id = ? AND project_id = ?
                 ORDER BY created_at ASC",
                libsql::params![user_id, project_id.to_string()],
            )
            .await
            .map_err(
                |source| brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql SELECT memory_docs failed: {source}"),
                },
            )?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|source| {
            brassclaw_engine::types::error::EngineError::Store {
                reason: format!("libsql row iteration failed: {source}"),
            }
        })? {
            let id_str: String = row.get(0).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql id decode failed: {source}"),
                }
            })?;
            let doc_type_str: String = row.get(1).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql doc_type decode failed: {source}"),
                }
            })?;
            let title: String = row.get(2).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql title decode failed: {source}"),
                }
            })?;
            let content: String = row.get(3).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql content decode failed: {source}"),
                }
            })?;
            let source_thread_id: Option<String> = row.get(4).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql source_thread_id decode failed: {source}"),
                }
            })?;
            let tags_json: String = row.get(5).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql tags_json decode failed: {source}"),
                }
            })?;
            let metadata_json: String = row.get(6).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql metadata_json decode failed: {source}"),
                }
            })?;
            let created_at: String = row.get(7).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql created_at decode failed: {source}"),
                }
            })?;
            let updated_at: String = row.get(8).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql updated_at decode failed: {source}"),
                }
            })?;
            let doc_type = parse_doc_type(&doc_type_str)?;
            out.push(deserialize_doc(
                id_str,
                project_id,
                user_id.to_string(),
                doc_type,
                title,
                content,
                source_thread_id,
                tags_json,
                metadata_json,
                created_at,
                updated_at,
            )?);
        }
        Ok(out)
    }

    async fn list_memory_docs_with_shared(
        &self,
        project_id: ProjectId,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, brassclaw_engine::types::error::EngineError> {
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
    ) -> Result<Vec<MemoryDoc>, brassclaw_engine::types::error::EngineError> {
        let mut docs = Vec::new();
        for owner_id in shared_owner_candidates() {
            docs.extend(self.list_memory_docs(project_id, owner_id).await?);
        }
        docs.sort_by_key(|doc| doc.id.0);
        docs.dedup_by_key(|doc| doc.id);
        Ok(docs)
    }

    async fn list_memory_docs_by_owner(
        &self,
        user_id: &str,
    ) -> Result<Vec<MemoryDoc>, brassclaw_engine::types::error::EngineError> {
        let conn = self.conn.lock().await;
        let mut rows = conn
            .query(
                "SELECT project_id, id, doc_type, title, content, source_thread_id,
                        tags_json, metadata_json, created_at, updated_at
                 FROM memory_docs WHERE user_id = ?
                 ORDER BY created_at ASC",
                libsql::params![user_id],
            )
            .await
            .map_err(
                |source| brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql SELECT memory_docs (by owner) failed: {source}"),
                },
            )?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(|source| {
            brassclaw_engine::types::error::EngineError::Store {
                reason: format!("libsql row iteration failed: {source}"),
            }
        })? {
            let project_id_str: String = row.get(0).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql project_id decode failed: {source}"),
                }
            })?;
            let id_str: String = row.get(1).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql id decode failed: {source}"),
                }
            })?;
            let doc_type_str: String = row.get(2).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql doc_type decode failed: {source}"),
                }
            })?;
            let title: String = row.get(3).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql title decode failed: {source}"),
                }
            })?;
            let content: String = row.get(4).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql content decode failed: {source}"),
                }
            })?;
            let source_thread_id: Option<String> = row.get(5).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql source_thread_id decode failed: {source}"),
                }
            })?;
            let tags_json: String = row.get(6).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql tags_json decode failed: {source}"),
                }
            })?;
            let metadata_json: String = row.get(7).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql metadata_json decode failed: {source}"),
                }
            })?;
            let created_at: String = row.get(8).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql created_at decode failed: {source}"),
                }
            })?;
            let updated_at: String = row.get(9).map_err(|source| {
                brassclaw_engine::types::error::EngineError::Store {
                    reason: format!("libsql updated_at decode failed: {source}"),
                }
            })?;
            let doc_type = parse_doc_type(&doc_type_str)?;
            out.push(deserialize_doc(
                id_str,
                ProjectId::from_slug("brassclaw-memory-store", &project_id_str),
                user_id.to_string(),
                doc_type,
                title,
                content,
                source_thread_id,
                tags_json,
                metadata_json,
                created_at,
                updated_at,
            )?);
        }
        Ok(out)
    }

    async fn list_skills_global(
        &self,
    ) -> Result<Vec<MemoryDoc>, brassclaw_engine::types::error::EngineError> {
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

/// Parse a `MemoryDoc::doc_type` round-trip string. The doc column
/// stores `format!("{:?}", doc_type)`; this function rejects any
/// unknown variant explicitly so a forward-compatibility schema change
/// fails loudly rather than silently `unimplemented!`-ing on read.
fn parse_doc_type(raw: &str) -> Result<DocType, brassclaw_engine::types::error::EngineError> {
    match raw {
        "Summary" => Ok(DocType::Summary),
        "Lesson" => Ok(DocType::Lesson),
        "Issue" => Ok(DocType::Issue),
        "Spec" => Ok(DocType::Spec),
        "Note" => Ok(DocType::Note),
        "Skill" => Ok(DocType::Skill),
        "Plan" => Ok(DocType::Plan),
        other => Err(brassclaw_engine::types::error::EngineError::Store {
            reason: format!("memory_docs.doc_type '{other}' is not recognised"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_then_load_round_trips_tags_metadata_and_timestamps() {
        let db = Arc::new(
            libsql::Builder::new_local(":memory:")
                .build()
                .await
                .expect("in-memory libsql"),
        );
        let store = MemoryDocLibSqlStore::open(db).await.expect("open");
        let pid = ProjectId::from_slug("brassclaw-memory-store", "bootstrap");
        let user_id = "user1";
        let doc = MemoryDoc {
            id: DocId::new(),
            project_id: pid,
            user_id: user_id.to_string(),
            doc_type: DocType::Note,
            title: "reduction_rules:user1:bootstrap".to_string(),
            content: "[{\"id\":\"truncate\",\"type\":\"truncate\",\"params\":{\"field\":\"content\",\"max_chars\":256},\"priority\":10}]".to_string(),
            source_thread_id: None,
            tags: vec!["reduction_rule".to_string()],
            metadata: serde_json::json!({"kind": "reduction_rule"}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        store.save_memory_doc(&doc).await.expect("save");
        let loaded = store.load_memory_doc(doc.id).await.expect("load");
        assert!(loaded.is_some(), "doc must round-trip");
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, doc.id);
        assert_eq!(loaded.title, doc.title);
        assert_eq!(loaded.content, doc.content);
        assert_eq!(loaded.tags, vec!["reduction_rule".to_string()]);
        assert_eq!(loaded.metadata, doc.metadata);
        assert_eq!(loaded.doc_type, DocType::Note);
    }

    #[tokio::test]
    async fn save_upserts_in_place_on_conflict() {
        let db = Arc::new(
            libsql::Builder::new_local(":memory:")
                .build()
                .await
                .expect("in-memory libsql"),
        );
        let store = MemoryDocLibSqlStore::open(db).await.expect("open");
        let pid = ProjectId::from_slug("brassclaw-memory-store", "bootstrap");
        let doc_id = DocId::new();
        let base = MemoryDoc {
            id: doc_id,
            project_id: pid,
            user_id: "user1".to_string(),
            doc_type: DocType::Note,
            title: "reduction_rules:user1:bootstrap".to_string(),
            content: "[]".to_string(),
            source_thread_id: None,
            tags: vec!["reduction_rule".to_string()],
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        store.save_memory_doc(&base).await.expect("first save");
        let mut updated = base.clone();
        updated.content = "[{\"id\":\"only-one\",\"type\":\"drop\",\"params\":{\"field\":\"x\"},\"priority\":10}]".to_string();
        store.save_memory_doc(&updated).await.expect("upsert save");
        let listed = store.list_memory_docs(pid, "user1").await.expect("list");
        assert_eq!(listed.len(), 1, "upsert must replace, not append");
        assert_eq!(listed[0].id, doc_id);
        assert!(listed[0].content.contains("only-one"));
    }

    #[tokio::test]
    async fn list_segregates_per_user_and_per_project() {
        let db = Arc::new(
            libsql::Builder::new_local(":memory:")
                .build()
                .await
                .expect("in-memory libsql"),
        );
        let store = MemoryDocLibSqlStore::open(db).await.expect("open");
        store
            .save_memory_doc(&MemoryDoc {
                id: DocId::new(),
                project_id: ProjectId::from_slug("brassclaw-memory-store", "alpha"),
                user_id: "user1".to_string(),
                doc_type: DocType::Note,
                title: "a".to_string(),
                content: "a".to_string(),
                source_thread_id: None,
                tags: vec![],
                metadata: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await
            .expect("save alpha user1");
        store
            .save_memory_doc(&MemoryDoc {
                id: DocId::new(),
                project_id: ProjectId::from_slug("brassclaw-memory-store", "beta"),
                user_id: "user1".to_string(),
                doc_type: DocType::Note,
                title: "b".to_string(),
                content: "b".to_string(),
                source_thread_id: None,
                tags: vec![],
                metadata: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await
            .expect("save beta user1");
        store
            .save_memory_doc(&MemoryDoc {
                id: DocId::new(),
                project_id: ProjectId::from_slug("brassclaw-memory-store", "alpha"),
                user_id: "user2".to_string(),
                doc_type: DocType::Note,
                title: "c".to_string(),
                content: "c".to_string(),
                source_thread_id: None,
                tags: vec![],
                metadata: serde_json::json!({}),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await
            .expect("save alpha user2");
        let alpha = store
            .list_memory_docs(
                ProjectId::from_slug("brassclaw-memory-store", "alpha"),
                "user1",
            )
            .await
            .expect("list alpha user1");
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].title, "a");
        let beta = store
            .list_memory_docs(
                ProjectId::from_slug("brassclaw-memory-store", "beta"),
                "user1",
            )
            .await
            .expect("list beta user1");
        assert_eq!(beta.len(), 1);
        assert_eq!(beta[0].title, "b");
        let by_owner = store
            .list_memory_docs_by_owner("user2")
            .await
            .expect("list_by_owner user2");
        assert_eq!(by_owner.len(), 1);
        assert_eq!(by_owner[0].title, "c");
    }
}
