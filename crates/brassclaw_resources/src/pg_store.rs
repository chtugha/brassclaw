//! Postgres-backed [`ResourceGovernorStore`] and [`BudgetGateStore`] implementations.
//!
//! ## PgResourceGovernorStore
//!
//! Stores the `ResourceGovernorSnapshot` as JSONB in `brassclaw_resource_accounts`
//! with a `version` CAS column. The synchronous `update` trait method is bridged
//! to async Postgres via `tokio::task::block_in_place`.
//!
//! ## PgBudgetGateStore
//!
//! Stores budget approval gates in `brassclaw_budget_gates` (V019).

use std::sync::Arc;

use brassclaw_host_api::ResourceScope;
use brassclaw_pg::PgPool;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{
    BudgetApprovalGate, BudgetGateError, BudgetGateId, BudgetGateOutcome, BudgetGateStatus,
    BudgetGateStore, ResourceError, ResourceGovernorSnapshot, ResourceGovernorStore,
};

fn map_pool_r(e: deadpool_postgres::PoolError) -> ResourceError {
    ResourceError::Storage { reason: e.to_string() }
}

fn map_pg_r(e: tokio_postgres::Error) -> ResourceError {
    ResourceError::Storage { reason: e.to_string() }
}

fn map_json_r(e: serde_json::Error) -> ResourceError {
    ResourceError::Storage { reason: e.to_string() }
}

fn map_pool_b(e: deadpool_postgres::PoolError) -> BudgetGateError {
    BudgetGateError::Storage { reason: e.to_string() }
}

fn map_pg_b(e: tokio_postgres::Error) -> BudgetGateError {
    BudgetGateError::Storage { reason: e.to_string() }
}

fn map_json_b(e: serde_json::Error) -> BudgetGateError {
    BudgetGateError::Storage { reason: e.to_string() }
}

// ---------------------------------------------------------------------------
// PgResourceGovernorStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`ResourceGovernorStore`].
pub struct PgResourceGovernorStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl PgResourceGovernorStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    fn read_snapshot_sync(&self) -> Result<(ResourceGovernorSnapshot, i64), ResourceError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let client = self.pool.get().await.map_err(map_pool_r)?;
                let row = client
                    .query_opt(
                        "SELECT payload, version FROM brassclaw_resource_accounts \
                         WHERE tenant_id = $1 AND scope_kind = 'tenant' AND scope_id = $1 \
                           AND period_key = '__governor__'",
                        &[&self.tenant_id],
                    )
                    .await
                    .map_err(map_pg_r)?;
                match row {
                    None => Ok((ResourceGovernorSnapshot::default(), 0_i64)),
                    Some(r) => {
                        let payload: Value = r.get(0);
                        let version: i64 = r.get(1);
                        let snapshot: ResourceGovernorSnapshot =
                            serde_json::from_value(payload).map_err(map_json_r)?;
                        Ok((snapshot, version))
                    }
                }
            })
        })
    }

    fn write_snapshot_sync(
        &self,
        snapshot: &ResourceGovernorSnapshot,
        expected_version: i64,
    ) -> Result<bool, ResourceError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let payload = serde_json::to_value(snapshot).map_err(map_json_r)?;
                let next_version = expected_version + 1;
                let client = self.pool.get().await.map_err(map_pool_r)?;
                let rows = client
                    .execute(
                        "INSERT INTO brassclaw_resource_accounts \
                         (id, tenant_id, scope_kind, scope_id, period_key, reserved, consumed, \
                          version, payload) \
                         VALUES ($1, $2, 'tenant', $2, '__governor__', 0, 0, 1, $3) \
                         ON CONFLICT (tenant_id, scope_kind, scope_id, period_key) DO UPDATE \
                         SET payload = excluded.payload, version = $4, updated_at = now() \
                         WHERE brassclaw_resource_accounts.version = $5",
                        &[
                            &format!("governor:{}", &self.tenant_id),
                            &self.tenant_id,
                            &payload,
                            &next_version,
                            &expected_version,
                        ],
                    )
                    .await
                    .map_err(map_pg_r)?;
                Ok(rows > 0)
            })
        })
    }
}

impl ResourceGovernorStore for PgResourceGovernorStore {
    fn update<T, F>(&self, update: F) -> Result<T, ResourceError>
    where
        T: Send + 'static,
        F: FnOnce(&mut ResourceGovernorSnapshot) -> Result<T, ResourceError> + Send + 'static,
    {
        let (mut snapshot, version) = self.read_snapshot_sync()?;
        let value = update(&mut snapshot)?;
        if self.write_snapshot_sync(&snapshot, version)? {
            return Ok(value);
        }
        Err(ResourceError::Storage {
            reason: "resource governor version conflict — retry from caller".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// PgBudgetGateStore
// ---------------------------------------------------------------------------

/// Postgres-backed [`BudgetGateStore`].
pub struct PgBudgetGateStore {
    pool: Arc<PgPool>,
    tenant_id: String,
}

impl std::fmt::Debug for PgBudgetGateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgBudgetGateStore")
            .field("tenant_id", &self.tenant_id)
            .finish()
    }
}

impl PgBudgetGateStore {
    pub fn new(pool: Arc<PgPool>, tenant_id: impl Into<String>) -> Self {
        Self {
            pool,
            tenant_id: tenant_id.into(),
        }
    }

    fn read_gate_sync(&self, id: BudgetGateId) -> Result<Option<BudgetApprovalGate>, BudgetGateError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let client = self.pool.get().await.map_err(map_pool_b)?;
                let row = client
                    .query_opt(
                        "SELECT payload FROM brassclaw_budget_gates \
                         WHERE id = $1 AND tenant_id = $2",
                        &[&id.as_uuid().to_string(), &self.tenant_id],
                    )
                    .await
                    .map_err(map_pg_b)?;
                match row {
                    None => Ok(None),
                    Some(r) => {
                        let payload: Value = r.get(0);
                        let gate: BudgetApprovalGate =
                            serde_json::from_value(payload).map_err(map_json_b)?;
                        Ok(Some(gate))
                    }
                }
            })
        })
    }

    fn status_kind_str(status: &BudgetGateStatus) -> &'static str {
        match status {
            BudgetGateStatus::Pending => "pending",
            BudgetGateStatus::Approved { .. } => "approved",
            BudgetGateStatus::Cancelled { .. } => "cancelled",
            BudgetGateStatus::Expired { .. } => "expired",
        }
    }
}

