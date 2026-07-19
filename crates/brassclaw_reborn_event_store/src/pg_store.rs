//! Postgres-backed [`DurableEventLog`] and [`DurableAuditLog`] implementations.
//!
//! Writes directly to `brassclaw_events` and `brassclaw_audit_log` (V013) using
//! the shared `PgPool` from composition. This replaces the previous VFS-fabric
//! path which opened its own pool.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_events::{
    DurableAuditLog, DurableEventLog, EventCursor, EventError, EventLogEntry, EventReplay,
    EventStreamKey, ReadScope, RuntimeEvent,
};
use brassclaw_host_api::AuditEnvelope;
use brassclaw_pg::PgPool;
use serde_json::Value;

fn map_pool(e: deadpool_postgres::PoolError) -> EventError {
    EventError::DurableLog { reason: e.to_string() }
}

fn map_pg(e: tokio_postgres::Error) -> EventError {
    EventError::DurableLog { reason: e.to_string() }
}

fn map_json(e: serde_json::Error) -> EventError {
    EventError::Serialize { reason: e.to_string() }
}

// ---------------------------------------------------------------------------
// PgDurableEventLog
// ---------------------------------------------------------------------------

/// Postgres-backed [`DurableEventLog`].
pub struct PgDurableEventLog {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgDurableEventLog {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl DurableEventLog for PgDurableEventLog {
    async fn append(
        &self,
        event: RuntimeEvent,
    ) -> Result<EventLogEntry<RuntimeEvent>, EventError> {
        let payload = serde_json::to_value(&event).map_err(map_json)?;
        let kind = format!("{:?}", event.kind);
        let run_id = event.scope.invocation_id.to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_one(
                "INSERT INTO brassclaw_events (tenant_id, run_id, kind, payload) \
                 VALUES ($1, $2, $3, $4) \
                 RETURNING seq",
                &[&self.tenant_id, &run_id, &kind, &payload],
            )
            .await
            .map_err(map_pg)?;
        let seq: i64 = row.get(0);
        Ok(EventLogEntry {
            cursor: EventCursor::new(seq as u64),
            record: event,
        })
    }

    async fn read_after_cursor(
        &self,
        stream: &EventStreamKey,
        _filter: &ReadScope,
        after: Option<EventCursor>,
        limit: usize,
    ) -> Result<EventReplay<RuntimeEvent>, EventError> {
        let after_seq = after.map(|c| c.as_u64() as i64).unwrap_or(0_i64);
        let user_id = stream.user_id.to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT seq, payload FROM brassclaw_events \
                 WHERE tenant_id = $1 AND seq > $2 \
                 ORDER BY seq ASC LIMIT $3",
                &[&self.tenant_id, &after_seq, &(limit as i64)],
            )
            .await
            .map_err(map_pg)?;
        let mut entries = Vec::with_capacity(rows.len());
        let mut last_cursor = after.unwrap_or_else(EventCursor::origin);
        for row in rows {
            let seq: i64 = row.get(0);
            let payload: Value = row.get(1);
            let event: RuntimeEvent = serde_json::from_value(payload).map_err(map_json)?;
            // Filter by user_id from stream key
            if event.scope.user_id.to_string() != user_id {
                continue;
            }
            let cursor = EventCursor::new(seq as u64);
            last_cursor = cursor;
            entries.push(EventLogEntry {
                cursor,
                record: event,
            });
        }
        let next_cursor = if entries.is_empty() {
            after.unwrap_or_else(EventCursor::origin)
        } else {
            last_cursor
        };
        Ok(EventReplay {
            entries,
            next_cursor,
        })
    }

    async fn head_cursor(
        &self,
        _stream: &EventStreamKey,
        after: EventCursor,
    ) -> Result<EventCursor, EventError> {
        let after_seq = after.as_u64() as i64;
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_one(
                "SELECT COALESCE(MAX(seq), $2) FROM brassclaw_events \
                 WHERE tenant_id = $1",
                &[&self.tenant_id, &after_seq],
            )
            .await
            .map_err(map_pg)?;
        let head_seq: i64 = row.get(0);
        let head = EventCursor::new(head_seq as u64);
        if head.as_u64() < after.as_u64() {
            return Err(EventError::ReplayGap {
                requested: after,
                earliest: head,
            });
        }
        Ok(head)
    }
}

// ---------------------------------------------------------------------------
// PgDurableAuditLog
// ---------------------------------------------------------------------------

/// Postgres-backed [`DurableAuditLog`].
pub struct PgDurableAuditLog {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgDurableAuditLog {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl DurableAuditLog for PgDurableAuditLog {
    async fn append(
        &self,
        record: AuditEnvelope,
    ) -> Result<EventLogEntry<AuditEnvelope>, EventError> {
        let payload = serde_json::to_value(&record).map_err(map_json)?;
        let actor_id = record.user_id.to_string();
        let action = record.action.kind.clone();
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_one(
                "INSERT INTO brassclaw_audit_log \
                 (tenant_id, actor_id, action, payload) \
                 VALUES ($1, $2, $3, $4) \
                 RETURNING seq",
                &[&self.tenant_id, &actor_id, &action, &payload],
            )
            .await
            .map_err(map_pg)?;
        let seq: i64 = row.get(0);
        Ok(EventLogEntry {
            cursor: EventCursor::new(seq as u64),
            record,
        })
    }

    async fn read_after_cursor(
        &self,
        _stream: &EventStreamKey,
        _filter: &ReadScope,
        after: Option<EventCursor>,
        limit: usize,
    ) -> Result<EventReplay<AuditEnvelope>, EventError> {
        let after_seq = after.map(|c| c.as_u64() as i64).unwrap_or(0_i64);
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT seq, payload FROM brassclaw_audit_log \
                 WHERE tenant_id = $1 AND seq > $2 \
                 ORDER BY seq ASC LIMIT $3",
                &[&self.tenant_id, &after_seq, &(limit as i64)],
            )
            .await
            .map_err(map_pg)?;
        let mut entries = Vec::with_capacity(rows.len());
        let mut last_cursor = after.unwrap_or_else(EventCursor::origin);
        for row in rows {
            let seq: i64 = row.get(0);
            let payload: Value = row.get(1);
            let record: AuditEnvelope = serde_json::from_value(payload).map_err(map_json)?;
            let cursor = EventCursor::new(seq as u64);
            last_cursor = cursor;
            entries.push(EventLogEntry {
                cursor,
                record,
            });
        }
        let next_cursor = if entries.is_empty() {
            after.unwrap_or_else(EventCursor::origin)
        } else {
            last_cursor
        };
        Ok(EventReplay {
            entries,
            next_cursor,
        })
    }
}
