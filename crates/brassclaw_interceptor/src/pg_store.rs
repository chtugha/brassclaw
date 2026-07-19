//! PostgreSQL-backed [`InterceptorStore`].
//!
//! Persists `ForensicPacket`s to `brassclaw_forensic_packets` (V026).
//!
//! # Storage contract
//!
//! - `save` is an upsert keyed on `packet.id`.
//! - `get` returns `None` for unknown ids.
//! - `list_recent` returns at most `limit` packets ordered by `captured_at DESC`.
//! - `link_chat_record` retroactively writes `chat_record_id` for the first
//!   chat-memory record produced by a given `(run_id, iteration)`.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_pg::PgPool;

use crate::error::InterceptorError;
use crate::packet::{
    CapturedPrompt, ForensicPacket, KohaiUsage, PacketId, PacketStatus, SempaiReviewOutcome,
};
use crate::store::InterceptorStore;

fn map_pool(error: deadpool_postgres::PoolError) -> InterceptorError {
    InterceptorError::StoreUnavailable { reason: error.to_string() }
}

fn map_pg(error: tokio_postgres::Error) -> InterceptorError {
    InterceptorError::Internal { reason: error.to_string() }
}

fn map_json(error: serde_json::Error) -> InterceptorError {
    InterceptorError::Internal { reason: format!("json: {error}") }
}

/// PostgreSQL-backed persistence for [`ForensicPacket`]s.
///
/// Wired by `brassclaw_reborn_composition::factory` in the production path.
/// Replaces `NoopInterceptorStore`.
pub struct PgInterceptorStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgInterceptorStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self { pool, tenant_id: tenant_id.into() }
    }

    /// Retroactively associate a `chat_record_id` with the first memory record
    /// produced by `(run_id, iteration)`.  Called by `PgChatMemoryRecordStore`
    /// after Path A write so the forensic packet can join to chat-memory rows.
    pub async fn link_chat_record(
        &self,
        run_id: &str,
        iteration: u32,
        chat_record_id: &str,
    ) -> Result<(), InterceptorError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE brassclaw_forensic_packets \
                 SET chat_record_id = $3 \
                 WHERE tenant_id = $1 \
                 AND run_id = $2 \
                 AND iteration = $4 \
                 AND chat_record_id IS NULL",
                &[&self.tenant_id, &run_id, &chat_record_id, &(iteration as i32)],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }
}

#[async_trait]
impl InterceptorStore for PgInterceptorStore {
    async fn save(&self, packet: &ForensicPacket) -> Result<(), InterceptorError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let status = packet_status_str(packet.status);
        let prompt_json = serde_json::to_value(&packet.prompt).map_err(map_json)?;
        let sempai_review_json = packet
            .sempai_review
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(map_json)?;
        let (input_tokens, output_tokens, cache_read, cache_create) =
            unpack_usage(packet.kohai_usage);
        client
            .execute(
                "INSERT INTO brassclaw_forensic_packets \
                 (id, tenant_id, run_id, iteration, status, captured_at, completed_at, \
                  prompt, kohai_response, \
                  kohai_input_tokens, kohai_output_tokens, \
                  kohai_cache_read_input_tokens, kohai_cache_creation_input_tokens, \
                  sempai_review) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
                 ON CONFLICT (id) DO UPDATE SET \
                  status = excluded.status, \
                  completed_at = excluded.completed_at, \
                  kohai_response = excluded.kohai_response, \
                  kohai_input_tokens = excluded.kohai_input_tokens, \
                  kohai_output_tokens = excluded.kohai_output_tokens, \
                  kohai_cache_read_input_tokens = excluded.kohai_cache_read_input_tokens, \
                  kohai_cache_creation_input_tokens = excluded.kohai_cache_creation_input_tokens, \
                  sempai_review = excluded.sempai_review, \
                  updated_at = now()",
                &[
                    &packet.id.as_str(),
                    &self.tenant_id,
                    &packet.run_id,
                    &(packet.iteration as i32),
                    &status,
                    &packet.captured_at,
                    &packet.completed_at,
                    &prompt_json,
                    &packet.kohai_response,
                    &input_tokens,
                    &output_tokens,
                    &cache_read,
                    &cache_create,
                    &sempai_review_json,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(())
    }

    async fn get(&self, packet_id: &PacketId) -> Result<Option<ForensicPacket>, InterceptorError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT id, run_id, iteration, status, captured_at, completed_at, \
                        prompt, kohai_response, \
                        kohai_input_tokens, kohai_output_tokens, \
                        kohai_cache_read_input_tokens, kohai_cache_creation_input_tokens, \
                        sempai_review \
                 FROM brassclaw_forensic_packets \
                 WHERE id = $1 AND tenant_id = $2",
                &[&packet_id.as_str(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        rows.first().map(row_to_packet).transpose()
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<ForensicPacket>, InterceptorError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT id, run_id, iteration, status, captured_at, completed_at, \
                        prompt, kohai_response, \
                        kohai_input_tokens, kohai_output_tokens, \
                        kohai_cache_read_input_tokens, kohai_cache_creation_input_tokens, \
                        sempai_review \
                 FROM brassclaw_forensic_packets \
                 WHERE tenant_id = $1 \
                 ORDER BY captured_at DESC \
                 LIMIT $2",
                &[&self.tenant_id, &(limit as i64)],
            )
            .await
            .map_err(map_pg)?;
        rows.iter().map(row_to_packet).collect()
    }
}

fn packet_status_str(status: PacketStatus) -> &'static str {
    match status {
        PacketStatus::AwaitingKohai => "awaiting_kohai",
        PacketStatus::Complete => "complete",
        PacketStatus::SempaiReviewed => "sempai_reviewed",
    }
}

