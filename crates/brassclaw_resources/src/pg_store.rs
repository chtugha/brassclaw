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

use std::{future::Future, sync::Arc, time::Duration};

use brassclaw_host_api::ResourceScope;
use brassclaw_pg::PgPool;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::{
    BudgetApprovalGate, BudgetGateError, BudgetGateId, BudgetGateOutcome, BudgetGateStatus,
    BudgetGateStore, ResourceError, ResourceGovernorSnapshot, ResourceGovernorStore,
};

fn map_pool_r(e: deadpool_postgres::PoolError) -> ResourceError {
    ResourceError::Storage {
        reason: e.to_string(),
    }
}

fn map_pg_r(e: tokio_postgres::Error) -> ResourceError {
    ResourceError::Storage {
        reason: e.to_string(),
    }
}

fn map_json_r(e: serde_json::Error) -> ResourceError {
    ResourceError::Storage {
        reason: e.to_string(),
    }
}

fn map_pool_b(e: deadpool_postgres::PoolError) -> BudgetGateError {
    BudgetGateError::Storage {
        reason: e.to_string(),
    }
}

fn map_pg_b(e: tokio_postgres::Error) -> BudgetGateError {
    BudgetGateError::Storage {
        reason: e.to_string(),
    }
}

fn map_json_b(e: serde_json::Error) -> BudgetGateError {
    BudgetGateError::Storage {
        reason: e.to_string(),
    }
}

