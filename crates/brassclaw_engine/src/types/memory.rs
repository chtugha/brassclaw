//! Memory documents — the unit of durable knowledge.
//!
//! Memory docs are structured knowledge produced by reflection on completed
//! threads. They are project-scoped and used for context building (retrieval,
//! not replay of raw history).
//!
//! # Migration note
//!
//! [`DocType`] and [`MemoryDoc`] are **legacy types** being retired in favour
//! of the class-based component tables (V027–V043, `reborn_skills`, `reborn_specs`,
//! etc.). New code should use [`crate::memory::ComponentItem`] and
//! [`crate::memory::RetrievalSource`] instead.
//!
//! [`DocType`] is kept for the DB-less / legacy `Store`-backed path
//! until the `Store` trait is retired (PG-8 / Phase 6 cleanup).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::project::ProjectId;
use crate::types::thread::ThreadId;

use super::{OwnerId, default_user_id};

/// Strongly-typed document identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocId(pub Uuid);

impl DocId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DocId {
    fn default() -> Self {
        Self::new()
    }
}

/// The kind of knowledge a memory document captures.
///
/// # Deprecated
///
/// `DocType` is being retired in favour of the integer `class_code` field on
/// the new component tables (V027–V043). Use [`crate::memory::ComponentItem::class_code`]
/// and [`crate::memory::intent_system::class_label`] for new code.
/// This type is retained for the DB-less `MemoryDoc` / `Store` legacy path.
#[allow(deprecated)]
#[deprecated(
    since = "0.1.0",
    note = "Use class_code (i32) from ComponentItem / component tables instead."
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocType {
    /// What a thread accomplished.
    Summary,
    /// Durable learning from experience.
    Lesson,
    /// Detected problem for follow-up.
    Issue,
    /// Missing capability request.
    Spec,
    /// Working memory / scratch notes.
    Note,
    /// Reusable skill with activation metadata and optional code snippets.
    Skill,
    /// Structured execution plan with steps, status, and progress tracking.
    Plan,
    /// Cookbook entry linking a trigger to an ordered sequence of tool-skill
    /// invocations (Phase 7 Recipe-Skill-Tool learning pipeline).
    Recipe,
    /// Tight description of ONE tool usage pattern (mid-layer of the
    /// Recipe-Skill-Tool hierarchy; token-cheap alternative to Skill blobs).
    ToolSkill,
}

/// A memory document — structured durable knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDoc {
    pub id: DocId,
    pub project_id: ProjectId,
    /// Tenant isolation: the user who owns this document.
    #[serde(default = "default_user_id")]
    pub user_id: String,
    pub doc_type: DocType,
    pub title: String,
    pub content: String,
    pub source_thread_id: Option<ThreadId>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MemoryDoc {
    pub fn new(
        project_id: ProjectId,
        user_id: impl Into<String>,
        doc_type: DocType,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: DocId::new(),
            project_id,
            user_id: user_id.into(),
            doc_type,
            title: title.into(),
            content: content.into(),
            source_thread_id: None,
            tags: Vec::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_source_thread(mut self, thread_id: ThreadId) -> Self {
        self.source_thread_id = Some(thread_id);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn owner_id(&self) -> OwnerId<'_> {
        OwnerId::from_user_id(&self.user_id)
    }

    pub fn is_owned_by(&self, user_id: &str) -> bool {
        self.owner_id().matches_user(user_id)
    }
}