impl BudgetGateStore for PgBudgetGateStore {
    fn open(&self, _scope: &ResourceScope, gate: BudgetApprovalGate) -> Result<(), BudgetGateError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let payload = serde_json::to_value(&gate).map_err(map_json_b)?;
                // Use Display (not Debug) — Debug gives "InputTokens", Display gives "input_tokens".
                let gate_kind = gate.needed.dimension.to_string();
                let requested_amount: f64 = match &gate.needed.requested {
                    crate::ResourceValue::Decimal(d) => {
                        d.to_string().parse::<f64>().unwrap_or(0.0)
                    }
                    crate::ResourceValue::Integer(i) => *i as f64,
                };
                let client = self.pool.get().await.map_err(map_pool_b)?;
                client
                    .execute(
                        "INSERT INTO brassclaw_budget_gates \
                         (id, tenant_id, gate_kind, status, requested_amount, payload, \
                          expires_at) \
                         VALUES ($1, $2, $3, 'pending', $4, $5, $6) \
                         ON CONFLICT (id) DO NOTHING",
                        &[
                            &gate.id.as_uuid().to_string(),
                            &self.tenant_id,
                            &gate_kind,
                            &requested_amount,
                            &payload,
                            &gate.expires_at,
                        ],
                    )
                    .await
                    .map_err(map_pg_b)?;
                Ok(())
            })
        })
    }

    fn resolve(
        &self,
        _scope: &ResourceScope,
        id: BudgetGateId,
        outcome: BudgetGateOutcome,
        at: DateTime<Utc>,
    ) -> Result<BudgetApprovalGate, BudgetGateError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut gate = self
                    .read_gate_sync(id)?
                    .ok_or(BudgetGateError::Unknown { id })?;
                if gate.status.is_terminal() {
                    return Err(BudgetGateError::AlreadyResolved { id });
                }
                gate.status = match outcome {
                    BudgetGateOutcome::Approve { increased_limit, by } => {
                        BudgetGateStatus::Approved { increased_limit, by, at }
                    }
                    BudgetGateOutcome::Cancel { by } => BudgetGateStatus::Cancelled { by, at },
                };
                let new_status_str = Self::status_kind_str(&gate.status);
                let payload = serde_json::to_value(&gate).map_err(map_json_b)?;
                let client = self.pool.get().await.map_err(map_pool_b)?;
                client
                    .execute(
                        "UPDATE brassclaw_budget_gates \
                         SET status = $1, payload = $2, updated_at = now() \
                         WHERE id = $3 AND tenant_id = $4 AND status = 'pending'",
                        &[&new_status_str, &payload, &id.as_uuid().to_string(), &self.tenant_id],
                    )
                    .await
                    .map_err(map_pg_b)?;
                Ok(gate)
            })
        })
    }

    fn expire_pending_older_than(
        &self,
        _scope: &ResourceScope,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<BudgetApprovalGate>, BudgetGateError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let client = self.pool.get().await.map_err(map_pool_b)?;
                let rows = client
                    .query(
                        "SELECT payload FROM brassclaw_budget_gates \
                         WHERE tenant_id = $1 AND status = 'pending' \
                           AND created_at < $2",
                        &[&self.tenant_id, &cutoff],
                    )
                    .await
                    .map_err(map_pg_b)?;
                let gates: Vec<BudgetApprovalGate> = rows
                    .into_iter()
                    .filter_map(|r| {
                        let payload: Value = r.get(0);
                        serde_json::from_value(payload).ok()
                    })
                    .collect();
                if !gates.is_empty() {
                    client
                        .execute(
                            "UPDATE brassclaw_budget_gates \
                             SET status = 'expired', updated_at = now() \
                             WHERE tenant_id = $1 AND status = 'pending' \
                               AND created_at < $2",
                            &[&self.tenant_id, &cutoff],
                        )
                        .await
                        .map_err(map_pg_b)?;
                }
                Ok(gates)
            })
        })
    }

    fn get(
        &self,
        _scope: &ResourceScope,
        id: BudgetGateId,
    ) -> Result<Option<BudgetApprovalGate>, BudgetGateError> {
        self.read_gate_sync(id)
    }

    fn list_pending(&self, _scope: &ResourceScope) -> Result<Vec<BudgetApprovalGate>, BudgetGateError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let client = self.pool.get().await.map_err(map_pool_b)?;
                let rows = client
                    .query(
                        "SELECT payload FROM brassclaw_budget_gates \
                         WHERE tenant_id = $1 AND status = 'pending' \
                         ORDER BY created_at ASC",
                        &[&self.tenant_id],
                    )
                    .await
                    .map_err(map_pg_b)?;
                Ok(rows
                    .into_iter()
                    .filter_map(|r| {
                        let payload: Value = r.get(0);
                        serde_json::from_value(payload).ok()
                    })
                    .collect())
            })
        })
    }
}
