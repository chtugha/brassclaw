//! Postgres-backed [`ProcessStore`] and [`ProcessResultStore`] implementations.
//!
//! Stores process lifecycle records in `brassclaw_processes` (V009).
//! Status values are the snake_case serde representations of [`ProcessStatus`]
//! (`#[serde(rename_all = "snake_case")]`): `running`, `completed`, `failed`, `killed`.
//!
//! `PgProcessResultStore::complete/fail/kill` writes the same ULID into both
//! `brassclaw_processes.id` and `brassclaw_processes.process_id` columns per the
//! FK invariant noted in §4.15.

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_host_api::{ProcessId, ResourceScope};
use brassclaw_pg::PgPool;
use serde_json::Value;

use crate::types::{
    ProcessError, ProcessRecord, ProcessResultRecord, ProcessStart, ProcessStatus,
    ProcessStore, ProcessResultStore,
};

fn map_pool(e: deadpool_postgres::PoolError) -> ProcessError {
    ProcessError::InvalidStoredRecord { reason: e.to_string() }
}

fn map_pg(e: tokio_postgres::Error) -> ProcessError {
    ProcessError::InvalidStoredRecord { reason: e.to_string() }
}

fn map_json(e: serde_json::Error) -> ProcessError {
    ProcessError::Deserialization(e.to_string())
}

fn status_str(status: ProcessStatus) -> &'static str {
    match status {
        ProcessStatus::Running => "running",
        ProcessStatus::Completed => "completed",
        ProcessStatus::Failed => "failed",
        ProcessStatus::Killed => "killed",
    }
}

fn is_unique_violation(e: &tokio_postgres::Error) -> bool {
    e.code()
        .is_some_and(|c| c == &tokio_postgres::error::SqlState::UNIQUE_VIOLATION)
}

// ---------------------------------------------------------------------------
// PgProcessStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`ProcessStore`].
pub struct PgProcessStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgProcessStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    async fn read_record(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<Option<ProcessRecord>, ProcessError> {
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT payload FROM brassclaw_processes \
                 WHERE id = $1 AND tenant_id = $2 AND user_id = $3",
                &[
                    &process_id.to_string(),
                    &self.tenant_id,
                    &scope.user_id.to_string(),
                ],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: Value = r.get(0);
                Ok(Some(
                    serde_json::from_value(payload).map_err(map_json)?,
                ))
            }
        }
    }

    async fn update_status(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
        to: ProcessStatus,
        error_kind: Option<String>,
    ) -> Result<ProcessRecord, ProcessError> {
        let mut record = self
            .read_record(scope, process_id)
            .await?
            .ok_or(ProcessError::UnknownProcess { process_id })?;
        crate::types::ensure_status_transition(process_id, record.status, to)?;
        record.status = to;
        record.error_kind = error_kind;
        let payload = serde_json::to_value(&record).map_err(map_json)?;
        let status_col = status_str(to);
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "UPDATE brassclaw_processes \
                 SET status = $1, payload = $2, updated_at = now() \
                 WHERE id = $3 AND tenant_id = $4",
                &[
                    &status_col,
                    &payload,
                    &process_id.to_string(),
                    &self.tenant_id,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(record)
    }
}

#[async_trait]
impl ProcessStore for PgProcessStore {
    async fn start(&self, start: ProcessStart) -> Result<ProcessRecord, ProcessError> {
        let record = ProcessRecord {
            process_id: start.process_id,
            parent_process_id: start.parent_process_id,
            invocation_id: start.invocation_id,
            scope: start.scope,
            extension_id: start.extension_id,
            capability_id: start.capability_id,
            runtime: start.runtime,
            status: ProcessStatus::Running,
            grants: start.grants,
            mounts: start.mounts,
            estimated_resources: start.estimated_resources,
            resource_reservation_id: start.resource_reservation_id,
            error_kind: None,
        };
        let payload = serde_json::to_value(&record).map_err(map_json)?;
        let user_id = record.scope.user_id.to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        let result = client
            .execute(
                "INSERT INTO brassclaw_processes \
                 (id, tenant_id, user_id, status, payload) \
                 VALUES ($1, $2, $3, 'running', $4)",
                &[
                    &start.process_id.to_string(),
                    &self.tenant_id,
                    &user_id,
                    &payload,
                ],
            )
            .await;
        match result {
            Ok(_) => Ok(record),
            Err(e) if is_unique_violation(&e) => Err(ProcessError::ProcessAlreadyExists {
                process_id: start.process_id,
            }),
            Err(e) => Err(map_pg(e)),
        }
    }

