//! Chat-memory record writer seam (Path A — §4.29, §7.4 revision 17).
//!
//! Defines the port that `BuiltinFirstPartyTools` calls on every
//! `memory_write` dispatch.  The production implementation lives in
//! `brassclaw_reborn_composition::PgChatMemoryRecordStore`; tests and
//! non-Postgres runtimes use no-op implementations.

use async_trait::async_trait;

use crate::path::MemoryDocumentScope;

/// Port for writing Path A chat-memory records.
///
/// Called unconditionally on every `memory_write` dispatch.  Errors are
/// swallowed best-effort by the caller so a failing store never blocks the
/// capability write.
#[async_trait]
pub trait ChatMemoryWriterPort: Send + Sync {
    /// Record a chat-memory entry and return the minted `chat_record_id`.
    ///
    /// `run_id` is the agent-loop turn run identifier (from `TurnRunId`).
    /// It is optional because the caller may not have the run ID in scope
    /// (e.g. when called from the capability dispatch layer which only
    /// carries an `invocation_id`).  When supplied, the row's `run_id` column
    /// is populated so `link_chat_record` can correlate the memory record with
    /// the forensic packet for the same run.
    ///
    /// `iteration` is the prompt-assembly iteration counter within the run.
    /// Also optional for the same reasons.
    ///
    /// Returns `None` when the write fails so callers can skip Path B
    /// source-ref linking without propagating the error.
    async fn write_chat_memory_record(
        &self,
        scope: &MemoryDocumentScope,
        kind: &str,
        content: &str,
        run_id: Option<&str>,
        iteration: Option<u32>,
    ) -> Option<String>;
}
