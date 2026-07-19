//! PostgreSQL-backed [`ConversationStateRepository`].
//!
//! Stores the full serialized [`InMemoryState`] as a JSONB blob with
//! compare-and-swap via a monotonic `revision` counter.  The table is
//! `brassclaw_conversation_state` (created by V022).
//!
//! The table uses `tenant_id` as the primary key (one row per tenant);
//! the caller supplies the `tenant_id` at construction time.

use async_trait::async_trait;
use deadpool_postgres::Pool;

use crate::{
    InboundTurnError,
    memory::InMemoryState,
    state_store::{ConversationStateRepository, PersistedConversationState},
};

/// PostgreSQL-backed conversation state repository keyed by tenant id.
///
/// A single row per tenant holds the full `InMemoryState` as JSONB with a
/// `revision` counter for compare-and-swap writes.
pub struct PgConversationStateStore {
    pool: Pool,
    tenant_id: String,
}

impl PgConversationStateStore {
    pub fn new(pool: Pool, tenant_id: String) -> Self {
        Self { pool, tenant_id }
    }

    async fn connect(&self) -> Result<deadpool_postgres::Object, InboundTurnError> {
        self.pool
            .get()
            .await
            .map_err(|error| InboundTurnError::DurableState {
                reason: format!("pg conversation state connect: {error}"),
            })
    }
}

#[async_trait]
impl ConversationStateRepository for PgConversationStateStore {
    async fn load_state(&self) -> Result<PersistedConversationState, InboundTurnError> {
        let client = self.connect().await?;
        let row = client
            .query_opt(
                "SELECT state_blob, revision \
                 FROM brassclaw_conversation_state \
                 WHERE tenant_id = $1 \
                 LIMIT 1",
                &[&self.tenant_id],
            )
            .await
            .map_err(|error| InboundTurnError::DurableState {
                reason: format!("pg conversation state load: {error}"),
            })?;

        match row {
            Some(row) => {
                let blob: serde_json::Value =
                    row.try_get("state_blob")
                        .map_err(|error| InboundTurnError::DurableState {
                            reason: format!("pg conversation state read state_blob: {error}"),
                        })?;
                let revision: i64 =
                    row.try_get("revision")
                        .map_err(|error| InboundTurnError::DurableState {
                            reason: format!("pg conversation state read revision: {error}"),
                        })?;
                let state: InMemoryState =
                    serde_json::from_value(blob).map_err(|error| InboundTurnError::DurableState {
                        reason: format!("pg conversation state deserialize: {error}"),
                    })?;
                Ok(PersistedConversationState { state, revision })
            }
            None => Ok(PersistedConversationState {
                state: InMemoryState::default(),
                revision: 0,
            }),
        }
    }

    async fn save_state(
        &self,
        expected_revision: i64,
        state: &InMemoryState,
    ) -> Result<i64, InboundTurnError> {
        let blob = serde_json::to_value(state).map_err(|error| InboundTurnError::DurableState {
            reason: format!("pg conversation state serialize: {error}"),
        })?;

        let client = self.connect().await?;
        let new_revision = expected_revision + 1;

        if expected_revision == 0 {
            // Initial insert with ON CONFLICT to handle concurrent bootstrap.
            // The WHERE guard ensures only one revision=0 writer wins.
            let rows = client
                .execute(
                    "INSERT INTO brassclaw_conversation_state \
                         (tenant_id, state_blob, revision) \
                         VALUES ($1, $2, $3) \
                         ON CONFLICT (tenant_id) DO UPDATE \
                         SET state_blob = EXCLUDED.state_blob, \
                             revision   = EXCLUDED.revision \
                         WHERE brassclaw_conversation_state.revision = $4",
                    &[
                        &self.tenant_id,
                        &blob,
                        &new_revision,
                        &expected_revision,
                    ],
                )
                .await
                .map_err(|error| InboundTurnError::DurableState {
                    reason: format!("pg conversation state insert: {error}"),
                })?;
            if rows == 0 {
                return Err(InboundTurnError::DurableState {
                    reason: "pg conversation state CAS conflict on insert".to_string(),
                });
            }
        } else {
            let rows = client
                .execute(
                    "UPDATE brassclaw_conversation_state \
                         SET state_blob = $1, revision = $2 \
                         WHERE tenant_id = $3 AND revision = $4",
                    &[&blob, &new_revision, &self.tenant_id, &expected_revision],
                )
                .await
                .map_err(|error| InboundTurnError::DurableState {
                    reason: format!("pg conversation state update: {error}"),
                })?;
            if rows == 0 {
                return Err(InboundTurnError::DurableState {
                    reason: "pg conversation state CAS conflict on update".to_string(),
                });
            }
        }

        Ok(new_revision)
    }
}
