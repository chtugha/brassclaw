//! Skill telemetry stub.
//!
//! The v2 MemoryDoc-backed `SkillTracker` has been retired: `PgThreadEngineStore`
//! (the sole production `Store` implementation) stubs out `load_memory_doc` and
//! `save_memory_doc`, so the old tracker was always a silent no-op in production.
//! Skill usage telemetry is now owned by the `reborn_skills` DB columns
//! (`usage_count`, `success_count`, `failure_count`) and updated by the
//! Validator/reward path in `brassclaw_reborn_composition`.
//!
//! This module is retained as an empty shim so that callers of `SkillTracker`
//! in the orchestrator can be migrated without an atomic refactor.

/// A no-op skill telemetry shim.  Formerly updated `V2SkillMetadata` inside
/// `MemoryDoc.metadata`; that path is retired.  The `reborn_skills` table's
/// `usage_count` / `success_count` / `failure_count` columns are the live store.
pub struct SkillTracker;

impl SkillTracker {
    pub fn new() -> Self {
        Self
    }

    /// Record that a skill was used.  This is now a no-op: the production Store
    /// implementation never supplied writable MemoryDocs for this path, and the
    /// live telemetry is updated via `brassclaw_skills::db_store::DbSkillStore`.
    pub async fn record_usage(&self, _doc_id: uuid::Uuid, _success: bool) {
        // Intentionally empty — see module-level note.
    }
}

impl Default for SkillTracker {
    fn default() -> Self {
        Self::new()
    }
}
