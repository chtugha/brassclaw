//! Postgres-backed [`RunStateStore`] implementation.
//!
//! Stores invocation lifecycle state in `brassclaw_runs` (V004__runs.sql).
//! All status values are the snake_case serde representations of [`RunStatus`]
//! (e.g. `BlockedApproval` → `"blocked_approval"`).
//!
//! Tenant isolation is enforced by the `tenant_id` column; wrong-scope lookups
//! return `Ok(None)` or `UnknownInvocation` rather than leaking cross-tenant
//! records.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{ApprovalRequest, InvocationId, ResourceScope};
use brassclaw_pg::PgPool;
use serde_json::Value;

use crate::{RunRecord, RunStart, RunStateError, RunStateStore, RunStatus};

fn map_pg_pool(e: deadpool_postgres::PoolError) -> RunStateError {
    RunStateError::Backend(e.to_string())
}

fn map_pg(e: tokio_postgres::Error) -> RunStateError {
    RunStateError::Backend(e.to_string())
}

fn map_json(e: serde_json::Error) -> RunStateError {
    RunStateError::Serialization(e.to_string())
}

/// Serialize a [`RunRecord`] as JSONB payload.
fn run_record_payload(record: &RunRecord) -> Result<Value, RunStateError> {
    serde_json::to_value(record).map_err(map_json)
}

/// Deserialize a [`RunRecord`] from a JSONB payload row.
fn run_record_from_row(payload: Value) -> Result<RunRecord, RunStateError> {
    serde_json::from_value(payload).map_err(|e| RunStateError::Deserialization(e.to_string()))
}

/// snake_case string for a [`RunStatus`] value.
///
/// `RunStatus` has `#[serde(rename_all = "snake_case")]` so we can roundtrip
/// through JSON string to obtain the DB-column value.
fn status_str(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "running",
        RunStatus::BlockedApproval => "blocked_approval",
        RunStatus::BlockedAuth => "blocked_auth",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
    }
}

// ---------------------------------------------------------------------------
// PgRunStateStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`RunStateStore`].
///
/// Stores invocation lifecycle records in `brassclaw_runs`. Tenant isolation
/// is enforced by the `tenant_id` column; all mutations scope their
/// `WHERE` clauses to the supplied `scope.tenant_id`.
pub struct PgRunStateStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgRunStateStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    async fn read_record(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
    ) -> Result<Option<RunRecord>, RunStateError> {
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let row = client
            .query_opt(
                "SELECT payload FROM brassclaw_runs \
                 WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL",
                &[&invocation_id.to_string(), &self.tenant_id],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: Value = r.get(0);
                let record = run_record_from_row(payload)?;
                // Scope guard: wrong-scope lookups must look unknown.
                if !same_scope_owner(&record.scope, scope) {
                    return Ok(None);
                }
                Ok(Some(record))
            }
        }
    }

    async fn update_record(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
        update: impl FnOnce(&mut RunRecord),
    ) -> Result<RunRecord, RunStateError> {
        let mut record = self
            .read_record(scope, invocation_id)
            .await?
            .ok_or(RunStateError::UnknownInvocation { invocation_id })?;
        update(&mut record);
        let payload = run_record_payload(&record)?;
        let status = status_str(record.status);
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let rows = client
            .execute(
                "UPDATE brassclaw_runs \
                 SET payload = $1, status = $2, updated_at = now() \
                 WHERE id = $3 AND tenant_id = $4 AND deleted_at IS NULL",
                &[
                    &payload,
                    &status,
                    &invocation_id.to_string(),
                    &self.tenant_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        if rows == 0 {
            return Err(RunStateError::UnknownInvocation { invocation_id });
        }
        Ok(record)
    }
}

#[async_trait]
impl RunStateStore for PgRunStateStore {
    async fn start(&self, start: RunStart) -> Result<RunRecord, RunStateError> {
        let record = RunRecord {
            invocation_id: start.invocation_id,
            capability_id: start.capability_id,
            scope: start.scope,
            status: RunStatus::Running,
            approval_request_id: None,
            error_kind: None,
        };
        let payload = run_record_payload(&record)?;
        let status = status_str(record.status);
        let thread_id = record.scope.thread_id.as_ref().map(|t| t.to_string());
        let user_id = record.scope.user_id.to_string();
        let agent_id = record.scope.agent_id.as_ref().map(|a| a.to_string());
        let project_id = record.scope.project_id.as_ref().map(|p| p.to_string());

        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let result = client
            .execute(
                "INSERT INTO brassclaw_runs \
                 (id, tenant_id, user_id, agent_id, project_id, thread_id, status, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &record.invocation_id.to_string(),
                    &self.tenant_id,
                    &user_id,
                    &agent_id,
                    &project_id,
                    &thread_id,
                    &status,
                    &payload,
                ],
            )
            .await;
        match result {
            Ok(_) => Ok(record),
            Err(e) if is_unique_violation(&e) => {
                Err(RunStateError::InvocationAlreadyExists {
                    invocation_id: record.invocation_id,
                })
            }
            Err(e) => Err(map_pg(e)),
        }
    }

    async fn block_approval(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
        approval: ApprovalRequest,
    ) -> Result<RunRecord, RunStateError> {
        let approval_id = approval.id;
        self.update_record(scope, invocation_id, |r| {
            r.status = RunStatus::BlockedApproval;
            r.approval_request_id = Some(approval_id);
            r.error_kind = None;
        })
        .await
    }

    async fn block_auth(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
        error_kind: String,
    ) -> Result<RunRecord, RunStateError> {
        self.update_record(scope, invocation_id, |r| {
            r.status = RunStatus::BlockedAuth;
            r.approval_request_id = None;
            r.error_kind = Some(error_kind.clone());
        })
        .await
    }

    async fn complete(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
    ) -> Result<RunRecord, RunStateError> {
        self.update_record(scope, invocation_id, |r| {
            r.status = RunStatus::Completed;
            r.approval_request_id = None;
            r.error_kind = None;
        })
        .await
    }

    async fn fail(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
        error_kind: String,
    ) -> Result<RunRecord, RunStateError> {
        self.update_record(scope, invocation_id, |r| {
            r.status = RunStatus::Failed;
            r.approval_request_id = None;
            r.error_kind = Some(error_kind.clone());
        })
        .await
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        invocation_id: InvocationId,
    ) -> Result<Option<RunRecord>, RunStateError> {
        self.read_record(scope, invocation_id).await
    }

    async fn records_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<RunRecord>, RunStateError> {
        let user_id = scope.user_id.to_string();
        let client = self.pool.get().await.map_err(map_pg_pool)?;
        let rows = client
            .query(
                "SELECT payload FROM brassclaw_runs \
                 WHERE tenant_id = $1 AND user_id = $2 AND deleted_at IS NULL \
                 ORDER BY id",
                &[&self.tenant_id, &user_id],
            )
            .await
            .map_err(map_pg)?;
        let mut records = Vec::new();
        for row in rows {
            let payload: Value = row.get(0);
            match run_record_from_row(payload) {
                Ok(record) if same_scope_owner(&record.scope, scope) => records.push(record),
                Ok(_) => {}
                Err(_) => {}
            }
        }
        Ok(records)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
