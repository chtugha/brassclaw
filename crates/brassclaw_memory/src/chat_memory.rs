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
    /// `run_id` is the agent-loop `TurnRunId` for the run that triggered the
    /// `memory_write` call.  Always `None` at the capability dispatch layer
    /// (`ResourceScope` only carries `invocation_id`, not the `TurnRunId`).
    /// When a future integration exposes the `TurnRunId` at the dispatch layer,
    /// pass it here so `link_chat_record` can correlate the memory row with
    /// the forensic packet for that run.
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

    /// Update `source_ref` on an existing Path A row after Path B writes the
    /// chunk subtree (§4.30.1 step 5).  Best-effort — errors are swallowed.
    ///
    /// Default is a no-op so non-Postgres implementations do not need to
    /// implement this.
    async fn update_source_ref(&self, chat_record_id: &str, source_ref: &str) {
        let _ = (chat_record_id, source_ref);
    }
}