// The synchronous governor is called on the production multithread runtime.
// Reject incompatible callers before block_in_place can panic. Do not detach
// mutations onto another executor: cancellation must not lose a reservation.
fn blocking_pg<T, E>(
    operation: impl Future<Output = Result<T, E>>,
    storage_error: impl Fn(&str) -> E,
) -> Result<T, E> {
    let runtime = tokio::runtime::Handle::try_current()
        .map_err(|_| storage_error("PostgreSQL resource store requires a Tokio runtime"))?;
    if runtime.runtime_flavor() != tokio::runtime::RuntimeFlavor::MultiThread {
        return Err(storage_error(
            "PostgreSQL resource store requires a multithread Tokio runtime",
        ));
    }
    tokio::task::block_in_place(|| {
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), operation)
                .await
                .map_err(|_| storage_error(
                    "PostgreSQL resource operation timed out; write outcome may require readback",
                ))?
        })
    })
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
        blocking_pg(
            async {
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
                    None => Ok((ResourceGovernorSnapshot::default(), 0i64)),
                    Some(r) => {
                        let payload: Value = r.get(0);
                        let version: i64 = r.get(1);
                        let snapshot: ResourceGovernorSnapshot =
                            serde_json::from_value(payload).map_err(map_json_r)?;
                        Ok((snapshot, version))
                    }
                }
            },
            |reason| ResourceError::Storage {
                reason: reason.to_owned(),
            },
        )
    }

    fn write_snapshot_sync(
        &self,
        snapshot: &ResourceGovernorSnapshot,
        expected_version: i64,
    ) -> Result<bool, ResourceError> {
        blocking_pg(
            async {
                let payload = serde_json::to_value(snapshot).map_err(map_json_r)?;
                let next_version =
                    expected_version
                        .checked_add(1)
                        .ok_or_else(|| ResourceError::Storage {
                            reason: "resource governor version exhausted".to_owned(),
                        })?;
                let client = self.pool.get().await.map_err(map_pool_r)?;
                let rows = if expected_version == 0 {
                    // First writer wins. A stale initial reader must not replace
                    // another writer's already initialized snapshot.
                    client.execute(
                        "INSERT INTO brassclaw_resource_accounts \
                         (id, tenant_id, scope_kind, scope_id, period_key, reserved, consumed, version, payload) \
                         VALUES ($1, $2, 'tenant', $2, '__governor__', 0, 0, 1, $3) \
                         ON CONFLICT (tenant_id, scope_kind, scope_id, period_key) DO NOTHING",
                        &[&format!("governor:{}", self.tenant_id), &self.tenant_id, &payload],
                    ).await.map_err(map_pg_r)?
                } else {
                    // A disappeared row cannot be re-created by a stale writer.
                    client.execute(
                        "UPDATE brassclaw_resource_accounts SET payload = $2, version = $3, updated_at = now() \
                         WHERE tenant_id = $1 AND scope_kind = 'tenant' AND scope_id = $1 \
                         AND period_key = '__governor__' AND version = $4",
                        &[&self.tenant_id, &payload, &next_version, &expected_version],
                    ).await.map_err(map_pg_r)?
                };
                Ok(rows > 0)
            },
            |reason| ResourceError::Storage {
                reason: reason.to_owned(),
            },
        )
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

    // Every caller verifies the indexed envelope before trusting the payload.
    // PostgreSQL timestamps have microsecond precision; match the driver's
    // truncation relative to its 2000 epoch, including dates before that epoch.
    fn gate_from_row(row: &tokio_postgres::Row) -> Result<BudgetApprovalGate, BudgetGateError> {
        let gate: BudgetApprovalGate =
            serde_json::from_value(row.get("payload")).map_err(map_json_b)?;
        let amount: String = row.get("requested_amount");
        let amount = Decimal::from_str_exact(&amount).map_err(|_| BudgetGateError::Storage {
            reason: "invalid indexed budget amount".into(),
        })?;
        let expected_amount = match &gate.needed.requested {
            crate::ResourceValue::Decimal(value) => *value,
            crate::ResourceValue::Integer(value) => Decimal::from(*value),
        };
        let epoch = DateTime::from_timestamp(946_684_800, 0).expect("valid PostgreSQL epoch");
        let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at").map_err(map_pg_b)?;
        let expected_expiry = gate
            .expires_at
            .signed_duration_since(epoch)
            .num_microseconds();
        if row.get::<_, String>("id") != gate.id.to_string()
            || row.get::<_, String>("status") != Self::status_kind_str(&gate.status)
            || row.get::<_, String>("gate_kind") != gate.needed.dimension.to_string()
            || amount != expected_amount
            || expected_expiry.is_none()
            || expires_at.and_then(|value| value.signed_duration_since(epoch).num_microseconds())
                != expected_expiry
        {
            return Err(BudgetGateError::Storage {
                reason: "inconsistent budget gate envelope".into(),
            });
        }
        Ok(gate)
    }

    fn read_gate_sync(
        &self,
        id: BudgetGateId,
    ) -> Result<Option<BudgetApprovalGate>, BudgetGateError> {
        blocking_pg(
            async {
                let client = self.pool.get().await.map_err(map_pool_b)?;
                let row = client
                    .query_opt(
                        "SELECT id, payload, status, expires_at, gate_kind, requested_amount::text \
                         FROM brassclaw_budget_gates \
                         WHERE id = $1 AND tenant_id = $2",
                        &[&id.as_uuid().to_string(), &self.tenant_id],
                    )
                    .await
                    .map_err(map_pg_b)?;
                row.as_ref().map(Self::gate_from_row).transpose()
            },
            |reason| BudgetGateError::Storage {
                reason: reason.to_owned(),
            },
        )
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
    fn open(
        &self,
        _scope: &ResourceScope,
        gate: BudgetApprovalGate,
    ) -> Result<(), BudgetGateError> {
        blocking_pg(
            async {
                if gate.status != BudgetGateStatus::Pending {
                    return Err(BudgetGateError::Storage {
                        reason: "only pending budget gates can be opened".into(),
                    });
                }
                let payload = serde_json::to_value(&gate).map_err(map_json_b)?;
                // Use Display (not Debug) — Debug gives "InputTokens", Display gives "input_tokens".
                let gate_kind = gate.needed.dimension.to_string();
                let requested_amount = match &gate.needed.requested {
                    crate::ResourceValue::Decimal(d) => d.to_string(),
                    crate::ResourceValue::Integer(i) => i.to_string(),
                };
                let client = self.pool.get().await.map_err(map_pool_b)?;
                let inserted = client
                    .execute(
                        "INSERT INTO brassclaw_budget_gates \
                         (id, tenant_id, gate_kind, status, requested_amount, payload, \
                          expires_at) \
                         VALUES ($1, $2, $3, 'pending', $4::text::numeric, $5, $6) \
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
                if inserted == 0 {
                    let row = client
                        .query_opt(
                            "SELECT id, payload, status, expires_at, gate_kind, requested_amount::text \
                             FROM brassclaw_budget_gates WHERE id=$1 AND tenant_id=$2",
                            &[&gate.id.to_string(), &self.tenant_id],
                        )
                        .await
                        .map_err(map_pg_b)?;
                    let existing = row.as_ref().map(Self::gate_from_row).transpose()?;
                    if existing.as_ref() != Some(&gate) {
                        return Err(BudgetGateError::Storage {
                            reason: "budget gate identifier conflict".into(),
                        });
                    }
                }
                Ok(())
            },
            |reason| BudgetGateError::Storage {
                reason: reason.to_owned(),
            },
        )
    }

    fn resolve(
        &self,
        _scope: &ResourceScope,
        id: BudgetGateId,
        outcome: BudgetGateOutcome,
        at: DateTime<Utc>,
    ) -> Result<BudgetApprovalGate, BudgetGateError> {
        blocking_pg(
            async {
                let mut client = self.pool.get().await.map_err(map_pool_b)?;
                let transaction = client.transaction().await.map_err(map_pg_b)?;
                let row = transaction
                    .query_opt(
                        "SELECT id, payload, status, expires_at, gate_kind, requested_amount::text \
                     FROM brassclaw_budget_gates WHERE id=$1 AND tenant_id=$2 FOR UPDATE",
                        &[&id.to_string(), &self.tenant_id],
                    )
                    .await
                    .map_err(map_pg_b)?
                    .ok_or(BudgetGateError::Unknown { id })?;
                let mut gate = Self::gate_from_row(&row)?;
                if gate.status.is_terminal() {
                    return Err(BudgetGateError::AlreadyResolved { id });
                }
                gate.status = match outcome {
                    BudgetGateOutcome::Approve {
                        increased_limit,
                        by,
                    } => BudgetGateStatus::Approved {
                        increased_limit,
                        by,
                        at,
                    },
                    BudgetGateOutcome::Cancel { by } => BudgetGateStatus::Cancelled { by, at },
                };
                let new_status_str = Self::status_kind_str(&gate.status);
                let payload = serde_json::to_value(&gate).map_err(map_json_b)?;
                let updated = transaction
                    .execute(
                        "UPDATE brassclaw_budget_gates \
                         SET status = $1, payload = $2, updated_at = now() \
                         WHERE id = $3 AND tenant_id = $4 AND status = 'pending'",
                        &[
                            &new_status_str,
                            &payload,
                            &id.as_uuid().to_string(),
                            &self.tenant_id,
                        ],
                    )
                    .await
                    .map_err(map_pg_b)?;
                if updated != 1 {
                    return Err(BudgetGateError::AlreadyResolved { id });
                }
                transaction.commit().await.map_err(map_pg_b)?;
                Ok(gate)
            },
            |reason| BudgetGateError::Storage {
                reason: reason.to_owned(),
            },
        )
    }

    fn expire_pending_older_than(
        &self,
        _scope: &ResourceScope,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<BudgetApprovalGate>, BudgetGateError> {
        blocking_pg(
            async {
                let mut client = self.pool.get().await.map_err(map_pool_b)?;
                let transaction = client.transaction().await.map_err(map_pg_b)?;
                // Lock the exact set being expired, so a concurrent approval cannot
                // be overwritten and returned payloads match durable terminal state.
                let rows = transaction
                    .query(
                        "SELECT id, payload, status, expires_at, gate_kind, requested_amount::text
                 FROM brassclaw_budget_gates
                 WHERE tenant_id=$1 AND status='pending' AND expires_at <= $2
                 ORDER BY id FOR UPDATE",
                        &[&self.tenant_id, &cutoff],
                    )
                    .await
                    .map_err(map_pg_b)?;
                let mut gates = Vec::with_capacity(rows.len());
                for row in rows {
                    let mut gate = Self::gate_from_row(&row)?;
                    // SQL can select the microsecond containing a later exact
                    // payload deadline. Leave that gate pending until eligible.
                    if gate.expires_at > cutoff {
                        continue;
                    }
                    gate.status = BudgetGateStatus::Expired { at: cutoff };
                    let payload = serde_json::to_value(&gate).map_err(map_json_b)?;
                    let updated = transaction.execute(
                    "UPDATE brassclaw_budget_gates SET status='expired', payload=$1, updated_at=now()
                     WHERE id=$2 AND tenant_id=$3 AND status='pending'",
                    &[&payload, &gate.id.to_string(), &self.tenant_id],
                ).await.map_err(map_pg_b)?;
                    if updated != 1 {
                        return Err(BudgetGateError::AlreadyResolved { id: gate.id });
                    }
                    gates.push(gate);
                }
                transaction.commit().await.map_err(map_pg_b)?;
                Ok(gates)
            },
            |reason| BudgetGateError::Storage {
                reason: reason.to_owned(),
            },
        )
    }

    fn get(
        &self,
        _scope: &ResourceScope,
        id: BudgetGateId,
    ) -> Result<Option<BudgetApprovalGate>, BudgetGateError> {
        self.read_gate_sync(id)
    }

    fn list_pending(
        &self,
        _scope: &ResourceScope,
    ) -> Result<Vec<BudgetApprovalGate>, BudgetGateError> {
        blocking_pg(
            async {
                let client = self.pool.get().await.map_err(map_pool_b)?;
                let rows = client
                    .query(
                        "SELECT id, payload, status, expires_at, gate_kind, requested_amount::text \
                         FROM brassclaw_budget_gates \
                         WHERE tenant_id = $1 AND status = 'pending' \
                         ORDER BY created_at ASC",
                        &[&self.tenant_id],
                    )
                    .await
                    .map_err(map_pg_b)?;
                rows.into_iter()
                    .map(|row| Self::gate_from_row(&row))
                    .collect::<Result<_, _>>()
            },
            |reason| BudgetGateError::Storage {
                reason: reason.to_owned(),
            },
        )
    }
}