    async fn complete(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<ProcessRecord, ProcessError> {
        self.update_status(scope, process_id, ProcessStatus::Completed, None).await
    }

    async fn fail(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
        error_kind: String,
    ) -> Result<ProcessRecord, ProcessError> {
        self.update_status(scope, process_id, ProcessStatus::Failed, Some(error_kind)).await
    }

    async fn kill(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<ProcessRecord, ProcessError> {
        self.update_status(scope, process_id, ProcessStatus::Killed, None).await
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<Option<ProcessRecord>, ProcessError> {
        self.read_record(scope, process_id).await
    }

    async fn records_for_scope(
        &self,
        scope: &ResourceScope,
    ) -> Result<Vec<ProcessRecord>, ProcessError> {
        let user_id = scope.user_id.to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        let rows = client
            .query(
                "SELECT payload FROM brassclaw_processes \
                 WHERE tenant_id = $1 AND user_id = $2 \
                 ORDER BY id",
                &[&self.tenant_id, &user_id],
            )
            .await
            .map_err(map_pg)?;
        let mut records = Vec::new();
        for row in rows {
            let payload: Value = row.get(0);
            if let Ok(record) = serde_json::from_value::<ProcessRecord>(payload) {
                records.push(record);
            }
        }
        Ok(records)
    }
}

// ---------------------------------------------------------------------------
// PgProcessResultStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`ProcessResultStore`].
pub struct PgProcessResultStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgProcessResultStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    async fn store_result(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
        status: ProcessStatus,
        output: Option<Value>,
        error_kind: Option<String>,
    ) -> Result<ProcessResultRecord, ProcessError> {
        let record = ProcessResultRecord {
            process_id,
            scope: scope.clone(),
            status,
            output: output.clone(),
            output_ref: None,
            error_kind,
        };
        let payload = serde_json::to_value(&record).map_err(map_json)?;
        let status_col = status_str(status);
        let user_id = scope.user_id.to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        client
            .execute(
                "INSERT INTO brassclaw_process_results \
                 (id, tenant_id, user_id, status, output, payload) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (id) DO UPDATE \
                 SET status = excluded.status, output = excluded.output, \
                     payload = excluded.payload, updated_at = now()",
                &[
                    &process_id.to_string(),
                    &self.tenant_id,
                    &user_id,
                    &status_col,
                    &output,
                    &payload,
                ],
            )
            .await
            .map_err(map_pg)?;
        Ok(record)
    }
}

#[async_trait]
impl ProcessResultStore for PgProcessResultStore {
    async fn complete(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
        output: Value,
    ) -> Result<ProcessResultRecord, ProcessError> {
        self.store_result(scope, process_id, ProcessStatus::Completed, Some(output), None).await
    }

    async fn fail(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
        error_kind: String,
    ) -> Result<ProcessResultRecord, ProcessError> {
        self.store_result(scope, process_id, ProcessStatus::Failed, None, Some(error_kind)).await
    }

    async fn kill(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<ProcessResultRecord, ProcessError> {
        self.store_result(scope, process_id, ProcessStatus::Killed, None, None).await
    }

    async fn get(
        &self,
        scope: &ResourceScope,
        process_id: ProcessId,
    ) -> Result<Option<ProcessResultRecord>, ProcessError> {
        let user_id = scope.user_id.to_string();
        let client = self.pool.get().await.map_err(map_pool)?;
        let row = client
            .query_opt(
                "SELECT payload FROM brassclaw_process_results \
                 WHERE id = $1 AND tenant_id = $2 AND user_id = $3",
                &[&process_id.to_string(), &self.tenant_id, &user_id],
            )
            .await
            .map_err(map_pg)?;
        match row {
            None => Ok(None),
            Some(r) => {
                let payload: Value = r.get(0);
                Ok(Some(serde_json::from_value(payload).map_err(map_json)?))
            }
        }
    }
}
