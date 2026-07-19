//! Postgres-backed [`ApprovalRequestStore`] implementation.
//!
//! Stores approval request lifecycle state in `brassclaw_approvals`
//! (V005__approvals.sql). All status values are the snake_case serde
//! representations of [`ApprovalStatus`]:
//!   `Pending→"pending"`, `Approved→"approved"`, `Denied→"denied"`, `Expired→"expired"`.
//!
//! Tenant isolation is enforced by the `tenant_id` column on every query.
//! Wrong-scope lookups return `Ok(None)` rather than leaking cross-tenant records.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{ApprovalRequest, ApprovalRequestId, ResourceScope};
use brassclaw_pg::PgPool;
use brassclaw_run_state::{ApprovalRecord, ApprovalRequestStore, ApprovalStatus, RunStateError};
use serde_json::Value;

fn map_pg_pool(e: deadpool_postgres::PoolError) -> RunStateError {
    RunStateError::Backend(e.to_string())
}

fn map_pg(e: tokio_postgres::Error) -> RunStateError {
    RunStateError::Backend(e.to_string())
}

fn map_json(e: serde_json::Error) -> RunStateError {
    RunStateError::Serialization(e.to_string())
}

fn status_str(status: ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Denied => "denied",
        ApprovalStatus::Expired => "expired",
    }
}

fn record_payload(record: &ApprovalRecord) -> Result<Value, RunStateError> {
    serde_json::to_value(record).map_err(map_json)
}

fn record_from_row(payload: Value) -> Result<ApprovalRecord, RunStateError> {
    serde_json::from_value(payload).map_err(|e| RunStateError::Deserialization(e.to_string()))
}

fn same_scope_owner(left: &ResourceScope, right: &ResourceScope) -> bool {
    left.tenant_id == right.tenant_id
        && left.user_id == right.user_id
        && left.agent_id == right.agent_id
        && left.project_id == right.project_id
}

fn is_unique_violation(e: &tokio_postgres::Error) -> bool {
    e.code()
        .is_some_and(|c| c == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION)
}

// ---------------------------------------------------------------------------
// PgApprovalRequestStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`ApprovalRequestStore`].
///
/// Stores pending/approved/denied/expired approval records in
/// `brassclaw_approvals`. Tenant isolation is enforced by `tenant_id` on every
/// query; wrong-scope lookups appear as not-found.
pub struct PgApprovalRequestStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgApprovalRequestStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    async fn read_record(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<Option<ApprovalRecord>, RunStateError> {
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let row = client
            .query_opt(
                "SELECT request FROM brassclaw_approvals \
                 WHERE id = $1 AND tenant_id = $2",
                &[&request_id.as_uuid().to_string(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: Value = r.get(0);
                let record = record_from_row(payload)?;
                if !same_scope_owner(&record.scope, scope) {
                    return Ok(None);
                }
                Ok(Some(record))
            }
        }
    }

    async fn transition_status(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
        new_status: ApprovalStatus,
    ) -> Result<ApprovalRecord, RunStateError> {
        let mut record = self
            .read_record(scope, request_id)
            .await?
            .ok_or(RunStateError::UnknownApprovalRequest { request_id })?;
        if record.status != ApprovalStatus::Pending {
            return Err(RunStateError::ApprovalNotPending {
                request_id,
                status: record.status,
            });
        }
        record.status = new_status;
        let payload = record_payload(&record)?;
        let status_col = status_str(new_status);
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let rows = client
            .execute(
                "UPDATE brassclaw_approvals \
                 SET status = $1, request = $2, updated_at = now() \
                 WHERE id = $3 AND tenant_id = $4 AND status = 'pending'",
                &[
                    &status_col,
                    &payload,
                    &request_id.as_uuid().to_string(),
                    &self.tenant_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        if rows == 0 {
            // Either already transitioned or not found.
            return Err(RunStateError::ApprovalNotPending {
                request_id,
                status: record.status,
            });
        }
        Ok(record)
    }
}

#[async_trait]
impl ApprovalRequestStore for PgApprovalRequestStore {
    async fn save_pending(
        &self,
        scope: ResourceScope,
        request: ApprovalRequest,
    ) -> Result<ApprovalRecord, RunStateError> {
        let record = ApprovalRecord {
            scope,
            request,
            status: ApprovalStatus::Pending,
        };
        let payload = record_payload(&record)?;
        // run_id is the invocation_id from the scope, stored in the FK column.
        let run_id = record.scope.invocation_id.to_string();

        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let result = client
            .execute(
                "INSERT INTO brassclaw_approvals (id, tenant_id, run_id, status, request) \
                 VALUES ($1, $2, $3, 'pending', $4)",
                &[
                    &record.request.id.as_uuid().to_string(),
                    &self.tenant_id,
                    &run_id,
                    &payload,
                ],
            )
            .await;
        match result {
            Ok(_) => Ok(record),
            Err(e) if is_unique_violation(&e) => {
                Err(RunStateError::ApprovalRequestAlreadyExists {
                    request_id: record.request.id,
                })
            }
            Err(e) => Err(map_pg(e)),
        }
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<Option<ApprovalRecord>, RunStateError> {
        self.read_record(scope, request_id).await
    }

    async fn approve(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<ApprovalRecord, RunStateError> {
        self.transition_status(scope, request_id, ApprovalStatus::Approved)
            .await
    }

    async fn deny(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<ApprovalRecord, RunStateError> {
        self.transition_status(scope, request_id, ApprovalStatus::Denied)
            .await
    }

    async fn discard_pending(
        &self,
        scope: &ResourceScope,
        request_id: ApprovalRequestId,
    ) -> Result<ApprovalRecord, RunStateError> {
        // Delete the pending row entirely rather than leaving a tombstone.
        let record = self
            .read_record(scope, request_id)
            .await?
            .ok_or(RunStateError::UnknownApprovalRequest { request_id })?;
        if record.status != ApprovalStatus::Pending {
            return Err(RunStateError::ApprovalNotPending {
                request_id,
                status: record.status,
            });
        }
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        client
            .execute(
                "DELETE FROM brassclaw_approvals \
                 WHERE id = $1 AND tenant_id = $2 AND status = 'pending'",
                &[
                    &request_id.as_uuid().to_string(),
                    &self.tenant_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(record)
    }

    async fn records_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<ApprovalRecord>, RunStateError> {
        let run_id = scope.invocation_id.to_string();
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let rows = client
            .query(
                "SELECT request FROM brassclaw_approvals \
                 WHERE tenant_id = $1 AND run_id = $2 \
                 ORDER BY id",
                &[&self.tenant_id, &run_id],
            )
            .await
            .map_err(map_pg)?;
        let mut records = Vec::new();
        for row in rows {
            let payload: Value = row.get(0);
            match record_from_row(payload) {
                Ok(record) if same_scope_owner(&record.scope, scope) => records.push(record),
                Ok(_) | Err(_) => {}
            }
        }
        Ok(records)
    }
}
