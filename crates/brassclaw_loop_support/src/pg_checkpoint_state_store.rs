//! Postgres-backed [`CheckpointStateStore`] implementation.
//!
//! Stores host-owned loop checkpoint payloads in `brassclaw_checkpoints`
//! (V012__checkpoints.sql).  Payloads are stored as raw `BYTEA`; metadata fields
//! are extracted to typed columns for indexing.
//!
//! Records are write-once (immutable after insert).  The caller is responsible
//! for retention sweeps per §4.13 / §4.21.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_pg::PgPool;
use chrono::Utc;

use brassclaw_turns::{
    CheckpointStateRecord, CheckpointStateStore, GetCheckpointStateRequest,
    LoopCheckpointStateRef, PutCheckpointStateRequest, RedactedCheckpointPayload, TurnError,
    checkpoint_state_metadata_matches_request, checkpoint_state_record_matches_request,
    new_checkpoint_state_ref,
};

fn map_pg_pool(e: deadpool_postgres::PoolError) -> TurnError {
    TurnError::Unavailable {
        reason: e.to_string(),
    }
}

fn map_pg(e: tokio_postgres::Error) -> TurnError {
    TurnError::Unavailable {
        reason: e.to_string(),
    }
}

/// Returns the `as_str()` string for a [`LoopCheckpointKind`].
fn kind_str(kind: brassclaw_turns::LoopCheckpointKind) -> &'static str {
    kind.as_str()
}

/// Parse a [`LoopCheckpointKind`] from a DB string.
fn kind_from_str(s: &str) -> Result<brassclaw_turns::LoopCheckpointKind, TurnError> {
    match s {
        "before_model" => Ok(brassclaw_turns::LoopCheckpointKind::BeforeModel),
        "before_side_effect" => Ok(brassclaw_turns::LoopCheckpointKind::BeforeSideEffect),
        "before_block" => Ok(brassclaw_turns::LoopCheckpointKind::BeforeBlock),
        "final" => Ok(brassclaw_turns::LoopCheckpointKind::Final),
        other => Err(TurnError::Unavailable {
            reason: format!("unknown checkpoint kind in DB: {other}"),
        }),
    }
}

// ---------------------------------------------------------------------------
// PgCheckpointStateStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`CheckpointStateStore`].
///
/// Stores checkpoint payloads in `brassclaw_checkpoints`.
/// Payload bytes are stored as `BYTEA`; metadata fields are in typed columns.
pub struct PgCheckpointStateStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgCheckpointStateStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }
}

#[async_trait]
impl CheckpointStateStore for PgCheckpointStateStore {
    async fn put_checkpoint_state(
        &self,
        request: PutCheckpointStateRequest,
    ) -> Result<CheckpointStateRecord, TurnError> {
        // Validate payload length before hitting the DB.
        let payload_bytes = request.payload_bytes().to_vec();
        let payload = RedactedCheckpointPayload::new(payload_bytes.clone())
            .map_err(|reason| TurnError::InvalidRequest { reason })?;

        let state_ref = new_checkpoint_state_ref()?;
        let schema_version: i64 = request.schema_version.as_u64() as i64;
        let kind_col = kind_str(request.kind);
        let created_at = Utc::now();

        let client = self.pool.get().await.map_err(map_pg_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_checkpoints \
                 (tenant_id, turn_id, run_id, state_ref, schema_id, schema_version, kind, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                 ON CONFLICT (tenant_id, run_id, state_ref, schema_id, schema_version, kind) \
                 DO NOTHING",
                &[
                    &self.tenant_id,
                    &request.turn_id.to_string(),
                    &request.run_id.to_string(),
                    &state_ref.as_str(),
                    &request.schema_id.as_str(),
                    &schema_version,
                    &kind_col,
                    &payload_bytes,
                ],
            )
            .await
            .map_err(map_pg)?;

        Ok(CheckpointStateRecord {
            state_ref,
            scope: request.scope,
            turn_id: request.turn_id,
            run_id: request.run_id,
            schema_id: request.schema_id,
            schema_version: request.schema_version,
            kind: request.kind,
            payload,
            created_at,
        })
    }

    async fn get_checkpoint_state(
        &self,
        request: GetCheckpointStateRequest,
    ) -> Result<Option<CheckpointStateRecord>, TurnError> {
        let schema_version: i64 = request.schema_version.as_u64() as i64;
        let kind_col = kind_str(request.kind);

        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let row = client
            .query_opt(
                "SELECT turn_id, run_id, state_ref, schema_id, schema_version, kind, payload \
                 FROM brassclaw_checkpoints \
                 WHERE tenant_id = $1 \
                   AND run_id = $2 \
                   AND state_ref = $3 \
                   AND schema_id = $4 \
                   AND schema_version = $5 \
                   AND kind = $6",
                &[
                    &self.tenant_id,
                    &request.run_id.to_string(),
                    &request.state_ref.as_str(),
                    &request.schema_id.as_str(),
                    &schema_version,
                    &kind_col,
                ],
            )
            .await
            .map_err(map_pg)?;

        let Some(r) = row else {
            return Ok(None);
        };

        let state_ref_str: String = r.get(2);
        let schema_id_str: String = r.get(3);
        let sv: i64 = r.get(4);
        let kind_str_col: String = r.get(5);
        let payload_bytes: Vec<u8> = r.get(6);

        let state_ref = LoopCheckpointStateRef::new(state_ref_str).map_err(|e| {
            TurnError::Unavailable {
                reason: format!("invalid state_ref in DB: {e}"),
            }
        })?;
        let schema_id =
            brassclaw_turns::CheckpointSchemaId::new(schema_id_str).map_err(|e| {
                TurnError::Unavailable {
                    reason: format!("invalid schema_id in DB: {e}"),
                }
            })?;
        let kind = kind_from_str(&kind_str_col)?;
        let schema_version_typed =
            brassclaw_turns::RunProfileVersion::new(sv as u64);

        let payload = RedactedCheckpointPayload::new(payload_bytes)
            .map_err(|reason| TurnError::Unavailable { reason })?;

        let record = CheckpointStateRecord {
            state_ref,
            scope: request.scope.clone(),
            turn_id: request.turn_id,
            run_id: request.run_id,
            schema_id,
            schema_version: schema_version_typed,
            kind,
            payload,
            created_at: Utc::now(), // not persisted; callers don't rely on it from DB
        };

        // Apply the metadata match predicate that the in-memory store applies.
        if checkpoint_state_record_matches_request(&record, &request) {
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }
}