fn parse_packet_status(s: &str) -> Result<PacketStatus, InterceptorError> {
    match s {
        "awaiting_kohai" => Ok(PacketStatus::AwaitingKohai),
        "complete" => Ok(PacketStatus::Complete),
        "sempai_reviewed" => Ok(PacketStatus::SempaiReviewed),
        other => Err(InterceptorError::Internal {
            reason: format!("unknown packet status: {other}"),
        }),
    }
}

fn unpack_usage(usage: Option<KohaiUsage>) -> (Option<i32>, Option<i32>, Option<i32>, Option<i32>) {
    match usage {
        Some(u) => (
            Some(u.input_tokens as i32),
            Some(u.output_tokens as i32),
            Some(u.cache_read_input_tokens as i32),
            Some(u.cache_creation_input_tokens as i32),
        ),
        None => (None, None, None, None),
    }
}

fn row_to_packet(row: &tokio_postgres::Row) -> Result<ForensicPacket, InterceptorError> {
    let status_str: String = row.get(3);
    let status = parse_packet_status(&status_str)?;
    let prompt_json: serde_json::Value = row.get(6);
    let prompt: CapturedPrompt = serde_json::from_value(prompt_json).map_err(map_json)?;
    let sempai_review_json: Option<serde_json::Value> = row.get(12);
    let sempai_review: Option<SempaiReviewOutcome> = sempai_review_json
        .map(serde_json::from_value)
        .transpose()
        .map_err(map_json)?;
    let input_tokens: Option<i32> = row.get(8);
    let output_tokens: Option<i32> = row.get(9);
    let cache_read: Option<i32> = row.get(10);
    let cache_create: Option<i32> = row.get(11);
    let kohai_usage = match (input_tokens, output_tokens, cache_read, cache_create) {
        (Some(i), Some(o), Some(cr), Some(cc)) => Some(KohaiUsage {
            input_tokens: i as u32,
            output_tokens: o as u32,
            cache_read_input_tokens: cr as u32,
            cache_creation_input_tokens: cc as u32,
        }),
        _ => None,
    };
    Ok(ForensicPacket {
        id: PacketId(row.get(0)),
        status,
        run_id: row.get(1),
        iteration: row.get::<_, i32>(2) as u32,
        captured_at: row.get(4),
        completed_at: row.get(5),
        prompt,
        kohai_response: row.get(7),
        kohai_usage,
        sempai_review,
    })
}
