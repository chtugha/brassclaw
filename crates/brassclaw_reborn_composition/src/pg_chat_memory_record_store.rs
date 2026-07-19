//! `PgChatMemoryRecordStore` — Path A write for every `memory_write` call.
//!
//! On every `memory_write` dispatch:
//! 1. A new ULID is minted as `chat_record_id`.
//! 2. A row is inserted into `brassclaw_memory_chat_records` (V025).
//!    `source_ref` is NULL initially (updated by Path B after chunk write).
//! 3. `PgInterceptorStore::link_chat_record(run_id, iteration, chat_record_id)`
//!    is called best-effort to backfill the forensic packet cross-reference.
//! 4. `write_path_b_posthook` is called to update `source_ref` after chunk write.
//!
//! See §4.29, §7.4 (revision 17).

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_memory::{ChatMemoryWriterPort, MemoryDocumentScope};
use brassclaw_pg::PgPool;
use thiserror::Error;

use brassclaw_interceptor::PgInterceptorStore;

/// Errors from the chat-memory record store.
#[derive(Debug, Error)]
pub(crate) enum ChatMemoryRecordError {
    #[error("pool error: {reason}")]
    Pool { reason: String },
    #[error("database error: {reason}")]
    Database { reason: String },
}

fn map_pool(e: deadpool_postgres::PoolError) -> ChatMemoryRecordError {
    ChatMemoryRecordError::Pool { reason: e.to_string() }
}

fn map_pg(e: tokio_postgres::Error) -> ChatMemoryRecordError {
    ChatMemoryRecordError::Database { reason: e.to_string() }
}

/// Input for a single Path A chat-memory write.
pub(crate) struct ChatMemoryRecordInput {
    pub tenant_id: String,
    pub user_id: String,
    pub project_id: Option<String>,
    pub agent_id: Option<String>,
    pub session_thread_id: Option<String>,
    pub run_id: Option<String>,
    pub iteration: Option<u32>,
    pub kind: String,
    pub content: String,
    pub summary: Option<String>,
    /// Optional back-reference to the forensic packet for this turn.
    pub forensic_packet_id: Option<String>,
}

/// PostgreSQL-backed Path A chat-memory record store.
///
/// Wired by `build_backend_production` in `factory.rs` when a pool is available.
pub(crate) struct PgChatMemoryRecordStore {
    pool: Arc<PgPool>,
    interceptor_store: Arc<PgInterceptorStore>,
}

impl PgChatMemoryRecordStore {
    pub(crate) fn new(pool: Arc<PgPool>, interceptor_store: Arc<PgInterceptorStore>) -> Self {
        Self { pool, interceptor_store }
    }

    /// Write a Path A chat-memory record.
    ///
    /// Returns the minted `chat_record_id` so the caller can pass it to the
    /// Path B chunk indexer (`indexer.index_content(..., Some(chat_record_id))`).
    pub(crate) async fn write_record(
        &self,
        input: &ChatMemoryRecordInput,
    ) -> Result<String, ChatMemoryRecordError> {
        let id = ulid_str();
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_memory_chat_records \
                 (id, tenant_id, user_id, project_id, agent_id, session_thread_id, \
                  run_id, kind, content, summary, forensic_packet_id) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                &[
                    &id,
                    &input.tenant_id,
                    &input.user_id,
                    &input.project_id,
                    &input.agent_id,
                    &input.session_thread_id,
                    &input.run_id,
                    &input.kind,
                    &input.content,
                    &input.summary,
                    &input.forensic_packet_id,
                ],
            )
            .await
            .map_err(map_pg)?;

        // Best-effort: link this chat record to the forensic packet for this run/iteration.
        if let (Some(run_id), Some(iteration)) = (&input.run_id, input.iteration) {
            let link_result = self
                .interceptor_store
                .link_chat_record(run_id, iteration, &id)
                .await;
            if let Err(err) = link_result {
                tracing::debug!(
                    chat_record_id = %id,
                    run_id = %run_id,
                    iteration = %iteration,
                    error = %err,
                    "link_chat_record best-effort update failed"
                );
            }
        }

        Ok(id)
    }

    /// Path B post-hook: update `source_ref` after chunk rows are written.
    ///
    /// `source_ref` is the canonical VFS path of the chunk subtree for this
    /// chat record (e.g. `/memory/chat/<chat_record_id>`).
    ///
    /// The WHERE clause uses only `id` because ULIDs are globally unique —
    /// the additional `tenant_id` filter is unnecessary and the tenant context
    /// is not available from the `ChatMemoryWriterPort` trait call site.
    async fn set_source_ref(
        &self,
        chat_record_id: &str,
        source_ref: &str,
    ) -> Result<(), ChatMemoryRecordError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE brassclaw_memory_chat_records \
                 SET source_ref = $2 \
                 WHERE id = $1",
                &[&chat_record_id, &source_ref],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }
}

/// Mint a new ULID string.
///
/// ULIDs are 26-character Crockford base32 strings that sort lexicographically
/// by creation time, making them suitable as primary keys in time-ordered
/// tables.  The `chat_record_id` is used as the chunk subtree path
/// `/memory/chat/<chat_record_id>` so time-sortability is important.
fn ulid_str() -> String {
    ulid::Ulid::new().to_string()
}

// ---------------------------------------------------------------------------
// ChatMemoryWriterPort implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ChatMemoryWriterPort for PgChatMemoryRecordStore {
    async fn write_chat_memory_record(
        &self,
        scope: &MemoryDocumentScope,
        kind: &str,
        content: &str,
        run_id: Option<&str>,
        iteration: Option<u32>,
    ) -> Option<String> {
        let input = ChatMemoryRecordInput {
            tenant_id: scope.tenant_id().to_string(),
            user_id: scope.user_id().to_string(),
            project_id: scope.project_id().map(str::to_string),
            agent_id: scope.agent_id().map(str::to_string),
            session_thread_id: None,
            run_id: run_id.map(str::to_string),
            iteration,
            kind: kind.to_string(),
            content: content.to_string(),
            summary: None,
            forensic_packet_id: None,
        };
        match self.write_record(&input).await {
            Ok(id) => Some(id),
            Err(err) => {
                tracing::debug!(
                    error = %err,
                    kind = %kind,
                    "Path A chat-memory record write failed (best-effort)"
                );
                None
            }
        }
    }

    async fn update_source_ref(&self, chat_record_id: &str, source_ref: &str) {
        if let Err(err) = self.set_source_ref(chat_record_id, source_ref).await {
            tracing::debug!(
                chat_record_id = %chat_record_id,
                error = %err,
                "update_source_ref best-effort update failed"
            );
        }
    }
}
